// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A host that relays to another host (ADR 0078 §4: *"The window reaches a runner through its
//! own `local-ui` connection to its own host, which relays"*, #2042).
//!
//! **Every client link gets a link of its own to the other host**, made with the client's first
//! command and let go of with the client's link. Each command is sent on as it came, and
//! answered with the other host's answer, or its refusal, word for word. Each view's stream the
//! other host opens is joined to a stream on the client's link, byte for byte both ways
//! (snapshot, live bytes one way, acknowledgements the other) under the header the relay writes
//! for it, so the client draws and acknowledges it as one the host opened itself. What the
//! other host pushes reaches the client once it subscribes.
//!
//! **The link to the other host is kept** (#2215, ADR 0078 §2): a relay made with
//! [`Relay::kept`] keeps it through
//! [`keep_linked`](purlis_session_protocol::hostlink::keep_linked), so a link that drops comes
//! back on its own and one renewed ahead of its limit is replaced by the next, with the same
//! pauses and renewal as any kept link, and to the host pinned for it alone. Over each new link
//! the relay carries over what the client's link stands on, and nothing else: it holds the
//! projects the client holds, subscribes again from the client's cursor (the last event it
//! handed the client, so none comes twice), and attaches the client's views again, each with a
//! snapshot, under the numbers the client knows them by and at their next epoch, so the client
//! draws each again as a view that came back. A link renewed ahead of its limit carries the
//! client's commands until the new one carries all of that, and only then ends; after a drop,
//! the client's commands wait while it is carried over. **No command the client sent is sent
//! again**: one the link ended under is refused as unanswered, and while no link is up every
//! command is refused in words saying so, and is not kept. A view the other host will not
//! attach again has ended for good, and the client is told so. What the other host named to the
//! link that ended, a chat's confinement among them, is not named again by this: the other host
//! names a link only what that link made or took back (#2511).
//!
//! **What only the window may send, the relay sends on for the window alone** (ruled
//! 2026-10-15, #2042): holding a project, dealing its numbers, a start with the window's own
//! `launch`, the window's reply to a line a chat's hooks wrote, and a chat's confinement (its
//! start, the window's answers to its asks, a port withdrawn from it, #2217, and a start that
//! names the confinement the other host composes for it, #2407) cross only for a
//! `local-ui` client, whatever else this host's grants let a client send; and what only the
//! window hears (a held project's hook lines, how saving it goes, what a chat's sandbox did)
//! reaches only a `local-ui` client. This is the one place that rule is enforced: the other
//! host cannot tell a relayed command from one its peer composed, and trusts its peer, this
//! host, as it trusts the window. A start marked as opting out of the sandbox, and a launch
//! that names a harness, are refused by the other host from any scope but its own window's, so
//! one sent on is refused there: a runner composes its harness chats by their names, with its
//! own sandbox. Everything else the other host decides: it serves its link
//! as the scope that link was admitted as, `remote-link` for the link between hosts, and its
//! own check of that scope's grants (FD-27) refuses what the scope may not send, before its
//! host hears of it; the client is told that refusal.

use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::time::Duration;

use futures::future::BoxFuture;
use purlis_session_protocol::auth::Scope;
use purlis_session_protocol::grants::Power;
use purlis_session_protocol::hostlink::{Linked, Links};
use purlis_session_protocol::link::{Acceptor, Link, Opener, Stream};
use purlis_session_protocol::session::{
    self, Answer, Attached, CallError, Client, Command, Events, Peer, Pushed, Refusal, Since, Told,
    ViewEnd, ViewKind, ViewRole,
};
use purlis_session_protocol::view::{Epoch, Header, ViewId};
use tokio::io::AsyncWriteExt;
use tokio::sync::{Notify, mpsc, oneshot, watch};
use tokio::task::AbortHandle;

/// How a relay reaches the other host once: a new link to it, or why there is none.
pub type Dial = Arc<dyn Fn() -> BoxFuture<'static, Result<Link, String>> + Send + Sync>;

/// How a relay keeps a link to the other host: each call keeps one more, for one client link,
/// as [`keep_linked`](purlis_session_protocol::hostlink::keep_linked) does (to the host pinned
/// for it, linked again when it drops, renewed ahead of its limit), and dropping what it
/// answered lets that link go.
pub type Keep = Arc<dyn Fn() -> Links + Send + Sync>;

/// One client link's links to the other host, told in order, until no more will come.
trait Taken: Send {
    fn next(&mut self) -> BoxFuture<'_, Option<Linked>>;

    /// The relay has moved to the last link it was told is up.
    fn moved(&self);
}

impl Taken for Links {
    fn next(&mut self) -> BoxFuture<'_, Option<Linked>> {
        Box::pin(Links::next(self))
    }

    fn moved(&self) {
        Links::moved(self);
    }
}

/// One dial's link, or why there is none, and then no more.
struct Dialed(Option<oneshot::Receiver<Result<Link, String>>>);

impl Taken for Dialed {
    fn next(&mut self) -> BoxFuture<'_, Option<Linked>> {
        Box::pin(async move {
            let made = self.0.take()?.await;
            Some(match made {
                Ok(Ok(link)) => Linked::Up(Box::new(link)),
                Ok(Err(why)) => Linked::Down(why),
                Err(_) => Linked::Down("the dial was let go of".into()),
            })
        })
    }

    fn moved(&self) {}
}

/// What a relay takes each client link's links to the other host from.
type Source = Arc<dyn Fn() -> Box<dyn Taken> + Send + Sync>;

/// The most pushed events the relay holds for a client that has not read them.
const MOST_PUSHED_QUEUED: usize = 1024;

/// How long a view's stream the other host opened waits for its attach's answer before it is
/// let go of: the link's own deadline for hearing nothing (ADR 0078, amended 2026-10-14).
const PARKED_FOR: Duration = Duration::from_secs(30);

/// What a relay keeps for each client link, by the client link's number.
type ByLink = Arc<Mutex<HashMap<u64, Arc<Relayed>>>>;

/// Relays every client link's commands to the other host its links reach. A clone is the same
/// relay.
#[derive(Clone)]
pub struct Relay {
    source: Source,
    links: ByLink,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl Relay {
    /// A relay that dials one link to the other host for each client link, with its first
    /// command. A link that ends is dialed again with the next command after it, and carries
    /// over what the client's link stands on as a kept link does.
    pub fn new(dial: Dial) -> Relay {
        Relay::from_source(Arc::new(move || {
            // The dial runs on a task of its own, so a client link that ends while it dials
            // lets go of the link it makes once it is made, rather than cutting it off halfway.
            let (made, link) = oneshot::channel();
            let dialing = dial();
            tokio::spawn(async move {
                let _ = made.send(dialing.await);
            });
            Box::new(Dialed(Some(link)))
        }))
    }

    /// A relay that keeps a link to the other host for each client link through `keep`, from
    /// the client's first command until its link ends (#2215).
    pub fn kept(keep: Keep) -> Relay {
        Relay::from_source(Arc::new(move || Box::new(keep())))
    }

    fn from_source(source: Source) -> Relay {
        Relay {
            source,
            links: Arc::default(),
        }
    }

    /// What the relay keeps for `peer`'s link, made with its first command. A link that has
    /// ended gets nothing: a call it sent that is carried out after its end makes nothing, and
    /// leaves nothing behind.
    fn relayed(&self, peer: &Peer) -> Result<Arc<Relayed>, Refusal> {
        if peer.has_ended() {
            return Err(ended());
        }
        let relayed = Arc::clone(lock(&self.links).entry(peer.link()).or_insert_with(|| {
            Arc::new(Relayed::start(
                Arc::clone(&self.source),
                peer.opener().clone(),
                peer.scope(),
            ))
        }));
        // The link ended while this one was made: what it made goes with it.
        if peer.has_ended() {
            lock(&self.links).remove(&peer.link());
            return Err(ended());
        }
        Ok(relayed)
    }
}

fn ended() -> Refusal {
    Refusal {
        code: session::NOT_STARTED.into(),
        why: "the link this was sent on has ended".into(),
    }
}

/// Where the link to the other host is, as a client link's commands find it.
#[derive(Clone)]
enum Standing {
    /// A link is coming up, or what the client's link stands on is being carried over to it.
    Linking,
    /// A link is up, and carries what the client's link stands on.
    Up(Arc<Upstream>),
    /// No link is up, and why.
    Down(String),
    /// No link is up and no more will come of the links being taken: the next command takes
    /// links anew.
    Over,
}

/// What the relay keeps for one client link.
struct Relayed {
    client: Arc<ClientLink>,
    standing: watch::Receiver<Standing>,
    /// A command found the links over: take links anew.
    again: Arc<Notify>,
    keeper: AbortHandle,
}

impl Drop for Relayed {
    fn drop(&mut self) {
        // What was being dialed is let go of once it is made, by the source's own task.
        self.keeper.abort();
    }
}

impl Relayed {
    fn start(source: Source, opener: Opener, scope: Scope) -> Relayed {
        let client = Arc::new(ClientLink {
            scope,
            opener,
            kept: Mutex::default(),
            handing: tokio::sync::Mutex::new(()),
            changing: watch::channel(0).0,
            parked: AtomicU64::new(0),
        });
        let (tell, standing) = watch::channel(Standing::Linking);
        let again = Arc::new(Notify::new());
        let keeper = tokio::spawn(keep(source, Arc::clone(&client), tell, Arc::clone(&again)))
            .abort_handle();
        Relayed {
            client,
            standing,
            again,
            keeper,
        }
    }

    /// The link that is up and carries what the client's link stands on; or the refusal that
    /// says no link is up. A client link that ended while it waited is refused, so nothing it
    /// sent is sent on.
    async fn up(&self, peer: &Peer) -> Result<Arc<Upstream>, Refusal> {
        let mut standing = self.standing.clone();
        loop {
            match standing.borrow_and_update().clone() {
                Standing::Up(_) if peer.has_ended() => return Err(ended()),
                Standing::Up(upstream) => return Ok(upstream),
                Standing::Down(why) => return Err(not_linked(&why)),
                Standing::Over => self.again.notify_one(),
                Standing::Linking => {}
            }
            if standing.changed().await.is_err() {
                return Err(ended());
            }
        }
    }
}

fn not_linked(why: &str) -> Refusal {
    Refusal {
        code: session::NOT_STARTED.into(),
        why: format!(
            "Runner not linked ({why}). purlis is linking to it again and sent it nothing: send \
             this again once the runner is back"
        ),
    }
}

/// Take `source`'s links for a client link, one after another, and carry what the client's
/// link stands on over to each that comes up. When no more come, wait for a command to ask for
/// links again.
async fn keep(
    source: Source,
    client: Arc<ClientLink>,
    tell: watch::Sender<Standing>,
    again: Arc<Notify>,
) {
    loop {
        let mut links = source();
        let mut up: Option<Arc<Upstream>> = None;
        // A link renewed ahead of its limit, which still answers what was sent on it.
        let mut retiring: Option<Arc<Upstream>> = None;
        let mut no_more = false;
        while !(no_more && up.is_none()) {
            let link_ended = async {
                match &up {
                    Some(upstream) => upstream.ended.notified().await,
                    None => std::future::pending().await,
                }
            };
            let drained = async {
                match &retiring {
                    Some(upstream) => upstream.drained().await,
                    None => std::future::pending().await,
                }
            };
            tokio::select! {
                linked = links.next(), if !no_more => match linked {
                    Some(Linked::Up(link)) => {
                        // A renewal leaves the link before it carrying commands while this is
                        // carried over; after a drop, commands wait for it.
                        if up.is_none() {
                            tell.send_replace(Standing::Linking);
                        }
                        match carry_over(*link, &client, &tell).await {
                            Some(upstream) => {
                                // The link it replaces ends once what was sent on it is
                                // answered: the kept link is told the relay has moved then.
                                retiring = up.replace(Arc::clone(&upstream));
                                tell.send_replace(Standing::Up(upstream));
                                if retiring.is_none() {
                                    links.moved();
                                }
                            }
                            None => {
                                up = None;
                                retiring = None;
                                tell.send_replace(Standing::Down(
                                    "the link to it ended as it came up".into(),
                                ));
                            }
                        }
                    }
                    Some(Linked::Down(why)) => {
                        up = None;
                        retiring = None;
                        tell.send_replace(Standing::Down(why));
                    }
                    None => no_more = true,
                },
                () = drained => {
                    retiring = None;
                    links.moved();
                }
                () = link_ended => {
                    up = None;
                    tell.send_replace(Standing::Down("the link to it ended".into()));
                }
            }
        }
        tell.send_replace(Standing::Over);
        again.notified().await;
        tell.send_replace(Standing::Linking);
    }
}

/// A client link, as the relay keeps it: who it is, and what it stands on, which the relay
/// carries over to each new link to the other host.
struct ClientLink {
    scope: Scope,
    opener: Opener,
    kept: Mutex<Kept>,
    /// Held while an event is handed to the client, so the cursor moves only once it has been.
    handing: tokio::sync::Mutex<()>,
    /// How many commands that change what the client's link stands on are being carried out.
    changing: watch::Sender<usize>,
    /// The number the next view stream that waits for its attach is given.
    parked: AtomicU64,
}

/// What a client link stands on at the other host.
#[derive(Default)]
struct Kept {
    /// The projects it holds there, by id, with where each is.
    held: BTreeMap<String, String>,
    /// Where what the other host pushes goes: the client's subscription, while it has one.
    subscriber: Option<mpsc::Sender<Pushed>>,
    /// For each device, the last event handed to the client, or where it subscribed from.
    cursor: Since,
    /// Its views, by the number the client knows each by.
    views: BTreeMap<u32, KeptView>,
    /// The number the next view is given.
    last_view: u32,
}

/// One of a client's views: what attaches it again, and its last epoch on the client's link.
struct KeptView {
    program: String,
    kind: ViewKind,
    role: Option<ViewRole>,
    rate: Option<u32>,
    epoch: u32,
}

impl KeptView {
    fn attach(&self) -> Command {
        Command::AttachView {
            program: self.program.clone(),
            kind: self.kind,
            role: self.role,
            rate: self.rate,
        }
    }
}

/// One of what a count counts, while it lasts: a command in flight on a link, or one that
/// changes what a client link stands on, which a carry-over waits for so it carries what it
/// changed.
struct Counted<'a>(&'a watch::Sender<usize>);

impl<'a> Counted<'a> {
    fn start(count: &'a watch::Sender<usize>) -> Counted<'a> {
        count.send_modify(|count| *count += 1);
        Counted(count)
    }
}

impl Drop for Counted<'_> {
    fn drop(&mut self) {
        self.0.send_modify(|count| *count -= 1);
    }
}

impl ClientLink {
    fn kept(&self) -> MutexGuard<'_, Kept> {
        lock(&self.kept)
    }

    /// Hand `pushed` to the client's subscription, while it has one.
    async fn tell(&self, pushed: Pushed) {
        let Some(target) = self.kept().subscriber.clone() else {
            return;
        };
        if target.send(pushed).await.is_err() {
            self.kept().subscriber = None;
        }
    }

    /// Hand an event to the client's subscription unless it was handed one at or after it,
    /// over this link or one before; the cursor moves once it has been handed.
    async fn hand_event(&self, event: session::Event) {
        let _handing = self.handing.lock().await;
        let (target, device, seq) = {
            let kept = self.kept();
            let Some(target) = kept.subscriber.clone() else {
                return;
            };
            if kept
                .cursor
                .get(&event.device_id)
                .is_some_and(|last| event.seq <= *last)
            {
                return;
            }
            (target, event.device_id.clone(), event.seq)
        };
        if target.send(Pushed::Event(event)).await.is_err() {
            self.kept().subscriber = None;
            return;
        }
        self.kept().cursor.insert(device, seq);
    }

    /// Join `from`, a view's stream the other host opened, after its header, to a new stream on
    /// the client's link for its view `view`, at that view's next epoch.
    async fn join(&self, mut from: Stream, view: u32, snapshot: u32) {
        let Some(epoch) = self.kept().views.get_mut(&view).and_then(|kept| {
            kept.epoch = kept.epoch.checked_add(1)?;
            Some(kept.epoch)
        }) else {
            return;
        };
        let Ok(mut to) = self.opener.open().await else {
            return;
        };
        let header = Header {
            view: ViewId(view),
            epoch: Epoch(epoch),
            snapshot,
        };
        if to.write_all(&header.to_bytes()).await.is_err() {
            return;
        }
        let _ = tokio::io::copy_bidirectional(&mut from, &mut to).await;
    }
}

/// One link to the other host.
struct Upstream {
    client: Client,
    /// How many of the client's commands are in flight on this link.
    calls: watch::Sender<usize>,
    joins: Arc<Mutex<Joins>>,
    /// The link has ended: what the other host pushed on it has all been handed on.
    ended: Arc<Notify>,
    tasks: Vec<AbortHandle>,
}

impl Drop for Upstream {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
    }
}

/// Which client view each view the other host opened on one link is, and each stream it opened
/// before its attach was answered, waiting to be joined.
#[derive(Default)]
struct Joins {
    /// The other host's number for a view on this link, to the client's.
    of: HashMap<u32, u32>,
    /// By the other host's number: which wait it is, the stream, and its snapshot's length.
    parked: HashMap<u32, (u64, Stream, u32)>,
}

impl Upstream {
    fn on(link: Link, client: &Arc<ClientLink>) -> Arc<Upstream> {
        Arc::new_cyclic(|upstream: &Weak<Upstream>| {
            let (to_the_other_host, views, events) = Client::new(link);
            let joins: Arc<Mutex<Joins>> = Arc::default();
            let ended = Arc::new(Notify::new());
            let joining = tokio::spawn(join_views(views, Arc::clone(client), Arc::clone(&joins)))
                .abort_handle();
            let pushing = tokio::spawn({
                let (client, joins, ended, upstream) = (
                    Arc::clone(client),
                    Arc::clone(&joins),
                    Arc::clone(&ended),
                    upstream.clone(),
                );
                async move {
                    push_on(events, &client, &joins, &upstream).await;
                    ended.notify_one();
                }
            })
            .abort_handle();
            Upstream {
                client: to_the_other_host,
                calls: watch::channel(0).0,
                joins,
                ended,
                tasks: vec![joining, pushing],
            }
        })
    }

    /// Send the client's `command` on this link, counted in flight until it is answered.
    async fn call(&self, command: Command) -> Result<Answer, Refusal> {
        let _in_flight = Counted::start(&self.calls);
        self.client.call(command).await.map_err(|why| refused(&why))
    }

    /// Once no command of the client's is in flight on this link.
    async fn drained(&self) {
        let _ = self.calls.subscribe().wait_for(|calls| *calls == 0).await;
    }

    /// The other host's view `theirs` on this link is the client's `ours`: join the stream it
    /// opened for it, if it came first.
    fn joined(&self, theirs: u32, ours: u32, client: &Arc<ClientLink>) {
        let parked = {
            let mut joins = lock(&self.joins);
            joins.of.insert(theirs, ours);
            joins.parked.remove(&theirs)
        };
        if let Some((_, stream, snapshot)) = parked {
            let client = Arc::clone(client);
            tokio::spawn(async move { client.join(stream, ours, snapshot).await });
        }
    }

    /// The other host's number on this link for the client's view `ours`, let go of.
    fn let_go(&self, ours: u32) -> Option<u32> {
        let mut joins = lock(&self.joins);
        let theirs = joins
            .of
            .iter()
            .find_map(|(theirs, of)| (*of == ours).then_some(*theirs))?;
        joins.of.remove(&theirs);
        Some(theirs)
    }
}

/// What a carry-over has carried to its new link so far.
#[derive(Default)]
struct Carried {
    held: BTreeMap<String, String>,
    subscribed: bool,
    /// Each view carried, by the client's number, with its program.
    views: BTreeMap<u32, String>,
}

/// Why a carry-over stopped: the new link ended.
struct LinkEnded;

/// Carry what `client`'s link stands on over to `link`, new to the other host: hold its
/// projects, subscribe from its cursor, and attach its views again. The link before it, if
/// one is still up, carries commands while the first pass is made; then commands wait, the
/// commands still changing what the client stands on are let finish, and what they changed is
/// carried too. `None` when the new link ends first.
async fn carry_over(
    link: Link,
    client: &Arc<ClientLink>,
    tell: &watch::Sender<Standing>,
) -> Option<Arc<Upstream>> {
    let upstream = Upstream::on(link, client);
    let mut carried = Carried::default();
    carry(&upstream, client, &mut carried).await.ok()?;
    tell.send_replace(Standing::Linking);
    let _ = client
        .changing
        .subscribe()
        .wait_for(|changing| *changing == 0)
        .await;
    carry(&upstream, client, &mut carried).await.ok()?;
    Some(upstream)
}

/// One pass of a carry-over: what the client's link stands on that is not yet carried to
/// `upstream`. A view the other host will not attach again has ended for good, and the client is
/// told so; one the client let go of since it was carried is let go of there too.
async fn carry(
    upstream: &Arc<Upstream>,
    client: &Arc<ClientLink>,
    carried: &mut Carried,
) -> Result<(), LinkEnded> {
    fn closed<T>(result: &Result<T, CallError>) -> bool {
        matches!(result, Err(CallError::Closed(_)))
    }
    let (held, subscribe, attach, detach) = {
        let kept = client.kept();
        let held: Vec<(String, String)> = kept
            .held
            .iter()
            .filter(|(project, path)| carried.held.get(*project) != Some(*path))
            .map(|(project, path)| (project.clone(), path.clone()))
            .collect();
        let subscribe = kept
            .subscriber
            .as_ref()
            .filter(|subscriber| !subscriber.is_closed() && !carried.subscribed)
            .map(|_| kept.cursor.clone());
        let attach: Vec<(u32, String, Command)> = kept
            .views
            .iter()
            .filter(|(ours, _)| !carried.views.contains_key(ours))
            .map(|(ours, view)| (*ours, view.program.clone(), view.attach()))
            .collect();
        let detach: Vec<u32> = carried
            .views
            .keys()
            .filter(|ours| !kept.views.contains_key(ours))
            .copied()
            .collect();
        (held, subscribe, attach, detach)
    };
    for (project, path) in held {
        let held = upstream.client.hold_project(&project, &path).await;
        if closed(&held) {
            return Err(LinkEnded);
        }
        carried.held.insert(project, path);
    }
    if let Some(cursor) = subscribe {
        let subscribed = upstream.client.subscribe(cursor).await;
        if closed(&subscribed) {
            return Err(LinkEnded);
        }
        carried.subscribed = true;
    }
    for (ours, program, attach) in attach {
        let attached = upstream.client.call(attach).await;
        if closed(&attached) {
            return Err(LinkEnded);
        }
        carried.views.insert(ours, program);
        if let Ok(Answer::Attached(attached)) = attached {
            upstream.joined(attached.view, ours, client);
        } else {
            client.kept().views.remove(&ours);
            client
                .tell(Pushed::ViewEnded {
                    view: ours,
                    why: ViewEnd::Closed,
                })
                .await;
        }
    }
    for ours in detach {
        let program = carried.views.remove(&ours);
        if let (Some(theirs), Some(program)) = (upstream.let_go(ours), program) {
            let _ = upstream
                .client
                .call(Command::DetachView {
                    program,
                    view: theirs,
                })
                .await;
        }
    }
    Ok(())
}

/// Joins each view's stream the other host opens on one link to a new stream on the client's
/// link, under the client's number for that view. A stream whose attach is not answered yet
/// waits for it, for [`PARKED_FOR`] at the most.
async fn join_views(mut views: Acceptor, client: Arc<ClientLink>, joins: Arc<Mutex<Joins>>) {
    while let Ok(mut from) = views.accept().await {
        let (client, joins) = (Arc::clone(&client), Arc::clone(&joins));
        tokio::spawn(async move {
            let Ok(header) = Header::read_from(&mut from).await else {
                return;
            };
            let theirs = header.view.0;
            // Its attach is answered already, or it waits for that, for so long.
            let parked = {
                let mut joined = lock(&joins);
                match joined.of.get(&theirs).copied() {
                    Some(ours) => Ok((ours, from)),
                    None => {
                        let wait = client.parked.fetch_add(1, Ordering::Relaxed);
                        joined.parked.insert(theirs, (wait, from, header.snapshot));
                        Err(wait)
                    }
                }
            };
            let (ours, from) = match parked {
                Ok(joining) => joining,
                Err(wait) => {
                    tokio::time::sleep(PARKED_FOR).await;
                    let mut joins = lock(&joins);
                    if joins
                        .parked
                        .get(&theirs)
                        .is_some_and(|(parked, _, _)| *parked == wait)
                    {
                        joins.parked.remove(&theirs);
                    }
                    return;
                }
            };
            client.join(from, ours, header.snapshot).await;
        });
    }
}

/// Hands what the other host pushes on one link to the client's subscription, while it has one:
/// what only the window hears reaches it only where the client's link is the window's; an
/// event it was handed already, over this link or one before, is not handed again; a view that
/// ended is named by the client's number for it; and where the relay fell behind the other host
/// it subscribes again there from the client's cursor, so no event is missed in silence.
async fn push_on(
    mut events: Events,
    client: &ClientLink,
    joins: &Mutex<Joins>,
    upstream: &Weak<Upstream>,
) {
    while let Some(told) = events.recv().await {
        let pushed = match told {
            Told::Pushed(pushed) => pushed,
            Told::FellBehind { .. } => {
                let cursor = {
                    let kept = client.kept();
                    kept.subscriber.as_ref().map(|_| kept.cursor.clone())
                };
                if let (Some(cursor), Some(upstream)) = (cursor, upstream.upgrade()) {
                    let _ = upstream.client.subscribe(cursor).await;
                }
                continue;
            }
        };
        if client.scope != Scope::LocalUi && heard_by_the_window_alone(&pushed) {
            continue;
        }
        match pushed {
            Pushed::Event(event) => client.hand_event(event).await,
            Pushed::ViewEnded { view, why } => {
                let Some(ours) = lock(joins).of.remove(&view) else {
                    continue;
                };
                client.kept().views.remove(&ours);
                client.tell(Pushed::ViewEnded { view: ours, why }).await;
            }
            pushed => client.tell(pushed).await,
        }
    }
}

/// Whether `command` is one only the window sends: what the relay sends on for a `local-ui`
/// client alone.
fn the_windows_alone(command: &Command) -> bool {
    match command {
        Command::StartProgram(start) => {
            start.launch.is_some() || start.without_sandbox || start.confinement.is_some()
        }
        command => the_windows_power(command.power()),
    }
}

/// Whether `power` is one only the window holds of those the relay meets in a command or a
/// push: holding a project, replying to its hook lines, saving it, and confining a chat.
fn the_windows_power(power: Power) -> bool {
    matches!(
        power,
        Power::Hold | Power::Heard | Power::Save | Power::Confine
    )
}

/// Whether `pushed` is one only the window hears: a held project's hook lines, how saving it
/// goes, and what a chat's sandbox did. The other host pushes them to the relay's link for
/// every client, as it pushes them to a link that may hear them; only a `local-ui` client is
/// handed them.
fn heard_by_the_window_alone(pushed: &Pushed) -> bool {
    pushed.heard_with().is_some_and(the_windows_power)
}

fn refused(why: &CallError) -> Refusal {
    why.refusal().cloned().unwrap_or_else(|| Refusal {
        code: session::NOT_STARTED.into(),
        why: format!("the host this one relays to did not answer: {why}"),
    })
}

impl session::Host for Relay {
    async fn call(&self, command: Command, peer: &Peer) -> Result<Answer, Refusal> {
        if the_windows_alone(&command) && peer.scope() != Scope::LocalUi {
            return Err(Refusal {
                code: session::NOT_ALLOWED.into(),
                why: format!(
                    "`{}` is relayed to another host for purlis's window only, and this link is \
                     `{}`",
                    command.word(),
                    peer.scope()
                ),
            });
        }
        let relayed = self.relayed(peer)?;
        let client = &relayed.client;
        match command {
            Command::AttachView {
                program,
                kind,
                role,
                rate,
            } => {
                let upstream = relayed.up(peer).await?;
                let _changing = Counted::start(&client.changing);
                let view = KeptView {
                    program,
                    kind,
                    role,
                    rate,
                    epoch: 0,
                };
                let answer = upstream.call(view.attach()).await?;
                let Answer::Attached(attached) = answer else {
                    return Ok(answer);
                };
                let ours = {
                    let mut kept = client.kept();
                    kept.last_view = kept.last_view.checked_add(1).ok_or_else(|| Refusal {
                        code: session::NOT_STARTED.into(),
                        why: "this link has attached 4 billion views".into(),
                    })?;
                    let ours = kept.last_view;
                    kept.views.insert(ours, view);
                    ours
                };
                upstream.joined(attached.view, ours, client);
                Ok(Answer::Attached(Attached {
                    view: ours,
                    ..attached
                }))
            }
            Command::DetachView { program, view } => {
                // Let go of here first, so no link after this attaches it again. With no link
                // up the view is gone already: it went with the link, and comes on no other.
                client.kept().views.remove(&view);
                let upstream = match relayed.up(peer).await {
                    Ok(upstream) => upstream,
                    Err(_) if !peer.has_ended() => return Ok(Answer::Done),
                    Err(refusal) => return Err(refusal),
                };
                match upstream.let_go(view) {
                    Some(theirs) => {
                        upstream
                            .call(Command::DetachView {
                                program,
                                view: theirs,
                            })
                            .await
                    }
                    None => Ok(Answer::Done),
                }
            }
            Command::HoldProject {
                project,
                project_path,
            } => {
                let upstream = relayed.up(peer).await?;
                let _changing = Counted::start(&client.changing);
                let answer = upstream
                    .call(Command::HoldProject {
                        project: project.clone(),
                        project_path: project_path.clone(),
                    })
                    .await?;
                client.kept().held.insert(project, project_path);
                Ok(answer)
            }
            Command::LetGoOfProject { project } => {
                let upstream = relayed.up(peer).await?;
                let _changing = Counted::start(&client.changing);
                let answer = upstream
                    .call(Command::LetGoOfProject {
                        project: project.clone(),
                    })
                    .await?;
                client.kept().held.remove(&project);
                Ok(answer)
            }
            command => relayed.up(peer).await?.call(command).await,
        }
    }

    async fn link_ended(&self, peer: &Peer) {
        lock(&self.links).remove(&peer.link());
    }

    async fn subscribe(
        &self,
        since: Since,
        peer: &Peer,
    ) -> Result<mpsc::Receiver<Pushed>, Refusal> {
        let relayed = self.relayed(peer)?;
        let upstream = relayed.up(peer).await?;
        let _changing = Counted::start(&relayed.client.changing);
        let (told, pushed) = mpsc::channel(MOST_PUSHED_QUEUED);
        {
            let mut kept = relayed.client.kept();
            kept.subscriber = Some(told);
            kept.cursor = since.clone();
        }
        match upstream.call(Command::Subscribe { since }).await? {
            Answer::Subscribed => Ok(pushed),
            other => Err(Refusal {
                code: session::NOT_STARTED.into(),
                why: format!("the host this one relays to answered a subscribe with {other:?}"),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use purlis_session_protocol::session::{Event, Exit, SandboxTold, ViewEnd};

    /// One of each thing a host pushes.
    fn every_push() -> Vec<Pushed> {
        vec![
            Pushed::Event(Event {
                v: 1,
                device_id: "01J9ZZDEVICE0000000000000".into(),
                seq: 1,
                ulid: "01J9ZZEVENT00000000000000".into(),
                chat: None,
                run: None,
                parent_run: None,
                kind: "hook.stop".into(),
                body: serde_json::Value::Null,
            }),
            Pushed::Missed {
                device: "01J9ZZDEVICE0000000000000".into(),
                after: 1,
                resumes_at: 2,
            },
            Pushed::Ended {
                program: "01J9ZZPRGRM00000000000000".into(),
                run: None,
                exit: Exit::Code(0),
            },
            Pushed::ViewEnded {
                view: 1,
                why: ViewEnd::Closed,
            },
            Pushed::Heard {
                project: "01J9PRJCT00000000000000000".into(),
                heard: 1,
                line: serde_json::Value::Null,
            },
            Pushed::Sandbox {
                confinement: "01J9CNFNMNT0000000000000000".into(),
                told: SandboxTold::RefusedMore { count: 1 },
            },
            Pushed::Saving {
                project: "01J9PRJCT00000000000000000".into(),
                saved: true,
                moved_tree: false,
            },
            Pushed::WhoIsMidTurn {
                question: 1,
                project: "01J9PRJCT00000000000000000".into(),
                workspace: "alpha".into(),
            },
        ]
    }

    /// FD-27's table, as the relay meets it: a push reaches a client of the relay that is not
    /// the window exactly when some scope that is neither the window's nor a link between hosts
    /// may hear it on a host of its own.
    #[test]
    fn what_the_relay_keeps_from_a_client_that_is_not_the_window_is_what_the_table_grants_the_window_alone()
     {
        for pushed in every_push() {
            let anyones = pushed.heard_with().is_none_or(|power| {
                Scope::ALL
                    .into_iter()
                    .filter(|scope| !matches!(scope, Scope::LocalUi | Scope::RemoteLink))
                    .any(|scope| scope.may(power))
            });
            assert_eq!(!heard_by_the_window_alone(&pushed), anyones, "{pushed:?}");
        }
    }
}

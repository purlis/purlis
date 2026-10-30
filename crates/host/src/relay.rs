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
//! **The link to the other host is kept** (#2215, ADR 0078 §2): a relay made with [`Relay::kept`]
//! keeps it through [`keep_linked`](purlis_session_protocol::hostlink::keep_linked), so a link that
//! drops comes back on its own and one renewed ahead of its limit is replaced by the next, with the
//! same pauses and renewal as any kept link, and to the host pinned for it alone. Over each new
//! link the relay carries over what the client's link stands on, and nothing else: it holds the
//! projects the client holds, each where the client holds it now, subscribes again from the
//! client's cursor (the last event it handed the client, so none comes twice), and attaches the
//! client's views again, each with a snapshot, under the numbers the client knows them by and at
//! their next epoch, so the client draws each again as a view that came back. A hold is carried
//! once the other host takes it (#2535): a project it will not hold on the new link is no longer
//! kept for the client, and the client is pushed `not_held` for it, in the other host's words, as
//! the carry-over ends, so it holds the project again itself (#2645). A link renewed
//! ahead of its limit carries the client's commands until the new one carries all of that, and only
//! then ends; after a drop, the client's commands wait while it is carried over. **No command the
//! client sent is sent again**: one the link ended under is refused as unanswered, and while no
//! link is up every command is refused in words saying so, and is not kept. **The client is told
//! the link is down**, pushed `link_down` once each time no link is up, and `link_back` once one is
//! up again and carries what its link stands on (#2496): a link renewed ahead of its limit is
//! neither, and a relay made with [`Relay::new`] tells its client both as a kept one does. These,
//! and `not_held`, are this host's own words: the other host's are never handed on, and neither holds up linking again
//! for a client that has stopped reading. A view the other host will not attach again has ended for
//! good, and the client is told so.
//!
//! **The window takes back its own runner chats' sandboxes** (#2511, ruled 2026-10-17). The
//! relay gives each client link a name of its own, and on every link it makes to the other host
//! for that client it says the name as it carries over the projects held (`take_back`): the other
//! host names that link the confinements of the chats the client started over the links before,
//! made or composed for them, and no other. So after a re-link the window answers its runner
//! chats' asks and changes their reach again, Forgets included. The name is the relay's own
//! word: a client's `take_back` is refused.
//!
//! **A window that comes back takes back its runner chats too** (#2623). The window may name
//! itself on a link, as the link's first command (`name_window`), by the name it keeps for that
//! link across its restarts. The relay then names that client link to the other host by a digest
//! of that name ([`taken_back_as`]), the same after the window restarts, so the runner names the
//! window that comes back its chats again; a link that names itself otherwise, or not at all, is
//! given a name of its own and gets none of them. The name is never sent on, and never said: the
//! other host hears only the digest. While a client link holds a name no other link takes it,
//! and a link names itself before anything is sent on it, or not at all.
//!
//! **Code is relayed for the window alone** (1.34, #2046): `take_code` and `give_code` are sent
//! on for a `local-ui` client, and the code stream the other host opens for one is joined to a
//! stream on that client's link under the same header, byte for byte both ways, as it comes. A
//! code stream is never parked or carried over: a transfer the link ended under is refused, and
//! the window sends it again.
//!
//! **A runner chat's browser runs on the runner, relayed for the window alone** (1.38, #2050):
//! its start, its views, its chat's acts, the person driving it and a login purlis performs in
//! it are sent on for a `local-ui` client only. A login's values cross only as the window
//! decided to send them (the chat's persona opted in for that pool, the desktop connected), and
//! the runner keeps none of them. **A kept login is never relayed** (#2021): a browser's start
//! from kept logins, and a login that keeps one, are refused here, so a kept login stays on the
//! device it was made on.
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
use purlis_session_protocol::browsing::Secret;
use purlis_session_protocol::code;
use purlis_session_protocol::grants::Power;
use purlis_session_protocol::hostlink::{Down, Linked, Links};
use purlis_session_protocol::link::{Acceptor, Link, Opener, Stream};
use purlis_session_protocol::session::{
    self, Answer, Attached, CallError, Client, Command, Events, Peer, Pushed, Refusal, Since,
    StartProgram, Told, ViewEnd, ViewKind, ViewRole,
};
use purlis_session_protocol::view::{Epoch, Header, ViewId};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
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
                Ok(Err(why)) => Linked::Down(Down::because(why)),
                Err(_) => Linked::Down(Down::because("the dial was let go of")),
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

/// What each client link that named itself is named to the other host, by the client link's
/// number, until its first command after the name makes what the relay keeps for it (#2623).
type Named = Arc<Mutex<HashMap<u64, String>>>;

/// **The window each client link of a host's relays is named, by the client link's number**
/// (#2744): one for the whole host, shared by every relay its links make
/// ([`Relay::claiming`]), so while one link holds a window's name, no link of another relay of
/// the same host takes it. A link's name ends with it.
pub type Claimed = Arc<Mutex<HashMap<u64, String>>>;

/// Relays every client link's commands to the other host its links reach. A clone is the same
/// relay.
#[derive(Clone)]
pub struct Relay {
    source: Source,
    links: ByLink,
    named: Named,
    claimed: Claimed,
    /// What each new link to the other host sends first while this host is stopped (#2051).
    stop_first: Option<crate::reach::StopFirst>,
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
            named: Arc::default(),
            claimed: Arc::default(),
            stop_first: None,
        }
    }

    /// This relay, each new link it makes to the other host carrying `first`'s stop before
    /// anything else while this host is stopped (#2051, ADR 0078 §7).
    pub(crate) fn stopping_first(mut self, first: crate::reach::StopFirst) -> Relay {
        self.stop_first = Some(first);
        self
    }

    /// This relay, holding the names its client links take in `claimed`, which other relays of
    /// the same host share (#2744): a name another of their links holds is refused here too.
    pub fn claiming(mut self, claimed: Claimed) -> Relay {
        self.claimed = claimed;
        self
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
                lock(&self.named).remove(&peer.link()),
                self.stop_first.clone(),
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

impl Relay {
    /// **`peer`'s link is the window that keeps `name` for it** (#2623): every link to the other
    /// host made for it names the window there by [`taken_back_as`] `name`. Refused on a link
    /// that sent anything before, and while another client link holds the name.
    fn name(&self, name: &Secret, peer: &Peer) -> Result<Answer, Refusal> {
        if peer.has_ended() {
            return Err(ended());
        }
        let window = taken_back_as(name);
        let links = lock(&self.links);
        let mut named = lock(&self.named);
        if links.contains_key(&peer.link()) || named.contains_key(&peer.link()) {
            return Err(Refusal {
                code: session::MALFORMED.into(),
                why: "a window names itself as its link's first command, and this link has \
                      sent one before"
                    .into(),
            });
        }
        let mut claimed = lock(&self.claimed);
        let held = links
            .values()
            .any(|relayed| relayed.client.window == window)
            || named.values().any(|other| *other == window)
            || claimed
                .iter()
                .any(|(link, other)| *link != peer.link() && *other == window);
        if held {
            return Err(Refusal {
                code: session::NOT_ALLOWED.into(),
                why: "another window's link holds this name now".into(),
            });
        }
        claimed.insert(peer.link(), window.clone());
        named.insert(peer.link(), window);
        // A link that ended meanwhile holds no name: its end may already have been dealt with,
        // and would not let go of one taken after it.
        if peer.has_ended() {
            claimed.remove(&peer.link());
            named.remove(&peer.link());
            return Err(ended());
        }
        Ok(Answer::Done)
    }
}

/// **The name the relay gives the window that keeps `name`, on every link to the other host it
/// makes for it** (#2623): a digest of it, so the other host never learns the name itself, and
/// the same after the window restarts.
fn taken_back_as(name: &Secret) -> String {
    use sha2::Digest;
    let mut digest = sha2::Sha256::new();
    digest.update(b"purlis window, as a relay names it\0");
    digest.update(name.expose().as_bytes());
    digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
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
    Down(Down),
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
    /// What the relay keeps for a client link, named to the other host as `window`, where it
    /// named itself, and else by a name of its own.
    fn start(
        source: Source,
        opener: Opener,
        scope: Scope,
        window: Option<String>,
        stop_first: Option<crate::reach::StopFirst>,
    ) -> Relayed {
        let client = Arc::new(ClientLink {
            scope,
            stop_first,
            window: window.unwrap_or_else(|| ulid::Ulid::generate().to_string()),
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
                Standing::Down(down) => return Err(not_linked(&down)),
                Standing::Over => self.again.notify_one(),
                Standing::Linking => {}
            }
            if standing.changed().await.is_err() {
                return Err(ended());
            }
        }
    }
}

impl Relayed {
    /// [`Relayed::up`], for a command that changes what the client's link stands on: counted
    /// as changing it, on a link that is still the one up once it is counted, so a carry-over
    /// that began meanwhile waits for it, or it waits for the carry-over.
    async fn up_changing(&self, peer: &Peer) -> Result<(Arc<Upstream>, Counted<'_>), Refusal> {
        loop {
            let upstream = self.up(peer).await?;
            let changing = Counted::start(&self.client.changing);
            if matches!(&*self.standing.borrow(), Standing::Up(now) if Arc::ptr_eq(now, &upstream))
            {
                return Ok((upstream, changing));
            }
        }
    }
}

fn not_linked(down: &Down) -> Refusal {
    let why = &down.why;
    if down.key_changed {
        return Refusal {
            code: session::KEY_CHANGED.into(),
            why: format!(
                "Runner refused ({why}). purlis sent it nothing and does not link to it until it \
                 is removed and added again"
            ),
        };
    }
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
/// links again. The client is told once each time no link is up, and once one is back
/// ([`LinkTold`]); a link renewed ahead of its limit is neither.
async fn keep(
    source: Source,
    client: Arc<ClientLink>,
    tell: watch::Sender<Standing>,
    again: Arc<Notify>,
) {
    let mut told = LinkTold::default();
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
                            Err(NotCarried::StopRefused) => {
                                // The runner did not take the stop: no link to it carries the
                                // window on, the one being renewed neither (#2051).
                                up = None;
                                retiring = None;
                                let why = "the runner did not take this machine's stop";
                                tell.send_replace(Standing::Down(Down::because(why)));
                                told.down(&client, why, false);
                            }
                            Ok(upstream) => {
                                // The link it replaces ends once what was sent on it is
                                // answered: the kept link is told the relay has moved then.
                                retiring = up.replace(Arc::clone(&upstream));
                                tell.send_replace(Standing::Up(upstream));
                                told.back(&client);
                                if retiring.is_none() {
                                    links.moved();
                                }
                            }
                            // The new link ended first: a link renewed ahead of its limit carries
                            // on until it ends, and with none, no link is up.
                            Err(NotCarried::Ended) => match &up {
                                Some(old) => {
                                    tell.send_replace(Standing::Up(Arc::clone(old)));
                                }
                                None => {
                                    let why = "the link to it ended as it came up";
                                    tell.send_replace(Standing::Down(Down::because(why)));
                                    told.down(&client, why, false);
                                }
                            },
                        }
                    }
                    Some(Linked::Down(down)) => {
                        up = None;
                        retiring = None;
                        told.down(&client, &down.why, down.key_changed);
                        tell.send_replace(Standing::Down(down));
                    }
                    None => no_more = true,
                },
                () = drained => {
                    retiring = None;
                    links.moved();
                }
                () = link_ended => {
                    up = None;
                    let why = "the link to it ended";
                    tell.send_replace(Standing::Down(Down::because(why)));
                    told.down(&client, why, false);
                }
            }
        }
        tell.send_replace(Standing::Over);
        again.notified().await;
        tell.send_replace(Standing::Linking);
    }
}

/// What the client was last told of the link to the other host (#2496).
#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum LinkTold {
    /// Nothing since the link was last up: it is up, as far as the client knows.
    #[default]
    Up,
    /// That no link is up.
    Down,
}

impl LinkTold {
    /// No link is up, for `why`: the client is told, unless it was told already.
    fn down(&mut self, client: &ClientLink, why: &str, key_changed: bool) {
        if std::mem::replace(self, LinkTold::Down) == LinkTold::Up {
            client.tell_now(Pushed::LinkDown {
                why: why.into(),
                key_changed,
            });
        }
    }

    /// A link is up and carries what the client's link stands on: a client told it was down is
    /// told it is back. One word for each outage, so it waits for room, on a task of its own,
    /// and never holds up the link.
    fn back(&mut self, client: &Arc<ClientLink>) {
        if std::mem::replace(self, LinkTold::Up) == LinkTold::Down {
            let client = Arc::clone(client);
            tokio::spawn(async move { client.tell(Pushed::LinkBack {}).await });
        }
    }
}

/// A client link, as the relay keeps it: who it is, and what it stands on, which the relay
/// carries over to each new link to the other host.
struct ClientLink {
    scope: Scope,
    /// What a new link to the other host sends first while this host is stopped (#2051).
    stop_first: Option<crate::reach::StopFirst>,
    /// The relay's name for this client link, which every link to the other host made for it
    /// says ([`Command::TakeBack`], #2511): a name no other live client link is given, the
    /// digest of the window's own where it named itself ([`taken_back_as`], #2623).
    window: String,
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
    /// How many times the client has subscribed: a carry-over carries the last.
    subscriptions: u64,
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

    /// Hand `pushed` to the client's subscription, while it has one, without waiting for room:
    /// a client that has stopped reading never holds up linking again, and is not told what
    /// does not fit. It learns its views are back as each draws again (#2496).
    fn tell_now(&self, pushed: Pushed) {
        let Some(target) = self.kept().subscriber.clone() else {
            return;
        };
        if let Err(mpsc::error::TrySendError::Closed(_)) = target.try_send(pushed) {
            self.kept().subscriber = None;
        }
    }

    /// The client's view `ours` ended for good: it is told so once, however many links say so.
    async fn view_ended(&self, ours: u32, why: ViewEnd) {
        if self.kept().views.remove(&ours).is_some() {
            self.tell(Pushed::ViewEnded { view: ours, why }).await;
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

    /// Join `from`, a code stream the other host opened, after its header, to a new stream on the
    /// client's link under the same header (1.34, #2046), byte for byte both ways. Only a
    /// `local-ui` client asked for one: the relay sends no transfer on for any other.
    async fn join_code(&self, mut from: Stream, header: code::Header) {
        if self.scope != Scope::LocalUi {
            return;
        }
        let Ok(mut to) = self.opener.open().await else {
            return;
        };
        if to.write_all(&header.to_bytes()).await.is_err() {
            return;
        }
        let _ = tokio::io::copy_bidirectional(&mut from, &mut to).await;
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

    /// Send the client's `command` on this link, counted in flight until it is answered. A
    /// browser's, a login's values among them, goes only to a host that knows browsers relayed
    /// for a window (1.38, #2050): one before refuses it, and is sent nothing to refuse.
    async fn call(&self, command: Command) -> Result<Answer, Refusal> {
        if command.is_about_a_browser() && self.client.version() < session::KNOWS_RELAYED_BROWSERS {
            return Err(Refusal {
                code: session::NOT_ALLOWED.into(),
                why: format!(
                    "the runner this host relays to speaks session protocol {}.{}, which runs no \
                     browser for a window, so nothing was sent to it; update purlis on the \
                     runner (removing the pool and adding it again makes it afresh)",
                    self.client.version().major,
                    self.client.version().minor
                ),
            });
        }
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
    /// The client's subscription carried, by its number ([`Kept::subscriptions`]).
    subscribed: Option<u64>,
    /// Each view carried, by the client's number, with its program.
    views: BTreeMap<u32, String>,
}

/// Why a carry-over stopped: the new link ended.
struct LinkEnded;

/// Carry what `client`'s link stands on over to `link`, new to the other host: hold its
/// projects, name the client to take back its chats' confinements (#2511), subscribe from its
/// cursor, and attach its views again. The link before it, if
/// one is still up, carries commands while the first pass is made; then commands wait, the
/// commands still changing what the client stands on are let finish, and what they changed is
/// carried too. A project the last pass could not hold is no longer kept for the client
/// ([`not_held_on`]). `None` when the new link ends first.
async fn carry_over(
    link: Link,
    client: &Arc<ClientLink>,
    tell: &watch::Sender<Standing>,
) -> Result<Arc<Upstream>, NotCarried> {
    let upstream = Upstream::on(link, client);
    // While this host is stopped, a stop is the first thing the new link carries (#2051), and a
    // link whose runner does not take it carries nothing of the window's.
    if let Some(first) = &client.stop_first
        && !first.on(&upstream.client).await
    {
        return Err(NotCarried::StopRefused);
    }
    let mut carried = Carried::default();
    carry(&upstream, client, &mut carried)
        .await
        .map_err(|LinkEnded| NotCarried::Ended)?;
    tell.send_replace(Standing::Linking);
    let _ = client
        .changing
        .subscribe()
        .wait_for(|changing| *changing == 0)
        .await;
    let refused = carry(&upstream, client, &mut carried)
        .await
        .map_err(|LinkEnded| NotCarried::Ended)?;
    not_held_on(client, refused);
    Ok(upstream)
}

/// Why a new link carried nothing of the client's.
enum NotCarried {
    /// It ended first.
    Ended,
    /// Its runner did not take the stop it carried first while this host is stopped (#2051).
    StopRefused,
}

/// The projects the new link's last pass could not hold are no longer kept for the client: no
/// later link holds them in its name, and the other host refuses the client's next command for
/// one as a project it does not hold. The client is told of each, in the other host's words
/// (`not_held`, #2645), so it holds the project again itself: one word for each, in order, on a
/// task of its own that waits for room, so it never holds up the link.
fn not_held_on(client: &Arc<ClientLink>, refused: Refused) {
    let mut told = Vec::new();
    {
        let mut kept = client.kept();
        for (project, (path, why)) in refused {
            if kept.held.get(&project) == Some(&path) {
                kept.held.remove(&project);
                crate::say(&format!(
                    "project {project} is not held at {path} on the new link to the other host, \
                     which refused it: {why}"
                ));
                told.push(Pushed::NotHeld { project, why });
            }
        }
    }
    if told.is_empty() {
        return;
    }
    let client = Arc::clone(client);
    tokio::spawn(async move {
        for pushed in told {
            client.tell(pushed).await;
        }
    });
}

/// The holds a pass of a carry-over sent that the other host refused: by project, where the
/// client holds it, and the other host's words.
type Refused = BTreeMap<String, (String, String)>;

/// One pass of a carry-over: what the client's link stands on that is not yet carried to
/// `upstream`. A view the other host will not attach again has ended for good, and the client is
/// told so; one the client let go of since it was carried is let go of there too. A project the
/// client holds again at another path is let go of where it was carried, then held where the
/// client holds it now. A hold is carried once the other host takes it: the holds it refused
/// are answered, and the next pass sends them again.
async fn carry(
    upstream: &Arc<Upstream>,
    client: &Arc<ClientLink>,
    carried: &mut Carried,
) -> Result<Refused, LinkEnded> {
    fn closed<T>(result: &Result<T, CallError>) -> bool {
        matches!(result, Err(CallError::Closed(_)))
    }
    let (held, let_go, subscribe, attach, detach) = {
        let kept = client.kept();
        let held: Vec<(String, String)> = kept
            .held
            .iter()
            .filter(|(project, path)| carried.held.get(*project) != Some(*path))
            .map(|(project, path)| (project.clone(), path.clone()))
            .collect();
        let let_go: Vec<String> = carried
            .held
            .keys()
            .filter(|project| !kept.held.contains_key(*project))
            .cloned()
            .collect();
        let subscribe = kept
            .subscriber
            .as_ref()
            .filter(|subscriber| {
                !subscriber.is_closed() && carried.subscribed != Some(kept.subscriptions)
            })
            .map(|_| (kept.subscriptions, kept.cursor.clone()));
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
        (held, let_go, subscribe, attach, detach)
    };
    for project in let_go {
        let let_go = upstream
            .client
            .call(Command::LetGoOfProject {
                project: project.clone(),
            })
            .await;
        match let_go {
            Ok(_) => {
                carried.held.remove(&project);
            }
            Err(CallError::Closed(_)) => return Err(LinkEnded),
            // Still held where it was carried: the next pass lets go of it again.
            Err(_) => {}
        }
    }
    let mut refused = BTreeMap::new();
    for (project, path) in held {
        // Held again at another path: the other host holds a project at one place for each
        // link, so the new link lets go of where it holds it first.
        if carried.held.contains_key(&project) {
            let let_go = upstream
                .client
                .call(Command::LetGoOfProject {
                    project: project.clone(),
                })
                .await;
            match let_go {
                Ok(_) => {
                    carried.held.remove(&project);
                }
                Err(CallError::Closed(_)) => return Err(LinkEnded),
                // Still held where it was carried, so it is not held at the new path.
                Err(why) => {
                    refused.insert(project, (path, words(&why)));
                    continue;
                }
            }
        }
        match upstream.client.hold_project(&project, &path).await {
            Ok(()) => {
                carried.held.insert(project, path);
            }
            Err(CallError::Closed(_)) => return Err(LinkEnded),
            Err(why) => {
                refused.insert(project, (path, words(&why)));
            }
        }
    }
    // A host that does not know the word takes nothing back.
    let taken = upstream.client.take_back(&client.window).await;
    if closed(&taken) {
        return Err(LinkEnded);
    }
    if let Some((subscription, cursor)) = subscribe {
        let subscribed = upstream.client.subscribe(cursor).await;
        if closed(&subscribed) {
            return Err(LinkEnded);
        }
        carried.subscribed = Some(subscription);
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
            client.view_ended(ours, ViewEnd::Closed).await;
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
    Ok(refused)
}

/// The other host's words for why it refused, or what else went wrong.
fn words(why: &CallError) -> String {
    why.refusal()
        .map_or_else(|| why.to_string(), |refusal| refusal.why.clone())
}

/// Joins each view's stream the other host opens on one link to a new stream on the client's
/// link, under the client's number for that view. A stream whose attach is not answered yet
/// waits for it, for [`PARKED_FOR`] at the most.
async fn join_views(mut views: Acceptor, client: Arc<ClientLink>, joins: Arc<Mutex<Joins>>) {
    while let Ok(mut from) = views.accept().await {
        let (client, joins) = (Arc::clone(&client), Arc::clone(&joins));
        tokio::spawn(async move {
            // A code stream (1.34, #2046) is joined as it comes, under its own header: the
            // transfer it names is the client's own number, which the command carried.
            let Ok(kind) = from.read_u8().await else {
                return;
            };
            if kind == code::CODE {
                if let Ok(header) = code::Header::read_after_kind(&mut from).await {
                    client.join_code(from, header).await;
                }
                return;
            }
            if kind != purlis_session_protocol::view::VIEW {
                return;
            }
            let Ok(header) = Header::read_after_kind(&mut from).await else {
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
        // The other host's own event log is the window's alone (1.41, #3393).
        if client.scope != Scope::LocalUi
            && matches!(pushed, Pushed::Event(_) | Pushed::Missed { .. })
        {
            continue;
        }
        match pushed {
            Pushed::Event(event) => client.hand_event(event).await,
            Pushed::ViewEnded { view, why } => {
                let Some(ours) = lock(joins).of.remove(&view) else {
                    continue;
                };
                client.view_ended(ours, why).await;
            }
            // Whether the link to the other host is up, and which projects it holds for the
            // client, is this host's to say, never the other host's (#2496, #2645).
            Pushed::LinkDown { .. } | Pushed::LinkBack {} | Pushed::NotHeld { .. } => {}
            pushed => client.tell(pushed).await,
        }
    }
}

/// Whether `command` is one only the window sends: what the relay sends on for a `local-ui`
/// client alone.
fn the_windows_alone(command: &Command) -> bool {
    match command {
        Command::StartProgram(start) => {
            start.launch.is_some()
                || start.without_sandbox
                || start.confinement.is_some()
                || !start.alone.is_empty()
                || start.kind == session::ProgramKind::Browser
        }
        Command::AttachView { kind, .. } => *kind == session::ViewKind::Browser,
        command => the_windows_power(command.power()),
    }
}

/// `command`, without the chat and run ids a start names unless its client is the window
/// (#2446): the other host keeps the ids its peer sends on, as its window's (ADR 0066), so a
/// start from any other client is sent on to be given ids of the other host's own, as this
/// host gives such a start ids of its own.
fn ids_for_the_window_alone(command: Command, scope: Scope) -> Command {
    match command {
        Command::StartProgram(start) if scope != Scope::LocalUi => {
            Command::StartProgram(Box::new(StartProgram {
                program: None,
                run: None,
                ..*start
            }))
        }
        command => command,
    }
}

/// Whether `power` is one only the window holds of those the relay meets in a command or a
/// push: holding a project, replying to its hook lines, saving it, confining a chat, moving
/// code (1.34, #2046), and a browser, its acts, its driving and its login (1.38, #2050).
fn the_windows_power(power: Power) -> bool {
    matches!(
        power,
        Power::Hold
            | Power::Heard
            | Power::Save
            | Power::Confine
            | Power::Code
            | Power::Browser
            | Power::LogIn
            | Power::EventLog
            | Power::Rearm
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
        // A kept login stays on the device it was made on (#2021, #2050): a browser on another
        // host is never opened from one, and keeps none.
        if command.uses_kept_login() {
            return Err(Refusal {
                code: session::NOT_ALLOWED.into(),
                why: "a kept login stays on the device it was made on: a browser on another host \
                      is not opened from one, and keeps none"
                    .into(),
            });
        }
        // A window's link is relayed to a runner by its own host alone (#2679): a relaying host
        // never asks the host it relays to to relay again.
        if let Command::RelayTo { .. } = command {
            return Err(Refusal {
                code: session::NOT_ALLOWED.into(),
                why: "this link is relayed to a runner already, and is relayed to no other".into(),
            });
        }
        // A runner is unpaired by the desktop that removes it, on a link of its own (#2525):
        // never sent on for a client.
        if command == Command::Unpair {
            return Err(Refusal {
                code: session::NOT_ALLOWED.into(),
                why: "a runner is unpaired as purlis removes it, never through a relaying host"
                    .into(),
            });
        }
        // A runner keeps a profile for the desktop it is paired with, on that desktop's own link
        // to it (#2049): never sent on for a client.
        if matches!(
            command,
            Command::KeepProfile(_) | Command::StageProfileSettings(_)
        ) {
            return Err(Refusal {
                code: session::NOT_ALLOWED.into(),
                why: "a runner keeps a profile for its desktop on that desktop's own link, never \
                      through a relaying host"
                    .into(),
            });
        }
        // While this host is stopped nothing starts on the runner either (#2051): its stop is
        // carried first, and a start sent meanwhile is refused here, whatever the runner holds.
        if matches!(command, Command::StartProgram(_) | Command::Start(_))
            && self
                .stop_first
                .as_ref()
                .is_some_and(|first| (first.stopped)())
        {
            return Err(Refusal {
                code: session::STOPPED.into(),
                why: "every chat is stopped by the kill switch; nothing starts on a runner until \
                      it is re-armed in purlis's window"
                    .into(),
            });
        }
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
        if let Command::NameWindow { name } = &command {
            return self.name(name, peer);
        }
        if let Command::TakeBack { .. } = command {
            return Err(Refusal {
                code: session::NOT_ALLOWED.into(),
                why: "`take_back` is the relaying host's own word, and no client sends it".into(),
            });
        }
        let command = ids_for_the_window_alone(command, peer.scope());
        let relayed = self.relayed(peer)?;
        let client = &relayed.client;
        match command {
            Command::AttachView {
                program,
                kind,
                role,
                rate,
            } => {
                let (upstream, _changing) = relayed.up_changing(peer).await?;
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
                let (upstream, _changing) = relayed.up_changing(peer).await?;
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
                let (upstream, _changing) = relayed.up_changing(peer).await?;
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
        lock(&self.named).remove(&peer.link());
        lock(&self.claimed).remove(&peer.link());
    }

    async fn subscribe(
        &self,
        since: Since,
        peer: &Peer,
    ) -> Result<mpsc::Receiver<Pushed>, Refusal> {
        let relayed = self.relayed(peer)?;
        let (upstream, _changing) = relayed.up_changing(peer).await?;
        let (told, pushed) = mpsc::channel(MOST_PUSHED_QUEUED);
        {
            let mut kept = relayed.client.kept();
            kept.subscriber = Some(told);
            kept.subscriptions += 1;
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
mod tests;

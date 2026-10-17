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
//! [`keep_linked`](purlis_session_protocol::hostlink::keep_linked), so a link that drops comes back
//! on its own and one renewed ahead of its limit is replaced by the next, with the same pauses
//! and renewal as any kept link, and to the host pinned for it alone. Over each new link the
//! relay carries over what the client's link stands on, and nothing else: it holds the
//! projects the client holds, subscribes again from the client's cursor (the last event it
//! handed the client, so none comes twice), and attaches the client's views again, each with a
//! snapshot, under the numbers the client knows them by and at their next epoch, so the client
//! draws each again as a view that came back. The client's commands wait while that is done.
//! **No command the client sent is sent again**: one the link ended under is refused as
//! unanswered, and while no link is up every command is refused in words saying so, and is not
//! kept. A view the other host will not attach again has ended for good, and the client is
//! told so.
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
use std::sync::{Arc, Mutex, MutexGuard};

use futures::StreamExt;
use futures::future::BoxFuture;
use futures::stream::BoxStream;
use purlis_session_protocol::auth::Scope;
use purlis_session_protocol::grants::Power;
use purlis_session_protocol::hostlink::{Linked, Links};
use purlis_session_protocol::link::{Acceptor, Link, Opener, Stream};
use purlis_session_protocol::session::{
    self, Answer, Attached, CallError, Client, Command, Events, Peer, Pushed, Refusal, Since, Told,
    ViewEnd, ViewKind, ViewRole,
};
use purlis_session_protocol::view;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{Notify, mpsc, oneshot, watch};
use tokio::task::AbortHandle;

/// How a relay reaches the other host once: a new link to it, or why there is none.
pub type Dial = Arc<dyn Fn() -> BoxFuture<'static, Result<Link, String>> + Send + Sync>;

/// How a relay keeps a link to the other host: each call keeps one more, for one client link,
/// as [`keep_linked`](purlis_session_protocol::hostlink::keep_linked) does (to the host pinned for it, linked again when it drops,
/// renewed ahead of its limit), and dropping what it answered lets that link go.
pub type Keep = Arc<dyn Fn() -> Links + Send + Sync>;

/// What a relay takes its links to the other host from: each call starts one client link's
/// links, told in order; they end when no more will come.
type Source = Arc<dyn Fn() -> BoxStream<'static, Linked> + Send + Sync>;

/// The most pushed events the relay holds for a client that has not read them.
const MOST_PUSHED_QUEUED: usize = 1024;

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
            futures::stream::once(async move {
                match link.await {
                    Ok(Ok(link)) => Linked::Up(Box::new(link)),
                    Ok(Err(why)) => Linked::Down(why),
                    Err(_) => Linked::Down("the dial was let go of".into()),
                }
            })
            .boxed()
        }))
    }

    /// A relay that keeps a link to the other host for each client link through `keep`, from
    /// the client's first command until its link ends (#2215).
    pub fn kept(keep: Keep) -> Relay {
        Relay::from_source(Arc::new(move || {
            futures::stream::unfold(keep(), |mut links| async move {
                links.next().await.map(|linked| (linked, links))
            })
            .boxed()
        }))
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

    /// The link that is up, once what the client's link stands on is carried over to it; or the
    /// refusal that says no link is up.
    /// A client link that ended while it waited is refused, so nothing it sent is sent on.
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
            "the host this one relays to is not linked to it now ({why}), so nothing was sent \
             to it"
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
        let mut no_more = false;
        while !(no_more && up.is_none()) {
            let link_ended = async {
                match &up {
                    Some(upstream) => upstream.ended.notified().await,
                    None => std::future::pending().await,
                }
            };
            tokio::select! {
                linked = links.next(), if !no_more => match linked {
                    Some(Linked::Up(link)) => {
                        tell.send_replace(Standing::Linking);
                        let upstream = carry_over(*link, &client).await;
                        up = Some(Arc::clone(&upstream));
                        tell.send_replace(Standing::Up(upstream));
                    }
                    Some(Linked::Down(why)) => {
                        up = None;
                        tell.send_replace(Standing::Down(why));
                    }
                    None => no_more = true,
                },
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

    /// Join `from`, a view's stream the other host opened after its header, to a new stream on
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
        let mut header = Vec::with_capacity(13);
        header.push(view::VIEW);
        header.extend_from_slice(&view.to_be_bytes());
        header.extend_from_slice(&epoch.to_be_bytes());
        header.extend_from_slice(&snapshot.to_be_bytes());
        if to.write_all(&header).await.is_err() {
            return;
        }
        let _ = tokio::io::copy_bidirectional(&mut from, &mut to).await;
    }
}

/// One link to the other host.
struct Upstream {
    client: Client,
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

/// Which client view each view the other host opened on one link is, and a stream it opened
/// before its attach was answered, waiting to be joined.
#[derive(Default)]
struct Joins {
    /// The other host's number for a view on this link, to the client's.
    of: HashMap<u32, u32>,
    parked: HashMap<u32, (Stream, u32)>,
}

impl Upstream {
    fn on(link: Link, client: &Arc<ClientLink>) -> Upstream {
        let (to_the_other_host, views, events) = Client::new(link);
        let joins: Arc<Mutex<Joins>> = Arc::default();
        let ended = Arc::new(Notify::new());
        let joining =
            tokio::spawn(join_views(views, Arc::clone(client), Arc::clone(&joins))).abort_handle();
        let pushing = tokio::spawn({
            let (client, joins, ended) =
                (Arc::clone(client), Arc::clone(&joins), Arc::clone(&ended));
            async move {
                push_on(events, &client, &joins).await;
                ended.notify_one();
            }
        })
        .abort_handle();
        Upstream {
            client: to_the_other_host,
            joins,
            ended,
            tasks: vec![joining, pushing],
        }
    }

    /// The other host's view `theirs` on this link is the client's `ours`: join the stream it
    /// opened for it, if it came first.
    fn joined(&self, theirs: u32, ours: u32, client: &Arc<ClientLink>) {
        let parked = {
            let mut joins = lock(&self.joins);
            joins.of.insert(theirs, ours);
            joins.parked.remove(&theirs)
        };
        if let Some((stream, snapshot)) = parked {
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

/// Carry what `client`'s link stands on over to `link`, new to the other host: hold its
/// projects, subscribe from its cursor, and attach its views again. A view the other host will
/// not attach again has ended for good, and the client is told so; a link that ends while this
/// is done leaves the rest for the next.
async fn carry_over(link: Link, client: &Arc<ClientLink>) -> Arc<Upstream> {
    let upstream = Arc::new(Upstream::on(link, client));
    let (held, subscribed, views) = {
        let kept = client.kept();
        let subscribed = kept
            .subscriber
            .as_ref()
            .filter(|subscriber| !subscriber.is_closed())
            .map(|_| kept.cursor.clone());
        let views: Vec<(u32, Command)> = kept
            .views
            .iter()
            .map(|(ours, view)| (*ours, view.attach()))
            .collect();
        (kept.held.clone(), subscribed, views)
    };
    for (project, path) in held {
        if let Err(CallError::Closed(_)) = upstream.client.hold_project(&project, &path).await {
            return upstream;
        }
    }
    if let Some(cursor) = subscribed
        && let Err(CallError::Closed(_)) = upstream.client.subscribe(cursor).await
    {
        return upstream;
    }
    for (ours, attach) in views {
        match upstream.client.call(attach).await {
            Ok(Answer::Attached(attached)) => upstream.joined(attached.view, ours, client),
            Err(CallError::Closed(_)) => return upstream,
            _ => {
                client.kept().views.remove(&ours);
                client
                    .tell(Pushed::ViewEnded {
                        view: ours,
                        why: ViewEnd::Closed,
                    })
                    .await;
            }
        }
    }
    upstream
}

/// Joins each view's stream the other host opens on one link to a new stream on the client's
/// link, under the client's number for that view.
async fn join_views(mut views: Acceptor, client: Arc<ClientLink>, joins: Arc<Mutex<Joins>>) {
    while let Ok(from) = views.accept().await {
        let (client, joins) = (Arc::clone(&client), Arc::clone(&joins));
        tokio::spawn(async move {
            let Some((from, theirs, snapshot)) = header(from).await else {
                return;
            };
            let ours = {
                let mut joins = lock(&joins);
                match joins.of.get(&theirs) {
                    Some(ours) => *ours,
                    // Its attach is not answered yet: it is joined once it is.
                    None => {
                        joins.parked.insert(theirs, (from, snapshot));
                        return;
                    }
                }
            };
            client.join(from, ours, snapshot).await;
        });
    }
}

/// Read a view stream's header ([`view`]'s wire): its view's number, and its snapshot's length.
/// Its epoch is the other host's, and the client is given its own.
async fn header(mut from: Stream) -> Option<(Stream, u32, u32)> {
    if from.read_u8().await.ok()? != view::VIEW {
        return None;
    }
    let theirs = from.read_u32().await.ok()?;
    let _their_epoch = from.read_u32().await.ok()?;
    let snapshot = from.read_u32().await.ok()?;
    Some((from, theirs, snapshot))
}

/// Hands what the other host pushes on one link to the client's subscription, while it has one:
/// what only the window hears reaches it only where the client's link is the window's; an
/// event it was handed already, over this link or one before, is not handed again; and a view
/// that ended is named by the client's number for it.
async fn push_on(mut events: Events, client: &ClientLink, joins: &Mutex<Joins>) {
    while let Some(told) = events.recv().await {
        let Told::Pushed(pushed) = told else {
            // The relay fell behind the other host: the client's own cursor is what it
            // subscribes again from, and a reader that fell behind is told so by its own link.
            continue;
        };
        if client.scope != Scope::LocalUi && heard_by_the_window_alone(&pushed) {
            continue;
        }
        let pushed = match pushed {
            Pushed::Event(event) => {
                let mut kept = client.kept();
                if kept.subscriber.is_none() {
                    continue;
                }
                let last = kept.cursor.entry(event.device_id.clone()).or_default();
                if event.seq <= *last {
                    continue;
                }
                *last = event.seq;
                Pushed::Event(event)
            }
            Pushed::ViewEnded { view, why } => {
                let Some(ours) = lock(joins).of.remove(&view) else {
                    continue;
                };
                client.kept().views.remove(&ours);
                Pushed::ViewEnded { view: ours, why }
            }
            pushed => pushed,
        };
        client.tell(pushed).await;
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
                let view = KeptView {
                    program,
                    kind,
                    role,
                    rate,
                    epoch: 0,
                };
                let answer = upstream
                    .client
                    .call(view.attach())
                    .await
                    .map_err(|why| refused(&why))?;
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
                    Some(theirs) => upstream
                        .client
                        .call(Command::DetachView {
                            program,
                            view: theirs,
                        })
                        .await
                        .map_err(|why| refused(&why)),
                    None => Ok(Answer::Done),
                }
            }
            Command::HoldProject {
                project,
                project_path,
            } => {
                let upstream = relayed.up(peer).await?;
                let answer = upstream
                    .client
                    .call(Command::HoldProject {
                        project: project.clone(),
                        project_path: project_path.clone(),
                    })
                    .await
                    .map_err(|why| refused(&why))?;
                client.kept().held.insert(project, project_path);
                Ok(answer)
            }
            Command::LetGoOfProject { project } => {
                let upstream = relayed.up(peer).await?;
                let answer = upstream
                    .client
                    .call(Command::LetGoOfProject {
                        project: project.clone(),
                    })
                    .await
                    .map_err(|why| refused(&why))?;
                client.kept().held.remove(&project);
                Ok(answer)
            }
            command => {
                let upstream = relayed.up(peer).await?;
                upstream
                    .client
                    .call(command)
                    .await
                    .map_err(|why| refused(&why))
            }
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
        let (told, pushed) = mpsc::channel(MOST_PUSHED_QUEUED);
        {
            let mut kept = relayed.client.kept();
            kept.subscriber = Some(told);
            kept.cursor = since.clone();
        }
        upstream
            .client
            .subscribe(since)
            .await
            .map_err(|why| refused(&why))?;
        Ok(pushed)
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

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A project's chats run in the session host, behind the start-time switch (#1998).
//!
//! **One build holds both ways of running a chat, for the migration only** (spec #1974, ruled
//! 2026-10-14): in the window's own process ([`crate::sessions::Sessions`], the default), or in
//! the session host the window started ([`HostedSessions`]), chosen once as the app starts
//! ([`chats_run_in_the_host`]) and never switched while it runs. With the switch on and no host
//! of the window's own, the window runs no chats and says why: there is no fallback to the
//! window's own process (ADR 0068, Ruled 3).
//!
//! **Above the seam nothing changes.** The window composes a chat's start as it always has (its
//! sandbox, its hooks, its `PATH`), and hands the host that start as its [`Launch`];
//! the host composes the chat's environment from it with the same code the window's own path
//! runs (`purlis_core::chatspec`), against the environment the window started it with. The host
//! runs the program in a terminal of its own and the window draws it through a view, types into
//! it, resizes and stops it over the session protocol.
//!
//! **The host owns each project's hook channel** (#2000). It deals each chat's number, which the
//! window asks for before the start so the board knows the chat before its first hook; it binds
//! the project's hook socket, issues each chat its token as it starts it, and admits each line
//! as the window did. Every line it admits comes to the window on the link and is heard with the
//! window's own hearing ([`crate::hooks::Relayed`]), so the board, Needs you and the asks move
//! as before; the reply goes back the same way. The token never reaches the window.
//!
//! **The host holds each chat's confinement** (#1999, `crate::hostedconfinement`): the window
//! plans a chat's egress proxy from the sandbox it compiled, and the host starts and holds it,
//! so it outlives the window's link.
//!
//! **The kill switch is the host's too** (#2001, ADR 0071): the window's switch reaches it over
//! the same link ([`Connection`] is its [`crate::killswitch::Authority`]), so a stop ends every
//! program the host runs and holds across the host's restart, and only the window re-arms.
//!
//! **The event log is the host's too** (#2003): the window opens none, and hands the host each
//! write it decides ([`record_in_the_host`]); what its hooks' hearing would write of a line goes
//! back with the line's reply. The host keeps what a program's end leaves, window or no window.
//!
//! **One link for every project.** The window's link to its host ([`HostLink`]) carries every
//! project's chats: their views come on it, word that a program ended, and their hooks' lines.
//!
//! **The host saves the project too** (#2002): the window asks it to as the project opens
//! ([`HostedSessions`]'s `keep_saving`), and starts no auto-save of its own, so exactly one
//! saves it. What each look did comes back over the link, and the window reads its standing
//! again as it did when its own worker told it. A quit stops the host's saving before the
//! window's own save at quit runs; a window that is gone without a quit leaves it saving.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use purlis_core::engine::Size;
use purlis_session_protocol::link::{Acceptor, Link};
use purlis_session_protocol::session::{
    self, Answer, Client, Command, Events, Exit, Launch, ProgramKind, Pushed, StartProgram, Told,
    ViewKind,
};
use purlis_session_protocol::view::{Chunk, Reader, Viewer};

use purlis_core::hookwire::relay::{Heard, Replied, Reply};

use crate::hooks::Relayed;
use crate::host::{Ends, Opening, Readiness, SessionHost, Sink, Watching};
use crate::sessions::{ENDED, STOPPED, Utf8Stream};

/// The variable that turns the switch on: chats run in the session host. Read once, as the app
/// starts; internal to the migration, and gone with it (spec #1974).
pub const SWITCH: &str = "PURLIS_CHATS_IN_THE_HOST";

/// Whether this launch runs its chats in the session host ([`SWITCH`] set to `1`). Anything
/// else, and no value, is the window's own process, the default.
pub fn chats_run_in_the_host() -> bool {
    switched_on(purlis_core::envvar::var(SWITCH).as_deref())
}

fn switched_on(value: Option<&str>) -> bool {
    value == Some("1")
}

/// Why a chat whose profile hands it credentials is not started in the session host (#2036).
pub const NOT_HANDED_IN_THE_HOST: &str = "this chat's profile signs it in with credentials from \
     your vault, and purlis hands those only to a chat the window starts, never over the link to \
     the session host, so nothing was started. Start purlis without PURLIS_CHATS_IN_THE_HOST to \
     run it.";

/// How long a start waits for the host's link to be ready, before it says why there is none.
const LINK_WITHIN: Duration = Duration::from_secs(30);

/// How long a command waits for the host's answer.
const ANSWER_WITHIN: Duration = Duration::from_secs(30);

/// What a view is written before the screen of its next epoch: a full reset, so a pane drawn
/// from a fresh snapshot after its view fell behind starts from blank, as the snapshot assumes.
const RESET: &str = "\x1bc";

/// **Writes `entry` in the event log the session host holds** (#2003), once the link is ready:
/// `Ok` once the host has written it durably, or why it did not. What the window's
/// [`crate::hooks::Events`] hands every write it decides to while chats run in the host.
pub fn record_in_the_host(
    connection: &Connection,
    entry: &purlis_core::eventlog::Entry,
) -> Result<(), String> {
    let entry = serde_json::to_value(entry)
        .map_err(|why| format!("the write could not be put in words for the host: {why}"))?;
    match connection
        .wait(LINK_WITHIN)?
        .call(Command::Record { entry })?
    {
        Answer::Done => Ok(()),
        other => Err(format!("the session host answered a write with {other:?}")),
    }
}

/// Where the window's link to its session host stands: being made, ready, or not to be had and
/// why. Shared by every project's [`HostedSessions`].
#[derive(Default)]
pub struct Connection {
    state: Mutex<Linked>,
    changed: Condvar,
}

#[derive(Default)]
enum Linked {
    #[default]
    Making,
    Ready(Arc<HostLink>),
    Gone(String),
}

impl Connection {
    /// The link is ready.
    pub fn ready(&self, link: Arc<HostLink>) {
        *lock(&self.state) = Linked::Ready(link);
        self.changed.notify_all();
    }

    /// There is no link, for `why`: every chat start says so from now on.
    pub fn gone(&self, why: impl Into<String>) {
        *lock(&self.state) = Linked::Gone(why.into());
        self.changed.notify_all();
    }

    /// The link, once it is ready, waiting at most `within`; or why there is none.
    pub(crate) fn wait(&self, within: Duration) -> Result<Arc<HostLink>, String> {
        let deadline = Instant::now() + within;
        let mut state = lock(&self.state);
        loop {
            match &*state {
                Linked::Ready(link) => return Ok(Arc::clone(link)),
                Linked::Gone(why) => return Err(no_host(why)),
                Linked::Making => {}
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err(no_host("the session host did not answer in time"));
            }
            state = self
                .changed
                .wait_timeout(state, left)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }
}

/// The window's switch reaches the host's over the window's link, as `local-ui`: the one link
/// whose `rearm` the host takes.
impl crate::killswitch::Authority for Connection {
    fn stop(&self) -> Result<Option<String>, String> {
        let switch = self.kill_switch_after(Command::StopAll)?;
        if !switch.stopped {
            return Err(switch
                .why
                .unwrap_or_else(|| "it does not hold every agent stopped".into()));
        }
        Ok(switch.why)
    }

    fn rearm(&self) -> Result<(), String> {
        let switch = self.kill_switch_after(Command::Rearm)?;
        if switch.stopped {
            return Err(switch
                .why
                .unwrap_or_else(|| "it still holds every agent stopped".into()));
        }
        Ok(())
    }

    fn stopped(&self) -> Result<bool, String> {
        Ok(self.kill_switch_after(Command::KillSwitch)?.stopped)
    }
}

impl Connection {
    /// Sends one of the kill switch's commands, and answers the switch as the host holds it.
    fn kill_switch_after(&self, command: Command) -> Result<session::KillSwitch, String> {
        match self.wait(LINK_WITHIN)?.call(command)? {
            Answer::KillSwitch(switch) => Ok(switch),
            other => Err(format!(
                "the session host answered the kill switch with {other:?}"
            )),
        }
    }
}

/// What a chat start says when there is no host to run it (ADR 0068, Ruled 3).
fn no_host(why: &str) -> String {
    format!(
        "Chats run in purlis's session host on this machine, and it is not running: {why}. \
         Restart purlis to start it again."
    )
}

/// Where a browser view's frames go, each a JPEG image, whole (#2012). An empty one says the
/// view has ended for good.
pub type FrameSink = Box<dyn FnMut(bytes::Bytes) + Send>;

/// What a view's stream is drawn into: a pane's terminal, as text, or a browser view's canvas,
/// a whole frame at a time.
#[derive(Clone)]
enum Draws {
    Text(Arc<Mutex<Sink>>),
    Frames(Arc<Mutex<FrameSink>>),
}

/// What draws one view: the pane's sink, and where its stream stands.
struct Drawing {
    sink: Draws,
    /// The host said the view ended for good.
    ended: bool,
    /// The view's current stream has ended.
    drained: bool,
    /// [`ENDED`] has been written.
    finished: bool,
}

/// The window's link to its session host: the client its commands go through, the views the
/// host opens on it, and word of each program's end. One for every project.
pub struct HostLink {
    client: Arc<Client>,
    runtime: tokio::runtime::Handle,
    shared: Arc<Shared>,
}

#[derive(Default)]
struct Shared {
    /// The views, under one lock, so a stream and its pane always meet.
    views: Mutex<Views>,
    /// Who is told as each program ends, by its id.
    ends: Mutex<HashMap<String, TellEnd>>,
    /// What hears each held project's hook lines, by the project's stable id (#2000).
    hearing: Mutex<HashMap<String, Arc<Relayed>>>,
    /// Who is told what each confinement's proxy did, by its id (#1999).
    sandboxes: Mutex<HashMap<String, Arc<crate::hostedconfinement::Hears>>>,
    /// Who is told what each look of a project's auto-save did, by the project's id (#2002).
    savings: Mutex<HashMap<String, crate::autosave::Told>>,
    /// Who says which chats are mid-turn in a workspace of a project the host saves, by the
    /// project's id: its plane's board (#2002).
    turns: Mutex<HashMap<String, Turns>>,
}

/// The most a reply to a hook line may be on the link, its frame's envelope aside: a larger
/// answer is refused with a sentence rather than sent.
const A_REPLY_IS_AT_MOST: usize = purlis_session_protocol::link::MOST_CONTROL_FRAME_BYTES - 4096;

/// Which chats are mid-turn in a workspace, or `None` where the window cannot say.
type Turns = Arc<dyn Fn(&str) -> Option<Vec<String>> + Send + Sync>;

/// Told once, as a program ends.
type TellEnd = Box<dyn FnOnce(Exit) + Send>;

#[derive(Default)]
struct Views {
    /// Each view being drawn, by the number the host gave it.
    drawing: HashMap<u32, Drawing>,
    /// A view's stream that came before its pane was registered.
    waiting: HashMap<u32, Reader>,
}

impl HostLink {
    /// Speak the session protocol on `link`, which the host admitted as `local-ui`: subscribe
    /// to word of programs ending, and take every view the host opens. Run on `runtime`;
    /// `ended` is called once the link ends.
    pub async fn over(
        link: Link,
        runtime: tokio::runtime::Handle,
        ended: impl FnOnce() + Send + 'static,
    ) -> Result<HostLink, String> {
        let (client, views, events) = Client::new(link);
        let client = Arc::new(client);
        client
            .subscribe(session::Since::new())
            .await
            .map_err(|why| format!("the session host refused to tell of its programs: {why}"))?;
        let shared = Arc::new(Shared::default());
        runtime.spawn(take_views(views, Arc::clone(&shared)));
        let hearing = hear(
            events,
            Arc::clone(&client),
            Arc::clone(&shared),
            HookLines {
                client: Arc::clone(&client),
                runtime: runtime.clone(),
            },
        );
        runtime.spawn(async move {
            hearing.await;
            ended();
        });
        Ok(HostLink {
            client,
            runtime,
            shared,
        })
    }

    /// Send `command` and wait for the host's answer, from any thread.
    pub(crate) fn call(&self, command: Command) -> Result<Answer, String> {
        self.call_within(command, ANSWER_WITHIN)
    }

    /// [`Self::call`], waiting at most `within`.
    fn call_within(&self, command: Command, within: Duration) -> Result<Answer, String> {
        self.call_refused_within(command, within)
            .map_err(|refused| refused.why)
    }

    /// [`Self::call_within`], refused with the host's code where it gave one: what a browser's
    /// act, which may wait on its page, is asked with (#2013).
    pub(crate) fn call_refused_within(
        &self,
        command: Command,
        within: Duration,
    ) -> Result<Answer, purlis_session_protocol::session::Refusal> {
        let (answered, answer) = std::sync::mpsc::channel();
        let client = Arc::clone(&self.client);
        self.runtime.spawn(async move {
            let _ = answered.send(client.call(command).await);
        });
        let unanswered = |why: String| purlis_session_protocol::session::Refusal {
            code: String::new(),
            why,
        };
        match answer.recv_timeout(within) {
            Ok(Ok(answer)) => Ok(answer),
            Ok(Err(why)) => Err(why
                .refusal()
                .cloned()
                .unwrap_or_else(|| unanswered(why.to_string()))),
            Err(_) => Err(unanswered(
                "the session host did not answer in time".to_owned(),
            )),
        }
    }

    /// Send `command` and do not wait for its answer: what it refuses is logged.
    pub(crate) fn send(&self, command: Command) {
        let client = Arc::clone(&self.client);
        self.runtime.spawn(async move {
            let word = command.word();
            if let Err(why) = client.call(command).await {
                tracing::warn!("purlis: the session host refused `{word}`: {why}");
            }
        });
    }

    /// Draw view `view` with `sink`: its stream may already be here.
    fn draw(&self, view: u32, sink: Sink) {
        self.draw_into(view, Draws::Text(Arc::new(Mutex::new(sink))));
    }

    /// Draw browser view `view`'s frames into `sink`, each whole (#2012): its stream may already
    /// be here.
    pub(crate) fn draw_frames(&self, view: u32, sink: FrameSink) {
        self.draw_into(view, Draws::Frames(Arc::new(Mutex::new(sink))));
    }

    fn draw_into(&self, view: u32, sink: Draws) {
        let waiting = {
            let mut views = lock(&self.shared.views);
            views.drawing.insert(
                view,
                Drawing {
                    sink,
                    ended: false,
                    drained: false,
                    finished: false,
                },
            );
            views.waiting.remove(&view)
        };
        if let Some(reader) = waiting {
            self.runtime
                .spawn(draw_from(reader, Arc::clone(&self.shared)));
        }
    }

    /// Stop drawing view `view`.
    pub(crate) fn undraw(&self, view: u32) {
        let mut views = lock(&self.shared.views);
        views.drawing.remove(&view);
        views.waiting.remove(&view);
    }

    /// Tell `hears` what confinement `id`'s proxy does (#1999).
    fn hear(&self, id: &str, hears: crate::hostedconfinement::Hears) {
        lock(&self.shared.sandboxes).insert(id.to_owned(), Arc::new(hears));
    }

    /// Tell nobody what confinement `id`'s proxy does any more.
    pub(crate) fn unhear(&self, id: &str) {
        lock(&self.shared.sandboxes).remove(id);
    }

    /// Tell `tell` once program `program` ends.
    fn when_it_ends(&self, program: &str, tell: TellEnd) {
        lock(&self.shared.ends).insert(program.to_owned(), tell);
    }

    /// Hear the lines `project`'s hooks write with `relayed`, from now on.
    fn hear_project(&self, project: &str, relayed: Arc<Relayed>) {
        lock(&self.shared.hearing).insert(project.to_owned(), relayed);
    }
}

/// Where the hook lines the host pushes are heard: each by what hears its project's lines
/// ([`Relayed::take`]), each reply going back on the link in the order it was made.
struct HookLines {
    client: Arc<Client>,
    runtime: tokio::runtime::Handle,
}

impl HookLines {
    /// Hear line `id` that `project`'s hooks wrote, with what hears that project's lines.
    fn hear(&self, shared: &Shared, project: String, id: u64, line: serde_json::Value) {
        let Some(relayed) = lock(&shared.hearing).get(&project).cloned() else {
            tracing::warn!("purlis: the session host told a hook line of a project not open here");
            return;
        };
        let heard: Heard = match serde_json::from_value(line) {
            Ok(heard) => heard,
            Err(why) => {
                tracing::warn!(
                    "purlis: a hook line the session host told could not be read: {why}"
                );
                return;
            }
        };
        let reply = self.reply(project, id);
        relayed.take(id, heard, reply);
    }

    /// What replies to line `id` of `project`: each reply sent at once, behind the one before.
    fn reply(&self, project: String, id: u64) -> Reply {
        let client = Arc::clone(&self.client);
        let runtime = self.runtime.clone();
        Arc::new(move |replied: Replied| {
            let reply = within_a_frame(replied);
            let sent = client.call_now(Command::ReplyHeard {
                project: project.clone(),
                heard: id,
                reply,
            });
            runtime.spawn(async move {
                if let Err(why) = sent.await {
                    tracing::warn!("purlis: the session host refused a hook line's reply: {why}");
                }
            });
        })
    }
}

/// `replied`, as JSON no larger than the link carries: an answer too large is refused in a
/// sentence instead.
fn within_a_frame(replied: Replied) -> serde_json::Value {
    let too_large = |replied: &Replied| {
        serde_json::to_vec(replied).map_or(true, |said| said.len() > A_REPLY_IS_AT_MOST)
    };
    let replied = if too_large(&replied) {
        let why = format!(
            "the answer was larger than the session host's link carries ({A_REPLY_IS_AT_MOST} \
             bytes)"
        );
        match replied {
            Replied::Answer(_) => Replied::Answer(purlis_core::hookwire::Answer::No { why }),
            _ => Replied::NotTaken { why },
        }
    } else {
        replied
    };
    serde_json::to_value(replied).unwrap_or(serde_json::Value::Null)
}

/// Take each view's stream the host opens, and hand it to its pane, or keep it until the pane
/// is registered: the host opens the stream before it answers the attach.
async fn take_views(mut views: Acceptor, shared: Arc<Shared>) {
    let mut viewer = Viewer::default();
    while let Ok(stream) = views.accept().await {
        let Ok(reader) = viewer.accept(stream).await else {
            continue;
        };
        let view = reader.view().0;
        let mut views = lock(&shared.views);
        match views.drawing.get_mut(&view) {
            Some(drawing) => {
                drawing.drained = false;
                drop(views);
                tokio::spawn(draw_from(reader, Arc::clone(&shared)));
            }
            None => {
                views.waiting.insert(view, reader);
            }
        }
    }
}

/// Draw one stream of a view into its pane, as text, acknowledging what was handed over. A
/// stream after the first (the view fell behind, and came back at its next epoch) starts its
/// pane over.
async fn draw_from(mut reader: Reader, shared: Arc<Shared>) {
    let view = reader.view().0;
    let frames = lock(&shared.views)
        .drawing
        .get(&view)
        .and_then(|drawing| match &drawing.sink {
            Draws::Frames(sink) => Some(Arc::clone(sink)),
            Draws::Text(_) => None,
        });
    if let Some(sink) = frames {
        return draw_frames_from(reader, &shared, &sink).await;
    }
    let again = reader.epoch().0 > 1;
    let mut text = Utf8Stream::default();
    if again {
        give(&shared, view, RESET.to_owned());
    }
    while let Some(Ok(chunk)) = reader.next().await {
        let bytes = match chunk {
            Chunk::Snapshot(bytes) | Chunk::Live(bytes) => bytes,
        };
        give(&shared, view, text.push(&bytes));
        if reader.ack(bytes.len()).await.is_err() {
            break;
        }
    }
    let finish = lock(&shared.views)
        .drawing
        .get_mut(&view)
        .is_some_and(|drawing| {
            drawing.drained = true;
            drawing.ended
        });
    if finish {
        finish_view(&shared, view);
    }
}

/// Draw one stream of a browser view (#2012): each whole frame handed over as it completes,
/// and acknowledged once handed. A stream after the first starts on a whole frame too: the
/// host's snapshot is the latest frame.
async fn draw_frames_from(mut reader: Reader, shared: &Arc<Shared>, sink: &Arc<Mutex<FrameSink>>) {
    let view = reader.view().0;
    let mut frames = purlis_session_protocol::view::frames::Frames::default();
    while let Some(Ok(chunk)) = reader.next().await {
        let bytes = match chunk {
            Chunk::Snapshot(bytes) | Chunk::Live(bytes) => bytes,
        };
        let Ok(whole) = frames.push(&bytes) else {
            break;
        };
        for frame in whole {
            if frame.format == purlis_session_protocol::view::frames::JPEG
                && !frame.image.is_empty()
            {
                (lock(sink))(frame.image);
            }
        }
        if reader.ack(bytes.len()).await.is_err() {
            break;
        }
    }
    let finish = lock(&shared.views)
        .drawing
        .get_mut(&view)
        .is_some_and(|drawing| {
            drawing.drained = true;
            drawing.ended
        });
    if finish {
        finish_view(shared, view);
    }
}

/// Hand `text` to view `view`'s pane, if it still has one.
fn give(shared: &Shared, view: u32, text: String) {
    if text.is_empty() {
        return;
    }
    let sink = lock(&shared.views)
        .drawing
        .get(&view)
        .and_then(|drawing| match &drawing.sink {
            Draws::Text(sink) => Some(Arc::clone(sink)),
            Draws::Frames(_) => None,
        });
    if let Some(sink) = sink {
        (lock(&sink))(text);
    }
}

/// The view ended for good, and its last bytes are drawn: its pane is told, once.
fn finish_view(shared: &Shared, view: u32) {
    let sink = {
        let mut views = lock(&shared.views);
        match views.drawing.get_mut(&view) {
            Some(one) if !one.finished => {
                one.finished = true;
                Some(one.sink.clone())
            }
            _ => None,
        }
    };
    match sink {
        Some(Draws::Text(sink)) => (lock(&sink))(ENDED.to_owned()),
        Some(Draws::Frames(sink)) => (lock(&sink))(bytes::Bytes::new()),
        None => {}
    }
}

/// What the host pushes: a program's end, a view's, and its projects' hook lines.
async fn hear(mut events: Events, client: Arc<Client>, shared: Arc<Shared>, lines: HookLines) {
    while let Some(told) = events.recv().await {
        match told {
            Told::Pushed(Pushed::Heard {
                project,
                heard,
                line,
            }) => lines.hear(&shared, project, heard, line),
            Told::Pushed(Pushed::Ended { program, exit, .. }) => {
                let tell = lock(&shared.ends).remove(&program);
                if let Some(tell) = tell {
                    tell(exit);
                }
            }
            Told::Pushed(Pushed::Saving {
                project,
                saved,
                moved_tree,
            }) => {
                let told = lock(&shared.savings).get(&project).cloned();
                if let Some(told) = told {
                    // What it tells reads the plane's model and the window: off the runtime.
                    tokio::task::spawn_blocking(move || {
                        told(crate::autosave::Looked { saved, moved_tree });
                    });
                }
            }
            Told::Pushed(Pushed::WhoIsMidTurn {
                question,
                project,
                workspace,
            }) => {
                let turns = lock(&shared.turns).get(&project).cloned();
                let client = Arc::clone(&client);
                tokio::spawn(async move {
                    // The board is read off the runtime; a project the window does not save
                    // in the host is one it cannot say anything of.
                    let chats = match turns {
                        Some(turns) => tokio::task::spawn_blocking(move || turns(&workspace))
                            .await
                            .ok()
                            .flatten(),
                        None => None,
                    };
                    let _ = client.call(Command::MidTurn { question, chats }).await;
                });
            }
            Told::Pushed(Pushed::Sandbox { confinement, told }) => {
                let hears = lock(&shared.sandboxes).get(&confinement).cloned();
                if let Some(hears) = hears {
                    hears.told(told);
                }
            }
            Told::Pushed(Pushed::ViewEnded { view, .. }) => {
                let drained = lock(&shared.views)
                    .drawing
                    .get_mut(&view)
                    .is_some_and(|drawing| {
                        drawing.ended = true;
                        drawing.drained
                    });
                if drained {
                    finish_view(&shared, view);
                }
            }
            // The window keeps no events of the host's log yet: only the notices matter, so a
            // reader that fell behind subscribes again from now.
            Told::FellBehind { .. } => {
                let _ = client.subscribe(session::Since::new()).await;
            }
            Told::Pushed(_) => {}
        }
    }
}

/// One chat the host runs for this project.
struct Program {
    /// Its id in the host.
    id: String,
    /// Its process, until it has ended.
    process: Option<u32>,
    /// The views the window has open on it, by the number the host gave each.
    views: Vec<u32>,
}

/// A project's chats, run in the session host (#1998): [`SessionHost`] over the window's link.
pub struct HostedSessions {
    connection: Arc<Connection>,
    root: PathBuf,
    /// The project's stable id, once the host holds it.
    held: Arc<Mutex<Option<String>>>,
    /// What hears the lines the host hands on from the project's hook channel (#2000).
    relayed: Arc<Mutex<Option<Arc<Relayed>>>>,
    running: Arc<Mutex<HashMap<u32, Program>>>,
    /// The highest number the window knows went out: the host deals them (#2000), and this is
    /// what the window's record keeps and what it tells the host went out before it held the
    /// project. It only rises.
    opened: Arc<AtomicU32>,
    ended: Mutex<Option<Ends>>,
    kill_switch: Arc<crate::killswitch::KillSwitch>,
}

impl HostedSessions {
    /// The chats of the project at `root`, run in the host `connection` reaches.
    pub fn new(connection: Arc<Connection>, root: &Path) -> Self {
        HostedSessions {
            connection,
            root: root.to_path_buf(),
            held: Arc::default(),
            relayed: Arc::default(),
            running: Arc::default(),
            opened: Arc::default(),
            ended: Mutex::new(None),
            kill_switch: crate::killswitch::KillSwitch::unthrown(),
        }
    }

    fn relayed(&self) -> Option<Arc<Relayed>> {
        lock(&self.relayed).clone()
    }

    /// The link, with this project held on it.
    fn link(&self) -> Result<(Arc<HostLink>, String), String> {
        held_on(
            &self.connection,
            &self.held,
            &self.relayed,
            &self.opened,
            &self.root,
        )
    }

    /// The next number the host deals in this project.
    fn dealt_by_the_host(&self) -> Result<u32, String> {
        let (link, project) = self.link()?;
        match link.call(Command::Deal { project })? {
            Answer::Dealt { number } => {
                self.opened.fetch_max(number, Ordering::Relaxed);
                Ok(number)
            }
            other => Err(format!("the session host answered a deal with {other:?}")),
        }
    }

    /// The link, and the host's id of chat `id`.
    fn program(&self, id: u32) -> Result<(Arc<HostLink>, String), String> {
        let program = lock(&self.running)
            .get(&id)
            .map(|program| program.id.clone())
            .ok_or_else(|| gone(id))?;
        Ok((self.connection.wait(LINK_WITHIN)?, program))
    }

    /// The number a chat opens under: `wanted` where it can still be had, else the next the
    /// host deals (`crate::sessions::Sessions`'s rule, the host dealing).
    fn number_for(&self, wanted: Option<u32>) -> Result<u32, String> {
        let free = wanted.filter(|wanted| *wanted > 0 && !lock(&self.running).contains_key(wanted));
        match free {
            Some(wanted) => {
                self.opened.fetch_max(wanted, Ordering::Relaxed);
                Ok(wanted)
            }
            None => self.dealt_by_the_host(),
        }
    }

    /// Every chat's id in the host, and the link.
    fn every_program(&self) -> Option<(Arc<HostLink>, Vec<String>)> {
        let programs: Vec<String> = lock(&self.running)
            .values()
            .map(|program| program.id.clone())
            .collect();
        let link = self.connection.wait(Duration::ZERO).ok()?;
        Some((link, programs))
    }

    /// Sends `command` for each program at once, and waits for every answer.
    fn to_every_program(&self, command: impl Fn(String) -> Command) -> usize {
        let Some((link, programs)) = self.every_program() else {
            return 0;
        };
        let asked = programs.len();
        let answers: Vec<_> = programs
            .into_iter()
            .map(|program| {
                let (answered, answer) = std::sync::mpsc::channel();
                let client = Arc::clone(&link.client);
                let command = command(program);
                link.runtime.spawn(async move {
                    let _ = answered.send(client.call(command).await);
                });
                answer
            })
            .collect();
        for answer in answers {
            let _ = answer.recv_timeout(ANSWER_WITHIN);
        }
        asked
    }
}

/// The link `connection` reaches, with the project at `root` held on it: `held` keeps its id
/// once it is. Before the hold the host's lines for the project are heard through `relayed`,
/// and after it the host is told the numbers `opened` has dealt (#2000).
fn held_on(
    connection: &Connection,
    held: &Mutex<Option<String>>,
    relayed: &Mutex<Option<Arc<Relayed>>>,
    opened: &AtomicU32,
    root: &Path,
) -> Result<(Arc<HostLink>, String), String> {
    let link = connection.wait(LINK_WITHIN)?;
    let mut held = lock(held);
    if let Some(project) = &*held {
        return Ok((link, project.clone()));
    }
    let project = purlis_core::scaffold::planefile::ensure_project_id(root)
        .map_err(|why| format!("this project has no id the session host can name it by: {why}"))?;
    // Before the hold, so the host's first line for the project is heard.
    if let Some(relayed) = lock(relayed).clone() {
        link.hear_project(&project, relayed);
    }
    link.call(Command::HoldProject {
        project: project.clone(),
        project_path: root.display().to_string(),
    })?;
    // The numbers that went out before the host held the project are never dealt again.
    let dealt = opened.load(Ordering::Relaxed);
    if dealt > 0 {
        link.call(Command::DealtAlready {
            project: project.clone(),
            dealt,
        })?;
    }
    *held = Some(project.clone());
    Ok((link, project))
}

/// What holds a project on the window's link, for a thread of its own: [`held_on`]'s.
struct Holding {
    connection: Arc<Connection>,
    held: Arc<Mutex<Option<String>>>,
    relayed: Arc<Mutex<Option<Arc<Relayed>>>>,
    opened: Arc<AtomicU32>,
    root: PathBuf,
}

/// A project the session host saves for the window (#2002): asked for once the link is ready,
/// off the thread that opened the project, and stopped when this is dropped.
struct SavedInTheHost {
    asked: Arc<Mutex<Asked>>,
    poke: std::sync::mpsc::Sender<crate::autosave::Poke>,
    /// Who says which chats are mid-turn: the plane, once it is built.
    mid_turn: Arc<Mutex<Option<crate::autosave::MidTurn>>>,
    /// A quit already stopped it, answered or not: its drop asks nothing more of the host.
    stopped_by_a_quit: bool,
}

#[derive(Default)]
struct Asked {
    /// Dropped: nothing more is asked of the host.
    stopped: bool,
    /// The host saves the project, on this link.
    saving: Option<(Arc<HostLink>, String)>,
}

impl SavedInTheHost {
    fn start(holding: Holding, told: crate::autosave::Told) -> SavedInTheHost {
        let Holding {
            connection,
            held,
            relayed,
            opened,
            root,
        } = holding;
        let asked: Arc<Mutex<Asked>> = Arc::default();
        let (poke, poked) = std::sync::mpsc::channel();
        let asking = Arc::clone(&asked);
        let mid_turn: Arc<Mutex<Option<crate::autosave::MidTurn>>> = Arc::default();
        let board = Arc::clone(&mid_turn);
        let started = std::thread::Builder::new()
            .name("saved in the session host".into())
            .spawn(move || {
                let (link, project) = match held_on(&connection, &held, &relayed, &opened, &root) {
                    Ok(held) => held,
                    Err(why) => {
                        tracing::warn!(
                            "purlis: {} is not saved by itself on this launch: {why}",
                            root.display()
                        );
                        return;
                    }
                };
                // The window reads the standing again as the host says it moved, as it did
                // when its own worker said so.
                lock(&link.shared.savings).insert(project.clone(), {
                    let root = root.clone();
                    Arc::new(move |looked| {
                        purlis_core::planegit::touch(&root);
                        told(looked);
                    })
                });
                // And the host asks the window's board who is mid-turn.
                lock(&link.shared.turns).insert(
                    project.clone(),
                    Arc::new(move |workspace: &str| {
                        let ask = lock(&board).clone();
                        ask.map(|ask| ask(workspace))
                    }),
                );
                let mut asked = lock(&asking);
                if asked.stopped {
                    forget(&link, &project);
                    return;
                }
                match link.call(Command::StartSaving {
                    project: project.clone(),
                }) {
                    Ok(_) => asked.saving = Some((link, project)),
                    Err(why) => {
                        forget(&link, &project);
                        tracing::warn!(
                            "purlis: the session host does not save {}: {why}",
                            root.display()
                        );
                    }
                }
                drop(asked);
                // What the window pokes: a fetch is the host's to make. A chat's end the host
                // hears for itself, as its program ends.
                while let Ok(poke) = poked.recv() {
                    let asked = lock(&asking);
                    if asked.stopped {
                        return;
                    }
                    if let (crate::autosave::Poke::Fetch, Some((link, project))) =
                        (poke, &asked.saving)
                    {
                        link.send(Command::FetchProject {
                            project: project.clone(),
                        });
                    }
                }
            });
        if let Err(why) = started {
            tracing::warn!("purlis: the project's saving was not asked of the session host: {why}");
        }
        SavedInTheHost {
            asked,
            poke,
            mid_turn,
            stopped_by_a_quit: false,
        }
    }

    /// Stops the host's saving, waiting at most `within` for its answer: whether it answered.
    /// Nothing asked of the host, or nothing more to stop, is stopped.
    fn stop_within(&self, within: Duration) -> bool {
        let deadline = Instant::now() + within;
        // A start still being asked holds this until the host answers it: waited for only
        // until the deadline.
        let mut asked = loop {
            match self.asked.try_lock() {
                Ok(asked) => break asked,
                Err(std::sync::TryLockError::Poisoned(poisoned)) => break poisoned.into_inner(),
                Err(std::sync::TryLockError::WouldBlock) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(std::sync::TryLockError::WouldBlock) => {
                    tracing::warn!("purlis: the session host did not answer the project's saving");
                    return false;
                }
            }
        };
        asked.stopped = true;
        let answered = match asked.saving.take() {
            Some((link, project)) => {
                forget(&link, &project);
                let left = deadline.saturating_duration_since(Instant::now());
                match link.call_within(Command::StopSaving { project }, left) {
                    Ok(_) => true,
                    Err(why) => {
                        tracing::warn!("purlis: the session host did not stop saving: {why}");
                        false
                    }
                }
            }
            None => true,
        };
        drop(asked);
        // Wakes the poke thread, which sees it is stopped.
        let _ = self.poke.send(crate::autosave::Poke::Fetch);
        answered
    }
}

/// Nobody on this window is told of `project`'s saving, or asked of its turns, any more.
fn forget(link: &HostLink, project: &str) {
    lock(&link.shared.savings).remove(project);
    lock(&link.shared.turns).remove(project);
}

impl crate::autosave::Saver for SavedInTheHost {
    fn poker(&self) -> std::sync::mpsc::Sender<crate::autosave::Poke> {
        self.poke.clone()
    }

    /// What the host asks of this window's board ([`Pushed::WhoIsMidTurn`]).
    fn asks_mid_turn_of(&self, ask: crate::autosave::MidTurn) {
        *lock(&self.mid_turn) = Some(ask);
    }

    fn stop_by(mut self: Box<Self>, deadline: Instant) -> bool {
        self.stopped_by_a_quit = true;
        self.stop_within(deadline.saturating_duration_since(Instant::now()))
    }
}

/// Stops the host's saving and answers once the host has: a close's or a quit's own save comes
/// after, so the project is never saved by both. A quit stops it sooner ([`Self::stop_within`]).
impl Drop for SavedInTheHost {
    fn drop(&mut self) {
        if !self.stopped_by_a_quit {
            self.stop_within(ANSWER_WITHIN);
        }
    }
}

/// What kind of program the host is asked to start: a shell is the operator's, or the person's
/// own login shell with no harness in it; anything else, a profile's own command among them, is
/// a harness. The host lets the window's shell through the kill switch (ADR 0071, ruled 3), and
/// the window refuses, before it asks, every shell that is not the operator's.
fn kind_of(opening: &Opening) -> ProgramKind {
    let the_persons_shell = opening.harness.is_none() && opening.program.is_none();
    if opening.operator_shell || the_persons_shell {
        ProgramKind::Shell
    } else {
        ProgramKind::Harness
    }
}

/// The window's own start, as the host is handed it. Where the chat's hooks report is the
/// host's to say (#2000), so the launch says nothing of it.
fn launch_of(opening: &Opening, program: String) -> Launch {
    let confinement = opening.confinement.clone();
    Launch {
        program,
        args: opening.args.clone(),
        cwd: opening.cwd.clone(),
        set: opening.env.clone(),
        strip: opening.env_strip.clone(),
        pass: opening.env_pass.clone(),
        declared: opening.declared_env.clone(),
        harness: opening.harness.map(|harness| harness.name().to_owned()),
        git_hooks: opening
            .git_hooks
            .as_ref()
            .map(|hooks| hooks.dir().display().to_string()),
        reporting: None,
        confinement,
    }
}

/// Each call is documented once, on [`SessionHost`].
impl SessionHost for HostedSessions {
    fn open(
        &self,
        wanted: Option<u32>,
        opening: &Opening,
        announce: &dyn Fn(u32),
    ) -> Result<u32, String> {
        if !opening.operator_shell && self.kill_switch.is_stopped() {
            return Err(STOPPED.to_owned());
        }
        // A profile's credentials are handed to a program the window starts, and never over the
        // link (#2036): a chat that is handed some is not started here.
        if !opening.handed.is_empty() {
            return Err(NOT_HANDED_IN_THE_HOST.to_owned());
        }
        let (link, project) = self.link()?;
        let program = opening
            .program
            .clone()
            .unwrap_or_else(crate::sessions::shell);
        let id = self.number_for(wanted)?;
        let start = StartProgram {
            number: Some(id),
            cols: Some(opening.size.columns),
            rows: Some(opening.size.rows),
            launch: Some(launch_of(opening, program)),
            ..StartProgram::of(project, kind_of(opening))
        };
        // Before the program exists, so its very first hook lands somewhere.
        announce(id);
        let started = match link.call(Command::StartProgram(Box::new(start))) {
            // The host runs it under the number the window announced, or the window would key
            // it under one its hooks do not carry: it is closed, and the start fails.
            Ok(Answer::ProgramStarted {
                program, number, ..
            }) if number != Some(id) => {
                link.send(Command::Close { program });
                return Err(format!(
                    "the session host started this chat as number {}, not {id}, so it was \
                     closed; start it again",
                    number.map_or_else(|| "none".to_owned(), |number| number.to_string())
                ));
            }
            Ok(Answer::ProgramStarted {
                program, process, ..
            }) => (program, process),
            Ok(other) => {
                return Err(format!("the session host answered a start with {other:?}"));
            }
            Err(why) => return Err(why),
        };
        let (program, pid) = started;
        // Its program, which the host reads every line for the chat from: what a caller that
        // waited compares (qw381).
        let relayed = self.relayed();
        if let (Some(relayed), Some(pid)) = (&relayed, pid) {
            relayed.started(id, pid);
        }
        // No hook reports a program dying: the host says so, as the operating system told it.
        let tell = lock(&self.ended).clone();
        let running = Arc::clone(&self.running);
        let ended = program.clone();
        link.when_it_ends(
            &program,
            Box::new(move |exit| {
                // Only the program that ended: a chat closed and put back under its number is
                // another program, which runs on.
                match lock(&running).get_mut(&id) {
                    Some(chat) if chat.id == ended => chat.process = None,
                    _ => return,
                }
                if let Some(relayed) = &relayed {
                    relayed.ended(id, pid);
                }
                if let Some(tell) = &tell {
                    tell(
                        id,
                        match exit {
                            Exit::Code(code) => purlis_core::session::Exit::Code(code),
                            Exit::Signal(signal) => purlis_core::session::Exit::Signal(signal),
                        },
                    );
                }
            }),
        );
        lock(&self.running).insert(
            id,
            Program {
                id: program.clone(),
                process: pid,
                views: Vec::new(),
            },
        );
        // A stop that landed while this program was being started is carried out now.
        if !opening.operator_shell && self.kill_switch.is_stopped() {
            link.send(Command::Stop { chat: program });
        }
        Ok(id)
    }

    fn input(&self, id: u32, bytes: &[u8]) -> Result<(), String> {
        let (link, program) = self.program(id)?;
        link.call(Command::Write {
            chat: program,
            bytes: bytes.to_vec(),
        })
        .map(|_| ())
    }

    fn resize(&self, id: u32, size: Size) -> Result<(), String> {
        let (link, program) = self.program(id)?;
        link.call(Command::Resize {
            chat: program,
            cols: size.columns,
            rows: size.rows,
        })
        .map(|_| ())
    }

    fn watch(&self, id: u32, sink: Sink) -> Result<Watching, String> {
        let (link, program) = self.program(id)?;
        let attached = match link.call(Command::AttachView {
            program,
            kind: ViewKind::Terminal,
            role: None,
            rate: None,
        })? {
            Answer::Attached(attached) => attached,
            other => return Err(format!("the session host answered a view with {other:?}")),
        };
        link.draw(attached.view, sink);
        if let Some(program) = lock(&self.running).get_mut(&id) {
            program.views.push(attached.view);
        }
        Ok(Watching {
            view: attached.view,
            size: Size {
                columns: attached.cols,
                rows: attached.rows,
            },
        })
    }

    fn unwatch(&self, id: u32, view: u32) -> Result<(), String> {
        let (link, program) = self.program(id)?;
        let had = lock(&self.running).get_mut(&id).is_some_and(|program| {
            let had = program.views.contains(&view);
            program.views.retain(|one| *one != view);
            had
        });
        if !had {
            return Err(format!("session {id} has no view {view}"));
        }
        link.undraw(view);
        link.send(Command::DetachView { program, view });
        Ok(())
    }

    fn close(&self, id: u32) -> Result<(), String> {
        if let Some(relayed) = self.relayed() {
            relayed.ended(id, None);
        }
        let program = lock(&self.running).remove(&id).ok_or_else(|| gone(id))?;
        if let Ok(link) = self.connection.wait(Duration::ZERO) {
            for view in &program.views {
                link.undraw(*view);
            }
            // Answered once the host has let go of it, before its program has ended: its
            // number is free for a chat put back under it.
            if let Err(why) = link.call(Command::Close {
                program: program.id,
            }) {
                tracing::warn!("purlis: the session host did not close chat {id}: {why}");
            }
        }
        Ok(())
    }

    /// Every program is stopped, which answers once it is gone, then closed.
    fn end_all(&self) {
        self.to_every_program(|chat| Command::Stop { chat });
        self.to_every_program(|program| Command::Close { program });
        lock(&self.running).clear();
    }

    fn stop_every_program(&self) -> usize {
        self.to_every_program(|chat| Command::Stop { chat })
    }

    fn stopped_by(&mut self, switch: Arc<crate::killswitch::KillSwitch>) {
        self.kill_switch = switch;
    }

    fn process_id(&self, id: u32) -> Option<u32> {
        lock(&self.running).get(&id)?.process
    }

    fn when_one_ends(&self, tell: Ends) {
        *lock(&self.ended) = Some(tell);
    }

    fn running(&self) -> Vec<u32> {
        let mut ids: Vec<u32> = lock(&self.running).keys().copied().collect();
        ids.sort_unstable();
        ids
    }

    fn readiness(&self, id: u32) -> Result<Readiness, String> {
        let (link, program) = self.program(id)?;
        match link.call(Command::Readiness { program })? {
            Answer::Readiness(readiness) => Ok(Readiness {
                edits_lines: readiness.edits_lines,
                quiet_for: Duration::from_millis(readiness.quiet_for_ms),
            }),
            other => Err(format!(
                "the session host answered readiness with {other:?}"
            )),
        }
    }

    /// The host deals it (#2000). With no host there is no chat to start under it, so a
    /// number past every one the window knows is taken, and the start says why it cannot run.
    fn deal(&self) -> u32 {
        self.dealt_by_the_host().unwrap_or_else(|why| {
            tracing::warn!("purlis: the session host dealt no number ({why})");
            // Never wrapped round to a chat's that runs: past the last number, the last.
            self.opened
                .try_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
                .map_or(u32::MAX, |was| was + 1)
        })
    }

    fn dealt(&self) -> u32 {
        self.opened.load(Ordering::Relaxed)
    }

    /// Kept, and told to the host as it holds the project: a project the host holds already
    /// is told now.
    fn already_dealt(&self, dealt: u32) {
        self.opened.fetch_max(dealt, Ordering::Relaxed);
        let project = lock(&self.held).clone();
        if let (Some(project), Ok(link)) = (project, self.connection.wait(Duration::ZERO)) {
            link.send(Command::DealtAlready { project, dealt });
        }
    }

    fn reports_to(&self) -> Option<PathBuf> {
        self.relayed().map(|relayed| relayed.socket().to_path_buf())
    }

    fn hear_through(&self, relayed: Arc<Relayed>) {
        *lock(&self.relayed) = Some(relayed);
    }

    /// Saved in the session host, where this project's chats run: the window starts no worker
    /// of its own (#2002).
    fn keep_saving(
        &self,
        root: &Path,
        told: crate::autosave::Told,
    ) -> Box<dyn crate::autosave::Saver> {
        Box::new(SavedInTheHost::start(
            Holding {
                connection: Arc::clone(&self.connection),
                held: Arc::clone(&self.held),
                relayed: Arc::clone(&self.relayed),
                opened: Arc::clone(&self.opened),
                root: root.to_path_buf(),
            },
            told,
        ))
    }

    /// Started in the session host from the same plan, and held there for as long as the chat
    /// runs: what its proxy does is told to `hearing` as the host pushes it (#1999).
    fn confine(
        &self,
        plan: &purlis_core::sandbox::plan::Plan,
        hearing: purlis_core::sandbox::plan::Hearing,
    ) -> Result<purlis_core::sandbox::Confinement, String> {
        use crate::hostedconfinement::{Hears, HeldInTheHost, placed, wire};
        let (link, project) = self.link()?;
        let id = purlis_core::reopen::mint();
        let asks = hearing.asks.is_some();
        // Heard before it is asked for, so nothing its proxy does is missed.
        link.hear(&id, Hears::of(hearing));
        let answered = link.call(Command::Confine(Box::new(session::Confine {
            confinement: id.clone(),
            project,
            plan: wire(plan),
            asks,
        })));
        let confined = match answered {
            Ok(Answer::Confined(confined)) => confined,
            other => {
                link.unhear(&id);
                return Err(match other {
                    Ok(other) => format!("the session host answered a confinement with {other:?}"),
                    Err(why) => why,
                });
            }
        };
        // Held from here: dropping it, on any path, lets the host end it.
        let held = HeldInTheHost::new(id, Arc::clone(&link), asks);
        purlis_core::sandbox::Confinement::held_elsewhere(placed(&confined), Box::new(held))
            .map_err(|why| why.to_string())
    }
}

fn gone(id: u32) -> String {
    format!("session {id} is not running")
}

fn lock<T: ?Sized>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests;

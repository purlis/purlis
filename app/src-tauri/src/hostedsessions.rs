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
//! sandbox and proxy, its hooks, its `PATH`), deals its number and issues its token, and hands
//! the host that start as its [`Launch`]; the host composes the chat's environment from it with
//! the same code the window's own path runs (`purlis_core::chatspec`), against the environment
//! the window started it with. The host runs the program in a terminal of its own and the
//! window draws it through a view, types into it, resizes and stops it over the session
//! protocol.
//!
//! **One link for every project.** The window's link to its host ([`HostLink`]) carries every
//! project's chats: their views come on it, and so does word that a program ended.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use purlis_core::engine::Size;
use purlis_session_protocol::link::{Acceptor, Link};
use purlis_session_protocol::session::{
    self, Answer, Client, Command, Events, Exit, Launch, ProgramKind, Pushed, Reporting as Reports,
    StartProgram, Told, ViewKind,
};
use purlis_session_protocol::view::{Chunk, Reader, Viewer};

use crate::host::{Ends, Opening, Readiness, SessionHost, Sink, Watching};
use crate::sessions::{ENDED, Reporting, STOPPED, Utf8Stream};

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

/// How long a start waits for the host's link to be ready, before it says why there is none.
const LINK_WITHIN: Duration = Duration::from_secs(30);

/// How long a command waits for the host's answer.
const ANSWER_WITHIN: Duration = Duration::from_secs(30);

/// What a view is written before the screen of its next epoch: a full reset, so a pane drawn
/// from a fresh snapshot after its view fell behind starts from blank, as the snapshot assumes.
const RESET: &str = "\x1bc";

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
    fn wait(&self, within: Duration) -> Result<Arc<HostLink>, String> {
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

/// What a chat start says when there is no host to run it (ADR 0068, Ruled 3).
fn no_host(why: &str) -> String {
    format!(
        "Chats run in purlis's session host on this machine, and it is not running: {why}. \
         Restart purlis to start it again."
    )
}

/// What draws one view: the pane's sink, and where its stream stands.
struct Drawing {
    sink: Arc<Mutex<Sink>>,
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
}

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
        let hearing = hear(events, Arc::clone(&client), Arc::clone(&shared));
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
    fn call(&self, command: Command) -> Result<Answer, String> {
        let (answered, answer) = std::sync::mpsc::channel();
        let client = Arc::clone(&self.client);
        self.runtime.spawn(async move {
            let _ = answered.send(client.call(command).await);
        });
        match answer.recv_timeout(ANSWER_WITHIN) {
            Ok(Ok(answer)) => Ok(answer),
            Ok(Err(why)) => Err(why
                .refusal()
                .map_or_else(|| why.to_string(), |refusal| refusal.why.clone())),
            Err(_) => Err("the session host did not answer in time".into()),
        }
    }

    /// Send `command` and do not wait for its answer: what it refuses is logged.
    fn send(&self, command: Command) {
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
        let sink = Arc::new(Mutex::new(sink));
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
    fn undraw(&self, view: u32) {
        let mut views = lock(&self.shared.views);
        views.drawing.remove(&view);
        views.waiting.remove(&view);
    }

    /// Tell `tell` once program `program` ends.
    fn when_it_ends(&self, program: &str, tell: TellEnd) {
        lock(&self.shared.ends).insert(program.to_owned(), tell);
    }
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

/// Hand `text` to view `view`'s pane, if it still has one.
fn give(shared: &Shared, view: u32, text: String) {
    if text.is_empty() {
        return;
    }
    let sink = lock(&shared.views)
        .drawing
        .get(&view)
        .map(|drawing| Arc::clone(&drawing.sink));
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
                Some(Arc::clone(&one.sink))
            }
            _ => None,
        }
    };
    if let Some(sink) = sink {
        (lock(&sink))(ENDED.to_owned());
    }
}

/// What the host pushes: a program's end, and a view's.
async fn hear(mut events: Events, client: Arc<Client>, shared: Arc<Shared>) {
    while let Some(told) = events.recv().await {
        match told {
            Told::Pushed(Pushed::Ended { program, exit, .. }) => {
                let tell = lock(&shared.ends).remove(&program);
                if let Some(tell) = tell {
                    tell(exit);
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
    held: Mutex<Option<String>>,
    reporting: Option<Reporting>,
    running: Arc<Mutex<HashMap<u32, Program>>>,
    /// The highest number dealt so far: it only rises ([`crate::sessions::Sessions`]).
    opened: AtomicU32,
    ended: Mutex<Option<Ends>>,
    kill_switch: Arc<crate::killswitch::KillSwitch>,
}

impl HostedSessions {
    /// The chats of the project at `root`, run in the host `connection` reaches, reporting to
    /// `reporting`'s socket.
    pub fn new(connection: Arc<Connection>, root: &Path, reporting: Option<Reporting>) -> Self {
        HostedSessions {
            connection,
            root: root.to_path_buf(),
            held: Mutex::new(None),
            reporting,
            running: Arc::default(),
            opened: AtomicU32::new(0),
            ended: Mutex::new(None),
            kill_switch: crate::killswitch::KillSwitch::unthrown(),
        }
    }

    /// The link, with this project held on it.
    fn link(&self) -> Result<(Arc<HostLink>, String), String> {
        let link = self.connection.wait(LINK_WITHIN)?;
        let mut held = lock(&self.held);
        if let Some(project) = &*held {
            return Ok((link, project.clone()));
        }
        let project =
            purlis_core::scaffold::planefile::ensure_project_id(&self.root).map_err(|why| {
                format!("this project has no id the session host can name it by: {why}")
            })?;
        link.call(Command::HoldProject {
            project: project.clone(),
            project_path: self.root.display().to_string(),
        })?;
        *held = Some(project.clone());
        Ok((link, project))
    }

    /// The link, and the host's id of chat `id`.
    fn program(&self, id: u32) -> Result<(Arc<HostLink>, String), String> {
        let program = lock(&self.running)
            .get(&id)
            .map(|program| program.id.clone())
            .ok_or_else(|| gone(id))?;
        Ok((self.connection.wait(LINK_WITHIN)?, program))
    }

    /// The number a chat opens under: `wanted` where it can still be had, else the next
    /// (`crate::sessions::Sessions`'s rule).
    fn number_for(&self, wanted: Option<u32>) -> u32 {
        let running = lock(&self.running);
        let id = match wanted {
            Some(wanted) if wanted > 0 && !running.contains_key(&wanted) => wanted,
            _ => self.opened.load(Ordering::Relaxed) + 1,
        };
        self.opened.fetch_max(id, Ordering::Relaxed);
        id
    }

    fn forget_token(&self, id: u32) {
        if let Some(reporting) = &self.reporting {
            reporting.tokens.forget(id);
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

/// The window's own start, as the host is handed it.
fn launch_of(opening: &Opening, program: String, token: Option<(&Path, &str)>) -> Launch {
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
        reporting: token.map(|(socket, token)| Reports {
            socket: socket.display().to_string(),
            token: token.to_owned(),
        }),
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
        let (link, project) = self.link()?;
        let program = opening
            .program
            .clone()
            .unwrap_or_else(crate::sessions::shell);
        let id = self.number_for(wanted);
        let token = match &self.reporting {
            Some(reporting) => Some(
                reporting
                    .tokens
                    .issue(id)
                    .map_err(|err| format!("no token for the chat: {err}"))?,
            ),
            None => None,
        };
        let start = StartProgram {
            number: Some(id),
            cols: Some(opening.size.columns),
            rows: Some(opening.size.rows),
            launch: Some(launch_of(
                opening,
                program,
                self.reporting
                    .as_ref()
                    .zip(token.as_ref())
                    .map(|(reporting, token)| (reporting.socket.as_path(), token.expose())),
            )),
            ..StartProgram::of(
                project,
                match opening.harness {
                    Some(_) => ProgramKind::Harness,
                    None => ProgramKind::Shell,
                },
            )
        };
        // Before the program exists, so its very first hook lands somewhere.
        announce(id);
        let started = match link.call(Command::StartProgram(Box::new(start))) {
            // The host runs it under the number the window announced, or the window would key
            // it under one its hooks do not carry: it is closed, and the start fails.
            Ok(Answer::ProgramStarted {
                program, number, ..
            }) if number != Some(id) => {
                self.forget_token(id);
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
                self.forget_token(id);
                return Err(format!("the session host answered a start with {other:?}"));
            }
            Err(why) => {
                self.forget_token(id);
                return Err(why);
            }
        };
        let (program, pid) = started;
        // Its program, which every line for the chat must come from or from inside (D-1407-6),
        // as the window's own path binds it.
        if let (Some(reporting), Some(pid)) = (&self.reporting, pid) {
            match purlis_core::hookwire::Program::of(pid) {
                Some(bound) => reporting.tokens.bind(id, bound),
                None => tracing::warn!(
                    "purlis: chat {id}'s program (pid {pid}) could not be confirmed as it \
                     started, so none of its lines will be read until it is restarted"
                ),
            }
        }
        // No hook reports a program dying: the host says so, as the operating system told it.
        let tell = lock(&self.ended).clone();
        let tokens = self
            .reporting
            .as_ref()
            .map(|reporting| Arc::clone(&reporting.tokens));
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
                if let (Some(tokens), Some(pid)) = (&tokens, pid) {
                    tokens.program_ended(id, pid);
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
        self.forget_token(id);
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

    fn deal(&self) -> u32 {
        self.opened.fetch_add(1, Ordering::Relaxed) + 1
    }

    fn dealt(&self) -> u32 {
        self.opened.load(Ordering::Relaxed)
    }

    fn already_dealt(&self, dealt: u32) {
        self.opened.fetch_max(dealt, Ordering::Relaxed);
    }

    fn reports_to(&self) -> Option<PathBuf> {
        self.reporting
            .as_ref()
            .map(|reporting| reporting.socket.clone())
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

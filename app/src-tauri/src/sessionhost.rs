// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The session host the window starts, and is the first client of (ADR 0068 §1, #1997).
//!
//! **The window's own binary runs the host.** The app starts itself again with
//! [`purlis_host::SERVE_FOR_THE_WINDOW`] (`main.rs` answers it before anything of the app
//! starts), so the host is the same program, with the same Keychain identity, and nothing
//! prompts (ADR 0068 §1, amended 2026-10-14). One host serves every project open in the window.
//!
//! **It lives as long as the window.** The window holds the other end of the host's standard
//! input; the host stops when it closes, so a quit, a crash and a kill all end it. At a quit the
//! window closes it and waits a moment for the host to go ([`SessionHost::end`]).
//!
//! **The window supervises it** (ADR 0068 §3, #2007): a host that ends without the window
//! asking, or whose link to the window ends, is started again, until three such ends within
//! five minutes trip the breaker ([`breaker`]); then the window says the host is failing, with
//! its last lines and Try again (`crate::hostfailing`). Every chat the host ran ends with it and
//! comes back as failed or hibernated with Resume (`crate::hostlost`), and the new host settles
//! what the last one left from its run journal ([`Kept`]).
//!
//! **Its folder is made first, the host is started off the first paint.** The socket's folder
//! is made this user's alone on the window's own start, before any chat can start, so every
//! chat's sandbox denies a folder that is there; the host then starts on a thread of its own,
//! and nothing the window draws waits for it. Once it says it is ready, the window connects as `local-ui` with the
//! credential the host minted, and holds the link.
//!
//! **Chats run in it behind the start-time switch** (#1998, `crate::hostedsessions`): the link
//! the window holds is what every project's chats run over when the switch is on, and with no
//! host of the window's own, those chats are refused with the reason ([`Connection`]). Wherever
//! chats run, the host takes every process below the window as inside a chat and refuses it a
//! person's scope (`purlis_host::Window`). What this host says goes to the window's log.
//!
//! **While chats run in it, it writes the event log** (#2003, [`Kept`]): it opens the log as it
//! first holds a project, which only a window running its chats here asks, records each line
//! its hook channels hear with what the window's hearing made of it, drains their spools, writes
//! what the window decided, and keeps what each program's end leaves, window or no window. The
//! window then opens no log at all.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use purlis_session_protocol::auth::{Credential, Scope};
use purlis_session_protocol::{link, session};

use crate::hostedsessions::{Connection, HostLink};

/// How long the window waits for its host to say it is ready.
const READY_WITHIN: Duration = Duration::from_secs(20);

/// How long a quit waits for the host to end once its standard input is closed, before it is
/// killed.
const ENDS_WITHIN: Duration = Duration::from_secs(2);

/// The exit code a host gives when another host of this user already serves on this device
/// (`purlis_host::HostError::AlreadyServing`).
const ANOTHER_SERVES: i32 = 3;

/// Where this user's session host listens and keeps its credentials on this device: the place
/// the window's host takes and `purlis serve` takes, so each finds the other.
pub fn place() -> Option<purlis_host::Place> {
    let config = purlis_core::machine::config_root()?;
    Some(purlis_host::Place::at(
        purlis_core::hostsocket::here(),
        purlis_core::machine::charterd(&config),
    ))
}

/// The command that starts `binary`, the window's own, as its session host: the host's
/// standard input is the window's to hold, and what it says is read for the window's log.
pub fn command(binary: &Path) -> Command {
    let mut command = Command::new(binary);
    command
        .arg(purlis_host::SERVE_FOR_THE_WINDOW)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
}

/// What a host that did not become ready said by its exit, for the window's log.
pub fn why_not_ready(code: Option<i32>) -> &'static str {
    match code {
        Some(ANOTHER_SERVES) => {
            "another session host already serves this user on this device; this window \
             leaves it serving and does not connect to it"
        }
        _ => "the session host ended before it was ready",
    }
}

/// A chat the window composed, as the window would start it: the core's composition, against
/// the environment the window started this host with (#1998). The host's [`purlis_host::Compose`]
/// for the window's binary.
pub fn launched(
    launch: &session::Launch,
    number: u32,
    size: purlis_core::engine::Size,
) -> Result<purlis_core::session::Spec, String> {
    purlis_core::chatspec::launched(
        std::env::vars_os(),
        &purlis_core::chatspec::Launched {
            program: &launch.program,
            args: &launch.args,
            cwd: launch.cwd.as_deref(),
            set: &launch.set,
            strip: &launch.strip,
            pass: &launch.pass,
            declared: &launch.declared,
            harness: launch.harness.as_deref(),
            git_hooks: launch.git_hooks.as_deref(),
            reporting: launch.reporting.as_ref().map(|reporting| {
                purlis_core::chatspec::Reporting {
                    socket: Path::new(&reporting.socket),
                    token: &reporting.token,
                }
            }),
        },
        number,
        size,
    )
}

/// **The event log in the window's session host, and what a program's end leaves there**
/// (#2003): the host's [`purlis_host::Record`] for the window's binary, shared with its hook
/// channels ([`HookChannels`]).
///
/// The log is opened the first time it is needed, which is as the host first holds a project:
/// only a window whose chats run in the host holds one, so a host whose window keeps the log
/// itself never opens it, and the device has one writer either way. One that cannot be opened
/// is said once in the host's log, and every write is refused in words from then on.
#[derive(Clone)]
pub struct Kept(Arc<KeptLog>);

struct KeptLog {
    open: Box<dyn Fn() -> std::io::Result<purlis_core::eventlog::Recorder> + Send + Sync>,
    opened: std::sync::OnceLock<Option<crate::hooks::Events>>,
}

impl Kept {
    /// The event log of this machine, as the window would open it
    /// (`purlis_core::eventlog::Recorder::open`).
    pub fn on_this_machine() -> Kept {
        Kept::opening(purlis_core::eventlog::Recorder::open)
    }

    /// The event log `open` opens, the first time it is needed.
    pub fn opening(
        open: impl Fn() -> std::io::Result<purlis_core::eventlog::Recorder> + Send + Sync + 'static,
    ) -> Kept {
        Kept(Arc::new(KeptLog {
            open: Box::new(open),
            opened: std::sync::OnceLock::new(),
        }))
    }

    /// The log, opened now if it was not yet: `None` where it could not be.
    fn events(&self) -> Option<&crate::hooks::Events> {
        self.0
            .opened
            .get_or_init(|| match (self.0.open)() {
                Ok(recorder) => Some(crate::hooks::Events::here(recorder)),
                Err(why) => {
                    purlis_host_says(&format!(
                        "no event log ({why}); what chats do in this host is not recorded"
                    ));
                    None
                }
            })
            .as_ref()
    }

    /// The recorder itself, for a drain of a hook spool into it.
    fn log(&self) -> Option<Arc<Mutex<purlis_core::eventlog::Recorder>>> {
        self.events().and_then(crate::hooks::Events::held).cloned()
    }

    /// Writes `entry` durably: what the window decided, or what its hearing made of a line.
    fn write(&self, entry: purlis_core::eventlog::Entry) -> std::io::Result<()> {
        match self.events() {
            Some(events) => events.record(entry),
            None => Err(std::io::Error::other(NO_EVENT_LOG)),
        }
    }
}

/// What a write is refused with in a host whose event log could not be opened.
const NO_EVENT_LOG: &str = "the session host has no event log on this machine, so nothing was \
     recorded";

impl purlis_host::Record for Kept {
    fn record(&self, entry: serde_json::Value) -> Result<(), String> {
        let entry: purlis_core::eventlog::Entry = serde_json::from_value(entry).map_err(|why| {
            format!("the window asked for a write this host does not read: {why}")
        })?;
        self.write(entry).map_err(|why| why.to_string())
    }

    /// **What a chat's program's end leaves, kept in the host** (#2003): what its hooks spooled
    /// is recorded, and its sub-agents' runs end with its own in the end state and for the
    /// cause the host's own act or its exit gives (ADR 0076 §2, §6), window or no window. Where
    /// no window held its project as it ended by itself, the dispatch it worked ends as one
    /// that went without its report, kept for its asking chat
    /// (`purlis_core::dispatchrecord::ended_unwatched`); a window that holds the project decides
    /// that itself, from the board and the asking chat it holds (D-2000-1, #2194).
    fn ended(&self, ended: &purlis_host::Ended<'_>) {
        let Some(log) = self.log() else { return };
        let (root, number) = (ended.project_path, ended.number);
        let socket = crate::hooks::socket_for(Some(root)).socket;
        crate::hooks::drain_while_running(
            &purlis_core::hookwire::spool::dir_for(&socket),
            root,
            &log,
            number,
        );
        let exit = match &ended.exit {
            session::Exit::Code(code) => purlis_core::session::Exit::Code(*code),
            session::Exit::Signal(signal) => purlis_core::session::Exit::Signal(signal.clone()),
        };
        let (state, cause) = crate::planes::how_its_run_ended(ended.stopped.map(stop_by), &exit);
        crate::planes::its_children_end(&log, root, number, state, cause);
        if ended.watched || ended.stopped.is_some() {
            return;
        }
        let chat = log
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .chat_of(root, number);
        if let Some(chat) = chat {
            purlis_core::dispatchrecord::ended_unwatched(root, &chat, number, chrono::Utc::now());
        }
    }

    fn left_by_its_last_host(&self, left: &purlis_host::Started) {
        self.left_by_a_crash(left);
    }
}

/// **What a chat the last host ran leaves, kept by the host that took its place** (#2007): the
/// last host ended without ending it, so what its hooks spooled is recorded now, and its
/// sub-agents' runs end with its own, `failed` for the cause `host-crash` (ADR 0076 §5, §6).
impl Kept {
    fn left_by_a_crash(&self, left: &purlis_host::Started) {
        use purlis_core::state::run::{Cause, RunState};
        let Some(log) = self.log() else { return };
        let root = left.project_path.as_path();
        let socket = crate::hooks::socket_for(Some(root)).socket;
        crate::hooks::drain_while_running(
            &purlis_core::hookwire::spool::dir_for(&socket),
            root,
            &log,
            left.number,
        );
        crate::planes::its_children_end(
            &log,
            root,
            left.number,
            RunState::Failed,
            Cause::HostCrash,
        );
    }
}

/// The host's own act that ended a program, as a run's end names it (ADR 0076 §2).
fn stop_by(stopped: purlis_host::Stopped) -> purlis_core::state::run::StopBy {
    use purlis_core::state::run::StopBy;
    match stopped {
        purlis_host::Stopped::Closed => StopBy::Closed,
        purlis_host::Stopped::Operator => StopBy::Operator,
        purlis_host::Stopped::Quit => StopBy::Quit,
        purlis_host::Stopped::Killed => StopBy::Killed,
    }
}

/// What hears each held project's hooks in the window's session host (#2000): the host's
/// [`purlis_host::Hear`] for the window's binary.
///
/// **The project's hook channel, as the window bound it before.** At the same place
/// ([`crate::hooks::socket_for`]), with the same listener and every one of its checks: the
/// token, the process that sent it, each framed ask again, the spool's keys beside it. Only who
/// listens changes, and the host is each chat's ancestor, so a hook admits it. Each line it
/// admits is handed on to the window ([`purlis_core::hookwire::relay`]).
///
/// **And recorded here** (#2003): what the window's hearing would have written of a line comes
/// back with its reply and is written in the event log the host keeps ([`Kept`]), on the thread
/// that heard the line; what a hook spooled is drained into the same log as the project is held,
/// soon after a chat is heard from, and as a chat is closed. So a line and its spooled copy are
/// told apart in one process, and recorded once (#952).
///
/// **It keeps the project's kept board** (#2194, [`KeptBoard`]), which outlives the window.
pub struct HookChannels(pub Kept);

/// **A held project's kept board, in the session host** (#2194, CONTEXT.md,
/// `purlis_core::state::kept`).
///
/// The window makes each move on its own copy and tells this one ([`KeptBoard::moved`]), so
/// the two read the same; a window that holds the project takes this one whole
/// ([`KeptBoard::whole`]). A line no window took moves it here, as the window's own hearing
/// would with nothing said of what a chat waits on ([`KeptBoard::handing_on`]), and so does a
/// program that ends with no window there ([`KeptBoard::ended`]): nothing a chat says while the
/// window is away is lost.
#[derive(Clone, Default)]
pub struct KeptBoard(Arc<Mutex<purlis_core::state::Board>>);

/// The most a board answered to a window may be, its frame's envelope aside.
const A_BOARD_IS_AT_MOST: usize = purlis_session_protocol::link::MOST_CONTROL_FRAME_BYTES - 4096;

impl KeptBoard {
    fn board(&self) -> std::sync::MutexGuard<'_, purlis_core::state::Board> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// What hands each line on through `hand_on`, the board moved by one no window took.
    pub fn handing_on(&self, hand_on: purlis_host::HandOn) -> purlis_core::hookwire::relay::HandOn {
        let kept = self.clone();
        Box::new(move |id, heard| {
            let told = serde_json::to_value(heard).is_ok_and(|line| hand_on(id, line));
            if !told {
                kept.board().hear(heard);
            }
            told
        })
    }

    /// The window made `moved` of chat `number` on its own board: whether this host read it.
    /// One it cannot read moves nothing, and is said in the log.
    pub fn moved(&self, number: u32, moved: serde_json::Value) -> bool {
        match serde_json::from_value::<purlis_core::state::Move>(moved) {
            Ok(moved) => {
                self.board().make(number, moved);
                true
            }
            Err(why) => {
                purlis_host_says(&format!(
                    "a move of chat {number} the window made could not be read, so the board \
                     kept here did not make it: {why}"
                ));
                false
            }
        }
    }

    /// The board, whole, for a window that holds the project. One larger than the link carries
    /// is answered as none, and said in the log: the window then starts from its own.
    pub fn whole(&self) -> serde_json::Value {
        let kept = serde_json::to_value(self.board().kept()).unwrap_or(serde_json::Value::Null);
        let size = serde_json::to_vec(&kept).map_or(usize::MAX, |said| said.len());
        if size > A_BOARD_IS_AT_MOST {
            purlis_host_says(&format!(
                "the board kept here is {size} bytes, more than the link carries \
                 ({A_BOARD_IS_AT_MOST}), so the window starts from its own"
            ));
            return serde_json::Value::Null;
        }
        kept
    }

    /// Chat `number`'s program ended, `exit`, with no window there to say so.
    pub fn ended(&self, number: u32, exit: &session::Exit) {
        let exit = match exit {
            session::Exit::Code(code) => purlis_core::session::Exit::Code(*code),
            session::Exit::Signal(signal) => purlis_core::session::Exit::Signal(signal.clone()),
        };
        self.board().exited(number, crate::hooks::code_of(&exit));
    }
}

impl purlis_host::Hear for HookChannels {
    fn open(
        &self,
        _project: &str,
        path: &Path,
        hand_on: purlis_host::HandOn,
    ) -> Result<Option<Arc<dyn purlis_host::Channel>>, String> {
        use purlis_core::hookwire::relay::{Keeping, Relaying};
        let at = crate::hooks::socket_for(Some(path));
        let listener = purlis_core::hookwire::Listener::bind(&at.within, &at.socket)
            .map_err(|why| format!("its hook socket at {} ({why})", at.socket.display()))?;
        let socket = listener.path().to_path_buf();
        // What its chats' hooks spooled while nobody took their lines, before a chat is started
        // here and issued a token (FD-30), as the window drains it as it binds the socket.
        let kept = self.0.clone();
        crate::hooks::drain_at_open(&socket, path, kept.log().as_deref());
        let tokens = listener.tokens();
        let board = KeptBoard::default();
        // A chat's spool drained while it runs (#952), set off by its lines.
        let spooled = {
            let (dir, root, kept) = (
                purlis_core::hookwire::spool::dir_for(&socket),
                path.to_path_buf(),
                kept.clone(),
            );
            crate::hooks::watching_spools(move |chat| {
                if let Some(log) = kept.log() {
                    crate::hooks::drain_while_running(&dir, &root, &log, chat);
                }
            })
        };
        let relaying = Relaying::keeping(
            board.handing_on(hand_on),
            Keeping {
                record: {
                    let (kept, root) = (kept.clone(), path.to_path_buf());
                    // A line of this project's is recorded under this project.
                    Box::new(move |entry| match &entry {
                        purlis_core::eventlog::Entry::Report { plane, .. }
                        | purlis_core::eventlog::Entry::Tool { plane, .. }
                        | purlis_core::eventlog::Entry::Refused { plane, .. }
                            if *plane == root =>
                        {
                            kept.write(entry)
                        }
                        _ => Err(std::io::Error::other(
                            "what the window sent back to record is not of this project's line",
                        )),
                    })
                },
                heard_from: Box::new(move |chat| spooled.heard_from(chat)),
            },
        );
        let reading = listener.hear(relaying.hearing(Arc::clone(&tokens)));
        Ok(Some(Arc::new(HookChannel {
            socket,
            root: path.to_path_buf(),
            kept,
            tokens,
            relaying,
            board,
            reading: Mutex::new(Some(reading)),
        })))
    }
}

/// One held project's hook channel in the host.
struct HookChannel {
    socket: PathBuf,
    /// Where its project is on this machine: what its chats are recorded under.
    root: PathBuf,
    kept: Kept,
    tokens: Arc<purlis_core::hookwire::ChatTokens>,
    relaying: Arc<purlis_core::hookwire::relay::Relaying>,
    /// The project's board, kept here (#2194).
    board: KeptBoard,
    reading: Mutex<Option<purlis_core::hookwire::Reading>>,
}

impl purlis_host::Channel for HookChannel {
    fn report_to(&self, number: u32) -> Result<session::Reporting, String> {
        let token = self.tokens.issue(number).map_err(|why| why.to_string())?;
        Ok(session::Reporting {
            socket: self.socket.display().to_string(),
            token: token.expose().to_owned(),
        })
    }

    fn bind(&self, number: u32, pid: u32) {
        match purlis_core::hookwire::Program::of(pid) {
            Some(program) => self.tokens.bind(number, program),
            None => purlis_host_says(&format!(
                "chat {number}'s program (pid {pid}) could not be confirmed as it started, so \
                 none of its lines will be read until it is restarted"
            )),
        }
    }

    fn ended(&self, number: u32, pid: u32) {
        self.tokens.program_ended(number, pid);
    }

    fn forget(&self, number: u32) {
        self.tokens.forget(number);
        // What it spooled is recorded and its keys dropped, off the host's runtime: a drain
        // makes each line durable (V99i, #2003).
        let (socket, root, kept) = (self.socket.clone(), self.root.clone(), self.kept.clone());
        let drained = std::thread::Builder::new()
            .name("purlisd-spool-ended".into())
            .spawn(move || {
                crate::hooks::spool_ended(&socket, &root, kept.log().as_deref(), number);
            });
        if let Err(why) = drained {
            purlis_host_says(&format!(
                "chat {number}'s hook spool was not drained as it was closed ({why}); it is \
                 drained at the next hold of its project"
            ));
        }
    }

    fn replied(&self, id: u64, reply: serde_json::Value) {
        match serde_json::from_value(reply) {
            Ok(replied) => self.relaying.replied(id, replied),
            Err(why) => {
                purlis_host_says(&format!("a reply the window sent could not be read: {why}"))
            }
        }
    }

    fn moved(&self, number: u32, moved: serde_json::Value) {
        self.board.moved(number, moved);
    }

    fn board(&self) -> serde_json::Value {
        self.board.whole()
    }

    fn ended_with_no_window(&self, number: u32, exit: &session::Exit) {
        self.board.ended(number, exit);
    }
}

/// The channel stops: every line still waiting on the window is let go of, then the socket.
impl Drop for HookChannel {
    fn drop(&mut self) {
        self.relaying.let_go();
        drop(
            self.reading
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .take(),
        );
    }
}

/// One line in the host's log, which the window keeps.
fn purlis_host_says(line: &str) {
    use std::io::Write;
    let _ = writeln!(std::io::stderr(), "purlisd: {line}");
}

/// The window's session host, while it runs, and its supervisor (#2007).
#[derive(Default)]
pub struct SessionHost {
    held: Arc<Mutex<Option<Held>>>,
    /// The link to it, for the chats that run in it.
    connection: Arc<Connection>,
    supervision: Arc<Supervision>,
}

struct Held {
    child: Child,
    /// The host's standard input: closing it ends the host.
    stdin: Option<ChildStdin>,
}

/// **The window supervises its host** (ADR 0068 §3, #2007): no service manager may be used
/// for a host that is not a login item, so the window is the one that starts it again. What the
/// supervisor shares with the window's other threads.
#[derive(Default)]
struct Supervision {
    /// The window is ending its host: nothing starts it again.
    ending: std::sync::atomic::AtomicBool,
    /// What wakes the supervisor.
    woken: Mutex<Woken>,
    changed: std::sync::Condvar,
    breaker: Mutex<breaker::Breaker>,
    /// The last lines the host wrote to its log, oldest first.
    said: Mutex<std::collections::VecDeque<String>>,
    /// What the window says while the breaker holds the host stopped.
    failing: Arc<crate::hostfailing::HostFailing>,
}

#[derive(Default)]
struct Woken {
    /// The window's link to the host ended without the window ending it.
    link_lost: bool,
    /// The person pressed Try again.
    try_again: bool,
}

/// How many of the host's last lines the window keeps, to say them when it is failing.
const LINES_KEPT: usize = 20;

/// How often the supervisor looks at its host when nothing wakes it.
const LOOK_EVERY: Duration = Duration::from_millis(100);

impl Supervision {
    fn ending(&self) -> bool {
        self.ending.load(std::sync::atomic::Ordering::SeqCst)
    }

    fn wake(&self, how: impl FnOnce(&mut Woken)) {
        how(&mut self.woken.lock().unwrap_or_else(PoisonError::into_inner));
        self.changed.notify_all();
    }

    /// The link ended: a host the window cannot speak to is ended, and started again.
    fn link_lost(&self) {
        self.wake(|woken| woken.link_lost = true);
    }

    fn heard(&self, line: &str) {
        let mut said = self.said.lock().unwrap_or_else(PoisonError::into_inner);
        if said.len() == LINES_KEPT {
            said.pop_front();
        }
        said.push_back(line.to_owned());
    }

    /// Waits up to [`LOOK_EVERY`] for something to wake it, and answers what did.
    fn rest(&self) -> Woken {
        let woken = self.woken.lock().unwrap_or_else(PoisonError::into_inner);
        let (mut woken, _) = self
            .changed
            .wait_timeout_while(woken, LOOK_EVERY, |woken| {
                !woken.link_lost && !woken.try_again && !self.ending()
            })
            .unwrap_or_else(PoisonError::into_inner);
        std::mem::take(&mut *woken)
    }
}

impl SessionHost {
    /// Makes the host's socket folder this user's alone now, on the calling thread, so that
    /// every chat's sandbox, compiled from here on, denies a folder that is there; then starts
    /// the host on a thread of its own and connects to it once it is ready, and supervises it
    /// from there. Nothing waits for the host; what goes wrong is said in the log.
    pub fn start(&self) {
        let Some(place) = place() else {
            let why = "there is no home folder to keep its credentials in";
            tracing::warn!("purlis: no session host: {why}");
            self.connection.gone(why);
            return;
        };
        self.start_at(place, || {
            std::env::current_exe()
                .map(|binary| command(&binary))
                .map_err(|why| format!("this program's path is unknown: {why}"))
        });
    }

    /// [`SessionHost::start`], the host serving at `place` and started, each time, by the
    /// command `starts` gives, on the host's own thread: the window's own binary as [`command`]
    /// makes it, or, for the behaviour suite, that binary in an environment of the test's
    /// (#2170).
    pub(crate) fn start_at(
        &self,
        place: purlis_host::Place,
        starts: impl Fn() -> Result<Command, String> + Send + Sync + 'static,
    ) {
        let connection = &self.connection;
        if let Err(why) = purlis_core::hostsocket::make_the_folder(&place.socket) {
            let why = format!(
                "its folder {} could not be made this user's alone: {why}",
                place.socket.parent().unwrap_or(&place.socket).display()
            );
            tracing::warn!("purlis: no session host: {why}");
            connection.gone(why);
            return;
        }
        let held = Arc::clone(&self.held);
        let connection = Arc::clone(connection);
        let supervision = Arc::clone(&self.supervision);
        let try_again = Arc::clone(&self.supervision);
        self.supervision.failing.tried_again_by(Arc::new(move || {
            try_again.wake(|woken| woken.try_again = true)
        }));
        let spawned = std::thread::Builder::new()
            .name("session host".into())
            .spawn({
                let connection = Arc::clone(&connection);
                move || supervise(&held, &place, &starts, &connection, &supervision)
            });
        if let Err(why) = spawned {
            tracing::warn!("purlis: the session host was not started: {why}");
            connection.gone(format!("it was not started: {why}"));
        }
    }

    /// The link to the host, for the chats that run in it: ready once the host is, or gone with
    /// the reason there is none.
    pub fn connection(&self) -> Arc<Connection> {
        Arc::clone(&self.connection)
    }

    /// What kills the host running now outright, as a crash or `kill -9` ends it, leaving the
    /// window to see it go: the behaviour suite's crash test (#2007).
    #[cfg(test)]
    pub(crate) fn killer(&self) -> Arc<dyn Fn() + Send + Sync> {
        let held = Arc::clone(&self.held);
        Arc::new(move || {
            if let Some(host) = held.lock().unwrap_or_else(PoisonError::into_inner).as_mut() {
                let _ = host.child.kill();
            }
        })
    }

    /// What the window says while the host is failing, and its Try again (#2007).
    pub fn failing(&self) -> Arc<crate::hostfailing::HostFailing> {
        Arc::clone(&self.supervision.failing)
    }

    /// Ends the host: nothing starts it again, its standard input is closed, it is given
    /// [`ENDS_WITHIN`] to go, and is killed if it has not. For the app's exit.
    pub fn end(&self) {
        self.supervision
            .ending
            .store(true, std::sync::atomic::Ordering::SeqCst);
        self.supervision.wake(|_| {});
        end_held(&self.held);
    }

    /// What ends the host from elsewhere, as [`Self::end`] does: a quit that the host did not
    /// answer in time ends it before the save at quit (#2002). Ending it twice ends it once.
    pub fn ender(&self) -> Arc<dyn Fn() + Send + Sync> {
        let held = Arc::clone(&self.held);
        let supervision = Arc::clone(&self.supervision);
        Arc::new(move || {
            supervision
                .ending
                .store(true, std::sync::atomic::Ordering::SeqCst);
            supervision.wake(|_| {});
            end_held(&held);
        })
    }
}

/// Ends the host `held` holds, if it still does: closes its standard input, waits
/// [`ENDS_WITHIN`] for it to go, and kills it if it has not.
fn end_held(held: &Mutex<Option<Held>>) {
    let Some(mut held) = held.lock().unwrap_or_else(|e| e.into_inner()).take() else {
        return;
    };
    drop(held.stdin.take());
    let deadline = Instant::now() + ENDS_WITHIN;
    while Instant::now() < deadline {
        if matches!(held.child.try_wait(), Ok(Some(_))) {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let _ = held.child.kill();
    let _ = held.child.wait();
}

/// How a start of the host went.
enum Started {
    /// It serves, and the window connects to it.
    Serving,
    /// Another host of this user already serves: this window leaves it be.
    AnotherServes,
    /// It did not start, or ended before it was ready.
    Failed,
}

/// **The supervisor** (ADR 0068 §3, #2007), on the host's own thread: starts the host, and
/// after each unexpected exit starts it again, until its breaker trips; then says the host is
/// failing and waits for the person's Try again. Nothing starts it once the window ends it.
///
/// An unexpected exit is any end the window did not ask for: the host crashed or was killed, it
/// ended before it was ready, or the window's link to it ended, after which the host is ended
/// first. Each chat that ran in it has ended with it (`crate::hostedsessions::HostLink`), and the
/// next host settles what the last one left from its run journal.
fn supervise(
    held: &Mutex<Option<Held>>,
    place: &purlis_host::Place,
    starts: &(dyn Fn() -> Result<Command, String> + Send + Sync),
    connection: &Arc<Connection>,
    supervision: &Arc<Supervision>,
) {
    loop {
        if supervision.ending() {
            return;
        }
        match start_here(held, place, starts, connection, supervision) {
            Started::AnotherServes => return,
            Started::Serving => wait_for_an_unexpected_end(held, supervision),
            Started::Failed => {}
        }
        if supervision.ending() {
            return;
        }
        // Every start from here waits for the next host, as the first waited for the first.
        connection.restarting();
        let after = supervision
            .breaker
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .exited(Instant::now());
        if after == breaker::After::StartAgain {
            tracing::warn!("purlis: the session host ended unexpectedly; starting it again");
            continue;
        }
        let said: Vec<String> = supervision
            .said
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .cloned()
            .collect();
        let why = format!(
            "it ended unexpectedly {} times within {} minutes, so purlis stopped starting it \
             again",
            breaker::EXITS,
            breaker::WITHIN.as_secs() / 60
        );
        tracing::warn!("purlis: the session host is failing: {why}");
        connection.gone(why);
        supervision.failing.failing(crate::hostfailing::Failing {
            exits: u32::try_from(breaker::EXITS).unwrap_or(u32::MAX),
            within_minutes: u32::try_from(breaker::WITHIN.as_secs() / 60).unwrap_or(u32::MAX),
            said,
        });
        loop {
            let woken = supervision.rest();
            if supervision.ending() {
                return;
            }
            if woken.try_again {
                break;
            }
        }
        tracing::info!("purlis: the session host is tried again");
        supervision
            .breaker
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .reset();
        supervision.failing.not_failing();
        connection.restarting();
    }
}

/// Returns once the host has ended without the window ending it, or the window is ending it.
/// A host whose link to the window ended is ended here first: the window cannot speak to it.
fn wait_for_an_unexpected_end(held: &Mutex<Option<Held>>, supervision: &Supervision) {
    loop {
        let woken = supervision.rest();
        let mut held = held.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(host) = held.as_mut() else {
            // The window took it to end it.
            return;
        };
        match host.child.try_wait() {
            Ok(Some(status)) => {
                tracing::warn!("purlis: the session host ended ({status})");
                *held = None;
                return;
            }
            Ok(None) if woken.link_lost && !supervision.ending() => {
                tracing::warn!(
                    "purlis: the window's link to the session host ended while it ran; it is \
                     ended, to be started again"
                );
                let _ = host.child.kill();
            }
            Ok(None) => {}
            Err(why) => {
                tracing::warn!("purlis: the session host could not be waited for: {why}");
                let _ = host.child.kill();
            }
        }
    }
}

/// Starts the host with the command `starts` gives and connects to it, on the calling thread.
fn start_here(
    held: &Mutex<Option<Held>>,
    place: &purlis_host::Place,
    starts: &(dyn Fn() -> Result<Command, String> + Send + Sync),
    connection: &Arc<Connection>,
    supervision: &Arc<Supervision>,
) -> Started {
    let mut command = match starts() {
        Ok(command) => command,
        Err(why) => {
            tracing::warn!("purlis: no session host: {why}");
            connection.gone(why);
            return Started::Failed;
        }
    };
    let mut child = match purlis_core::forklock::spawn(&mut command) {
        Ok(child) => child,
        Err(why) => {
            tracing::warn!("purlis: the session host did not start: {why}");
            connection.gone(format!("it did not start: {why}"));
            return Started::Failed;
        }
    };
    if let Some(said) = child.stderr.take() {
        log_what_it_says(said, Arc::clone(supervision));
    }
    let stdout = child.stdout.take();
    let stdin = child.stdin.take();
    let pid = child.id();
    *held.lock().unwrap_or_else(|e| e.into_inner()) = Some(Held { child, stdin });

    if !stdout.is_some_and(said_ready) {
        let code = wait_for_its_end(held, Instant::now() + ENDS_WITHIN);
        tracing::warn!("purlis: {} (process {pid})", why_not_ready(code));
        connection.gone(why_not_ready(code));
        if code == Some(ANOTHER_SERVES) {
            return Started::AnotherServes;
        }
        // One that never said it was ready and still runs is ended: it is started again.
        if let Some(mut host) = held.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let _ = host.child.kill();
            let _ = host.child.wait();
        }
        return Started::Failed;
    }
    tracing::info!(
        "purlis: the session host serves at {}",
        place.socket.display()
    );
    connect(
        place.socket.clone(),
        place.credentials.clone(),
        Arc::clone(connection),
        Arc::clone(supervision),
    );
    Started::Serving
}

/// The host's exit code, once it has ended by `deadline`.
fn wait_for_its_end(held: &Mutex<Option<Held>>, deadline: Instant) -> Option<i32> {
    while Instant::now() < deadline {
        let mut held = held.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(Ok(Some(status))) = held.as_mut().map(|held| held.child.try_wait()) {
            *held = None;
            return status.code();
        }
        drop(held);
        std::thread::sleep(Duration::from_millis(20));
    }
    None
}

/// Whether the host's first line says it is ready, within [`READY_WITHIN`]. A host that ends
/// first, or says anything else, is not.
fn said_ready(stdout: std::process::ChildStdout) -> bool {
    let (told, heard) = std::sync::mpsc::channel();
    let reading = std::thread::Builder::new()
        .name("session host ready".into())
        .spawn(move || {
            let mut line = String::new();
            let read = BufReader::new(stdout).read_line(&mut line);
            let _ = told.send(read.is_ok() && line.trim_end() == purlis_host::READY);
        });
    reading.is_ok() && heard.recv_timeout(READY_WITHIN).unwrap_or(false)
}

/// Each line the host says on its standard error, into the window's log, the last of them kept
/// to say when the host is failing.
fn log_what_it_says(said: std::process::ChildStderr, supervision: Arc<Supervision>) {
    let _ = std::thread::Builder::new()
        .name("session host log".into())
        .spawn(move || {
            for line in BufReader::new(said).lines().map_while(Result::ok) {
                tracing::info!("{line}");
                supervision.heard(&line);
            }
        });
}

/// Connect to the host as `local-ui`, the window's scope, and hold the link for as long as the
/// window runs: the chats that run in the host go over it ([`HostLink`]). A link that cannot be
/// made, or ends, wakes the supervisor.
fn connect(
    socket: PathBuf,
    credentials: PathBuf,
    connection: Arc<Connection>,
    supervision: Arc<Supervision>,
) {
    tauri::async_runtime::spawn(async move {
        let lost = |why: String| {
            tracing::warn!("purlis: {why}");
            connection.gone(why);
            supervision.link_lost();
        };
        let credential = match Credential::read(&credentials, Scope::LocalUi) {
            Ok(credential) => credential,
            Err(why) => {
                return lost(format!("its credential could not be read: {why}"));
            }
        };
        let linked = match tokio::net::UnixStream::connect(&socket).await {
            Ok(stream) => {
                link::connect(stream, session::speaks(), Scope::LocalUi, &credential).await
            }
            Err(why) => return lost(format!("its socket did not answer: {why}")),
        };
        let link = match linked {
            Ok(link) => link,
            Err(why) => return lost(format!("it refused the window: {why}")),
        };
        let ended = {
            let connection = Arc::clone(&connection);
            let supervision = Arc::clone(&supervision);
            move || {
                if supervision.ending() {
                    tracing::info!("purlis: the link to the session host ended");
                    connection.gone("the link to it ended");
                    return;
                }
                tracing::warn!("purlis: the link to the session host ended unexpectedly");
                connection.restarting();
                supervision.link_lost();
            }
        };
        match HostLink::over(link, tokio::runtime::Handle::current(), ended).await {
            Ok(link) => connection.ready(Arc::new(link)),
            Err(why) => lost(why),
        }
    });
}

mod breaker;

#[cfg(test)]
mod tests;

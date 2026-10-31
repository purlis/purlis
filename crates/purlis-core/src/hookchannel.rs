// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What hears each held project's hooks in a session host** (#2000, #2447): the host's
//! [`purlis_host::Hear`], one for both binaries' hosts, the window's and `purlis serve`.
//!
//! **The project's hook channel, as the window bound it before.** At the same place
//! ([`socket_for`]), with the same listener and every one of its checks: the token, the process
//! that sent it, each framed ask again, the spool's keys beside it. Only who listens changes,
//! and the host is each chat's ancestor, so a hook admits it. Each line it admits is handed on to
//! the links that hold the project ([`crate::hookwire::relay`]): the window's own, or on a runner
//! the link its desktop relays the window's over.
//!
//! **And recorded here** (#2003): what the window's hearing would have written of a line comes
//! back with its reply and is written in the event log the host keeps ([`EventLog`]), on the
//! thread that heard the line; what a hook spooled is drained into the same log as the project is
//! held, soon after a chat is heard from, and as a chat is closed. So a line and its spooled copy
//! are told apart in one process, and recorded once (#952).
//!
//! **It keeps the project's kept board** (#2194, [`KeptBoard`]), which outlives the window.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use purlis_session_protocol::session;

use crate::eventlog::{Entry, Recorder};
use crate::hookwire::relay::{Keeping, Relaying};

mod record;
pub use record::{how_its_run_ended, its_children_end};

/// One line in the host's log: the window keeps what its own host says, and `purlis serve` says
/// it on its standard error.
fn host_says(line: &str) {
    let _ = writeln!(std::io::stderr(), "purlisd: {line}");
}

/// Where a host listens for a project's hooks, and where containment of that path begins.
///
/// The two travel together because [`crate::hookwire::Listener::bind`] needs both: a link
/// anywhere between them is refused, and the root is named rather than worked out — a walk from
/// the root of the filesystem would refuse every path on macOS, where `/tmp` and `/var` are
/// themselves links.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Where {
    /// A directory the caller already trusts. Nothing above it is checked.
    pub within: PathBuf,
    pub socket: PathBuf,
}

/// Where a project's chats' hooks report.
///
/// Beside the record M1.7 already writes (`.charter/app/`) when there is a plane, so everything
/// purlis keeps for a plane is in one place and an operator looking for it finds it.
/// `Listener::bind` makes that directory 0700.
///
/// **There is always somewhere.** The channel belongs to purlis, not to the plane: a chat
/// started outside a plane is still a chat, and it would be a strange rule that a harness can
/// report what it is doing only when there happens to be a `charter.toml` above it.
///
/// The fallback is keyed by the plane (or by nothing) so two hosts do not land on one socket,
/// and by the user so two accounts on one machine do not either. A path is handed to each
/// chat in its environment, so nothing ever has to guess it.
pub fn socket_for(plane: Option<&Path>) -> Where {
    if let Some(plane) = plane {
        let beside_the_record = crate::names::state(plane).join("app").join("hooks.sock");
        // macOS allows 104 bytes for a unix socket path including the terminator
        // (`sys/un.h`), Linux 108; the smaller is the one to hold to, since a plane is
        // portable. A plane nested deeper than that is not a failure, just not somewhere the
        // socket can live.
        if beside_the_record.as_os_str().len() <= LONGEST_SOCKET_PATH {
            // The plane is the root: it is the operator's own tree, and `.charter/app` below
            // it is charter's own directory (charter-app#28 rules the same for the record
            // that already lives there).
            return Where {
                within: plane.to_path_buf(),
                socket: beside_the_record,
            };
        }
    }
    let within = private_dir();
    let socket = within
        .join(format!(
            "charter-{}-{:016x}",
            whoami(),
            crate::renamelocal::busy::fallback_key(plane)
        ))
        .join("hooks.sock");
    Where { within, socket }
}

/// Where a socket goes when it cannot go beside the plane.
///
/// `$XDG_RUNTIME_DIR` first: on Linux it is the standard per-user 0700 directory for exactly
/// this, and the temp directory there is `/tmp`, which everyone can write to. macOS has no
/// such variable and its `TMPDIR` is already a per-user 0700 directory.
///
/// Either way `Listener::bind` creates the socket's own directory with the mode set as it is
/// made and refuses one it does not own, so this chooses a good neighbourhood rather than
/// being the thing that keeps anyone out.
fn private_dir() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .filter(|dir| dir.is_absolute() && dir.is_dir())
        .unwrap_or_else(std::env::temp_dir)
}

/// The longest a unix socket path may be, on the stricter of the two platforms charter builds
/// for. One byte is left for the terminator.
const LONGEST_SOCKET_PATH: usize = 103;

/// Something short and stable that differs between users on one machine.
///
/// `TMPDIR` is already per-user on macOS and is not on Linux, so this is what keeps two
/// accounts apart there. It is not a secret and is not relied on to be one — the directory's
/// 0700 is what keeps others out.
fn whoami() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .ok()
        .filter(|user| {
            user.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        })
        .unwrap_or_else(|| "charter".to_owned())
}

/// **The event log a session host keeps** (#2003), opened the first time it is needed, which is
/// as the host first holds a project: only a window whose chats run in the host holds one, so a
/// host whose window keeps the log itself never opens it, and the device has one writer either
/// way. One that cannot be opened is said once in the host's log, and every write is refused in
/// words from then on. A clone is the same log.
#[derive(Clone)]
pub struct EventLog(Arc<Opening>);

type Open = dyn Fn() -> std::io::Result<Recorder> + Send + Sync;

struct Opening {
    open: Box<Open>,
    opened: std::sync::OnceLock<Option<Arc<Mutex<Recorder>>>>,
}

/// What a write is refused with in a host whose event log could not be opened.
const NO_EVENT_LOG: &str = "the session host has no event log on this machine, so nothing was \
     recorded";

impl EventLog {
    /// The event log of this machine ([`Recorder::open`]).
    pub fn on_this_machine() -> EventLog {
        EventLog::opening(Recorder::open)
    }

    /// The event log `open` opens, the first time it is needed.
    pub fn opening(open: impl Fn() -> std::io::Result<Recorder> + Send + Sync + 'static) -> Self {
        EventLog(Arc::new(Opening {
            open: Box::new(open),
            opened: std::sync::OnceLock::new(),
        }))
    }

    /// The recorder, opened now if it was not yet: `None` where it could not be. Its one writer
    /// drains a hook spool into it, so a line and its spooled copy are told apart in one
    /// process (#952).
    pub fn recorder(&self) -> Option<Arc<Mutex<Recorder>>> {
        self.0
            .opened
            .get_or_init(|| match (self.0.open)() {
                Ok(recorder) => Some(Arc::new(Mutex::new(recorder))),
                Err(why) => {
                    host_says(&format!(
                        "no event log ({why}); what chats do in this host is not recorded"
                    ));
                    None
                }
            })
            .clone()
    }

    /// **Writes `entry` durably**: what the window decided, or what its hearing made of a line.
    /// `Ok` once it is kept, which is when a hook is told its line is taken (FD-30). The `fsync`
    /// is outside the log's lock, so lines recorded at once share one.
    pub fn write(&self, entry: Entry) -> std::io::Result<()> {
        self.write_as(entry, |_, entry| entry)
    }

    /// [`Self::write`], `entry` made what `checked` makes of it against the log's writer, under
    /// the same hold as the write.
    fn write_as(
        &self,
        entry: Entry,
        checked: impl FnOnce(&Recorder, Entry) -> Entry,
    ) -> std::io::Result<()> {
        let Some(log) = self.recorder() else {
            return Err(std::io::Error::other(NO_EVENT_LOG));
        };
        let (written, durable) = {
            let mut log = log.lock().unwrap_or_else(PoisonError::into_inner);
            let entry = checked(&log, entry);
            (log.apply(entry), log.durable())
        };
        match written?.last() {
            Some(last) => durable.through(last.seq),
            None => Ok(()),
        }
    }
}

/// What a drain of the hook spool found, each item into the host's log and made durable before
/// the next: the drain removes a line's file only once this answers `Ok` for it. The log is
/// held for one item at a time, as a live line holds it, so the hooks of every other chat go
/// on being recorded while one chat's spool is drained.
fn into_the_log<'a>(
    log: &'a Mutex<Recorder>,
    root: &'a Path,
) -> impl FnMut(crate::hookwire::spool::Drained) -> std::io::Result<()> + 'a {
    move |item| {
        let (event, durable) = {
            let mut log = log.lock().unwrap_or_else(PoisonError::into_inner);
            (log.spooled(root, item), log.durable())
        };
        durable.through(event?.seq)
    }
}

/// **Drains the hook spool beside `socket`, of the project at `root`, into `log`, as the project
/// is held**: by the window, or by the session host as it first holds the project (#2003),
/// whichever writes the event log. With no log, the keys of the chats the reopen record does not
/// bring back are dropped, unless their spools still hold a line.
pub fn drain_at_open(socket: &Path, root: &Path, log: Option<&Mutex<Recorder>>) {
    let dir = crate::hookwire::spool::dir_for(socket);
    let record = crate::reopen::path(root)
        .is_file()
        .then(|| crate::reopen::read_or_refusal(root).ok())
        .flatten();
    let chats = record.map(|record| record.chats).unwrap_or_default();
    // The numbers of the chats this open brings back.
    let back: Vec<u32> = chats.iter().filter_map(|chat| chat.number).collect();
    let Some(log) = log else {
        if let Err(why) = crate::hookwire::spool::forget_ended_at_open(&dir, &back) {
            tracing::warn!(
                "purlis: the ended chats' hook spool keys were not dropped ({why}); they are \
                 dropped at the next start"
            );
        }
        return;
    };
    let mut log = log.lock().unwrap_or_else(PoisonError::into_inner);
    for chat in chats {
        if let (Some(number), Some(id), Some(run)) =
            (chat.number, chat.identity.id, chat.identity.run)
        {
            log.knows(
                root,
                number,
                crate::eventlog::RunOf {
                    chat: &id,
                    run: &run,
                },
            );
        }
    }
    let durable = log.durable();
    let drained = crate::hookwire::spool::drain_at_open(&dir, &back, &mut |item| {
        let event = log.spooled(root, item)?;
        durable.through(event.seq)
    });
    if let Err(why) = drained {
        tracing::warn!(
            "purlis: the hook spool was not drained ({why}); it is drained at the next start"
        );
    }
}

/// **Chat `chat` of the project at `root` was closed**: what its hooks spooled is drained into
/// `log` and its spool keys dropped, by whichever process writes the event log (#2003). With no
/// log the keys are dropped all the same.
pub fn spool_ended(socket: &Path, root: &Path, log: Option<&Mutex<Recorder>>, chat: u32) {
    use crate::hookwire::spool;
    let dir = spool::dir_for(socket);
    let ended = match log {
        Some(log) => spool::end_chat(&dir, chat, &mut into_the_log(log, root)),
        None => spool::forget_chat(&dir, chat),
    };
    if let Err(why) = ended {
        tracing::warn!(
            "purlis: chat {chat}'s hook spool was not drained as it ended ({why}); its key is \
             kept until the next open of the project"
        );
    }
}

/// How long after one of a chat's lines is heard a host looks at its spool (#952). A hook spools
/// a line once the host has not taken it in a quarter of a second, and gives the disk a second
/// to keep it, so a line the host heard late is in the spool by then.
pub const A_SPOOL_IS_LOOKED_AT_AFTER: Duration = Duration::from_secs(2);

/// Where the host's hearers tell which chat they heard from, so its spool is drained while the
/// host runs (#952): one thread per project, [`watching_spools`]'s.
#[derive(Clone)]
pub struct SpoolWatch(std::sync::mpsc::Sender<u32>);

impl SpoolWatch {
    /// Chat `chat` was heard from on the hook channel. Never waits: the look is the thread's.
    pub fn heard_from(&self, chat: u32) {
        // A thread that is gone (it could not be started) drains nothing, and the spool waits
        // for the chat's end or the project's next open, as it did before #952.
        let _ = self.0.send(chat);
    }
}

/// **A chat's spool is drained while the host runs** (#952), set off from the hook channel: a
/// line of the chat heard there has the chat's spool looked at `after` later, and drained into
/// the event log if it holds a line. So a line a hook spooled because this host was slow to
/// take it is recorded within seconds, once the chat is heard from again, or at its end or the
/// project's next open, whichever comes first; a line the host took late and the hook spooled
/// as well is recorded once (`hookwire::Deliveries`).
///
/// **One look per chat per wait**: lines heard while a look is pending add none, so a chat that
/// is busy has its spool looked at once every few seconds at most, and a look at a spool that
/// holds no line is one listing of its folder ([`crate::hookwire::spool::holds_lines`]). The
/// drain keeps the chat's keys ([`crate::hookwire::spool::drain_running`]). With no event log
/// nothing is drained: there is nowhere to record a line, and the spool waits for a host that
/// has one. `drain` drains one chat's spool, where there is a log to drain it into.
///
/// The thread ends with the hearers, when the project stops listening; a look still pending
/// then is let go, and the chat's spool is drained at its end or the next open.
pub fn watching_spools(after: Duration, drain: impl Fn(u32) + Send + 'static) -> SpoolWatch {
    use std::sync::mpsc::RecvTimeoutError;
    use std::time::Instant;
    let (heard, hears) = std::sync::mpsc::channel::<u32>();
    let watching = std::thread::Builder::new()
        .name("purlis-spool-watch".into())
        .spawn(move || {
            let mut due: std::collections::BTreeMap<u32, Instant> = Default::default();
            loop {
                let next = due.values().min().copied();
                let got = match next {
                    None => hears.recv().map_err(|_| RecvTimeoutError::Disconnected),
                    Some(at) => hears.recv_timeout(at.saturating_duration_since(Instant::now())),
                };
                match got {
                    Ok(chat) => {
                        due.entry(chat).or_insert_with(|| Instant::now() + after);
                    }
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => return,
                }
                let now = Instant::now();
                let ready: Vec<u32> = due
                    .iter()
                    .filter(|(_, at)| **at <= now)
                    .map(|(chat, _)| *chat)
                    .collect();
                for chat in ready {
                    due.remove(&chat);
                    drain(chat);
                }
            }
        });
    if let Err(why) = watching {
        tracing::warn!(
            "purlis: the hook spool cannot be drained while this project runs ({why}); it is \
             drained as each chat ends and at the next open"
        );
    }
    SpoolWatch(heard)
}

/// Chat `chat`'s spool in `dir`, of the project at `root`, drained into `log` while the host
/// runs, where it holds a line ([`watching_spools`]).
pub fn drain_while_running(dir: &Path, root: &Path, log: &Mutex<Recorder>, chat: u32) {
    use crate::hookwire::spool;
    if !spool::holds_lines(dir, chat) {
        return;
    }
    if let Err(why) = spool::drain_running(dir, chat, &mut into_the_log(log, root)) {
        tracing::warn!(
            "purlis: chat {chat}'s hook spool was not drained while it runs ({why}); it is \
             drained at the next look, at its end or at the next open"
        );
    }
}

/// **A held project's kept board, in the session host** (#2194, CONTEXT.md,
/// [`crate::state::kept`]).
///
/// The window makes each move on its own copy and tells this one ([`KeptBoard::moved`]), so
/// the two read the same; a window that holds the project takes this one whole
/// ([`KeptBoard::whole`]). A line no window took moves it here, as the window's own hearing
/// would with nothing said of what a chat waits on ([`KeptBoard::handing_on`]), and so does a
/// program that ends with no window there ([`KeptBoard::ended`]): nothing a chat says while the
/// window is away is lost.
#[derive(Clone, Default)]
pub struct KeptBoard(Arc<Mutex<crate::state::Board>>);

/// The most a board answered to a window may be, its frame's envelope aside.
const A_BOARD_IS_AT_MOST: usize = purlis_session_protocol::link::MOST_CONTROL_FRAME_BYTES - 4096;

impl KeptBoard {
    fn board(&self) -> std::sync::MutexGuard<'_, crate::state::Board> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// What hands each line on through `hand_on`, the board moved by one no window took.
    pub fn handing_on(&self, hand_on: purlis_host::HandOn) -> crate::hookwire::relay::HandOn {
        let kept = self.clone();
        Box::new(move |id, heard| {
            let told = serde_json::to_value(heard).is_ok_and(|line| hand_on(id, line));
            if !told {
                kept.board().hear_at(heard, chrono::Utc::now().timestamp());
            }
            told
        })
    }

    /// The window made `moved` of chat `number` on its own board: whether this host read it.
    /// One it cannot read moves nothing, and is said in the log.
    pub fn moved(&self, number: u32, moved: serde_json::Value) -> bool {
        match serde_json::from_value::<crate::state::Move>(moved) {
            Ok(moved) => {
                self.board().make(number, moved);
                true
            }
            Err(why) => {
                host_says(&format!(
                    "a move of chat {number} the window made could not be read, so the board \
                     kept here did not make it: {why}"
                ));
                false
            }
        }
    }

    /// The board, whole, for a window that holds the project. One larger than the link carries
    /// is refused in a sentence, which the window shows the person (#2006), and said in the log:
    /// the window then starts from its own.
    pub fn whole(&self) -> Result<serde_json::Value, String> {
        let kept = serde_json::to_value(self.board().kept()).unwrap_or(serde_json::Value::Null);
        let size = serde_json::to_vec(&kept).map_or(usize::MAX, |said| said.len());
        if size > A_BOARD_IS_AT_MOST {
            host_says(&format!(
                "the board kept here is {size} bytes, more than the link carries \
                 ({A_BOARD_IS_AT_MOST}), so the window starts from its own"
            ));
            return Err(format!(
                "the board the session host kept is {size} bytes, more than one answer carries \
                 ({A_BOARD_IS_AT_MOST})"
            ));
        }
        Ok(kept)
    }

    /// A line handed on whose window went before it replied (#2006): heard here, as one no
    /// window took is.
    fn unheard(&self, heard: &crate::hookwire::relay::Heard) {
        self.board().hear_at(heard, chrono::Utc::now().timestamp());
    }

    /// Chat `number`'s program ended, `exit`, with no window there to say so.
    pub fn ended(&self, number: u32, exit: &session::Exit) {
        let code = match exit {
            session::Exit::Code(code) => i32::try_from(*code).ok(),
            // A signal leaves no code behind, and it is not a clean end.
            session::Exit::Signal(_) => None,
        };
        self.board().exited(number, code);
    }
}

/// **What hears each held project's hooks in a session host**: the host's
/// [`purlis_host::Hear`], for the window's binary and for `purlis serve` alike (module docs).
pub struct HookChannels {
    log: EventLog,
    spools_after: Duration,
}

impl HookChannels {
    /// Each held project's hook channel, recording in `log`.
    pub fn keeping(log: EventLog) -> HookChannels {
        HookChannels {
            log,
            spools_after: A_SPOOL_IS_LOOKED_AT_AFTER,
        }
    }

    /// Each chat's spool looked at `after` one of its lines is heard, not
    /// [`A_SPOOL_IS_LOOKED_AT_AFTER`]: for a test that waits on the look, not on the clock.
    pub fn looking_at_spools_after(self, after: Duration) -> HookChannels {
        HookChannels {
            spools_after: after,
            ..self
        }
    }
}

impl purlis_host::Hear for HookChannels {
    fn open(
        &self,
        _project: &str,
        path: &Path,
        hand_on: purlis_host::HandOn,
    ) -> Result<Option<Arc<dyn purlis_host::Channel>>, String> {
        let at = socket_for(Some(path));
        let listener = crate::hookwire::Listener::bind(&at.within, &at.socket)
            .map_err(|why| format!("its hook socket at {} ({why})", at.socket.display()))?;
        let socket = listener.path().to_path_buf();
        // What its chats' hooks spooled while nobody took their lines, before a chat is started
        // here and issued a token (FD-30), as the window drains it as it binds the socket.
        let log = self.log.clone();
        drain_at_open(&socket, path, log.recorder().as_deref());
        let tokens = listener.tokens();
        let board = KeptBoard::default();
        // A chat's spool drained while it runs (#952), set off by its lines.
        let spooled = {
            let (dir, root, log) = (
                crate::hookwire::spool::dir_for(&socket),
                path.to_path_buf(),
                log.clone(),
            );
            watching_spools(self.spools_after, move |chat| {
                if let Some(recorder) = log.recorder() {
                    drain_while_running(&dir, &root, &recorder, chat);
                }
            })
        };
        let relaying = Relaying::keeping(
            board.handing_on(hand_on),
            Keeping {
                record: hook_lines_recorded(log.clone(), path),
                heard_from: Box::new(move |chat| spooled.heard_from(chat)),
                unheard: {
                    let board = board.clone();
                    Box::new(move |heard| board.unheard(heard))
                },
            },
        );
        // A `secret exec` of a chat composed here runs here on the values its desktop sends
        // (#2047).
        relaying.of_project_at(path);
        // A Claude Code chat's server dials in on macOS alone (#3316): each slot is handed on to
        // the window beside this socket, which holds the doors.
        if cfg!(target_os = "macos") {
            relaying.handing_slots_to(&crate::hookwire::serve::beside(&socket));
        }
        let reading = listener.hear(relaying.hearing(Arc::clone(&tokens)));
        Ok(Some(Arc::new(HookChannel {
            socket,
            root: path.to_path_buf(),
            log,
            tokens,
            relaying,
            board,
            reading: Mutex::new(Some(reading)),
        })))
    }
}

/// What records, in `log`, what the window sends back of each hook line this channel heard: under
/// the project this host holds the channel for, at `root` (#3304). The project's path the window
/// names is not used: a pool's runner holds the project at `<data>/projects/<id>`, not where the
/// window holds it, and the relay records only the record of the line this channel heard
/// ([`crate::hookwire::relay`]), so the line is this project's whatever path the window names.
/// Anything but a hook line is refused.
fn hook_lines_recorded(
    log: EventLog,
    root: &Path,
) -> Box<dyn Fn(Entry) -> std::io::Result<()> + Send + Sync + 'static> {
    let root = root.to_path_buf();
    Box::new(move |entry| {
        let plane = root.clone();
        let entry = match entry {
            Entry::Report {
                report,
                followed,
                begun,
                ..
            } => Entry::Report {
                plane,
                report,
                followed,
                begun,
            },
            Entry::Tool { call, .. } => Entry::Tool { plane, call },
            Entry::Refused { chat, .. } => Entry::Refused { plane, chat },
            _ => {
                return Err(std::io::Error::other(
                    "what the window sent back to record is not a hook line",
                ));
            }
        };
        // What the window's board made of a state hook's line is checked against what the log
        // holds of the chat's run, and decided here where it is not taken (#3394).
        log.write_as(entry, |recorder, entry| match entry {
            Entry::Report {
                plane,
                report,
                followed,
                begun,
            } => Entry::Report {
                followed: recorder.followed_as_heard(&plane, &report, followed),
                plane,
                report,
                begun,
            },
            other => other,
        })
    })
}

/// One held project's hook channel in the host.
struct HookChannel {
    socket: PathBuf,
    /// Where its project is on this machine: what its chats are recorded under.
    root: PathBuf,
    log: EventLog,
    tokens: Arc<crate::hookwire::ChatTokens>,
    relaying: Arc<Relaying>,
    /// The project's board, kept here (#2194).
    board: KeptBoard,
    reading: Mutex<Option<crate::hookwire::Reading>>,
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
        match crate::hookwire::Program::of(pid) {
            Some(program) => self.tokens.bind(number, program),
            None => host_says(&format!(
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
        crate::hostedchats::forget(&self.root, number);
        // What it spooled is recorded and its keys dropped, off the host's runtime: a drain
        // makes each line durable (V99i, #2003).
        let (socket, root, log) = (self.socket.clone(), self.root.clone(), self.log.clone());
        let drained = std::thread::Builder::new()
            .name("purlisd-spool-ended".into())
            .spawn(move || {
                spool_ended(&socket, &root, log.recorder().as_deref(), number);
            });
        if let Err(why) = drained {
            host_says(&format!(
                "chat {number}'s hook spool was not drained as it was closed ({why}); it is \
                 drained at the next hold of its project"
            ));
        }
    }

    fn replied(&self, id: u64, reply: serde_json::Value) {
        match serde_json::from_value(reply) {
            Ok(replied) => self.relaying.replied(id, replied),
            Err(why) => host_says(&format!("a reply the window sent could not be read: {why}")),
        }
    }

    fn moved(&self, number: u32, moved: serde_json::Value) {
        self.board.moved(number, moved);
    }

    fn board(&self) -> Result<serde_json::Value, String> {
        self.board.whole()
    }

    /// The window went: what waits for its reply is let go of, and the board kept here hears
    /// each report among it (#2006).
    fn no_window(&self) {
        self.relaying.let_go();
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

#[cfg(test)]
mod tests;

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What the app asks of whatever runs its sessions and hears their hooks: the session host.
//!
//! Today the host is in the app's own process: [`crate::sessions::Sessions`] runs the
//! sessions, and [`crate::hooks::Hooks`] holds the board their hooks report to. ADR 0068
//! (accepted 2026-09-30) moves both into `charterd`, behind a socket. These two traits are that
//! seam, drawn first in place with nothing moved and nothing changed. The app reaches a session
//! only through [`SessionHost`], and a chat's hook state only through [`ChatBoard`].
//!
//! - [`SessionHost`] starts a session, writes to it, resizes it, opens and closes views of it,
//!   ends it, and tells whoever asked when its program exits. It also deals the numbers chats
//!   are keyed on (charter-app#90, and ADR 0066 for the identity that follows).
//! - [`ChatBoard`] is what the board says about each chat, and the three moves the window
//!   makes on it: a chat closed, a request ignored, a report handed back.
//!
//! They are two traits because they are two things with different owners today. The window
//! reaches the board through the plane, and the chats layer does not reach it at all.
//!
//! **What else sits on the host's side.** Auto-save and `planewatch` are started when a plane
//! is held and stopped when it is let go of. Auto-save hears a chat end through
//! [`SessionHost::when_one_ends`], and nothing in the app calls either of them outside that
//! lifecycle, so neither needs a trait here. What a session is told to report to (the hook
//! socket and the chats' tokens, [`crate::sessions::Reporting`]) goes to the host when the
//! host is made.

use std::sync::Arc;

use purlis_core::engine::Size;
use purlis_core::session::Exit;
use purlis_core::state::State;

use crate::hooks::Moved;

/// What to run in a new session. No program is the operator's shell.
#[derive(Debug, Clone)]
pub struct Opening {
    pub program: Option<String>,
    pub args: Vec<String>,
    pub cwd: Option<String>,
    pub size: Size,
    /// Set in the program's environment, on top of what the app itself was started with.
    pub env: Vec<(String, String)>,
    /// Extra variable names kept out of this chat, beyond the `OP_*` prefix: every identity
    /// source and target a vault of this plane declares (`registry::identity_vars`), so a vault
    /// bound to a non-`OP_` variable (`--token-env PROD_1P_TOKEN`) leaks it to no chat either
    /// (#271 review, U6). Removed from both the set env and the inherited one.
    pub env_strip: Vec<String>,
    /// The harness the chat runs, whose own declared variables it is also started with
    /// ([`purlis_core::harness::Harness::env_passed`]). None is a shell.
    pub harness: Option<purlis_core::harness::Harness>,
    /// The `env` namespace of the harness a project declares, for a chat on one
    /// ([`purlis_core::harness_declaration::Declaration::env`], #967): each variable it reaches
    /// in the app's own environment is passed, judged one at a time, and none named like a
    /// credential ([`purlis_core::chatenv::namespace`], #1013). Empty for every other chat.
    pub declared_env: Vec<String>,
    /// The operator's own additions to what a chat is started with, from the chat's plane
    /// (`[chat_env] pass`, [`purlis_core::chatenv::read`]).
    pub env_pass: Vec<String>,
    /// Whether this is a shell the operator opened from the window, which the kill switch lets
    /// through: looking at what the agents did is a human act (OV-1, ADR 0071). Every other
    /// start — every chat, every relaunch, every shell a record puts back — is refused while
    /// agents are stopped.
    pub operator_shell: bool,
    /// charter's git hooks, for a chat that runs a harness (SQ-16, ADR 0074): the chat's git is
    /// armed with them after every other variable is settled, so a `GIT_CONFIG_COUNT` the
    /// operator passes or a profile sets keeps its pairs. None for a shell.
    pub git_hooks: Option<purlis_core::githooks::GitHooks>,
    /// What its profile's sign-in hands its program as it starts (#2036): each variable and its
    /// value, read from the vault for this one start and set last on the program's environment.
    /// The commands, hooks and tools the program starts inherit them. purlis writes them to no
    /// file or log, and its `Debug` names the variables alone. Nothing for every other chat, a
    /// shell tab's among them, and never sent over the link to the session host.
    pub handed: purlis_core::profilesignin::Handed,
    /// The confinement the chat runs in, where it is held outside this process (#1999): the
    /// session host's, by the id [`SessionHost::confine`] gave it. None for a chat without
    /// one, or whose proxy runs in the window.
    pub confinement: Option<String>,
    /// The run this start begins (ADR 0066), which the window minted and writes its
    /// `run.started` under. The session host is handed it with the start, so its list and the
    /// program's end name it (#2172).
    pub run: Option<String>,
    /// Whether it ends with this window (#3274): a grade's chat (SK-16), which only the window
    /// that started it watches. A session host ends it as soon as this window's link to it ends
    /// without a quit, rather than keeping it for the window to come back to (1.42).
    pub ends_with_its_window: bool,
}

/// A shell with nothing set, for fixtures (#2453): no program, the smallest terminal, nothing
/// added to or kept from its environment, not the operator's own shell, handed nothing and held
/// in no confinement. A fixture names what it sets and takes the rest with
/// `..Opening::default()`, so a new field touches only the code that sets it. A real start sets
/// every field (the start in `Chats`).
#[cfg(test)]
impl Default for Opening {
    fn default() -> Self {
        Opening {
            program: None,
            args: Vec::new(),
            cwd: None,
            size: Size::MIN,
            env: Vec::new(),
            env_strip: Vec::new(),
            harness: None,
            declared_env: Vec::new(),
            env_pass: Vec::new(),
            operator_shell: false,
            git_hooks: None,
            handed: purlis_core::profilesignin::Handed::default(),
            confinement: None,
            run: None,
            ends_with_its_window: false,
        }
    }
}

/// **A chat by what it is named, for the session host to compose** (#2171): its profile and
/// persona, its workspace, the conversation it carries on, its first message, and whether the
/// person opted it out of the sandbox. And who it is and the run this start begins (ADR 0066),
/// where the window knows them: a start that names none is given new ones. And the services of
/// the machine it runs on the person allowed it alone, which a host that composes it leaves out
/// where its own sandbox does not give them, and asks about (#2590).
#[derive(Debug, Clone)]
pub struct Named {
    pub chat: Option<String>,
    pub run: Option<String>,
    pub profile: Option<String>,
    pub persona: Option<String>,
    pub workspace: Option<String>,
    pub resume: Option<String>,
    pub first_message: Option<String>,
    pub without_sandbox: bool,
    pub alone: Vec<u16>,
    pub size: Size,
}

/// **A chat its host composed by its names, as the host answered its start** (#2692): its
/// number, the confinement the host named to this window where it named one (#2407), and how
/// the host applied the sandbox to its run, by the host's own record (1.29). `ran` is `None`
/// from a host that keeps no such record, which the window takes as sandboxed.
#[derive(Debug)]
pub struct Composed {
    pub number: u32,
    pub confinement: Option<purlis_core::sandbox::Confinement>,
    pub ran: Option<purlis_session_protocol::session::RunSandbox>,
}

/// **A chat that moves between this machine and a runner** (#2048): where it works, the profile
/// it runs on, and the conversation it is in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Moving {
    pub workspace: String,
    pub profile: String,
    pub conversation: String,
}

/// What a host that moves no chat says.
pub const MOVES_NO_CHAT: &str = "chats do not move to or from where this one runs";

/// Why a chat by what it is named alone is not started in the window's own process.
pub const NOT_COMPOSED_HERE: &str =
    "a chat in the window is started from what the window composed, never by its names alone";

/// Where a view's text goes. It is called on the view's own thread, one batch at a time.
pub type Sink = Box<dyn FnMut(String) + Send>;

/// Told how each session ended, as it ends, with the session's id. An `Arc` so each session
/// can hold it for as long as its program runs.
pub type Ends = Arc<dyn Fn(u32, Exit) + Send + Sync>;

/// Told which sessions are reconnecting while the link to the session host they run on is
/// down, and which are reconnected (#2496).
pub type Reconnecting = Arc<dyn Fn(Vec<u32>, crate::reconnecting::Reconnection) + Send + Sync>;

/// A view the UI has open, and the size the screen it opened on was drawn for.
#[derive(Debug, Clone, Copy)]
pub struct Watching {
    pub view: u32,
    pub size: Size,
}

/// Whether a session's program is ready to be typed at, as the terminal and the clock say it:
/// never what the program wrote. It is what a curation prompt waits on (ADR 0061).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Readiness {
    /// Whether the terminal is still in the kernel's line editing rather than handing keys to
    /// the program (`purlis_core::session::Session::edits_lines`), or none where the platform
    /// cannot say.
    pub edits_lines: Option<bool>,
    /// How long the program has written nothing (`purlis_core::session::Session::quiet_for`):
    /// when bytes last arrived, never what they were.
    pub quiet_for: std::time::Duration,
}

/// Runs the app's sessions, and is the only way anything above it reaches one.
///
/// Every call that names a session that is not running is refused with a sentence, never a
/// panic.
pub trait SessionHost: Send + Sync {
    /// Starts a session, and answers with the id it is called by from now on.
    ///
    /// `announce` is called with that id BEFORE the program starts. A harness fires
    /// `SessionStart` at its own exec, and anything that learned the chat's number afterwards
    /// would miss it — for a chat that is then idle, waiting for a first prompt, no second
    /// event ever comes and it reads `unknown` for the rest of the run. A review found that
    /// on the relaunch path, where a whole record's worth of chats start at once.
    ///
    /// `wanted` is the number this chat ALREADY answers to, where it has one. A relaunch
    /// hands the number the record kept for each chat, so the chat comes back on its own
    /// `.charter/sessions/<sid>.workspace` and `<sid>.lock` rather than on whichever the
    /// order of the record would have dealt it (charter-app#90). `None` is a chat that has
    /// no number yet, which is every chat the operator starts.
    ///
    /// A number a session is **already running under** is refused and a fresh one taken
    /// instead: a record that names one twice — hand-edited, or two records concatenated —
    /// would otherwise give two live chats one key, which is this same defect pointed the
    /// other way. Whichever number is chosen, the counter is raised past it, so nothing
    /// dealt later can land on it either.
    fn open(
        &self,
        wanted: Option<u32>,
        opening: &Opening,
        announce: &dyn Fn(u32),
    ) -> Result<u32, String>;

    /// **Starts a chat the session host composes itself** (#2171): by what `named` names of it,
    /// and nothing the window composed. Only a host composes one: the window's own process
    /// refuses it in a sentence.
    ///
    /// **The confinement the host composes for it is heard by this window** (#2407): where
    /// `hearing` asks live, the host names that confinement to the window's link, `hearing` is
    /// told what its proxy does, and its asks are answered from here, as a chat's the window
    /// planned. The answer holds it beside the chat's id: dropping it lets the host end it, as
    /// for one the window planned. None where the host composed none, or named it to no link.
    /// And how the host says it applied the sandbox to the run it started ([`Composed::ran`]).
    fn open_named(
        &self,
        _wanted: Option<u32>,
        _named: &Named,
        _hearing: purlis_core::sandbox::plan::Hearing,
        _announce: &dyn Fn(u32),
    ) -> Result<Composed, String> {
        Err(NOT_COMPOSED_HERE.to_owned())
    }

    /// **Whether this host composes every harness chat the window starts in it** (#2446): a
    /// runner's, which takes no harness launch from the window (ADR 0068, amended for #2217).
    /// The window starts such a chat by its names ([`Self::open_named`]), and a shell by its own
    /// launch ([`Self::open`]), as everywhere. Never, where chats run on this machine.
    fn composes_its_harness_chats(&self) -> bool {
        false
    }

    /// **Whether a harness chat started in `workspace` is composed by the host it runs on**
    /// (#2046): where the window routes each workspace's chats to the host that runs them, a
    /// workspace whose chats run on a pool's runner is started by its names there. Every other
    /// host answers as [`Self::composes_its_harness_chats`] does.
    fn composes_its_harness_chats_in(&self, workspace: Option<&str>) -> bool {
        let _ = workspace;
        self.composes_its_harness_chats()
    }

    /// **The pool a chat started in `workspace` runs on** (#2049), where the window routes that
    /// workspace's chats to a pool's runner: what the profile it starts on is kept on, and
    /// signed in on, before it starts. None, where it starts on this machine.
    fn pool_for(&self, workspace: Option<&str>) -> Option<String> {
        let _ = workspace;
        None
    }

    /// **The pool chat `id` runs on**, where it runs on a pool's runner (#2046): what the window
    /// holds its writes to the project for the person by. None, where it runs on this machine.
    fn pool_of(&self, id: u32) -> Option<String> {
        let _ = id;
        None
    }

    /// Each pool's runner log the project's chats read, by the pool (#3393): none where they
    /// all run here.
    fn runner_logs(&self) -> Vec<(String, crate::runnerlog::RunnerLogNow)> {
        Vec::new()
    }

    /// **Holds the project in this host now** (#2046), as a window that is about to start a
    /// chat on a runner beside it does: this machine's host keeps the project's board for
    /// every chat, those on runners included. Nothing, where the window's own process runs the
    /// chats.
    fn hold_here(&self) {}

    /// **Fetches the branches of `workspace`'s clones back from the runner of `pool`** (#2046),
    /// where this host runs the project's chats there: each clone's `refs/remotes/<pool>/*`.
    /// Answers what could not be fetched, in sentences. Nothing, where chats run elsewhere.
    fn fetch_back(&self, workspace: &str, pool: &str) -> Vec<String> {
        let _ = (workspace, pool);
        Vec::new()
    }

    /// **Readies the runner of `pool` for a chat in `workspace` to move there** (#2048): the
    /// project held there, and each clone's branch sent ahead, while the chat keeps working
    /// here. Refused in a sentence where chats cannot move to it.
    fn move_ahead(&self, pool: &str, workspace: &str) -> Result<(), String> {
        let _ = (pool, workspace);
        Err(MOVES_NO_CHAT.to_owned())
    }

    /// **Moves `chat`'s work and conversation to the runner of `pool`** (#2048), at its pause:
    /// each clone's work tree taken over there, and its conversation kept in its profile's
    /// harness folder there. Refused, in a sentence, where any of it does not move.
    fn move_in(&self, pool: &str, chat: &Moving) -> Result<(), String> {
        let _ = (pool, chat);
        Err(MOVES_NO_CHAT.to_owned())
    }

    /// **Moves `chat`'s work and conversation back from the runner of `pool`** (#2048), at its
    /// pause: each clone's work tree here taken over onto the runner's, and its conversation kept
    /// in its profile's harness folder here. Answers what came back that the person is to look
    /// at before a tool here runs it, by its path in the project.
    fn move_out(&self, pool: &str, chat: &Moving) -> Result<Vec<String>, String> {
        let _ = (pool, chat);
        Err(MOVES_NO_CHAT.to_owned())
    }

    /// Sends what a pane typed to the program: bytes, written to its pty as they are. Text is
    /// its UTF-8; a mouse report in the default encoding is bytes that are not text at all
    /// (charter#493).
    fn input(&self, id: u32, bytes: &[u8]) -> Result<(), String>;

    /// Gives a session's terminal, and the screen its views are drawn from, a new size. The
    /// program hears of it from its terminal, as it would in any other.
    fn resize(&self, id: u32, size: Size) -> Result<(), String>;

    /// Opens a view of a session: `sink` is sent the screen as it already is and then the
    /// session's output, as text, until the view is closed. The answer says which view that
    /// is, and the size its screen was drawn for.
    fn watch(&self, id: u32, sink: Sink) -> Result<Watching, String>;

    /// Closes a view. Its session keeps running, with its terminal, for the next pane.
    fn unwatch(&self, id: u32, view: u32) -> Result<(), String>;

    /// Ends a session and everything it started. Its views end with it, and so does its token.
    fn close(&self, id: u32) -> Result<(), String>;

    /// Ends every session, and does not return until their programs are gone. This is what
    /// quitting calls: the process is about to end, and a thread would not be waited for.
    fn end_all(&self);

    /// Interrupts and ends every session's program, and everything each started, all at once,
    /// and does not return until they are gone. The sessions stay, with their last screens, as
    /// they do when a program ends on its own: this is the kill switch (OV-1), not a close.
    /// Answers how many it asked.
    fn stop_every_program(&self) -> usize;

    /// Refuses every start but the operator's own shell while `switch` is thrown (OV-1, ADR
    /// 0071). Every way a chat starts — the operator, a relaunch, a handoff, a curation
    /// action — comes through [`Self::open`], so that is the one place the refusal has to be.
    fn stopped_by(&mut self, switch: Arc<crate::killswitch::KillSwitch>);

    /// The operating system's id for a session's program, while it runs, where the host can
    /// say. The record keeps it, so `commit-msg` can tell a commit the chat's harness made from
    /// one made in a program that only inherited the chat's environment (V82, #1018); the tests
    /// ask it to see a stopped program is gone.
    fn process_id(&self, _id: u32) -> Option<u32> {
        None
    }

    /// Calls `tell` as each session's program ends, with the id and how it ended.
    ///
    /// No hook reports a program dying, and none can — the process is gone. This is the
    /// operating system telling the app, not charter reading a screen (ADR 0018).
    fn when_one_ends(&self, tell: Ends);

    /// Tell `tell` which sessions are reconnecting while the link to the host they run on is
    /// down, and when it is back (#2496): a host that relays to a runner says so. Never, where
    /// chats run in the window's own process.
    fn when_reconnecting(&self, tell: Reconnecting) {
        let _ = tell;
    }

    /// **What the window holds of what the runner these chats run on recorded** (#3393), read
    /// over the link from a cursor of its own; none for chats on this machine.
    fn runner_log(&self) -> Option<crate::runnerlog::RunnerLogNow> {
        None
    }

    /// The sessions that are reconnecting now (#2496), in order. None, where chats run in the
    /// window's own process.
    fn reconnecting(&self) -> Vec<u32> {
        Vec::new()
    }

    /// Whether session `id`'s program ended with the session host it ran in, which ended
    /// unexpectedly (#2007), rather than by itself or by the window's act: so until it is
    /// started again or closed. Never, where chats run in the window's own process.
    fn lost_with_its_host(&self, id: u32) -> bool {
        let _ = id;
        false
    }

    /// The sessions that are running, in the order they were opened.
    fn running(&self) -> Vec<u32>;

    /// Whether a session's program is ready to be typed at. Refused for a session that is gone.
    fn readiness(&self, id: u32) -> Result<Readiness, String>;

    /// A number no chat in this plane has had, taken now for a chat about to be started with
    /// it — for a caller that has to NAME the chat by its number before it starts: a handed-off
    /// chat with no task name is `<persona> <N>` (charter-app#258). Passed to [`Self::open`] as
    /// the number wanted, it is the one the chat gets, because nothing else can be dealt it.
    fn deal(&self) -> u32;

    /// The highest number this plane has dealt, for the record to keep.
    fn dealt(&self) -> u32;

    /// Says `dealt` numbers have already gone out in this plane, so none of them is dealt
    /// again — charter-app#90.
    ///
    /// A launch calls this with what the record kept, BEFORE putting any chat back. The
    /// numbers above what is in the record are the ones that matter: a chat that was closed
    /// before the quit is not in the record at all, and its `.charter/sessions/<n>.workspace`
    /// and `<n>.lock` are still on disk for the 30 days `wscmd::select`'s prune leaves them.
    fn already_dealt(&self, dealt: u32);

    /// **Starts a chat's confinement from `plan`**, its egress proxy and what runs beside it,
    /// where this host runs its chats (#1999), with `hearing` told what the proxy does. In the
    /// window's own process it starts here; the session host starts it there, from the same
    /// plan with the same code, and holds it for as long as the chat runs. A start that runs in
    /// it names it ([`Opening::confinement`]).
    fn confine(
        &self,
        plan: &purlis_core::sandbox::plan::Plan,
        hearing: purlis_core::sandbox::plan::Hearing,
    ) -> Result<purlis_core::sandbox::Confinement, String> {
        plan.start(hearing.here()).map_err(|why| why.to_string())
    }

    /// The socket the sessions' hooks report on, where this host listens on one: a sandbox
    /// charter wraps a chat in lets the chat reach it ([`purlis_core::sandbox::At`]).
    fn reports_to(&self) -> Option<std::path::PathBuf> {
        None
    }

    /// Saves the project at `root` from now on, where its chats run: its auto-save, telling
    /// `told` what each look that did something did. Dropping what it answers stops it. One
    /// per project: the window starts none of its own where its chats run in the session host,
    /// which saves the project there (#2002). This one is the window's own worker.
    fn keep_saving(
        &self,
        root: &std::path::Path,
        told: crate::autosave::Told,
    ) -> Box<dyn crate::autosave::Saver> {
        crate::autosave::here(root.to_path_buf(), told)
    }

    /// Hears what the session host hands on from the hook channel it owns for this plane with
    /// `relayed` (#2000). A host that runs its chats in this process listens itself, and is
    /// never told this.
    fn hear_through(&self, relayed: std::sync::Arc<crate::hooks::Relayed>) {
        let _ = relayed;
    }

    /// **Whether chat `number` is still in the host from before this window** (#2006): its
    /// program runs there, or ended there, after the window that started it went without a
    /// quit. A chat put back is then reattached ([`Self::reattach`]) rather than started again,
    /// on the conversation and the screen it has now. A host in the window's own process
    /// outlives no window, so it has none.
    fn left_running(&self, number: u32) -> bool {
        let _ = number;
        false
    }

    /// [`Self::left_running`], of a chat that worked in `workspace` (#2046): where the
    /// project's chats are routed, it is asked of this machine's host and of the one pool the
    /// workspace runs its chats on, and no other.
    fn left_running_in(&self, number: u32, workspace: Option<&str>) -> bool {
        let _ = workspace;
        self.left_running(number)
    }

    /// **Reattaches chat `number`** as a chat of this window (#2006): what [`Self::open`] does,
    /// for a program the host already runs, or ran, under that number. `announce` is called
    /// with the number before anything is told of the chat, as for a start. A program that
    /// ended while no window was there is told ended ([`Self::when_one_ends`]) once it is
    /// reattached.
    fn reattach(&self, number: u32, announce: &dyn Fn(u32)) -> Result<u32, String> {
        let _ = announce;
        Err(format!(
            "chat {number} is not in the session host to be reattached"
        ))
    }

    /// **The confinement chat `number` runs in, reattached** with `hearing` told what its proxy
    /// does from now on (#2006): none where the chat runs in none, or is not left running.
    fn confined_again(
        &self,
        number: u32,
        hearing: purlis_core::sandbox::plan::Hearing,
    ) -> Option<Result<purlis_core::sandbox::Confinement, String>> {
        let _ = (number, hearing);
        None
    }

    /// Why the state of each chat reattached is not the host's, where it is not: the board the
    /// host kept could not be taken as the window held the project (#2006), so each such chat
    /// reads as this window's own board has it until it next reports.
    fn state_not_reattached(&self) -> Option<String> {
        None
    }

    /// The plan of the confinement chat `number` ran in, as the window that started it planned
    /// it, where it is left running in one (#2006): what the window compares with the plan it
    /// makes now, to know whether its sandbox changed while no window was there.
    fn ran_with(&self, number: u32) -> Option<purlis_core::sandbox::plan::Plan> {
        let _ = number;
        None
    }

    /// **How chat `number`, left running, is sandboxed, as its host applied it** (#2636): `None`
    /// where its host keeps no such record, which the window takes as sandboxed. Read before
    /// it is reattached.
    fn ran_in(&self, number: u32) -> Option<purlis_session_protocol::session::RunSandbox> {
        let _ = number;
        None
    }

    /// Every chat this plane left running in the host that nothing reattached is closed
    /// (#2006): what a put back that settled leaves no tab for, so nothing runs unseen.
    fn close_what_was_not_reattached(&self) {}

    /// **The browsers this plane left in the host from before this window** (#2025), running
    /// or ended, each answered once: what the put back takes back with their chats
    /// (`crate::chatbrowsers::take_back`). A host in the window's own process outlives no window,
    /// and one before 1.39 answers no browser, so either has none.
    fn browsers_left(&self) -> Vec<purlis_session_protocol::session::HeldProgram> {
        Vec::new()
    }
}

/// One chat as the board has it, read under one hold so the three agree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Glance {
    pub state: State,
    /// Whether it is asking for you.
    pub asking: bool,
    /// How many turns it has taken this run.
    pub turns: u32,
    /// The prompt it is stopped on in its terminal, where it is (#1691).
    pub prompt: Option<purlis_core::harness::model::Prompt>,
}

impl Glance {
    /// Whether it is stopped, now, on the prompt it asked mid-turn (#1601): it asked, and it
    /// has not been answered or got past it since, which puts it back to running.
    /// `purlis_core::state::Chat::waits_on_its_prompt`, read off one glance.
    pub fn waits_on_its_prompt(&self) -> bool {
        self.asking && self.state == State::Waiting
    }
}

/// What the host's board says about each chat, as its hooks reported it, and the moves the
/// window makes on it. Nothing here reads a harness's output (ADR 0018).
///
/// Each move answers the [`Moved`] the window must now be told, built under the same hold as
/// the change, so the queue it carries is the board's as of that change.
pub trait ChatBoard: Send + Sync {
    /// Chat `session` as the board has it now.
    fn glance(&self, session: u32) -> Glance;

    /// The conversation chat `session`'s harness is in, where the board knows it.
    fn conversation(&self, session: u32) -> Option<String>;

    /// What the window is told when something other than a hook moves a chat: a chat opening,
    /// or a program that has died. A chat closing is [`Self::closed`].
    fn now(&self, session: u32) -> Moved;

    /// Takes a chat off the board — closed, not merely ended.
    fn closed(&self, session: u32) -> Moved;

    /// Drops a chat's request without answering it — the operator's Ignore (charter-app#248).
    /// Answered whether or not the board changed: a window that asked to ignore a chat believes
    /// it is asking, and the board's answer is the one to leave it with either way.
    fn ignored(&self, session: u32) -> Moved;

    /// A chat `session` handed work to, shown as `from`, has reported back to it
    /// (charter-app#259). Nothing when no reader would see a difference.
    fn reported_back(&self, session: u32, from: &str) -> Option<Moved>;

    /// The operator stopped `from`, a chat `session` started (#1448): its row says so, and it
    /// is no needs-you item. Nothing when no reader would see a difference.
    fn stopped_below(&self, session: u32, from: &str) -> Option<Moved>;

    /// The app found that chat `session` needs the person, for `need` (#1448): a needs-you
    /// item on it. Nothing when no reader would see a difference.
    fn needs(&self, session: u32, need: purlis_core::state::Need) -> Option<Moved>;

    /// purlis's scan refused a commit the app was making for chat `session` (#1476): a
    /// needs-you item on it saying `what`, masked, as a chat's own refused commit raises
    /// (SQ-16). Nothing when no reader would see a difference.
    fn commit_refused(&self, session: u32, what: &str) -> Option<Moved>;

    /// Chat `session`'s report reached the chat that asked for it (#1448): the turn it is in
    /// ends without a needs-you item. Nothing a reader sees changes now.
    fn reported_to_its_asker(&self, session: u32);

    /// What chat `session` waited on is over, and nothing will prompt it (#1491): the end of
    /// turn that was held for its tasks, or for its asker's answer, is the needs-you item now.
    /// Nothing when no reader would see a difference.
    fn rested(&self, session: u32) -> Option<Moved>;

    /// A task chat `session` asked for failed, ended without a report, or did not start
    /// (#1491): a needs-you item on it. Nothing when no reader would see a difference.
    fn task_failed(&self, session: u32, failed: purlis_core::state::FailedTask) -> Option<Moved>;

    /// The person looked at failure `id` of chat `session`, or cleared its row (#1491): that
    /// one item goes. Nothing when no reader would see a difference.
    fn failure_cleared(&self, session: u32, id: &str) -> Option<Moved>;
    /// The person answered, in the window, the prompt `asker` showed in chat `session` (HP-6):
    /// only that prompt goes, and its turn goes on once nobody else's is open
    /// (`state::Chat::answered_by`, #1644). Nothing when no reader would see a difference.
    fn answered_by(
        &self,
        session: u32,
        asker: &purlis_core::harness::hooked::Asker,
    ) -> Option<Moved>;
}

/// A session host that runs nothing, for a test of what sits above the seam.
#[cfg(test)]
pub(crate) mod pretend {
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
    use std::time::Duration;

    use purlis_core::engine::Size;
    use purlis_core::session::Exit;

    use super::{Ends, NOT_COMPOSED_HERE, Named, Opening, Readiness, SessionHost, Sink, Watching};

    /// Deals numbers, remembers what it was asked to run, and ends a session when a test says
    /// its program ended. A clone is the same host, so a test keeps one after handing one over.
    #[derive(Clone, Default)]
    pub struct Pretend {
        seen: Arc<Seen>,
    }

    #[derive(Default)]
    struct Seen {
        /// The switch that stops every agent, where one was handed over: a start is refused
        /// while it is thrown, as the real host refuses it.
        switch: Mutex<Option<Arc<crate::killswitch::KillSwitch>>>,
        asked: Mutex<Vec<(u32, String)>>,
        openings: Mutex<Vec<Opening>>,
        /// The sessions running as each opening was asked for, in the order asked.
        running_when_opened: Mutex<Vec<Vec<u32>>>,
        socket: Mutex<Option<std::path::PathBuf>>,
        running: Mutex<Vec<u32>>,
        typed: Mutex<Vec<(u32, Vec<u8>)>>,
        dealt: AtomicU32,
        ends: Mutex<Option<Ends>>,
        /// Why it starts nothing, while a test has it refuse ([`Pretend::refuses`]).
        refusing: Mutex<Option<String>>,
        /// The numbers it starts nothing under, and why ([`Pretend::refuses_number`]).
        refusing_numbers: Mutex<Vec<(u32, String)>>,
        /// How long each start takes ([`Pretend::takes`]).
        taking: Mutex<Duration>,
        /// The numbers whose start does not return until let go ([`Pretend::hangs_number`]).
        hanging: Mutex<Vec<u32>>,
        /// How many starts are under way now, and the most there were at once.
        starting: AtomicU32,
        most_starting: AtomicU32,
        /// Whether ending every session waits until a test lets it go
        /// ([`Pretend::holds_its_end`]): a chat that ignores its end.
        end_held: Mutex<bool>,
        end_let_go: std::sync::Condvar,
        /// How many ends of every session are under way now, and the most there were at once.
        ending: AtomicU32,
        most_ending: AtomicU32,
        /// The sessions whose program ended with their host ([`Pretend::program_ends_with_its_host`]).
        lost: Mutex<Vec<u32>>,
        /// The chats left running from before this window, and the plan each ran in
        /// ([`Pretend::leaves_running`], #2006).
        left: Mutex<Vec<(u32, Option<purlis_core::sandbox::plan::Plan>)>>,
        /// How each chat left running is sandboxed, as its host recorded it (#2636).
        ran_in: Mutex<Vec<(u32, Option<purlis_session_protocol::session::RunSandbox>)>>,
        /// The chats reattached, in order.
        reattached: Mutex<Vec<u32>>,
        /// The chats left running that nothing reattached, closed.
        closed_left: Mutex<Vec<u32>>,
        /// The browsers left from before this window ([`Pretend::leaves_a_browser`], #2025).
        browsers_left: Mutex<Vec<purlis_session_protocol::session::HeldProgram>>,
        /// Whether it runs its chats on a runner, which composes each harness chat
        /// ([`Pretend::on_a_runner`], #2446).
        on_a_runner: std::sync::atomic::AtomicBool,
        /// The pool that runner is of, where a test named one ([`Pretend::on_pool`], #2049).
        pool: Mutex<Option<String>>,
        /// Each fetch back it was asked for, by workspace and pool, and how many ran at once
        /// at most (#2046).
        fetched: Mutex<Vec<(String, String)>>,
        /// Each move asked of it (#2048): `ahead`, `in` or `out`, the pool and the workspace.
        moved: Mutex<Vec<(&'static str, String, String)>>,
        fetching: AtomicU32,
        most_fetching: AtomicU32,
        /// Each chat started by its names, its number, and whether its hearing asks.
        named: Mutex<Vec<(u32, Named, bool)>>,
        /// How it records the run of each chat it composes from now on, where a test said
        /// ([`Pretend::records_composed_runs_as`], #2692): in `seatbelt` otherwise.
        composed_ran: Mutex<Option<Option<purlis_session_protocol::session::RunSandbox>>>,
        /// The chats it says run on a pool's runner, and the pool ([`Pretend::runs_on_pool`]).
        pools: Mutex<Vec<(u32, String)>>,
    }

    impl Pretend {
        /// Says chat `id` runs on `pool`'s runner from now on (#2047), as a router does.
        pub fn runs_on_pool(&self, id: u32, pool: &str) {
            lock(&self.seen.pools).push((id, pool.to_owned()));
        }

        /// Runs its chats on a runner from now on, which composes each harness chat (#2446).
        pub fn on_a_runner(&self) {
            self.seen.on_a_runner.store(true, Ordering::SeqCst);
        }

        /// Runs its chats on a runner of the pool `pool` from now on (#2049).
        pub fn on_pool(&self, pool: &str) {
            self.on_a_runner();
            *lock(&self.seen.pool) = Some(pool.to_owned());
        }

        /// Each fetch back it was asked for, by workspace and pool, in order.
        /// Each move asked of it (#2048).
        pub fn moved(&self) -> Vec<(&'static str, String, String)> {
            lock(&self.seen.moved).clone()
        }

        pub fn fetched(&self) -> Vec<(String, String)> {
            lock(&self.seen.fetched).clone()
        }

        /// The most fetches back that ran at once.
        pub fn most_fetching_at_once(&self) -> u32 {
            self.seen.most_fetching.load(Ordering::SeqCst)
        }

        /// Records the run of each chat it composes from now on as `ran`, as a host answers its
        /// start: `None` for a host that keeps no such record (#2692).
        pub fn records_composed_runs_as(
            &self,
            ran: Option<purlis_session_protocol::session::RunSandbox>,
        ) {
            *lock(&self.seen.composed_ran) = Some(ran);
        }

        /// Each chat started by its names, in order: its number, what it was named, and whether
        /// its hearing asks.
        pub fn named(&self) -> Vec<(u32, Named, bool)> {
            lock(&self.seen.named).clone()
        }

        /// Chat `number` runs from before this window, in `plan` where it is confined: a window
        /// that comes back reattaches it (#2006).
        pub fn leaves_running(&self, number: u32, plan: Option<purlis_core::sandbox::plan::Plan>) {
            self.leaves_running_in(
                number,
                plan,
                Some(purlis_session_protocol::session::RunSandbox::By(
                    "seatbelt".to_owned(),
                )),
            );
        }

        /// [`Self::leaves_running`], its run sandboxed as `ran` says, as the host recorded it:
        /// `None` for a host that keeps no such record (#2636).
        pub fn leaves_running_in(
            &self,
            number: u32,
            plan: Option<purlis_core::sandbox::plan::Plan>,
            ran: Option<purlis_session_protocol::session::RunSandbox>,
        ) {
            lock(&self.seen.left).push((number, plan));
            lock(&self.seen.ran_in).push((number, ran));
        }

        /// Browser `browser` runs from before this window: a window that comes back takes it
        /// back with its chat, or ends it (#2025).
        pub fn leaves_a_browser(&self, browser: purlis_session_protocol::session::HeldProgram) {
            lock(&self.seen.browsers_left).push(browser);
        }

        /// The chats reattached, in order.
        pub fn reattached(&self) -> Vec<u32> {
            lock(&self.seen.reattached).clone()
        }

        /// The chats left running that nothing reattached, and so were closed.
        pub fn closed_left(&self) -> Vec<u32> {
            lock(&self.seen.closed_left).clone()
        }

        /// Ending every session waits, from now on, until [`Self::lets_its_end_go`]: its chats
        /// ignore their end, as a slow program does.
        pub fn holds_its_end(&self) {
            *lock(&self.seen.end_held) = true;
        }

        /// The most ends of every session that were under way at once.
        pub fn most_ending_at_once(&self) -> u32 {
            self.seen.most_ending.load(Ordering::SeqCst)
        }

        /// Lets every end waiting in [`Self::holds_its_end`] go on.
        pub fn lets_its_end_go(&self) {
            *lock(&self.seen.end_held) = false;
            self.seen.end_let_go.notify_all();
        }

        /// Every session it was asked to start, by number and program.
        pub fn asked(&self) -> Vec<(u32, String)> {
            lock(&self.seen.asked).clone()
        }

        /// The sessions running as each start was asked for, in the order asked.
        pub fn running_when_opened(&self) -> Vec<Vec<u32>> {
            lock(&self.seen.running_when_opened).clone()
        }

        /// Everything it was asked to start, whole, in the order it was asked.
        pub fn openings(&self) -> Vec<Opening> {
            lock(&self.seen.openings).clone()
        }

        /// Refuses every start from now with `why`, as a host with no pseudo-terminal to give
        /// does; `None` starts them again.
        pub fn refuses(&self, why: Option<&str>) {
            *lock(&self.seen.refusing) = why.map(str::to_owned);
        }

        /// Refuses a start asked for under `number` with `why`, and starts every other: what
        /// a launch meets when one chat's program will not start and the rest do.
        pub fn refuses_number(&self, number: u32, why: &str) {
            lock(&self.seen.refusing_numbers).push((number, why.to_owned()));
        }

        /// Every start from now takes `long`, as a harness and its sandbox coming up do.
        pub fn takes(&self, long: Duration) {
            *lock(&self.seen.taking) = long;
        }

        /// A start asked for under `number` does not return until [`Self::lets_go`]: a
        /// program that hangs as it starts.
        pub fn hangs_number(&self, number: u32) {
            lock(&self.seen.hanging).push(number);
        }

        /// The start under `number` that hung returns.
        pub fn lets_go(&self, number: u32) {
            lock(&self.seen.hanging).retain(|one| *one != number);
        }

        /// The most starts that were under way at once.
        pub fn most_at_once(&self) -> u32 {
            self.seen.most_starting.load(Ordering::SeqCst)
        }

        /// Starts under `number` again.
        pub fn starts_number(&self, number: u32) {
            lock(&self.seen.refusing_numbers).retain(|(one, _)| *one != number);
        }

        /// Everything written to session `id`'s terminal, in order: what a test reads to know
        /// which keys reached a chat, and that none did.
        pub fn typed(&self, id: u32) -> Vec<Vec<u8>> {
            lock(&self.seen.typed)
                .iter()
                .filter(|(to, _)| *to == id)
                .map(|(_, bytes)| bytes.clone())
                .collect()
        }

        /// Says its sessions report on `socket`.
        pub fn reporting_on(&self, socket: std::path::PathBuf) {
            *lock(&self.seen.socket) = Some(socket);
        }

        /// Session `id`'s program ends with `exit`, and whoever asked is told.
        pub fn program_ends(&self, id: u32, exit: Exit) {
            let tell = lock(&self.seen.ends).clone();
            if let Some(tell) = tell {
                tell(id, exit);
            }
        }

        /// Session `id`'s program ends with its session host, which ended unexpectedly
        /// (#2007), and whoever asked is told.
        pub fn program_ends_with_its_host(&self, id: u32) {
            lock(&self.seen.lost).push(id);
            self.program_ends(id, Exit::Signal("Hangup".to_owned()));
        }

        fn here(&self, id: u32) -> Result<(), String> {
            if lock(&self.seen.running).contains(&id) {
                Ok(())
            } else {
                Err(format!("session {id} is not running"))
            }
        }
    }

    impl SessionHost for Pretend {
        fn open(
            &self,
            wanted: Option<u32>,
            opening: &Opening,
            announce: &dyn Fn(u32),
        ) -> Result<u32, String> {
            if let Some(why) = lock(&self.seen.refusing).clone() {
                return Err(why);
            }
            if let Some((_, why)) = lock(&self.seen.refusing_numbers)
                .iter()
                .find(|(number, _)| wanted == Some(*number))
            {
                return Err(why.clone());
            }
            let stopped = lock(&self.seen.switch)
                .as_ref()
                .is_some_and(|switch| switch.is_stopped());
            if !opening.operator_shell && stopped {
                return Err(crate::sessions::STOPPED.to_owned());
            }
            let now = self.seen.starting.fetch_add(1, Ordering::SeqCst) + 1;
            self.seen.most_starting.fetch_max(now, Ordering::SeqCst);
            let long = *lock(&self.seen.taking);
            if !long.is_zero() {
                std::thread::sleep(long);
            }
            while wanted.is_some_and(|number| lock(&self.seen.hanging).contains(&number)) {
                std::thread::sleep(Duration::from_millis(2));
            }
            self.seen.starting.fetch_sub(1, Ordering::SeqCst);
            let id = wanted.unwrap_or_else(|| self.deal());
            self.seen.dealt.fetch_max(id, Ordering::SeqCst);
            announce(id);
            let program = opening.program.clone().unwrap_or_default();
            // Under one hold: starts come several at once (`restoring.rs`), and a test reads
            // an opening by its place among the ones asked.
            let mut asked = lock(&self.seen.asked);
            asked.push((id, program));
            lock(&self.seen.openings).push(opening.clone());
            let running_now = lock(&self.seen.running).clone();
            lock(&self.seen.running_when_opened).push(running_now);
            lock(&self.seen.running).push(id);
            drop(asked);
            Ok(id)
        }
        fn open_named(
            &self,
            wanted: Option<u32>,
            named: &Named,
            hearing: purlis_core::sandbox::plan::Hearing,
            announce: &dyn Fn(u32),
        ) -> Result<super::Composed, String> {
            if !self.composes_its_harness_chats() {
                return Err(NOT_COMPOSED_HERE.to_owned());
            }
            if let Some(why) = lock(&self.seen.refusing).clone() {
                return Err(why);
            }
            let id = wanted.unwrap_or_else(|| self.deal());
            self.seen.dealt.fetch_max(id, Ordering::SeqCst);
            announce(id);
            lock(&self.seen.named).push((id, named.clone(), hearing.asks.is_some()));
            lock(&self.seen.running).push(id);
            Ok(super::Composed {
                number: id,
                confinement: None,
                ran: lock(&self.seen.composed_ran).clone().unwrap_or_else(|| {
                    Some(purlis_session_protocol::session::RunSandbox::By(
                        "seatbelt".to_owned(),
                    ))
                }),
            })
        }
        fn move_ahead(&self, pool: &str, workspace: &str) -> Result<(), String> {
            lock(&self.seen.moved).push(("ahead", pool.to_owned(), workspace.to_owned()));
            Ok(())
        }
        fn move_in(&self, pool: &str, chat: &super::Moving) -> Result<(), String> {
            lock(&self.seen.moved).push(("in", pool.to_owned(), chat.workspace.clone()));
            Ok(())
        }
        fn move_out(&self, pool: &str, chat: &super::Moving) -> Result<Vec<String>, String> {
            lock(&self.seen.moved).push(("out", pool.to_owned(), chat.workspace.clone()));
            Ok(Vec::new())
        }
        fn fetch_back(&self, workspace: &str, pool: &str) -> Vec<String> {
            let now = self.seen.fetching.fetch_add(1, Ordering::SeqCst) + 1;
            self.seen.most_fetching.fetch_max(now, Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(50));
            lock(&self.seen.fetched).push((workspace.to_owned(), pool.to_owned()));
            self.seen.fetching.fetch_sub(1, Ordering::SeqCst);
            Vec::new()
        }
        fn composes_its_harness_chats(&self) -> bool {
            self.seen.on_a_runner.load(Ordering::SeqCst)
        }
        fn pool_of(&self, id: u32) -> Option<String> {
            lock(&self.seen.pools)
                .iter()
                .find(|(chat, _)| *chat == id)
                .map(|(_, pool)| pool.clone())
        }
        fn pool_for(&self, _workspace: Option<&str>) -> Option<String> {
            lock(&self.seen.pool).clone()
        }
        fn input(&self, id: u32, bytes: &[u8]) -> Result<(), String> {
            self.here(id)?;
            lock(&self.seen.typed).push((id, bytes.to_vec()));
            Ok(())
        }
        fn resize(&self, id: u32, _size: Size) -> Result<(), String> {
            self.here(id)
        }
        fn watch(&self, id: u32, _sink: Sink) -> Result<Watching, String> {
            self.here(id)?;
            Ok(Watching {
                view: 1,
                size: Size {
                    columns: 80,
                    rows: 24,
                },
            })
        }
        fn unwatch(&self, id: u32, _view: u32) -> Result<(), String> {
            self.here(id)
        }
        fn close(&self, id: u32) -> Result<(), String> {
            self.here(id)?;
            lock(&self.seen.running).retain(|one| *one != id);
            Ok(())
        }
        fn end_all(&self) {
            let now = self.seen.ending.fetch_add(1, Ordering::SeqCst) + 1;
            self.seen.most_ending.fetch_max(now, Ordering::SeqCst);
            let mut held = lock(&self.seen.end_held);
            while *held {
                held = self
                    .seen
                    .end_let_go
                    .wait(held)
                    .unwrap_or_else(PoisonError::into_inner);
            }
            drop(held);
            lock(&self.seen.running).clear();
            self.seen.ending.fetch_sub(1, Ordering::SeqCst);
        }
        fn stop_every_program(&self) -> usize {
            lock(&self.seen.running).len()
        }
        fn stopped_by(&mut self, switch: Arc<crate::killswitch::KillSwitch>) {
            *lock(&self.seen.switch) = Some(switch);
        }
        fn when_one_ends(&self, tell: Ends) {
            *lock(&self.seen.ends) = Some(tell);
        }
        fn lost_with_its_host(&self, id: u32) -> bool {
            lock(&self.seen.lost).contains(&id)
        }
        fn running(&self) -> Vec<u32> {
            lock(&self.seen.running).clone()
        }
        fn readiness(&self, id: u32) -> Result<Readiness, String> {
            self.here(id).map(|()| Readiness {
                edits_lines: None,
                quiet_for: Duration::ZERO,
            })
        }
        fn deal(&self) -> u32 {
            self.seen.dealt.fetch_add(1, Ordering::SeqCst) + 1
        }
        fn dealt(&self) -> u32 {
            self.seen.dealt.load(Ordering::SeqCst)
        }
        fn already_dealt(&self, dealt: u32) {
            self.seen.dealt.fetch_max(dealt, Ordering::SeqCst);
        }
        fn reports_to(&self) -> Option<std::path::PathBuf> {
            lock(&self.seen.socket).clone()
        }
        fn left_running(&self, number: u32) -> bool {
            lock(&self.seen.left).iter().any(|(one, _)| *one == number)
        }
        fn ran_with(&self, number: u32) -> Option<purlis_core::sandbox::plan::Plan> {
            lock(&self.seen.left)
                .iter()
                .find(|(one, _)| *one == number)
                .and_then(|(_, plan)| plan.clone())
        }
        fn ran_in(&self, number: u32) -> Option<purlis_session_protocol::session::RunSandbox> {
            lock(&self.seen.ran_in)
                .iter()
                .find(|(one, _)| *one == number)
                .and_then(|(_, ran)| ran.clone())
        }
        fn reattach(&self, number: u32, announce: &dyn Fn(u32)) -> Result<u32, String> {
            let mut left = lock(&self.seen.left);
            let at = left
                .iter()
                .position(|(one, _)| *one == number)
                .ok_or_else(|| format!("chat {number} is not left running"))?;
            left.remove(at);
            drop(left);
            self.seen.dealt.fetch_max(number, Ordering::SeqCst);
            announce(number);
            lock(&self.seen.reattached).push(number);
            lock(&self.seen.running).push(number);
            Ok(number)
        }
        fn close_what_was_not_reattached(&self) {
            let left: Vec<u32> = lock(&self.seen.left)
                .drain(..)
                .map(|(one, _)| one)
                .collect();
            lock(&self.seen.closed_left).extend(left);
        }
        fn browsers_left(&self) -> Vec<purlis_session_protocol::session::HeldProgram> {
            std::mem::take(&mut *lock(&self.seen.browsers_left))
        }
    }

    fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
        mutex.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests;

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
    /// Its prompt key (#3369), which the window minted for a harness chat: set in its
    /// environment for its hooks, and handed to the session host in its launch, which keeps it
    /// for a window that takes the chat back. Its `Debug` shows nothing of it. None for a shell.
    pub prompt_key: Option<purlis_core::skillspan::PromptSalt>,
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
            prompt_key: None,
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
    /// The runner it starts on where that is not its pool's own: a chat's fresh runner, by the
    /// name a link reaches it by (#3818).
    pub runner: Option<String>,
    /// Whether its runner holds its code from an earlier start: a chat's fresh runner holds the
    /// only copy of its work, which sending this machine's branches over would undo (#4569).
    pub its_code_is_there: bool,
    pub chat: Option<String>,
    pub run: Option<String>,
    pub profile: Option<String>,
    pub persona: Option<String>,
    pub workspace: Option<String>,
    pub resume: Option<String>,
    pub first_message: Option<String>,
    pub without_sandbox: bool,
    /// Where its start chose to run it (#4536): what the router asks before the levels.
    pub runs_on: Option<purlis_core::machine::StartChoice>,
    /// What the person allowed of its persona here, as its digest: what arms its tools on a
    /// runner (1.55, #4071).
    pub tools_allowed: Option<String>,
    /// Whether the plane's skills reach it, as this window read the Skill studio's flag: what a
    /// runner arms it by (1.62, SK-20b #3084).
    pub plane_skills: bool,
    pub alone: Vec<u16>,
    pub size: Size,
    /// Marked as its program is asked to run there (#4781), shared with whoever started it.
    pub asked_to_run: AskedToRun,
}

/// **Whether a start's program was asked to run** (#4781): marked before the host's answer, so
/// a start that fails after it counts as one that may have run, and wrote work, there.
#[derive(Debug, Clone, Default)]
pub struct AskedToRun(std::sync::Arc<std::sync::atomic::AtomicBool>);

impl AskedToRun {
    /// Its program is asked to run now.
    pub fn mark(&self) {
        self.0.store(true, std::sync::atomic::Ordering::SeqCst);
    }

    /// Its host said its program did not start, in a refusal that says so.
    pub fn unmark(&self) {
        self.0.store(false, std::sync::atomic::Ordering::SeqCst);
    }

    /// Whether its program was asked to run.
    pub fn was(&self) -> bool {
        self.0.load(std::sync::atomic::Ordering::SeqCst)
    }
}

/// **What a runner's chat started without there, and the runner** (#6078): what
/// [`SessionHost::skills_not_there_left`] answers of a chat left running.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunnersLack {
    pub skills_not_there: purlis_session_protocol::session::SkillsNotThere,
    /// As [`Composed::runner`] names it.
    pub runner: Option<String>,
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
    /// The prompt key the host minted for it (#3369), where it says one.
    pub prompt_key: Option<purlis_core::skillspan::PromptSalt>,
    /// The skills the host armed it with, by name and level (1.77, #4218): `None` from a host
    /// before 1.77, which does not say them.
    pub armed_skills: Option<Vec<purlis_session_protocol::session::ArmedSkill>>,
    /// The skills its persona names that it started without on the host's machine (1.81,
    /// #2977): `None` where none is, and from a host before 1.81, which does not say them.
    pub skills_not_there: Option<purlis_session_protocol::session::SkillsNotThere>,
    /// The runner it started on, as `<pool>/<runner>` where its link names the runner, or its
    /// pool alone: `None` where the link reaches no pool's runner.
    pub runner: Option<String>,
}

/// **A chat that moves between this machine and a runner** (#2048): where it works, the profile
/// it runs on, and the conversation it is in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Moving {
    pub workspace: String,
    pub profile: String,
    pub conversation: String,
}

/// **What came back with chats moved off a runner** (#2048, #3811): what the person is to look
/// at before a tool here runs it, and each clone the runner's chat made, made here whole from
/// its work.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CameBack {
    /// Every file that came which a tool here runs or reads as its settings, every one a save
    /// would refuse, and every link, by its path in the project (the person's ruling of
    /// 2026-10-27).
    pub watch: Vec<String>,
    /// Each clone made here fresh from the runner's chat's work (#3811): every file of it was
    /// made there (#3990).
    pub made: Vec<MadeHere>,
    /// Each clone the runner's chat made that stayed on the runner, said with why and what to do
    /// (#5601).
    pub stayed: Vec<String>,
}

/// **A clone made here fresh from a runner's chat's work** (#3811, #3985).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MadeHere {
    /// Its folder in the project.
    pub clone: String,
    /// The `origin` the runner's clone had, offered to the person and never written into it, or
    /// why none is.
    pub remote: Result<String, String>,
}

/// What a host that moves no chat says.
pub const MOVES_NO_CHAT: &str = "chats do not move to or from where this one runs";

/// Why a chat by what it is named alone is not started in the window's own process.
pub const NOT_COMPOSED_HERE: &str =
    "a chat in the window is started from what the window composed, never by its names alone";

/// Where a view's output goes: the bytes its program wrote, as it wrote them, never decoded on
/// the way (#4745). It is called on the view's own thread, one batch at a time.
pub type Sink = Box<dyn FnMut(Vec<u8>) + Send>;

/// What a fresh runner's end is told once its work came back or did not (#4569): what did not, in
/// sentences. It answers whether the runner went.
pub type FreshDone = Box<dyn FnOnce(Vec<String>) -> bool + Send>;

/// Told how each session ended, as it ends, with the session's id. An `Arc` so each session
/// can hold it for as long as its program runs.
pub type Ends = Arc<dyn Fn(u32, Exit) + Send + Sync>;

/// Told which sessions are reconnecting while the link to the session host they run on is
/// down, and which are reconnected (#2496).
pub type Reconnecting = Arc<dyn Fn(Vec<u32>, crate::reconnecting::Reconnection) + Send + Sync>;

/// Told each skill run a runner's event log says started or ended, with the pool whose runner it
/// is where the host knows it ([`SessionHost::when_skill_runs_heard`], #3084).
pub type SkillRunsHeard =
    Arc<dyn Fn(Option<&str>, &purlis_core::skilluse::relayed::Relayed) + Send + Sync>;

/// A view the UI has open, and the size the screen it opened on was drawn for.
#[derive(Debug, Clone, Copy)]
pub struct Watching {
    pub view: u32,
    pub size: Size,
    /// How many lines of history the session keeps above those its screen was drawn with
    /// (#4742): what the pane asks for as the person scrolls up. None where it drew all.
    pub older: u32,
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
    /// (#2046), with what its `start` chose (#4536): where the window routes each chat to the
    /// host that runs it, a chat that runs on a pool's runner is started by its names there. Every other
    /// host answers as [`Self::composes_its_harness_chats`] does.
    fn composes_its_harness_chats_in(
        &self,
        workspace: Option<&str>,
        start: Option<&purlis_core::machine::StartChoice>,
    ) -> bool {
        let _ = (workspace, start);
        self.composes_its_harness_chats()
    }

    /// **Whether the host chat `id` runs on composes its harness chats** (SK-15g, #3604): asked
    /// of a chat this window reattaches, whose start it did not see. Where the window routes
    /// chats by workspace, that host is the one the chat was found running on, never the one its
    /// workspace names now. Every other host answers as [`Self::composes_its_harness_chats`]
    /// does.
    fn composes_the_harness_chat(&self, id: u32) -> bool {
        let _ = id;
        self.composes_its_harness_chats()
    }

    /// **The pool a chat started in `workspace` runs on** (#2049), with what its `start` chose
    /// (#4536), where the window routes chats to a pool's runner: what the profile it starts on
    /// is kept on, and signed in on, before it starts. None, where it starts on this machine.
    fn pool_for(
        &self,
        workspace: Option<&str>,
        start: Option<&purlis_core::machine::StartChoice>,
    ) -> Option<String> {
        let _ = (workspace, start);
        None
    }

    /// **The pool chat `id` runs on**, where it runs on a pool's runner (#2046): what the window
    /// holds its writes to the project for the person by. None, where it runs on this machine.
    fn pool_of(&self, id: u32) -> Option<String> {
        let _ = id;
        None
    }

    /// **Whether the runner chat `id` runs on records its turns** (session protocol 1.78,
    /// #5427): they come back with its code, and one is undone there. False for a chat here,
    /// whose turns the window records itself, and for a runner too old to.
    fn records_turns_of(&self, id: u32) -> bool {
        let _ = id;
        false
    }

    /// **Has the runner of `pool` undo `turn`** of a chat it runs, in the folder `place` names
    /// (1.78, #5427): answered as the runner answers, or why it could not be asked.
    fn undo_turn_on(
        &self,
        pool: &str,
        place: &purlis_core::files::turns::Place,
        turn: &purlis_core::files::TurnKey,
    ) -> Result<purlis_session_protocol::session::Answer, String> {
        let _ = (place, turn);
        Err(format!(
            "this window reaches no runner of pool {pool}, so the turn was not undone"
        ))
    }

    /// **Whether Move here brings back a clone the runner chat `id` runs on made** (#3811): its
    /// runner names the clones it holds (session protocol 1.51). False for a chat here, and for
    /// one on a runner before 1.51, or one this window has no link to now.
    fn clones_come_back(&self, id: u32) -> bool {
        let _ = id;
        false
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

    /// **Whether this host's chats stay on a runner that drains now, and new ones go elsewhere**
    /// (#4248): from a yes on, this host starts nothing, and whoever asked makes another for the
    /// pool's runner now. No, wherever chats run on nothing that drains.
    fn stays_on_a_draining_runner(&self) -> bool {
        false
    }

    /// Whether the draining runner this host stayed on is kept no more (#4248): its link has ended.
    fn runner_is_gone(&self) -> bool {
        false
    }

    /// **Fetches the branches of `workspace`'s clones back from the runner of `pool`** (#2046),
    /// where this host runs the project's chats there: each clone's `refs/remotes/<pool>/*`.
    /// Answers what could not be fetched, in sentences. Nothing, where chats run elsewhere.
    fn fetch_back(&self, workspace: &str, pool: &str) -> Vec<String> {
        let _ = (workspace, pool);
        Vec::new()
    }

    /// [`Self::fetch_back`] from a pool `workspace` no longer runs on (#4590): it is told nothing
    /// of what this machine has but what came back from it.
    fn fetch_back_left(&self, workspace: &str, pool: &str) -> Vec<String> {
        self.fetch_back(workspace, pool)
    }

    /// **Everything a chat's fresh runner `runner` (`<pool>/fresh-…`) has of `workspace` comes
    /// back to stay** (#4569): each clone it holds, one made there among them, each clone's
    /// branches kept under `refs/remotes/<runner>/`, as the runner is about to go. Answers what
    /// did not come, in sentences. A host that reaches no runner says so.
    fn fetch_back_whole(&self, workspace: &str, runner: &str) -> Vec<String> {
        vec![format!(
            "this host does not reach runner {runner}, so nothing of {workspace} came back"
        )]
    }

    /// **What fresh runner `runner` holds that this project does not** (#4569), in sentences. A
    /// runner goes only on none, so a host that reaches no runner says so. It writes nothing here
    /// and can take minutes, so it never runs under the one-at-a-time lock on fetches (#5393).
    fn not_home_of_runner(&self, runner: &str) -> Vec<String> {
        vec![format!(
            "this host does not reach runner {runner}, so purlis can't tell its work is here"
        )]
    }

    /// **Whether every program of this project on this host's runner has ended** (#5393), or why
    /// that can't be said: a close is answered before its program ends, and what still runs may
    /// yet commit after a fetch. Yes for a host that runs nothing of it on a runner.
    fn programs_have_ended(&self) -> Result<(), String> {
        Ok(())
    }

    /// Whether this host's link reaches the runner named `runner` now (#5393). No, for a host with
    /// no link open.
    fn reaches_runner(&self, _runner: &str) -> bool {
        false
    }

    /// **A chat's fresh runner `runner` is finished with** (#4569): what it has of `workspace`
    /// comes back ([`Self::fetch_back_whole`]), off this thread where it crosses a link, and
    /// `done` is told what did not. No workspace is a runner nothing ran on: `done` is told at
    /// once, and nothing is sent or asked of it.
    fn take_back_fresh(&self, runner: &str, workspace: Option<&str>, done: FreshDone) {
        done(workspace.map_or_else(Vec::new, |workspace| {
            let failed = self.fetch_back_whole(workspace, runner);
            if failed.is_empty() {
                self.not_home_of_runner(runner)
            } else {
                failed
            }
        }));
    }

    /// **The way to a runner kept no more goes** (#4569): its link ends.
    fn let_go_of_runner(&self) {}

    /// **Readies the runner of `pool` for a chat in `workspace` to move there** (#2048): the
    /// project held there, and each clone's branch sent ahead, while the chat keeps working
    /// here. Refused in a sentence where chats cannot move to it.
    fn move_ahead(&self, pool: &str, workspace: &str) -> Result<(), String> {
        let _ = (pool, workspace);
        Err(MOVES_NO_CHAT.to_owned())
    }

    /// **Moves `chats`' work and conversations to the runner of `pool`** (#2048), at their
    /// pause: the chats of one workspace, which move together (#3453). Each clone's work tree is
    /// taken over there once, and each conversation kept in its profile's harness folder there.
    /// Refused, in a sentence, where any of it does not move.
    fn move_in(&self, pool: &str, chats: &[Moving]) -> Result<(), String> {
        let _ = (pool, chats);
        Err(MOVES_NO_CHAT.to_owned())
    }

    /// **Moves `chats`' work and conversations back from the runner of `pool`** (#2048), at
    /// their pause: the chats of one workspace, which move together (#3453). Each clone's work
    /// tree here is taken over onto the runner's once, and each conversation kept in its
    /// profile's harness folder here. Answers what came back that the person is to look at
    /// before a tool here runs it.
    fn move_out(&self, pool: &str, chats: &[Moving]) -> Result<CameBack, String> {
        let _ = (pool, chats);
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

    /// [`SessionHost::watch`], with at most `history` lines of history above the screen, the
    /// newest (#4742): a pane opens on a page and asks for more as the person scrolls up. A
    /// session that pages none draws all of it, and says nothing is older.
    fn watch_within(&self, id: u32, sink: Sink, history: u32) -> Result<Watching, String> {
        let _ = history;
        self.watch(id, sink)
    }

    /// [`Self::watch_within`], for a pane that reports what its terminal has drawn (#4733): each
    /// piece `sink` is handed counts as the program's output only once [`Self::drawn`] says the
    /// pane drew it. A pane that falls behind is then behind where its host can see it, and is
    /// sent a fresh screen rather than a backlog. Where nothing paces a view, it is
    /// [`Self::watch_within`].
    fn watch_drawn(&self, id: u32, sink: Sink, history: u32) -> Result<Watching, String> {
        self.watch_within(id, sink, history)
    }

    /// The pane drawing view `view` of session `id` has drawn `pieces` more of what its sink
    /// was handed, in the order it was handed them (#4733). Nothing where nothing paces a view.
    fn drawn(&self, id: u32, view: u32, pieces: u32) -> Result<(), String> {
        let _ = (id, view, pieces);
        Ok(())
    }

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

    /// **Notes what session `id`'s program has started outside its process group so far**
    /// (#5806), so that ends with the program however the program ends: on its own, or on a
    /// signal from outside, after which no listing finds it. Asked as a chat's harness says it
    /// started a background shell or a monitor; returns at once. Nothing where the host cannot
    /// note them, or the program's start did not ask its strays to end with it.
    fn note_strays(&self, _id: u32) {}

    /// **Holds session `id`'s program still, its whole process group, or lets it go on** (ADR
    /// 0076 §4, #6015): what a chat paused at its budget or in a loop is, until the person
    /// answers. Nothing where the host cannot hold it: the chat is asked about all the same.
    fn hold(&self, _id: u32, _still: bool) {}

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

    /// **Tell `tell` each skill run a runner that keeps the project in its own checkout logs**
    /// (SK-20b, #3084): each start and end of one, as the runner's event log says it, with the
    /// pool whose runner it is, so the window logs it in this machine's plane. Called off the
    /// runtime. Never, where the project's chats run on this machine's disk: their runs are
    /// logged in it already.
    fn when_skill_runs_heard(&self, tell: SkillRunsHeard) {
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

    /// Whether the host session `id`'s program ended with was a pool's runner gone for good
    /// (#4350), whose home its conversation went with. Never, where chats run in the window's own
    /// process.
    fn lost_with_its_runner(&self, id: u32) -> bool {
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

    /// [`Self::left_running`], asked of `runner` (`<pool>/fresh-…`), the chat's own fresh runner
    /// (#4569): a chat found there is kept as that runner's.
    fn left_running_on(&self, number: u32, runner: &str) -> bool {
        let _ = (number, runner);
        false
    }

    /// [`Self::left_running`], of a chat that worked in `workspace` (#2046): where the
    /// project's chats are routed, it is asked of this machine's host and of one pool, the one it
    /// was kept on or else the one the workspace runs its chats on, and no other (#4590).
    fn left_running_in(&self, number: u32, workspace: Option<&str>) -> bool {
        let _ = workspace;
        self.left_running(number)
    }

    /// [`Self::left_running_in`], of a chat whose start chose where it runs (#4536): the pool it
    /// chose is the one asked where no record names one, never the one its workspace names.
    fn left_running_as_started(
        &self,
        number: u32,
        workspace: Option<&str>,
        start: Option<&purlis_core::machine::StartChoice>,
    ) -> bool {
        let _ = start;
        self.left_running_in(number, workspace)
    }

    /// [`Self::left_running`], `None` where this host could not say (#4590): what settles a
    /// record of a chat, which an unanswered question must not.
    fn left_known(&self, number: u32) -> Option<bool> {
        Some(self.left_running(number))
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

    /// **The prompt key chat `number` started with, as its host answered the start** (#3369),
    /// where the window `minted` one for its launch: a host before 1.60 answers none and sets
    /// none in the chat's environment, so its prompts are marked under none. A chat that runs in
    /// the window's own process started with the key minted.
    fn prompt_key_started(
        &self,
        number: u32,
        minted: Option<purlis_core::skillspan::PromptSalt>,
    ) -> Option<purlis_core::skillspan::PromptSalt> {
        let _ = number;
        minted
    }

    /// **The prompt key of chat `number`, left running, as its host kept it** (#3369): what the
    /// window digests what it types into the chat under once it is reattached. `None` where its
    /// host keeps none. Read before it is reattached.
    fn prompt_key_left(&self, number: u32) -> Option<purlis_core::skillspan::PromptSalt> {
        let _ = number;
        None
    }

    /// **The skills chat `number`, left running, was armed with, as its host said** (1.77,
    /// #4218): what a window that reattaches a runner's chat lists for it. `None` where its host
    /// says none: one before 1.77, or a chat the window composed. Read before it is reattached.
    fn armed_skills_left(
        &self,
        number: u32,
    ) -> Option<Vec<purlis_session_protocol::session::ArmedSkill>> {
        let _ = number;
        None
    }

    /// **What chat `number`, left running, started without there, and the runner it is on, as
    /// its host said** (1.83, #6078): what a window that reattaches a runner's chat says on its
    /// tab again, the runner named as [`Composed::runner`] is. `None` where its host says none:
    /// one before 1.83, a chat the window composed, or one that lacks nothing. Read before it is
    /// reattached.
    fn skills_not_there_left(&self, number: u32) -> Option<RunnersLack> {
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

    /// [`Self::close_what_was_not_reattached`], of these chats alone (#4590).
    fn close_left(&self, numbers: &[u32]) {
        let _ = numbers;
    }

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
    /// Whether its turn ended waiting on work of its own (#3878,
    /// `purlis_core::state::Chat::own_work`): it is still at work through it.
    pub own_work: bool,
    /// Whether a turn it begins now may be its own work's wake (#4647,
    /// `purlis_core::state::Chat::wakes_on_its_own_work`): it waits on that work, or the work is
    /// still out though the wait on it ended for the person.
    pub wakes_on_its_own_work: bool,
}

impl Glance {
    /// Whether it is stopped, now, on the prompt it asked mid-turn (#1601): it asked, and it
    /// has not been answered or got past it since, which puts it back to running.
    /// `purlis_core::state::Chat::waits_on_its_prompt`, read off one glance.
    pub fn waits_on_its_prompt(&self) -> bool {
        self.asking && self.state == State::Waiting
    }

    /// **Whether it is in its turn, as what waits for a pause between turns counts it**
    /// (#3887): running, or its turn ended waiting on work of its own (#3878), whose monitors
    /// and background shells a restart, a move or a save would cut off. The board says
    /// `waiting` for that, and it is no pause.
    pub fn in_its_turn(&self) -> bool {
        self.state == State::Running || (self.state == State::Waiting && self.own_work)
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

    /// The person sent chat `session` a line, while a background turn ran over its wait on them
    /// (#4562). Nothing where none did.
    fn replied(&self, session: u32) -> Option<Moved>;

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

    /// A sandbox block's Notice went up for chat `session` while it waits on its own work
    /// (#3878, `purlis_core::state::Board::blocked_while_waiting`): the wait ends, and the
    /// person has the next move. Nothing when no reader would see a difference.
    fn blocked_while_waiting(&self, session: u32) -> Option<Moved>;

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
        /// The sessions whose program ended with a runner gone for good
        /// ([`Pretend::program_ends_with_its_runner`]).
        lost_with_runner: Mutex<Vec<u32>>,
        /// The sessions reconnecting while the link to their runner is down
        /// ([`Pretend::link_down`], #2496).
        reconnecting: Mutex<Vec<u32>>,
        /// The sessions whose program ended with their host ([`Pretend::program_ends_with_its_host`]).
        lost: Mutex<Vec<u32>>,
        /// The chats left running from before this window, and the plan each ran in
        /// ([`Pretend::leaves_running`], #2006).
        left: Mutex<Vec<(u32, Option<purlis_core::sandbox::plan::Plan>)>>,
        /// The plans chats were confined from, in order ([`Pretend::plans_confined`]).
        confined: Mutex<Vec<purlis_core::sandbox::plan::Plan>>,
        /// How each chat left running is sandboxed, as its host recorded it (#2636).
        ran_in: Mutex<Vec<(u32, Option<purlis_session_protocol::session::RunSandbox>)>>,
        /// What each session shows, where a test said ([`Pretend::shows`]): what a view of it
        /// is sent first.
        screens: Mutex<Vec<(u32, String)>>,
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
        /// Why a fetch back fails, while a test says one does: a fresh runner's whole fetch too.
        fetch_fails: Mutex<Option<String>>,
        /// What it says is not home, while a test says so ([`Pretend::not_home_says`]).
        not_home: Mutex<Vec<String>>,
        /// The way to hold its proof until a test lets it go ([`Pretend::holds_its_proof`]).
        proof_held: Mutex<Option<Arc<std::sync::Barrier>>>,
        /// How many proofs it was asked for.
        proofs: AtomicU32,
        /// Why its programs have not ended, while a test says so ([`Pretend::program_still_runs`]).
        still_runs: Mutex<Option<String>>,
        /// The draining runner its link reaches, where a test said ([`Pretend::reaches`]).
        reaches: Mutex<Option<String>>,
        /// Whether it can't say what it has left ([`Pretend::cannot_say_what_it_left`]).
        says_nothing_left: std::sync::atomic::AtomicBool,
        /// How many times the way to this runner was let go of.
        let_go: Mutex<usize>,
        /// Each move asked of it (#2048): `ahead`, `in` or `out`, the pool and the workspace.
        moved: Mutex<Vec<(&'static str, String, String)>>,
        fetching: AtomicU32,
        most_fetching: AtomicU32,
        /// Each chat started by its names, its number, and whether its hearing asks.
        named: Mutex<Vec<(u32, Named, bool)>>,
        /// How it records the run of each chat it composes from now on, where a test said
        /// ([`Pretend::records_composed_runs_as`], #2692): in `seatbelt` otherwise.
        composed_ran: Mutex<Option<Option<purlis_session_protocol::session::RunSandbox>>>,
        /// The skills it says it armed each chat it composes, or one left running, with
        /// ([`Pretend::says_it_armed`], #4218): none said otherwise, as a host before 1.77.
        composed_armed: Mutex<Option<Vec<purlis_session_protocol::session::ArmedSkill>>>,
        /// What it says each chat it composes starts without, and the runner it names
        /// ([`Pretend::says_it_lacks`], #2977): nothing otherwise, as a host before 1.81.
        composed_lacking: Mutex<(
            Option<purlis_session_protocol::session::SkillsNotThere>,
            Option<String>,
        )>,
        /// The chats it says run on a pool's runner, and the pool ([`Pretend::runs_on_pool`]).
        pools: Mutex<Vec<(u32, String)>>,
        /// The chats whose runner records their turns (#5427).
        records_turns: Mutex<Vec<u32>>,
        /// Each undo asked of a runner, by its pool, and what it answers.
        undone: Mutex<Vec<(String, purlis_core::files::TurnKey)>>,
        undo_answers: Mutex<Option<purlis_session_protocol::session::Answer>>,
        /// Every input, resize and view of a terminal, in the order it reached the host
        /// ([`Pretend::terminal`], #4734).
        terminal: Mutex<Vec<String>>,
        /// How long each input, resize and view takes ([`Pretend::terminal_takes`]).
        terminal_taking: Mutex<Duration>,
        /// Wakes every input, resize and view still taking its time ([`Pretend::terminal_answers`]).
        terminal_answered: std::sync::Condvar,
    }

    impl Pretend {
        /// Says chat `id` runs on `pool`'s runner from now on (#2047), as a router does.
        pub fn runs_on_pool(&self, id: u32, pool: &str) {
            lock(&self.seen.pools).push((id, pool.to_owned()));
        }

        /// Says the runner chat `id` runs on records its turns (#5427).
        pub fn records_turns_of_chat(&self, id: u32) {
            lock(&self.seen.records_turns).push(id);
        }

        /// What the runner answers an undo from now on (#5427): `None` refuses it.
        pub fn answers_undo_with(&self, answer: Option<purlis_session_protocol::session::Answer>) {
            *lock(&self.seen.undo_answers) = answer;
        }

        /// Each undo asked of a runner, by its pool (#5427).
        pub fn undone_on_runners(&self) -> Vec<(String, purlis_core::files::TurnKey)> {
            lock(&self.seen.undone).clone()
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

        /// Each fetch back fails with `why` from now on, or none where it is `None`.
        pub fn fetch_fails_with(&self, why: Option<&str>) {
            *lock(&self.seen.fetch_fails) = why.map(str::to_owned);
        }

        /// It can't say what it left running from now on (#4590).
        pub fn cannot_say_what_it_left(&self) {
            self.seen.says_nothing_left.store(true, Ordering::SeqCst);
        }

        /// A fetch of a fresh runner's work fails with `why`.
        pub fn fetch_fails(&self, why: &str) {
            *lock(&self.seen.fetch_fails) = Some(why.to_owned());
        }

        /// Its proof says `missing` from now on.
        pub fn not_home_says(&self, missing: &[&str]) {
            *lock(&self.seen.not_home) = missing.iter().map(|one| (*one).to_owned()).collect();
        }

        /// Its next proofs each wait at `barrier`, twice: as it begins, and before it answers.
        pub fn holds_its_proof(&self, barrier: Arc<std::sync::Barrier>) {
            *lock(&self.seen.proof_held) = Some(barrier);
        }

        /// How many proofs it was asked for.
        pub fn proofs(&self) -> u32 {
            self.seen.proofs.load(Ordering::SeqCst)
        }

        /// A program of the project still runs on its runner from now on, or none.
        pub fn program_still_runs(&self, why: Option<&str>) {
            *lock(&self.seen.still_runs) = why.map(str::to_owned);
        }

        /// Its link reaches the runner `runner` from now on.
        pub fn reaches(&self, runner: &str) {
            *lock(&self.seen.reaches) = Some(runner.to_owned());
        }

        /// How many times the way to this runner was let go of.
        pub fn let_go(&self) -> usize {
            *lock(&self.seen.let_go)
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

        /// Says each chat it composes from now on, and each it left running, was armed with
        /// `skills`, as a host of 1.77 answers (#4218): `None` for one before, which says none.
        pub fn says_it_armed(
            &self,
            skills: Option<Vec<purlis_session_protocol::session::ArmedSkill>>,
        ) {
            *lock(&self.seen.composed_armed) = skills;
        }

        /// Says each chat it composes from now on, and each it left running, starts without
        /// `lacking` there, on the runner `runner` (#2977, #6078).
        pub fn says_it_lacks(
            &self,
            lacking: Option<purlis_session_protocol::session::SkillsNotThere>,
            runner: Option<&str>,
        ) {
            *lock(&self.seen.composed_lacking) = (lacking, runner.map(str::to_owned));
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

        /// The plans the chats it started were confined from, in order.
        pub fn plans_confined(&self) -> Vec<purlis_core::sandbox::plan::Plan> {
            lock(&self.seen.confined).clone()
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

        /// Chat `number`, left running, is no longer in the host: its program ended there after
        /// a window that comes back judged it kept, so it is started rather than reattached.
        pub fn forgets_it_left(&self, number: u32) {
            lock(&self.seen.left).retain(|(one, _)| *one != number);
        }

        /// Browser `browser` runs from before this window: a window that comes back takes it
        /// back with its chat, or ends it (#2025).
        pub fn leaves_a_browser(&self, browser: purlis_session_protocol::session::HeldProgram) {
            lock(&self.seen.browsers_left).push(browser);
        }

        /// Session `number` shows `screen`: what a view of it is sent first, its scrollback
        /// with it, as a real host sends a view's snapshot.
        pub fn shows(&self, number: u32, screen: &str) {
            let mut screens = lock(&self.seen.screens);
            screens.retain(|(one, _)| *one != number);
            screens.push((number, screen.to_owned()));
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

        /// What [`Self::refuses_number`] refused starts from now: its program is back.
        pub fn stops_refusing(&self, number: u32) {
            lock(&self.seen.refusing_numbers).retain(|(one, _)| *one != number);
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

        /// The link to the runner sessions `ids` run on is down: they are reconnecting (#2496),
        /// their programs still running there.
        pub fn link_down(&self, ids: &[u32]) {
            lock(&self.seen.reconnecting).extend_from_slice(ids);
        }

        /// The link to the runner sessions `ids` run on is back: they are reconnecting no more.
        pub fn link_back(&self, ids: &[u32]) {
            lock(&self.seen.reconnecting).retain(|id| !ids.contains(id));
        }

        /// Session `id`'s program ends with a pool's runner gone for good (#4350).
        pub fn program_ends_with_its_runner(&self, id: u32) {
            lock(&self.seen.lost_with_runner).push(id);
            self.program_ends_with_its_host(id);
        }

        /// The runner of session `id`, whose program already ended with its host, goes for good
        /// (#4421).
        pub fn its_runner_goes_since(&self, id: u32) {
            lock(&self.seen.lost_with_runner).push(id);
            self.program_ends(
                id,
                Exit::Signal(crate::hostedsessions::ITS_RUNNER_WENT.to_owned()),
            );
        }

        /// Every input, resize and view of a terminal it was asked for, in the order each
        /// reached it: `typed 7 <bytes>`, `resized 7 100x30`, `watched 7`, `unwatched 7 1`.
        pub fn terminal(&self) -> Vec<String> {
            lock(&self.seen.terminal).clone()
        }

        /// Each input, resize and view takes `long` from now on, as a host far away does.
        pub fn terminal_takes(&self, long: Duration) {
            *lock(&self.seen.terminal_taking) = long;
        }

        /// Every input, resize and view answers now, the ones still taking their time too, and
        /// from now on at once: a test's last word to a host it made slow, so nothing it began
        /// is still running once the test's folders are gone.
        pub fn terminal_answers(&self) {
            *lock(&self.seen.terminal_taking) = Duration::ZERO;
            self.seen.terminal_answered.notify_all();
        }

        /// Takes as long as [`Self::terminal_takes`] says, or until
        /// [`Self::terminal_answers`], then notes `what` reached it.
        fn reached(&self, what: String) {
            let taking = lock(&self.seen.terminal_taking);
            let long = *taking;
            if !long.is_zero() {
                let _ = self
                    .seen
                    .terminal_answered
                    .wait_timeout_while(taking, long, |now| !now.is_zero())
                    .unwrap_or_else(PoisonError::into_inner);
            } else {
                drop(taking);
            }
            lock(&self.seen.terminal).push(what);
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
        fn confine(
            &self,
            plan: &purlis_core::sandbox::plan::Plan,
            hearing: purlis_core::sandbox::plan::Hearing,
        ) -> Result<purlis_core::sandbox::Confinement, String> {
            lock(&self.seen.confined).push(plan.clone());
            plan.start(hearing.here()).map_err(|why| why.to_string())
        }
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
            // Taken once: a second lock in the same expression would wait on the first.
            let (skills_not_there, runner) = lock(&self.seen.composed_lacking).clone();
            Ok(super::Composed {
                number: id,
                confinement: None,
                ran: lock(&self.seen.composed_ran).clone().unwrap_or_else(|| {
                    Some(purlis_session_protocol::session::RunSandbox::By(
                        "seatbelt".to_owned(),
                    ))
                }),
                // The runner mints its own (#3369).
                prompt_key: purlis_core::skillspan::PromptSalt::of_key(&format!("composed-{id}")),
                armed_skills: lock(&self.seen.composed_armed).clone(),
                skills_not_there,
                runner,
            })
        }
        fn move_ahead(&self, pool: &str, workspace: &str) -> Result<(), String> {
            lock(&self.seen.moved).push(("ahead", pool.to_owned(), workspace.to_owned()));
            Ok(())
        }
        fn move_in(&self, pool: &str, chats: &[super::Moving]) -> Result<(), String> {
            for chat in chats {
                lock(&self.seen.moved).push(("in", pool.to_owned(), chat.workspace.clone()));
            }
            Ok(())
        }
        fn move_out(&self, pool: &str, chats: &[super::Moving]) -> Result<super::CameBack, String> {
            for chat in chats {
                lock(&self.seen.moved).push(("out", pool.to_owned(), chat.workspace.clone()));
            }
            Ok(super::CameBack::default())
        }
        fn fetch_back_whole(&self, workspace: &str, runner: &str) -> Vec<String> {
            if let Some(why) = lock(&self.seen.fetch_fails).clone() {
                return vec![why];
            }
            lock(&self.seen.fetched).push((workspace.to_owned(), runner.to_owned()));
            Vec::new()
        }
        fn not_home_of_runner(&self, _runner: &str) -> Vec<String> {
            self.seen.proofs.fetch_add(1, Ordering::SeqCst);
            let held = lock(&self.seen.proof_held).clone();
            if let Some(held) = held {
                held.wait();
                held.wait();
            }
            lock(&self.seen.not_home).clone()
        }
        fn programs_have_ended(&self) -> Result<(), String> {
            lock(&self.seen.still_runs).clone().map_or(Ok(()), Err)
        }
        fn reaches_runner(&self, runner: &str) -> bool {
            lock(&self.seen.reaches).as_deref() == Some(runner)
        }
        fn let_go_of_runner(&self) {
            *lock(&self.seen.let_go) += 1;
        }
        fn fetch_back(&self, workspace: &str, pool: &str) -> Vec<String> {
            let now = self.seen.fetching.fetch_add(1, Ordering::SeqCst) + 1;
            self.seen.most_fetching.fetch_max(now, Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(50));
            lock(&self.seen.fetched).push((workspace.to_owned(), pool.to_owned()));
            self.seen.fetching.fetch_sub(1, Ordering::SeqCst);
            lock(&self.seen.fetch_fails).iter().cloned().collect()
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
        fn records_turns_of(&self, id: u32) -> bool {
            lock(&self.seen.records_turns).contains(&id)
        }
        fn undo_turn_on(
            &self,
            pool: &str,
            _place: &purlis_core::files::turns::Place,
            turn: &purlis_core::files::TurnKey,
        ) -> Result<purlis_session_protocol::session::Answer, String> {
            lock(&self.seen.undone).push((pool.to_owned(), turn.clone()));
            lock(&self.seen.undo_answers)
                .clone()
                .ok_or_else(|| "the runner refused it".to_owned())
        }
        fn pool_for(
            &self,
            _workspace: Option<&str>,
            _start: Option<&purlis_core::machine::StartChoice>,
        ) -> Option<String> {
            lock(&self.seen.pool).clone()
        }
        fn input(&self, id: u32, bytes: &[u8]) -> Result<(), String> {
            self.here(id)?;
            self.reached(format!("typed {id} {}", String::from_utf8_lossy(bytes)));
            lock(&self.seen.typed).push((id, bytes.to_vec()));
            Ok(())
        }
        fn resize(&self, id: u32, size: Size) -> Result<(), String> {
            self.here(id)?;
            self.reached(format!("resized {id} {}x{}", size.columns, size.rows));
            Ok(())
        }
        fn watch(&self, id: u32, mut sink: Sink) -> Result<Watching, String> {
            self.here(id)?;
            self.reached(format!("watched {id}"));
            let shown = lock(&self.seen.screens)
                .iter()
                .find(|(one, _)| *one == id)
                .map(|(_, screen)| screen.clone());
            if let Some(screen) = shown {
                sink(screen.into_bytes());
            }
            Ok(Watching {
                view: 1,
                size: Size {
                    columns: 80,
                    rows: 24,
                },
                older: 0,
            })
        }
        fn unwatch(&self, id: u32, view: u32) -> Result<(), String> {
            self.here(id)?;
            self.reached(format!("unwatched {id} {view}"));
            Ok(())
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
        fn note_strays(&self, id: u32) {
            lock(&self.seen.terminal).push(format!("noted strays {id}"));
        }
        fn hold(&self, id: u32, still: bool) {
            let said = if still { "held" } else { "let go" };
            lock(&self.seen.terminal).push(format!("{said} {id}"));
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
        fn lost_with_its_runner(&self, id: u32) -> bool {
            lock(&self.seen.lost_with_runner).contains(&id)
        }
        fn reconnecting(&self) -> Vec<u32> {
            lock(&self.seen.reconnecting).clone()
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
        fn left_known(&self, number: u32) -> Option<bool> {
            (!self.seen.says_nothing_left.load(Ordering::SeqCst)).then(|| self.left_running(number))
        }
        fn prompt_key_left(&self, number: u32) -> Option<purlis_core::skillspan::PromptSalt> {
            self.left_running(number)
                .then(|| purlis_core::skillspan::PromptSalt::of_key(&format!("left-{number}")))
                .flatten()
        }
        fn armed_skills_left(
            &self,
            number: u32,
        ) -> Option<Vec<purlis_session_protocol::session::ArmedSkill>> {
            self.left_running(number)
                .then(|| lock(&self.seen.composed_armed).clone())
                .flatten()
        }
        fn skills_not_there_left(&self, number: u32) -> Option<super::RunnersLack> {
            let (lacking, runner) = lock(&self.seen.composed_lacking).clone();
            self.left_running(number)
                .then(|| {
                    lacking.map(|skills_not_there| super::RunnersLack {
                        skills_not_there,
                        runner,
                    })
                })
                .flatten()
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
        fn close_left(&self, numbers: &[u32]) {
            lock(&self.seen.left).retain(|(one, _)| !numbers.contains(one));
            lock(&self.seen.closed_left).extend_from_slice(numbers);
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

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What it means to start a chat on a harness profile.
//!
//! **One home, in the core**, because four callers reach it — the picker, a relaunch's
//! reopen, a handoff, and the CLI — and a gate each of them has to remember is a gate one of
//! them will not. The thing it would let through is a command out of a file a chat can
//! write, running with no prompt between the click and the exec.
//!
//! The order is the whole of it, and every step is a refusal that already has its own
//! reasons recorded elsewhere:
//!
//! 1. the profile is one this machine declares — a chat whose profile is gone is skipped by
//!    NAME and never given another (ADR 0022);
//! 2. the persona is one this plane has;
//! 3. the plane's guest layer reaches the directory the chat starts in, or gets written
//!    there — a git root of its own cuts the walk-up off (ADR 0027, closed by M1.x);
//! 4. [`crate::wiring::refusal`] — the kind is startable, the file is not one git would
//!    carry, and the operator approved the command. Nothing is installed: the app arms the
//!    chat itself ([`crate::plugin`]);
//! 5. [`crate::profilestore::sign_in::refusal`] — a profile this machine made is not one its
//!    harness says is signed out (ADR 0095 §3);
//! 6. [`crate::sandbox::for_start`] — where the plane turned the sandbox on, it is compiled for
//!    the harness, or the chat does not start (ADR 0067);
//! 7. only then are the arguments and the environment built.
//!
//! **The harness comes from the DECLARED kind**, never from the program's name.
//! [`crate::harness::Harness::of_command`] cannot tell a shell from a harness charter has
//! not measured, and a profile's command is commonly a wrapper script — ADR 0022 says so in
//! as many words — so inferring from it hands the board `None`, which is the narrowest rule
//! there is, and the chat silently loses the session id that makes it resumable.

use std::path::{Path, PathBuf};

use crate::harness::{Harness, SessionId};
use crate::profiles::{self, Profile};
use crate::reopen::{Fresh, Reopened};

/// What the operator picked, and what the record remembers.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Start {
    /// The profile's name. `None` is a chat that is not on a profile at all — the shell the
    /// app opened before there was a picker, which still reopens as itself.
    pub profile: Option<String>,
    /// The persona this chat adopts, or none for the plane's own default.
    pub persona: Option<String>,
    /// What the operator calls this chat, and what a harness that takes a name is given.
    pub name: String,
    pub cwd: Option<PathBuf>,
    /// The conversation to bring back, where the record holds one.
    pub resume: Option<SessionId>,
    /// Whether THIS chat draws charter's footer in its pane rather than a blank line.
    ///
    /// Default `false`, which is the app as it has always behaved. See [`FOOTER_ENV`] for
    /// what it does and ADR 0029 for why it is a chat's property and not a plane's.
    pub show_footer: bool,
    /// The session record this chat resumes, by its plane-relative path — a chat the Sessions
    /// panel's **Resume** starts (SI-8d). It reaches the chat as
    /// [`crate::sessionrecord::RESUMING_ENV`], which its session-start briefing quotes the
    /// record from. `None` for every other chat, and for a relaunch: the conversation carries
    /// the record from then on.
    pub resuming: Option<String>,
    /// A person's choice, in the window's new-chat picker, to start this one chat without the
    /// sandbox (ADR 0067 §7, ruling V78 a). `None` for every start the window's picker did not
    /// make: a relaunch, a resume, a handoff and the CLI all leave it so, which is what keeps
    /// an opt-out from being inherited by anything.
    pub without_sandbox: Option<crate::sandbox::OptOut>,
    /// The persona grants a handed-off chat holds instead of its own until the person allows
    /// them (#1362, D-1362-5): the app's record of the asking chat, never the request. `None`
    /// for every other start, which holds its own persona's.
    pub held: Option<crate::reopen::HeldGrants>,
    /// What a person let this one chat do past its project's sandbox, from a block's Notice
    /// (#1342): held by the app for that chat alone, and empty for every other start.
    pub grants: crate::sandbox::grant::Grants,
    /// **How this start was made, and by which chat where one asked** (#2071): what the
    /// person's own level is read through ([`crate::sandbox::your_level`]) and what its audit
    /// names. Set by the app from its own record, never from a request. The default,
    /// [`crate::sandbox::Made::Unnamed`], never takes a level of `off`.
    pub made: crate::sandbox::Made,
}

/// **The persona grants the recorded chat `chat` runs with** in the project at `root`: what
/// [`ready`] compiles its sandbox with when it starts again from its record (the start's
/// [`Start::held`] and [`Start::persona`] are the record's). The one answer the start and the
/// dispatch grants' reading of that chat as an asking chat both take (#1362, D-1362-5); a
/// handoff from it is decided as a dispatch is (#1444).
pub fn runs_with(chat: &crate::reopen::Chat, root: &Path) -> Option<String> {
    grants_persona(chat.held.as_ref(), chat.persona.as_deref(), || {
        persona_for_a_new_chat(root)
    })
}

/// **Whose persona grants a chat's sandbox is compiled with** (#1362): the grants a handoff
/// left it holding ([`Start::held`]), else its own persona's, else — for a chat that names none
/// — the persona a new chat adopts by default (`default`), which is the one its briefing
/// takes. Fixed at the start: a persona switched mid-chat changes them at the next start.
pub fn grants_persona(
    held: Option<&crate::reopen::HeldGrants>,
    persona: Option<&str>,
    default: impl FnOnce() -> Option<String>,
) -> Option<String> {
    match held {
        Some(held) => held.persona.clone(),
        None => persona.map(str::to_owned).or_else(default),
    }
}

/// Where a chat is told to draw charter's footer rather than a blank line.
///
/// **What this is not.** It does not bring back a footer Claude Code would otherwise draw:
/// `charter statusline` **is** Claude Code's `statusLine` command, so that line is charter's
/// to fill or to leave empty, and a harness with charter wired in has no footer of its own to
/// fall back on (charter ADR 0019 measured exactly that — a framed session "has no
/// context/cache gauge on any surface"). What this variable chooses is between **charter's
/// footer** and **nothing**.
///
/// Inside the app the answer has been "nothing", because the app's own panels draw the plane
/// (charter ADR 0019, transposed — see `purlis-cli/src/statusline.rs`). ADR 0029 makes that
/// a default rather than a law: a chat started with this variable set to [`FOOTER_SHOW`]
/// draws the footer, and every other chat in the same window is unaffected.
///
/// **Set by charter, never by a profile.** `CHARTER_`-prefixed names are refused in a
/// profile's `env` ([`crate::profiles`], ruling 14), so this cannot be turned on by editing
/// `charter.local.toml` — which is deliberate: it is a choice made in the picker, for one
/// chat, and recorded with that chat.
///
/// **Absence is the default.** Only the exact word [`FOOTER_SHOW`] draws the footer, so a
/// value inherited from somewhere else, or a stale one, reads as "blank" rather than as a
/// surprise. Both the name and the value are constants here, so nothing a chat or a record
/// can write ever reaches the environment charter builds.
pub const FOOTER_ENV: &str = "PURLIS_FOOTER";

/// The one value of [`FOOTER_ENV`] that means "draw it".
pub const FOOTER_SHOW: &str = "show";

/// A chat that may start, with everything the session core and the board need.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ready {
    pub program: String,
    /// The rest of the profile's own `command`, after [`Self::program`] — the words the
    /// operator wrote, which stand before anything charter adds.
    ///
    /// **Its own field because a profile's command is commonly a wrapper** (ADR 0022): for
    /// `["ccs", "work"]` it is `["work"]`, the wrapper's own subcommand, and a flag put in
    /// front of it is a flag handed to the wrapper rather than to the harness (M8.3).
    pub command: Vec<String>,
    /// Charter's own words — the new-session id or the resume — which follow the profile's
    /// command and whatever the app arms the harness with ([`Self::command_line`]). A caller
    /// may append after them (a handoff's positional first message is the last word).
    pub args: Vec<String>,
    /// The profile's environment, charter's own variables, and the persona — sorted, so two
    /// starts of one profile are the same launch.
    pub env: Vec<(String, String)>,
    pub cwd: Option<PathBuf>,
    /// The harness this chat runs, from its profile's declared kind.
    pub harness: Option<Harness>,
    /// The declaration of the harness a project or this machine declares, for a chat on one
    /// (ADR 0073, #967, #968): the one the launch read and, for a project's, the operator
    /// approved. A chat on it is started with what its `env` namespace reaches, its pane's
    /// Shift+Enter is its `[terminal] newline`, and a prompt is typed into it by its
    /// `[terminal]` facts. For a harness purlis ships it is this machine's replacement of its
    /// facts, where `charter.local.toml` has one ([`crate::harness_declaration::Declaration::replaces`]),
    /// and `None` otherwise, as its declaration is [`Harness::declared`]; `None` for a chat on
    /// no harness.
    pub declared: Option<crate::harness_declaration::Declaration>,
    /// The conversation this chat is now under — resumed, or the one charter just chose.
    pub session: Option<SessionId>,
    pub how: Reopened,
    /// The harness's own plugins this chat is handed on or off, by the harness's id: what the
    /// project chose among those installed, and the pins (charter-app#274, ADR 0050). Empty for
    /// a harness whose adapter cannot apply per chat. It reaches the harness through
    /// [`crate::harness::Harness::state_hooks`].
    pub plugins: crate::harness_plugin::Chosen,
    /// The sandbox this chat starts under, compiled for its harness, where the plane turned it
    /// on ([`crate::sandbox::for_start`], ADR 0067). It reaches the harness through
    /// [`crate::harness::Harness::state_hooks_under`].
    pub sandbox: Option<crate::sandbox::Applied>,
    /// Where the project turned the sandbox on and this chat starts without it — a person's
    /// opt-out, or a system with no backend — who chose it and why: what its
    /// `trust.sandbox.off` event records. `None` for a sandboxed chat, and for every chat in a
    /// project that has not turned the sandbox on.
    pub unsandboxed: Option<crate::sandbox::Lifted>,
    /// Why it starts with no sandbox in a project that has not turned it on, where the floor
    /// cannot be applied here (#2071, Q2): what its tab's sandbox mark says. `None` otherwise.
    pub floorless: Option<String>,
    /// What the window says on this chat's tab as it starts, one line each, non-modal: why its
    /// `AGENTS.md` was not written, and every `AGENTS.md` that charter's exclude line hides
    /// and charter did not write (V35, ADR 0085). Empty for nearly every start.
    pub notices: Vec<String>,
    /// The branches whose `AGENTS.md` a notice above names as the operator's and hidden by
    /// charter's exclude line (V35): what the window's **Open file** and **Move aside…** act on
    /// (NO-4). Named, never given as a path, so the window hands the core back a branch it
    /// places again. A hidden file outside every branch of the project is in the sentence only.
    pub agents_md: Vec<TheirAgentsMd>,
    /// How it signs in, from its profile (#2036, ADR 0087 as amended): the kind its run records
    /// as its model source, what it is billed to, and for a key or a cloud the vault reference
    /// of each variable its harness is handed, read where its program is spawned
    /// ([`crate::profilesignin::read`]). References only. `None` where purlis cannot see it.
    pub sign_in: Option<crate::profilesignin::AtStart>,
    /// The chat's first message, where a caller put one last on its line by its harness's route
    /// ([`crate::handoff::first_message_argv`]): what a session host that composes the chat
    /// itself is handed in its place (#2446). `None` from [`ready`].
    pub first_message: Option<String>,
}

/// A branch whose `AGENTS.md` is the operator's and hidden from `git status` by charter's line:
/// a repo's own folder (`piece: None`) or one of its pieces.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TheirAgentsMd {
    pub workspace: String,
    pub repo: String,
    pub piece: Option<String>,
}

impl Ready {
    /// The declaration of the harness this chat runs: the one it started on
    /// ([`Self::declared`], a project's or this machine's, a replacement of a built-in's facts
    /// among them), else the built-in's ([`Harness::declared`]), or none for a chat on no
    /// harness: what a prompt is typed into it by.
    pub fn declaration(&self) -> Option<&crate::harness_declaration::Declaration> {
        self.declared
            .as_ref()
            .or_else(|| self.harness.map(Harness::declared))
    }

    /// Every argument after [`Self::program`], with `armed` — the arguments that arm the
    /// harness for this session alone ([`crate::harness::StateHooks`]) — in their place.
    ///
    /// **The profile's whole command, then `armed`, then charter's own words.** The whole
    /// command first because it is one command the operator wrote, and a wrapper reads its
    /// own words before it hands the rest to the harness: `["ccs", "work"]` starts as
    /// `ccs work --plugin-dir … --settings … --session-id …`, never
    /// `ccs --plugin-dir … work`. Charter's words last, after the armed flags where they have
    /// always stood: Codex resumes through a subcommand (`resume <id>`), and a handoff's
    /// first message is positional.
    ///
    /// For a plain `["claude"]` or `["codex"]` the command is empty and this is the line it
    /// always was: `armed`, then charter's words.
    pub fn command_line(&self, armed: Vec<String>) -> Vec<String> {
        Self::line(self.command.clone(), armed, self.args.clone())
    }

    /// [`Self::command_line`] from its three parts, for a caller that holds them apart — the
    /// app's one place a session is opened, which also opens chats on no profile (an empty
    /// `command`).
    pub fn line(command: Vec<String>, armed: Vec<String>, charters: Vec<String>) -> Vec<String> {
        let mut line = command;
        line.extend(armed);
        line.extend(charters);
        line
    }
}

/// Everything a chat needs to start, or the one sentence saying why it may not.
///
/// Refuses rather than starting something else. Every refusal names the profile and what to
/// do; none of them offers a different profile.
pub fn ready(start: &Start, root: &Path) -> Result<Ready, String> {
    ready_in(start, root, &crate::harness_declaration::read(root))
}

/// [`ready`] with the project's harness declarations read once, by the caller: the profile,
/// the approval gate and the arguments are all taken from `declared`, so the declaration the
/// operator approved is the one whose words run, whatever the file says by the time the
/// program starts (ruling V66).
pub fn ready_in(
    start: &Start,
    root: &Path,
    declared: &crate::harness_declaration::Declarations,
) -> Result<Ready, String> {
    ready_on(
        start,
        root,
        declared,
        &crate::sandbox::Machine::this(),
        &crate::sandbox::backend::installed,
    )
}

/// [`ready_in`] on `machine`, with `has` answering whether a program the sandbox's backend
/// needs is installed: the seam a test names a machine through, so what [`sandbox_ahead`]
/// answers for the same machine can be held to what the start answers.
pub fn ready_on(
    start: &Start,
    root: &Path,
    declared: &crate::harness_declaration::Declarations,
    machine: &crate::sandbox::Machine,
    has: &dyn Fn(&str) -> bool,
) -> Result<Ready, String> {
    // The launch read, which has already asked git whether this plane's `charter.local.toml`
    // would reach every clone of it; every later read of that file in this start takes the
    // same answer (#2179).
    profiles::git_asked_once(|| {
        let launch = profiles::for_launch_in(root, declared);
        ready_given(start, root, declared, machine, has, &launch)
    })
}

/// [`ready_in`] from a launch read the caller already made ([`profiles::for_launch_in`] of
/// the same `declared`): a caller that chose the profile from that read starts the chat from
/// it too, and git is asked once (#1445).
pub fn ready_read(
    start: &Start,
    root: &Path,
    declared: &crate::harness_declaration::Declarations,
    launch: &(profiles::ProfileSet, profiles::IgnoreCheck),
) -> Result<Ready, String> {
    ready_given(
        start,
        root,
        declared,
        &crate::sandbox::Machine::this(),
        &crate::sandbox::backend::installed,
        launch,
    )
}

fn ready_given(
    start: &Start,
    root: &Path,
    declared: &crate::harness_declaration::Declarations,
    machine: &crate::sandbox::Machine,
    has: &dyn Fn(&str) -> bool,
    (set, check): &(profiles::ProfileSet, profiles::IgnoreCheck),
) -> Result<Ready, String> {
    let Some(name) = start.profile.as_deref() else {
        return Err(
            "this chat is not on a harness profile, so there is nothing to start it from — \
             pick one."
                .to_owned(),
        );
    };
    let Some(profile) = set.get(name) else {
        let refused = set
            .refused
            .iter()
            .find(|r| r.name == crate::shown::short(name))
            .map(|r| format!(" It was refused: {}", r.reason))
            .unwrap_or_default();
        let fix = if check.passes() {
            String::new()
        } else {
            format!(" {}", check.fix)
        };
        return Err(format!(
            "profile '{}' is not declared on this machine, so nothing was started — this \
             chat keeps its profile and comes back when that profile is declared again.{}{}",
            crate::shown::short(name),
            refused,
            fix
        ));
    };
    let persona = match start.persona.as_deref() {
        None => None,
        Some(who) => Some(startable_persona(who, root)?),
    };
    // A persona's deny-list holds or its chat does not start (#1451, D-1451-18): asked of the
    // persona the chat runs as, the project's default where it names none, before anything
    // is written or run. The same answer a dispatched chat's profile is chosen by.
    if let Some(who) = persona.clone().or_else(|| persona_for_a_new_chat(root))
        && let Some(why) =
            crate::personaverbs::chatstart::unenforced(root, &who, Harness::of_kind(&profile.kind))
    {
        return Err(why);
    }

    let here = start.cwd.clone().unwrap_or_else(|| root.to_path_buf());
    // The plane's layer: its ask/deny rules have to be in the tree before a chat stands in it.
    //
    // Nothing here runs a command out of a file a chat can write, which is what the consent
    // gate below exists for: this writes charter's own documents into a tree charter owns,
    // and it is idempotent, so doing it for a start that is then refused costs nothing.
    let mut notices = layered_or_refusal(&here, root, persona.as_deref())?;
    let mut floorless = None;
    let hidden = hidden_agents_md_in(&here);
    notices.extend(hidden.said(|p| p.display().to_string()));
    let agents_md = hidden
        .found
        .iter()
        .filter_map(|file| branch_holding(root, file))
        .collect();
    // The gate. Startable kind, ignored file, approved command — one call, so no caller can
    // start a chat past a check another caller makes.
    if let Some(why) = crate::wiring::refusal_in(profile, root, declared) {
        return Err(why);
    }
    // How it signs in, or no chat: a sign-in its harness cannot take, or credentials with no
    // way to it here, starts nothing (#2036).
    let sign_in = crate::profilesignin::at_start(
        profile,
        &crate::profilestore::listed_here(),
        &crate::chatenv::read(root),
    )?;
    // A profile this machine made, whose harness says it is not signed in here, does not
    // start: Sign in is the way out (#2035, ADR 0095 §3). Asked after the gate, so only a
    // program the person approved is run, and only its status words. A profile kept as signing
    // in with a key, a cloud or a local model is not the harness's own sign-in, and is not
    // asked (#2036).
    // The refusal leaves one wait for the project and profile, the Ask the window shows.
    let signs_in_otherwise = sign_in
        .as_ref()
        .is_some_and(|s| s.kind != crate::harness_declaration::SignInKind::Subscription);
    if !signs_in_otherwise
        && let Some(why) = crate::profilestore::sign_in::refusal(
            profile,
            &crate::chatenv::read(root),
            &crate::profilestore::sign_in::ThisMachine,
        )
    {
        crate::profilestore::sign_in::waits_on(root, profile);
        return Err(why);
    }

    let harness = Harness::of_kind(&profile.kind);
    // A kind with no `Harness` is one the project or this machine declares (ADR 0073): the
    // gate above refused every other. It runs at level 1, and its session words are its
    // declaration's. A built-in's facts are this machine's replacement of them, where
    // `charter.local.toml` has one and git would not carry that file (#968): its terminal and
    // its card, never how it is run or armed — its session words and namespace are the
    // built-in's by the declaration's own read, and the sandbox, the adapter and the program
    // below stay the built-in's.
    let declared = match harness {
        Some(harness) => check
            .passes()
            .then(|| declared.replacement(harness).cloned())
            .flatten(),
        None => declared.added().find(|d| d.name == profile.kind).cloned(),
    };
    // The sandbox, or no chat (ADR 0067 §1): asked before anything is resolved or run, so a
    // chat that cannot be confined never reaches the program. A person's opt-out, or a system
    // with no backend, starts it unsandboxed instead, and the tab says so for the chat's whole
    // life (§7).
    let (mut sandbox, unsandboxed) = match harness {
        Some(harness) => match crate::sandbox::decide_granted(
            harness,
            root,
            machine,
            has,
            &crate::sandbox::Person {
                opt_out: start.without_sandbox.as_ref(),
                // The chat's own persona, and the project's for one that names none: never the
                // default persona its grants fall back to (#2071).
                level_as: start.persona.as_deref(),
                made: &start.made,
            },
            // A persona's own hosts reach its chats and no other's (#1362).
            grants_persona(start.held.as_ref(), persona.as_deref(), || {
                persona_for_a_new_chat(root)
            })
            .as_deref(),
            &start.grants,
        )
        // Under a policy that forbids the opt-out, no refusal sends the person to it (#1423).
        .map_err(|refused| refused.said(&crate::sandbox::policy::Locks::of(root)))?
        {
            Some(crate::sandbox::Decided::Sandboxed(applied)) => (Some(applied), None),
            Some(crate::sandbox::Decided::Unsandboxed(lifted)) => {
                notices.push(lifted.notice());
                (None, Some(lifted))
            }
            // The floor could not be applied here (#2071): the chat starts as before, and its
            // tab's sandbox mark says why.
            Some(crate::sandbox::Decided::Floorless(why)) => {
                floorless = Some(why);
                (None, None)
            }
            None => (None, None),
        },
        None => {
            // A declared harness has no adapter, so nothing compiles a sandbox for it yet
            // (SD-2): in a project that turned the sandbox on, it is not started at all. Nor in
            // one whose manifest has gone or cannot be read, which may have turned it on
            // (D-1410e): neither reads as "the sandbox is off".
            let plane = crate::sandbox::Plane::read(root);
            if plane.missing() {
                return Err(crate::sandbox::NotStarted::PlaneMissing.to_string());
            }
            if plane.unreadable() {
                return Err(crate::sandbox::NotStarted::PlaneUnreadable.to_string());
            }
            // Its own sandbox, or the one an administrator's policy requires (D-1423-1).
            let locks = crate::sandbox::policy::Locks::of(root);
            if plane.in_force(&locks).is_some() {
                let its_own = plane.said().policy.is_some();
                return Err(format!(
                    "{}, and purlis cannot sandbox a {} chat yet — it is a declared harness \
                     with no adapter, and a chat that cannot be confined is not started. \
                     Nothing was started.{}",
                    if its_own {
                        "this project turns the sandbox on"
                    } else {
                        "policy requires the sandbox in this project"
                    },
                    crate::shown::short(&profile.kind),
                    locks
                        .opt_out_refused()
                        .filter(|_| !its_own)
                        .map_or_else(String::new, |policy| format!(" {policy}"))
                ));
            }
            (None, None)
        }
    };
    // Which profiles a sandboxed chat, and an unsandboxed one, may start on (#2037). A
    // sandboxed Codex chat on a profile for every chat leaves one wait for the project, the Ask
    // that makes a sandbox-only one.
    let sandbox_only = profiles::sandbox_only(profile);
    if let Some(why) = profile_refusal(
        harness,
        name,
        sandbox_only,
        sandbox.is_some(),
        machine.is_a_runner(),
    ) {
        if sandbox.is_some() && !sandbox_only {
            crate::profilestore::sandbox_only::waits_on(root);
        }
        return Err(why);
    }
    let (added, session, how) = match &declared {
        Some(declared) => declared_arguments(declared, profile, start),
        None => arguments(harness, profile, start),
    };
    let home = profiles::home().unwrap_or_else(|| PathBuf::from("~"));
    // Resolved HERE, in charter's own process, and the absolute path is what the terminal is
    // given (charter-app#134). A bare word handed to a pty is resolved against whatever
    // `PATH` the app itself was started with — which for a Finder-launched `.app` is
    // `/usr/bin:/bin:/usr/sbin:/sbin` and holds no harness.
    let pinned = pinned_program(profile, &home)?;
    let runs_its_pin = pinned.is_some();
    let mut argv = match pinned {
        Some(argv) => argv,
        None => crate::programs::resolve_argv(&profiles::expanded_command(profile, &home))
            .map_err(|gone| format!("{} Nothing was started.", gone.said()))?,
    };
    let mut program = argv.remove(0);
    let standing = Standing::of(&here, root);
    let mut env = environment(
        profile,
        root,
        persona.as_deref(),
        start.show_footer,
        &standing,
    );
    // A runner's pinned version is the one it runs (#3446): Claude Code's own updater, which
    // would fetch another version beside it, is kept off.
    if runs_its_pin && profile.kind == "claude" {
        env.push(("DISABLE_AUTOUPDATER".to_owned(), "1".to_owned()));
    }
    // What the program answered `--version`, where the sandbox's check asked it.
    let mut heard = None;
    // Ruling V87g: a sandbox binds only the harness it was compiled for, and never a program a
    // sandboxed chat could have changed. Resolved once, here: the real file it names is what the
    // check asks and what the chat then runs.
    if let (Some(harness), Some(applied)) = (harness, &mut sandbox) {
        // The profile's sign-in, kept from the agent's commands where the harness runs them
        // apart from itself (#2037, ADR 0095 §5): in the folders the chat's own environment
        // points its harness at.
        applied.on_profile(&crate::sandbox::signin::ProfileFolders {
            sandbox_only: profiles::sandbox_only(profile).then(|| profile.name.clone()),
            ..crate::sandbox::signin::ProfileFolders::of(harness, &env, machine)
        });
        let words: Vec<String> = std::iter::once(program.clone())
            .chain(argv.iter().cloned())
            .collect();
        let mut writable = applied.writable();
        // At the floor a program outside the project is held unwritten instead (#2071, Q4).
        if applied.level() == crate::sandbox::Level::Full {
            writable.extend(crate::sandbox::program::temp_roots(&env));
        }
        let (checked, said) = crate::sandbox::program::checked_answering(
            harness,
            &words,
            applied.root(),
            crate::sandbox::program::Chat {
                cwd: &here,
                writable: &writable,
                env: &env,
            },
            None,
        )
        .map_err(|refused| refused.to_string())?;
        program = checked[0].clone();
        // At the floor, the program it starts is never written by the chat (#2071, Q4).
        applied.hold_programs(&checked);
        // And on Linux, purlis's own program the wrap starts it under, the same (#1040), which
        // a brokered `secret exec` for the chat runs its command in too (#2342).
        applied
            .hold_wrap(crate::sandbox::program::Chat {
                cwd: &here,
                writable: &writable,
                env: &env,
            })
            .map_err(|refused| refused.to_string())?;
        // Claude Code goes through purlis's proxy from the version that takes its ports, and an
        // older one says once, on its tab, that it keeps its own (#1665).
        if let Some(said) = said {
            applied.answered(&said);
            heard = Some(said);
        }
    }
    // A chat runs on its profile's pinned harness version, or does not start (#2038).
    let words: Vec<String> = std::iter::once(program.clone())
        .chain(argv.iter().cloned())
        .collect();
    notices.extend(on_pinned_version(
        profile,
        &words,
        &here,
        &env,
        heard,
        start.resume.is_some(),
    )?);
    // Only a record's path: the briefing reads it through `sessionrecord::open`, which refuses
    // anything else, and a profile cannot set a `CHARTER_` name to forge one.
    if let Some(record) = start
        .resuming
        .as_deref()
        .filter(|path| crate::sessionrecord::locate(path).is_ok())
    {
        env.push((
            crate::sessionrecord::RESUMING_ENV.to_owned(),
            record.to_owned(),
        ));
        env.sort();
    }
    // purlis's pins, and the harness's own skills the project turned off (#3467: no other
    // plugin). A chat in a workspace — by its directory, which is how the window files it under
    // one — also takes that workspace's choices, between Shared and Local (charter-app#282).
    // Asked only for a kind that has an adapter: any other is handed nothing either way.
    let workspace = match &standing {
        Standing::Workspace(name) if crate::harness_plugin::adapter(&profile.kind).is_some() => {
            Some(name.as_str())
        }
        _ => None,
    };
    let plugins = crate::harness_plugin::for_start(
        &profile.kind,
        root,
        workspace,
        &crate::harness_plugin::Env::of(&env),
    );
    // The profile's own words stay together and in front; charter's go after them and after
    // whatever arms the harness. [`Ready::command_line`] is where the order is decided.
    Ok(Ready {
        program,
        command: argv,
        args: added,
        env,
        // Where the checks above stood, never the bare request: a chat asked to start in no
        // directory would otherwise take the app's own, which is `/` for an app opened from
        // the Finder or the Dock.
        cwd: Some(here),
        harness,
        declared,
        session,
        how,
        plugins,
        sandbox,
        unsandboxed,
        floorless,
        notices,
        agents_md,
        sign_in,
        first_message: None,
    })
}

/// **On a runner, a chat on a profile it keeps for its desktop with a pin runs exactly that
/// version** (#3446, [`crate::profilestore::installs`]): its whole command, its program the
/// pinned version installed in the runner's data home (installed now where it is not there yet).
/// `None` for every other profile and machine, whose program is found on `PATH`; `Err`, said by
/// the chat, where the version could not be installed.
fn pinned_program(profile: &Profile, home: &Path) -> Result<Option<Vec<String>>, String> {
    let installed = crate::profilestore::installs::for_a_start(profile)
        .map_err(|why| format!("{why}. Nothing was started."))?;
    Ok(installed.map(|program| with_program(profile, home, &program)))
}

/// `profile`'s whole command, its first word `program` in place of the one it names.
fn with_program(profile: &Profile, home: &Path, program: &Path) -> Vec<String> {
    let mut words = profiles::expanded_command(profile, home);
    let program = program.display().to_string();
    match words.first_mut() {
        Some(first) => *first = program,
        None => words.push(program),
    }
    words
}

/// Whether `profile` is one this machine keeps for its desktop with a pin: a runner's, whose
/// program is the pinned version it installs ([`pinned_program`]).
fn kept_pinned(profile: &Profile) -> bool {
    let records = crate::profilestore::listed_here();
    crate::profilestore::pin::record_of(profile, &records)
        .is_some_and(|r| r.kept_for.is_some() && r.pinned.is_some())
}

/// **A chat runs on its profile's pinned harness version** (#2038,
/// [`crate::profilestore::pin`]): for a profile of this machine's, made or adopted, the version
/// its program `words` answers `--version` with, in the chat's folder `cwd` and environment
/// `env` (`heard`, where the sandbox's check already asked it), is held to the profile's pin,
/// resuming a conversation or not. `Ok` with the sentence the chat's tab says, where there is one;
/// `Err` with the refusal and its way out. Nothing is asked of any other profile, and an adopted
/// profile's program is asked only where the sandbox's check asked it.
fn on_pinned_version(
    profile: &Profile,
    words: &[String],
    cwd: &Path,
    env: &[(String, String)],
    heard: Option<String>,
    resuming: bool,
) -> Result<Option<String>, String> {
    let records = crate::profilestore::listed_here();
    let Some(record) = crate::profilestore::pin::record_of(profile, &records) else {
        return Ok(None);
    };
    // An adopted profile never refuses a chat for its version, so its program is asked nothing
    // more than the sandbox's check already asked: its pin follows the answer heard, where there
    // is one, and a start that heard none costs no extra program run.
    if record.adopted.is_some() && heard.is_none() {
        return Ok(None);
    }
    let Some(config) = crate::machine::config_root() else {
        return Ok(None);
    };
    let said = heard.or_else(|| {
        crate::sandbox::program::version_of(
            words,
            crate::sandbox::program::Chat {
                cwd,
                writable: &[],
                env,
            },
        )
        .ok()
    });
    let installed = said
        .as_deref()
        .and_then(|said| crate::profilestore::pin::version_in(&profile.kind, said));
    crate::profilestore::pin::at_start(
        &config,
        record,
        installed.as_ref(),
        resuming,
        crate::halt::now(),
    )
}

/// Why the picker offers no "Start without the sandbox" for a profile for sandboxed chats only
/// (#2037): the start refuses that chat ([`profile_refusal`]).
pub const SANDBOX_ONLY_LOCK: &str =
    "This profile is for sandboxed chats only, so a chat on it starts only sandboxed.";

/// Why the picker offers no "Start without the sandbox" for a Claude Code chat where the picker
/// itself runs on a runner, a person's own Linux machine paired as one (#4083): the start refuses
/// that chat ([`profile_refusal`]). A desktop's picker for a pool's chat asks its own machine,
/// and the runner's start refuses it there.
pub const RUNNER_LOCK: &str =
    "On a runner, a Claude Code chat starts only inside purlis's sandbox.";

/// Why the picker offers no "Start without the sandbox" for a chat on `profile` on `machine`,
/// where [`profile_refusal`] would refuse it: a profile for sandboxed chats only, or a Claude Code
/// profile on a runner. `None` where the start lets it start without the sandbox.
pub fn without_sandbox_lock(
    profile: &Profile,
    machine: &crate::sandbox::Machine,
) -> Option<&'static str> {
    let sandbox_only = profiles::sandbox_only(profile);
    let harness = Harness::of_kind(&profile.kind);
    profile_refusal(
        harness,
        &profile.name,
        sandbox_only,
        false,
        machine.is_a_runner(),
    )
    .map(|_| {
        if sandbox_only {
            SANDBOX_ONLY_LOCK
        } else {
            RUNNER_LOCK
        }
    })
}

/// **Why a chat may not start on the profile `name`, sandboxed or not** (#2037, ruled
/// 2026-10-15), or `None`: a profile for sandboxed chats only (`sandbox_only`) never starts a
/// chat without the sandbox, and a sandboxed Codex chat starts on no other profile, so no
/// sandboxed chat writes the Codex home an unsandboxed run uses, and no sign-in is copied for
/// one (ADR 0067, D-88q, as amended 2026-10-15).
///
/// **On a runner (`on_a_runner`), a Claude Code profile is for wrapped chats only** (#4083, ruled
/// 2026-10-30): a Claude Code chat there starts inside purlis's wrap or not at all, whether the
/// person opted out, their own level is off, or the floor could not be applied. Its config folder
/// is written by wrapped chats, and so read only by them: no unsandboxed chat loads a transcript,
/// a memory or a trusted folder a wrapped one left ([`crate::sandbox::claude_wrap::prepare`]).
/// A person's own machine is unchanged.
pub fn profile_refusal(
    harness: Option<Harness>,
    name: &str,
    sandbox_only: bool,
    sandboxed: bool,
    on_a_runner: bool,
) -> Option<String> {
    let name = crate::shown::short(name);
    if sandbox_only && !sandboxed {
        return Some(format!(
            "'{name}' is a profile for sandboxed chats only, and this chat would start without \
             the sandbox, so nothing was started. Start it sandboxed, or pick another profile."
        ));
    }
    if harness == Some(Harness::ClaudeCode) && on_a_runner && !sandboxed {
        return Some(format!(
            "this chat would start without the sandbox, and on a runner a Claude Code chat runs \
             only inside purlis's sandbox, so its profile '{name}' is never shared with a chat \
             outside it; nothing was started. Start it sandboxed."
        ));
    }
    if harness == Some(Harness::Codex) && sandboxed && !sandbox_only {
        return Some(format!(
            "this chat runs sandboxed, and a sandboxed Codex chat runs only on a profile for \
             sandboxed chats only, which '{name}' is not, so nothing was started. Make or \
             choose a sandbox-only Codex profile: the Inbox's \"{ask}\" makes one and opens \
             Codex's own sign-in in it; then start the chat on it.",
            ask = crate::profilestore::sandbox_only::ASK
        ));
    }
    None
}

/// Whether the picker may run `profile`'s program to check it: exactly when the start's own
/// gate ([`crate::wiring::refusal`]) would let it start — a startable kind, a `charter.local.toml`
/// git would not carry, and an approved command. One gate for both, so the picker never runs
/// what the start would refuse to.
pub fn may_check_program(profile: &Profile, root: &Path) -> bool {
    crate::wiring::refusal(profile, root).is_none()
}

/// What the new-chat picker says about the sandbox for a chat on `profile` in the project at
/// `root`, before anything starts (ADR 0067 §7, ruling V78 a): every refusal [`ready`] would
/// give, with "Start without the sandbox" beside it; `None` for a kind with no harness.
///
/// **In the start's order, so it gives the start's first refusal.** [`ready`] asks the
/// project's sandbox and this machine (`sandbox::decide`: the policy, a held-back harness, the
/// backend's programs, the compile) before it checks the program, so a machine that cannot
/// apply the sandbox is said first here too, whatever the program is.
///
/// **Asked as the start asks it.** Where the chat would be sandboxed, the program is checked by
/// [`crate::sandbox::program::checked`] with the words, folder and environment [`ready`] uses —
/// the profile's whole command resolved once, the project root, the profile's environment — so
/// the picker and the start cannot disagree (ruling V87g). **Only past the start's gate**: the
/// check asks the program its `--version`, outside any sandbox, so it is not asked of a profile
/// [`may_check_program`] refuses, as the start asks it only past that gate.
pub fn sandbox_ahead(
    profile: &Profile,
    root: &Path,
    machine: &crate::sandbox::Machine,
    has: &dyn Fn(&str) -> bool,
    os_release: &str,
    probe: Option<&crate::sandbox::program::Probe<'_>>,
) -> Option<crate::sandbox::Ahead> {
    AheadOnce::new(root, machine, has, os_release, probe).of(profile)
}

/// [`sandbox_ahead`] for every row of one picker (#2179): **each harness's sandbox is compiled
/// once**, however many profiles of it the picker lists.
///
/// The picker asks for every profile it lists each time a New tab opens, and a compile cost
/// about 200 ms on the macOS runner, so a compile per row made every open pay one per profile.
/// The compile ([`crate::sandbox::decide`]) reads the harness, the project, the machine and
/// what is installed, and nothing of the profile, so every row of one harness is given the one
/// answer.
///
/// **Each profile's program is still checked for itself.** The check runs the program with the
/// profile's own environment, which names the profile (`PURLIS_HARNESS_PROFILE`), so no two
/// rows ask it the same thing.
///
/// Held for one ask only: the files a compile reads are edited by hand and by chats, and an
/// answer kept past the ask would be one nothing invalidates.
pub struct AheadOnce<'a> {
    root: &'a Path,
    machine: &'a crate::sandbox::Machine,
    has: &'a dyn Fn(&str) -> bool,
    os_release: &'a str,
    probe: Option<&'a crate::sandbox::program::Probe<'a>>,
    compiled: std::cell::RefCell<Vec<(Harness, Compiled)>>,
}

/// What [`crate::sandbox::decide`] answered for one harness, as [`AheadOnce`] keeps it.
type Compiled = Result<Option<crate::sandbox::Decided>, crate::sandbox::NotStarted>;

impl<'a> AheadOnce<'a> {
    /// One picker's ask for the project at `root` on `machine`, `has` saying what is installed,
    /// `os_release` the text of `/etc/os-release` and `probe` answering `--version` in place of
    /// running a program (`None` runs it).
    pub fn new(
        root: &'a Path,
        machine: &'a crate::sandbox::Machine,
        has: &'a dyn Fn(&str) -> bool,
        os_release: &'a str,
        probe: Option<&'a crate::sandbox::program::Probe<'a>>,
    ) -> Self {
        Self {
            root,
            machine,
            has,
            os_release,
            probe,
            compiled: std::cell::RefCell::default(),
        }
    }

    /// [`sandbox_ahead`] for `profile`.
    pub fn of(&self, profile: &Profile) -> Option<crate::sandbox::Ahead> {
        let harness = Harness::of_kind(&profile.kind)?;
        let root = self.root;
        let gated_in = may_check_program(profile, root);
        let check = |applied: &crate::sandbox::Applied| -> Result<(), crate::sandbox::NotStarted> {
            if !gated_in {
                return Ok(());
            }
            let home = profiles::home().unwrap_or_else(|| PathBuf::from("~"));
            // A program that is not there is the start's own refusal, not the sandbox's; a
            // runner's pinned version not installed yet is installed by the start (#3446).
            let words = match crate::profilestore::installs::installed_for(profile) {
                Some(program) => with_program(profile, &home, &program),
                None if kept_pinned(profile) => return Ok(()),
                None => {
                    let Ok(words) =
                        crate::programs::resolve_argv(&profiles::expanded_command(profile, &home))
                    else {
                        return Ok(());
                    };
                    words
                }
            };
            let env = environment(profile, root, None, false, &Standing::of(root, root));
            let mut writable = applied.writable();
            // At the floor a program outside the project is held unwritten instead (#2071, Q4).
            if applied.level() == crate::sandbox::Level::Full {
                writable.extend(crate::sandbox::program::temp_roots(&env));
            }
            crate::sandbox::program::checked(
                harness,
                &words,
                applied.root(),
                crate::sandbox::program::Chat {
                    cwd: root,
                    writable: &writable,
                    env: &env,
                },
                self.probe,
            )
            .map(|_| ())
        };
        // A sandboxed Codex chat on a profile for every chat is refused at its start, which
        // leaves the Ask that makes a sandbox-only one (#2037): nothing is offered here that
        // would start it without the sandbox instead.
        Some(crate::sandbox::ahead_decided(
            self.compiled(harness),
            root,
            self.os_release,
            &check,
        ))
    }

    /// What the compile answers for `harness`, compiled on the first ask.
    fn compiled(&self, harness: Harness) -> Compiled {
        if let Some((_, decided)) = self
            .compiled
            .borrow()
            .iter()
            .find(|(one, _)| *one == harness)
        {
            return decided.clone();
        }
        let decided =
            crate::sandbox::decide(harness, self.root, self.machine, self.has, None, None);
        self.compiled.borrow_mut().push((harness, decided.clone()));
        decided
    }
}

/// Where a chat starts, as the plane has it — and so what the chat is told about its
/// workspace (SI-1).
#[derive(Debug, Clone, PartialEq, Eq)]
enum Standing {
    /// In one of the plane's workspaces: its directory, a clone or a piece of it. The same
    /// question the window files the chat's tab by ([`crate::workspaces::Plane::workspace_of`]),
    /// asked once, so the tab and the chat cannot name two workspaces.
    Workspace(String),
    /// At the plane root: in no workspace, on purpose. The plane's own directory and anywhere
    /// else under it that is not one of its workspaces — `docs/`, `.charter/`, `workspaces/`
    /// itself (SI-1b) — which is every chat the window files on the plane root's tab.
    PlaneRoot,
    /// Outside the plane. The chat is told nothing, and its own ladder answers as it always
    /// has.
    Elsewhere,
}

impl Standing {
    fn of(here: &Path, root: &Path) -> Self {
        if let Some(name) = crate::workspaces::Plane::open(root).workspace_of(here) {
            return Self::Workspace(name);
        }
        if crate::active::inside_plane(root, here) {
            Self::PlaneRoot
        } else {
            Self::Elsewhere
        }
    }
}

/// Make sure the plane's guest layer is in the tree this chat would start in — or say why a
/// chat may not start there.
///
/// **Only a worktree of a workspace's clone**, named by path arithmetic
/// ([`crate::worktree::locate`]) and then re-checked as a path this workspace may hold
/// ([`crate::worktree::confine::within_workspace`]). Every other `cwd` is left alone: the
/// plane root and a workspace directory read the plane's own copies by walking up, a clone is
/// the Python's to wire until that port lands, and a directory that is none of those is
/// somewhere charter was pointed at rather than somewhere it owns.
///
/// **Write, then refuse on what did not land.** A worktree charter cut is wired at cut time,
/// so the usual answer here costs one `want` and a digest per file and changes nothing. What
/// this is for is the tree charter did *not* cut — `git worktree add` run by hand, or by
/// another tool — where the repair is this call, not a trip to another binary. A tree whose
/// layer charter cannot finish is refused with the sentence naming what blocked it, because a
/// chat that looks guarded and is not is the failure this refusal exists to prevent.
///
/// **And the chat's `AGENTS.md`** (ADR 0085): a piece is a chat's own worktree, so a chat
/// starting there is given its redacted guidance as that tree's `AGENTS.md`, rendered for
/// `persona` (or the persona the briefing's ladder names, where none is given). It is written
/// here and nowhere else, so a shared clone never gets one. Nothing that happens to it refuses
/// the chat: it is guidance, and [`crate::guest::Guidance`] says what became of it.
///
/// `Ok` carries what the window should say at the start (empty for nearly every chat).
pub fn layered_or_refusal(
    here: &Path,
    root: &Path,
    persona: Option<&str>,
) -> Result<Vec<String>, String> {
    let Some(found) = crate::worktree::locate(root, here) else {
        return Ok(Vec::new());
    };
    let piece = crate::worktree::path_for(root, &found.workspace, &found.repo, &found.piece)
        .and_then(|path| {
            crate::worktree::confine::within_workspace(root, &found.workspace, &path)
                .map_err(Into::into)
        })
        .map_err(|refusal| {
            format!(
                "the worktree this chat would start in is not one purlis may write \
                 ({refusal}), so nothing was started."
            )
        })?;
    let guidance = guidance_for(root, &piece, &found.workspace, persona);
    let (layered, guided) = crate::guest::wire_for_chat(root, &piece, guidance.as_deref());
    if layered.complete() {
        return Ok(guided.and_then(|g| g.notice()).into_iter().collect());
    }
    Err(format!("{} Nothing was started.", layered.refusal(&piece)))
}

/// V35: every `AGENTS.md` that charter's exclude line hides in the repository a chat starting
/// in `here` stands in, and charter did not write. Asked at every start, in a piece or a clone
/// alike, because the line hides the clone's.
fn hidden_agents_md_in(here: &Path) -> crate::guest::HiddenAgentsMd {
    here.ancestors()
        .find(|dir| crate::guest::git_dir(dir).is_some())
        .map(crate::guest::hidden_agents_md)
        .unwrap_or_default()
}

/// The branch of this project whose folder holds `file` at its top: a repo's own folder, or one
/// of its pieces. `None` for a file anywhere else. Path arithmetic on the layout
/// `docs/plane-format.md` records; the window's actions place the branch again by name.
fn branch_holding(root: &Path, file: &Path) -> Option<TheirAgentsMd> {
    let dir = std::fs::canonicalize(file.parent()?).ok()?;
    if let Some(found) = crate::worktree::locate(root, &dir) {
        let piece = crate::worktree::path_for(root, &found.workspace, &found.repo, &found.piece)
            .ok()
            .and_then(|path| std::fs::canonicalize(path).ok())?;
        return (piece == dir).then_some(TheirAgentsMd {
            workspace: found.workspace,
            repo: found.repo,
            piece: Some(found.piece),
        });
    }
    let workspaces = std::fs::canonicalize(root.join("workspaces")).ok()?;
    let parts: Vec<&str> = dir
        .strip_prefix(&workspaces)
        .ok()?
        .iter()
        .map(|part| part.to_str())
        .collect::<Option<_>>()?;
    match parts.as_slice() {
        [ws, repo]
            if crate::contain::workspace_name_ok(ws) && crate::contain::repo_name_ok(repo) =>
        {
            Some(TheirAgentsMd {
                workspace: (*ws).to_owned(),
                repo: (*repo).to_owned(),
                piece: None,
            })
        }
        _ => None,
    }
}

/// What a chat starting in `piece` as `persona` is told in its `AGENTS.md`
/// ([`crate::briefing::agents_md`]): asked as the chat's own briefing will be, with the
/// workspace and persona the start pins.
fn guidance_for(
    root: &Path,
    piece: &Path,
    workspace: &str,
    persona: Option<&str>,
) -> Option<String> {
    let persona = persona.map(str::to_owned);
    let workspace = workspace.to_owned();
    let env = move |name: &str| match name {
        crate::active::PERSONA_ENV => persona.clone(),
        crate::active::WORKSPACE_ENV => Some(workspace.clone()),
        _ => None,
    };
    let now = chrono::Utc::now();
    let note = crate::briefing::piece_announcement(
        root,
        &serde_json::json!({"cwd": piece.display().to_string()}),
        now,
    );
    crate::briefing::agents_md(
        &crate::briefing::Ask {
            root,
            cwd: piece,
            payload: &serde_json::json!({}),
            env: &env,
            now,
        },
        note,
    )
}

/// The arguments charter adds, the conversation the chat is now under, and which of the two
/// happened.
fn arguments(
    harness: Option<Harness>,
    profile: &Profile,
    start: &Start,
) -> (Vec<String>, Option<SessionId>, Reopened) {
    let Some(harness) = harness else {
        return (
            Vec::new(),
            None,
            Reopened::Fresh(Fresh::NoResumeForThisProgram),
        );
    };
    // The operator already named a session in the profile's own command, so charter adds
    // none of its own: two `--resume` on one command line is not a harness anyone has
    // measured, and the one the operator typed is the one they meant.
    if harness.session_named_in(&profile.command[1..]) {
        return (
            Vec::new(),
            None,
            Reopened::Fresh(Fresh::SessionNamedByTheOperator),
        );
    }
    match start.resume.as_ref() {
        Some(id) => match harness.resume_argv(id, &start.name) {
            Some(argv) => (argv, Some(id.clone()), Reopened::Resumed(id.clone())),
            None => (
                Vec::new(),
                None,
                Reopened::Fresh(Fresh::NoResumeForThisProgram),
            ),
        },
        // Where the harness takes an id charter chose, it is given one and that id is kept —
        // otherwise the next quit would have nothing to record and the chat could never be
        // resumed at all.
        None => {
            let chosen = harness.chooses_session_id().then(SessionId::fresh);
            let argv = chosen
                .as_ref()
                .map(|id| harness.new_session_argv(id, &start.name))
                .unwrap_or_default();
            (argv, chosen, Reopened::Fresh(Fresh::NoConversationRecorded))
        }
    }
}

/// [`arguments`] for a harness a project declares: its declaration's templates, by the same
/// rules (ADR 0073 §3).
fn declared_arguments(
    declared: &crate::harness_declaration::Declaration,
    profile: &Profile,
    start: &Start,
) -> (Vec<String>, Option<SessionId>, Reopened) {
    if declared.session_named_in(&profile.command[1..]) {
        return (
            Vec::new(),
            None,
            Reopened::Fresh(Fresh::SessionNamedByTheOperator),
        );
    }
    match start.resume.as_ref() {
        Some(id) => match declared.resume_argv(id, &start.name) {
            Some(argv) => (argv, Some(id.clone()), Reopened::Resumed(id.clone())),
            None => (
                Vec::new(),
                None,
                Reopened::Fresh(Fresh::NoResumeForThisProgram),
            ),
        },
        None => {
            let chosen = (declared.session.chosen_by
                == crate::harness_declaration::ChosenBy::Charter)
                .then(SessionId::fresh);
            let argv = chosen
                .as_ref()
                .map(|id| declared.new_session_argv(id, &start.name))
                .unwrap_or_default();
            (argv, chosen, Reopened::Fresh(Fresh::NoConversationRecorded))
        }
    }
}

/// The profile's environment, plus charter's own — which a profile may not set, because a
/// declaration that tried would have been refused.
///
/// `CHARTER_HARNESS` keeps the REGISTRY's name for the kind, and the profile rides beside it.
/// Hooks compare that variable to `claude-code` for session ids, resume and the working
/// spinner, so a value of `claude-work` would make each of them quietly answer "not Claude
/// Code".
///
/// [`FOOTER_ENV`] is pushed **only** when the chat asked for charter's footer. An
/// absent variable is the default, so nothing has to be unset for a chat that did not ask —
/// and a chat that did cannot be confused with one whose value came from somewhere else,
/// because only this line writes it.
///
/// **Where it started is said too** (SI-1): `$CHARTER_WORKSPACE` for a chat in a workspace,
/// `$CHARTER_PLANE_ROOT_SESSION=1` for one at the plane root, and neither for anywhere else.
/// Without them the window filed a chat under its workspace and the chat's own briefing asked
/// the operator which workspace it was in. A profile may not set either — both are
/// `CHARTER_`-prefixed, which a profile's `env` is refused (ruling 14).
fn environment(
    profile: &Profile,
    root: &Path,
    persona: Option<&str>,
    show_footer: bool,
    standing: &Standing,
) -> Vec<(String, String)> {
    let home = profiles::home().unwrap_or_else(|| PathBuf::from("~"));
    let mut env: Vec<(String, String)> =
        profiles::expanded_env(profile, &home).into_iter().collect();
    env.push(("PURLIS_ROOT".to_owned(), root.display().to_string()));
    env.push((
        crate::hookwire::HARNESS_ENV.to_owned(),
        profile.harness.clone(),
    ));
    env.push(("PURLIS_HARNESS_PROFILE".to_owned(), profile.name.clone()));
    if let Some(who) = persona {
        env.push((crate::active::PERSONA_ENV.to_owned(), who.to_owned()));
    }
    if show_footer {
        env.push((FOOTER_ENV.to_owned(), FOOTER_SHOW.to_owned()));
    }
    match standing {
        Standing::Workspace(name) => {
            env.push((crate::active::WORKSPACE_ENV.to_owned(), name.clone()));
        }
        Standing::PlaneRoot => env.push((
            crate::active::PLANE_ROOT_ENV.to_owned(),
            crate::active::PLANE_ROOT_ON.to_owned(),
        )),
        Standing::Elsewhere => {}
    }
    env.sort();
    env
}

/// `env` with the `PATH` a chat runs under ([`crate::programs::chat_path`]), unless it already
/// names one.
///
/// **A profile that declares `env = { PATH = "…" }` wins, whole** — the escape hatch
/// charter-app#136 was asked to keep. It is the operator's own line in their own machine's
/// file, and a `PATH` they wrote down is one they meant; charter does not append to it behind
/// their back. The app's own hooks still reach the bundled `charter`, because they name it by
/// its absolute path. The one thing put in front of it is a sandboxed persona chat's tools
/// (#1407, [`crate::sandbox::tools`]): the persona's own shims, which the profile's programs
/// still come after.
///
/// **Applied where a chat's program is opened, for every chat** — a profile's and the
/// operator's shell alike — and not inside [`ready`], because the `charter` whose directory
/// goes first is the app's to know ([`crate::harness::Harness::state_hooks`] is handed it the
/// same way) and a shell chat never passes through `ready` at all.
///
/// Sorted afterwards, like everything [`environment`] builds, so two starts of one chat are
/// the same launch.
pub fn with_chat_path(
    mut env: Vec<(String, String)>,
    charter: Option<&Path>,
) -> Vec<(String, String)> {
    // A sandboxed persona chat's tools go first, in front of whichever `PATH` it runs under
    // ([`crate::sandbox::tools::first_on_path`], #1407).
    if env.iter().any(|(name, _)| name == PATH_ENV) {
        return crate::sandbox::tools::first_on_path(env, None);
    }
    if let Some(path) = crate::programs::chat_path(charter) {
        env.push((PATH_ENV.to_owned(), path));
        env.sort();
    }
    crate::sandbox::tools::first_on_path(env, None)
}

/// The variable a chat's program searches for every bare word it runs.
const PATH_ENV: &str = "PATH";

/// The persona a new chat on this plane would adopt — the plane's `[persona] default`, but
/// **only when it is one the plane actually has**.
///
/// `[persona] default` is a line in a committed file and nothing checks that the persona it
/// names exists: it can point at a persona that was deleted, or at `_shared`, which is the
/// store every persona reads rather than a persona anybody adopts. Offering either as the
/// picker's preselection means the operator presses Start and is refused over a persona
/// they never chose — so what is offered is filtered against the list that is drawn beside
/// it, and the two cannot disagree.
///
/// The sidebar still shows `[persona] default` as it is written, because that is a report
/// of what the file says. This is a different question: what would START.
pub fn persona_for_a_new_chat(root: &Path) -> Option<String> {
    let plane = crate::workspaces::Plane::open(root.to_path_buf());
    let wanted = plane.default_persona()?;
    plane
        .personas()
        .ok()?
        .into_iter()
        .find(|have| *have == wanted)
}

/// Whether `who` is a persona a chat on this plane may adopt — [`startable_persona`]'s answer,
/// without its sentence, for a caller choosing whether to ask for one (a Resume, SI-8d).
pub fn persona_for(root: &Path, who: &str) -> bool {
    startable_persona(who, root).is_ok()
}

/// `who`, if it is a persona a chat on this plane may adopt.
///
/// **One source of truth with the list the picker draws** ([`crate::workspaces::Plane::personas`]),
/// and that is the whole point: offering a name the start then refuses is a dialog arguing
/// with itself. Three ways they used to disagree, each found by a review probe:
///
/// - the picker lists a **legacy flat** persona (`personas/<name>.md`) and the start wanted
///   `personas/<name>/persona.md`, so a plane on the old layout offered personas that could
///   not start;
/// - `_shared` is the store every persona READS rather than a persona anybody adopts. The
///   list excludes it by name and the start admitted it, so a caller that is not the picker
///   could point a chat's `CHARTER_PERSONA` at the shared store;
/// - a persona directory that is a **symlink out of the plane** was listed and started, so a
///   committed link decided what a chat adopts and where charter then read it from.
///
/// The containment check is on the entry that is opened, not on `personas/` above it.
fn startable_persona(who: &str, root: &Path) -> Result<String, String> {
    let plane = crate::workspaces::Plane::open(root.to_path_buf());
    let shown = crate::shown::short(who);
    let known = plane.personas().map_err(|e| {
        format!("purlis could not read this plane's personas ({e}), so nothing was started.")
    })?;
    if !known.iter().any(|have| have == who) {
        return Err(format!(
            "no persona '{shown}' on this plane, so nothing was started — pick one of: {}.",
            if known.is_empty() {
                "none declared".to_owned()
            } else {
                known.join(", ")
            }
        ));
    }
    // What the name RESOLVES to, both layouts, gated where it is opened.
    let dir = plane
        .persona(who)
        .map_err(|e| format!("{e}, so nothing was started."))?;
    let entry = if dir.dir().join("persona.md").is_file() {
        dir.dir().join("persona.md")
    } else {
        root.join("personas").join(format!("{who}.md"))
    };
    crate::contain::readable(root, &entry).map_err(|why| {
        format!("persona '{shown}' resolves outside this plane ({why}), so nothing was started.")
    })?;
    Ok(who.to_owned())
}

#[cfg(test)]
mod tests;

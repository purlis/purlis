// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The `charter` command line, in Rust.
//!
//! Only the plane commands M1.1 covers are here, and only as far as the PLANE goes. The
//! recorded scenarios (`tests/fixtures/recorded/`, ADR 0046) prove that: each runs this binary
//! against a copy of a fixture plane and compares the tree it leaves with what the Python
//! charter left.
//!
//! **`-w` and `--persona` are optional, and M2.9 is what made them so.** Every rung of both
//! resolution ladders lives in [`purlis_core::active`]; this file only decides which flag
//! feeds each one. Until then `charter recall` with no flags — how a harness calls it at
//! session start — refused with exit **2**, and every `-w` was a clap usage error.
//!
//! `root` and the hidden `--now` have no Python counterpart at all: `--now` is the test seam
//! charter itself has as `memstore.write(stamp=…)`.

use std::process::ExitCode;

use std::time::Duration;

use clap::{Args, Parser, Subcommand};
use purlis_core::extension::events::Event as ExtensionEvent;
use purlis_core::hookwire::{self, Report, SOCKET_ENV};
use purlis_core::profiles::{self, ProfileSet};
use purlis_core::shown;
use purlis_core::state::Event;
use purlis_core::workspaces::Plane;

#[cfg(unix)]
mod bridge;
mod brokered;
mod browsing;
mod change;
mod curation;
mod dispatch;
mod extcmd;
mod extensions;
mod gitask;
mod githook;
mod guard;
mod handoff;
mod hooks;
#[cfg(unix)]
mod listen;
mod mcp;
mod memory;
mod permission;
mod piece;
mod report;
mod scan;
mod secret;
mod session;
mod shellguard;
mod skill;
mod statusline;
mod theme;
mod themepreview;
mod voice;
mod whereworking;

/// One line of a ported command, in the voice purlis says it in.
///
/// [`purlis_core::repocmd::Say`] carries the mark and the message as a value so a test can
/// read them back; this is the one place they become the coloured line `charter/util.py`
/// prints. `eprintln!("{line}")` would print the mark uncoloured, which is right in a pipe
/// and wrong in a terminal.
pub(crate) fn speak(line: purlis_core::repocmd::Say) {
    use purlis_core::repocmd::Say;
    match line {
        Say::Info(text) => voice::info(text),
        Say::Done(text) => voice::ok(text),
        Say::Warn(text) => voice::warn(text),
        Say::Fail(text) => voice::err(text),
        // `raise SystemExit(message)`, which prints the message as it is — on stderr, where
        // every other mark goes.
        Say::Plain(text) => eprintln!("{text}"),
        // The command's ANSWER, on stdout, for the script reading it.
        Say::Out(text) => println!("{text}"),
    }
}

#[derive(Parser)]
#[command(
    name = "purlis",
    version = purlis_core::adopt::VERSION_LINE,
    about = "purlis: run tons of harness sessions in parallel"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Print the plane root the current directory sits in.
    Root,
    /// Run a server for this chat on PORT: the port a sandboxed chat was allowed to serve on,
    /// which purlis's door on this Mac answers on, while the server listens on the chat's own
    /// socket behind it. In a Claude Code chat on macOS a Node server dials in to the door
    /// instead. For example: `purlis listen 5173 -- npm run dev`.
    Listen {
        /// The port the chat serves on.
        port: u16,
        /// The server's command, after `--`.
        #[arg(last = true, allow_hyphen_values = true, required = true)]
        command: Vec<std::ffi::OsString>,
    },
    /// Run this user's session host on this device (`purlisd`) until it is ended: its socket
    /// under the runtime directory, a fresh credential for each of its clients' scopes, and the
    /// session protocol. The app starts its own; this is the same host for a machine with no
    /// window, such as a runner.
    Serve {
        /// A paired runner's host suspends itself after a day with no link from its desktop, or
        /// after DURATION where one is given (`--suspends-unlinked=12h`: a whole number of `s`,
        /// `m` or `h`, a minute to 30 days, a pool's `suspendAfter`, #3471): it stops serving,
        /// every chat ends, and its container stops with its home kept (#2051). What a Docker
        /// pool's runner is made with; without it, a host never suspends.
        #[arg(
            long,
            num_args = 0..=1,
            require_equals = true,
            value_name = "DURATION",
            value_parser = unlinked_for
        )]
        suspends_unlinked: Option<Option<std::time::Duration>>,
    },
    /// On a runner: join this standard input and output to the session host's link socket,
    /// for the connector the desktop runs (ADR 0078 §2). It holds no key and decides nothing.
    Bridge {
        /// Pair this runner with the desktop whose pin is on standard input, and print this
        /// runner's own pin.
        #[arg(long)]
        pair: bool,
        /// With --pair, on a cluster runner: pair with the link key the runner's pod made as it
        /// started, at FILE, rather than make one (#2056).
        #[arg(long, requires = "pair", value_name = "FILE", hide = true)]
        key_made_at_start: Option<std::path::PathBuf>,
        /// On a cluster runner's pod, as it starts and before anything else runs there: make
        /// the runner's link key at FILE and write its public half to --publish (#2056).
        #[arg(
            long,
            conflicts_with = "pair",
            requires = "publish",
            value_name = "FILE",
            hide = true
        )]
        make_key: Option<std::path::PathBuf>,
        /// With --make-key: where the public half is written, for the cluster to show.
        #[arg(long, requires = "make_key", value_name = "FILE", hide = true)]
        publish: Option<std::path::PathBuf>,
        /// With --make-key: the runner's home on its pod's volume, made from this HOME the
        /// first time; a home that keeps the runner's pairing keeps its key (#2057).
        #[arg(long, requires = "make_key", value_name = "DIR", hide = true)]
        home: Option<std::path::PathBuf>,
    },
    /// Scaffold a fresh control plane here: charter.toml, baseline dirs, .gitignore.
    ///
    /// Additive and idempotent — never touches existing content. A path purlis would write
    /// that is occupied by something it cannot safely touch, or that leads out of the plane,
    /// is named and left alone; everything else is still created, and the exit is 1.
    Init(InitCommand),
    /// Heal control-plane drift: create any missing baseline directory a newer purlis
    /// expects. Idempotent and additive — existing content is never touched.
    Reinit,
    /// Workspaces: their vision, their memory, their todos.
    #[command(subcommand, alias = "ws")]
    Workspace(WorkspaceCommand),

    /// Pieces: worktrees of a workspace's clones — cut, declared done or abandoned, removed.
    #[command(subcommand, alias = "wt")]
    Worktree(piece::WorktreeCommand),

    /// A cross-repo change: one piece of work across several of a workspace's repos — why,
    /// which repos, which branch in each, and which must land first
    /// (workspaces/<ws>/changes/<slug>.json).
    #[command(subcommand)]
    Change(change::ChangeCommand),

    /// Curation actions: what a workspace, a persona or the plane is offered — purlis's own
    /// and each persona's (personas/<name>/curation/<id>.md) — each a chat opened with its
    /// prompt typed and never sent.
    #[command(subcommand)]
    Curation(curation::CurationCommand),

    /// Session records: what a chat leaves behind when it closes through Smart close — a
    /// summary of its session in its workspace's sessions/ (or the plane's, at the plane
    /// root), never the transcript.
    #[command(subcommand)]
    Session(session::SessionCommand),

    /// Harness profiles: which program a chat runs, and with what environment.
    #[command(subcommand)]
    Harness(HarnessCommand),

    /// Themes: every theme available here and where it comes from, the theme check of any one
    /// of them — its errors, its contrast warnings and its hard floors — a chat's theme draft,
    /// and a theme folder imported from another program's theme or exported as a zip.
    #[command(subcommand)]
    Theme(theme::ThemeCommand),

    /// Profiles: one identity of a harness each, and how it signs in.
    #[command(subcommand)]
    Profile(ProfileCommand),

    /// purlis's plugin for a `claude` or `codex` started outside the app: its hooks, the Bash
    /// guard and its skills. The app arms its own chats; this is for the others.
    #[command(subcommand)]
    Plugin(PluginCommand),

    /// Force-prompt and stop-prompting rules for this plane, written in each harness's own
    /// syntax into the file each one reads — Claude Code, opencode and Codex, not only the one
    /// you are running. purlis keeps no list of its own (ADR 0014). Bare, it lists them.
    Guard {
        #[command(subcommand)]
        verb: Option<GuardCommand>,
    },

    // Retired (#2575): the words are still taken and answered with what to use instead
    // (`purlis_core::personaverbs::retired::BROWSER_INSTALL`), before a plane is looked for.
    /// A chat drives a browser with purlis's own browser tools, which the `browser` skill teaches.
    #[command(subcommand, hide = true)]
    Browser(BrowserCommand),

    /// Add what the plane's forges list to inventory/repos.json, then regenerate docs.
    Discover {
        /// Skip per-repo stack detection (faster).
        #[arg(long)]
        no_probe: bool,
        /// Do not regenerate docs afterward.
        #[arg(long)]
        no_docs: bool,
    },

    /// Clone repos on demand into a workspace, each on its own default branch.
    Clone {
        /// Repo name(s) or full path(s) from the inventory.
        repos: Vec<String>,
        /// The workspace to clone into (default: the active one).
        #[arg(short = 'w', long = "workspace")]
        workspace: Option<String>,
        /// Pin the clock the manifest's `updated_at` is stamped with, for tests only.
        #[arg(long, hide = true)]
        now: Option<String>,
    },

    /// Show the plane's workspaces and the cloned repos in the active one.
    ///
    /// The command an operator types when something has already gone wrong, so its output is
    /// a contract: which plane answered, which rung chose the workspace, and for every clone
    /// a branch and one of `clean`, `dirty` or `unknown` — never `clean` for a tree purlis
    /// could not read.
    Status {
        /// The workspace to detail (default: the active one).
        #[arg(short = 'w', long = "workspace")]
        workspace: Option<String>,
        /// Detail every workspace.
        #[arg(long)]
        all: bool,
    },

    /// Regenerate this plane's `docs/topology.md` — and the README's persona roster block.
    ///
    /// Bare `purlis docs` generates, as it did long before it grew subcommands: Makefiles in
    /// the wild call it that way, and making the group require a subcommand would refuse a
    /// command line planes already have.
    ///
    /// `docs list` and `docs show` read purlis's OWN documentation and are not this binary's
    /// — one command describes purlis, the other describes your repos.
    Docs {
        #[command(subcommand)]
        what: Option<DocsCommand>,
    },

    /// Commit and push the control plane's own changes over its forge's HTTPS token.
    ///
    /// It stages EVERYTHING pending in the plane's tree, not only what you changed, and prints
    /// the directory breakdown of what it is about to commit before it commits it.
    Save {
        /// The commit message. Default: `purlis save: N file(s)`.
        message: Option<String>,
        /// Sign the commit. Off by default, so a signer prompt can never hang an agent.
        #[arg(long)]
        sign: bool,
        /// Commit only; do not push.
        #[arg(long)]
        no_push: bool,
        /// First bring in what the remote has, as the app does: fetch the target branch and
        /// fast-forward a clean tree. Refuses, and saves nothing, when the tree has conflicts.
        #[arg(long)]
        pull: bool,
    },

    /// Golden rule 0: check — or `--apply` — token-only git auth on the plane and every clone.
    #[command(name = "git-policy")]
    GitPolicy {
        /// Write the policy. Without it, drift is reported and nothing is changed.
        #[arg(long)]
        apply: bool,
    },

    /// Fetch and fast-forward the clones in a workspace, skipping any that hold work.
    Sync {
        /// The workspace to sync (default: the active one).
        #[arg(short = 'w', long = "workspace")]
        workspace: Option<String>,
        /// Sync every workspace.
        #[arg(long, conflicts_with = "workspace")]
        all: bool,
    },

    /// The one memory gate: search/list across ALL bases (a workspace + a persona's own +
    /// shared), each hit labeled by source.
    Recall(memory::RecallArgs),

    /// Personas: their memory.
    #[command(subcommand)]
    Persona(memory::PersonaCommand),

    /// Skills: the ones a chat in this project gets, why each other one is left out, and every run
    /// of one with its signals (behind the `skill-studio` flag).
    #[command(subcommand)]
    Skill(skill::SkillCommand),

    /// Read/write secrets in a vault; values stay out of the model.
    #[command(subcommand)]
    Secret(secret::SecretCommand),

    /// Manage secret vaults (provider + config + persona).
    #[command(subcommand)]
    Vault(secret::VaultCommand),

    /// Preflight: check the plane, its workspaces, personas and profiles before working.
    ///
    /// Exits non-zero only on a blocker. Every check this purlis does not run yet is still
    /// listed, as a warning saying it was not checked — never as a pass.
    // The checks doctor does not run yet are planned in OB-8, #994.
    Doctor {
        /// Emit machine-readable results.
        #[arg(long)]
        json: bool,
        /// Run as the SessionStart hook does: no harness-profile probe and no git call for
        /// one. Every other check runs.
        #[arg(long)]
        preflight: bool,
        /// Repair first, then report. With no ID: install purlis's plugin for chats started
        /// outside the app (the `plugin-install` fix, what `purlis plugin install` does),
        /// apply the local fixes the doctor's rows offer (`reinit`, `local-ignore`,
        /// `memory-optimize`), and add the plane's default ask rule for `charter report --yes`
        /// when it is missing (what `purlis guard ask` does). `discover` goes over the network,
        /// and `rename-plane` commits the project's files under purlis's names, so each runs
        /// only by name. With an ID (the `fix` a row carries in `--json`): apply that fix
        /// alone. What each changed, or why it was refused, is printed on stderr.
        #[arg(
            long,
            value_name = "ID",
            num_args = 0..=1,
            value_parser = clap::builder::PossibleValuesParser::new(
                purlis_core::doctor::fix::FixId::ALL.map(purlis_core::doctor::fix::FixId::id)
            ),
        )]
        fix: Option<Option<String>>,
        /// The name `--fix git-identity` sets as git's global `user.name`.
        #[arg(long, value_name = "NAME", requires = "fix")]
        name: Option<String>,
        /// The email `--fix git-identity` sets as git's global `user.email`.
        #[arg(long, value_name = "EMAIL", requires = "fix")]
        email: Option<String>,
    },

    /// Move this machine's local state to the purlis names, or put it back with --undo.
    ///
    /// The config home and the session host's folder in it, the data home, the app's log
    /// folder, and in this project and each one this machine remembers, `charter.local.toml`
    /// and `.charter/`. Git is made to ignore the new names through the repository's own
    /// `info/exclude`; no committed file changes. Every move is journalled, and a move that
    /// fails leaves the old name where it was. Nothing moves while a running purlis has a
    /// project open. The app does this at launch; `--undo` stops that until this is run again.
    Migrate {
        /// Put back every move since the last undo, newest first.
        #[arg(long)]
        undo: bool,
    },

    /// Refresh the forge state the CI column is drawn from: each clone's open PR/MR and the
    /// last pipeline on the branch it is actually on.
    ///
    /// **This is the process that holds the forge credential, and it draws nothing.** The
    /// panels read the file it writes and can never fetch, which is deliberate: a fetch on a
    /// render path puts a forge token in the process that draws the window.
    #[command(name = "gl-refresh")]
    GlRefresh {
        /// The workspace to refresh (default: the active one).
        ///
        /// Resolved through [`purlis_core::active`] since M2.9, and resolved BEFORE
        /// `--detach` rather than after: the child is handed the name this process worked
        /// out, so a refresh cannot end up keyed to a different workspace than the one the
        /// operator was standing in.
        #[arg(short = 'w', long = "workspace")]
        workspace: Option<String>,
        /// Return at once and refresh in a process that outlives this one.
        ///
        /// What a hook's `async` used to buy, done by purlis — one harness skips async hooks
        /// outright.
        #[arg(long)]
        detach: bool,
        /// Pin the instant every entry is stamped with, for tests only.
        #[arg(long, hide = true)]
        now: Option<String>,
    },

    /// Propose a change to one of purlis's own configuration files, for the person to apply.
    ///
    /// In a chat the app started: `file` is the project's manifest, a persona's `mcp.json` or
    /// `persona.md`, or a `.claude/skills` file; `--from` is the whole new content, written in
    /// this chat's own folder. The window shows the person the change whole, and only they
    /// apply it. Nothing is written here (#1975).
    Propose {
        /// The configuration file to change.
        file: std::path::PathBuf,
        /// The file in this chat's own folder that holds its whole new content.
        #[arg(long, value_name = "DRAFT")]
        from: std::path::PathBuf,
    },

    /// Claude Code's footer, from the per-turn JSON on stdin.
    ///
    /// Inside the app it prints an empty line and still records the turn's token usage, which
    /// is the only place that record exists (ADR 0019). Everywhere else it draws the frame and
    /// the workspace's identity row, and says in the body which surfaces it does not draw yet.
    Statusline {
        /// Refused in this version: statusline does not repaint yet.
        ///
        /// Drawing one frame and exiting 0 read as a watch that stopped by itself. The flag is
        /// still parsed, so a plane wired for `purlis statusline --watch` meets purlis's
        /// reason, not a usage error.
        #[arg(long)]
        watch: bool,
        /// Refused with --watch: there are no repaints to space out yet.
        ///
        /// Parsed for the same reason `--watch` is, and never used.
        #[allow(dead_code)]
        #[arg(long, default_value = "10")]
        interval: f64,
        /// Pin the instant the footer's ages are measured from, for tests only.
        #[arg(long, hide = true)]
        now: Option<String>,
    },

    /// The kill switch: stop every chat and shell purlis started, in every project and window
    /// the app holds, and start no chat until you re-arm it in the app's title bar.
    ///
    /// It leaves one line in the kill switch's journal. There is no command to re-arm: an agent
    /// can run any command, and letting agents start again is the operator's to decide
    /// (ADR 0071). Agents purlis did not start, headless ones among them, are not reached.
    Stop {
        /// Every agent. Required: it is the only scope there is.
        #[arg(long, required = true)]
        all: bool,
    },

    /// What each version of the app brought: its CHANGELOG.md, newest first.
    News(NewsCommand),

    /// Which channel the app updates from. This command does NOT install anything.
    ///
    /// charter's own `update` moves a Python package with `uv tool install`; this purlis is a
    /// binary inside the app, and the app is what moves it. So `update` here says so, names the
    /// channel (`--channel` moves it), and points at `purlis news` for what a version brought.
    /// `--to` and `--bump` are taken and refused by name rather than rejected as unknown flags,
    /// because an agent that typed one is owed the reason.
    Update {
        /// Install exactly this version. Refused: nothing here installs.
        #[arg(long)]
        to: Option<String>,
        /// Also move this plane's pin. Refused: the pin names a published charter-cp release.
        #[arg(long)]
        bump: bool,
        /// Put the app on this machine on a release channel: `stable` (the default) or `dev`.
        #[arg(long, value_name = "stable|dev")]
        channel: Option<String>,
    },

    /// Which purlis this is, what this control plane pins, and whether they agree.
    ///
    /// Not purlis's three rows, and ADR 0030 is why: two of them — the installed wheel and
    /// the newest one on PyPI — have no subject for a binary that ships inside the app. What
    /// this prints instead is the app's own version and the pin itself (ADR 0045). The EXIT
    /// STATUS is purlis's: 0 with no pin, 0 when the pin is met, 1 on drift.
    Version {
        #[command(subcommand)]
        what: Option<VersionCommand>,
    },

    /// File a bug or a feature request on purlis's own tracker, as you, with purlis's own
    /// sign-in. Shows the draft first and sends nothing without your yes.
    #[command(subcommand)]
    Report(report::ReportCommand),

    /// What a shell tab's shims run in front of a harness started by hand there (ADR 0062):
    /// says it runs outside purlis's session tracking, tells the app, and runs the real one.
    ///
    /// Hidden: nobody types it. The shims purlis writes at every launch are its one caller.
    #[command(name = "shell-guard", hide = true)]
    ShellGuard {
        /// The shim directory, left out of the search and off the harness's `PATH`.
        #[arg(long, value_name = "DIR")]
        shims: std::path::PathBuf,
        /// The harness the operator typed: `claude`, `codex` or `opencode`.
        harness: String,
        /// Its arguments, exactly as typed, after `--`.
        #[arg(last = true, allow_hyphen_values = true)]
        args: Vec<std::ffi::OsString>,
    },

    /// purlis's MCP server, on stdin and stdout: the tools a chat is offered for its
    /// workspace's todos, memory, session records and changes, and a question for the operator
    /// (HP-7).
    ///
    /// Hidden: nobody types it. Each harness starts it for a chat the app armed.
    #[command(name = "mcp", hide = true)]
    Mcp {
        /// A grade's chat (SK-16): the tools that only read, and no other.
        #[arg(long = "graded", hide = true)]
        graded: bool,
    },

    /// Scan what is staged here as a chat's commit would be scanned: secrets and personal data
    /// in the lines it adds, and what the allowlist lets through. Commits nothing.
    Scan {
        /// For each finding, name its rule and the `.charter-scan-allow.toml` entry that would
        /// let it through, for the operator to review and commit.
        #[arg(long)]
        explain: bool,
    },

    /// purlis's part of a chat's git hooks (SQ-16): for `pre-commit` and `pre-merge-commit`, scans what the commit
    /// adds for secrets and personal data, and refuses it on a finding; for `commit-msg`,
    /// stamps the message with the chat's provenance trailers (GL-8).
    ///
    /// Hidden: nobody types it. The hooks the app writes at every launch are its one caller,
    /// and they run the repository's own hook after it.
    #[command(name = "git-hook", hide = true)]
    GitHook {
        /// The hook git is running: `pre-commit`, `pre-merge-commit` or `commit-msg`.
        name: String,
        /// git's own arguments to the hook: `commit-msg`'s message file.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },

    /// Dispatch a task to a persona: a chat of its own in the app, started on a brief you pass
    /// as a quoted heredoc on stdin, which reports back to this chat.
    ///
    /// The new chat runs as the persona you name with --to, or as this chat's own persona,
    /// which needs no grant. It works in this chat's folder, with the sandbox, hosts and vaults
    /// the project gives that persona, and is listed under this chat in the explorer. Its
    /// report reaches this chat as context on its next turn. A persona other than this chat's
    /// own needs a dispatch grant, which the person gives: where there is none the
    /// person is asked on this chat's tab, and the task starts when they allow it.
    ///
    /// It starts on the persona's own harness profile where its definition names one, else on
    /// this chat's; --profile names another of the project's profiles. Where the project
    /// lists profiles for the persona, it starts on one of those or not at all.
    ///
    /// --in says where else it works: `--in workspace:<name>` starts it in another workspace
    /// of the project, and `--in worktree` gives it a new worktree of the repo this chat works
    /// in, on a new branch purlis names. Nothing is merged for it. Where the project has no
    /// sandbox, a worktree task commits on that branch and its report names it. Where the new
    /// chat is sandboxed its own `git commit` is refused in a worktree, and it commits with
    /// `purlis worktree commit`, which the app runs for it.
    ///
    /// `purlis dispatch report --outcome done "<text>"`, from a chat a dispatch started, sends
    /// its one report back.
    ///
    /// The chat that asked can wait for that report (`--wait`, or `purlis dispatch wait
    /// <chat>` later), list its tasks (`purlis dispatch list`) and cancel one (`purlis dispatch
    /// cancel <chat>`). A report nobody waits for reaches the chat as context when it lands.
    ///
    /// While a task works, the chat that asked can send it a follow-up (`purlis dispatch tell
    /// <chat> "<text>"`), and the task can send a progress note (`purlis dispatch note
    /// "<text>"`) or ask a question and wait for the answer (`purlis dispatch ask
    /// "<question>"`, answered with `purlis dispatch answer <chat> "<text>"`).
    #[command(args_conflicts_with_subcommands = true)]
    Dispatch {
        /// The persona the new chat runs as (default: this chat's own).
        #[arg(long)]
        to: Option<String>,
        /// A short name for the task, which the new chat is called and listed under
        /// (`check the queue`). At most 64 characters.
        #[arg(long)]
        name: Option<String>,
        /// One of the project's harness profiles to start the new chat on (default: the
        /// persona's own, else this chat's).
        #[arg(long)]
        profile: Option<String>,
        /// Wait for the task's report and print it as this command's result, instead of
        /// carrying on. A wait that runs out says the task is still running.
        #[arg(long)]
        wait: bool,
        /// With --wait, how long to wait, in seconds (default 100, at most 540).
        #[arg(long, requires = "wait")]
        timeout: Option<u32>,
        /// Where the new chat works: `workspace:<name>` for another workspace, or `worktree`
        /// for a new worktree on its own branch (default: this chat's folder, or a worktree
        /// where the persona's definition says `dispatch-isolation: worktree`). From a
        /// workspace whose folder holds several clones, `worktree:<repo>` names the clone.
        #[arg(long = "in", value_name = "WHERE")]
        place: Option<String>,
        /// A long-running task, which works through many pieces and sends `purlis dispatch
        /// note` as it goes: its notes are its heartbeat, purlis never asks it for a report,
        /// and it ends when it reports done or failed, or is cancelled (#3905).
        #[arg(long)]
        long: bool,
        #[command(subcommand)]
        command: Option<dispatch::DispatchCommand>,
    },

    /// Open a chat in a workspace you name, already working on a brief you pass as a quoted
    /// heredoc on stdin. A handoff is a dispatch: to this chat's own persona it opens at
    /// once, and to another purlis asks you once for the pair, on this chat's tab.
    ///
    /// Work this chat needs an answer from, for its own persona or another, is `purlis
    /// dispatch` (with `--in workspace:<name>` when it must run elsewhere). A handoff is
    /// fire-and-forget: the person's work moves to a chat they will read themselves.
    ///
    /// **Put a flag first:** `purlis handoff --name "<task>" <workspace> <<'BRIEF'`. A chat the
    /// app starts on Claude Code runs that spelling without its harness asking first; a line
    /// that starts with the workspace is still a handoff, and the harness asks about it.
    ///
    /// **The app opens the chat.** Run from a chat the purlis app started, the new chat
    /// opens there as a tab in the workspace you name. With no app running, nothing is
    /// opened and purlis says to open the app. In front of that purlis refuses a handoff
    /// from a helper sub-agent, and one whose brief it cannot read as it is written
    /// (`purlis_core::handoff`).
    ///
    /// `purlis handoff report` is retired: it sends nothing, and names `purlis dispatch
    /// report`, which sends every report. A handoff owes none.
    Handoff {
        /// Where the chat opens — an existing workspace, or a new one with --create. Always
        /// named, this workspace included. `report`, followed by a summary, is refused, naming
        /// `purlis dispatch report`.
        workspace: String,
        /// With `report` as the first word: a report, which is refused (see above).
        summary: Option<String>,
        /// A short name for the task, which the new chat is called instead of its default
        /// (`drop account-console-commons`). At most 64 characters.
        #[arg(long)]
        name: Option<String>,
        /// Kept for chats that learned it: the work is dispatched as a task of this chat, as
        /// `purlis dispatch --in workspace:<name>` does it, and reports back. Use `purlis
        /// dispatch` instead.
        #[arg(long)]
        report: bool,
        /// Make the workspace first (LOCAL, never LIVE). Needs --vision.
        #[arg(long)]
        create: bool,
        /// What the new workspace is for, one line. A workspace with no vision is never
        /// proposed as a handoff target.
        #[arg(long)]
        vision: Option<String>,
        /// The persona the new chat runs as. Without it, this chat's own. Another persona
        /// needs a dispatch grant, which purlis asks you for the first time. To have another
        /// persona do work this chat needs an answer from, use `purlis dispatch --to <persona>`.
        #[arg(long)]
        persona: Option<String>,
        /// Pin the clock the stamp, the todo and the dispatch row are written at, for tests
        /// only.
        #[arg(long, hide = true)]
        now: Option<String>,
    },

    /// Answer a harness hook. Run by a harness's hooks, never by a person.
    ///
    /// It reads the harness's payload on stdin and answers the way that hook is answered.
    ///
    /// `sessionstart` briefs the session — the persona it was started as, that persona's
    /// memory, the workspace gate, the workspace's todos, the plane's other workspaces — as
    /// `additionalContext`, and freezes the persona tool gate's ceiling.
    ///
    /// `userpromptsubmit` keeps the session's heartbeat and adds, as `additionalContext`, the
    /// commitment gate — a prompt asking for work with a real fork in it is told to scout and
    /// ask before building; never on a lookup, never unattended, then quiet for three prompts —
    /// and any report a chat this one handed work to has sent back.
    ///
    /// `pretooluse` is the Bash guard and the persona tool gate, `pretooluse-read` the vault
    /// guard on Read/Grep, `pretooluse-edit` the state-directory guard on Write/Edit, and
    /// `pretooluse-dispatch` the refusal of a sub-agent call named for a persona, which names
    /// the dispatch route.
    ///
    /// `posttooluse` and `-skill` keep the memory nudges, the secret warning on a written
    /// memory, and the skill log; `-dispatch` and `-message` are answered and do nothing since
    /// a persona stopped being a sub-agent. Every event word also tells the app, over its
    /// socket, what the chat is doing.
    ///
    /// Exit 2 — "block" — only when a denial it decided could not be printed. `--list` prints
    /// every word it answers, with the event and tool matcher each is wired to; `--json` makes
    /// that the registry a plugin's `hooks.json` is generated from.
    Hook {
        /// The hook word: `sessionstart`, `pretooluse`, `pretooluse-read`, … (`--list`).
        #[arg(required_unless_present = "list")]
        name: Option<String>,

        /// Print every hook this binary answers, and exit.
        #[arg(long)]
        list: bool,

        /// With `--list`: the registry as JSON — `{"schema": 1, "handlers": [...]}`.
        #[arg(long, requires = "list")]
        json: bool,

        /// The retired Python charter's plugin version. Taken and ignored, and removed with that
        /// plugin.
        ///
        /// The retired plugin puts it on every one of its hook commands (`purlis hook
        /// sessionstart --plugin-version 0.62.1`); refusing the flag would mean refusing every
        /// call such a plugin still makes. purlis's own plugin never passes it.
        #[arg(long)]
        plugin_version: Option<String>,

        /// Pin the clock, for tests only.
        #[arg(long, hide = true)]
        now: Option<String>,
    },
}

#[derive(Args, Clone)]
struct NewsCommand {
    /// One version's section (`0.3.0`, or `Unreleased`), as its release notes print it.
    #[arg(long = "for", value_name = "VERSION")]
    for_version: Option<String>,
    /// Retired with the range view (#352). Taken and refused by name, with what to run instead.
    #[arg(long, hide = true)]
    pending: bool,
    /// Retired with the range view (#352).
    #[arg(long, hide = true)]
    since: Option<String>,
    /// Retired with the range view (#352).
    #[arg(long, hide = true)]
    until: Option<String>,
}

#[derive(Args)]
struct InitCommand {
    /// Forge this project's repos are on. Default: read from the origin of the repo the
    /// project is made for (`--adopt`, `--clone-this-repo`, `--plane-is-this-repo`) when it is
    /// on github.com or gitlab.com; otherwise `init` asks for it and writes nothing.
    #[arg(long, value_parser = purlis_core::scaffold::FORGES)]
    forge: Option<String>,
    /// Group/org/user that owns the repos (default: read from the same origin).
    #[arg(long)]
    owner: Option<String>,
    /// Self-hosted forge host (default: the forge's own public host).
    #[arg(long)]
    host: Option<String>,
    /// Also clone the git repo you are standing in into the first workspace.
    #[arg(long, conflicts_with = "adopt")]
    clone_this_repo: bool,
    /// Adopt an existing repository as this plane's first clone: the plane is made HERE and
    /// that repo is cloned into the first workspace, with nothing written into the repo
    /// itself. ADR 0035's default, where the directory this runs in is the "beside it".
    #[arg(long, value_name = "REPO")]
    adopt: Option<std::path::PathBuf>,
    /// The instant the first workspace's manifest records. Testing only; the wall clock
    /// otherwise.
    #[arg(long, hide = true)]
    now: Option<String>,
    /// Make the git repo you are standing in BE the control plane: write charter.toml,
    /// personas/, inventory/, workspaces/ and purlis's rules into that repo's own tracked
    /// .gitignore. Without it, `init` at the top of a repo writes nothing and says how to put
    /// the plane in a directory of its own (ADR 0035).
    #[arg(long)]
    plane_is_this_repo: bool,
    /// Name of the generic front-door persona to scaffold and declare. Skipped if this
    /// plane already has personas.
    #[arg(long, value_name = "NAME", overrides_with = "no_front_door")]
    front_door: Option<String>,
    /// Scaffold no persona at all; the plane declares no front door.
    #[arg(long, overrides_with = "front_door")]
    no_front_door: bool,
}

/// Each line in the voice `charter/util.py` gives it, through the one module that owns those
/// four glyphs — so `news` and `init` cannot come out looking like two different programs.
fn say_lines(said: &[purlis_core::scaffold::Say]) {
    use purlis_core::scaffold::Say;
    for line in said {
        match line {
            Say::Info(text) => voice::info(text.clone()),
            Say::Ok(text) => voice::ok(text.clone()),
            Say::Warn(text) => voice::warn(text.clone()),
            Say::Err(text) => voice::err(text.clone()),
        }
    }
}

/// What `purlis init` and `reinit` said, and the status they chose.
fn say(outcome: &purlis_core::scaffold::Outcome) -> ExitCode {
    say_lines(&outcome.said);
    ExitCode::from(outcome.code)
}

/// `purlis stop --all`: throws the kill switch the app's title bar throws (OV-1, ADR 0071).
///
/// It needs no plane and no app. The app acts on the switch within seconds when it is running,
/// and a launch that finds it thrown starts nothing, so the stop holds either way. A stop that
/// could not be written is a failure, said as one: no app would hear it.
fn stop_every_agent() -> ExitCode {
    use purlis_core::halt;
    let Some(config) = purlis_core::machine::config_root() else {
        voice::err("Agents were NOT stopped: there is no config home to keep the stop in.");
        return ExitCode::FAILURE;
    };
    match halt::stop(&config, halt::Actor::Cli, halt::now()) {
        Ok(()) => {
            voice::ok(
                "Stopping every chat and shell purlis started, in every project and window: \
                 the app ends them within seconds.",
            );
            voice::info(
                "Nothing starts again until you re-arm it from the purlis app's title bar.",
            );
            ExitCode::SUCCESS
        }
        Err(not_kept) if not_kept.journaled => {
            voice::warn(format!(
                "The stop is kept, but not as it should be: {not_kept}. A running app still \
                 hears it; re-arm from its title bar."
            ));
            ExitCode::FAILURE
        }
        Err(not_kept) => {
            voice::err(format!("Agents were NOT stopped: {not_kept}."));
            voice::info("Use Stop all on the purlis app's title bar instead.");
            ExitCode::FAILURE
        }
    }
}

/// stdout first, then stderr, then the status — the order the two streams are written in
/// matters only for a terminal, and this is the one purlis writes in.
fn emit(report: &purlis_core::news::Report) -> ExitCode {
    use std::io::Write;
    print!("{}", report.out);
    let _ = std::io::stdout().flush();
    say_lines(&report.said);
    ExitCode::from(report.code)
}

/// `--suspends-unlinked`'s DURATION: a pool's `suspendAfter`, spelled and bounded as
/// `runners.json` keeps it (#3471).
fn unlinked_for(text: &str) -> Result<std::time::Duration, String> {
    let after = purlis_core::spelledtime::read(text, purlis_core::spelledtime::UP_TO_HOURS)
        .ok_or_else(|| {
            format!(
                "\"{text}\" is not a duration: write a whole number followed by s, m or h, \
                 such as 24h"
            )
        })?;
    purlis_core::dockerpool::suspend_after(after).map_err(|why| why.0)
}

/// How long `purlis serve` is left without a link before it suspends, as it was asked: never
/// without `--suspends-unlinked`, a day with it bare, and its DURATION where one is given.
fn suspends_after(asked: Option<Option<std::time::Duration>>) -> Option<std::time::Duration> {
    #[cfg(unix)]
    let a_day = purlis_host::UNLINKED_FOR;
    #[cfg(not(unix))]
    let a_day = purlis_core::dockerpool::SUSPEND_AFTER;
    asked.map(|after| after.unwrap_or(a_day))
}

/// `purlis serve`: this user's session host, at the place the window's own host would take, so
/// one finds the other serving and leaves it be.
#[cfg(not(unix))]
fn serve(_suspends_after: Option<std::time::Duration>) -> ExitCode {
    voice::err("the session host runs on macOS and Linux only");
    ExitCode::FAILURE
}

/// `purlis serve`: this user's session host, at the place the window's own host would take, so
/// one finds the other serving and leaves it be.
#[cfg(unix)]
fn serve(suspends_after: Option<std::time::Duration>) -> ExitCode {
    // A service: its standard error is its log, the journal or the container's (#2753), so the
    // core's `info` is said there too, redacted as the app's log is.
    purlis_core::applog::widen_for_a_service();
    #[cfg(debug_assertions)]
    if std::env::var_os(CORE_INFORMS_ON_PURPOSE_ENV).is_some() {
        purlis_core::applog::inform_on_purpose();
        return ExitCode::SUCCESS;
    }
    let Some(config) = purlis_core::machine::config_root() else {
        voice::err("there is no home folder to keep the session host's credentials in");
        return ExitCode::FAILURE;
    };
    // The host keeps its credentials in the config home: it stays where it is while the host
    // runs (`renamelocal::busy::LOCK`, D-RN5-11).
    let _holds = purlis_core::renamelocal::busy::hold_shared(&config);
    // The skills folders a last host laid out and no running program uses, tidied as it
    // starts, as the window tidies them as it starts (#2879).
    purlis_core::skills::tidy_here_later(std::time::Duration::ZERO);
    let place = purlis_host::Place::at(
        purlis_core::hostsocket::here(),
        purlis_core::machine::charterd(&config),
    );
    // A runner's host answers links from the one desktop it is paired with, reading its
    // pairing as each link arrives (`runnerlink`, #2044); unpaired, it links nobody, and it
    // never mints the device id its pairing mints (#2539).
    let answering: purlis_host::Answering = {
        let config = config.clone();
        std::sync::Arc::new(move || purlis_core::runnerlink::answering(&config))
    };
    // The desktop it is paired with takes itself out of its peers as it removes the runner
    // (#2525).
    let pairing = purlis_host::Pairing {
        answering,
        unpairing: {
            let config = config.clone();
            std::sync::Arc::new(move |desktop| purlis_core::runnerlink::unpair(&config, desktop))
        },
        // It keeps that desktop's profiles in its own store, each in a folder of its own with
        // nothing of the desktop's in it, so its chats start on them there (#2049).
        keeping: std::sync::Arc::new(|desktop, profile| {
            use purlis_core::profilestore::{Homes, onrunner, sign_in};
            let homes = Homes::here().ok_or_else(|| {
                "this runner has no data home to keep a profile in, or it is inside a project"
                    .to_owned()
            })?;
            let home = purlis_core::profiles::home()
                .ok_or_else(|| "this runner has no home to keep a profile in".to_owned())?;
            onrunner::kept_for_the_desktop(
                &homes,
                &home,
                &desktop.device,
                profile,
                &sign_in::ThisMachine,
                purlis_core::halt::now(),
            )
        }),
        // And forgets one as the desktop takes it off the pool, its sign-in here with it
        // (#4047).
        forgetting: std::sync::Arc::new(|desktop, name| {
            use purlis_core::profilestore::{Homes, onrunner};
            let homes = Homes::here().ok_or_else(|| {
                "this runner has no data home its profiles are in, or it is inside a project"
                    .to_owned()
            })?;
            let home = purlis_core::profiles::home()
                .ok_or_else(|| "this runner has no home its profiles are in".to_owned())?;
            onrunner::forget(&homes, &home, name, &desktop.device)
        }),
        // And names the ones it keeps for the desktop, which forgets those no longer chosen
        // (#4060).
        listing: std::sync::Arc::new(|desktop| {
            let homes = purlis_core::profilestore::Homes::here().ok_or_else(|| {
                "this runner has no data home its profiles are in, or it is inside a project"
                    .to_owned()
            })?;
            purlis_core::profilestore::onrunner::kept_for(&homes, &desktop.device)
        }),
        // A sign-in's redirect, carried from the desktop's browser to the login listening here
        // (#4044). The login's answer stays here.
        delivering: std::sync::Arc::new(|target| {
            purlis_core::profilestore::sign_in::callback::deliver(target)
        }),
    };
    ExitCode::from(
        u8::try_from(purlis_host::serve_alone_answering(
            &place,
            |place| {
                // The event log of this machine, opened as the host first holds a project.
                let log = purlis_core::hookchannel::EventLog::on_this_machine();
                kept_in_the_data_home(purlis_core::browsers::given_to(
                    purlis_host::Hosting::kept(
                        &place.credentials,
                        Composes(purlis_core::hoststart::Kit::installed_beside_this_binary()),
                        // On a runner only the browser engine may make namespaces (D-2050-25).
                        serve_terminals(),
                        // Each held project's hooks, heard as the window's own host hears them
                        // (#2447).
                        purlis_core::hookchannel::HookChannels::keeping(log.clone()),
                        Confines,
                    )
                    // What a chat's end leaves, kept as the window's own host keeps it (#2494),
                    // and the skills folders no running program uses tidied after each end, as
                    // the window tidies them after a chat closes (#2879).
                    .recording_with(purlis_core::skills::TidiesAfterEachEnd::here(log))
                    // `purlis stop --all` is heard with no window there (#2190).
                    .heeding(purlis_core::halt::OnDisk(config.clone()))
                    // The projects it holds for its desktop, each in `<data>/projects/<id>`,
                    // and the code the desktop moves in them over the link (#2046).
                    .moving_code_with(purlis_core::codelink::Moves),
                ))
            },
            pairing,
            suspends_after,
        ))
        .unwrap_or(1),
    )
}

/// `purlis bridge`: a runner's pipe to its host's link socket, or its pairing.
#[cfg(unix)]
fn bridge(asked: bridge::Asked) -> ExitCode {
    match bridge::run(asked) {
        Ok(code) => ExitCode::from(code),
        Err(why) => {
            voice::err(why.to_string());
            ExitCode::FAILURE
        }
    }
}

/// `purlis bridge`: a runner's pipe, on macOS and Linux only.
#[cfg(not(unix))]
fn bridge() -> ExitCode {
    voice::err("a runner's bridge runs on macOS and Linux only");
    ExitCode::FAILURE
}

/// **The terminals `purlis serve` starts each program in** (#3298, D-2050-25): on Linux, each
/// program is started through `purlis _no-namespaces`, so it, and all it starts, can make no
/// namespace. A Docker pool's runner lets the browser engine's sandbox make user, process and
/// network namespaces (ADR 0089 as amended 2026-10-28), and the engine, which the host starts
/// itself and never in a terminal, is the only one that may: a chat, sandboxed or not, a shell
/// and a harness's own sandbox may not. A harness's own sandbox that needs them does not start,
/// and its chat is refused, never run without it (Claude Code's is compiled so).
#[cfg(unix)]
fn serve_terminals() -> impl purlis_host::Run {
    #[cfg(target_os = "linux")]
    return WithoutNamespaces(purlis_host::Terminals);
    #[cfg(not(target_os = "linux"))]
    return purlis_host::Terminals;
}

/// [`purlis_host::Terminals`], each program started through `purlis _no-namespaces`.
#[cfg(target_os = "linux")]
struct WithoutNamespaces(purlis_host::Terminals);

#[cfg(target_os = "linux")]
impl purlis_host::Run for WithoutNamespaces {
    fn run(
        &self,
        spec: purlis_core::session::Spec,
    ) -> Result<std::sync::Arc<dyn purlis_host::Running>, String> {
        self.0.run(without_namespaces(spec)?)
    }

    fn end_what_was_left(&self, left: &purlis_host::Started) {
        self.0.end_what_was_left(left);
    }
}

/// `spec`, its program started by this `purlis` through `_no-namespaces`: the same process, its
/// arguments and environment as they were.
#[cfg(target_os = "linux")]
fn without_namespaces(
    mut spec: purlis_core::session::Spec,
) -> Result<purlis_core::session::Spec, String> {
    let me = std::env::current_exe().map_err(|why| {
        format!("purlis could not name its own program to start this with: {why}")
    })?;
    let program = std::mem::take(&mut spec.program);
    let args = std::mem::take(&mut spec.args);
    spec.args = purlis_linux_sandbox::no_namespaces_line(program, args);
    spec.program = me.into_os_string();
    Ok(spec)
}

/// **How `purlis serve` composes each start** (#2174, #2171): a launch by its words, or, for a
/// start with no launch, the launch the window would compose, from the project's own files
/// ([`purlis_core::hoststart::spec`]), armed with what is installed beside this `purlis`
/// (#2447, [`purlis_core::hoststart::Kit::installed_beside_this_binary`]).
#[cfg(unix)]
struct Composes(purlis_core::hoststart::Kit);

#[cfg(unix)]
impl purlis_host::Compose for Composes {
    fn compose(
        &self,
        starting: &purlis_host::ToCompose<'_>,
    ) -> Result<purlis_core::session::Spec, String> {
        let beside = starting.beside;
        purlis_core::hoststart::spec(
            &purlis_core::hoststart::Starting {
                root: starting.project_path,
                number: starting.number,
                start: starting.start,
                reporting: starting.reporting,
                prompt_key: starting.prompt_key,
                started_by: starting.started_by.word(),
            },
            starting.size,
            &self.0,
            &(
                |plan: &purlis_host::protocol::session::ConfinePlan| beside.confine(plan),
                |entry| beside.record(entry),
                // A chat's router extension, asked at its start with this program's own
                // executor (SK-15b).
                std::sync::Arc::new(
                    purlis_core::skills::router::OnThisMachine::with_this_programs(
                        starting.project_path,
                    ),
                ) as std::sync::Arc<dyn purlis_core::skills::router::AskRouter>,
                // The chat's confinement as it is now, which its tool's runs here are held to
                // (#3407).
                |confinement: &str| beside.reach_of(confinement),
            ),
        )
    }

    /// How the run of `spec` is sandboxed, read by the core from what is started (#2636).
    fn sandbox_of(
        &self,
        spec: &purlis_core::session::Spec,
        confined: bool,
    ) -> purlis_host::protocol::session::RunSandbox {
        purlis_core::sandbox::ran::applied_to(
            &spec.program,
            &spec.args,
            confined,
            purlis_core::sandbox::Os::this(),
        )
    }
}

/// `purlis serve`'s host, keeping the projects it holds for its desktop in `<data>/projects`
/// (#2046), where there is a data home.
#[cfg(unix)]
fn kept_in_the_data_home(hosting: purlis_host::Hosting) -> purlis_host::Hosting {
    let Some(data) = purlis_core::datahome::root() else {
        return hosting;
    };
    // A data home inside a work tree or a project keeps nothing (ADR 0075 §6).
    if let Some(why) = purlis_core::datahome::refusal(&data) {
        voice::err(format!("this host keeps no projects of its own: {why}"));
        return hosting;
    }
    hosting.keeping_projects_in(data.join(purlis_core::datahome::PROJECTS))
}

/// **How `purlis serve` starts a chat's confinement** (#2217): the core's start of the plan it
/// is handed ([`purlis_core::sandbox::hosted::start`]), as the window's own host starts one, so
/// a chat on a runner is confined by the runner's own proxy.
#[cfg(unix)]
struct Confines;

#[cfg(unix)]
impl purlis_host::Confine for Confines {
    fn fits(&self, confining: &purlis_host::ToConfine<'_>) -> Result<(), String> {
        purlis_core::sandbox::hosted::fits_here(confining.project_path, &confining.confine.plan)
    }

    fn gives_now(&self, project_path: &std::path::Path, port: u16) -> Result<(), String> {
        purlis_core::sandbox::hosted::gives_here(project_path, port)
    }

    fn confine(
        &self,
        confining: &purlis_host::ToConfine<'_>,
        tell: purlis_host::Tell,
        askable: purlis_host::Askable,
    ) -> Result<Box<dyn purlis_host::Confining>, String> {
        purlis_core::sandbox::hosted::start(
            confining.project_path,
            confining.confine,
            match confining.planned {
                purlis_host::Planned::Here => purlis_core::sandbox::hosted::Planned::Here,
                purlis_host::Planned::Composed => purlis_core::sandbox::hosted::Planned::Composed,
                purlis_host::Planned::Elsewhere => purlis_core::sandbox::hosted::Planned::Elsewhere,
            },
            confining.alone,
            tell,
            askable,
        )
        .map(|hosted| Box::new(HeldByServe(hosted)) as Box<dyn purlis_host::Confining>)
    }
}

/// A confinement `purlis serve` holds: its proxy and folders end as this is dropped.
#[cfg(unix)]
struct HeldByServe(purlis_core::sandbox::hosted::Hosted);

#[cfg(unix)]
impl purlis_host::Confining for HeldByServe {
    fn confined(&self) -> purlis_host::protocol::session::Confined {
        self.0.confined()
    }

    fn change(
        &self,
        change: &purlis_host::protocol::session::ReachChange,
    ) -> Result<Vec<String>, String> {
        self.0.change(change)
    }

    fn now(&self) -> purlis_host::protocol::session::ReachNow {
        self.0.now()
    }

    fn give_back(&self, port: u16) -> Result<(), String> {
        self.0.give_back_port(port)
    }
}

/// What a command may do on a project that is read-only to this purlis (FR-24).
enum ReadOnly {
    /// It reports on the project or moves purlis itself forward, and says what is wrong itself.
    Runs,
    /// It only reads, so it runs, after one line saying the project is read-only (V5: a
    /// read-only project "opens read-only: it reads").
    RunsAndSays,
    /// It could write the project.
    Refused,
}

/// An explicit list of the commands that run on a read-only project. Everything else is
/// refused, so a new command is refused there until somebody shows it only reads.
fn read_only_standing(command: &Command) -> ReadOnly {
    match command {
        Command::Doctor { fix: None, .. }
        // It moves this machine's folders, and asks each project whether it may move its
        // state itself (`renamelocal`), so a read-only project here refuses nothing else.
        | Command::Migrate { .. }
        // This machine's profiles, in its data home: nothing of a project's.
        | Command::Profile(
            ProfileCommand::Create { .. }
            | ProfileCommand::Import { .. }
            | ProfileCommand::Remove { .. }
            | ProfileCommand::Authenticate { .. }
            | ProfileCommand::SignOut { .. },
        )
        | Command::Update { .. }
        | Command::Version { .. }
        | Command::News(_)
        // This machine's themes and the theme folders of the plane and its workspaces: read
        // through the theme engine, and nothing is written.
        | Command::Theme(_)
        | Command::Root => ReadOnly::Runs,
        // Each one is here because a test runs it on a lived-in read-only project, sees it
        // print what it read, and finds every file unchanged
        // (`tests/it/a_project_charter_cannot_write_is_read_only.rs`, #833).
        Command::Status { .. }
        | Command::Recall(_)
        | Command::Statusline { .. }
        | Command::Workspace(
            WorkspaceCommand::List | WorkspaceCommand::Current | WorkspaceCommand::Recall { .. },
        )
        | Command::Persona(memory::PersonaCommand::List)
        | Command::Skill(
            skill::SkillCommand::List { .. }
            | skill::SkillCommand::Show { .. }
            | skill::SkillCommand::Restore { name: None, .. },
        )
        | Command::Change(change::ChangeCommand::List { .. })
        | Command::Session(session::SessionCommand::List { .. })
        | Command::Harness(HarnessCommand::List | HarnessCommand::Show { .. })
        | Command::Guard {
            verb: None | Some(GuardCommand::List),
        }
        // Its read tools answer and every tool that writes is refused at the call (#1051):
        // `tests/it/the_mcp_server_on_a_read_only_project_changes_nothing.rs` calls every tool.
        | Command::Mcp { .. } => ReadOnly::RunsAndSays,
        _ => ReadOnly::Refused,
    }
}

/// Why the project this command stands in is read-only to this purlis, or `None` (FR-24).
fn read_only_project() -> Option<String> {
    let place = place().ok()?;
    if !place.is_plane {
        return None;
    }
    match purlis_core::compat::read(&place.root) {
        purlis_core::compat::Compat::Writable => None,
        purlis_core::compat::Compat::ReadOnly(why) => Some(why.to_string()),
    }
}

/// Where `init` and `reinit` act: `charter/root.py:find_root_or_cwd`.
fn place() -> Result<purlis_core::plane::Place, String> {
    let cwd =
        std::env::current_dir().map_err(|e| format!("cannot read the current directory: {e}"))?;
    Ok(purlis_core::plane::place(&cwd))
}

#[derive(Subcommand)]
enum BrowserCommand {
    // Retired (#2575): every word after it, `--help` included, is taken and not read, so it
    // always answers with what to use instead.
    /// A chat drives a browser with purlis's own browser tools, which the `browser` skill teaches.
    #[command(hide = true, disable_help_flag = true)]
    Install {
        /// What the retired command was given. Taken and not read.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true, hide = true)]
        given: Vec<String>,
    },
}

#[derive(Subcommand)]
enum GuardCommand {
    /// Always prompt before this command runs.
    Ask {
        /// e.g. 'terraform apply *' — wrapped as Bash(...) unless it already names a tool.
        pattern: String,
        /// Write this machine's own file (`.claude/settings.local.json`, not committed)
        /// instead of the plane's committed settings: the rule is yours alone.
        #[arg(long)]
        local: bool,
    },
    /// Stop the harness prompting for a command pattern (writes the harness's own allow rule).
    /// It reaches a chat at the plane root only.
    Allow {
        /// e.g. 'git status *'. A bare command is wrapped as a Bash rule.
        pattern: String,
        /// Write this machine's own file instead of the plane's committed settings.
        #[arg(long)]
        local: bool,
    },
    /// Retired: a handoff's consent is the dispatch grant, so this writes nothing and says
    /// what to run to have your harness ask as well.
    Handoff,
    /// Always prompt before `charter report … --yes` files an issue: the rule `purlis init`
    /// writes, put back (ADR 0059).
    Report,
    /// Show this plane's ask and allow rules, by the file each lives in.
    List,
}

#[derive(Subcommand)]
enum PluginCommand {
    /// Register purlis's plugin with each harness on this machine, so a chat started in a
    /// terminal runs purlis's hooks and guard. Prints each change; running it again changes
    /// nothing that is already so.
    Install {
        /// Only this harness (`claude`, `codex` or `opencode`); repeat for more. Default: each
        /// one whose config folder exists.
        #[arg(long, value_parser = purlis_core::plugin_install::HARNESSES)]
        harness: Vec<String>,
        /// Print what would change, and write nothing.
        #[arg(long)]
        dry_run: bool,
        /// The plugin folder to install from, instead of the one the app ships beside this
        /// binary.
        #[arg(long, hide = true)]
        plugin_from: Option<std::path::PathBuf>,
    },
    /// Take back what `install` wrote, and nothing else.
    Uninstall {
        /// Only this harness (`claude`, `codex` or `opencode`); repeat for more.
        #[arg(long, value_parser = purlis_core::plugin_install::HARNESSES)]
        harness: Vec<String>,
        /// Print what would change, and write nothing.
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Subcommand)]
enum HarnessCommand {
    /// Every profile purlis read, the file it came from, and why any was refused.
    List,
    /// A harness's declaration as purlis read it: a built-in, or the project's
    /// `harnesses/<name>.toml` (ADR 0073).
    Show {
        /// The harness's declared name, as a profile's `kind` names it.
        name: String,
    },
}

#[derive(Subcommand)]
enum ProfileCommand {
    /// Every profile purlis read, how each signs in, the file it came from, and why any was
    /// refused.
    List,
    /// Make a profile of this machine's, available in every project: a folder of its own in
    /// purlis's data home, with this machine's harness settings files copied in as they are.
    /// Sign it in with `purlis profile sign-in`.
    Create {
        /// The profile's name: letters, digits, '-' and '_'.
        name: String,
        /// The harness it runs: claude, codex or opencode.
        #[arg(long)]
        harness: String,
        /// Make it for sandboxed chats only, with nothing imported: only a sandboxed chat starts
        /// on it, and a sandboxed Codex chat starts on no other. Sign it in through the
        /// harness's own sign-in.
        #[arg(long)]
        sandbox_only: bool,
    },
    /// Copy this machine's harness settings files into a profile again, as they are. Its
    /// sign-in and conversations stay.
    Import {
        /// The profile's name.
        name: String,
    },
    /// Remove a profile of this machine's. Says what goes, and removes it only with --yes. An
    /// adopted harness folder is never deleted: purlis only forgets the profile.
    Remove {
        /// The profile's name.
        name: String,
        /// Remove it, having read what goes.
        #[arg(long)]
        yes: bool,
    },
    /// Sign a profile of this machine's in on this device, through its harness's own sign-in,
    /// run here in this terminal with the profile's folder set. The harness keeps what it
    /// signs in with in that folder; purlis reads none of it. With --pool, on that pool's
    /// runner instead: purlis follows the sign-in there, as the window does, and says each step
    /// here.
    #[command(name = "sign-in")]
    Authenticate {
        /// The profile's name.
        name: String,
        /// Only say whether its harness says it is signed in on this device.
        #[arg(long)]
        check: bool,
        /// For an opencode profile, the plan it signs in to: openai (ChatGPT) or
        /// github-copilot. A Claude plan is never signed in through opencode.
        #[arg(long)]
        provider: Option<String>,
        /// Sign it in on this pool's runner: it opens the sign-in page in your browser, reads
        /// the code it shows, and exits non-zero if it did not sign in.
        #[arg(long, value_name = "POOL", conflicts_with = "check")]
        pool: Option<String>,
        /// Ends the sign-in when this command's input closes: how a desktop drives it on a
        /// runner, where ending `docker exec` ends nothing in the container.
        #[arg(long, hide = true, conflicts_with_all = ["check", "pool"])]
        until_input_ends: bool,
        /// As --until-input-ends, in a terminal made here of this size, drawn on this command's
        /// output.
        #[arg(
            long,
            hide = true,
            value_name = "COLUMNSxROWS",
            conflicts_with_all = ["check", "until_input_ends", "pool"]
        )]
        in_own_terminal: Option<String>,
        /// Which of the harness's own flows to run, `browser` or `device`: how a desktop asks
        /// for one on a runner.
        #[arg(long, hide = true, conflicts_with_all = ["check", "pool"])]
        way: Option<String>,
    },
    /// Sign a profile of this machine's out on this device, through its harness's own sign-out:
    /// how a desktop signs one out on a runner.
    #[command(name = "sign-out", hide = true)]
    SignOut {
        /// The profile's name.
        name: String,
    },
}

/// The two `version` verbs that move a PUBLISHED `charter-cp` release.
///
/// Registered rather than left to clap so that each gets a sentence instead of a usage error
/// — M2.21's rule, applied to the verbs beside it. Their flags are declared as purlis
/// declares them, because a script that passes `--cli` or `--push` must meet the refusal
/// rather than the parser.
#[derive(Subcommand)]
enum VersionCommand {
    /// Move THIS plane to the version it pins.
    Sync {
        /// Conform the machine-global `purlis` binary instead.
        #[arg(long)]
        cli: bool,
    },
    /// Move the pin: install + verify the target, then write `charter.toml`.
    Bump {
        /// Version to pin (default: the latest published).
        #[arg(long)]
        to: Option<String>,
        /// Also commit + push the lock.
        #[arg(long)]
        push: bool,
    },
}

#[derive(Subcommand)]
enum DocsCommand {
    /// Regenerate `docs/topology.md` from the inventory, and the README's roster block.
    Generate,
    /// List purlis's own documentation topics.
    List,
    /// Print one of purlis's own documentation pages — served by the install that
    /// implements it, so it cannot be a version behind the CLI reading it.
    Show {
        /// e.g. secrets, personas, git-policy (see `docs list`).
        topic: String,
    },
}

#[derive(Subcommand)]
enum WorkspaceCommand {
    /// List the plane's workspaces, one per line.
    List,
    /// Print the active workspace — the name alone.
    ///
    /// It takes no `-w`, because purlis's own `workspace current` takes none: the top rung
    /// of the ladder is typed on the command it acts on, and a flag here would report a
    /// resolution that nothing performed.
    Current,
    /// Show or set a workspace's `## Vision`.
    ///
    /// With text, replace it; without, print it. Empty text is the SHOWING form, as it is
    /// in Python charter — `if text:` there, so `vision ""` prints rather than erasing a
    /// committed, hand-edited file.
    Vision {
        text: Option<String>,
        #[command(flatten)]
        common: Common,
    },
    /// Record one workspace memory (its own file, indexed) — the task journal. Omit the
    /// text to list the workspace's memories.
    Remember {
        text: Option<String>,
        /// Optional title (else derived from the first line).
        #[arg(long)]
        title: Option<String>,
        /// Don't reactively commit+push it now (LIVE workspaces; sync later).
        #[arg(long)]
        no_sync: bool,
        #[command(flatten)]
        common: Common,
    },
    /// Alias for `remember` — record a workspace memory (or list them).
    Note {
        message: Option<String>,
        /// Don't reactively commit+push it now (LIVE workspaces; sync later).
        #[arg(long)]
        no_sync: bool,
        #[command(flatten)]
        common: Common,
    },
    /// Search the workspace's memories (--query) or list them all.
    Recall {
        /// Keyword query; omit to list every memory chronologically.
        #[arg(short = 'q', long)]
        query: Option<String>,
        #[command(flatten)]
        common: Common,
    },
    /// Delete one workspace memory by slug or filename.
    Forget {
        /// Memory slug or filename (see `purlis workspace recall`).
        slug: String,
        #[command(flatten)]
        common: Common,
    },
    /// Rewrite one workspace memory in place: same filename, same stamp, index line
    /// retitled. What is not given is kept.
    Edit {
        /// Memory slug or filename (see `purlis workspace recall`).
        slug: String,
        /// The new body; `-` reads it from standard input (default: keep the body).
        #[arg(required_unless_present = "title")]
        text: Option<String>,
        /// The new title (default: keep the title).
        #[arg(long)]
        title: Option<String>,
        #[command(flatten)]
        common: Common,
    },
    /// Move one workspace memory into memory/archive/ and drop its index line (undo:
    /// unarchive).
    Archive {
        /// Memory slug or filename (see `purlis workspace recall`).
        slug: String,
        #[command(flatten)]
        common: Common,
    },
    /// Move one workspace memory to another scope: another workspace's journal, a persona,
    /// or shared. Its title and stamp go with it, and nothing is copied. Persona and shared
    /// memory are published with the project.
    #[command(name = "move-memory")]
    MoveMemory {
        /// Memory slug or filename (see `purlis workspace recall`).
        slug: String,
        #[command(flatten)]
        to: memory::MoveTo,
        #[command(flatten)]
        common: Common,
    },
    /// Move an archived workspace memory back into the journal and re-index it.
    Unarchive {
        /// The memory's slug or filename in memory/archive/.
        slug: String,
        /// Restore it under this filename instead (for one archiving had to number).
        #[arg(long = "as", value_name = "SLUG")]
        restore_as: Option<String>,
        #[command(flatten)]
        common: Common,
    },
    /// Curate a workspace's memory: collapse exact duplicates and repair the index with
    /// --apply; propose the rest.
    Optimize {
        /// Workspace to optimize (default: every one).
        name: Option<String>,
        /// Every workspace (the default).
        #[arg(long)]
        all: bool,
        /// Apply the safe, reversible ops. Proposals always stay manual.
        #[arg(long)]
        apply: bool,
        /// Age at which a memory is proposed for review (default: 90).
        #[arg(
            long = "stale-days",
            default_value_t = 90,
            allow_negative_numbers = true
        )]
        stale_days: i64,
        /// Pin the clock, for tests only.
        #[arg(long, hide = true)]
        now: Option<String>,
    },
    /// Delete a workspace and its clones. Guards work that removing it would discard.
    ///
    /// Exit 2 is the guard: a refusal that protected work is not the same failure as a name
    /// that is not a workspace, and a script can tell them apart.
    #[command(alias = "rm")]
    Remove {
        name: String,
        /// Remove it even though a clone or a worktree holds work nothing else does.
        #[arg(long)]
        force: bool,
    },
    /// Rename a workspace: its directory, its clones' worktrees, and every record that names it.
    ///
    /// Refused while a chat is running in it, and when the new name is taken or is not one a
    /// workspace can have. A rename that was interrupted is finished by running it again.
    #[command(alias = "mv")]
    Rename {
        /// The workspace's name now.
        old: String,
        /// The name it is to have.
        new: String,
    },
    /// Share a workspace's manifest + memory (LIVE), or make it private again (`--off`).
    Live {
        name: String,
        /// Make it LOCAL: untrack what is committed, then re-ignore it.
        #[arg(long)]
        off: bool,
    },
    /// Create a workspace: its directory, its baseline files and purlis's harness layer. In a
    /// chat the app started, the app makes it, and clones the repos named into it; a sandboxed
    /// chat is refused `--live` and `--use`.
    Create {
        name: String,
        /// Repos to clone into it immediately, by inventory name. POSITIONAL, as purlis's
        /// own `workspace create <name> [repos...]` takes them — a `--repos` of this port's
        /// own would be a command line that works against one purlis and not the other.
        repos: Vec<String>,
        /// What this workspace is for, recorded in its `workspace.md` purlis.
        #[arg(long, alias = "about")]
        vision: Option<String>,
        /// Share its charter, manifest and memory from birth.
        #[arg(long)]
        live: bool,
        /// Select it for this session once it exists.
        #[arg(long = "use")]
        use_it: bool,
        /// Select it even though this session is locked to another workspace.
        #[arg(long)]
        force: bool,
        /// Pin the clock the manifest's `updated_at` is stamped with, for tests only.
        #[arg(long, hide = true)]
        now: Option<String>,
    },
    /// Rebuild a workspace from its manifest — clone repos + checkout branches.
    Restore {
        /// The workspace to rebuild.
        name: String,
        /// Don't clone now; clone each repo when you enter it.
        #[arg(long = "on-demand")]
        on_demand: bool,
        /// Pin the clock a membership record is stamped with, for tests only.
        #[arg(long, hide = true)]
        now: Option<String>,
    },
    /// Fork a workspace: a new one pre-loaded with its charter, memory, todos and manifest.
    ///
    /// The clones are NOT copied — they are reconstructible. `--restore` clones them straight
    /// away, which since M2.26 is the whole restore: `purlis clone` per missing repo, then
    /// the recorded branch checked out and pulled.
    #[command(alias = "duplicate")]
    Fork {
        /// The workspace to fork from.
        src: String,
        /// The fork's name.
        new: String,
        /// Also clone the inherited repos now.
        #[arg(long)]
        restore: bool,
        /// Make the fork LIVE (default LOCAL).
        #[arg(long)]
        live: bool,
        /// Pin the clock the fork's manifest and note are stamped with, for tests only.
        #[arg(long, hide = true)]
        now: Option<String>,
    },
    /// Bring a workspace's structure and purlis's layer up to what this version writes.
    Reinit {
        /// The workspace (default: the active one).
        name: Option<String>,
        /// Every workspace this plane has. Given with a name, this wins and the name is
        /// ignored — purlis's own parser refuses neither, and a port that refused one would
        /// be a command line that works against one purlis and not the other.
        #[arg(long)]
        all: bool,
        /// Pin the clock a backfilled manifest is stamped with, for tests only.
        #[arg(long, hide = true)]
        now: Option<String>,
    },
    /// Select a workspace for this terminal and session, and lock the session to it.
    Use {
        name: String,
        /// Create it first, scaffolded exactly as `workspace create` scaffolds.
        #[arg(long)]
        create: bool,
        /// Switch even though this session is locked to another workspace.
        #[arg(long)]
        force: bool,
        /// Pin the clock a scaffolded manifest is stamped with, for tests only.
        #[arg(long, hide = true)]
        now: Option<String>,
    },
    /// Release this session's workspace lock so a different one can be selected.
    Unlock,
    /// Show, set or clear the workspace a session lands on when nothing else has decided.
    Default {
        name: Option<String>,
        /// Remove the nomination.
        #[arg(long)]
        clear: bool,
    },
    /// Capture this workspace's repos and branches into its committed manifest.
    Snapshot {
        /// The workspace (default: the active one).
        name: Option<String>,
        /// What this workspace is for, recorded in the manifest.
        #[arg(long)]
        description: Option<String>,
        /// Record the branches as they stand, even though some would not restore.
        #[arg(long)]
        force: bool,
        /// Pin the clock `updated_at` is stamped with, for tests only.
        #[arg(long, hide = true)]
        now: Option<String>,
    },
    /// Record a todo, list the open ones (`--done` for the done ones), mark one done with
    /// `done <slug>`, open a done one again with `reopen <slug>`, delete one with
    /// `forget <slug>`, or promote one to an issue with `promote <slug> --repo <repo>`.
    ///
    /// `done`/`reopen`/`forget`/`promote` are read as verbs rather than as todo text, and told
    /// apart by the shape of the call rather than the word: one positional records, two act on
    /// a todo. purlis's own parser does exactly this, and a real subcommand cannot.
    Todo {
        #[arg(num_args = 0..=2)]
        words: Vec<String>,
        /// List the done todos instead of the open ones: each stays, struck through in the
        /// window, until `reopen <slug>` opens it again or `forget <slug>` deletes it.
        #[arg(long)]
        done: bool,
        /// With `promote`: the workspace's repo to open the issue in, by its inventory name.
        /// May be left out when the workspace has one repo on a forge.
        #[arg(long)]
        repo: Option<String>,
        #[command(flatten)]
        common: Common,
    },
    /// Internal, answered and ignored: the Python plugin's SessionStart seed of a session's
    /// workspace pointer from its terminal pane. A chat charter-app starts has no pane to seed
    /// from — its workspace is the pointer keyed on the chat, which `purlis ws use` writes.
    #[command(name = "_reconcile", hide = true)]
    Reconcile,
    /// Internal, answered and ignored: the Python plugin's turn-end commit of a LIVE
    /// workspace's memory. It commits nothing under the default `share = "local"`; under
    /// `commit`/`push` charter-app leaves the commit to `purlis save`, which commits the plane
    /// as one decision instead of racing the operator's own git on every turn end.
    #[command(name = "_autosave", hide = true)]
    Autosave,
}

#[derive(Args)]
struct Common {
    /// The workspace to act on (default: the active one).
    #[arg(short = 'w', long = "workspace")]
    workspace: Option<String>,
    /// Pin the clock a write stamps itself with, for tests only. purlis's own
    /// `memstore.write` takes a `stamp=` for the same reason: ordering has to be testable
    /// across real time gaps, not just within one second.
    #[arg(long, hide = true)]
    now: Option<String>,
}

impl Common {
    fn stamp(&self) -> Result<chrono::NaiveDateTime, String> {
        match &self.now {
            Some(text) => text
                .parse()
                .map_err(|e| format!("--now is not a local naive timestamp: {e}")),
            None => Ok(chrono::Local::now().naive_local()),
        }
    }
}

/// Says `why` as purlis's refusal and fails — for a command answered outside [`run`], whose
/// `Err` is printed the same way.
fn refused(why: &str) -> ExitCode {
    eprintln!("purlis: {why}");
    ExitCode::FAILURE
}

/// Where this invocation is standing: the plane, the directory, and who is asking.
///
/// Built ONCE per command rather than per rung. Every piece of it is read from the process —
/// the cwd, the environment, the session and pane ids — and a second read is a second answer:
/// the cwd rung and the pointer rungs deciding from different snapshots is how a command
/// comes to act on one workspace and report another.
pub struct Here {
    pub plane: Plane,
    cwd: std::path::PathBuf,
    ids: purlis_core::active::Ids,
    workspace_env: Option<String>,
    /// `$CHARTER_PLANE_ROOT_SESSION`: the app started this chat at the plane root (SI-1).
    plane_root_env: Option<String>,
    persona_env: Option<String>,
}

/// The plane this invocation acts on, and nothing else about it.
///
/// Kept beside [`Here`] and called BY it, for the two commands that want the plane and never
/// the ladder: `gl-refresh` is handed its workspace by `main`, and `statusline` runs on every
/// paint, where building the two ids costs a syscall for an answer it does not read.
fn plane() -> Result<Plane, String> {
    let cwd =
        std::env::current_dir().map_err(|e| format!("cannot read the current directory: {e}"))?;
    purlis_core::plane::resolve(&cwd)
        .map(Plane::open)
        .map_err(|e| e.to_string())
}

impl Here {
    fn read() -> Result<Self, String> {
        let cwd = std::env::current_dir()
            .map_err(|e| format!("cannot read the current directory: {e}"))?;
        Ok(Self {
            plane: plane()?,
            cwd,
            ids: purlis_core::active::Ids::from_env(),
            workspace_env: purlis_core::envvar::var(purlis_core::active::WORKSPACE_ENV),
            plane_root_env: purlis_core::envvar::var(purlis_core::active::PLANE_ROOT_ENV),
            persona_env: purlis_core::envvar::var(purlis_core::active::PERSONA_ENV),
        })
    }

    /// The whole ladder, with `flag` on top of it.
    fn asking<'a>(
        &'a self,
        flag: Option<&'a str>,
        env: Option<&'a str>,
    ) -> purlis_core::active::Asking<'a> {
        purlis_core::active::Asking {
            root: self.plane.root(),
            cwd: &self.cwd,
            flag,
            ids: &self.ids,
            env,
        }
    }

    /// Whether this invocation is at the plane root, and why — `None` when `flag`, the
    /// environment, the tree or a pointer puts it in a workspace
    /// ([`purlis_core::active::plane_root`]).
    fn plane_root(&self, flag: Option<&str>) -> Option<purlis_core::active::PlaneRoot> {
        purlis_core::active::plane_root(
            &self.asking(flag, self.workspace_env.as_deref()),
            self.plane_root_env.as_deref(),
        )
    }

    /// Whether the app started this chat at the plane root and nothing names a workspace
    /// instead — the one kind of root session `workspace use` does not move.
    fn launched_at_plane_root(&self) -> bool {
        self.plane_root(None) == Some(purlis_core::active::PlaneRoot::Launched)
    }

    /// The workspace this invocation acts on — or, at the plane root, the sentence saying to
    /// name one with `-w` (SI-1). The plane root is a chat the app started there, or a session
    /// standing anywhere in the plane outside every workspace that has chosen none (SI-1b).
    /// Everywhere else there is always one: the ladder ends on `[workspace] default`, and
    /// under that on the literal `default`.
    pub fn active_workspace(&self, flag: Option<&str>) -> Result<String, String> {
        if let Some(by) = self.plane_root(flag) {
            return Err(purlis_core::active::plane_root_refusal(
                self.plane.root(),
                by,
            ));
        }
        Ok(purlis_core::active::workspace(&self.asking(flag, self.workspace_env.as_deref())).name)
    }

    /// Where this invocation works: the plane root, or the workspace it acts on — for a
    /// command that records where something came from (`handoff`'s stamp and todo).
    pub fn place(&self, flag: Option<&str>) -> purlis_core::active::Place {
        match self.active_workspace(flag) {
            Ok(ws) => purlis_core::active::Place::Workspace(ws),
            Err(_) => purlis_core::active::Place::PlaneRoot,
        }
    }

    /// The workspace this invocation is in, or `None` in a plane-root chat — for a command
    /// that reads a workspace when there is one and runs without one (`recall`, an extension's
    /// project choices, the background refresh).
    pub fn workspace_if_any(&self, flag: Option<&str>) -> Option<String> {
        self.active_workspace(flag).ok()
    }

    /// The persona this invocation acts as, or `None` — a plane may have no front door, and
    /// purlis inventing one would be it choosing an identity nobody asked for.
    pub fn active_persona(&self, flag: Option<&str>) -> Option<String> {
        purlis_core::active::persona(&self.asking(flag, self.persona_env.as_deref())).name
    }

    /// The workspace this command acts on, refusing a name that cannot be one.
    ///
    /// The refusal names the workspace RESOLVED, not the flag: with `-w` absent the operator
    /// never typed a name, and quoting an empty one would describe nothing.
    fn workspace(&self, flag: Option<&str>) -> Result<purlis_core::workspaces::Workspace, String> {
        self.plane
            .workspace(&self.active_workspace(flag)?)
            .map_err(|e| e.to_string())
    }
}

/// The Bash guard's word — answered by [`guard::pretooluse`], ahead of every other tool hook
/// because its refusals are the ones M3.1 was built for.
const GUARDED_TOOL_HOOK: &str = "pretooluse";

/// Whether a word this binary does NOT answer is a TOOL hook, where refusing means blocking.
///
/// **The namespace, not a list of words, and not a blanket rule either.** Both of those were
/// tried and both were wrong, each in the other's direction:
///
/// - A list of the words in `charter/hooks.py:_HANDLERS` fails OPEN on everything not in it.
///   `purlis hook pretooluse-notebook` exited 1, which a harness logs and ignores, so the day
///   purlis adds a matcher the Rust binary on PATH would allow that tool class silently.
/// - Blocking every unknown word instead fails the other way: `purlis hook stopp`, a typo in
///   a settings file purlis itself wrote, blocked the session from ENDING — which is the
///   exact hazard this whole file was written around.
///
/// The word says which hook it is. In the `pretooluse`/`posttooluse` namespace there is a tool
/// call to protect and blocking is the safe answer, for every matcher purlis has and every
/// one it adds. Outside it there is nothing to protect and blocking can only wedge a session.
///
/// **Every word purlis has today is answered before this is asked** — the registry
/// ([`purlis_core::hookreg::HANDLERS`]) and its no-ops. What is left here is only the word
/// nobody has invented yet, and the rule for it is unchanged because its argument is: a
/// program that has checked nothing may not say `allow`.
fn is_a_tool_hook(name: &str) -> bool {
    is_a_pretooluse_hook(name) || name.starts_with("posttooluse")
}

/// Whether the word is a `PreToolUse` hook: one that decides whether a tool call runs, so a
/// crash in it must refuse the call ([`guard::refuse_on_a_crash`]). The namespace, as
/// [`is_a_tool_hook`] reads it, and for its reason — a matcher purlis adds tomorrow is covered
/// today. `posttooluse` is not: the tool has already run, and there is nothing left to refuse.
fn is_a_pretooluse_hook(name: &str) -> bool {
    name.starts_with("pretooluse")
}

/// Whether this command line is `purlis hook <a PreToolUse word>`, read off the raw argv
/// because it is asked before clap has parsed anything — a crash in the parse is a crash too.
/// Any argument after `hook` counts, so a flag written before the word does not hide it.
fn is_a_pretooluse_call(argv: &[std::ffi::OsString]) -> bool {
    argv.get(1).is_some_and(|word| word == "hook")
        && argv
            .iter()
            .skip(2)
            .any(|arg| arg.to_str().is_some_and(is_a_pretooluse_hook))
}

/// What a harness reads as "block".
const BLOCK: u8 = 2;

/// Set in a debug build, `purlis` panics before it has read its command line (#349): the
/// test suite's way to watch what a crash answers. Compiled out of a release build.
#[cfg(debug_assertions)]
const PANIC_ON_PURPOSE_ENV: &str = "CHARTER_TEST_HOOK_PANICS";

/// Set in a debug build, `purlis-core` raises one warning before the command line is read: the
/// test suite's way to watch where a core diagnostic goes under the CLI (#647). Compiled out of
/// a release build.
#[cfg(debug_assertions)]
const CORE_WARNS_ON_PURPOSE_ENV: &str = "CHARTER_TEST_CORE_WARNS";

/// Set in a debug build, `purlis serve` raises one `info` line from `purlis-core` and ends
/// before it serves: the test suite's way to watch where the core's `info` goes under a
/// runner's host (#2753). Compiled out of a release build.
#[cfg(debug_assertions)]
#[cfg(unix)]
const CORE_INFORMS_ON_PURPOSE_ENV: &str = "PURLIS_TEST_CORE_INFORMS";

/// Set in a debug build, `purlis` says whose process group the gits it runs are in, and does
/// nothing else: the test suite's way to hold that this command keeps them in its own
/// ([`purlis_core::worktree::git::stay_in_the_callers_group`]). Compiled out of a release
/// build.
#[cfg(debug_assertions)]
const SAYS_ITS_GITS_GROUP_ENV: &str = "PURLIS_TEST_SAYS_ITS_GITS_GROUP";

/// How long the payload on stdin is waited for.
///
/// **Two seconds, and it used to be 25 milliseconds.** The spec allows the whole call 50 ms
/// and this binary was measured at 1.8, so 25 looked generous — but a review measured an
/// 8 MB `UserPromptSubmit` (a pasted log) at 47 ms, and a payload written in two pieces
/// missed 25 ms every time. Reaching the deadline is not free: purlis then cannot establish
/// which conversation the report is of, and before a chat has adopted a process that costs
/// the whole report.
///
/// This exists only against a harness that opens the hook's stdin and never writes, which
/// would otherwise hang the turn for good. Two seconds is well under the plugin's own 5 s
/// hook timeout, so the harness's deadline is still the one that fires first, and no ordinary
/// payload can reach this one.
///
/// A reporting hook only. A `PreToolUse` hook reads its payload inside its budget, and one that
/// does not arrive in time is a refusal, never an empty payload (#928).
const PAYLOAD_DEADLINE: Duration = Duration::from_secs(2);

/// Reads the harness's payload, or gives up on it as empty.
fn payload() -> String {
    payload_within(PAYLOAD_DEADLINE).unwrap_or_default()
}

/// Reads the harness's payload, or `None` when it has not all arrived within `limit`.
///
/// On its own thread, because a read from a pipe nobody is writing to cannot be interrupted.
/// The thread is left behind when the deadline passes: the process is about to exit, and
/// waiting for it is the very thing being avoided.
fn payload_within(limit: Duration) -> Option<String> {
    use std::io::Read;

    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut text = String::new();
        let _ = std::io::stdin().read_to_string(&mut text);
        let _ = tx.send(text);
    });
    rx.recv_timeout(limit).ok()
}

/// The rule a refused tool hook word this binary does not answer is recorded under.
const UNKNOWN_HOOK_RULE: &str = "unknown-hook";

/// The rule a tool call refused because its guard crashed is recorded under.
pub(crate) const GUARD_CRASHED_RULE: &str = "guard-crashed";

/// The rule a tool call refused because its guard did not answer in time is recorded under.
const GUARD_UNANSWERED_RULE: &str = "guard-unanswered";

/// One [`hookwire::ToolCall`] to the host, when a host is listening: the tool, the hash of its
/// arguments, the decision and how long the hook took. Never the arguments themselves.
///
/// **Before the harness is answered** (FD-30): the host says it has recorded the line, or the
/// line is in the chat's spool, durably, when this returns ([`hookwire::deliver_tool`]). A
/// line that reached neither is said only on stderr, as a report that did not arrive is, and
/// the answer is the same either way.
#[cfg(unix)]
fn tell_the_host_about_the_tool_call(
    word: &str,
    payload: &str,
    answered: &hooks::Answered,
    took: std::time::Duration,
    skill: Option<Box<hookwire::SkillStarted>>,
) {
    use std::io::Write as _;
    let Some(socket) = purlis_core::envvar::var_os(SOCKET_ENV) else {
        return;
    };
    let Some(chat) =
        purlis_core::envvar::var(hookwire::CHAT_ENV).and_then(|chat| chat.parse().ok())
    else {
        // Never `eprintln!` here: this runs from a crashed guard's panic hook too, and a
        // print to a closed stderr panics there, which aborts instead of refusing (exit 2).
        let _ = writeln!(
            std::io::stderr(),
            "purlis: this {word} names no chat (`${}` is not a chat number), so the app was \
             not told",
            hookwire::CHAT_ENV
        );
        return;
    };
    let data: serde_json::Value = serde_json::from_str(payload).unwrap_or_default();
    let text = |field: &str| data[field].as_str().map(str::to_owned);
    let call = hookwire::ToolCall {
        chat,
        tool_hook: word.to_owned(),
        tool: text("tool_name"),
        call: text("tool_use_id"),
        args: data.get("tool_input").map(purlis_core::eventlog::args_hash),
        decision: answered.decision,
        rule: answered.rule.clone(),
        hook_ms: u64::try_from(took.as_millis()).unwrap_or(u64::MAX),
        agent: hookwire::sub_agent(data["agent_id"].as_str(), &purlis_core::envvar::var),
        at_ms: std::time::SystemTime::now()
            .checked_sub(took)
            .map(hookwire::ms_since_1970)
            .unwrap_or_default(),
        skill,
    };
    let token = hookwire::ChatToken::from_env();
    if let Err(why) = hookwire::deliver_tool(std::path::Path::new(&socket), token.as_ref(), &call) {
        let _ = writeln!(std::io::stderr(), "{} {word} ({why})", hookwire::NOT_TAKEN);
    }
    // The file a file tool touched, for the tree's live marker (FM-6): on a line of its own,
    // sent once and never spooled, so no record holds it (D-86a). Not for a call the guard
    // refused, which touched nothing. A marker the app missed is not worth a word on stderr.
    if call.decision != hookwire::Decision::Deny
        && let Some(touching) = purlis_core::touching::of_tool(&data)
    {
        let _ = hookwire::touch(
            std::path::Path::new(&socket),
            token.as_ref(),
            &hookwire::Touching {
                chat,
                touching,
                wrote: purlis_core::touching::writes(&data),
            },
        );
    }
    // What the chat is doing, for the one line under its name (#1493): a kind from a fixed list
    // and at most one short name, on a line of its own, sent once and never spooled, so no
    // record holds it. Only for a call that will run: not one the guard refused, and not one
    // the person is being asked about, which they may refuse.
    if let Some(doing) = purlis_core::doing::of_answered_hook(word, &data, call.decision) {
        let _ = hookwire::tell_doing(
            std::path::Path::new(&socket),
            token.as_ref(),
            &hookwire::Doing {
                chat,
                doing,
                agent: call.agent.clone(),
                speaker: hookwire::Speaker::read(payload, &purlis_core::envvar::var),
            },
        );
    }
    // A shell command of the chat's own that came back, with its status (#1981): what
    // `purlis dispatch read` lists of a task. On a line of its own, sent once and never spooled.
    if call.agent.is_none()
        && let Some(turn_output) = hookwire::Output::of_ran(
            word,
            &data,
            hookwire::chat_harness(&purlis_core::envvar::var).as_deref(),
        )
    {
        let _ = hookwire::tell_turn_output(
            std::path::Path::new(&socket),
            token.as_ref(),
            &hookwire::TurnOutput {
                chat,
                turn_output,
                speaker: hookwire::Speaker::read(payload, &purlis_core::envvar::var),
            },
        );
    }
}

/// Each sandbox block a block word found in the call's result, to the host, when one is
/// listening (#1338): an operation and a kind, and whether it was purlis's own — never the path,
/// the command or what it printed. Sent once each and never spooled
/// ([`hookwire::tell_blocked`]); a block the app did not take is said only on stderr. Answers
/// the blocks the app took, which the chat is then told of ([`hooks::told_after`]).
///
/// A block that is the chat's own to retry (#3861,
/// [`purlis_core::sandboxblock::its_own_to_retry`]) is sent for the record too, and raises no
/// Notice there; it is answered apart, as the shape the chat retries in, whether or not the app
/// took it.
#[cfg(unix)]
fn tell_the_host_about_blocks(word: &str, payload: &str) -> hooks::Blocked {
    use std::io::Write as _;
    let mut taken = hooks::Blocked::default();
    // The word first: every other tool hook passes through here, and none of them is read.
    if !hooks::is_a_block_word(word) {
        return taken;
    }
    let Some(socket) = purlis_core::envvar::var_os(SOCKET_ENV) else {
        return taken;
    };
    let Some(chat) =
        purlis_core::envvar::var(hookwire::CHAT_ENV).and_then(|chat| chat.parse().ok())
    else {
        return taken;
    };
    let data: serde_json::Value = serde_json::from_str(payload).unwrap_or_default();
    let token = hookwire::ChatToken::from_env();
    let harness = hookwire::chat_harness(&purlis_core::envvar::var);
    // A tool that ran nothing (a read of a background command's output): a quiet line (#1884).
    let read_back = purlis_core::sandboxblock::read_back(&data);
    let chat_dir = hooks::chat_dir(&purlis_core::envvar::var);
    for (block, target) in hooks::blocks(&data, &purlis_core::envvar::var) {
        let own = purlis_core::sandboxblock::its_own_to_retry(
            &block,
            target.as_deref(),
            chat_dir.as_deref(),
        );
        taken.retry_with_purlis |= own;
        // A refused service on this Mac whose port only printed words name: offered to the
        // person to check, never as the target (qw384).
        let suggested = (block.kind == purlis_core::sandboxblock::Kind::LocalService
            && target.is_none())
        .then(|| purlis_core::sandboxblock::printed_port(&data))
        .flatten()
        .map(|port| port.to_string());
        let blocked = hookwire::SandboxBlocked {
            chat,
            sandbox_blocked: block,
            harness: harness.clone(),
            target,
            read_back,
            suggested,
        };
        match hookwire::tell_blocked(std::path::Path::new(&socket), token.as_ref(), &blocked) {
            Ok(()) if !own => taken.taken.push(block),
            Ok(()) => {}
            Err(why) => {
                let _ = writeln!(std::io::stderr(), "{} {word} ({why})", hookwire::NOT_TAKEN);
            }
        }
    }
    taken
}

#[cfg(not(unix))]
fn tell_the_host_about_blocks(_word: &str, _payload: &str) -> hooks::Blocked {
    hooks::Blocked::default()
}

/// What a crashed `PreToolUse` guard, or one that did not answer in time, tells the host from its
/// panic hook: the call was refused, and why. The payload is not read again in a process that is going down.
fn tell_the_host_about_a_crash(word: &str, unanswered: bool) {
    let rule = if unanswered {
        GUARD_UNANSWERED_RULE
    } else {
        GUARD_CRASHED_RULE
    };
    let answered =
        hooks::Answered::exit(ExitCode::from(BLOCK), hookwire::Decision::Deny, Some(rule));
    tell_the_host_about_the_tool_call(word, "", &answered, Duration::ZERO, None);
}

#[cfg(not(unix))]
fn tell_the_host_about_the_tool_call(
    _word: &str,
    _payload: &str,
    _answered: &hooks::Answered,
    _took: std::time::Duration,
    _skill: Option<Box<hookwire::SkillStarted>>,
) {
}

/// The rule a `PreToolUse` call refused for a disagreeing variable is recorded under.
const DISAGREEING_NAMES_RULE: &str = "env-names-disagree";

/// A hook run where a variable that chooses the project disagrees with its old name
/// (D-RN2d-8): it acts on no project, and says why on standard error.
///
/// - **A `PreToolUse` hook blocks** (exit 2, the sentence as the reason the harness shows): the
///   guard would otherwise judge the call against a project it guessed, and a refused write is
///   the safe side. The host hears the refusal, as it hears every tool call.
/// - **Every other hook stands aside** (exit 0, nothing on standard output): it only reports,
///   briefs or records, and exiting 2 from one would leave a session unable to end.
fn hook_on_disagreement(name: &str, disagree: &purlis_core::envvar::Disagreement) -> ExitCode {
    eprintln!("purlis: {disagree}");
    if !is_a_pretooluse_hook(name) {
        return ExitCode::SUCCESS;
    }
    let answered = hooks::Answered::exit(
        ExitCode::from(BLOCK),
        hookwire::Decision::Deny,
        Some(DISAGREEING_NAMES_RULE),
    );
    tell_the_host_about_the_tool_call(name, &payload(), &answered, Duration::ZERO, None);
    answered.print()
}

/// `purlis hook <name>` — always succeeds, whatever went wrong.
///
/// **Never exit 2, except where the whole point is to.** A harness reads 2 as "block": on
/// `Stop` it makes the harness carry on rather than end. Nothing purlis draws is worth that,
/// so every failure on a REPORTING hook is a silent 0 and the state the app draws is simply the
/// last one it was told. The three exceptions are all about a tool call: a denial a guard
/// decided and could not print ([`guard::deny`]), a word in the tool-hook namespace this
/// binary does not answer at all ([`is_a_tool_hook`]), and a `PreToolUse` hook that crashed
/// ([`guard::refuse_on_a_crash`]).
fn hook(name: &str, now: Option<&str>) -> ExitCode {
    // The harness started this process, not the chat: a refusal it meets is the chat's
    // sandbox's only where the harness's sandbox holds its hooks (#1421).
    purlis_core::sandbox::started_by_the_harness();
    // FIRST, in front of `Event::parse`, because none of these is one of the app's reporting
    // events: a tool call carries no chat state worth a `Report`.
    //
    // **The host hears every tool call before the harness is answered** (FD-9, FD-30): the
    // verdict is decided, then one line on the hook channel says what it is, and only once the
    // host has recorded it, or the line is in the chat's spool, is the verdict printed. The
    // exit status is the verdict's whatever the send does, and the send gives up on the host
    // after a bounded wait (`hookwire::deliver_tool`), so a slow or frozen app costs a guard's
    // answer that wait at most, never the answer.
    // The harness's permission prompt (HP-6): held open until the operator answers it in the
    // window, or the harness's own prompt decides. Not a tool hook: it decides nothing itself.
    if name == purlis_core::harness::hooked::WORD {
        return permission::permissionrequest(&payload());
    }
    let guarded = name == GUARDED_TOOL_HOOK;
    let handler = hooks::handler(name);
    if guarded || handler.is_some() {
        let began = std::time::Instant::now();
        let decide = |text: &str| match handler {
            Some(handler) if !guarded => hooks::run(handler, text, now),
            _ => guard::pretooluse(text, now),
        };
        // A `PreToolUse` hook decides inside one budget, payload and all, on a deep stack, and
        // a decision not made in time is a refusal (#1355, #928). The budget is what keeps the
        // harness's own timeout, which may run the tool, from ever being reached.
        let (text, answered) = if is_a_pretooluse_hook(name) {
            let budget = guard::Budget::start();
            let Some(text) = payload_within(budget.left()) else {
                guard::out_of_time()
            };
            let answered = guard::decided_within(&budget, || decide(&text));
            (text, answered)
        } else {
            let text = payload();
            let answered = decide(&text);
            (text, answered)
        };
        // The skill run the call starts (SK-3): heard once, as the call came back, and
        // never of a call the guard refused, which ran nothing.
        let skill = (purlis_core::hookreg::find(name)
            .is_some_and(|hook| hook.hears_a_call_come_back() || hook.hears_a_prompt_start())
            && answered.decision != hookwire::Decision::Deny)
            .then(|| hooks::with_hook(&text, now, purlis_core::toolhooks::skill_started))
            .flatten();
        tell_the_host_about_the_tool_call(name, &text, &answered, began.elapsed(), skill);
        let taken = tell_the_host_about_blocks(name, &text);
        // A block word answers nothing of its own, so this is the one line it prints (#1631).
        // And what a git command the sandbox stopped partway left behind (qw349), in the
        // same line: a hook prints one answer. Never of words only read back (#1884).
        let data = hooks::is_a_block_word(name)
            .then(|| serde_json::from_str::<serde_json::Value>(&text).ok())
            .flatten();
        let read_back = data
            .as_ref()
            .is_some_and(purlis_core::sandboxblock::read_back);
        let stopped = data
            .as_ref()
            .filter(|_| !read_back)
            .and_then(|payload| hooks::stopped_partway(payload, &purlis_core::envvar::var));
        if let Some(line) = hooks::told_after(name, &taken, read_back, stopped) {
            hooks::say(&line);
        }
        return answered.print();
    }
    if purlis_core::hookreg::NO_OPS.contains(&name) {
        // Answered with nothing, as ever, and still one call the host hears. The payload is
        // read only when a host is listening, so a terminal's own harness pays nothing for it.
        let began = std::time::Instant::now();
        if purlis_core::envvar::var_os(SOCKET_ENV).is_some() {
            let text = payload();
            let answered = hooks::Answered::exit(ExitCode::SUCCESS, hookwire::Decision::None, None);
            tell_the_host_about_the_tool_call(name, &text, &answered, began.elapsed(), None);
        }
        return ExitCode::SUCCESS;
    }
    let Some(event) = Event::parse(name) else {
        let tool = is_a_tool_hook(name);
        // The word comes out of a settings file a chat can write, and this sentence goes to
        // a terminal and into the harness's own log. Contained like every other value
        // charter quotes back.
        let name = purlis_core::shown::readable(name, purlis_core::shown::DISPLAY_LIMIT);
        eprintln!(
            "purlis: `{name}` is not a hook this binary answers (`purlis hook --list` names \
             them){}.",
            if tool {
                ", and it names a tool hook, so the tool call is refused rather than allowed \
                 by a program that checked nothing"
            } else {
                ""
            }
        );
        // Blocking only where there is a tool call to protect. Everywhere else a refusal that
        // a harness reads as "block" would wedge the session instead of guarding anything.
        if !tool {
            return ExitCode::FAILURE;
        }
        // A refused tool call is still one the host hears.
        if purlis_core::envvar::var_os(SOCKET_ENV).is_some() {
            let answered = hooks::Answered::exit(
                ExitCode::from(BLOCK),
                hookwire::Decision::Deny,
                Some(UNKNOWN_HOOK_RULE),
            );
            tell_the_host_about_the_tool_call(
                name.as_ref(),
                &payload(),
                &answered,
                Duration::ZERO,
                None,
            );
        }
        return ExitCode::from(BLOCK);
    };
    let socket = purlis_core::envvar::var_os(SOCKET_ENV);
    // The payload is read once, and only when something reads it: `sessionstart` always does,
    // `userpromptsubmit` for the heartbeat, and every event when the app is listening.
    let wanted = socket.is_some() || matches!(event, Event::SessionStart | Event::UserPromptSubmit);
    let text = if wanted { payload() } else { String::new() };
    // The session's own work comes BEFORE the report: a briefing the harness never receives
    // because the app's socket was slow would be the worse of the two to lose.
    match event {
        Event::SessionStart => hooks::sessionstart(&text, now),
        Event::UserPromptSubmit => hooks::userpromptsubmit(&text, now),
        _ => {}
    }
    let Some(socket) = socket else {
        // No app started this session — the operator's own harness in a terminal, with the
        // hooks pointed here. There is nothing to tell, and this is the ONE path on which
        // anything decides to refresh the forge cache (charter-app#89).
        if event == Event::SessionStart {
            refresh_the_forge_cache();
        }
        return ExitCode::SUCCESS;
    };
    let report = Report::read(event, &text, &purlis_core::envvar::var);
    if report.is_none() {
        // A host is listening and this hook names no chat it started: `$CHARTER_CHAT` is
        // missing or not a number. There is no chat to put the event under, so none is sent,
        // and that is said rather than left for nobody to notice.
        eprintln!(
            "purlis: this {} names no chat (`${}` is not a chat number), so the app was not told",
            event.word(),
            hookwire::CHAT_ENV
        );
    }
    // What the turn ended saying (#1981), before the report, so the app has it as it hears the
    // turn end: a task that ended its turns without a report is reported from it. The chat's
    // own run alone, on a line of its own, sent once and never spooled.
    if event == Event::Stop
        && let Some(report) = &report
        && report.agent.is_none()
        && let Some(turn_output) = hookwire::Output::of_stop(&text)
    {
        let _ = hookwire::tell_turn_output(
            std::path::Path::new(&socket),
            hookwire::ChatToken::from_env().as_ref(),
            &hookwire::TurnOutput {
                chat: report.chat,
                turn_output,
                speaker: hookwire::Speaker::read(&text, &purlis_core::envvar::var),
            },
        );
    }
    if let Some(report) = &report
        && let Err(why) = hookwire::deliver_report(
            std::path::Path::new(&socket),
            hookwire::ChatToken::from_env().as_ref(),
            report,
        )
    {
        // The app may have quit while this session was still running, which is the ordinary
        // way for this to fail and is not the harness's business — hence the exit 0 below.
        //
        // But it is said, because a report that never arrives is otherwise invisible
        // everywhere: the chat simply stops changing, and there is nothing anywhere to look
        // at. A zero-exit hook's stderr goes to the harness's debug log, which costs the
        // operator nothing and is exactly where somebody debugging this would look.
        eprintln!("{} {} ({why})", hookwire::NOT_TAKEN, event.word());
    }
    // After the report, so the app has heard the turn end before it hears the record: a
    // saved-record line the harness's sandbox kept from reaching the app (#517).
    if event == Event::Stop
        && let Some(report) = &report
    {
        session::pass_on_at_stop(std::path::Path::new(&socket), report);
    }
    ExitCode::SUCCESS
}

/// The forge cache's trigger on a plane with **no app open** — charter-app#89, which is the
/// half of #69 that was deferred.
///
/// # What Python actually does, since the issue says otherwise
///
/// #89 says this is "the `charter hook` trigger Python has". It is not: `charter/hooks.py`
/// never calls `glstate.maybe_spawn`, and `sessionstart()` does not go near it. Python's one
/// trigger is `charter/statusline.py:_render`, which calls `glstate.read_for` and then
/// `glstate.maybe_spawn` on the footer's own render path — every turn that draws the footer
/// kicks a refresh. So this is not a port of a call site. It is the POLICY ported (that is
/// `glstate.rs`, #69) put on a path Python does not use for it, and the reason is that the
/// path Python does use is not available here:
///
/// - **Inside the app** `purlis statusline` draws nothing and returns early (ADR 0019), and
///   the app has its own trigger already — `panels::repo_states`, focusing a workspace.
/// - **Outside it**, `statusLine` is armed only for chats the app starts and only where the
///   operator fills that line with nothing (`harness::claude_code_status_line`,
///   `footerclaim`). A plane with no app open has no `purlis statusline` running at all —
///   purlis #895 deleted the one that used to be wired — while the plugin's hooks DO run.
///
/// Which leaves this: the hook is the only thing purlis runs on a plane nobody has an app
/// open on, so it is the only place the trigger can go.
///
/// # Why only `SessionStart`, and only with no app behind it
///
/// **Once a session, not once a turn.** The brakes make a repeat trigger cheap — past the
/// cooldown is two file reads — but "cheap" multiplied by every prompt and every turn end is
/// the shape #69's brakes exist to prevent, on a path with a budget. `SessionStart` is the one
/// event that fires once, and with `REFRESH_TTL` at 300 s a per-turn trigger buys no freshness
/// a per-session one does not.
///
/// **And only where the app is not.** `$CHARTER_HOOK_SOCKET` in the environment means this
/// chat was started by the app, which already decides when a refresh runs. Two deciders on one
/// plane is not unsafe — the lock is what makes that safe — but it is two policies, and the
/// issue is about the plane that has none.
///
/// # What it costs
///
/// Everything here is filesystem-only: resolving the plane, reading the workspace ladder,
/// listing the workspace's clones and their worktree directories (`glrefresh::trees` is
/// `read_dir` and path arithmetic — no git spawn), then `glstate::decide`'s two file reads.
/// The one expensive act, the fork, happens at most once per `SPAWN_COOLDOWN` and is a spawn
/// and never a wait. Measured on this machine over 300 runs each, `purlis hook sessionstart`
/// against a plane whose cache is fresh: **3.68 ms before, 3.66 ms after**, against a
/// `purlis --version` floor of 3.63 ms — the added work does not clear the noise of starting
/// the process at all.
///
/// # Best-effort, and silent about it
///
/// Nothing here is worth a word on a reporting hook: no plane, no readable workspace, no
/// `current_exe`, a fork that failed — each simply means no refresh, and the column already
/// says "nothing has fetched this checkout" rather than reading as green (#69). Python's own
/// is a bare `except Exception: return` for the same reason.
fn refresh_the_forge_cache() {
    use purlis_core::{glrefresh, glstate};

    // **This very executable**, never a `charter` found on `$PATH`. That is
    // `charter/util.py:self_relaunch_argv`'s `-P` (charter #390) carried over: the child must
    // be the same charter as the parent, and a `PATH` lookup from inside a plane can find an
    // older install, the Python charter, or a `charter` in the checkout the chat is standing
    // in. `glstate::spawn`'s own doc gives the app's half of the same rule.
    let Ok(binary) = std::env::current_exe() else {
        return;
    };
    // Outside a plane there is nothing to refresh and nowhere to cache it —
    // `charter/glstate.py:maybe_spawn`'s `if not config.HAS_CONTROL_PLANE: return`, which is
    // the brake `glstate::decide` leaves to its callers because every other one is handed a
    // root that was already found (charter #527).
    let Ok(here) = Here::read() else {
        return;
    };
    // A plane-root chat is in no workspace, so there are no clones of one to refresh.
    let Some(workspace) = here.workspace_if_any(None) else {
        return;
    };
    let root = here.plane.root();
    // The same list the panel draws and the same list the child will fetch for, because a
    // staleness question asked about other trees is a question about nothing.
    let Ok(targets) = glrefresh::trees(root, &workspace) else {
        return;
    };
    // The answer is dropped on purpose: `Declined` is the brakes working, and `NotStarted` is
    // a fork that did not happen, which the next session start retries because a spawn that
    // failed does not arm the cooldown.
    let _ = glstate::maybe_spawn(root, &workspace, &targets.trees, &binary);
}

/// `purlis gl-refresh` — ask each clone's own forge about the branch it is on, and write the
/// answers into the cache the panels read.
///
/// A port of `charter/commands.py:cmd_gl_refresh`. The work itself is
/// [`purlis_core::glrefresh`], which is where the credential boundary is argued.
fn gl_refresh(ws: &str, detach: bool, now: Option<&str>) -> ExitCode {
    use purlis_core::glrefresh;

    // Checked FIRST: the point is to return before any of the work below, in a process the
    // harness will not tear down with the turn.
    if detach {
        return match detach_self(ws, now) {
            Ok(()) => ExitCode::SUCCESS,
            Err(why) => {
                eprintln!("purlis: {why}");
                ExitCode::FAILURE
            }
        };
    }
    let root = match plane() {
        Ok(plane) => plane.root().to_path_buf(),
        Err(why) => {
            eprintln!("purlis: {why}");
            return ExitCode::FAILURE;
        }
    };
    let stamp = match instant(now) {
        Ok(stamp) => stamp,
        Err(why) => {
            eprintln!("purlis: {why}");
            return ExitCode::FAILURE;
        }
    };
    // A workspace this plane does not have is REFUSED here, where charter answers "No repos in
    // workspace '<name>'." and exits 0. That is a declared divergence: a `-w` nobody can act on
    // reading as "there is nothing to do" is how a typo silently refreshes nothing for ever,
    // and this binary already takes that position everywhere else (`vision` refuses a
    // workspace it would otherwise have invented).
    let found = match glrefresh::trees(&root, ws) {
        Ok(found) => found,
        Err(why) => {
            eprintln!("purlis: {why}");
            return ExitCode::FAILURE;
        }
    };
    // Said, never dropped — `repos::clones`'s own rule. A refused directory is one this
    // refresh will not fetch for, and the row it feeds will stay empty until somebody is told
    // why.
    for (name, why) in &found.refused {
        voice::warn(format!("{name} is not refreshed — {why}"));
    }
    let trees = found.trees;
    if trees.is_empty() {
        voice::info(format!("No repos in workspace '{ws}'."));
        return ExitCode::SUCCESS;
    }
    let cache = glrefresh::refresh(&root, &trees, stamp);
    voice::ok(format!(
        "Refreshed forge state for {} tree(s) in '{ws}'.",
        trees.len()
    ));
    for tree in &trees {
        let entry = cache.get(&glrefresh::key_for(tree));
        let field = |name: &str| entry.and_then(|row| row.get(name));
        let mut bits: Vec<String> = Vec::new();
        // `if ent.get("change")`: a change of zero or none is no change to report.
        if let Some(change) = field("change")
            .and_then(serde_json::Value::as_u64)
            .filter(|n| *n > 0)
        {
            // An entry written before the forge protocol carried a sigil has none, and the
            // display default is GitLab's — which is what `cmd_gl_refresh` prints.
            let sigil = field("sigil")
                .and_then(serde_json::Value::as_str)
                .filter(|s| !s.is_empty())
                .unwrap_or("!");
            bits.push(format!("{sigil}{change}"));
        }
        if let Some(ci) = field("ci")
            .and_then(serde_json::Value::as_str)
            .filter(|s| !s.is_empty())
        {
            bits.push(format!("pipeline:{ci}"));
        }
        if !bits.is_empty() {
            let name = tree
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            voice::info(format!("  {name}: {}", bits.join(" · ")));
        }
    }
    ExitCode::SUCCESS
}

/// Re-run this binary's `gl-refresh` in a process that outlives this one.
/// `charter/util.py:detach_self`.
///
/// **Two differences from Python, both deliberate.**
///
/// Python re-launches `python -m charter` and drops `--workspace`, leaving the child to
/// resolve the active workspace for itself. This binary has no resolution ladder, so the
/// workspace is carried — and carrying it is the better half of that argument anyway:
/// `glstate.maybe_spawn` already passes `--workspace` explicitly because "a refresh keyed to a
/// different workspace than the row it is refreshing is the defect".
///
/// Python calls `setsid`; this sets the child's own process GROUP. A hook's process group is
/// what a harness tears down when the turn ends, so the group is what has to be left — and
/// `Command::process_group` is safe, where `setsid` would need a `pre_exec` closure and this
/// workspace forbids `unsafe`. What it does not buy is detachment from the controlling
/// terminal, which a background refresh writing to `/dev/null` never touches.
fn detach_self(ws: &str, now: Option<&str>) -> Result<(), String> {
    use std::process::{Command, Stdio};

    let me = std::env::current_exe().map_err(|e| format!("cannot find this binary: {e}"))?;
    let mut child = Command::new(me);
    child.arg("gl-refresh").arg("-w").arg(ws);
    if let Some(now) = now {
        child.arg("--now").arg(now);
    }
    child
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    in_a_group_of_its_own(&mut child);
    match child.spawn() {
        Ok(_) => Ok(()),
        Err(why) => Err(format!("could not start a detached refresh: {why}")),
    }
}

/// Puts the child in a process group of its own, which is what a harness's teardown at the
/// end of a turn does NOT reach.
#[cfg(unix)]
fn in_a_group_of_its_own(child: &mut std::process::Command) {
    use std::os::unix::process::CommandExt;
    child.process_group(0);
}

/// Windows has no process group in this sense, and what a harness tears down there is not a
/// group — so this is **not** the same guarantee under another name.
///
/// `CREATE_NEW_PROCESS_GROUP` is the nearest thing: it takes the child out of the parent's
/// Ctrl+C/Ctrl+Break group. Whether that is enough to outlive a Claude Code turn on Windows
/// is unmeasured, because no harness has ever run there — charter-app#100 is where that is
/// asked, with a test. Left here rather than omitted because a refresh in the parent's group
/// is strictly worse than one outside it, and neither is yet known to be right.
#[cfg(windows)]
fn in_a_group_of_its_own(child: &mut std::process::Command) {
    use std::os::windows::process::CommandExt;
    /// `CREATE_NEW_PROCESS_GROUP`, from `winbase.h`. Spelled out rather than pulled from a
    /// crate for one constant that has not changed since Windows NT.
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    child.creation_flags(CREATE_NEW_PROCESS_GROUP);
}

/// Anywhere else, the refresh simply runs in this process's group.
#[cfg(not(any(unix, windows)))]
fn in_a_group_of_its_own(_child: &mut std::process::Command) {}

/// The instant a refresh stamps every entry with: `--now` as a local naive time, else the
/// wall clock. Seconds since the epoch, as Python's `time.time()` answers.
fn instant(now: Option<&str>) -> Result<f64, String> {
    let Some(text) = now else {
        return Ok(std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|since| since.as_secs_f64())
            .unwrap_or(0.0));
    };
    let naive: chrono::NaiveDateTime = text
        .parse()
        .map_err(|e| format!("--now is not a local naive timestamp: {e}"))?;
    // A naive stamp is LOCAL time, as `--now` is everywhere in this binary.
    chrono::TimeZone::from_local_datetime(&chrono::Local, &naive)
        .single()
        .map(|local| local.timestamp() as f64)
        .ok_or_else(|| "--now names no single local instant".to_string())
}

/// `save` and `git-policy`, or `None` for any other command.
///
/// **They resolve the plane exactly as every other command does**
/// ([`purlis_core::plane::resolve`]): on the plane the vault, the personas and the memory
/// belong to, out of a linked worktree and outward through an enclosing plane's `workspaces/`.
/// `save`'s two refusals — you are standing in a worktree, you are standing in a nested plane
/// — only exist once that is the resolution, because they are about the caller standing
/// somewhere other than the tree being committed.
///
/// **There is no gap left to describe, and there were two.** M2.9 gave `plane::find_root` the
/// outward hop, so the read commands stopped acting on a clone's own plane; M2.16 gave it the
/// worktree redirect and deleted the second resolver these two used to ask, because a step
/// only `save` takes is a step on which `purlis save` and the command beside it name
/// different planes in the same directory. Python resolves once, through
/// `charter/root.py:find_root`, for every command it has.
///
/// What is still special here is only WHEN the plane is resolved: before the ladder, because
/// these two want the plane and never the workspace or the persona.
fn plane_command(command: &Command) -> Option<ExitCode> {
    use purlis_core::repocmd::Say;

    let mut say = |line: Say| eprintln!("{line}");
    let cwd = match std::env::current_dir() {
        Ok(cwd) => cwd,
        Err(e) => {
            eprintln!("purlis: cannot read the current directory: {e}");
            return Some(ExitCode::FAILURE);
        }
    };
    let root = match command {
        Command::Save { .. } | Command::GitPolicy { .. } => {
            match purlis_core::plane::resolve(&cwd) {
                Ok(root) => root,
                Err(why) => {
                    eprintln!("purlis: {why}");
                    return Some(ExitCode::FAILURE);
                }
            }
        }
        _ => return None,
    };
    let code = match command {
        Command::Save {
            message,
            sign,
            no_push,
            pull,
        } => {
            // A pull that failed, or met conflicts, stops the save: it would stage the markers.
            if *pull && purlis_core::planegit::pull(&root, &mut say) != 0 {
                1
            } else {
                // A chat's `charter save` is an agent run's, and carries its trailers (GL-8).
                let provenance =
                    purlis_core::provenance::Provenance::in_chat(&root, &purlis_core::envvar::var);
                purlis_core::planegit::save(
                    &purlis_core::planegit::Request {
                        root: &root,
                        message: message.as_deref(),
                        sign: *sign,
                        no_push: *no_push,
                        cwd: &cwd,
                        provenance: provenance.as_ref(),
                    },
                    &mut say,
                )
            }
        }
        Command::GitPolicy { apply } => purlis_core::gitpolicy::policy(&root, *apply, &mut say),
        _ => return None,
    };
    if code == 0 && matches!(command, Command::Save { .. }) {
        extensions::tell(&root, &ExtensionEvent::PlaneSaved);
    }
    Some(ExitCode::from(code))
}

/// `docs list` and `docs show`, or `None` for any other command.
///
/// **Kept apart from the `docs` that generates, and before any plane is resolved.** One
/// command describes purlis and the other describes your repos; they share a noun and
/// nothing else. `charter/cli.py` hangs all three off one parser and routes them to three
/// functions, of which only `cmd_docs` reads `config.ROOT` — so `purlis docs list` answers
/// outside a plane, and a Rust binary that resolved a plane first would refuse there.
///
/// **M2.21, and the alternative was an honest refusal.** These two were clap usage errors
/// (exit 2, the parser's own wording) for verbs the tool being replaced has: a Makefile
/// calling `purlis docs list` with a Rust `purlis` first on `$PATH` met one. The port costs
/// a vendored directory and a lookup, because `news` had already built the road — so it is
/// the port, not a sentence apologising for the gap.
fn docs_command(command: &Command) -> Option<ExitCode> {
    use purlis_core::docsrc;

    let mut say = speak;
    let code = match command {
        Command::Docs {
            what: Some(DocsCommand::List),
        } => docsrc::listing(&mut say),
        Command::Docs {
            what: Some(DocsCommand::Show { topic }),
        } => docsrc::show(topic, &mut say),
        _ => return None,
    };
    Some(ExitCode::from(code))
}

/// `discover`, `clone`, `sync`, `status` and `docs`, or `None` for any other command.
fn repo_command(command: &Command) -> Option<ExitCode> {
    use purlis_core::repocmd::{self, Say};

    let mut say = |line: Say| eprintln!("{line}");
    let here = match command {
        Command::Discover { .. }
        | Command::Clone { .. }
        | Command::Sync { .. }
        | Command::Status { .. }
        // `list` and `show` are NOT here: they read charter's own documentation, which this
        // binary carries, and asking `Here::read()` first would make them refuse outside a
        // plane where charter answers. [`docs_command`] takes them before this runs.
        | Command::Docs {
            what: None | Some(DocsCommand::Generate),
        } => match Here::read() {
            Ok(here) => here,
            Err(why) => {
                eprintln!("purlis: {why}");
                return Some(ExitCode::FAILURE);
            }
        },
        _ => return None,
    };
    let root = here.plane.root().to_path_buf();
    let code = match command {
        Command::Discover { no_probe, no_docs } => repocmd::discover::discover(
            &root,
            repocmd::discover::Options {
                no_probe: *no_probe,
                no_docs: *no_docs,
            },
            &mut say,
        ),
        Command::Clone {
            repos,
            workspace,
            now,
        } => {
            let on_a_test_clock = now.is_some();
            let now = match now {
                Some(text) => match text.parse::<chrono::NaiveDateTime>() {
                    // A naive stamp is LOCAL time, as `--now` is everywhere in this binary.
                    Ok(naive) => {
                        match chrono::TimeZone::from_local_datetime(&chrono::Local, &naive).single()
                        {
                            Some(local) => local.with_timezone(&chrono::Utc),
                            None => {
                                eprintln!("purlis: --now names no single local instant");
                                return Some(ExitCode::FAILURE);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("purlis: --now is not a local naive timestamp: {e}");
                        return Some(ExitCode::FAILURE);
                    }
                },
                None => chrono::Utc::now(),
            };
            // Who a manifest says last touched it. Python's `_author`: `$USER`, and a word
            // that says nobody knows rather than an empty field.
            let author = std::env::var("USER")
                .ok()
                .filter(|u| !u.is_empty())
                .unwrap_or_else(|| "unknown".to_string());
            // With no `-w`, the ladder: "default: the active one" is what charter's own
            // `--workspace` help has always promised for this command.
            let ws = match here.active_workspace(workspace.as_deref()) {
                Ok(ws) => ws,
                Err(why) => return Some(refused(&why)),
            };
            // In a chat the app started, the app clones (#1335): a sandboxed chat may not
            // write the clone's `.git/config`, hooks or editor settings. `--now` is a test's
            // clock, which only this process keeps.
            let asked = if on_a_test_clock {
                None
            } else {
                gitask::told(
                    gitask::forwarded(
                        &ws,
                        purlis_core::hookwire::GitWork::Clone {
                            repos: repos.clone(),
                        },
                        gitask::A_CLONE_IS_SILENT_AT_MOST,
                    ),
                    &mut say,
                )
            };
            if let Some(code) = asked {
                return Some(ExitCode::from(code));
            }
            repocmd::clone::clone(
                &repocmd::clone::Request {
                    root: &root,
                    ws: &ws,
                    repos,
                    now,
                    author: &author,
                    hosts: None,
                    watch: None,
                },
                &mut say,
            )
        }
        Command::Sync { workspace, all } => {
            // `--all` is the only thing that replaces the ladder here, and clap already
            // refuses it beside `-w`.
            let one = match (!*all).then(|| here.active_workspace(workspace.as_deref())) {
                Some(Err(why)) => return Some(refused(&why)),
                Some(Ok(ws)) => Some(ws),
                None => None,
            };
            let scope = match &one {
                Some(ws) => repocmd::sync::Scope::One(ws),
                None => repocmd::sync::Scope::All,
            };
            repocmd::sync::sync(&root, scope, &mut say)
        }
        Command::Status { workspace, all } => {
            // Resolved ONCE, and the sentence naming the rung comes off the same answer: a
            // header that named the workspace from one reading and the reason from another
            // would explain it by naming a rung that did not decide it.
            let asking = here.asking(workspace.as_deref(), here.workspace_env.as_deref());
            let chosen = purlis_core::active::workspace(&asking);
            let via = purlis_core::active::workspace_source(&here.ids, chosen.rung);
            // A plane-root chat is in no workspace: every workspace is reported, and none is
            // marked as the one it is in (SI-1). `(none)` can be no workspace's name.
            let at_root = here.plane_root(workspace.as_deref()).is_some();
            let (active, via) = if at_root {
                ("(none)", "the plane root".to_string())
            } else {
                (chosen.name.as_str(), via)
            };
            let mut out = |line: String| println!("{line}");
            repocmd::status::status(
                &repocmd::status::Request {
                    root: &root,
                    cwd: &here.cwd,
                    active,
                    via: &via,
                    all: *all || at_root,
                },
                &mut out,
                &mut say,
            )
        }
        // Bare `charter docs` and `charter docs generate` are the same command.
        Command::Docs {
            what: None | Some(DocsCommand::Generate),
        } => {
            let cfg = match purlis_core::forge::load_config(&root) {
                Ok(cfg) => cfg,
                Err(why) => {
                    eprintln!("{}", Say::Plain(why.into()));
                    return Some(ExitCode::FAILURE);
                }
            };
            repocmd::docs::docs(&root, &purlis_core::forge::group_of(&cfg, 0), &mut say)
        }
        _ => return None,
    };
    Some(ExitCode::from(code))
}

/// The workspace verbs that act on a workspace as a whole, or `None` for any other command.
///
/// Answered here rather than in [`run`] for the reason the repo commands are: each says
/// several lines as it goes and chooses its own exit status — `remove`'s guard is exit **2**,
/// which is neither a success nor the failure a bad name gets.
fn workspace_command(command: &Command) -> Option<ExitCode> {
    use purlis_core::wscmd;

    let verb = match command {
        Command::Workspace(verb) => verb,
        _ => return None,
    };
    // Only the verbs below; everything else stays with `run`.
    if !matches!(
        verb,
        WorkspaceCommand::Remove { .. }
            | WorkspaceCommand::Rename { .. }
            | WorkspaceCommand::Live { .. }
            | WorkspaceCommand::Use { .. }
            | WorkspaceCommand::Unlock
            | WorkspaceCommand::Default { .. }
            | WorkspaceCommand::Snapshot { .. }
            | WorkspaceCommand::Create { .. }
            | WorkspaceCommand::Fork { .. }
            | WorkspaceCommand::Restore { .. }
            | WorkspaceCommand::Reinit { .. }
    ) {
        return None;
    }
    // The verbs that rewrite a workspace's `workspace.json` are not a sandboxed chat's (#2046).
    let rewrites = match verb {
        WorkspaceCommand::Rename { .. } => Some("workspace rename"),
        WorkspaceCommand::Fork { .. } => Some("workspace fork"),
        _ => None,
    };
    if let Some(why) = rewrites.and_then(brokered::manifest_refused) {
        return Some(refused(&why));
    }
    let here = match Here::read() {
        Ok(here) => here,
        Err(why) => {
            eprintln!("purlis: {why}");
            return Some(ExitCode::FAILURE);
        }
    };
    let root = here.plane.root().to_path_buf();
    let mut sink = speak;
    let say: &mut dyn FnMut(purlis_core::repocmd::Say) = &mut sink;
    // What the extensions that hear it are told once the verb is done and has said everything
    // it says (charter-app#343).
    let mut heard: Vec<ExtensionEvent> = Vec::new();
    let code = match verb {
        WorkspaceCommand::Remove { name, force } => {
            // Never from a chat the app started under the sandbox (#1408, D-qw301-1): refused
            // before the guard and the delete run. The app that started the chat is handed the
            // name, and asks the person in its window (qw356); the sentence says which.
            let target = wscmd::workspace_dir(&root, name);
            if brokered::removal_held(&root, target.as_deref()) {
                return Some(refused(&brokered::removal_asked_of_the_person(name)));
            }
            // The list it refused on is the window's (charter-app#182): a terminal has already
            // been given every sentence in it, by the refusal itself.
            let code = wscmd::remove::remove(&root, name, *force, say).code;
            // The active workspace followed the removal: a pointer naming a workspace that is
            // gone resolves to it on every later command, and `workspaces/<gone>` is then
            // created again by the first write. Python resets it the same way and for the
            // same reason; the rung it tests for is spelled here as the two pointer rungs,
            // which are `session`/`active-file` in charter's own vocabulary.
            if code == 0 {
                let reset = reset_active_after_removal(&here, name, say);
                heard.push(ExtensionEvent::WorkspaceRemoved {
                    workspace: name.clone(),
                });
                reset
            } else {
                code
            }
        }
        WorkspaceCommand::Rename { old, new } => {
            // The chats the app has open in it, under either name so a rename finished after
            // a crash is guarded too. A terminal cannot see into the app, so it reads the
            // record the app keeps of what it has open, while an app is listening.
            let running = wscmd::rename::open_in_app(&root, &[old.as_str(), new.as_str()]);
            let config_root = purlis_core::machine::config_root();
            wscmd::rename::rename(
                &wscmd::rename::Request {
                    root: &root,
                    old,
                    new,
                    running: &running,
                    config_root: config_root.as_deref(),
                },
                say,
            )
        }
        WorkspaceCommand::Live { name, off } => wscmd::live::live(&root, name, *off, say),
        WorkspaceCommand::Use {
            name,
            create,
            force,
            now,
        } => {
            let Some(now) = pinned(now) else {
                return Some(ExitCode::FAILURE);
            };
            if here.launched_at_plane_root() {
                return Some(refused(&format!(
                    "this chat was started at the plane root and stays there, so it is not \
                     moved into '{name}'. Name the workspace on each command with -w {name}, or \
                     start a chat in it."
                )));
            }
            // `--create` makes one that is not there, which is a workspace being created too.
            let there_before = Plane::open(&root)
                .workspace(name)
                .is_ok_and(|ws| ws.dir().exists());
            let code =
                wscmd::select::use_workspace(&root, name, &here.ids, *create, *force, now, say);
            if code == 0 {
                if !there_before {
                    heard.push(ExtensionEvent::WorkspaceCreated {
                        workspace: name.clone(),
                    });
                }
                heard.push(ExtensionEvent::WorkspaceFocused {
                    workspace: name.clone(),
                });
            }
            code
        }
        WorkspaceCommand::Create {
            name,
            vision,
            live,
            use_it,
            force,
            repos,
            now,
        } => 'create: {
            // In a chat the app started, the app makes the workspace (ADR 0067 §2, #1408), as
            // it makes `handoff --create`'s: a new workspace and its vision, and with repos the
            // clones too, in one ask (qw387). What the app does not do for a sandboxed chat is
            // refused before anything is made; a pinned clock is a test's, and runs here.
            if now.is_none() {
                if brokered::sandboxed()
                    && let Some(why) = brokered::create_refused(name, *live, *use_it || *force)
                {
                    return Some(refused(&why));
                }
                let asked = if *live || *use_it || *force {
                    // Not sandboxed: the chat writes these itself, as a terminal does.
                    None
                } else if repos.is_empty() {
                    brokered::said_by_the_app(purlis_core::brokered::Write::WorkspaceCreate {
                        name: name.clone(),
                        vision: vision.clone(),
                    })
                } else {
                    gitask::told(
                        gitask::forwarded(
                            name,
                            purlis_core::hookwire::GitWork::Create {
                                vision: vision.clone(),
                                repos: repos.clone(),
                            },
                            gitask::A_CLONE_IS_SILENT_AT_MOST,
                        ),
                        say,
                    )
                };
                if let Some(code) = asked {
                    if code == 0 {
                        heard.push(ExtensionEvent::WorkspaceCreated {
                            workspace: name.clone(),
                        });
                    }
                    break 'create code;
                }
            }
            let Some(now) = pinned(now) else {
                return Some(ExitCode::FAILURE);
            };
            if *use_it && here.launched_at_plane_root() {
                return Some(refused(&format!(
                    "this chat was started at the plane root and stays there, so nothing was \
                     created: --use would move it into '{name}'. Create it without --use, and \
                     name it with -w {name} from here."
                )));
            }
            let owner = format!("workspaces/{name}");
            let was_there = purlis_core::skills::levels::there(&root, &owner);
            let code = wscmd::create::create(
                &wscmd::create::Request {
                    root: &root,
                    name,
                    vision: vision.as_deref(),
                    live: *live,
                    use_it: *use_it,
                    force: *force,
                    repos,
                    hosts: None,
                    now,
                    ids: &here.ids,
                },
                say,
            );
            // The person made it from their own terminal, not a chat: its skills reach chats
            // (SK-9). A chat's own create vouches for nothing.
            if code == 0
                && !crate::memory::in_a_chat()
                && let Some(note) =
                    purlis_core::skills::levels::vouch_if_made(&root, &owner, was_there)
            {
                say(purlis_core::repocmd::Say::Info(note.into()));
            }
            if code == 0 {
                heard.push(ExtensionEvent::WorkspaceCreated {
                    workspace: name.clone(),
                });
            }
            code
        }
        WorkspaceCommand::Restore {
            name,
            on_demand,
            now,
        } => {
            let Some(now) = pinned(now) else {
                return Some(ExitCode::FAILURE);
            };
            wscmd::restore::restore(
                &wscmd::restore::Request {
                    root: &root,
                    ws: name,
                    on_demand: *on_demand,
                    now,
                },
                say,
            )
        }
        WorkspaceCommand::Fork {
            src,
            new,
            restore,
            live,
            now,
        } => {
            let Some(now) = pinned(now) else {
                return Some(ExitCode::FAILURE);
            };
            // A fork that could not read every piece still exists and exits 1 (charter#1084),
            // so whether one was made is asked of the disk: there before, it is not this one.
            let made_here = || {
                purlis_core::workspaces::Plane::open(&root)
                    .workspace(new)
                    .is_ok_and(|ws| ws.dir().exists())
            };
            let there_before = made_here();
            let extension_folders = extensions::carried();
            let code = wscmd::fork::fork(
                &wscmd::fork::Request {
                    root: &root,
                    src,
                    new,
                    live: *live,
                    restore: *restore,
                    now,
                    extension_folders: &extension_folders,
                },
                say,
            );
            if !there_before && made_here() {
                heard.push(ExtensionEvent::WorkspaceForked {
                    workspace: new.clone(),
                    from: src.clone(),
                });
            }
            code
        }
        WorkspaceCommand::Reinit { name, all, now } => {
            let Some(now) = pinned(now) else {
                return Some(ExitCode::FAILURE);
            };
            // With no `--all` and no name, the ladder: "default: the active one", which is
            // what charter's own `reinit` resolves.
            let one = match (!*all).then(|| here.active_workspace(name.as_deref())) {
                Some(Err(why)) => return Some(refused(&why)),
                Some(Ok(ws)) => Some(ws),
                None => None,
            };
            let scope = match &one {
                Some(ws) => wscmd::reinit::Scope::One(ws),
                None => wscmd::reinit::Scope::All,
            };
            wscmd::reinit::reinit(&root, scope, now, say)
        }
        WorkspaceCommand::Unlock => wscmd::select::unlock_command(&root, &here.ids, say),
        WorkspaceCommand::Default { name, clear } => {
            wscmd::select::default_command(&root, name.as_deref(), *clear, say)
        }
        WorkspaceCommand::Snapshot {
            name,
            description,
            force,
            now,
        } => {
            let now = match now {
                Some(text) => match text.parse::<chrono::NaiveDateTime>() {
                    Ok(naive) => {
                        match chrono::TimeZone::from_local_datetime(&chrono::Local, &naive).single()
                        {
                            Some(local) => local.with_timezone(&chrono::Utc),
                            None => {
                                eprintln!("purlis: --now names no single local instant");
                                return Some(ExitCode::FAILURE);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("purlis: --now is not a local naive timestamp: {e}");
                        return Some(ExitCode::FAILURE);
                    }
                },
                None => chrono::Utc::now(),
            };
            // In a sandboxed chat the app records the chat's own workspace (#2046): its sandbox
            // writes no workspace's manifest. Another workspace's is refused.
            if brokered::sandboxed() {
                match brokered::named_here(&here, name.as_deref()) {
                    brokered::Named::Own => {
                        if let Some(code) = brokered::said_by_the_app(
                            purlis_core::brokered::Write::WorkspaceSnapshot {
                                description: description.clone(),
                                force: *force,
                            },
                        ) {
                            return Some(ExitCode::from(code));
                        }
                    }
                    brokered::Named::Here => {}
                    brokered::Named::Refused(why) => return Some(refused(&why)),
                }
            }
            wscmd::snapshot::snapshot(
                &wscmd::snapshot::Request {
                    root: &root,
                    ws: &match here.active_workspace(name.as_deref()) {
                        Ok(ws) => ws,
                        Err(why) => return Some(refused(&why)),
                    },
                    description: description.as_deref(),
                    force: *force,
                    now,
                },
                say,
            )
        }
        _ => unreachable!("filtered above"),
    };
    for event in &heard {
        extensions::tell(&root, event);
    }
    Some(ExitCode::from(code))
}

/// `--now` as an instant, or the wall clock when it was not given.
///
/// A naive stamp is LOCAL time, as `--now` is everywhere in this binary. `None` back means the
/// value was not an instant and the caller has already had the reason printed.
fn pinned(now: &Option<String>) -> Option<chrono::DateTime<chrono::Utc>> {
    let Some(text) = now else {
        return Some(chrono::Utc::now());
    };
    let naive: chrono::NaiveDateTime = match text.parse() {
        Ok(naive) => naive,
        Err(e) => {
            eprintln!("purlis: --now is not a local naive timestamp: {e}");
            return None;
        }
    };
    match chrono::TimeZone::from_local_datetime(&chrono::Local, &naive).single() {
        Some(local) => Some(local.with_timezone(&chrono::Utc)),
        None => {
            eprintln!("purlis: --now names no single local instant");
            None
        }
    }
}

/// Point this session back at the always-present workspace after the one it was on was
/// removed — `cmd_workspace_remove`'s closing branch.
///
/// Only when a POINTER is what named it. A `-w`, a `$CHARTER_WORKSPACE` or the tree the
/// caller is standing in are the operator's own and are not purlis's to rewrite; a pointer
/// purlis wrote is, and one naming a directory that no longer exists is how the next write
/// re-creates the workspace that was just deleted.
///
/// Answers the exit code: a pointer that could not be written is said as that, never as a
/// reset (#2103).
fn reset_active_after_removal(
    here: &Here,
    removed: &str,
    say: &mut dyn FnMut(purlis_core::repocmd::Say),
) -> u8 {
    use purlis_core::active::WorkspaceRung;

    let active = purlis_core::active::workspace(&here.asking(None, here.workspace_env.as_deref()));
    if active.name != removed
        || !matches!(
            active.rung,
            WorkspaceRung::SessionPointer | WorkspaceRung::TerminalPointer
        )
    {
        return 0;
    }
    let fallback = purlis_core::active::plane_default_workspace(here.plane.root());
    // `force`, because the session is locked to the workspace that just went away and that
    // lock can refuse nothing useful now.
    purlis_core::wscmd::select::reset_after_removal(
        here.plane.root(),
        removed,
        &fallback,
        &here.ids,
        say,
    )
}

fn run(command: Command) -> Result<u8, String> {
    let here = Here::read()?;
    match command {
        // Answered in `main`, before this: it is the one command whose exit code is not a
        // plain success or failure, and clap must never be allowed to exit 2 in front of it.
        Command::Hook { .. }
        | Command::Listen { .. }
        | Command::Serve { .. }
        | Command::Bridge { .. }
        | Command::Init(_)
        | Command::Reinit
        | Command::News(_)
        | Command::Theme(_)
        | Command::Stop { .. }
        | Command::Update { .. }
        | Command::Version { .. }
        | Command::Discover { .. }
        | Command::Clone { .. }
        | Command::Sync { .. }
        | Command::Status { .. }
        | Command::Docs { .. }
        | Command::Doctor { .. }
        | Command::Migrate { .. }
        | Command::GlRefresh { .. }
        | Command::Statusline { .. }
        | Command::Save { .. }
        | Command::Handoff { .. }
        | Command::Dispatch { .. }
        | Command::Report(_)
        | Command::ShellGuard { .. }
        | Command::Mcp { .. }
        | Command::GitHook { .. }
        | Command::Scan { .. }
        | Command::Workspace(WorkspaceCommand::Remove { .. })
        | Command::Workspace(WorkspaceCommand::Rename { .. })
        | Command::Workspace(WorkspaceCommand::Live { .. })
        | Command::Workspace(WorkspaceCommand::Use { .. })
        | Command::Workspace(WorkspaceCommand::Unlock)
        | Command::Workspace(WorkspaceCommand::Default { .. })
        | Command::Workspace(WorkspaceCommand::Snapshot { .. })
        | Command::Workspace(WorkspaceCommand::Create { .. })
        | Command::Workspace(WorkspaceCommand::Fork { .. })
        | Command::Workspace(WorkspaceCommand::Restore { .. })
        | Command::Workspace(WorkspaceCommand::Reinit { .. })
        | Command::Workspace(WorkspaceCommand::Reconcile)
        | Command::Workspace(WorkspaceCommand::Autosave)
        | Command::GitPolicy { .. }
        | Command::Secret(_)
        | Command::Plugin(_)
        | Command::Profile(
            ProfileCommand::Create { .. }
            | ProfileCommand::Import { .. }
            | ProfileCommand::Remove { .. }
            | ProfileCommand::Authenticate { .. }
            | ProfileCommand::SignOut { .. },
        )
        | Command::Vault(_) => {
            unreachable!("answered before run")
        }
        Command::Root => {
            println!("{}", here.plane.root().display());
        }
        Command::Propose { file, from } => {
            return brokered::propose(here.plane.root(), &file, &from);
        }
        Command::Workspace(WorkspaceCommand::Current) => {
            // The name alone on stdout, for a script; the rung that decided it on stderr, for
            // the person asking why (#999). Both off ONE resolution, as `status` does, so the
            // reason given is the rung that produced the name.
            if let Some(by) = here.plane_root(None) {
                return Err(purlis_core::active::plane_root_refusal(
                    here.plane.root(),
                    by,
                ));
            }
            let chosen =
                purlis_core::active::workspace(&here.asking(None, here.workspace_env.as_deref()));
            println!("{}", chosen.name);
            voice::info(format!(
                "via {}",
                purlis_core::active::workspace_source(&here.ids, chosen.rung)
            ));
        }
        Command::Guard { verb } => {
            use purlis_core::guardcmd::{self, Bucket};
            let root = here.plane.root().to_path_buf();
            let (pattern, bucket, local) = match &verb {
                None | Some(GuardCommand::List) => {
                    let (listing, whole) = guardcmd::list(&root);
                    print!("{listing}");
                    return Ok(u8::from(!whole));
                }
                Some(GuardCommand::Ask { pattern, local }) => {
                    (pattern.as_str(), Bucket::Ask, *local)
                }
                Some(GuardCommand::Allow { pattern, local }) => {
                    (pattern.as_str(), Bucket::Allow, *local)
                }
                Some(GuardCommand::Handoff) => {
                    eprintln!("purlis: {}", guardcmd::HANDOFF_RETIRED);
                    return Ok(2);
                }
                Some(GuardCommand::Report) => (guardcmd::REPORT_PATTERN, Bucket::Ask, false),
            };
            let rule = match guardcmd::as_rule(pattern) {
                Ok(rule) => rule,
                Err(why) => {
                    eprintln!("purlis: {why} Example: purlis guard ask 'terraform apply *'");
                    return Ok(2);
                }
            };
            let (said, code) = guardcmd::report(&root, &rule, bucket, local);
            print!("{said}");
            return Ok(code);
        }
        // Answered in `main` before a plane is even looked for.
        Command::Browser(BrowserCommand::Install { .. }) => unreachable!("answered before run"),
        Command::Harness(HarnessCommand::List) => {
            let root = here.plane.root().to_path_buf();
            // The git check runs HERE because a person typed this command; it never runs on
            // a config read, so no hook pays a git call per tool call.
            let check = profiles::ignore_check(&root);
            let set = profiles::with_ignore_check(profiles::current(&root), &check);
            eprint!("{}", harness_listing(&set, &check, None));
        }
        Command::Profile(ProfileCommand::List) => {
            let root = here.plane.root().to_path_buf();
            // As `harness list`: a person typed this, so the git check runs here.
            let check = profiles::ignore_check(&root);
            let set = profiles::with_ignore_check(profiles::current(&root), &check);
            let declared = purlis_core::harness_declaration::read(&root);
            let passed = purlis_core::chatenv::read(&root);
            // This machine's own profiles, read and never written: the app adopts the
            // machine's harness folders at its launch (#2033), from the environment its chats
            // start in, which a terminal's may not be.
            let machine = purlis_core::profilestore::listed_here();
            eprint!(
                "{}",
                harness_listing(&set, &check, Some((&declared, &passed, &machine)))
            );
        }
        Command::Harness(HarnessCommand::Show { name }) => {
            return Ok(harness_show(here.plane.root(), &name));
        }
        Command::Workspace(WorkspaceCommand::List) => {
            let mut names = here.plane.workspaces().map_err(|e| e.to_string())?;
            // The workspace `resolve` TERMINATES on is always listable, whether or not its
            // directory is there: a plane where nobody selected anything resolves to a name
            // this listing did not contain, and the table then marked no row at all
            // (charter#745). It is `config.DEFAULT_WORKSPACE` — `[workspace] default`, and
            // only the literal `default` when the plane declares none.
            let always = purlis_core::active::plane_default_workspace(here.plane.root());
            if !names.contains(&always) {
                names.push(always);
                names.sort();
            }
            for name in names {
                println!("{name}");
            }
        }
        Command::Workspace(WorkspaceCommand::Vision { text, common }) => {
            // Inside a chat the app started, the app writes it for the chat (#1384), as it does
            // `workspace remember`'s: workspace.md is not the chat's to write from its sandbox.
            // `-w <its own>` is taken as no `-w`, and another is refused in a sandboxed chat
            // (#1408).
            if let Some(text) = text.as_deref().filter(|t| !t.is_empty()) {
                match brokered::named_here(&here, common.workspace.as_deref()) {
                    brokered::Named::Own => {
                        let write = purlis_core::brokered::Write::WorkspaceVision {
                            text: text.to_owned(),
                        };
                        if let Some(code) =
                            memory::said_forwarded(crate::brokered::forwarded(write), |_, _| {})
                        {
                            return Ok(code);
                        }
                    }
                    brokered::Named::Here => {}
                    brokered::Named::Refused(why) => {
                        voice::err(why.to_string());
                        return Ok(1);
                    }
                }
            }
            let ws = here.workspace(common.workspace.as_deref())?;
            // A workspace charter does not have is not one this scaffolds: `vision` shows or
            // replaces, and inventing the directory is what made a bad `-w` silent.
            if !ws.dir().is_dir() {
                return Err(format!("no workspace '{}'", ws.name()));
            }
            match text.as_deref().filter(|t| !t.is_empty()) {
                Some(text) => {
                    ws.set_vision(text).map_err(|e| e.to_string())?;
                    voice::ok(format!(
                        "Vision set for '{0}' → workspaces/{0}/workspace.md",
                        ws.name()
                    ));
                }
                None => {
                    let vision = ws.vision();
                    if !vision.is_empty() {
                        println!("{vision}");
                    }
                }
            }
        }
        Command::Recall(args) => return memory::recall(&here, args),
        Command::Persona(command) => return memory::persona(&here, command),
        Command::Worktree(command) => return piece::run(&here, command),
        Command::Change(command) => return change::run(&here, command),
        Command::Curation(command) => return curation::run(here.plane.root(), command),
        Command::Skill(command) => return skill::run(here.plane.root(), command),
        Command::Session(command) => return session::run(&here, command),
        Command::Workspace(WorkspaceCommand::Remember {
            text,
            title,
            no_sync,
            common,
        }) => {
            return memory::workspace_remember_typed(
                &here,
                common.workspace.as_deref(),
                text.as_deref(),
                title.as_deref(),
                no_sync,
                common.now.as_deref(),
            );
        }
        Command::Workspace(WorkspaceCommand::Note {
            message,
            no_sync,
            common,
        }) => {
            return memory::workspace_remember_typed(
                &here,
                common.workspace.as_deref(),
                message.as_deref(),
                None,
                no_sync,
                common.now.as_deref(),
            );
        }
        Command::Workspace(WorkspaceCommand::Recall { query, common }) => {
            return memory::workspace_recall(
                &here.plane,
                &here.active_workspace(common.workspace.as_deref())?,
                query.as_deref(),
            );
        }
        Command::Workspace(WorkspaceCommand::Forget { slug, common }) => {
            return memory::workspace_forget(&here, common.workspace.as_deref(), &slug);
        }
        Command::Workspace(WorkspaceCommand::Edit {
            slug,
            text,
            title,
            common,
        }) => {
            let text = memory::body_arg(text)?;
            return memory::workspace_edit(
                &here,
                common.workspace.as_deref(),
                &slug,
                title.as_deref(),
                text.as_deref(),
            );
        }
        Command::Workspace(WorkspaceCommand::Archive { slug, common }) => {
            return memory::workspace_archive(&here, common.workspace.as_deref(), &slug);
        }
        Command::Workspace(WorkspaceCommand::MoveMemory { slug, to, common }) => {
            let name = here.active_workspace(common.workspace.as_deref())?;
            return memory::move_memory(
                &here,
                &purlis_core::memscope::Scope::Workspace(name),
                &slug,
                &to.scope(),
                None,
            );
        }
        Command::Workspace(WorkspaceCommand::Unarchive {
            slug,
            restore_as,
            common,
        }) => {
            return memory::workspace_unarchive(
                &here,
                common.workspace.as_deref(),
                &slug,
                restore_as.as_deref(),
            );
        }
        Command::Workspace(WorkspaceCommand::Optimize {
            name,
            // `--all` is the default and changes nothing, as in charter; taken so a script
            // that passes it is not refused.
            all: _all,
            apply,
            stale_days,
            now,
        }) => {
            return memory::workspace_optimize(
                &here,
                name.as_deref(),
                apply,
                stale_days,
                now.as_deref(),
            );
        }
        Command::Workspace(WorkspaceCommand::Todo {
            words,
            done,
            repo,
            common,
        }) => {
            // In a chat the app started, a todo for the chat's own workspace is the app's to
            // write (#1333), and so is closing one (#1408), its own named with `-w` or not; a
            // sandboxed chat naming another is refused. Listing reads; promoting opens an
            // issue as the person, and is not brokered.
            let closes = matches!(words.as_slice(), [verb, _] if todo_close(verb).is_some());
            let adds = matches!(words.as_slice(),
                [text] if !["done", "reopen", "forget", "promote"].contains(&text.as_str()));
            let own = if (closes || adds) && !done && repo.is_none() && common.now.is_none() {
                match brokered::named_here(&here, common.workspace.as_deref()) {
                    brokered::Named::Own => true,
                    brokered::Named::Here => false,
                    brokered::Named::Refused(why) => {
                        voice::err(why.to_string());
                        return Ok(1);
                    }
                }
            } else {
                false
            };
            if own
                && let [verb, slug] = words.as_slice()
                && let Some(how) = todo_close(verb)
                && let Some(code) =
                    brokered::said_by_the_app(purlis_core::brokered::Write::TodoClose {
                        how,
                        slug: slug.clone(),
                    })
            {
                return Ok(code);
            }
            if own
                && adds
                && let [text] = words.as_slice()
            {
                let write = purlis_core::brokered::Write::Todo { text: text.clone() };
                if let Some(code) =
                    memory::said_forwarded(brokered::forwarded(write), |to, path| {
                        said_todo_recorded(&here.plane, to, path);
                    })
                {
                    return Ok(code);
                }
            }
            let ws = here.workspace(common.workspace.as_deref())?;
            let stamp = common.stamp()?;
            return Ok(todo(&here.plane, &ws, &words, done, repo.as_deref(), stamp));
        }
    }
    Ok(0)
}

/// `purlis ws todo` — record one todo, list them, or close one with `done`/`forget <slug>`,
/// saying what it did as `commands_workspace.cmd_workspace_todo` says it (M8.5: the port
/// did the writes and said nothing, so an agent recording a todo had no confirmation and
/// one closing a mistyped slug could not tell it had closed nothing).
fn todo(
    plane: &Plane,
    ws: &purlis_core::workspaces::Workspace,
    words: &[String],
    done: bool,
    repo: Option<&str>,
    stamp: chrono::NaiveDateTime,
) -> u8 {
    let root = plane.root();
    let name = ws.name();
    let dir = ws.dir().join("todos");
    let see = format!("purlis ws todo --workspace {name}");
    let promoting = words.first().is_some_and(|verb| verb == "promote") && words.len() == 2;
    if repo.is_some() && !promoting {
        voice::err("--repo is taken only with `charter ws todo promote <slug>`.");
        return 1;
    }
    if done && !words.is_empty() {
        voice::err("--done lists the done todos, and is taken with nothing else.");
        voice::info(format!("  See them: {see} --done"));
        return 1;
    }
    finish_promotes(plane, ws, stamp);
    match words {
        [] if done => {
            let (closed, unread) = purlis_core::memstore::read_entries(
                root,
                &dir.join(purlis_core::memstore::ARCHIVE),
            );
            voice::unread(root, &unread);
            if closed.is_empty() {
                voice::info(format!("No done todos in '{name}'."));
                return 0;
            }
            for todo in closed {
                let stem = todo.path.file_stem().unwrap_or_default().to_string_lossy();
                println!("  {stem}  {}", purlis_core::personas::one_line(&todo.title));
            }
            voice::info("  Reopen one: purlis ws todo reopen <slug>");
            0
        }
        [verb, slug] if verb == "promote" => promote_todo(plane, ws, slug, repo, stamp),
        [] => {
            let (open, unread) = purlis_core::memstore::read_entries(root, &dir);
            voice::unread(root, &unread);
            if open.is_empty() {
                voice::info(format!(
                    "No open todos in '{name}'. Record one: purlis ws todo \"<what>\""
                ));
                return 0;
            }
            // Oldest first, with its age: what surfaces is what is being avoided.
            let today = stamp.date();
            for todo in open {
                let file = todo.path.file_name().unwrap_or_default().to_string_lossy();
                let stem = todo.path.file_stem().unwrap_or_default().to_string_lossy();
                let age = purlis_core::memstore::memory_date(&todo.text, &file)
                    .map_or(0, |d| (today - d).num_days());
                println!(
                    "  {stem}  {age}d  {}",
                    purlis_core::personas::one_line(&todo.title)
                );
            }
            0
        }
        [verb, slug] if let Some(how) = todo_close(verb) => {
            purlis_core::wscmd::todo::close(plane, ws, how, slug, stamp, &mut speak)
        }
        // A lone verb is NOT todo text. charter refuses it, and the reason is that
        // `todo done` with a forgotten slug would otherwise record a todo called "done" and
        // leave the one it meant to close open.
        [verb] if verb == "promote" => {
            voice::err("`todo promote` needs the slug of the todo to promote.");
            voice::info(format!("  The slug is the first column: {see}"));
            voice::info(
                "  To record a todo actually called \"promote\", capitalise it or add a word: \
                 purlis ws todo \"Promote …\"",
            );
            1
        }
        [verb] if todo_close(verb).is_some() => {
            let to = if verb == "reopen" { "reopen" } else { "close" };
            voice::err(format!("`todo {verb}` needs the slug of the todo to {to}."));
            voice::info(format!("  The slug is the first column: {see}"));
            let capital: String = verb
                .chars()
                .take(1)
                .flat_map(char::to_uppercase)
                .chain(verb.chars().skip(1))
                .collect();
            voice::info(format!(
                "  To record a todo actually called \"{verb}\", capitalise it or add a word: \
                 purlis ws todo \"{capital} …\""
            ));
            1
        }
        [text] => {
            // The rule is the core's (`Workspace::record_todo`), so the window's Todos panel
            // refuses the same todos in the same words.
            match ws.record_todo(text, stamp) {
                Ok(path) => {
                    said_todo_recorded(plane, name, &path.to_string_lossy());
                    0
                }
                Err(refused @ purlis_core::workspaces::RecordRefused::AlreadyListed(_)) => {
                    voice::err(refused.to_string());
                    voice::info(format!("  See it: {see}"));
                    1
                }
                Err(refused) => {
                    voice::err(refused.to_string());
                    1
                }
            }
        }
        _ => {
            voice::err(
                "usage: purlis ws todo [-w WS] [\"<text>\" | done <slug> | reopen <slug> | \
                 forget <slug> | promote <slug> [--repo <repo>]] [--done]",
            );
            1
        }
    }
}

/// What `ws todo "<text>"` says once the todo is recorded in workspace `name`, at `path`.
fn said_todo_recorded(plane: &Plane, name: &str, path: &str) {
    voice::ok(format!(
        "Todo recorded in '{name}' → workspaces/{name}/todos/{}",
        std::path::Path::new(path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
    ));
    if !plane.is_live(name) {
        voice::info(format!(
            "  '{name}' is LOCAL (private) — todos stay on disk, not committed."
        ));
    }
}

/// Finish closing every todo of `ws` a promote already aliased to an issue, when a crash cut the
/// promote short after its alias (ADR 0088 §5): the core's, which the app runs for a chat too.
fn finish_promotes(
    plane: &Plane,
    ws: &purlis_core::workspaces::Workspace,
    stamp: chrono::NaiveDateTime,
) {
    purlis_core::wscmd::todo::finish_promotes(plane, ws, stamp, &mut speak);
}

/// How `ws todo <verb> <slug>` changes a todo, when `verb` is `done`, `reopen` or `forget`.
fn todo_close(verb: &str) -> Option<purlis_core::wscmd::todo::Close> {
    use purlis_core::wscmd::todo::Close;
    [Close::Done, Close::Reopen, Close::Forget]
        .into_iter()
        .find(|how| how.verb() == verb)
}

/// `charter ws todo promote <slug> [--repo <repo>]` (ADR 0088 §5): name the repo and whether it
/// is public, open the issue as the operator, alias the todo to it, and close the todo.
fn promote_todo(
    plane: &Plane,
    ws: &purlis_core::workspaces::Workspace,
    slug: &str,
    repo: Option<&str>,
    stamp: chrono::NaiveDateTime,
) -> u8 {
    use purlis_core::work::promote::{self, Step};
    let root = plane.root();
    let slug = purlis_core::memstore::py_strip(slug);
    if !purlis_core::contain::segment_ok(slug) {
        voice::err(purlis_core::repocmd::clone::not_a_segment(slug).to_string());
        return 1;
    }
    let target = match promote::target(root, ws.name(), repo) {
        Ok(target) => target,
        Err(why) => {
            voice::err(why.to_string());
            return 1;
        }
    };
    let device = match purlis_core::machine::this_device_id() {
        Ok(device) => device,
        Err(why) => {
            voice::err(format!(
                "this device has no id, so the work link log has no file to write: {why}"
            ));
            return 1;
        }
    };
    let backend = target.forge.backend();
    let mut tell = |step: Step<'_>| match step {
        Step::Sending {
            target,
            about,
            label,
        } => {
            let label = match label {
                Some(label) => format!(", labelled {label}"),
                None => ", with no workspace label".to_string(),
            };
            voice::info(format!(
                "Opening an issue in {} ({} at {}, {}) with the todo's title and text{label}.",
                target.name,
                target.path(),
                target.forge.host,
                about.visibility.readers()
            ));
        }
        Step::Created(item) => voice::info(format!("  Opened {} — {}", item.key, item.url)),
    };
    match promote::promote(
        ws,
        slug,
        &target,
        backend.as_ref(),
        &purlis_core::forge::Caller::command(),
        &device,
        stamp,
        &mut tell,
    ) {
        Ok(done) => {
            // The close is `ws todo done`'s, so it says what `done` says about the journal.
            memory::said_remembered(plane, ws.name(), &done.journal, false);
            voice::ok(format!(
                "Promoted '{}' in '{}' to {} — the todo is closed, and its work links now reach \
                 the issue.",
                done.title,
                ws.name(),
                done.item.key
            ));
            0
        }
        Err(why) => {
            voice::err(why.to_string());
            1
        }
    }
}

/// This machine, as `purlis plugin` and `purlis doctor --fix` act on it: this binary by its
/// resolved path, and the plugin from `from` or else the one the app ships beside it.
fn plugin_machine(
    from: Option<&std::path::Path>,
) -> Result<purlis_core::plugin_install::Machine, String> {
    // The core's, so `charter plugin install` and the doctor's `plugin-install` fix build one
    // machine the one way.
    purlis_core::doctor::fix::this_machine(from)
}

/// `purlis profile create|import|remove|sign-in` (#2034, #2035, ADR 0095): this machine's own
/// profiles, in its data home ([`purlis_core::profilestore`]). Each is the person's act, so a
/// chat is refused it, as the window serves the same acts to itself alone. What it did, and a
/// refusal, go to stderr, as purlis's reports do.
fn machine_profile(verb: &ProfileCommand) -> ExitCode {
    use purlis_core::profilestore::{self, Homes, MachineSettings};
    if purlis_core::envvar::var(purlis_core::hookwire::CHAT_ENV)
        .is_some_and(|chat| !chat.trim().is_empty())
    {
        return refused(
            "a chat may not make, import, remove or sign in a profile: that is yours, from \
             Settings › Profiles in the window or from your own terminal",
        );
    }
    let Some(store) = Homes::here() else {
        return refused(
            "purlis cannot tell where its data home is, or it is inside a project or a git \
             work tree, so no profile was changed",
        );
    };
    let home = purlis_core::profiles::home().unwrap_or_default();
    let env = |name: &str| std::env::var(name).ok();
    let machine = MachineSettings {
        home: &home,
        env: &env,
    };
    let now = purlis_core::halt::now();
    let said = match verb {
        ProfileCommand::Create {
            name,
            harness,
            sandbox_only,
        } => {
            let made = if *sandbox_only {
                profilestore::create_sandbox_only(&store, name, harness, now)
            } else {
                profilestore::create(&store, name, harness, &machine, now)
            };
            made.map(|made| {
                format!(
                    "made the profile '{}' ({}) in {}\n{}sign it in through its harness's own \
                     sign-in with purlis profile sign-in {}\n",
                    made.record.name,
                    made.record.harness,
                    store.folder_of(&made.record.name).display(),
                    imported_lines(&made),
                    made.record.name
                )
            })
        }
        ProfileCommand::Import { name } => {
            profilestore::import(&store, name, &machine, now).map(|made| {
                format!(
                    "imported the settings of '{}' again\n{}",
                    made.record.name,
                    imported_lines(&made)
                )
            })
        }
        ProfileCommand::Remove { name, yes } => match profilestore::removal(&store, name) {
            Err(why) => Err(why),
            Ok(removal) if !yes => {
                let mut out = format!("{}\n", removal.says());
                for one in removal.conversations.iter().flatten() {
                    out.push_str(&format!("  {one}\n"));
                }
                out.push_str(&format!(
                    "nothing was removed: to remove it, run purlis profile remove {} --yes\n",
                    removal.name
                ));
                eprint!("{out}");
                return ExitCode::FAILURE;
            }
            Ok(removal) => profilestore::remove(&store, name, &removal).map(|gone| {
                if gone.adopted {
                    format!(
                        "forgot the profile '{}'; {} stays\n",
                        gone.name,
                        gone.folder.display()
                    )
                } else {
                    format!(
                        "removed the profile '{}' and deleted {}\n",
                        gone.name,
                        gone.folder.display()
                    )
                }
            }),
        },
        ProfileCommand::Authenticate {
            name,
            check,
            provider,
            pool,
            until_input_ends,
            in_own_terminal,
            way,
        } => {
            if let Some(pool) = pool {
                return match pool_sign_in(&store, name, pool, provider.as_deref()) {
                    Ok(()) => ExitCode::SUCCESS,
                    Err(why) => refused(&why),
                };
            }
            use purlis_core::profilestore::sign_in::Way;
            let way = match way.as_deref().map(Way::named) {
                None => Way::Asked,
                Some(Some(way)) => way,
                Some(None) => return refused("--way is browser or device"),
            };
            let held = match in_own_terminal {
                Some(size) => match terminal_size(size) {
                    Ok(size) => Held::InOwnTerminal(size),
                    Err(why) => return refused(&why),
                },
                None if *until_input_ends => Held::UntilInputEnds,
                None => Held::No,
            };
            return profile_sign_in(&store, name, *check, provider.as_deref(), held, way);
        }
        ProfileCommand::SignOut { name } => return profile_sign_out(&store, name),
        ProfileCommand::List => unreachable!("listed in run"),
    };
    match said {
        Ok(said) => {
            eprint!("{said}");
            ExitCode::SUCCESS
        }
        Err(why) => refused(&why),
    }
}

/// `purlis profile sign-in <name>` (#2035, ADR 0095 §3): the harness's own sign-in, run in this
/// terminal with the profile's folder variables set, to `provider` for opencode, its input and
/// output the person's own and none of it read; or, with `check`, whether its harness says it
/// is signed in here. On a runner, a profile it keeps for its desktop with a pin is signed in
/// and asked by that version, fetched first where it is not installed yet, as a chat's start
/// fetches it, with a line in this terminal saying so (#4007); a fetch that is refused says why
/// and signs nothing in. Held to its input ([`Held`]), the login ends with it (#4043).
fn profile_sign_in(
    store: &purlis_core::profilestore::Homes,
    name: &str,
    check: bool,
    provider: Option<&str>,
    held: Held,
    way: purlis_core::profilestore::sign_in::Way,
) -> ExitCode {
    use purlis_core::profilestore::{self, installs, sign_in};
    let record = match profilestore::list(store.config_root()) {
        Ok(records) => records.into_iter().find(|p| p.name == name),
        Err(why) => return refused(&why),
    };
    let Some(record) = record else {
        return refused(&format!(
            "this machine has no profile named '{}'. purlis profile list names the ones it has.",
            shown::short(name)
        ));
    };
    // Only a desktop drives a sign-in held to its input, on a runner that keeps the profile.
    if !matches!(held, Held::No) && record.kept_for.is_none() {
        return refused(&format!(
            "'{}' is not kept on this device for a desktop, so purlis holds no sign-in of it \
             here: run purlis profile sign-in {} without those options",
            record.name, record.name
        ));
    }
    let here = profilestore::this_device();
    // Watched through a fetch too, so a sign-in cancelled meanwhile starts no login.
    let input = (!matches!(held, Held::No)).then(held_input);
    let pinned = match installs::on_this_runner(&record, &|line| eprintln!("{line}")) {
        Ok(pinned) => pinned,
        Err(why) if check => {
            return refused(&format!(
                "{why}. purlis cannot tell whether '{}' is signed in on {here}.",
                record.name
            ));
        }
        Err(why) => return refused(&format!("{why}. Nothing was signed in.")),
    };
    if check {
        let said = match sign_in::standing_of(&record, &sign_in::ThisMachine) {
            None => format!(
                "'{}' is signed in by {}'s own sign-in on {here}",
                record.name, record.title
            ),
            Some(sign_in::Standing::SignedIn) => {
                format!("'{}' is signed in on {here}", record.name)
            }
            Some(sign_in::Standing::SignedOut) => {
                eprintln!(
                    "'{}' is not signed in on {here}: purlis profile sign-in {} signs it in",
                    record.name, record.name
                );
                return ExitCode::FAILURE;
            }
            Some(sign_in::Standing::Fetching) => format!(
                "its harness is still being fetched on {here}, so it cannot yet say whether '{}' \
                 is signed in; purlis profile sign-in {} signs it in",
                record.name, record.name
            ),
            Some(sign_in::Standing::CannotTell) => format!(
                "its harness cannot tell whether '{}' is signed in on {here}; a chat on it \
                 starts, and purlis profile sign-in {} signs it in",
                record.name, record.name
            ),
        };
        eprintln!("{said}");
        return ExitCode::SUCCESS;
    }
    let login = match sign_in::login_by(&record, provider, pinned.as_deref(), way) {
        Ok(login) => login,
        Err(why) => return refused(&why),
    };
    eprintln!(
        "signing '{}' in on {here} through its harness's own sign-in; it keeps what it signs \
         in with in {}",
        record.name,
        record.folders.config.display()
    );
    if let Some(input) = input {
        return held_login(&login, held, input, &record.harness, provider);
    }
    let ran = std::process::Command::new(&login.program)
        .args(&login.args)
        .envs(login.env.iter().map(|(n, v)| (n, v)))
        .status();
    match ran {
        Ok(status) if status.success() => ExitCode::SUCCESS,
        Ok(status) => {
            eprintln!(
                "purlis: the harness's sign-in ended without signing '{}' in ({status})",
                record.name
            );
            ExitCode::FAILURE
        }
        Err(e) => refused(&format!("{} could not be run: {e}", login.program)),
    }
}

/// `purlis profile sign-in <name> --pool <pool>` (#4046, spec #4023): the sign-in the window's
/// strip drives ([`sign_in::driven`]), run on pool `pool`'s runner through this machine's `docker`
/// and followed here ([`sign_in::terminal`]), for a profile the runner already keeps. Only a pool
/// shown to be the person's own is signed in on, as the window's is. Nothing of this machine's
/// sign-in goes to the runner (ADR 0095).
///
/// [`sign_in::driven`]: purlis_core::profilestore::sign_in::driven
/// [`sign_in::terminal`]: purlis_core::profilestore::sign_in::terminal
fn pool_sign_in(
    store: &purlis_core::profilestore::Homes,
    name: &str,
    pool: &str,
    provider: Option<&str>,
) -> Result<(), String> {
    use purlis_core::dockerpool::Docker;
    use purlis_core::profilestore::{self, sign_in};
    use purlis_core::runners;
    let config = store.config_root();
    let record = profilestore::list(config)?
        .into_iter()
        .find(|p| p.name == name)
        .ok_or_else(|| {
            format!(
                "this machine has no profile named '{}'. purlis profile list names the ones it \
                 has.",
                shown::short(name)
            )
        })?;
    sign_in::plan_named(&record.harness, provider)?;
    let plans = sign_in::providers(&record.harness);
    if provider.is_none() && !plans.is_empty() {
        let named: Vec<String> = plans
            .iter()
            .map(|p| format!("--provider {} ({})", p.provider, p.title))
            .collect();
        return Err(format!(
            "'{}' signs in to the plan you name: add {}.",
            record.name,
            named.join(" or ")
        ));
    }
    // A login approved in the browser waits for an answer only the window carries (#4044).
    if sign_in::driven::Driven::of_plan(&record.harness, provider, std::time::Instant::now())
        .is_none()
        || sign_in::driven::waits_for_the_browser(&record.harness, provider)
    {
        return Err(format!(
            "purlis follows only Claude Code's and opencode's Copilot sign-in on a pool so far. \
             Sign '{}' in from its line in Settings › Pools, with a project open: its Sign in \
             opens a shell tab on the runner.",
            record.name
        ));
    }
    // Nothing reaches a runner, nor is read of one, before its engine is shown to be yours.
    let engine = Docker::engine_on_this_machine();
    purlis_core::poollogins::owned(engine.clone())?;
    let engine = engine?;
    if runners::cluster_runner_for(config, pool)
        .map_err(|why| why.to_string())?
        .is_some()
    {
        return Err(format!(
            "pool {pool} is a cluster pool, and purlis signs a profile in only on a Docker \
             pool's runner so far."
        ));
    }
    let runner = runners::runner_for(config, pool)
        .map_err(|why| format!("{why}. Settings › Pools lists this machine's pools."))?;
    let path = purlis_core::envvar::var("PATH");
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
    let docker = Docker::on_this_machine(path.as_deref(), home.as_deref()).map_err(|why| why.0)?;
    // A runner on another version has no sign-in held to what drives it (#4043): refused in
    // words, as the window refuses it.
    let ours = purlis_core::adopt::release_version();
    match docker.version_of(&runner.made) {
        Ok(theirs) if theirs == ours => {}
        Ok(theirs) => {
            return Err(format!(
                "pool {pool}'s runner runs purlis {theirs}, and this runs {ours}, so purlis signs \
                 nothing in there itself. Sign '{name}' in from its line in Settings › Pools with \
                 Open in terminal, or once the runner runs this version."
            ));
        }
        Err(why) => {
            return Err(format!(
                "pool {pool}'s runner's version could not be read, so it may not run this \
                 purlis {ours}: {}. Nothing was started.",
                why.0
            ));
        }
    }
    eprintln!("asking pool {pool}'s runner whether it keeps '{name}'…");
    docker
        .standing_on_runner(&runner.made, &engine, name)
        .map_err(|why| {
            format!(
                "pool {pool}'s runner could not say whether '{name}' is signed in there, so \
                 nothing was started: {why}. A profile goes on a pool from its row in Settings \
                 › Pools."
            )
        })?;
    let login = docker.driven_sign_in(&runner.made, &engine, name, provider)?;
    let check = {
        let (docker, made, engine, name) = (
            docker.clone(),
            runner.made.clone(),
            engine.clone(),
            name.to_owned(),
        );
        move || {
            docker
                .standing_on_runner(&made, &engine, &name)
                .unwrap_or(sign_in::Standing::CannotTell)
        }
    };
    let driving = sign_in::driven::drive_with(&login, &record.harness, provider, None, check)?;
    let (typed, lines) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for line in std::io::stdin().lines().map_while(Result::ok) {
            if typed.send(line).is_err() {
                return;
            }
        }
    });
    sign_in::terminal::follow(&driving, &lines, &mut ThisTerminal, name, pool)
}

/// This command's terminal, as a sign-in on a pool's runner says its steps there.
struct ThisTerminal;

impl purlis_core::profilestore::sign_in::terminal::Terminal for ThisTerminal {
    fn say(&mut self, line: &str) {
        eprintln!("{line}");
    }

    /// The browser, where this system opens one. Where it does not, the page was said.
    fn open(&mut self, url: &str) {
        let program = if cfg!(target_os = "macos") {
            "/usr/bin/open"
        } else {
            "xdg-open"
        };
        let _ = std::process::Command::new(program)
            .arg(url)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
    }
}

/// How a sign-in a desktop drives is held to this command's input, which ends it (#4043).
enum Held {
    No,
    UntilInputEnds,
    InOwnTerminal(purlis_core::engine::Size),
}

/// The largest terminal a sign-in is drawn in.
const MOST_TERMINAL: (u16, u16) = (500, 200);

/// `COLUMNSxROWS`, as `--in-own-terminal` is given it, no larger than [`MOST_TERMINAL`].
fn terminal_size(said: &str) -> Result<purlis_core::engine::Size, String> {
    said.split_once('x')
        .and_then(|(columns, rows)| {
            let (columns, rows) = (columns.parse().ok()?, rows.parse().ok()?);
            ((1..=MOST_TERMINAL.0).contains(&columns) && (1..=MOST_TERMINAL.1).contains(&rows))
                .then_some(purlis_core::engine::Size { columns, rows })
        })
        .ok_or_else(|| format!("{} is not a terminal's size", shown::short(said)))
}

/// This command's input, watched for what a held login does with it.
#[cfg(unix)]
fn held_input() -> purlis_core::profilestore::sign_in::held::Input {
    purlis_core::profilestore::sign_in::held::Input::of(std::io::stdin())
}

/// A runner is never Windows.
#[cfg(not(unix))]
fn held_input() {}

/// Runs `login` held to `input`, this command's own, as `held` says: nothing where it closed
/// already.
#[cfg(unix)]
fn held_login(
    login: &purlis_core::profilestore::sign_in::Login,
    held: Held,
    input: purlis_core::profilestore::sign_in::held::Input,
    kind: &str,
    plan: Option<&str>,
) -> ExitCode {
    use purlis_core::profilestore::sign_in::held::{
        in_own_terminal, until_answered_or_input_ends, until_input_ends,
    };
    use purlis_core::profilestore::sign_in::{driven, waiting};
    if input.closed_by_now() {
        return ExitCode::FAILURE;
    }
    let ended = match held {
        Held::InOwnTerminal(size) => in_own_terminal(login, size, input, std::io::stdout()),
        // Only a login waiting for the browser's answer is handed one here (#4044).
        _ if driven::waits_for_the_browser(kind, plan) => {
            let followed = driven::Driven::of_plan(kind, plan, std::time::Instant::now());
            match (followed, waiting::kept_here()) {
                (Some(followed), Some(kept_in)) => until_answered_or_input_ends(
                    login,
                    followed,
                    &kept_in,
                    input,
                    std::io::stdout(),
                    std::io::stderr(),
                ),
                _ => Err("this runner has no data home to note this sign-in in".to_owned()),
            }
        }
        _ => until_input_ends(login, input),
    };
    match ended {
        Ok(Some(0)) => ExitCode::SUCCESS,
        Ok(_) => ExitCode::FAILURE,
        Err(why) => refused(&why),
    }
}

/// A runner is never Windows.
#[cfg(not(unix))]
fn held_login(
    _: &purlis_core::profilestore::sign_in::Login,
    _: Held,
    _: (),
    _: &str,
    _: Option<&str>,
) -> ExitCode {
    refused("a sign-in is held to its input only on a runner")
}

/// `purlis profile sign-out <name>` (#4043): the harness's own sign-out, on this device, with the
/// profile's folder variables set: what a desktop runs on a runner, so the runner holds no
/// sign-in the person does not want. A pinned version is the one that runs it, as for a sign-in.
fn profile_sign_out(store: &purlis_core::profilestore::Homes, name: &str) -> ExitCode {
    use purlis_core::profilestore::{self, installs, sign_in};
    let record = match profilestore::list(store.config_root()) {
        Ok(records) => records.into_iter().find(|p| p.name == name),
        Err(why) => return refused(&why),
    };
    let Some(record) = record else {
        return refused(&format!(
            "this machine has no profile named '{}'. purlis profile list names the ones it has.",
            shown::short(name)
        ));
    };
    // A desktop signs out what a runner keeps for it; on any other device the person runs their
    // harness's own sign-out.
    if record.kept_for.is_none() {
        return refused(&format!(
            "'{}' is not kept on this device for a desktop, so purlis signs nothing out: run its \
             harness's own sign-out",
            record.name
        ));
    }
    let pinned = match installs::on_this_runner(&record, &|line| eprintln!("{line}")) {
        Ok(pinned) => pinned,
        Err(why) => return refused(&format!("{why}. Nothing was signed out.")),
    };
    let logout = match sign_in::logout_of(&record, pinned.as_deref()) {
        Ok(logout) => logout,
        Err(why) => return refused(&why),
    };
    let ran = std::process::Command::new(&logout.program)
        .args(&logout.args)
        .envs(logout.env.iter().map(|(n, v)| (n, v)))
        .stdin(std::process::Stdio::null())
        .status();
    match ran {
        Ok(status) if status.success() => ExitCode::SUCCESS,
        Ok(status) => {
            eprintln!(
                "purlis: the harness's sign-out of '{}' did not end well ({status})",
                record.name
            );
            ExitCode::FAILURE
        }
        Err(e) => refused(&format!("{} could not be run: {e}", logout.program)),
    }
}

/// What an import copied and left out, a line each, for `machine_profile`.
fn imported_lines(made: &purlis_core::profilestore::Made) -> String {
    let mut out = String::new();
    match &made.record.imported {
        Some(imported) if !made.copied.copied.is_empty() => out.push_str(&format!(
            "imported its settings from {}: {}\n",
            imported.from.display(),
            made.copied.copied.join(", ")
        )),
        Some(imported) => out.push_str(&format!(
            "found no settings to import in {}\n",
            imported.from.display()
        )),
        None => {}
    }
    for one in &made.copied.skipped {
        out.push_str(&format!("left out {one}\n"));
    }
    out
}

/// `purlis plugin install|uninstall`: needs no plane, and acts on this machine's harnesses.
/// `migrate [--undo]`: rename-local on this machine, with the project the current directory is
/// in among the projects it moves (RN-5). What it did goes to stdout, one line a step; a
/// refusal, which moved nothing, to stderr. Exit 1 when anything was refused or failed.
fn migrate(undo: bool) -> ExitCode {
    use purlis_core::renamelocal::{self, Local, Seams};
    let here: Vec<std::path::PathBuf> = std::env::current_dir()
        .ok()
        .and_then(|cwd| purlis_core::plane::find_root(&cwd).ok())
        .into_iter()
        .collect();
    let Some(mut local) = Local::of_this_machine(&here) else {
        eprintln!(
            "purlis: cannot tell where this machine's config home is, so there is nowhere to \
             journal a move; nothing was moved"
        );
        return ExitCode::FAILURE;
    };
    // The harness plugin moves with the rest, installed from this charter and the plugin
    // shipped beside it, as `plugin install` installs it (RN-8). Without a plugin beside it,
    // Claude Code's registration is still pointed at the copy where it moved (D-RN8-13).
    if let Ok(machine) = purlis_core::doctor::fix::this_machine(None) {
        local.plugin = Some(machine);
    }
    let moved = if undo {
        renamelocal::undo(&local, &Seams::real())
    } else {
        renamelocal::run(&local, &Seams::real())
    };
    if let Some(why) = &moved.refused {
        eprintln!("purlis: {why}");
        return ExitCode::FAILURE;
    }
    for line in &moved.said {
        println!("{line}");
    }
    if moved.complete {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn plugin(verb: &PluginCommand) -> ExitCode {
    use purlis_core::plugin_install::{self as install, Verb};
    let (verb, harness, dry_run, from) = match verb {
        PluginCommand::Install {
            harness,
            dry_run,
            plugin_from,
        } => (Verb::Install, harness, *dry_run, plugin_from.clone()),
        PluginCommand::Uninstall { harness, dry_run } => (Verb::Uninstall, harness, *dry_run, None),
    };
    let machine = match plugin_machine(from.as_deref()) {
        Ok(machine) => machine,
        Err(why) => {
            eprintln!("purlis: {why}");
            return ExitCode::FAILURE;
        }
    };
    let outcomes = install::run(&machine, verb, harness, dry_run);
    print!("{}", install::render(&outcomes, dry_run));
    if install::failed(&outcomes) {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn main() -> ExitCode {
    // purlis's own wrap of a chat on Linux (#1040), before anything else of the command runs:
    // the hidden `purlis _sandbox`, never in help and not a public command line. Each call its
    // supervisor refuses is told to the app as the chat's sandbox block (#2344).
    if let Some(code) =
        purlis_linux_sandbox::serve_if_asked_telling(purlis_core::sandboxblock::supervised::teller)
    {
        std::process::exit(code);
    }
    // FIRST, before anything that could panic, argv and clap included: a guard that crashed
    // must refuse the tool call, not allow it (#349).
    if is_a_pretooluse_call(&std::env::args_os().collect::<Vec<_>>()) {
        guard::refuse_on_a_crash(tell_the_host_about_a_crash);
    }
    // Started again by the session host as a browser engine's keeper (#2012): keep its process
    // group and never return, before anything else of the command runs.
    #[cfg(unix)]
    purlis_core::browsers::keep_if_asked();
    // Before any git is run: whoever started this command ends it by its process group (a
    // terminal's Ctrl-C, a harness ending a hook, the app ending a chat), so the gits it runs
    // stay in that group, where the same stop reaches them.
    purlis_core::worktree::git::stay_in_the_callers_group();
    #[cfg(debug_assertions)]
    if std::env::var_os(SAYS_ITS_GITS_GROUP_ENV).is_some() {
        println!(
            "{}",
            if purlis_core::worktree::git::each_git_leads_its_own_group() {
                "each git leads a group of its own"
            } else {
                "each git stays in this command's group"
            }
        );
        return ExitCode::SUCCESS;
    }
    // A warning the core raises is said on this terminal rather than dropped (#647). Output a
    // command prints on purpose never goes through it.
    purlis_core::applog::install_for_a_terminal();
    #[cfg(debug_assertions)]
    if std::env::var_os(PANIC_ON_PURPOSE_ENV).is_some() {
        panic!("{PANIC_ON_PURPOSE_ENV} is set, so purlis crashes");
    }
    #[cfg(debug_assertions)]
    if std::env::var_os(CORE_WARNS_ON_PURPOSE_ENV).is_some() {
        purlis_core::applog::warn_on_purpose();
    }
    // **`Cli::parse` exits 2 on a bad command line, and 2 is the one code a harness reads as
    // "block".** A hook that exited 2 by accident would make a session unable to end, so this
    // binary answers for its own argv before clap can, whatever the command turns out to be.
    // `secret exec`'s `-- <command…>` is the child's, flags included, and is peeled off before
    // the parser can read any of it as charter's — `cli._split_exec_command`.
    let (argv, graft) = secret::split_exec(std::env::args_os().collect());
    // An extension's command, or a core-owned alias onto one (charter-app#342) — asked only
    // about a first word clap does not answer itself, so a core word is always the core's.
    let mut parser = {
        use clap::CommandFactory;
        Cli::command()
    };
    parser.build();
    // An extension's command runs before clap and before the read-only gate below, so it is
    // gated here: any first word that is not a core command or a flag goes to an extension,
    // and an extension's command can write the project (FR-24).
    let words: Vec<String> = argv
        .iter()
        .skip(1)
        .map(|word| word.to_string_lossy().into_owned())
        .collect();
    if purlis_core::extension::cli::extension_command(&words).is_some() {
        // An extension's command can act on the project too (D-RN2d-8).
        if let Some(disagree) = purlis_core::envvar::disagreement() {
            return refused(&disagree.to_string());
        }
        if let Some(why) = read_only_project() {
            return refused(&why);
        }
    }
    if let Some(code) = extcmd::intercept(&argv, |word| parser.find_subcommand(word).is_some()) {
        return code;
    }
    // The first word, when clap does not answer it: what the line after clap's own error names.
    // clap reports a bad word under a core command with the same error kind, and that one is
    // not about extensions.
    let unknown_first = argv
        .get(1)
        .map(|word| word.to_string_lossy().into_owned())
        .filter(|word| parser.find_subcommand(word).is_none());
    let cli = match Cli::try_parse_from(argv) {
        Ok(cli) => cli,
        Err(err) => {
            let _ = err.print();
            if err.kind() == clap::error::ErrorKind::InvalidSubcommand
                && let Some(word) = &unknown_first
            {
                extcmd::note_after_an_unknown_word(word);
            }
            return match err.exit_code() {
                0 => ExitCode::SUCCESS,
                _ => ExitCode::FAILURE,
            };
        }
    };
    // **A variable that chooses what to act on, named twice with two values, is refused**
    // (D-RN2d-8), never read under one of its names: inside a chat both names carry the same
    // value, so this is a `CHARTER_ROOT=…` typed in front of a command, which would otherwise
    // act on the chat's own project. Every command that acts on a project refuses; a hook
    // decides per kind ([`hook_on_disagreement`]); the commands below need no project, and a
    // guarded program that is `charter` refuses on its own.
    if let Some(disagree) = purlis_core::envvar::disagreement() {
        match &cli.command {
            Command::Hook {
                name, list: false, ..
            } => return hook_on_disagreement(name.as_deref().unwrap_or_default(), &disagree),
            // `report`, the shell guard, git's hooks and `scan` need no project: a commit in a
            // repository that is not one is never blocked over the project's variables.
            Command::Hook { list: true, .. }
            | Command::Report(_)
            | Command::ShellGuard { .. }
            | Command::GitHook { .. }
            | Command::Scan { .. } => {}
            _ => return refused(&disagree.to_string()),
        }
    }
    // Before `run`, because its exit code is not a plain success or failure: a tool hook this
    // binary does not answer must BLOCK rather than be read as "allow".
    if let Command::Hook {
        name,
        list,
        json,
        plugin_version: _,
        now,
    } = &cli.command
    {
        if *list {
            return hooks::list(*json);
        }
        return hook(name.as_deref().unwrap_or_default(), now.as_deref());
    }
    // Needs no plane: a chat anywhere can report a charter bug.
    if let Command::Report(command) = &cli.command {
        return report::run(command);
    }
    // Needs no plane either, and must never stand in the way of the harness it guards.
    if let Command::ShellGuard {
        shims,
        harness,
        args,
    } = &cli.command
    {
        return shellguard::run(shims, harness, args);
    }
    // Needs no plane: it runs in whatever repository a chat commits to.
    if let Command::GitHook { name, args } = &cli.command {
        return githook::run(name, args);
    }
    // Needs no plane either: it scans the repository it stands in.
    if let Command::Scan { explain } = &cli.command {
        return scan::run(*explain);
    }
    // Needs no project: it runs a chat's server, wherever the chat is (#2019).
    #[cfg(unix)]
    if let Command::Listen { port, command } = &cli.command {
        return listen::run(*port, command);
    }
    // Needs no project: one host serves every project of this user on this device (ADR 0068).
    if let Command::Serve { suspends_unlinked } = &cli.command {
        return serve(suspends_after(*suspends_unlinked));
    }
    // Needs no project: a runner's bridge to its own host, or its pairing (ADR 0078 §2, §3).
    if let Command::Bridge {
        pair,
        key_made_at_start,
        make_key,
        publish,
        home,
    } = &cli.command
    {
        #[cfg(not(unix))]
        {
            let _ = (pair, key_made_at_start, make_key, publish, home);
            return bridge();
        }
        #[cfg(unix)]
        let asked = match (make_key, publish) {
            (Some(handoff), Some(published)) => bridge::Asked::MakeKey {
                handoff: handoff.clone(),
                published: published.clone(),
                home: home.clone(),
            },
            _ if *pair => bridge::Asked::Pair {
                key_made_at_start: key_made_at_start.clone(),
            },
            _ => bridge::Asked::Pipe,
        };
        #[cfg(unix)]
        return bridge(asked);
    }
    // Needs no project: the app that started the chat answers it, from its own record (#1450).
    if let Command::Persona(memory::PersonaCommand::Where { now }) = &cli.command {
        return whereworking::run(now.as_deref());
    }
    // The three internal words the Python charter's plugin wires beside its hooks. Answered —
    // exit 0, nothing printed, nothing read — so a plugin that still names them can never fail
    // a session start or a turn end on them; `hookreg` says why each is left out on purpose.
    if matches!(
        &cli.command,
        Command::Workspace(WorkspaceCommand::Reconcile | WorkspaceCommand::Autosave)
            | Command::Persona(memory::PersonaCommand::Gc { .. })
    ) {
        return ExitCode::SUCCESS;
    }
    // `persona sync-agents` is retired with the persona sub-agent (#1451). The word is taken
    // and refused by name, with what replaced it, wherever it is run from.
    if matches!(
        &cli.command,
        Command::Persona(memory::PersonaCommand::SyncAgents { .. })
    ) {
        voice::err(purlis_core::personaverbs::retired::SYNC_AGENTS);
        return ExitCode::FAILURE;
    }
    // `browser install` is retired (#2575): taken and refused by name, with what replaced it,
    // wherever it is run from.
    if matches!(
        &cli.command,
        Command::Browser(BrowserCommand::Install { .. })
    ) {
        voice::err(purlis_core::personaverbs::retired::BROWSER_INSTALL);
        return ExitCode::FAILURE;
    }
    // A project this charter cannot write is read-only to it (FR-24, V37a,
    // `docs/plane-format.md` § Compatibility across charter versions): a format it does not
    // understand, a feature it lacks, or a `charter.toml` it cannot read. Every command that
    // could write it is refused, naming why; the ones that only say what is wrong or move
    // charter itself forward still run.
    //
    // `init` and `reinit` facing a `charter.toml` that is a link are left to their own
    // containment gate, which refuses a link out of the project with the words that name it and
    // writes nothing; reading through the link here would answer a file that is not the
    // project's.
    let own_gate = matches!(&cli.command, Command::Init(_) | Command::Reinit)
        && place().is_ok_and(|p| purlis_core::scaffold::manifest_escapes(&p.root));
    if !own_gate && let Some(why) = read_only_project() {
        match read_only_standing(&cli.command) {
            ReadOnly::Runs => {}
            ReadOnly::RunsAndSays => eprintln!("purlis: {why}"),
            ReadOnly::Refused => return refused(&why),
        }
    }
    // After the gate: on a project this purlis cannot write, the server starts, and refuses
    // each tool that writes at its call (#1051).
    if let Command::Mcp { graded } = &cli.command {
        // Started by the harness, as a hook is (#1421).
        purlis_core::sandbox::started_by_the_harness();
        // A chat holds this server for its whole life: the config home stays where it is until
        // it ends (`renamelocal::busy::LOCK`, D-RN5-11).
        let _holds = purlis_core::machine::config_root_if_there()
            .and_then(|root| purlis_core::renamelocal::busy::hold_shared(&root));
        return match mcp::serve(*graded) {
            Ok(code) => ExitCode::from(code),
            Err(why) => {
                voice::err(why.to_string());
                ExitCode::FAILURE
            }
        };
    }
    // `init`, `reinit` and `doctor` each say several lines of their own and choose their own
    // exit status — and for `doctor` the status IS the verdict, where a blocker is not an
    // error message.
    match &cli.command {
        Command::Init(init) => {
            let args = purlis_core::scaffold::InitArgs {
                forge: init.forge.clone(),
                owner: init.owner.clone().unwrap_or_default(),
                host: init.host.clone(),
                clone_this_repo: init.clone_this_repo,
                plane_is_this_repo: init.plane_is_this_repo,
                adopt: init.adopt.clone(),
                // Refused rather than quietly replaced by the wall clock: the flag exists so
                // the differential can pin the first workspace's manifest, and a pin that
                // silently did not take would make that comparison green for the wrong reason.
                now: match instant(init.now.as_deref()) {
                    Ok(secs) => chrono::DateTime::from_timestamp(secs as i64, 0),
                    Err(why) => {
                        eprintln!("purlis: {why}");
                        return ExitCode::FAILURE;
                    }
                },
                front_door: if init.no_front_door {
                    None
                } else {
                    Some(
                        init.front_door
                            .clone()
                            .unwrap_or_else(|| "steward".to_owned()),
                    )
                },
            };
            return match place() {
                Ok(place) => say(&purlis_core::scaffold::init(&place, &args)),
                Err(message) => {
                    eprintln!("purlis: {message}");
                    ExitCode::FAILURE
                }
            };
        }
        Command::Reinit => {
            return match place() {
                Ok(place) => say(&purlis_core::scaffold::reinit(&place)),
                Err(message) => {
                    eprintln!("purlis: {message}");
                    ExitCode::FAILURE
                }
            };
        }
        Command::Doctor {
            json,
            preflight,
            fix,
            name,
            email,
        } => {
            // FX-3: either flag is the git identity's input; the core says what is missing.
            let identity = (name.is_some() || email.is_some()).then(|| {
                (
                    name.as_deref().unwrap_or_default(),
                    email.as_deref().unwrap_or_default(),
                )
            });
            return doctor(
                *json,
                *preflight,
                fix.as_ref().map(Option::as_deref),
                identity,
            );
        }
        Command::Migrate { undo } => return migrate(*undo),
        Command::Plugin(verb) => return plugin(verb),
        // This machine's profiles: no plane is read or written.
        Command::Profile(
            verb @ (ProfileCommand::Create { .. }
            | ProfileCommand::Import { .. }
            | ProfileCommand::Remove { .. }
            | ProfileCommand::Authenticate { .. }
            | ProfileCommand::SignOut { .. }),
        ) => return machine_profile(verb),
        // A background refresh and a footer: neither is a plane write, and both choose their
        // own exit status as their Python counterparts do.
        Command::GlRefresh {
            workspace,
            detach,
            now,
        } => {
            let here = match Here::read() {
                Ok(here) => here,
                Err(why) => {
                    eprintln!("purlis: {why}");
                    return ExitCode::FAILURE;
                }
            };
            let workspace = match here.active_workspace(workspace.as_deref()) {
                Ok(ws) => ws,
                Err(why) => return refused(&why),
            };
            return gl_refresh(&workspace, *detach, now.as_deref());
        }
        Command::Statusline { watch, now, .. } => {
            // Before stdin is read or a plane is looked for: nothing is drawn, nothing is
            // recorded, and the answer does not depend on where it is asked (HY-11).
            // Whether `--watch` repaints or is retired: #997.
            if *watch {
                return refused(
                    "`statusline --watch` is not in this version yet; statusline does not repaint, \
                     and one frame would pass for a watch that stopped, so run \
                     `purlis statusline` once per turn from the harness's status-line \
                     command instead",
                );
            }
            let payload = payload();
            // Resolved BEFORE the render, and a bad value is refused rather than quietly
            // replaced by the wall clock: the flag exists so a differential can pin the ages
            // on the row, and a pin that silently did not take would make the comparison
            // green for the wrong reason.
            let when = match instant(now.as_deref()) {
                Ok(secs) => chrono::DateTime::from_timestamp(secs as i64, 0)
                    .unwrap_or_else(chrono::Utc::now),
                Err(why) => {
                    eprintln!("purlis: {why}");
                    return ExitCode::FAILURE;
                }
            };
            // No plane means nothing is recorded: see `statusline::run`, which declares that
            // divergence from Python and why it is the right way round.
            let here = plane().ok();
            statusline::run(
                here.as_ref().map(|plane| plane.root()),
                &payload,
                &statusline::Ambient::here(),
                when,
            );
            return ExitCode::SUCCESS;
        }
        // `charter version`, and it needs no plane: Python builds `config.ROOT` from
        // `find_root_or_cwd`, so the command answers outside one and simply has no pin to
        // report. What it answers, and why it is not Python's three rows, is ADR 0030 as
        // amended by ADR 0045.
        Command::Version { what } => {
            use purlis_core::adopt;
            return emit(&match what {
                Some(VersionCommand::Sync { .. }) => adopt::version_move_refusal("sync"),
                Some(VersionCommand::Bump { .. }) => adopt::version_move_refusal("bump"),
                None => {
                    let cwd =
                        std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
                    let place = purlis_core::plane::place(&cwd);
                    adopt::version_report(place.is_plane.then_some(place.root.as_path()))
                }
            });
        }
        Command::Stop { .. } => return stop_every_agent(),
        // `theme` reads this machine's themes, and the plane's when it runs in one, and needs
        // none; it chooses its own exit, a linter's (`theme.rs`).
        Command::Theme(command) => return theme::run(command),
        // `news` and `update` say several lines of their own on both streams and choose their
        // own exit status, exactly as `init` does.
        Command::News(news) => {
            return emit(&purlis_core::news::report(&purlis_core::news::Args {
                for_version: news.for_version.clone(),
                since: news.since.is_some(),
                until: news.until.is_some(),
                pending: news.pending,
            }));
        }
        Command::Update { to, bump, channel } => {
            let config_root = purlis_core::machine::config_root();
            if let Some(word) = channel {
                return emit(&purlis_core::adopt::set_channel_report(
                    config_root.as_deref(),
                    word,
                ));
            }
            let args = purlis_core::adopt::UpdateArgs {
                to: to.clone().unwrap_or_default(),
                bump: *bump,
            };
            return emit(&purlis_core::adopt::update_report_with_channel(
                config_root.as_deref(),
                &args,
            ));
        }
        _ => {}
    }
    // The repo commands speak line by line as they go — a clone is slow, and the line that
    // says which repo is being fetched is worth nothing once it has been — and choose their
    // own exit status, as their Python counterparts do.
    // Before the repo commands, because `docs list` and `docs show` need no plane and those
    // resolve one first.
    if let Some(code) = docs_command(&cli.command) {
        return code;
    }
    if let Some(code) = repo_command(&cli.command) {
        return code;
    }
    // `save` and `git-policy` likewise: several lines each, and an exit status of their own.
    if let Some(code) = plane_command(&cli.command) {
        return code;
    }
    // The workspace verbs that act on a workspace as a whole — `remove`'s guard exits 2.
    if let Some(code) = workspace_command(&cli.command) {
        return code;
    }
    // `dispatch` says what it started, or that it is held for the person, and exits 0; or says
    // one refusal and exits 1.
    if let Command::Dispatch {
        to,
        name,
        profile,
        wait,
        timeout,
        place,
        long,
        command,
    } = &cli.command
    {
        if let Some(command) = command {
            return dispatch::run(command);
        }
        let here = match Here::read() {
            Ok(here) => here,
            Err(why) => {
                eprintln!("purlis: {why}");
                return ExitCode::FAILURE;
            }
        };
        let waits = wait.then(|| timeout.unwrap_or(purlis_core::dispatched::WAITS_BY_DEFAULT));
        return dispatch::dispatch(
            &here,
            to.as_deref(),
            name.as_deref(),
            &dispatch::Options {
                profile: profile.as_deref(),
                place: place.as_deref(),
                long: *long,
            },
            waits,
        );
    }
    // `handoff` answers for itself: opened or held for the person (0), or one refusal (1).
    if let Command::Handoff {
        workspace,
        summary,
        name,
        report,
        create,
        vision,
        persona,
        now,
    } = &cli.command
    {
        let here = match Here::read() {
            Ok(here) => here,
            Err(why) => {
                eprintln!("purlis: {why}");
                return ExitCode::FAILURE;
            }
        };
        let args = handoff::Args {
            workspace: workspace.clone(),
            create: *create,
            vision: vision.clone(),
            persona: persona.clone(),
            name: name.clone(),
            report: *report,
            summary: summary.clone(),
            now: now.clone(),
        };
        let code = handoff::handoff(&here, &args);
        if code == ExitCode::SUCCESS && args.creates_a_handoff() {
            extensions::tell(
                here.plane.root(),
                &ExtensionEvent::HandoffCreated {
                    workspace: workspace.clone(),
                },
            );
        }
        return code;
    }
    // The secrets commands: each chooses its own exit status (2 for a refusal, the child's for
    // `exec`), and `exec` takes the command the argv split set aside.
    let command = match cli.command {
        Command::Secret(c) => return with_here(|here| secret::secret(here, c, graft)),
        Command::Vault(c) => return with_here(|here| secret::vault(here, c)),
        Command::Persona(memory::PersonaCommand::Secret(c)) => {
            return with_here(|here| secret::persona_secret(here, c, graft));
        }
        other => other,
    };
    match run(command) {
        Ok(code) => ExitCode::from(code),
        Err(message) => {
            eprintln!("purlis: {message}");
            ExitCode::FAILURE
        }
    }
}

/// Run `f` on this invocation's [`Here`], or say why there is none.
fn with_here(f: impl FnOnce(&Here) -> u8) -> ExitCode {
    match Here::read() {
        Ok(here) => ExitCode::from(f(&here)),
        Err(why) => {
            eprintln!("purlis: {why}");
            ExitCode::FAILURE
        }
    }
}

/// `purlis doctor`: every check, as a table or as `--json`, and the verdict as the exit.
///
/// `--fix` repairs before it reports, so the report reads as the state after the repair, as
/// Python's did when it installed the Claude Code plugin first. Bare `--fix` applies purlis's
/// plugin install (#373) and every fix the rows offer that is not `by_name_only` (the fix
/// registry, FX-1 and FX-2), then the plane's ask rule for `charter report --yes` (ADR 0059);
/// `--fix <id>` applies that one fix alone, `discover` included.
///
/// `fix` is `None` without `--fix`, `Some(None)` for a bare `--fix`, and `Some(Some(id))` for
/// one fix.
///
/// `identity` is `--name` and `--email`, the input `git-identity` takes (FX-3); a fix that
/// takes input and was given none is refused by the registry, which says what to give.
fn doctor(
    json: bool,
    preflight: bool,
    fix: Option<Option<&str>>,
    identity: Option<(&str, &str)>,
) -> ExitCode {
    use std::io::IsTerminal;

    // `--name`/`--email` with `--fix <id>` for another fix would be dropped without a word.
    if identity.is_some()
        && let Some(Some(id)) = fix
        && id != purlis_core::doctor::fix::FixId::GitIdentity.id()
    {
        eprintln!("purlis doctor: --name and --email go with --fix git-identity, not --fix {id}");
        return ExitCode::from(2);
    }

    let mut fix_failed = false;
    let cwd = match std::env::current_dir() {
        Ok(cwd) => cwd,
        Err(e) => {
            eprintln!("purlis: cannot read the current directory: {e}");
            return ExitCode::FAILURE;
        }
    };
    // The registry's fixes (FX-1, FX-2): the one asked for, or every one a row offers. Bare
    // `--fix` always installs charter's plugin for the chats started outside the app (#373),
    // offered or not, as it always has: through the registry's `plugin-install`, the one code
    // path, so a row that offers it too does not install it twice. Each fix's lines go to
    // stderr so a `--json` reader still gets JSON alone.
    if let Some(asked) = fix {
        use purlis_core::doctor::fix::{self as registry, FixId};
        let doctor = purlis_core::doctor::Doctor::new(&cwd, preflight);
        let mut ids: Vec<FixId> = match asked {
            // clap took only an id the registry has.
            Some(id) => FixId::parse(id).into_iter().collect(),
            // Bare: the local, additive fixes only. A fix that goes over the network runs
            // only when it is named (`FixId::by_name_only`, D-FX2-9).
            None => {
                let mut ids = doctor.fixes();
                ids.push(FixId::PluginInstall);
                ids.retain(|id| !id.by_name_only());
                ids.sort();
                ids.dedup();
                ids
            }
        };
        // `--name`/`--email` with a bare `--fix` are the identity's input even when the row
        // offers no fix: the core then says the identity is already set, and the flags are
        // never dropped without a word (FX-3).
        if identity.is_some() && !ids.contains(&FixId::GitIdentity) {
            ids.push(FixId::GitIdentity);
            ids.sort();
        }
        for id in ids {
            let lines = match (id, identity) {
                (FixId::GitIdentity, Some((name, email))) => {
                    match registry::identity::apply(doctor.root(), name, email) {
                        Ok(fixed) => {
                            fix_failed |= !fixed.complete();
                            fixed.lines()
                        }
                        Err(invalid) => {
                            fix_failed = true;
                            invalid
                                .lines()
                                .into_iter()
                                .map(|why| format!("✗ refused: {why}"))
                                .collect()
                        }
                    }
                }
                _ => {
                    let fixed = registry::apply(doctor.root(), id);
                    fix_failed |= !fixed.complete();
                    fixed.lines()
                }
            };
            eprintln!("fix {id}:");
            for line in lines {
                eprintln!("  {line}");
            }
        }
    }
    // The plane repair: the ask rule a report is filed behind (ADR 0059, amended 2026-09-26).
    if fix == Some(None)
        && let Some((said, code)) = purlis_core::doctor::fix_report_rule(&cwd)
    {
        eprint!("{said}");
        fix_failed |= code != 0;
    }
    let rows = purlis_core::doctor::Doctor::new(&cwd, preflight).run();
    if json {
        print!("{}", purlis_core::doctor::json(&rows));
    } else {
        print!(
            "{}",
            purlis_core::doctor::table(&rows, std::io::stdout().is_terminal())
        );
    }
    ExitCode::from(purlis_core::doctor::exit_code(&rows).max(u8::from(fix_failed)))
}

/// `purlis harness show <name>`: the declaration on stdout under one comment line naming
/// where it came from and its digest, or the one sentence on stderr saying why there is none.
///
/// Each line through [`shown::readable`]: the file is one a chat can write, and a control
/// byte in it would otherwise drive the reader's terminal. **So what it prints is not always
/// the digested bytes**: anything outside printable ASCII (a tab, an accented letter in a
/// comment) is printed escaped, and the digest in the first line is of the file as it is on
/// disk.
fn harness_show(root: &std::path::Path, name: &str) -> u8 {
    use purlis_core::harness_declaration as decl;
    let declared = decl::read(root);
    let Some(found) = declared.get(name) else {
        let file = format!("{}/{}.toml", decl::DIR, shown::short(name));
        if let Some(refused) = declared.refused.iter().find(|r| r.file == file) {
            eprintln!("purlis: {}", refused.reason);
            return 1;
        }
        let names: Vec<&str> = declared.declared.iter().map(|d| d.name.as_str()).collect();
        eprintln!(
            "purlis: no harness '{}' is declared — the harnesses are {}. A project declares \
             one in {}/<name>.toml.",
            shown::short(name),
            names.join(", "),
            decl::DIR
        );
        return 1;
    };
    let lines: Vec<String> = found
        .text
        .lines()
        .map(|line| {
            if line.is_empty() {
                String::new()
            } else {
                shown::readable(line, usize::MAX)
            }
        })
        .collect();
    println!("# {}: {}, {}", found.name, found.file, found.digest);
    if lines
        .iter()
        .zip(found.text.lines())
        .any(|(shown, line)| shown != line)
    {
        println!(
            "# shown escaped: what is not printable ASCII is written as an escape, so the text \
             below is not the bytes the digest is of"
        );
    }
    for line in lines {
        println!("{line}");
    }
    0
}

/// The profile listing, as `purlis harness list` prints it on stderr, and with `declared`, as
/// `purlis profile list` does: a SIGN-IN column, the kind of sign-in purlis can see a profile
/// has, from its harness's declaration, its `env` and `[chat_env] pass` (`passed`), or
/// `unknown` where it cannot tell (#2031). A built-in this machine adopted (#2033) is FROM its
/// title and its folder: `Claude Code (this Mac) at ~/.claude`.
///
/// Built-ins first in registry order — a declared replacement keeps its kind's place — then
/// declared profiles by name. **Every width is measured from the cells about to be printed**
/// rather than guessed: a fixed `{:<28}` pads a short value and does nothing at all to a
/// long one, which pushes that row's remaining columns somewhere no other row's land and
/// stops the table being a table.
///
/// Every cell that came out of the file has already been contained by the time it arrives
/// here (`profiles::display`, `shown::readable`): a command is text a chat can write, and a
/// carriage return in one would otherwise redraw this line.
fn harness_listing(
    set: &ProfileSet,
    check: &profiles::IgnoreCheck,
    declared: Option<(
        &purlis_core::harness_declaration::Declarations,
        &[String],
        &[purlis_core::profilestore::MachineProfile],
    )>,
) -> String {
    let order: Vec<String> = profiles::builtins().into_iter().map(|p| p.name).collect();
    let mut rows: Vec<&purlis_core::profiles::Profile> = set.profiles().iter().collect();
    rows.sort_by_key(|p| {
        let place = order.iter().position(|name| *name == p.name);
        (place.is_none(), place.unwrap_or(0), p.name.clone())
    });

    let sign_in =
        |p: &purlis_core::profiles::Profile,
         (declared, passed): (&purlis_core::harness_declaration::Declarations, &[String])| {
            declared
                .get(&p.kind)
                .and_then(|declaration| declaration.profile_sign_in(&p.env, passed))
                // Through the closure, and never by the kind's type name: this binary names no
                // forge sign-in, and `a_forge_token_never_reaches_a_chat.rs` holds its sources to
                // that by the type's word.
                .map_or("unknown", |kind| kind.word())
                .to_owned()
        };
    let heads: Vec<&str> = match declared {
        Some(_) => vec!["NAME", "KIND", "SIGN-IN", "COMMAND"],
        None => vec!["NAME", "KIND", "COMMAND"],
    };
    let body: Vec<Vec<String>> = rows
        .iter()
        .map(|p| {
            let mut row = vec![shown::short(&p.name), p.kind.clone()];
            if let Some((declarations, passed, _)) = declared {
                row.push(sign_in(p, (declarations, passed)));
            }
            row.push(profiles::display(p));
            row
        })
        .collect();
    // Each column is its header, its widest cell, and the gap to the next one — counted
    // inside the width so a caller pads once rather than padding and then adding spaces.
    // Every cell is printable ASCII by now, so a character is a column.
    let widths: Vec<usize> = heads
        .iter()
        .enumerate()
        .map(|(i, head)| {
            body.iter()
                .map(|row| row[i].chars().count())
                .chain(std::iter::once(head.chars().count()))
                .max()
                .unwrap_or(0)
                + 2
        })
        .collect();

    let line = |mark: &str, cells: &[&str], last: &str| {
        let mut out = mark.to_owned();
        for (cell, width) in cells.iter().zip(&widths) {
            out.push_str(cell);
            out.extend(std::iter::repeat_n(
                ' ',
                width.saturating_sub(cell.chars().count()),
            ));
        }
        out.push_str(last);
        format!("{}\n", out.trim_end())
    };

    let mut out = line("  ", &heads, "FROM");
    for (p, cells) in rows.iter().zip(&body) {
        // The row the selector starts on, marked. It launches nothing by itself.
        let mark = if set.default.as_deref() == Some(p.name.as_str()) {
            "* "
        } else {
            "  "
        };
        let adopted = declared
            .filter(|_| p.source == profiles::Source::BuiltIn)
            .and_then(|(_, _, machine)| purlis_core::profilestore::adopted_as(machine, &p.name));
        let source = match adopted {
            Some(machine) => format!(
                "{} at {}",
                shown::short(&machine.title),
                shown::short(&home_spelled(&machine.folders.config))
            ),
            None => p.source.as_str().to_owned(),
        };
        let cells: Vec<&str> = cells.iter().map(String::as_str).collect();
        out.push_str(&line(mark, &cells, &source));
    }
    if !set.refused.is_empty() {
        out.push_str("refused:\n");
        for refused in &set.refused {
            // A whole-file refusal has no profile name, so it is named by its FILE rather
            // than printing a line that starts with a bare colon.
            let who = if refused.name.is_empty() {
                &refused.source
            } else {
                &refused.name
            };
            out.push_str(&format!("  {who}: {}\n", refused.reason));
        }
    }
    if !check.fix.is_empty() {
        out.push_str(&format!(
            "! to use the profiles in {}: {}\n",
            profiles::LOCAL_FILE,
            check.fix
        ));
    }
    out
}

/// `folder` with the person's home written `~`, as a listing shows a path.
fn home_spelled(folder: &std::path::Path) -> String {
    match profiles::home().and_then(|home| {
        folder
            .strip_prefix(home)
            .ok()
            .map(std::path::Path::to_path_buf)
    }) {
        Some(rest) if rest.as_os_str().is_empty() => "~".to_owned(),
        Some(rest) => format!("~/{}", rest.display()),
        None => folder.display().to_string(),
    }
}

#[cfg(test)]
#[path = "main/tests/core_words.rs"]
mod core_word_tests;

#[cfg(test)]
#[path = "main/tests/serve.rs"]
mod serve_tests;

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The scenario host** (T1.1-06, #6853): one chat, run as production runs it, with the real
//! hooks, proxy, OS wrap, stop writer and Tauri commands, for a protection, a persona and a
//! dialect of the fake harness.
//!
//! - **In the session host**, production's default (`hostedsessions::chats_run_in_the_host`):
//!   the host's own code over the in-memory link (`behaviour::a_host_with`), its proxies started
//!   through `sandbox::hosted::start_through` by the fixture's own [`Confines`], which hands
//!   each one scaled [`Timings`] and the scenario's resolver seam ([`Names`]). The window is the
//!   app's own `Planes`, its chats in that host (`hostedsessions::chats_in_the_host`), timed by
//!   the same value, managed by a Tauri mock app whose commands a scripted person calls.
//! - **The kit is what the app ships** ([`Kit`]): the `purlis` CI builds apart from this test
//!   build (`PURLIS_BINARY`), the repo's plugin, git hooks that binary wrote, the skill sets'
//!   and chat settings' folders, and the window's shims. The wrap is that binary too
//!   (`sandboxing::scenarios_wrap_with`).
//! - **Its own homes**: each scenario runs in a child of this test binary whose data and config
//!   homes and `PATH` are its own ([`Homes::of_this_scenario`]): the app
//!   reads its homes from the environment, which a test cannot set in its own process.
//! - **Under `strict`** ([`DEFAULT_TEMPLATE`]), unless a cell names another template (Q308):
//!   its `[policy]` written in the project's manifest, pressed for where the window would ask,
//!   and what a chat's start resolves checked against it ([`runs_under`]).
//!
//! A scenario that cannot apply its wrap panics on CI (`GITHUB_ACTIONS` or `CI` set), and
//! elsewhere returns with the reason printed: the OS wrap does not nest inside a sandboxed task.

use std::ffi::OsString;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use purlis_core::sandbox::names::Names;
use purlis_core::sandbox::timings::Timings;
use tauri::test::MockRuntime;

use crate::hostedsessions::Connection;
use crate::planes::{Held, PlaneId, Planes};

/// The scale every scenario's timers run at, unless a cell runs at production's (T1.1-01).
pub(crate) const SCALE: f64 = 0.5;

/// The variable naming the `purlis` CI built apart from this test build: neither `fenced` nor
/// `test-support` (`.github/actions/sandbox-scenarios`).
pub(crate) const BINARY_VAR: &str = "PURLIS_BINARY";

/// The variable naming the `purlis-app` CI built apart from this test build, which
/// `in_a_host_process_at` runs as the session host's process (Linux).
pub(crate) const HOST_APP_VAR: &str = "PURLIS_HOST_APP";

/// The variable naming the real Claude Code binary (`claude`) at the version
/// `tests/fixtures/harness-versions/claude.txt` pins, which the Claude Code leg starts (T1.1-10,
/// on macOS; T1.1-53, on Linux). CI sets it; the leg does not run without it.
pub(crate) const CLAUDE_CODE_VAR: &str = "PURLIS_CLAUDE_CODE";

/// The variable naming the real Codex binary (`codex`) at the version
/// `tests/fixtures/harness-versions/codex.txt` pins, which the Codex leg starts (T1.1-53).
pub(crate) const CODEX_VAR: &str = "PURLIS_CODEX";

/// The variable naming the real OpenCode binary (`opencode`) at the version
/// `tests/fixtures/harness-versions/opencode.txt` pins, which the OpenCode leg starts (T1.1-53).
pub(crate) const OPENCODE_VAR: &str = "PURLIS_OPENCODE";

/// The variable that names `dialect`'s real binary, for a leg that starts it.
pub(crate) fn program_var(dialect: Dialect) -> &'static str {
    match dialect {
        Dialect::Claude => CLAUDE_CODE_VAR,
        Dialect::Codex => CODEX_VAR,
        Dialect::OpenCode => OPENCODE_VAR,
    }
}

/// The variable a scenario's child is started with: its home.
const HOME_VAR: &str = "PURLIS_SCENARIO_HOME";

/// The variable a scenario's child is started with: the run's own `$HOME`, outside the temp
/// folder, where a scenario makes a folder no chat may write ([`Scenario::a_folder_outside`]).
const OUTER_HOME_VAR: &str = "PURLIS_SCENARIO_OUTER_HOME";

/// **Whether this process is the scenario's own child** (the re-run of the test that runs the
/// scenario), as opposed to the parent that starts it: only the child has a home.
pub(crate) fn in_the_scenarios_child() -> bool {
    std::env::var_os(HOME_VAR).is_some()
}

/// **Whether this run is CI's**, where a scenario that cannot run fails rather than returns.
pub(crate) fn on_ci() -> bool {
    std::env::var_os("GITHUB_ACTIONS").is_some_and(|v| !v.is_empty())
        || std::env::var_os("CI").is_some_and(|v| !v.is_empty())
}

/// How long a real harness is given to start and play its steps (T1.1-53): longer than the
/// fake's ([`crate::scenarios::PLAYS_WITHIN`]), which starts at once. Codex and OpenCode start a
/// terminal interface and, on a runner, a cold binary under the wrap.
pub(crate) const REAL_PLAYS_WITHIN: Duration = Duration::from_secs(300);

/// How many of each screen a real harness's driver has answered ([`Scenario::answers_a_screen`]).
#[derive(Default)]
struct Answered {
    first_run: u32,
    newer_model: u32,
    bypass: u32,
    permission: u32,
}

/// **What a real harness's driver reads and says** ([`Scenario::answers_until_played`]).
pub(crate) struct Driving<'a> {
    /// Whether the stand-in model has heard the harness yet.
    pub heard: &'a dyn Fn() -> bool,
    /// The one host the leg itself asks about, whose ask is the person's to press and is left to
    /// the cell; every other ask the chat raises is kept blocked.
    pub held: Option<&'a str>,
    /// What the stand-in model heard, for a red run's reader.
    pub evidence: &'a dyn Fn() -> String,
}

/// **Why a scenario did not run**: a failure on CI, a printed reason elsewhere (`None`).
pub(crate) fn cannot_run<T>(why: &str) -> Option<T> {
    assert!(!on_ci(), "the scenario cannot run on CI: {why}");
    let _ = writeln!(std::io::stderr(), "skipped: {why}");
    None
}

/// **A leg that does not apply on this OS** is not a scenario that could not run: it says why and
/// answers `None`, on CI too (the Claude Code leg on Linux, where purlis wraps Claude Code
/// itself).
pub(crate) fn does_not_apply<T>(why: &str) -> Option<T> {
    let _ = writeln!(std::io::stderr(), "skipped: {why}");
    None
}

/// The repo's root.
pub(crate) fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repo")
}

/// **A chat's protection**, as the project's `[sandbox]` sets it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Protection {
    /// `mode = "off"`. Named by the cells ticket 7 runs; no test of the host itself needs it.
    #[allow(dead_code)]
    Off,
    /// The floor: no `mode`, the default (R-013).
    Basic,
    /// `mode = "on"`.
    Full,
}

impl Protection {
    /// The project manifest's `[sandbox]` for it.
    ///
    /// At Full the project lists its egress itself, as T1.1-04's wrap test does: no preset, so
    /// no package-cache roots. With the default presets (toolchains) a wrapped Codex or opencode
    /// chat's Seatbelt profile on macOS is about 510 KB under a runner's real home, and
    /// `sandbox-exec` refuses it as too large, so no chat starts (#5919, confirmed by this
    /// fixture). A cell about the presets names them itself.
    fn manifest(self) -> &'static str {
        match self {
            Self::Off => "[sandbox]\nmode = \"off\"\n",
            Self::Basic => "",
            Self::Full => "[sandbox]\nmode = \"on\"\negress = []\n",
        }
    }

    /// The word a leg report names it by (ticket 5).
    pub(crate) fn word(self) -> &'static str {
        match self {
            Self::Off => "none",
            Self::Basic => "basic",
            Self::Full => "full",
        }
    }

    /// The same, as a leg report's record holds it. Named by [`Scenario::report_cell`].
    #[allow(dead_code)]
    pub(crate) fn in_report(self) -> purlis_scenarios::model::Protection {
        use purlis_scenarios::model::Protection as Report;
        match self {
            Self::Off => Report::None,
            Self::Basic => Report::Basic,
            Self::Full => Report::Full,
        }
    }
}

/// **A dialect of the fake harness** (T1.1-04), as the shim of its name in
/// `crates/fake-harness/shims` runs it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Dialect {
    Claude,
    Codex,
    OpenCode,
}

impl Dialect {
    pub(crate) const ALL: [Dialect; 3] = [Dialect::Claude, Dialect::Codex, Dialect::OpenCode];

    pub(crate) fn word(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::OpenCode => "opencode",
        }
    }

    /// The same, as a leg report's record holds it. Named by [`Scenario::report_cell`].
    #[allow(dead_code)]
    pub(crate) fn in_report(self) -> purlis_scenarios::model::Dialect {
        use purlis_scenarios::model::Dialect as Report;
        match self {
            Self::Claude => Report::Claude,
            Self::Codex => Report::Codex,
            Self::OpenCode => Report::Opencode,
        }
    }
}

/// **A `[policy]` template** (Q308): what every chat of a scenario runs under, core's own
/// ([`purlis_core::enforcement::Template`]). A scenario runs under [`DEFAULT_TEMPLATE`],
/// `strict`, unless a cell names another; the product's own default, `watch`, is never assumed.
pub(crate) use purlis_core::enforcement::Template;

/// The template every scenario runs under unless a cell names another: `strict`, every family
/// enforced, purlis as it was before Q308.
pub(crate) const DEFAULT_TEMPLATE: Template = Template::Strict;

/// **The project manifest's `[policy]` for `template`**: always written, `strict` too, since a
/// project that names none runs under the product's default, `watch`.
pub(crate) fn policy_of(template: Template) -> String {
    format!(
        "[{}]\n{} = \"{}\"\n",
        purlis_core::enforcement::TABLE,
        purlis_core::enforcement::TEMPLATE_KEY,
        template.word()
    )
}

/// **What a chat starting in a project under `template` runs under**: each family as the
/// template sets it, under this machine's locks. `strict` is every family enforced.
pub(crate) fn resolved_under(
    template: Template,
    root: &Path,
) -> purlis_core::enforcement::Resolved {
    purlis_core::enforcement::Resolved::of(
        &purlis_core::enforcement::Settings {
            template,
            template_said: true,
            ..Default::default()
        },
        &purlis_core::sandbox::policy::Locks::of(root),
    )
}

/// **The project at `root` runs under `template` at every chat's start** (Q308a): the person
/// presses, through `press`, where the window would offer it (the template is looser than what
/// they accepted, as `open` is than the shipped `watch`), and then what a start resolves
/// ([`purlis_core::enforcement::at_start`]) is the template's; under `strict`, every family
/// enforced. Panics where it is not: a scenario never runs under a template it did not ask for.
pub(crate) fn runs_under(template: Template, root: &Path, press: impl FnOnce(String)) {
    let standing = purlis_core::enforcement::standing(root);
    assert!(
        standing.refused.is_empty(),
        "the project's [policy] did not read: {:?}",
        standing.refused
    );
    // The window offers the press where the project runs looser than what the person accepted,
    // and where an acceptance kept for it no longer holds: a scenario that makes its project's
    // folder again, at the same path, is another clone to purlis, which reads what was accepted
    // for the one before as broken until the person accepts again.
    if !standing.waiting.is_empty() || standing.unsealed {
        press(standing.digest);
    }
    let at_start = purlis_core::enforcement::at_start(root);
    let standing = purlis_core::enforcement::standing(root);
    assert_eq!(
        at_start,
        resolved_under(template, root),
        "a chat in the project starts under {} as the scenario asked: {} (an acceptance that does not hold: {}; what a press would lift: {:?}; the denies in force: {:?})",
        template.word(),
        at_start.spelled(),
        standing.unsealed,
        standing.lifted,
        purlis_core::enforcement::denies_in_force(root)
    );
    if template == Template::Strict {
        assert_eq!(
            at_start,
            purlis_core::enforcement::Resolved::all(purlis_core::enforcement::Mode::Enforce),
            "under strict every family is enforced"
        );
    }
}

/// **This process is purlis's app, as `main.rs` makes every `purlis-app` process** (the window's
/// and the session host's): only the app resolves a project's `[policy]` and keeps what the
/// person accepted (`enforcement::in_force`); any other process runs `strict` whatever the
/// project says. In a fenced build it seals in memory (`seal::in_memory_for_this_process`), so
/// the person's press can be kept.
fn as_the_app() {
    purlis_core::secrets::keyhold::this_process_is_the_app();
    purlis_core::secrets::seal::in_memory_for_this_process();
}

/// **A scenario's own folders**: its data and config homes, the fake harness's state, and the
/// folder its turn scripts are copied to.
#[derive(Debug, Clone)]
pub(crate) struct Homes {
    pub home: PathBuf,
    /// Its own `$HOME`: nothing of the runner's or the person's own home is read.
    pub user: PathBuf,
    pub data: PathBuf,
    pub config: PathBuf,
    pub state: PathBuf,
    pub scripts: PathBuf,
    pub host: PathBuf,
    /// Where each scenario of this test makes its project: `projects/1`, `projects/2`, …
    pub projects: PathBuf,
}

impl Homes {
    fn at(home: PathBuf) -> Homes {
        Homes {
            user: home.join("user"),
            data: home.join("data"),
            config: home.join("config"),
            state: home.join("fake-state"),
            scripts: home.join("scripts"),
            host: home.join("host"),
            projects: home.join("projects"),
            home,
        }
    }

    /// **This scenario's homes, in a child of this test binary** (the rusty-fork pattern): run
    /// here, it makes them in a folder of its own, re-runs the calling test in a child with them
    /// as its environment, waits for it to pass, checks nothing is left (no process of the
    /// scenario's, no folder but the home itself), removes the home and answers `None`: the
    /// child ran the scenario. In that child it answers the homes, and the child is purlis's app
    /// as `main.rs` makes it ([`as_the_app`]).
    pub(crate) fn of_this_scenario() -> Option<Homes> {
        Homes::of_this_scenario_with(&[])
    }

    /// [`Homes::of_this_scenario`] with `more` in the child's environment: a cell's own
    /// variables, such as the decoy token of a person's login a chat must never present.
    pub(crate) fn of_this_scenario_with(more: &[(&str, &str)]) -> Option<Homes> {
        Homes::of_this_scenario_ahead(more, None)
    }

    /// [`Homes::of_this_scenario_with`], the harness on the `PATH` the real binary `program` names
    /// where one is given (a real-harness leg, T1.1-10 and T1.1-53), and the fake harness's
    /// shims, which stand in for it, left off the `PATH` altogether.
    pub(crate) fn of_this_scenario_ahead(
        more: &[(&str, &str)],
        program: Option<&Path>,
    ) -> Option<Homes> {
        if let Some(home) = std::env::var_os(HOME_VAR) {
            as_the_app();
            return Some(Homes::at(PathBuf::from(home)));
        }
        let me = std::thread::current();
        let test = me
            .name()
            .filter(|name| *name != "main")
            .expect("libtest names the thread a test runs on after the test")
            .to_owned();
        // Under the temp folder, which a fenced build's store must be inside.
        let made = tempfile::Builder::new()
            .prefix("scenario-")
            .tempdir()
            .expect("the scenario's home");
        let homes = Homes::at(made.path().canonicalize().expect("its home"));
        for folder in [
            &homes.user,
            &homes.data,
            &homes.config,
            &homes.state,
            &homes.scripts,
            &homes.host,
            &homes.projects,
        ] {
            std::fs::create_dir_all(folder).expect("a folder of the scenario's");
        }
        let ahead = match program {
            Some(program) => program
                .parent()
                .expect("the real harness's folder")
                .to_path_buf(),
            None => repo().join("crates/fake-harness/shims"),
        };
        let path = std::env::join_paths(std::iter::once(ahead).chain(std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_default(),
        )))
        .expect("a PATH");
        let fake = crate::behaviour::fake_harness();
        let mut env: Vec<(&str, OsString)> = vec![
            (HOME_VAR, homes.home.clone().into()),
            ("HOME", homes.user.clone().into()),
            (purlis_core::datahome::HOME_VAR, homes.data.clone().into()),
            (purlis_core::machine::HOME_VAR, homes.config.clone().into()),
            // Never the turn script, its transcript or its state: what the app runs of a harness
            // beside a chat (its sign-in status, its version) inherits this environment and would
            // play the script outside any sandbox. Each chat is handed its own (`starts`).
            ("FAKE_HARNESS_BIN", fake.into()),
            ("PATH", path),
        ];
        env.extend(
            more.iter()
                .map(|(name, value)| (*name, OsString::from(value))),
        );
        // The re-run clears every variable of purlis's own; these two are the run's.
        for kept in [
            BINARY_VAR,
            HOST_APP_VAR,
            CLAUDE_CODE_VAR,
            CODEX_VAR,
            OPENCODE_VAR,
        ] {
            if let Some(value) = std::env::var_os(kept) {
                env.push((kept, value));
            }
        }
        // Its own `$HOME`, and the toolchain where the run's home kept it: a build the child
        // starts (`testrun::built`) still finds Cargo and rustup.
        let own_home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default();
        env.push((OUTER_HOME_VAR, own_home.clone().into()));
        for (name, under_home) in [("CARGO_HOME", ".cargo"), ("RUSTUP_HOME", ".rustup")] {
            let value = std::env::var_os(name).unwrap_or_else(|| own_home.join(under_home).into());
            env.push((name, value));
        }
        let env: Vec<(&str, &std::ffi::OsStr)> = env
            .iter()
            .map(|(name, value)| (*name, value.as_os_str()))
            .collect();
        let said = purlis_core::testrun::rerun_steered(&[&test], &env);
        let skipped = said.lines().any(|l| l.starts_with("skipped: "));
        for line in said.lines().filter(|l| {
            l.starts_with("skipped: ")
                || l.starts_with("expected fail ")
                || l.starts_with("unexpected pass ")
        }) {
            let _ = writeln!(std::io::stderr(), "{line}");
        }
        homes.nothing_left(skipped);
        made.close().expect("the scenario's home is removed");
        None
    }

    /// Where a scenario's teardown writes what it started (`left.json`), for its parent.
    fn left(&self) -> PathBuf {
        self.home.join("left.json")
    }

    /// Adds `more` to what this child's scenarios left, beside what one before it left: one
    /// child may run several.
    fn record_left(&self, more: Left) {
        let mut kept: Left = std::fs::read_to_string(self.left())
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        kept.pids.extend(more.pids);
        kept.folders.extend(more.folders);
        kept.groups_left.extend(more.groups_left);
        kept.background.extend(more.background);
        kept.quits.extend(more.quits);
        kept.unwatched.extend(more.unwatched);
        // Best effort, from a teardown that may run as a test fails: a `left.json` not written
        // fails the parent's check.
        if let Ok(text) = serde_json::to_string(&kept) {
            let _ = std::fs::write(self.left(), text);
        }
    }

    /// The process groups this child's scenarios left that were #7103's (`Left::background`).
    pub(crate) fn background_left(&self) -> Vec<u32> {
        std::fs::read_to_string(self.left())
            .ok()
            .and_then(|text| serde_json::from_str::<Left>(&text).ok())
            .map(|left| left.background)
            .unwrap_or_default()
    }

    /// **This child ran no scenario, on purpose** (a dialect a leg reports `not_run`, or a test
    /// with no chat): an empty `left.json`, so its parent tells it from a child that died before
    /// its teardown.
    pub(crate) fn ran_no_scenario(&self) {
        self.record_left(Left::default());
    }

    /// **Nothing the scenario started outlived it** (T1.1-06): every process it recorded has
    /// ended, every folder of its own outside its home is gone, every quit it asked of the host
    /// was answered, and no chat's process group outlived its host's quit. Run by the parent,
    /// after the child that ran the scenario ended. A child that said it could not run
    /// (`skipped`) may leave no `left.json`; any other must have written one.
    fn nothing_left(&self, skipped: bool) {
        let text = match std::fs::read_to_string(self.left()) {
            Ok(text) => text,
            Err(_) if skipped => return,
            Err(why) => panic!(
                "the scenario's child wrote no {} ({why}): its teardown never ran, so nothing \
                 says what it left",
                self.left().display()
            ),
        };
        let left: Left = serde_json::from_str(&text).expect("left.json reads");
        assert!(
            left.quits.is_empty(),
            "the host did not answer the window's quit: {:?}",
            left.quits
        );
        assert!(
            left.unwatched.is_empty(),
            "the teardown could not watch every chat by its process group: {:?}",
            left.unwatched
        );
        assert!(
            left.groups_left.is_empty(),
            "a chat's or a tool's process group outlived its host's quit: {:?}",
            left.groups_left
        );
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        for pid in &left.pids {
            while purlis_core::process::alive(*pid) && std::time::Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(50));
            }
            assert!(
                !purlis_core::process::alive(*pid),
                "process {pid}, which the scenario started, outlived it"
            );
        }
        for folder in &left.folders {
            assert!(
                !folder.exists(),
                "{}, the scenario's own, outlived it",
                folder.display()
            );
        }
    }
}

/// The folder of a project a scenario's chats write their transcript and resume cursor in,
/// inside the wrap.
fn own_folder(root: &Path) -> PathBuf {
    root.join(".scenario")
}

/// The one transcript every chat of a scenario appends to.
const TRANSCRIPT: &str = "transcript.jsonl";

/// The lines of one transcript file that parse; none for a file not yet written.
fn lines_of(file: &std::path::Path) -> Vec<serde_json::Value> {
    std::fs::read_to_string(file)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

/// What a scenario started, written by its teardown for its parent's check.
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub(crate) struct Left {
    /// Every process of a chat it saw run, a chat that ended earlier included, and every one a
    /// test said it started.
    pub pids: Vec<u32>,
    pub folders: Vec<PathBuf>,
    /// The process groups of its chats and of their tool commands still running once the host's
    /// quit answered, but for #7103's (`background`).
    #[serde(default)]
    pub groups_left: Vec<u32>,
    /// The process groups the quit left on macOS that a test registered as a chat's background
    /// process's, whose every chat had ended: #7103's expected fail.
    #[serde(default)]
    pub background: Vec<u32>,
    /// Each quit of the host that was not answered, and why.
    #[serde(default)]
    pub quits: Vec<String>,
    /// Each chat process the teardown could not watch by its process group, and why.
    #[serde(default)]
    pub unwatched: Vec<String>,
}

/// **The kit the scenario host arms each chat with: what the app ships** (T1.1-06), hand-built
/// because `Composes::shipped` finds its parts beside the app's executable and in Tauri's
/// resources, which a test has not.
#[derive(Debug, Clone)]
pub(crate) struct Kit {
    pub binary: PathBuf,
    pub plugin: PathBuf,
    pub git_hooks: purlis_core::githooks::GitHooks,
    pub shims: purlis_core::shellguard::Shims,
    pub skill_sets: PathBuf,
    pub chat_settings: PathBuf,
}

impl Kit {
    /// The kit, its writable parts in `data` (the app's data folder), or why there is none.
    pub(crate) fn shipped(data: &Path) -> Result<Kit, String> {
        let binary = std::env::var_os(BINARY_VAR)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .ok_or_else(|| {
                format!(
                    "{BINARY_VAR} names no `purlis`: build one apart from the test build \
                     (`cargo build -p purlis-cli --target-dir target/scenario-kit`) and name it"
                )
            })?;
        if !binary.is_file() {
            return Err(format!(
                "{BINARY_VAR} names {}, which is not there",
                binary.display()
            ));
        }
        let plugin = repo().join("app/src-tauri/plugin");
        let app = data.join(purlis_core::names::BUNDLE_ID.write);
        let git_hooks = purlis_core::githooks::GitHooks::at(app.join("git-hooks"));
        git_hooks
            .write(&binary)
            .map_err(|why| format!("the git hooks were not written: {why}"))?;
        let shims = purlis_core::shellguard::Shims::at(app.join("shims"));
        shims
            .write(&binary)
            .map_err(|why| format!("the shims were not written: {why}"))?;
        Ok(Kit {
            binary,
            plugin,
            git_hooks,
            shims,
            skill_sets: purlis_core::hoststart::skill_sets_in(&app),
            chat_settings: purlis_core::hoststart::chat_settings_in(&app),
        })
    }

    /// The host's half: what `Composes` arms a chat the host composes with.
    pub(crate) fn for_the_host(&self) -> purlis_core::hoststart::Kit {
        purlis_core::hoststart::Kit {
            binary: Some(self.binary.clone()),
            plugin: Some(self.plugin.clone()),
            git_hooks: Some(self.git_hooks.clone()),
            skill_sets: Some(self.skill_sets.clone()),
            chat_settings: Some(self.chat_settings.clone()),
        }
    }

    /// The window's half: what its `Planes` arm a chat the window composes with.
    pub(crate) fn for_the_window(&self) -> crate::Shipped {
        crate::Shipped {
            binary: Some(self.binary.clone()),
            plugin: Some(self.plugin.clone()),
            shims: Some(self.shims.clone()),
            git_hooks: Some(self.git_hooks.clone()),
        }
    }
}

/// **Whether purlis's OS wrap applies in this process**, or why not: it does not nest inside
/// another sandbox, such as a sandboxed task chat's.
pub(crate) fn the_wrap_applies() -> Result<(), String> {
    let mut probe = if cfg!(target_os = "macos") {
        let mut probe = std::process::Command::new("/usr/bin/sandbox-exec");
        probe.args(["-p", "(version 1)(allow default)", "/usr/bin/true"]);
        probe
    } else {
        let mut probe = std::process::Command::new("bwrap");
        probe.args(["--ro-bind", "/", "/", "--unshare-user", "true"]);
        probe
    };
    match purlis_core::forklock::status(&mut probe) {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(format!("purlis's wrap does not apply here ({status})")),
        Err(why) => Err(format!("purlis's wrap does not apply here ({why})")),
    }
}

/// **What the scenario host's own `Confine` started**: each confinement's plan and its board's
/// hold, so a test sees the timings the host side was given.
#[derive(Debug, Clone)]
pub(crate) struct Started {
    pub planned: purlis_host::Planned,
    pub holds_asks_for: Option<Duration>,
    /// The chat's own temp folder beside its proxy, where it has one.
    pub tmp: Option<PathBuf>,
    /// Its ssh route's folder, where it has one.
    pub ssh: Option<String>,
    /// The plan the host was handed for it: what its proxy carries and gives.
    pub plan: purlis_session_protocol::session::ConfinePlan,
}

/// **The scenario host's `Confine`** (T1.1-06): the production `ConfinesHere` (a unit struct, so
/// production's timings), with the scenario's scaled [`Timings`] and its resolver seam handed to
/// `sandbox::hosted::start_through`.
pub(crate) struct Confines {
    timings: Timings,
    names: Arc<Names>,
    started: Mutex<Vec<Started>>,
}

impl Confines {
    pub(crate) fn new(timings: Timings, names: Arc<Names>) -> Arc<Confines> {
        Arc::new(Confines {
            timings,
            names,
            started: Mutex::default(),
        })
    }

    /// Every confinement started so far, in order.
    pub(crate) fn started(&self) -> Vec<Started> {
        self.started.lock().expect("started").clone()
    }
}

/// The host's handle on [`Confines`].
pub(crate) struct ConfinesInTheScenario(pub Arc<Confines>);

impl purlis_host::Confine for ConfinesInTheScenario {
    fn fits(&self, confining: &purlis_host::ToConfine<'_>) -> Result<(), String> {
        crate::hostedconfinement::ConfinesHere.fits(confining)
    }

    fn gives_now(&self, project_path: &Path, port: u16) -> Result<(), String> {
        crate::hostedconfinement::ConfinesHere.gives_now(project_path, port)
    }

    fn confine(
        &self,
        confining: &purlis_host::ToConfine<'_>,
        tell: purlis_host::Tell,
        askable: purlis_host::Askable,
    ) -> Result<Box<dyn purlis_host::Confining>, String> {
        let hosted = purlis_core::sandbox::hosted::start_through(
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
            &self.0.timings,
            Arc::clone(&self.0.names),
        )?;
        self.0.started.lock().expect("started").push(Started {
            planned: confining.planned,
            holds_asks_for: hosted.holds_asks_for(),
            tmp: hosted.confined().tmp.map(PathBuf::from),
            ssh: hosted.confined().ssh,
            plan: confining.confine.plan.clone(),
        });
        Ok(crate::hostedconfinement::held_here(hosted))
    }
}

/// How many variants `grant::What` has, as [`kind_of`] names them.
pub(crate) const GRANT_KINDS: usize = 6;

/// **Each `grant::What` variant's place**, by a `match` with no wildcard: a variant added to
/// `What` fails to compile here, and once named, the maximal chat must hold one
/// ([`one_of_every_kind`]) or `the_maximal_chat_holds_every_grant_kind` fails.
pub(crate) fn kind_of(what: &purlis_core::sandbox::grant::What) -> usize {
    use purlis_core::sandbox::grant::What;
    match what {
        What::Host(_) => 0,
        What::Write(_) => 1,
        What::Socket(_) => 2,
        What::Service(_) => 3,
        What::Port(_) => 4,
        What::Listen(_) => 5,
    }
}

/// **The maximal chat's grants**: one of every `grant::What` kind, in the project at `root`,
/// its socket in `sockets`, a folder no chat may write.
pub(crate) fn one_of_every_kind(
    root: &Path,
    sockets: &Path,
) -> Vec<purlis_core::sandbox::grant::What> {
    use purlis_core::sandbox::grant::What;
    let granted = root.join("granted");
    std::fs::create_dir_all(&granted).expect("a folder to grant");
    vec![
        What::Host(
            purlis_core::sandbox::hosts::Host::parse("granted.example.test").expect("a host"),
        ),
        What::Write(granted.clone()),
        What::Socket(sockets.join("service.sock")),
        What::Service("com.example.granted".to_owned()),
        What::Port(15432),
        What::Listen(18787),
    ]
}

/// **One scenario**: the session host and the window on it, one project of the protection and
/// persona asked for, and the means to start the dialect's chats in it.
pub(crate) struct Scenario {
    pub protection: Protection,
    pub persona: Option<String>,
    pub dialect: Dialect,
    pub template: Template,
    pub timings: Timings,
    pub names: Arc<Names>,
    pub confines: Arc<Confines>,
    pub kit: Kit,
    pub homes: Homes,
    /// Where the host keeps the device's event log for this scenario.
    pub log: PathBuf,
    pub root: PathBuf,
    pub plane: PlaneId,
    /// The session host, its own code, while it runs.
    hosting: Option<purlis_host::Hosting>,
    /// The window on it, while there is one.
    window: Option<Window>,
    /// Each chat it started, as the next one's number is counted (the canary's is left out).
    chats: Mutex<Vec<u32>>,
    /// Each chat's own transcript file, by chat id: a step name is the scenario's, so two chats
    /// playing the same step name keep their results apart.
    transcripts: Mutex<Vec<(u32, std::path::PathBuf)>>,
    /// How many chats it has started, the canary's included: a log with no event after one is
    /// read from the wrong place.
    started: std::sync::atomic::AtomicUsize,
    /// Each chat's pane, as the window's view of it draws it.
    panes: Mutex<Vec<(u32, crate::behaviour::Pane)>>,
    /// How each chat was launched, as the window composed it: what a red run's reader needs.
    launched: Mutex<Vec<String>>,
    /// The profile its chats start on, made once.
    profiled: std::sync::Once,
    /// Processes a test says the scenario started besides its chats.
    also: Mutex<Vec<u32>>,
    /// Processes a test says a chat left running in the background
    /// ([`Self::left_in_the_background`]): only their process groups can be #7103's.
    background: Mutex<Vec<u32>>,
    /// Every chat process it saw run, and its process group, an ended chat's included: what
    /// its teardown checks for leftovers.
    seen: Mutex<Vec<(u32, Option<u32>)>>,
    /// Each running chat process whose process group could not be read, and why: the teardown
    /// cannot watch it, so the parent fails on it.
    unwatched: Mutex<Vec<String>>,
    /// Folders it made outside its home ([`Self::a_folder_outside`]), which its teardown removes.
    outside: Mutex<Vec<PathBuf>>,
    /// Each chat's sandbox as the window composed it at its start, by its number.
    applied: Mutex<Vec<(u32, Option<purlis_core::sandbox::Applied>)>>,
    /// When it was made, for its run time (T1.1-06: ticket 54 sets its budgets from these).
    began: std::time::Instant,
    /// Ended last: the runtime the host and the link run on.
    runtime: Option<tokio::runtime::Runtime>,
    /// What the person, the queries and the global checks keep (T1.1-07).
    pub(crate) ledger: super::checks::Ledger,
    /// A real-harness leg at Full owes its canary ([`Self::for_real_harness`]): its teardown
    /// fails if it never ran ([`canary_owed`]).
    owes_the_canary: bool,
    /// The leg's canary ran to its end ([`Self::the_canary_ran`]).
    canary_ran: std::sync::atomic::AtomicBool,
}

/// **The leg's canary is owed and was not run**: a real-harness scenario at Full whose canary
/// never ran has said nothing about what holds the harness, so it fails.
pub(crate) fn canary_owed(owes: bool, ran: bool) -> Result<(), String> {
    if owes && !ran {
        Err(
            "the real harness leg's canary never ran: no cell of the leg may say what holds \
             the harness without a refused write and a refused connect seen first \
             (`real_leg::canary`)"
                .to_owned(),
        )
    } else {
        Ok(())
    }
}

impl Scenario {
    /// **The scenario host for `protection`, `persona` and `dialect`**, under `strict`
    /// ([`DEFAULT_TEMPLATE`]), in this scenario's own homes; `None` where it cannot run here
    /// (never on CI).
    pub(crate) fn new(
        homes: &Homes,
        protection: Protection,
        persona: Option<&str>,
        dialect: Dialect,
    ) -> Option<Scenario> {
        Scenario::under(homes, protection, persona, dialect, DEFAULT_TEMPLATE)
    }

    /// [`Scenario::new`], under `template`: its `[policy]` in the project's manifest, the
    /// person's press where the window would ask for one, and every chat's start checked to
    /// resolve it ([`runs_under`]).
    pub(crate) fn under(
        homes: &Homes,
        protection: Protection,
        persona: Option<&str>,
        dialect: Dialect,
        template: Template,
    ) -> Option<Scenario> {
        Scenario::built(homes, protection, persona, dialect, template, true)
    }

    /// **The scenario host for a real harness** (T1.1-10, T1.1-53): `protection`, under `strict`,
    /// with no canary of the fake's own. Claude Code on macOS runs its own sandbox, which the
    /// fake harness does not play, so at Full this runs the leg's own canary through the real
    /// binary and `model` ([`super::real_leg::canary`]) before it answers: no cell of the leg
    /// starts without it, and a scenario at Full whose canary did not run fails its teardown
    /// ([`canary_owed`]). The others are wrapped by purlis, whose wrap the fake's canary would
    /// prove of the fake alone.
    pub(crate) fn for_real_harness(
        homes: &Homes,
        protection: Protection,
        dialect: Dialect,
        model: &super::real_leg::Model,
    ) -> Option<Scenario> {
        let mut scenario =
            Scenario::built(homes, protection, None, dialect, DEFAULT_TEMPLATE, false)?;
        if protection == Protection::Full {
            scenario.owes_the_canary = true;
            super::real_leg::canary(&scenario, model);
        }
        Some(scenario)
    }

    /// The leg's canary ran to its end, its verdict judged.
    pub(crate) fn the_canary_ran(&self) {
        self.canary_ran
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    fn built(
        homes: &Homes,
        protection: Protection,
        persona: Option<&str>,
        dialect: Dialect,
        template: Template,
        with_the_fakes_canary: bool,
    ) -> Option<Scenario> {
        if let Err(why) = the_wrap_applies() {
            return cannot_run(&why);
        }
        let kit = match Kit::shipped(&homes.data) {
            Ok(kit) => kit,
            Err(why) => return cannot_run(&why),
        };
        crate::sandboxing::scenarios_wrap_with(&kit.binary);
        let began = std::time::Instant::now();
        // The device's event log, a folder of this scenario's own: one before it in the same test
        // may still be letting go of its own. The `args` digests are reproducible: the key is
        // there before the host opens its log.
        let timings = Timings::scaled(SCALE);
        let names = Names::new();
        let confines = Confines::new(timings, Arc::clone(&names));

        let root = (1..)
            .map(|n| homes.projects.join(n.to_string()))
            .find(|root| !root.exists())
            .expect("a project's folder");
        std::fs::create_dir_all(&root).expect("the project");
        let root = root.canonicalize().expect("the project");
        let log = homes.data.join(format!(
            "log-{}",
            root.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
        ));
        std::fs::create_dir_all(&log).expect("the event log's folder");
        purlis_core::eventlog::ArgsKey::open(&log).expect("the args key");
        std::fs::write(
            root.join(purlis_core::plane::MANIFEST),
            format!("{}{}", protection.manifest(), policy_of(template)),
        )
        .expect("the project's manifest");
        if let Some(persona) = persona {
            let folder = root.join("personas").join(persona);
            std::fs::create_dir_all(&folder).expect("its persona");
            std::fs::write(folder.join("persona.md"), format!("# {persona}\n"))
                .expect("its persona");
        }

        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("a runtime");
        let hosting = a_host(&runtime, &log, homes, &kit, &confines);
        let window = a_window(&runtime, &hosting, homes, &kit, timings);
        let plane = window.planes().open(&root);
        reopened(&window, &plane, &root);
        runs_under(template, &root, |shown| {
            tauri::async_runtime::block_on(crate::policypress::accept_project_policy(
                tauri::Manager::state(&window.app),
                plane.clone(),
                shown,
            ))
            .expect("the person accepts the project's [policy] in the window");
        });
        let scenario = Scenario {
            protection,
            persona: persona.map(str::to_owned),
            dialect,
            template,
            timings,
            names,
            confines,
            kit,
            homes: homes.clone(),
            log,
            root,
            plane,
            hosting: Some(hosting),
            window: Some(window),
            chats: Mutex::default(),
            transcripts: Mutex::default(),
            started: std::sync::atomic::AtomicUsize::new(0),
            panes: Mutex::default(),
            launched: Mutex::default(),
            profiled: std::sync::Once::new(),
            also: Mutex::default(),
            background: Mutex::default(),
            seen: Mutex::default(),
            unwatched: Mutex::default(),
            outside: Mutex::default(),
            applied: Mutex::default(),
            began,
            runtime: Some(runtime),
            ledger: super::checks::Ledger::default(),
            owes_the_canary: false,
            canary_ran: std::sync::atomic::AtomicBool::new(false),
        };
        scenario.ledger.begins(&scenario);
        // The canary, in every scenario whose protection is Full, where the project's own wall is
        // up: a write outside the project and a direct dial are both refused. Protection off has
        // no wrap, and the floor (basic) writes every folder but what it holds
        // (`docs/plane-format.md`, `[sandbox].mode`: "every folder is written but what the floor
        // holds"), so a write outside the project is not
        // refused there and the canary is not asked of it. Claude Code on macOS runs its own
        // sandbox, which the fake harness does not play (ticket 10, #6900): the canary is not
        // run there, and the report says so.
        if protection == Protection::Full && with_the_fakes_canary {
            if dialect == Dialect::Claude && cfg!(target_os = "macos") {
                super::report::write(&[super::a_fixture_record(
                    "the canary",
                    None,
                    None,
                    purlis_scenarios::model::ReportVerdict::NotRun,
                    "Claude Code on macOS, at Full: it runs its own sandbox, which the fake harness does not \
                     play: the canary's refused write and refused connect are not asked (#6900)",
                )])
                .expect("the leg's report");
            } else {
                scenario.canary();
            }
        }
        Some(scenario)
    }

    /// The window as it is now.
    pub(crate) fn window(&self) -> &Window {
        self.window.as_ref().expect("a window")
    }

    /// **`restart: window`, the crash and reattach** (T1.1-06): every link of the window is cut
    /// as its crash cuts them, and the window goes with nothing said to the host; the host and
    /// its chats live on. A new window then comes up on the same host and homes, opens the
    /// project, and takes its chats back as a relaunch after a crash does.
    pub(crate) fn the_window_crashes_and_comes_back(&mut self) {
        let window = self.window.take().expect("a window");
        window.cut();
        // As a crash: no let-go, no quit. Its runtime-bound pieces end with the app.
        std::mem::forget(window.planes().held(&self.plane));
        drop(window);
        let runtime = self.runtime.as_ref().expect("the runtime");
        let hosting = self.hosting.as_ref().expect("the host");
        let window = a_window(runtime, hosting, &self.homes, &self.kit, self.timings);
        self.plane = window.planes().open(&self.root);
        self.window = Some(window);
        reopened(self.window(), &self.plane, &self.root);
    }

    /// **`restart: quit`, a person's quit and relaunch** (T1.1-06; `sessionhost.rs`): the window
    /// lets go of its projects and tells the host to quit, which ends every chat and stops; then a
    /// new host and a new window start on the same homes, and the window relaunches the live chats.
    ///
    /// The host must answer the quit, and every chat process it ran must have ended by then,
    /// before the old host is dropped: dropping it ends its programs too, which would hide a
    /// quit that ended nothing.
    pub(crate) fn quits_and_relaunches(&mut self) {
        self.note_running();
        let running = self.pids();
        let window = self.window.take().expect("a window");
        // As the app's quit does: the record keeps every chat that was live, for the relaunch.
        window.planes().let_go_of_all_to_update();
        if let Err(why) = window.quits() {
            panic!("the host did not answer the window's quit: {why}");
        }
        drop(window);
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while running.iter().any(|pid| purlis_core::process::alive(*pid))
            && std::time::Instant::now() < deadline
        {
            std::thread::sleep(Duration::from_millis(50));
        }
        let still: Vec<u32> = running
            .iter()
            .copied()
            .filter(|pid| purlis_core::process::alive(*pid))
            .collect();
        assert!(
            still.is_empty(),
            "the host answered its quit with chat processes {still:?} still running"
        );
        self.hosting.take();
        let runtime = self.runtime.as_ref().expect("the runtime");
        let hosting = a_host(runtime, &self.log, &self.homes, &self.kit, &self.confines);
        let window = a_window(runtime, &hosting, &self.homes, &self.kit, self.timings);
        self.plane = window.planes().open(&self.root);
        self.hosting = Some(hosting);
        self.window = Some(window);
        reopened(self.window(), &self.plane, &self.root);
    }

    /// The window's `Planes`, as the app manages them.
    pub(crate) fn planes(&self) -> tauri::State<'_, Planes> {
        self.window().planes()
    }

    /// The Tauri mock app managing the window's `Planes`, whose commands the person calls.
    pub(crate) fn app(&self) -> &tauri::App<MockRuntime> {
        &self.window().app
    }

    /// The project as the window holds it.
    pub(crate) fn held(&self) -> Arc<Held> {
        self.planes()
            .held(&self.plane)
            .expect("the project is held")
    }

    /// **Copies `script` into this scenario's own folder** and answers where: a resume cursor
    /// (T1.1-04) never touches the source tree or another scenario's.
    pub(crate) fn script(&self, name: &str, script: &serde_json::Value) -> PathBuf {
        let at = self.homes.scripts.join(format!("{name}.json"));
        std::fs::write(&at, serde_json::to_vec_pretty(script).expect("JSON")).expect("the script");
        at
    }
}

/// **A Seatbelt profile, as a red run's reader needs it**: its size, its lines, and the ten
/// rule openings it repeats most (its first 40 characters), so a profile `sandbox-exec` refuses
/// as too large says what fills it.
fn profile_said(profile: Result<String, &'static str>) -> String {
    let profile = match profile {
        Ok(profile) => profile,
        Err(why) => return format!("none: {why}"),
    };
    let mut kinds: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for line in profile.lines() {
        *kinds
            .entry(line.trim().chars().take(40).collect())
            .or_default() += 1;
    }
    let mut most: Vec<(usize, String)> = kinds.into_iter().map(|(k, n)| (n, k)).collect();
    most.sort_by(|a, b| b.cmp(a));
    most.truncate(10);
    format!(
        "{} bytes, {} lines; most: {most:?}",
        profile.len(),
        profile.lines().count()
    )
}

/// The process group of each of `pids`, as `ps` says it, each once, never this test's own.
fn process_groups(pids: &[u32]) -> Vec<u32> {
    let own = process_group_of(std::process::id());
    let mut groups: Vec<u32> = Vec::new();
    for pid in pids {
        if let Some(group) = process_group_of(*pid)
            .filter(|group| *group > 1 && Some(*group) != own && !groups.contains(group))
        {
            groups.push(group);
        }
    }
    groups
}

/// `pid`'s process group, as `ps` says it.
fn process_group_of(pid: u32) -> Option<u32> {
    let out = purlis_core::forklock::output(std::process::Command::new("/bin/ps").args([
        "-o",
        "pgid=",
        "-p",
        &pid.to_string(),
    ]))
    .ok()?;
    String::from_utf8_lossy(&out.stdout).trim().parse().ok()
}

/// **Which of the process groups the host's quit left are #7103's** (macOS): a group is that
/// known leak only when a test registered it as the group of a process a chat left in the
/// background (`background`, [`Scenario::left_in_the_background`]) and every chat process seen
/// in it has ended. Any other group the quit left is a leak, on every OS: a chat that still
/// runs, and a tool's group nobody registered, which no chat was ever seen in. Answers the
/// groups that are #7103's and the groups that are leaks.
pub(crate) fn of_the_quit(
    still: &[u32],
    seen: &[(u32, Option<u32>)],
    background: &[u32],
    alive: impl Fn(u32) -> bool,
    macos: bool,
) -> (Vec<u32>, Vec<u32>) {
    still.iter().copied().partition(|group| {
        macos
            && background.contains(group)
            && seen
                .iter()
                .filter(|(_, of)| *of == Some(*group))
                .all(|(pid, _)| !alive(*pid))
    })
}

/// The file each chat of a scenario's fake harness writes the process group of every tool
/// command it starts to, one per line (`FAKE_HARNESS_GROUPS`): a tool's command runs in a group
/// of its own, which no chat's process is in.
fn tool_groups_file(root: &Path) -> PathBuf {
    own_folder(root).join("tool-groups")
}

/// The process group of every tool command the chats of the project at `root` started.
fn tool_groups(root: &Path) -> Vec<u32> {
    std::fs::read_to_string(tool_groups_file(root))
        .unwrap_or_default()
        .lines()
        .filter_map(|line| line.trim().parse().ok())
        .collect()
}

/// **Whether the tool-group channel lost a command** (#7197): every shell command a chat's fake
/// harness ran says its process group in `FAKE_HARNESS_GROUPS` before the teardown reads it, so
/// fewer `said` lines than shell results in the `transcript` means a group nobody watches (a
/// variable not passed on, a failed write, an unreadable file). Answers what to say, or none.
pub(crate) fn tool_groups_lost(transcript: &[serde_json::Value], said: usize) -> Option<String> {
    let ran = transcript
        .iter()
        .filter(|line| line["kind"] == "tool_result" && line["exit"].is_number())
        .filter(|line| {
            !(line["exit"] == 127
                && line["stderr"]
                    .as_str()
                    .is_some_and(|e| e.starts_with("cannot run the shell")))
        })
        .count();
    (said < ran).then(|| {
        format!(
            "the tool-group channel (FAKE_HARNESS_GROUPS) holds {said} process groups for {ran} \
             shell commands the chats ran: a tool command's group is not watched"
        )
    })
}

/// Whether any process of `group` runs.
fn group_alive(group: u32) -> bool {
    purlis_core::forklock::status(std::process::Command::new("/bin/kill").args([
        "-0",
        "--",
        &format!("-{group}"),
    ]))
    .is_ok_and(|status| status.success())
}

/// The size every scenario's chat is drawn at.
pub(crate) const SIZE: purlis_core::engine::Size = purlis_core::engine::Size {
    columns: 160,
    rows: 50,
};

impl Scenario {
    /// **Starts a chat of the scenario's dialect playing `script`**, as the person starts one
    /// from the window: on the sandbox-only profile `work` this machine signed in, its turn script
    /// copied into the scenario's own folder, its transcript and resume cursor in a folder of the
    /// project's that it writes inside the wrap, and the script's first prompt typed. Answers the
    /// chat's number.
    pub(crate) fn starts(&self, script: &serde_json::Value) -> u32 {
        self.starts_holding(script, purlis_core::sandbox::grant::Grants::default())
    }

    /// [`Self::starts`], the chat holding `grants`, as the person allowed them for it alone.
    pub(crate) fn starts_holding(
        &self,
        script: &serde_json::Value,
        grants: purlis_core::sandbox::grant::Grants,
    ) -> u32 {
        self.profiled.call_once(|| {
            let homes = purlis_core::profilestore::Homes::here().expect("the scenario's homes");
            // Once per machine: a scenario before this one in the same test made it already.
            if let Err(why) = purlis_core::profilestore::create_sandbox_only(
                &homes,
                &self.profile(),
                self.dialect.word(),
                1,
            ) {
                assert!(why.contains("already has a profile"), "the profile: {why}");
            }
        });
        let number = self.chats.lock().expect("chats").len() + 1;
        let at = self.script(&format!("chat-{number}"), script);
        let own = own_folder(&self.root);
        std::fs::create_dir_all(&own).expect("the chat's own folder");
        let transcript = own.join(format!(
            "{}-{}.jsonl",
            TRANSCRIPT.trim_end_matches(".jsonl"),
            self.started.load(std::sync::atomic::Ordering::SeqCst) + 1
        ));
        let name = format!("chat {number}");
        let mut ready = purlis_core::start::ready(
            &purlis_core::start::Start {
                profile: Some(self.profile()),
                persona: self.persona.clone(),
                name: name.clone(),
                cwd: Some(self.root.clone()),
                made: purlis_core::sandbox::Made::Window,
                grants,
                ..Default::default()
            },
            &self.root,
        )
        .expect("the chat is ready to start");
        let places = serde_json::json!({"work": self.root.display().to_string()});
        ready.env.extend(
            [
                ("FAKE_HARNESS_SCRIPT", at.display().to_string()),
                ("FAKE_HARNESS_TRANSCRIPT", transcript.display().to_string()),
                (
                    "FAKE_HARNESS_STATE",
                    own.join("fake-state").display().to_string(),
                ),
                (
                    "FAKE_HARNESS_BIN",
                    crate::behaviour::fake_harness().display().to_string(),
                ),
                ("FAKE_HARNESS_PLACES", places.to_string()),
                (
                    "FAKE_HARNESS_GROUPS",
                    tool_groups_file(&self.root).display().to_string(),
                ),
            ]
            .map(|(k, v)| (k.to_owned(), v)),
        );
        let id = self.launches(&name, &ready, &transcript);
        if let Some(prompt) = script["turns"][0]["prompt"].as_str() {
            // Typed once the harness says its session, as a person types at a harness that is
            // ready: a harness that takes the terminal raw drops what came before.
            let said = format!("{} session ", self.dialect.word());
            let deadline = std::time::Instant::now() + crate::scenarios::PLAYS_WITHIN;
            while !self.screen_of(id).contains(&said) {
                assert!(
                    std::time::Instant::now() < deadline,
                    "chat {id} never said its session\n{}",
                    self.said()
                );
                std::thread::sleep(Duration::from_millis(100));
            }
            self.types(id, prompt);
        }
        self.note_running();
        id
    }

    /// **Starts the chat `ready` composed for, as the window does**, and keeps what the scenario
    /// reads of it: how it was launched, its transcript file, its sandbox and its pane. Answers
    /// the chat's number.
    fn launches(
        &self,
        name: &str,
        ready: &purlis_core::start::Ready,
        transcript: &std::path::Path,
    ) -> u32 {
        let chat = purlis_core::reopen::Chat {
            program: ready.program.clone(),
            cwd: ready.cwd.clone(),
            name: name.to_owned(),
            resume: ready.session.clone(),
            profile: Some(self.profile()),
            persona: self.persona.clone(),
            ..Default::default()
        };
        let launched = format!(
            "{name}: {} {:?}, sandbox {:?}, wrapped by {:?}, form {:?}",
            ready.program,
            ready.args,
            ready
                .sandbox
                .as_ref()
                .map(purlis_core::sandbox::Applied::level),
            ready
                .sandbox
                .as_ref()
                .and_then(purlis_core::sandbox::Applied::wrap_program),
            ready
                .sandbox
                .as_ref()
                .map(|applied| format!("{:?}", applied.form())
                    .chars()
                    .take(60)
                    .collect::<String>()),
        );
        let launched = match ready
            .sandbox
            .as_ref()
            .map(purlis_core::sandbox::Applied::form)
        {
            Some(purlis_core::sandbox::Form::Codex(wrap)) => format!(
                "{launched}\n  its Seatbelt profile: {}",
                profile_said(purlis_core::sandbox::codex::profile_serving(
                    wrap,
                    &self.root,
                    &std::env::temp_dir(),
                    &[1, 2],
                    None,
                    None,
                ))
            ),
            _ => launched,
        };
        self.launched.lock().expect("launched").push(launched);
        let held = self.held();
        let id = held
            .chats()
            .start_ready(&chat, ready, SIZE)
            .expect("the chat starts");
        self.chats.lock().expect("chats").push(id);
        self.transcripts
            .lock()
            .expect("transcripts")
            .push((id, transcript.to_path_buf()));
        self.started
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.applied
            .lock()
            .expect("applied")
            .push((id, ready.sandbox.clone()));
        if let Ok(pane) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            crate::behaviour::Pane::of(held.chats().sessions(), id)
        })) {
            self.panes.lock().expect("panes").push((id, pane));
        }
        id
    }

    /// **Starts the real harness** (T1.1-10, T1.1-53) as the person starts a chat from the
    /// window: through [`purlis_core::start::ready`] and the host's own chat start
    /// ([`Self::launches`]), so the launch line, the environment, the bundled plugin and the
    /// settings are what purlis writes, never a hand launch. The sandbox-only profile is signed
    /// in with the first-run state of an interactive start where purlis does not seed it
    /// ([`super::real_leg::seed_first_run`], Claude Code), what the leg adds to the
    /// keep-list is named in the project's local `[chat_env] pass`
    /// ([`super::real_leg::chat_env_pass`]) with its values in this process's environment, the
    /// stand-in's port is granted where purlis walls the chat's network, and the stand-in model
    /// answers the chat's model calls, its request log being the transcript
    /// ([`super::real_leg::Model::attach`]). Answers the chat's number.
    pub(crate) fn starts_real(&self, model: &super::real_leg::Model, prompt: &str) -> u32 {
        self.start_real(model, prompt, false)
    }

    /// [`Self::starts_real`] with the first message typed once the harness has begun its session
    /// (its SessionStart hook has answered), as a person types at a harness that is ready,
    /// instead of given on its command line.
    pub(crate) fn starts_real_typing(&self, model: &super::real_leg::Model, prompt: &str) -> u32 {
        self.start_real(model, prompt, true)
    }

    fn start_real(&self, model: &super::real_leg::Model, prompt: &str, typing: bool) -> u32 {
        let dialect = self.dialect;
        self.profiled.call_once(|| {
            let homes = purlis_core::profilestore::Homes::here().expect("the scenario's homes");
            if let Err(why) = purlis_core::profilestore::create_sandbox_only(
                &homes,
                &self.profile(),
                dialect.word(),
                1,
            ) {
                assert!(why.contains("already has a profile"), "the profile: {why}");
            }
        });
        if dialect == Dialect::Claude {
            let config = purlis_core::profilestore::listed_here()
                .into_iter()
                .find(|profile| profile.name == self.profile())
                .map(|profile| profile.folders.config)
                .expect("the sandbox-only profile is listed");
            super::real_leg::seed_first_run(&config, &self.root);
        }
        std::fs::write(
            self.root.join(purlis_core::profiles::LOCAL_FILE),
            super::real_leg::chat_env_pass(dialect),
        )
        .expect("the project's local settings");
        if !super::real_leg::holds_itself(dialect) {
            // purlis walls the chat's network: the stand-in is reached by its granted loopback
            // port, as the recordings' chats reach it (`tests/recordings/tests/record.rs`).
            purlis_core::sandbox::local::grant_port(&self.root, model.port())
                .expect("the stand-in's port is granted");
            self.ledger.setup_grant(self, &model.port().to_string());
        }
        let number = self.chats.lock().expect("chats").len() + 1;
        let own = own_folder(&self.root);
        std::fs::create_dir_all(&own).expect("the chat's own folder");
        // Outside the project, which a harness's sandbox may let a chat write: the transcript is
        // the stand-in model's request log, and a chat must not be able to rewrite it (the leg's
        // canary tries, `real_leg::canary`).
        let transcript = self.next_transcript();
        let name = format!("chat {number}");
        let mut ready = purlis_core::start::ready(
            &purlis_core::start::Start {
                profile: Some(self.profile()),
                name: name.clone(),
                cwd: Some(self.root.clone()),
                made: purlis_core::sandbox::Made::Window,
                ..Default::default()
            },
            &self.root,
        )
        .expect("the chat is ready to start");
        // What the stand-in model needs of the harness's own words: for Claude Code no permission
        // prompt, which an auto mode would send to Anthropic's servers to judge (the recordings'
        // conditions, `tests/recordings/README.md`), for Codex its model provider, and the prompt
        // it starts with.
        ready.args.extend(super::real_leg::launch_words(
            dialect,
            &model.url(),
            (!typing).then_some(prompt),
        ));
        model.attach(&transcript);
        let sessions_before = self.sessions_begun();
        let id = self.launches(&name, &ready, &transcript);
        if typing {
            let deadline = std::time::Instant::now() + REAL_PLAYS_WITHIN;
            let mut answered = Answered::default();
            while self.sessions_begun() <= sessions_before {
                assert!(
                    std::time::Instant::now() < deadline,
                    "chat {id} never began its session; its screen:\n{}\n{}",
                    self.screen_tail_of(id),
                    self.said()
                );
                self.answers_a_screen(id, true, &mut answered);
                std::thread::sleep(Duration::from_millis(250));
            }
            self.types(id, prompt);
        }
        self.note_running();
        id
    }

    /// How many sessions the event log says harnesses have begun (their SessionStart hook).
    fn sessions_begun(&self) -> usize {
        self.try_events().map_or(0, |events| {
            events
                .iter()
                .filter(|event| event.kind == "hook.sessionstart")
                .count()
        })
    }

    /// **Until chat `id` has a result for each of `steps`**, answering what a real harness asks
    /// a person on its screen as the person does (T1.1-53), within [`REAL_PLAYS_WITHIN`]:
    ///
    /// - a first-run question the harness asks in its terminal before the chat (a folder's trust,
    ///   a key's use, Codex's "Folder access"): Enter on its default, while the model has heard
    ///   nothing ([`Driving::heard`] says it has);
    /// - Codex's offer of a newer model: "Use existing model";
    /// - Claude Code's bypass-permissions warning, which defaults to "No, exit": "Yes, I accept";
    /// - OpenCode's "Permission required" for a folder outside the project: allowed once, so the
    ///   wrap, not the harness, decides the call;
    /// - an ask the proxy raises for a host the leg did not name ([`Driving::held`]), which is the
    ///   harness's own start-up traffic: kept blocked at once, as a person who wants none of it
    ///   would, so the harness is refused now and does not wait out the proxy's hold.
    ///
    /// The same answers `tests/recordings/tests/record.rs` gives its harnesses, bar the last.
    pub(crate) fn answers_until_played(&self, id: u32, steps: &[&str], driving: &Driving<'_>) {
        let deadline = std::time::Instant::now() + REAL_PLAYS_WITHIN;
        let mut answered = Answered::default();
        while !steps.iter().all(|step| self.result_of(id, step).is_some()) {
            assert!(
                std::time::Instant::now() < deadline,
                "chat {id} played none of {steps:?} in time; its screen:\n{}\n{}\n{}\n{}",
                self.screen_tail_of(id),
                (driving.evidence)(),
                self.profile_logs_said(),
                self.said()
            );
            for ask in self.queries().counted().asks {
                let target = super::person::shown_target(&ask);
                if ask.session == id
                    && !target.is_empty()
                    && driving.held != Some(target.as_str())
                    && ask.options.iter().any(|one| one.id == crate::asking::KEEP)
                {
                    let _ = self
                        .person()
                        .presses_on(&ask, &super::person::Press::Keep(None));
                }
            }
            self.answers_a_screen(id, !(driving.heard)(), &mut answered);
            std::thread::sleep(Duration::from_millis(250));
        }
        self.note_running();
    }

    /// **Answers what chat `id`'s screen asks of a person**, once if it asks anything: see
    /// [`Self::answers_until_played`] for the questions. `nothing_heard` is whether the stand-in
    /// model has heard nothing from the harness yet, which a first-run question is answered
    /// only while.
    fn answers_a_screen(&self, id: u32, nothing_heard: bool, answered: &mut Answered) {
        let press = |bytes: &[u8]| {
            let _ = self.held().chats().sessions().input(id, bytes);
        };
        let screen = self.screen_of(id).to_lowercase();
        if answered.first_run < 3
            && nothing_heard
            && [
                "do you trust",
                "trust the files",
                "press enter",
                "enter to confirm",
                "yes, proceed",
                "open restricted",
                "enter continue",
            ]
            .iter()
            .any(|ask| screen.contains(ask))
        {
            press(b"\r");
            answered.first_run += 1;
            std::thread::sleep(Duration::from_secs(2));
        } else if answered.newer_model < 3 && screen.contains("use existing model") {
            press(b"\x1b[B");
            std::thread::sleep(Duration::from_millis(600));
            press(b"\r");
            answered.newer_model += 1;
            std::thread::sleep(Duration::from_secs(2));
        } else if answered.bypass < 5
            && screen.contains("bypass permissions mode")
            && screen.contains("yes, i accept")
        {
            std::thread::sleep(Duration::from_secs(1));
            press(b"\x1b[B");
            std::thread::sleep(Duration::from_millis(600));
            press(b"\r");
            answered.bypass += 1;
            std::thread::sleep(Duration::from_secs(2));
        } else if answered.permission < 8 && screen.contains("permission required") {
            // Once purlis's own hook on the ask has had its say: pressed sooner, OpenCode cuts
            // that hook off (`record.rs`).
            std::thread::sleep(Duration::from_secs(1));
            press(b"\r");
            answered.permission += 1;
            std::thread::sleep(Duration::from_secs(2));
        }
    }

    /// **What the harness's profile folder says in its logs**, for a red run's reader: the tail of
    /// each `*.log` under the sandbox-only profile's config folder (Codex keeps
    /// `log/codex-tui.log` there), at most four files of thirty lines.
    pub(crate) fn profile_logs_said(&self) -> String {
        let Some(config) = purlis_core::profilestore::listed_here()
            .into_iter()
            .find(|profile| profile.name == self.profile())
            .map(|profile| profile.folders.config)
        else {
            return "no profile folder to read logs of".to_owned();
        };
        let mut logs = Vec::new();
        let mut stack = vec![(PathBuf::from(&config), 0)];
        while let Some((dir, depth)) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() && depth < 3 {
                    stack.push((path, depth + 1));
                } else if path.extension().is_some_and(|ext| ext == "log") {
                    logs.push(path);
                }
            }
        }
        logs.sort();
        let mut said = format!("{} log files under {}", logs.len(), config.display());
        for log in logs.into_iter().take(4) {
            let text = std::fs::read_to_string(&log).unwrap_or_default();
            let lines: Vec<&str> = text.lines().collect();
            said.push_str(&format!("\n--- {}:\n", log.display()));
            for line in &lines[lines.len().saturating_sub(30)..] {
                said.push_str(&line.chars().take(300).collect::<String>());
                said.push('\n');
            }
        }
        said
    }

    /// What is on chat `id`'s screen, without its blank lines and cut to a readable width: the
    /// last forty lines, for a red run's reader.
    pub(crate) fn screen_tail_of(&self, id: u32) -> String {
        let lines: Vec<String> = self
            .screen_of(id)
            .lines()
            .map(|line| line.trim().chars().take(200).collect::<String>())
            .filter(|line| !line.is_empty())
            .collect();
        lines[lines.len().saturating_sub(40)..].join("\n")
    }

    /// Where the next chat's transcript is written: outside the project, in the run's scripts
    /// folder ([`Self::starts_real`]).
    pub(crate) fn next_transcript(&self) -> PathBuf {
        self.homes.scripts.join(format!(
            "transcript-{}.jsonl",
            self.started.load(std::sync::atomic::Ordering::SeqCst) + 1
        ))
    }

    /// **A folder of the person's own that no chat may write**: in the run's own home, outside
    /// the temp folder and the project, only this user's (`0700`). The teardown removes it, and
    /// the parent checks it is gone.
    pub(crate) fn a_folder_outside(&self, name: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt as _;
        let home = std::env::var_os(OUTER_HOME_VAR)
            .filter(|home| !home.is_empty())
            .map(PathBuf::from)
            .expect("the run's own home");
        let own = self
            .homes
            .home
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let folder = home.join(format!(".purlis-{own}-{name}"));
        std::fs::create_dir_all(&folder).expect("a folder outside");
        std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o700))
            .expect("only this user's");
        let folder = folder.canonicalize().expect("the folder outside");
        self.outside.lock().expect("outside").push(folder.clone());
        folder
    }

    /// Chat `id`'s sandbox, as the window composed it at its start.
    pub(crate) fn applied_of(&self, id: u32) -> Option<purlis_core::sandbox::Applied> {
        self.applied
            .lock()
            .expect("applied")
            .iter()
            .find(|(chat, _)| *chat == id)
            .and_then(|(_, applied)| applied.clone())
    }

    /// **Notes every chat process running now**, and its process group, so the teardown checks
    /// it even once its chat has ended.
    pub(crate) fn note_running(&self) {
        let running = self.pids();
        let mut seen = self.seen.lock().expect("seen");
        for pid in running {
            if !seen.iter().any(|(one, _)| *one == pid) {
                let group = process_group_of(pid);
                if group.is_none() && purlis_core::process::alive(pid) {
                    self.unwatched.lock().expect("unwatched").push(format!(
                        "chat process {pid} runs, but `ps` said no process group for it"
                    ));
                }
                seen.push((pid, group));
            }
        }
    }

    /// **The person closes chat `id`** in the window: it ends, and is no longer live.
    pub(crate) fn closes(&self, id: u32) {
        self.note_running();
        self.held().chats().close(id).expect("the chat closes");
    }

    /// The name of every chat the window has open now.
    pub(crate) fn open_names(&self) -> Vec<String> {
        let Some(window) = &self.window else {
            return Vec::new();
        };
        let Ok(held) = window.planes().held(&self.plane) else {
            return Vec::new();
        };
        held.chats()
            .open_now()
            .into_iter()
            .map(|open| open.name)
            .collect()
    }

    /// **What a red run's reader needs**: how each chat was launched, the chats open now, the
    /// kinds of event the log holds, and the transcript.
    pub(crate) fn said(&self) -> String {
        let kinds: Vec<String> = self
            .try_events()
            .unwrap_or_default()
            .into_iter()
            .map(|event| {
                let body = event.body.to_string();
                format!(
                    "{} {}",
                    event.kind,
                    body.chars().take(300).collect::<String>()
                )
            })
            .collect();
        // What the window's queries show, where the window can still be asked.
        let queries = if self.window.is_some() {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.queries().said()))
                .unwrap_or_else(|_| "the window's queries could not be read".to_owned())
        } else {
            "no window".to_owned()
        };
        format!(
            "launched: {:#?}\nscreens: {:#?}\nopen now: {:?}\nconfinements: {:?}\n\
             events: {kinds:#?}\nthe window's queries:\n{queries}\ntranscript: {:#?}",
            self.launched.lock().expect("launched"),
            self.panes
                .lock()
                .expect("panes")
                .iter()
                .map(|(id, pane)| format!("chat {id}: {}", pane.text()))
                .collect::<Vec<_>>(),
            self.open_chats(),
            self.confines.started(),
            self.transcript()
        )
    }

    /// Chat `id`'s screen as the window's view of it draws it, as text.
    pub(crate) fn screen_of(&self, id: u32) -> String {
        self.panes
            .lock()
            .expect("panes")
            .iter()
            .find(|(chat, _)| *chat == id)
            .map(|(_, pane)| pane.text())
            .unwrap_or_default()
    }

    /// The sandbox-only profile its chats start on: one per dialect, as a person signs one in.
    fn profile(&self) -> String {
        format!("work-{}", self.dialect.word())
    }

    /// Every event in the device's event log, which the host writes in the data home, or why it
    /// could not be read: never an empty log made of an error.
    pub(crate) fn try_events(&self) -> Result<Vec<purlis_core::eventlog::Event>, String> {
        let (events, unparsed) = purlis_core::eventlog::read_counting_unparsed(&self.log, |_| true)
            .map_err(|why| {
                format!(
                    "the event log at {} could not be read: {why}",
                    self.log.display()
                )
            })?;
        if unparsed > 0 {
            return Err(format!(
                "the event log at {} holds {unparsed} lines that are no event: a check judged \
                 on the rest would pass on what it cannot see",
                self.log.display()
            ));
        }
        Ok(events)
    }

    /// **Writes the record of `cell`, which this scenario ran** (#7224, #7229), to the leg's
    /// report with the kinds of the events its log gained after `since`, the [`Self::latest_seq`]
    /// taken when the cell began: the canary's and earlier cells' events are not the cell's. A log
    /// that cannot be read is an error, never a record with no events. Named by the runner of the cells format, which no scenario has yet:
    /// the cells that run today are Rust tests that name no cell.
    #[expect(dead_code)]
    pub(crate) fn report_cell(
        &self,
        cell: &purlis_scenarios::model::Cell,
        verdict: purlis_scenarios::model::ReportVerdict,
        reason: Option<String>,
        since: u64,
    ) -> Result<(), String> {
        let record = super::report::ran(
            cell,
            self.dialect.in_report(),
            self.protection.in_report(),
            purlis_scenarios::model::TimingsMode::Scaled,
            verdict,
            reason,
            super::report::kinds_since(&self.try_events()?, since),
        );
        super::report::write(&[record]).map_err(|why| format!("the report: {why}"))?;
        Ok(())
    }

    /// [`Self::try_events`] where a scenario's own step reads the log: a log that cannot be read
    /// fails the step.
    pub(crate) fn events(&self) -> Vec<purlis_core::eventlog::Event> {
        self.try_events().unwrap_or_else(|why| panic!("{why}"))
    }

    /// The number of the log's newest event, 0 for an empty log or one that cannot be read.
    pub(crate) fn latest_seq(&self) -> u64 {
        let mut newest = 0;
        let _ = purlis_core::eventlog::read_back(&self.log, |event| {
            newest = event.seq;
            std::ops::ControlFlow::Break(())
        });
        newest
    }

    /// How many chats the scenario has started, the canary's included.
    pub(crate) fn chats_started(&self) -> usize {
        self.started.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// **The canary's chat is closed and forgotten**: it ends, and the next chat is numbered, and
    /// its script named, as the first of the scenario.
    pub(crate) fn forget_the_canary_chat(&self, id: u32) {
        self.closes(id);
        self.chats.lock().expect("chats").retain(|one| *one != id);
    }

    /// How many events of `kind` the log holds.
    pub(crate) fn count_of(&self, kind: &str) -> usize {
        self.events()
            .into_iter()
            .filter(|event| event.kind == kind)
            .count()
    }

    /// **The person types `text` into chat `id`, and Enter**, as T1.1-03's driver types a turn.
    pub(crate) fn types(&self, id: u32, text: &str) {
        let held = self.held();
        held.chats()
            .sessions()
            .input(id, text.as_bytes())
            .expect("typed");
        std::thread::sleep(Duration::from_millis(800));
        held.chats().sessions().input(id, b"\r").expect("Enter");
    }

    /// The scenario's transcript so far, every chat's in the order they started, one value a
    /// line.
    pub(crate) fn transcript(&self) -> Vec<serde_json::Value> {
        let files: Vec<std::path::PathBuf> = self
            .transcripts
            .lock()
            .expect("transcripts")
            .iter()
            .map(|(_, file)| file.clone())
            .collect();
        files.iter().flat_map(|file| lines_of(file)).collect()
    }

    /// The tool result chat `id` was handed for `step`, once it was: read from that chat's own
    /// transcript, so another chat's step of the same name is never taken for it.
    pub(crate) fn result_of(&self, id: u32, step: &str) -> Option<serde_json::Value> {
        let file = self
            .transcripts
            .lock()
            .expect("transcripts")
            .iter()
            .find(|(chat, _)| *chat == id)
            .map(|(_, file)| file.clone())?;
        lines_of(&file)
            .into_iter()
            .find(|line| line["kind"] == "tool_result" && line["step"] == step)
    }

    /// **Until chat `id` has a result for each of `steps`**, within [`crate::scenarios::PLAYS_WITHIN`].
    pub(crate) fn until_played(&self, id: u32, steps: &[&str]) {
        let deadline = std::time::Instant::now() + crate::scenarios::PLAYS_WITHIN;
        while !steps.iter().all(|step| self.result_of(id, step).is_some()) {
            assert!(
                std::time::Instant::now() < deadline,
                "chat {id} played none of {steps:?} in time\n{}",
                self.said()
            );
            std::thread::sleep(Duration::from_millis(100));
        }
        self.note_running();
    }

    /// The scenario started `pid` too, which its teardown must end.
    pub(crate) fn also_started(&self, pid: u32) {
        self.also.lock().expect("also").push(pid);
    }

    /// **A chat left `pid` running in the background**, as #7103's sleeper: its teardown ends
    /// it, and on macOS its process group, and no other, may be #7103's expected fail
    /// ([`of_the_quit`]).
    pub(crate) fn left_in_the_background(&self, pid: u32) {
        self.also_started(pid);
        self.background.lock().expect("background").push(pid);
    }

    /// The process of each chat it started that runs now.
    pub(crate) fn pids(&self) -> Vec<u32> {
        self.open_chats()
            .into_iter()
            .filter_map(|(_, pid)| pid)
            .collect()
    }

    /// Every chat the window has open now, and its process where it runs.
    pub(crate) fn open_chats(&self) -> Vec<(u32, Option<u32>)> {
        let Some(window) = &self.window else {
            return Vec::new();
        };
        let Ok(held) = window.planes().held(&self.plane) else {
            return Vec::new();
        };
        held.chats()
            .open_now()
            .into_iter()
            .map(|open| {
                (
                    open.session,
                    held.chats().sessions().process_id(open.session),
                )
            })
            .collect()
    }
}

impl Drop for Scenario {
    /// **Teardown**: the window lets go of every project and tells the host to quit, which ends
    /// every chat it runs; the runtime goes; then the project's folder. What it started (every
    /// chat process it saw, an ended chat's too), a quit the host did not answer, a chat it
    /// could not watch by its process group, and each process group of a chat or of a tool
    /// command (`FAKE_HARNESS_GROUPS`) still running after the quit are written for the parent,
    /// which fails on any of them. On macOS a group the quit left that a test registered as a
    /// chat's background process's ([`Self::left_in_the_background`]), whose every chat has
    /// ended, is #7103: an expected fail, said and ended ([`of_the_quit`]).
    fn drop(&mut self) {
        // **The global checks** (T1.1-07), judged first, while the window and its chats are
        // still up, and failed last, once the teardown below has run: a scenario that passed
        // its own steps but breaks one of them is red (`checks`).
        // A scenario with no window at its end is not skipped: it fails, so that no scenario is
        // green without having been checked. A panicking one is red already.
        let verdict = if std::thread::panicking() {
            None
        } else {
            Some(
                super::checks::window_for_the_checks(self.window.is_some())
                    .and_then(|()| {
                        canary_owed(
                            self.owes_the_canary,
                            self.canary_ran.load(std::sync::atomic::Ordering::SeqCst),
                        )
                    })
                    .and_then(|()| super::checks::at_the_end(self)),
            )
        };
        let verdict = verdict.map(|checked| match checked {
            Ok(outcome) if outcome.failures.is_empty() => {
                for (check, issue, what) in &outcome.expected {
                    let _ = writeln!(
                        std::io::stderr(),
                        "expected fail #{issue}: {}: {what}",
                        check.word()
                    );
                    let _ = super::report::write(&[purlis_scenarios::model::LegRecord {
                        kind: purlis_scenarios::model::EntryKind::Fixture,
                        row: "T1.1-07".to_owned(),
                        test: std::thread::current().name().map(str::to_owned),
                        dialect: None,
                        protection: None,
                        os: super::report::this_os(),
                        leg: purlis_scenarios::model::ReportLeg::Fixture,
                        timings: Some(purlis_scenarios::model::TimingsMode::Scaled),
                        verdict: purlis_scenarios::model::ReportVerdict::ExpectedFail,
                        reason: Some(format!("#{issue}: {}: {what}", check.word())),
                        refusal_expected: false,
                    }]);
                }
                Ok(())
            }
            Ok(outcome) => Err(format!(
                "a global check failed:\n  {}\n{}",
                outcome.failures.join("\n  "),
                self.said()
            )),
            Err(why) => Err(format!(
                "the global checks could not run, or the window drew an ask wrongly: {why}\n{}",
                self.said()
            )),
        });
        self.note_running();
        let seen = self.seen.lock().expect("seen").clone();
        let mut left = Left {
            pids: seen
                .iter()
                .map(|(pid, _)| *pid)
                .chain(self.also.lock().expect("also").iter().copied())
                .collect(),
            folders: std::iter::once(self.root.clone())
                .chain(
                    self.confines
                        .started()
                        .into_iter()
                        .filter_map(|one| one.tmp),
                )
                .chain(self.outside.lock().expect("outside").iter().cloned())
                .collect(),
            ..Left::default()
        };
        let own = process_group_of(std::process::id());
        left.unwatched
            .extend(self.unwatched.lock().expect("unwatched").iter().cloned());
        // A chat the teardown cannot watch by its group is said, never skipped.
        for (pid, group) in &seen {
            match group {
                Some(group) if *group <= 1 => left
                    .unwatched
                    .push(format!("chat process {pid} is in process group {group}")),
                Some(group) if Some(*group) == own => left.unwatched.push(format!(
                    "chat process {pid} is in the test's own process group {group}"
                )),
                _ => {}
            }
        }
        // A tool command whose group never reached the file is said, never skipped (#7197).
        if let Some(lost) = tool_groups_lost(&self.transcript(), tool_groups(&self.root).len()) {
            left.unwatched.push(lost);
        }
        // Read while they run: a group the quit left can be #7103's only if it is one of these.
        let background = process_groups(&self.background.lock().expect("background"));
        let mut groups: Vec<u32> = Vec::new();
        for group in seen
            .iter()
            .filter_map(|(_, group)| *group)
            .chain(process_groups(&left.pids))
            .chain(tool_groups(&self.root))
        {
            if group > 1 && Some(group) != own && !groups.contains(&group) {
                groups.push(group);
            }
        }
        if let Some(window) = self.window.take() {
            window.planes().let_go_of_all();
            if let Err(why) = window.quits() {
                left.quits.push(why);
            }
        }
        let still: Vec<u32> = groups.into_iter().filter(|g| group_alive(*g)).collect();
        if !still.is_empty() {
            // A chat the quit is ending is given the time the parent gives any process.
            let deadline = std::time::Instant::now() + Duration::from_secs(10);
            while seen.iter().any(|(pid, group)| {
                group.is_some_and(|g| still.contains(&g)) && purlis_core::process::alive(*pid)
            }) && std::time::Instant::now() < deadline
            {
                std::thread::sleep(Duration::from_millis(50));
            }
            // Only what still runs once the wait is over: a chat that ended in it left nothing.
            let still: Vec<u32> = still.into_iter().filter(|g| group_alive(*g)).collect();
            let (background, leaks) = of_the_quit(
                &still,
                &seen,
                &background,
                purlis_core::process::alive,
                cfg!(target_os = "macos"),
            );
            left.groups_left.extend(leaks.iter().copied());
            left.background.extend(background.iter().copied());
            if !background.is_empty() {
                let _ = writeln!(
                    std::io::stderr(),
                    "expected fail #7103: the host's quit ended every chat but left process \
                     groups {background:?} running a chat's background process on macOS; the \
                     teardown ends them"
                );
                let _ = super::report::write(&[purlis_scenarios::model::LegRecord {
                    kind: purlis_scenarios::model::EntryKind::Fixture,
                    row: "T1.1-06".to_owned(),
                    test: std::thread::current().name().map(str::to_owned),
                    dialect: None,
                    protection: None,
                    os: super::report::this_os(),
                    leg: purlis_scenarios::model::ReportLeg::Fixture,
                    timings: Some(purlis_scenarios::model::TimingsMode::Scaled),
                    verdict: purlis_scenarios::model::ReportVerdict::ExpectedFail,
                    reason: Some(
                        "#7103: a chat's background process outlives the host's quit on macOS"
                            .to_owned(),
                    ),
                    refusal_expected: false,
                }]);
            }
            // Ended either way, so nothing of the scenario outlives the run.
            for group in &still {
                let _ =
                    purlis_core::forklock::status(std::process::Command::new("/bin/kill").args([
                        "-KILL",
                        "--",
                        &format!("-{group}"),
                    ]));
            }
        }
        self.homes.record_left(left);
        self.hosting.take();
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_timeout(Duration::from_secs(5));
        }
        let _ = std::fs::remove_dir_all(&self.root);
        for folder in self.outside.lock().expect("outside").iter() {
            let _ = std::fs::remove_dir_all(folder);
        }
        // Its run time, set up to teardown, with what it ran as and whether it passed.
        let _ = super::report::time_of(&serde_json::json!({
            "test": std::thread::current().name().unwrap_or_default(),
            "os": super::report::os_word(),
            "dialect": self.dialect.word(),
            "protection": self.protection.word(),
            "template": self.template.word(),
            "seconds": self.began.elapsed().as_secs_f64(),
            "passed": !std::thread::panicking() && !matches!(verdict, Some(Err(_))),
        }));
        if let Some(Err(report)) = verdict
            && !std::thread::panicking()
        {
            panic!("{report}");
        }
    }
}

/// **The window**: the Tauri mock app managing its `Planes`, whose chats run in the host over
/// links a test can cut as a crash cuts them.
pub(crate) struct Window {
    pub app: tauri::App<MockRuntime>,
    pub connection: Arc<Connection>,
    cuts: Arc<Mutex<Vec<tokio::task::AbortHandle>>>,
    /// Every block the block teller was told (T1.1-07: the Notices' raise).
    told: Arc<Mutex<Vec<crate::hooks::ChatBlocked>>>,
    /// Every move the window was told (T1.1-07: the needs-you mark).
    moves: Arc<Mutex<Vec<crate::hooks::Moved>>>,
}

impl Window {
    /// Every block the block teller told this window, in order.
    pub(crate) fn told_blocks(&self) -> Vec<crate::hooks::ChatBlocked> {
        self.told.lock().expect("told").clone()
    }

    /// Every chat move this window was told, in order.
    pub(crate) fn moved(&self) -> Vec<crate::hooks::Moved> {
        self.moves.lock().expect("moves").clone()
    }

    pub(crate) fn planes(&self) -> tauri::State<'_, Planes> {
        tauri::Manager::state::<Planes>(&self.app)
    }

    /// Cuts every link of the window's, as its crash would.
    fn cut(&self) {
        for cut in self.cuts.lock().expect("the cuts").iter() {
            cut.abort();
        }
    }

    /// **Tells the host to quit**, which ends every chat it runs and stops it, and answers
    /// whether the host answered within the window's own wait for it.
    fn quits(&self) -> Result<(), String> {
        let link = self
            .connection
            .wait(Duration::ZERO)
            .map_err(|why| format!("the window has no link to the host: {why}"))?;
        link.quit_within(Duration::from_secs(7))
    }
}

/// **The window reopens the project, as the app does at every open**: the record's chats are put
/// back (none, the first time), and from then on the project's reopen record is written, which
/// a window holds back until it has reopened (`Held::begin_reopen`).
fn reopened(window: &Window, plane: &PlaneId, root: &Path) {
    window
        .planes()
        .held(plane)
        .expect("the project is held")
        .reopen(
            SIZE,
            purlis_core::reopen::read_or_refusal(root),
            purlis_core::reopen::Choice::ReopenAll,
        );
}

/// **The session host, as the window's binary composes it**, on `runtime`: the shipped kit, the
/// scenario's own `Confine`, its kill switch in the host's folder and the event log in the data
/// home.
fn a_host(
    runtime: &tokio::runtime::Runtime,
    log: &Path,
    homes: &Homes,
    kit: &Kit,
    confines: &Arc<Confines>,
) -> purlis_host::Hosting {
    let (folder, data) = (homes.host.clone(), log.to_path_buf());
    let composes = crate::sessionhost::Composes(kit.for_the_host());
    let confine = ConfinesInTheScenario(Arc::clone(confines));
    runtime
        .block_on(async move { crate::behaviour::a_host_with(&folder, &data, composes, confine) })
}

/// **The window on `hosting`**: the app's own `Planes`, its chats in that host
/// (`hostedsessions::chats_in_the_host`), timed by `timings`, armed with the kit's window half,
/// its config home the scenario's, and managed by a Tauri mock app.
fn a_window(
    runtime: &tokio::runtime::Runtime,
    hosting: &purlis_host::Hosting,
    homes: &Homes,
    kit: &Kit,
    timings: Timings,
) -> Window {
    let cuts: Arc<Mutex<Vec<tokio::task::AbortHandle>>> = Arc::default();
    let link = runtime
        .block_on(crate::behaviour::a_cuttable_link(
            hosting.clone(),
            Arc::clone(&cuts),
        ))
        .expect("the window speaks to the host");
    let dial: crate::hostedsessions::Dial = {
        let (hosting, cuts, handle) =
            (hosting.clone(), Arc::clone(&cuts), runtime.handle().clone());
        Arc::new(move |within, _naming| {
            crate::hostedsessions::open_within(
                &handle,
                within,
                crate::behaviour::a_cuttable_link(hosting.clone(), Arc::clone(&cuts)),
            )
        })
    };
    let connection = Arc::new(Connection::default());
    connection.ready(Arc::new(link), dial);
    let moves: Arc<Mutex<Vec<crate::hooks::Moved>>> = Arc::default();
    let told: Arc<Mutex<Vec<crate::hooks::ChatBlocked>>> = Arc::default();
    let tell: crate::planes::Teller = {
        let moves = Arc::clone(&moves);
        Arc::new(move |moved| moves.lock().expect("moves").push(moved))
    };
    let blocks: crate::hooks::BlockTeller = {
        let told = Arc::clone(&told);
        Arc::new(move |block| told.lock().expect("told").push(block))
    };
    let planes = crate::hostedsessions::chats_in_the_host(
        Planes::telling(tell, kit.for_the_window(), Some(homes.config.clone()))
            .timed_by(timings)
            .telling_blocks(blocks),
        &connection,
    );
    let app = tauri::test::mock_builder()
        .build(tauri_context!(test = true))
        .expect("the app builds on the mock runtime");
    tauri::Manager::manage(&app, planes);
    Window {
        app,
        connection,
        cuts,
        told,
        moves,
    }
}

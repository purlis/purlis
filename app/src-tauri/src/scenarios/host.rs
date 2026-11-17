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
//! - **Under `strict`** ([`Template`]), unless a cell names another template (Q308).
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

/// The variable a scenario's child is started with: its home.
const HOME_VAR: &str = "PURLIS_SCENARIO_HOME";

/// **Whether this run is CI's**, where a scenario that cannot run fails rather than returns.
pub(crate) fn on_ci() -> bool {
    std::env::var_os("GITHUB_ACTIONS").is_some_and(|v| !v.is_empty())
        || std::env::var_os("CI").is_some_and(|v| !v.is_empty())
}

/// **Why a scenario did not run**: a failure on CI, a printed reason elsewhere (`None`).
pub(crate) fn cannot_run<T>(why: &str) -> Option<T> {
    assert!(!on_ci(), "the scenario cannot run on CI: {why}");
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
}

/// **A `[policy]` template** (Q308): what every chat of a scenario runs under. `strict` is
/// today's purlis, and every cell runs under it unless it names another.
///
/// Until Q308 (PR #6992) lands on main and is merged here, `strict` is today's behaviour with no
/// `[policy]` key written, and the others are refused: the parameter is threaded through so that
/// merge has only [`Template::manifest`] to change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Template {
    Open,
    Watch,
    Team,
    #[default]
    Strict,
}

impl Template {
    pub(crate) fn word(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Watch => "watch",
            Self::Team => "team",
            Self::Strict => "strict",
        }
    }

    /// The template a cell names, by its word.
    pub(crate) fn of(word: &str) -> Option<Self> {
        [Self::Open, Self::Watch, Self::Team, Self::Strict]
            .into_iter()
            .find(|one| one.word() == word)
    }

    /// The project manifest's lines for it: none for `strict`, today's purlis.
    pub(crate) fn manifest(self) -> Result<&'static str, String> {
        match self {
            Self::Strict => Ok(""),
            other => Err(format!(
                "the `{}` template is Q308's `[policy]`, which this branch does not have yet",
                other.word()
            )),
        }
    }
}

/// **A scenario's own folders**: its data and config homes, the fake harness's state, and the
/// folder its turn scripts are copied to.
#[derive(Debug, Clone)]
pub(crate) struct Homes {
    pub home: PathBuf,
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
    /// child ran the scenario. In that child it answers the homes.
    pub(crate) fn of_this_scenario() -> Option<Homes> {
        if let Some(home) = std::env::var_os(HOME_VAR) {
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
            &homes.data,
            &homes.config,
            &homes.state,
            &homes.scripts,
            &homes.host,
            &homes.projects,
        ] {
            std::fs::create_dir_all(folder).expect("a folder of the scenario's");
        }
        let shims = repo().join("crates/fake-harness/shims");
        let path = std::env::join_paths(std::iter::once(shims).chain(std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_default(),
        )))
        .expect("a PATH");
        let fake = crate::behaviour::fake_harness();
        let mut env: Vec<(&str, OsString)> = vec![
            (HOME_VAR, homes.home.clone().into()),
            (purlis_core::datahome::HOME_VAR, homes.data.clone().into()),
            (purlis_core::machine::HOME_VAR, homes.config.clone().into()),
            // Never the turn script, its transcript or its state: what the app runs of a harness
            // beside a chat (its sign-in status, its version) inherits this environment and would
            // play the script outside any sandbox. Each chat is handed its own (`starts`).
            ("FAKE_HARNESS_BIN", fake.into()),
            ("PATH", path),
        ];
        // The re-run clears every variable of purlis's own; these two are the run's.
        for kept in [BINARY_VAR, HOST_APP_VAR] {
            if let Some(value) = std::env::var_os(kept) {
                env.push((kept, value));
            }
        }
        let env: Vec<(&str, &std::ffi::OsStr)> = env
            .iter()
            .map(|(name, value)| (*name, value.as_os_str()))
            .collect();
        let said = purlis_core::testrun::rerun_steered(&[&test], &env);
        if said.contains("skipped: ") {
            for line in said.lines().filter(|l| l.starts_with("skipped: ")) {
                let _ = writeln!(std::io::stderr(), "{line}");
            }
        }
        homes.nothing_left();
        made.close().expect("the scenario's home is removed");
        None
    }

    /// Where a scenario's teardown writes what it started (`left.json`), for its parent.
    fn left(&self) -> PathBuf {
        self.home.join("left.json")
    }

    /// **Nothing the scenario started outlived it** (T1.1-06): every process it recorded has
    /// ended and every folder of its own outside its home is gone. Run by the parent, after the
    /// child that ran the scenario ended.
    fn nothing_left(&self) {
        let Ok(text) = std::fs::read_to_string(self.left()) else {
            return;
        };
        let left: Left = serde_json::from_str(&text).expect("left.json reads");
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

/// What a scenario started, written by its teardown for its parent's check.
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub(crate) struct Left {
    pub pids: Vec<u32>,
    pub folders: Vec<PathBuf>,
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

/// **The maximal chat's grants**: one of every `grant::What` kind, in the project at `root`.
pub(crate) fn one_of_every_kind(root: &Path) -> Vec<purlis_core::sandbox::grant::What> {
    use purlis_core::sandbox::grant::What;
    let granted = root.join("granted");
    std::fs::create_dir_all(&granted).expect("a folder to grant");
    vec![
        What::Host(
            purlis_core::sandbox::hosts::Host::parse("granted.example.test").expect("a host"),
        ),
        What::Write(granted.clone()),
        What::Socket(granted.join("service.sock")),
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
    /// Each chat it started.
    chats: Mutex<Vec<u32>>,
    /// Each chat's pane, as the window's view of it draws it.
    panes: Mutex<Vec<(u32, crate::behaviour::Pane)>>,
    /// How each chat was launched, as the window composed it: what a red run's reader needs.
    launched: Mutex<Vec<String>>,
    /// The profile its chats start on, made once.
    profiled: std::sync::Once,
    /// Processes a test says the scenario started besides its chats.
    also: Mutex<Vec<u32>>,
    /// When it was made, for its run time (T1.1-06: ticket 54 sets its budgets from these).
    began: std::time::Instant,
    /// Ended last: the runtime the host and the link run on.
    runtime: Option<tokio::runtime::Runtime>,
}

impl Scenario {
    /// **The scenario host for `protection`, `persona` and `dialect`**, under `strict`, in this
    /// scenario's own homes; `None` where it cannot run here (never on CI).
    pub(crate) fn new(
        homes: &Homes,
        protection: Protection,
        persona: Option<&str>,
        dialect: Dialect,
    ) -> Option<Scenario> {
        Scenario::under(homes, protection, persona, dialect, Template::Strict)
    }

    /// [`Scenario::new`], under `template`.
    pub(crate) fn under(
        homes: &Homes,
        protection: Protection,
        persona: Option<&str>,
        dialect: Dialect,
        template: Template,
    ) -> Option<Scenario> {
        if let Err(why) = the_wrap_applies() {
            return cannot_run(&why);
        }
        let kit = match Kit::shipped(&homes.data) {
            Ok(kit) => kit,
            Err(why) => return cannot_run(&why),
        };
        let policy = match template.manifest() {
            Ok(policy) => policy,
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
            format!("{}{policy}", protection.manifest()),
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
        Some(Scenario {
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
            panes: Mutex::default(),
            launched: Mutex::default(),
            profiled: std::sync::Once::new(),
            also: Mutex::default(),
            began,
            runtime: Some(runtime),
        })
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
    pub(crate) fn quits_and_relaunches(&mut self) {
        let window = self.window.take().expect("a window");
        // As the app's quit does: the record keeps every chat that was live, for the relaunch.
        window.planes().let_go_of_all_to_update();
        window.quits();
        drop(window);
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
const SIZE: purlis_core::engine::Size = purlis_core::engine::Size {
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
        let transcript = own.join(TRANSCRIPT);
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
            ]
            .map(|(k, v)| (k.to_owned(), v)),
        );
        let chat = purlis_core::reopen::Chat {
            program: ready.program.clone(),
            cwd: ready.cwd.clone(),
            name: name.clone(),
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
            .start_ready(&chat, &ready, SIZE)
            .expect("the chat starts");
        self.chats.lock().expect("chats").push(id);
        if let Ok(pane) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            crate::behaviour::Pane::of(held.chats().sessions(), id)
        })) {
            self.panes.lock().expect("panes").push((id, pane));
        }
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
        id
    }

    /// **What a red run's reader needs**: how each chat was launched, the chats open now, the
    /// kinds of event the log holds, and the transcript.
    pub(crate) fn said(&self) -> String {
        let kinds: Vec<String> = self
            .events()
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
        format!(
            "launched: {:#?}\nscreens: {:#?}\nopen now: {:?}\nconfinements: {:?}\n\
             events: {kinds:#?}\ntranscript: {:#?}",
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

    /// Every event in the device's event log, which the host writes in the data home.
    pub(crate) fn events(&self) -> Vec<purlis_core::eventlog::Event> {
        purlis_core::eventlog::read(&self.log).unwrap_or_default()
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

    /// The scenario's transcript so far, every chat's, one value a line. A step's name is the
    /// scenario's own, so a step says which chat played it.
    pub(crate) fn transcript(&self) -> Vec<serde_json::Value> {
        std::fs::read_to_string(own_folder(&self.root).join(TRANSCRIPT))
            .unwrap_or_default()
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect()
    }

    /// The tool result chat `id` was handed for `step`, once it was.
    pub(crate) fn result_of(&self, _id: u32, step: &str) -> Option<serde_json::Value> {
        self.transcript()
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
    }

    /// The scenario started `pid` too, which its teardown must end.
    pub(crate) fn also_started(&self, pid: u32) {
        self.also.lock().expect("also").push(pid);
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
    /// every chat it runs; the runtime goes; then the project's folder. What it started is
    /// written for the parent, which checks none of it outlived the scenario.
    fn drop(&mut self) {
        let left = Left {
            pids: self
                .pids()
                .into_iter()
                .chain(self.also.lock().expect("also").iter().copied())
                .collect(),
            folders: std::iter::once(self.root.clone())
                .chain(
                    self.confines
                        .started()
                        .into_iter()
                        .filter_map(|one| one.tmp),
                )
                .collect(),
        };
        // Beside what scenarios before it in the same test left: one child may run several.
        let mut kept: Left = std::fs::read_to_string(self.homes.left())
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        kept.pids.extend(left.pids.iter().copied());
        kept.folders.extend(left.folders);
        if let Ok(text) = serde_json::to_string(&kept) {
            let _ = std::fs::write(self.homes.left(), text);
        }
        let groups = process_groups(&left.pids);
        if let Some(window) = self.window.take() {
            window.planes().let_go_of_all();
            window.quits();
        }
        // Every process of each chat's group ends with the scenario, a process its chat left in
        // the background included. Where the host's quit left one, it is said: a person's quit
        // would have left it too.
        for group in groups {
            if group_alive(group) {
                let _ = writeln!(
                    std::io::stderr(),
                    "the host's quit left chat process group {group} running; the teardown ends it"
                );
                let _ =
                    purlis_core::forklock::status(std::process::Command::new("/bin/kill").args([
                        "-KILL",
                        "--",
                        &format!("-{group}"),
                    ]));
            }
        }
        self.hosting.take();
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_timeout(Duration::from_secs(5));
        }
        let _ = std::fs::remove_dir_all(&self.root);
        // Its run time, set up to teardown, with what it ran as and whether it passed.
        let _ = super::report::time_of(&serde_json::json!({
            "test": std::thread::current().name().unwrap_or_default(),
            "os": super::report::os_word(),
            "dialect": self.dialect.word(),
            "protection": self.protection.word(),
            "template": self.template.word(),
            "seconds": self.began.elapsed().as_secs_f64(),
            "passed": !std::thread::panicking(),
        }));
    }
}

/// **The window**: the Tauri mock app managing its `Planes`, whose chats run in the host over
/// links a test can cut as a crash cuts them.
pub(crate) struct Window {
    pub app: tauri::App<MockRuntime>,
    pub connection: Arc<Connection>,
    cuts: Arc<Mutex<Vec<tokio::task::AbortHandle>>>,
}

impl Window {
    pub(crate) fn planes(&self) -> tauri::State<'_, Planes> {
        tauri::Manager::state::<Planes>(&self.app)
    }

    /// Cuts every link of the window's, as its crash would.
    fn cut(&self) {
        for cut in self.cuts.lock().expect("the cuts").iter() {
            cut.abort();
        }
    }

    /// Tells the host to quit, which ends every chat it runs and stops it.
    fn quits(&self) {
        if let Ok(link) = self.connection.wait(Duration::ZERO) {
            let _ = link.quit_within(Duration::from_secs(7));
        }
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
    let tell: crate::planes::Teller = Arc::new(|_| {});
    let planes = crate::hostedsessions::chats_in_the_host(
        Planes::telling(tell, kit.for_the_window(), Some(homes.config.clone())).timed_by(timings),
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
    }
}

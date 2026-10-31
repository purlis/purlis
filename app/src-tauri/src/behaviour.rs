// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The behaviour suite: what a chat does, written once and run against every place a chat can
//! run (spec #1974, #1998). It is the gate each later step of moving chats into the session
//! host passes: a behaviour is added here once, and every target runs it.
//!
//! **The targets are the two ways the start-time switch runs a chat** (`crate::hostedsessions`):
//! in the window's own process ([`crate::sessions::Sessions`]), and in a session host the window
//! speaks the session protocol to ([`crate::hostedsessions::HostedSessions`]). The host is a
//! real host process (`in_a_host_process`, #2170): the window's own binary, started and
//! connected to by the window's own code ([`crate::sessionhost::SessionHost`]), spoken to over
//! its socket, with real terminals. It runs on Linux, where each test gives its host a socket
//! and a config home of its own; the host's code over an in-memory link (`in_the_host`) runs
//! everywhere beside it. **And a third, through a link**: the window's host relaying to another
//! host over the link between hosts ([`through_a_link`], #2042). Each behaviour speaks only what
//! the window speaks to any of them, [`SessionHost`], and checks only what the window can
//! observe: what its pane draws, what it is told, what ends. A test ends the host process it
//! started. When the in-app path is deleted (#2005), its target goes with it, and the host's
//! stay; Docker and a cluster join as targets of their own (spec #2041).
//!
//! **The chat's program is the repo's fake harness** (`crates/fake-harness`), or a shell where a
//! behaviour needs the terminal itself (its size, its line discipline) or runs `purlis hook` as
//! a harness's hooks do. These open terminals and sockets, which a sandboxed agent cannot: they
//! first run on CI.
//!
//! **Each target has a project, and the window's own hooks for it** (#2000): the board and the
//! asks the window draws Needs you and its asks from. In the window's own process the window
//! binds the project's hook socket and issues each chat its token; in the session host, the
//! host does both and hands each line it admits on to the window. A behaviour checks only what
//! the window is told either way.
//!
//! **The kill switch is each target's** (#2001): the window's own switch, kept in a config home
//! of the test's, which in the host's target holds the host's switch too, as the app's does
//! when chats run in the host. What the host itself does with it, across its restart and for
//! links that are not the window's, is the host's own tests'
//! (`crates/host/tests/it/the_kill_switch_is_held_by_the_host.rs`).
//!
//! **The event log has one writer on each target** (#2003): the window, holding it in a data
//! home of the test's, or the host, holding it there while the window hands it each write and
//! never opens it.
//!
//! **To add a behaviour:** write `fn the_behaviour(target: &Target)`, and name it in the
//! [`on_every_target!`] list at the bottom.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use purlis_core::engine::{AlacrittyEngine, Engine as _, Size};
use purlis_core::sandbox::hosts::Host;
use purlis_core::sandbox::plan::{self, Asking, Hearing, Plan};
use purlis_core::sandbox::reach::{By, Reach};
use purlis_core::sandbox::{Confinement, egress};
use purlis_core::session::Exit;
use purlis_session_protocol::auth::{Credentials, Scope};
use purlis_session_protocol::{link, session};

use crate::hooks::{Hooks, Moved};
use crate::host::{Opening, SessionHost, Sink};
use crate::hostedsessions::{Connection, HostLink, HostedSessions};
use crate::killswitch::KillSwitch;
use crate::sessions::{ENDED, Sessions};

const SIZE: Size = Size {
    columns: 80,
    rows: 24,
};
const PATIENCE: Duration = Duration::from_secs(20);

/// One place a chat runs, as the window reaches it.
pub(crate) struct Target {
    pub sessions: Box<dyn SessionHost>,
    /// The project its chats run in, and that it saves.
    pub root: PathBuf,
    /// The window's own hooks for the target's project: its board and its asks.
    pub hooks: Hooks,
    /// Every move of a chat the window was told, in order.
    pub moved: Arc<Mutex<Vec<Moved>>>,
    /// The kill switch, as the title bar throws and re-arms it.
    pub switch: Arc<KillSwitch>,
    /// The environment of the process that starts the target's chats, as it was started.
    pub inherited: Vec<(OsString, OsString)>,
    /// How many savers of its project run in the test's own process while it is saved: one
    /// where the window, or the host's code, runs in it; none where the host is a process of
    /// its own, whose saver the window sees only by what it is told and what is committed.
    pub savers_here: usize,
    /// Where the device's event log is: the window's, or that of the host its chats run in.
    pub log: PathBuf,
    /// The window's link to the host, on a host's target: what the host is asked directly.
    pub host: Option<Arc<Connection>>,
    /// What kills the host outright, on a host process's target (#2007).
    pub kill_the_host: Option<Arc<dyn Fn() + Send + Sync>>,
    /// The runner's container, on a Docker pool's target: the machine its chats run on.
    pub runner: Option<OnTheRunner>,
    /// The window's own host, where its sessions are another host's: what saves the project as
    /// the window holds it, which a runner never does (#3312). `None` where the window's
    /// sessions save it themselves.
    pub saved_by: Option<Box<dyn SessionHost>>,
    /// What keeps the target running for as long as the test does.
    _keeps: Vec<Box<dyn std::any::Any>>,
}

impl Target {
    /// What the window asks to save its project: the window's own host, wherever the project's
    /// chats run (#3312).
    fn saver(&self) -> &dyn SessionHost {
        self.saved_by.as_deref().unwrap_or(self.sessions.as_ref())
    }

    /// The project as `runner` holds it, `<data>/projects/<id>`.
    fn project_on_its_runner(&self, runner: &OnTheRunner) -> String {
        let project =
            purlis_core::scaffold::planefile::id_to_hold_by(&self.root).expect("the project's id");
        runner
            .path(&format!(
                "{}/{}/{project}",
                purlis_core::datahome::IN_A_RUNNER_CONTAINER,
                purlis_core::datahome::PROJECTS
            ))
            .expect("the runner holds the project")
    }

    /// You let every chat of the project reach the service on `port` of the machine its chats
    /// run on, as that machine's own sandbox for the project gives it (#2445): on a pool's
    /// runner, in the project as the runner holds it, `<data>/projects/<id>`.
    fn gives(&self, port: u16) {
        let Some(runner) = &self.runner else {
            purlis_core::sandbox::local::grant_port(&self.root, port).expect("you gave it");
            return;
        };
        let there = self.project_on_its_runner(runner);
        // Granted on the runner, by a program of the runner's run as its host is, so the
        // runner's own sandbox for the project, under its config home, gives it.
        runner.runs_as_its_host(
            &self.inherited,
            &[
                &fake_harness().display().to_string(),
                "--grant-port",
                &port.to_string(),
                "--in",
                &there,
            ],
        );
    }

    /// Whether the process `pid`, as the window was told it, is alive on the machine the
    /// target's chats run on.
    fn alive(&self, pid: u32) -> bool {
        match &self.runner {
            Some(runner) => runner.has(pid),
            None => purlis_core::process::alive(pid),
        }
    }

    /// Every event in the event log of the machine the target's chats run on: on a pool's
    /// target, the runner's own, which its host alone writes (ADR 0078 §7); elsewhere,
    /// [`Target::log`].
    fn events_where_its_chats_run(&self) -> Vec<purlis_core::eventlog::Event> {
        match &self.runner {
            Some(runner) => {
                let (_copy, log) = runner.copied(&runner.log()).expect("the runner's log");
                purlis_core::eventlog::read(&log).expect("the runner's log reads")
            }
            None => purlis_core::eventlog::read(&self.log).expect("the log reads"),
        }
    }

    /// Whether the event log of the machine the target's chats run on is held by its one
    /// writer, so no second one is let in.
    fn its_log_has_its_writer(&self) -> bool {
        match &self.runner {
            Some(runner) => {
                runner.holds_the_lock(&format!("{}/{}", runner.log(), purlis_core::eventlog::LOCK))
            }
            None => purlis_core::eventlog::Log::open(&self.log, DEVICE).is_err(),
        }
    }

    /// Whether chat `id`'s hook spool holds a line, on the machine it runs on: beside the
    /// project's hook socket as that machine holds the project, which a runner does in
    /// `<data>/projects/<id>` (#2046). On a runner the socket is found in its container, so a
    /// spool looked for in the wrong place fails the test instead of holding nothing.
    fn spool_holds_lines_of(&self, id: u32) -> bool {
        match &self.runner {
            Some(runner) => {
                let project = purlis_core::scaffold::planefile::id_to_hold_by(&self.root)
                    .expect("the project's id");
                let socket = runner
                    .path(&format!(
                        "{}/{}/{project}/.[a-z]*/app/hooks.sock",
                        purlis_core::datahome::IN_A_RUNNER_CONTAINER,
                        purlis_core::datahome::PROJECTS
                    ))
                    .expect("the runner's hook socket for the project");
                let spool = purlis_core::hookwire::spool::dir_for(Path::new(&socket));
                let spool = spool.to_string_lossy();
                match runner.copied(&spool) {
                    Some((_copy, spool)) => purlis_core::hookwire::spool::holds_lines(&spool, id),
                    // Gone, which holds nothing; a copy that failed of a spool still there
                    // says nothing, so it fails the test.
                    None => {
                        assert!(
                            runner.path(&spool).is_none(),
                            "the runner's spool could not be copied"
                        );
                        false
                    }
                }
            }
            None => purlis_core::hookwire::spool::holds_lines(
                &purlis_core::hookwire::spool::dir_for(
                    &crate::hooks::socket_for(Some(&self.root)).socket,
                ),
                id,
            ),
        }
    }

    /// What the host its chats run on said of a hook line it did not record, where its log can
    /// be read: a runner's, in its container. A line its hook was not told was taken is
    /// recorded later from the spool, so the event log alone does not tell the two apart.
    fn hook_lines_its_host_did_not_record(&self) -> Vec<String> {
        self.runner.as_ref().map_or_else(Vec::new, |runner| {
            runner
                .said()
                .lines()
                .filter(|line| line.contains("a line on the hook channel for chat"))
                .filter(|line| line.contains("was not recorded"))
                .map(str::to_owned)
                .collect()
        })
    }
}

/// A project, its manifest written, and where it is.
fn a_project() -> (tempfile::TempDir, PathBuf) {
    let project = tempfile::tempdir().expect("a project");
    let root = std::fs::canonicalize(project.path()).expect("a resolved project");
    std::fs::write(root.join(purlis_core::plane::MANIFEST), "").expect("a manifest");
    (project, root)
}

/// The window's hooks for the project at `root`, bound by the window itself or heard from the
/// host, and what they tell the window.
fn hooks_of(root: &Path, relayed: bool) -> (Hooks, Arc<Mutex<Vec<Moved>>>) {
    let plane: crate::planes::PlaneId =
        serde_json::from_str(&serde_json::to_string(root).expect("a path")).expect("a plane");
    let moved: Arc<Mutex<Vec<Moved>>> = Arc::default();
    let tell: crate::planes::Teller = {
        let moved = Arc::clone(&moved);
        Arc::new(move |what| moved.lock().expect("the moves").push(what))
    };
    let at = crate::hooks::socket_for(Some(root));
    let hooks = if relayed {
        Hooks::relayed(plane, &at, tell, Arc::new(|_| {}))
    } else {
        Hooks::listening_on(plane, &at, tell, Arc::new(|_| {})).expect("the window listens")
    };
    (hooks, moved)
}

/// The device's event log in `dir`, as the app opens one.
fn an_event_log(dir: &Path) -> std::io::Result<purlis_core::eventlog::Recorder> {
    use purlis_core::eventlog::{ArgsKey, Log, Recorder};
    Ok(Recorder::new(Log::open(dir, DEVICE)?, ArgsKey::open(dir)?))
}

/// The device every target's event log is written for.
const DEVICE: &str = "01J9ZZDEVICE0000000000000";

/// A chat in the window's own process: the switch set to `0`, the way back until #2005.
pub(crate) fn in_the_window() -> Target {
    let (project, root) = a_project();
    let (hooks, moved) = hooks_of(&root, false);
    let config = tempfile::tempdir().expect("a config home");
    let data = tempfile::tempdir().expect("a data home");
    hooks.record_into(crate::hooks::Events::here(
        an_event_log(data.path()).expect("the event log"),
    ));
    let switch = KillSwitch::kept_in(Some(config.path().to_path_buf()));
    let mut sessions = Sessions::reporting_to(hooks.reporting());
    sessions.stopped_by(Arc::clone(&switch));
    Target {
        sessions: Box::new(sessions),
        hooks,
        moved,
        root,
        switch,
        inherited: std::env::vars_os().collect(),
        savers_here: 1,
        log: data.path().to_path_buf(),
        host: None,
        kill_the_host: None,
        runner: None,
        saved_by: None,
        _keeps: vec![Box::new(project), Box::new(config), Box::new(data)],
    }
}

/// A chat in the session host: the default. The host's own code, serving the session protocol
/// over an in-memory link, its programs each in a real terminal, composed as the window's binary
/// composes them, and its projects saved as the window's binary saves them.
pub(crate) fn in_the_host() -> Target {
    let host = tempfile::tempdir().expect("the host's folder");
    let mut target = in_a_host_keeping_its_switch_in(host.path());
    target._keeps.push(Box::new(host));
    target
}

/// [`in_the_host`], the host keeping its kill switch in `folder`, and the window's own switch
/// in a config home of its own, which starts with nothing thrown.
fn in_a_host_keeping_its_switch_in(folder: &Path) -> Target {
    let folder = folder.to_path_buf();
    let data = tempfile::tempdir().expect("a data home");
    let log = data.path().to_path_buf();
    let mut target = a_window_on(log.clone(), Runs::Here, move || a_host(&folder, &log));
    target._keeps.push(Box::new(data));
    target
}

/// A chat on another session host, reached through this machine's (#2042): the window speaks to
/// host A, which relays to host B over the link between hosts, as the desktop's host reaches a
/// runner's (ADR 0078 §4). Host B is the host's own code with real terminals, composed as the
/// window's binary composes them; both hosts are on this machine, linked in memory over the
/// encrypted link, each end's link key pinned by the other. Host B takes the window's start
/// and its replies because host A relays them from the window's own link (ruled 2026-10-15).
/// The window knows its chats run on a runner, so it starts a harness chat there by its names
/// (#2446).
pub(crate) fn through_a_link() -> Target {
    let folder = tempfile::tempdir().expect("host B's folder");
    // Host B runs the chats, so it writes their event log.
    let data = tempfile::tempdir().expect("host B's data home");
    let host_b = a_host(folder.path(), data.path());
    let mut target = a_window_on(data.path().to_path_buf(), Runs::OnARunner, move || {
        purlis_host::Relay::new(Arc::new(move || {
            let host_b = host_b.clone();
            Box::pin(async move { Ok(linked_to(host_b).await) })
        }))
    });
    target._keeps.push(Box::new(folder));
    target._keeps.push(Box::new(data));
    the_windows_own_host_saves(&mut target);
    target
}

/// **The window's own host saves the project as the window holds it**, wherever its chats run
/// (#3312, spec #2041): never a runner, whose link may not ask a host to save (`Power::Save`),
/// and whose work comes back as branches and proposals. Here the window's chats run on host B
/// through host A, a relay alone; so the window's own host, which in the app is host A too, is
/// one of its own on this machine, the window's own link to it a link of its own.
fn the_windows_own_host_saves(target: &mut Target) {
    let (folder, data) = (
        tempfile::tempdir().expect("the window's host's folder"),
        tempfile::tempdir().expect("its data home"),
    );
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("a runtime");
    let host = runtime.block_on(async { a_host(folder.path(), data.path()) });
    let (connection, window) = crate::hostedsessions::tests::in_memory(host, &runtime);
    target.saved_by = Some(Box::new(HostedSessions::new(connection, &target.root)));
    target._keeps.insert(0, Box::new(window));
    target._keeps.insert(1, Box::new(runtime));
    target._keeps.push(Box::new(folder));
    target._keeps.push(Box::new(data));
}

/// Where the window's chats of a target's project run, as its sessions are told.
#[derive(Clone)]
enum Runs {
    /// In the host the window speaks to.
    Here,
    /// On a runner that host relays to, which holds the project at the window's own path.
    OnARunner,
    /// On the runner of the pool named, kept in the desktop's config home `config`, which the
    /// window's host relays the project's links to, and which holds the project where it keeps
    /// its projects and is sent its code (#2046).
    OnAPoolsRunner { pool: String, config: PathBuf },
}

/// The variable naming the runner image a Docker pool's target runs, by digest: the parity
/// job's (`.github/workflows/ci.yml`, #2045). Without it, the target is not made and its tests
/// run nothing, as every real-Docker test here does; the `rust` job leaves them out by name, so
/// they are reported skipped there, not passed.
const DOCKER_POOL_IMAGE: &str = "DOCKER_POOL_TEST_IMAGE";

/// The variable the parity job sets: there a Docker pool's target is made or the test fails,
/// so an image that went missing never passes as nothing run.
const PARITY: &str = "PURLIS_PARITY";

mod pool_journey;

/// **A chat on a Docker pool's runner** (#2045, spec #2041): a pool added on this machine as
/// the window adds one ([`a_pool_on`]). The window speaks to its own host, which relays to the
/// runner's `purlis serve` over the link between hosts, through `docker exec` and the key pinned
/// at the pairing ([`crate::runnerlinks::ToARunner`]), and its chats are routed as the window
/// routes a pool's ([`crate::routedsessions::RoutedSessions::every_start_on`]). The runner
/// holds the project where it keeps its projects, sent its code by the window, so the project
/// is a repository with one commit. What a chat on it inherits is what the runner's host
/// inherited, read from the container.
///
/// The image is the runner image of this build with the repo's fake harness and `purlis` at
/// the paths this test's build has them, so a behaviour's program is where it names it on
/// either machine (`tests/parity/runner.Dockerfile`). Each test adds a pool of its own and
/// removes it, its container and its home, however the test ends.
pub(crate) fn in_a_docker_pool() -> Option<Target> {
    let Ok(image) = std::env::var(DOCKER_POOL_IMAGE) else {
        assert!(
            std::env::var_os(PARITY).is_none(),
            "{PARITY} is set and {DOCKER_POOL_IMAGE} names no runner image: the parity job built none"
        );
        return None;
    };
    let docker = purlis_core::dockerpool::Docker::at(
        std::env::var_os("DOCKER_POOL_TEST_DOCKER").unwrap_or("docker".into()),
    );
    Some(a_window_on_a_pool_of(&image, docker).0)
}

/// A window on a pool of `image` it adds on a machine of its own through `docker`: the target,
/// that machine, and the pool's name.
fn a_window_on_a_pool_of(
    image: &str,
    docker: purlis_core::dockerpool::Docker,
) -> (Target, crate::runnerlinks::Machine, String) {
    let config = tempfile::tempdir().expect("the desktop's config home");
    let docker = docker.with_settings_in(config.path().join("docker-settings"));
    let machine = crate::runnerlinks::Machine {
        config: config.path().to_path_buf(),
        docker: docker.clone(),
        keyring: Arc::new(purlis_core::secrets::keyring::FileStore::at(
            config.path().join("keyring.json"),
        )),
    };
    let (name, removes) = a_pool_on(&machine, image);
    let made = purlis_core::runners::runner_for(config.path(), &name)
        .expect("its runner is kept")
        .made;
    let inherited = environment_in(&docker, made.container.as_str());
    // The window's own host, as the window's binary composes it, relaying the project's links
    // to the pool's runner as the app's does (`crate::runnerlinks::relays_to`); it keeps the
    // device's event log in `data`.
    let (folder, data) = (
        tempfile::tempdir().expect("the window's host's folder"),
        tempfile::tempdir().expect("a data home"),
    );
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("a runtime");
    let host = runtime.block_on(async {
        a_host(folder.path(), data.path()).relaying_with(crate::runnerlinks::relaying({
            let machine = machine.clone();
            move || Ok(machine.clone())
        }))
    });
    // The window's links to it, as the window's binary dials them: its own, and each relayed
    // to a pool's runner by the host.
    let handle = runtime.handle().clone();
    let link_to = move |naming, pool: Option<String>| {
        let (host, inner) = (host.clone(), handle.clone());
        crate::hostedsessions::open_within(&handle, PATIENCE, async move {
            crate::hostedsessions::HostLink::over_relayed(
                a_local_ui_link_to(host).await,
                inner,
                || {},
                naming,
                pool.as_deref(),
            )
            .await
        })
    };
    let connection = Arc::new(Connection::default());
    // The pool's runners are kept where this target added them, as the window keeps its own
    // (D-2050-27).
    connection.runners_in(config.path().to_path_buf());
    let window = link_to(None, None).expect("the window's own link");
    let dial: crate::hostedsessions::Dial = {
        let link_to = link_to.clone();
        Arc::new(move |_within, naming| link_to(naming, None))
    };
    let relayed: crate::hostedsessions::RelayedDial =
        Arc::new(move |_within, naming, pool: &str| link_to(naming, Some(pool.to_owned())));
    connection.ready_relaying(Arc::new(window), dial, relayed);
    let mut target = a_window_over(
        connection,
        inherited,
        data.path().to_path_buf(),
        Runs::OnAPoolsRunner {
            pool: name.clone(),
            config: config.path().to_path_buf(),
        },
    );
    target.runner = Some(OnTheRunner {
        docker: docker.clone(),
        container: made.container.as_str().to_owned(),
        pool: name.clone(),
    });
    a_repository(&target.root);
    // Ended in this order once the window's sessions are: the runtime and its links, the
    // window's host, the pool and its runner, and last the config home it is kept in.
    target._keeps.push(Box::new(runtime));
    target._keeps.push(Box::new(folder));
    target._keeps.push(Box::new(data));
    target._keeps.push(Box::new(removes));
    target._keeps.push(Box::new(config));
    (target, machine, name)
}

/// A `local-ui` link to `host` over an in-memory stream, served as the window's host serves
/// each link it admits.
async fn a_local_ui_link_to(host: purlis_host::Hosting) -> link::Link {
    let held = Credentials::mint().expect("credentials");
    let (a, b) = tokio::io::duplex(1 << 20);
    let (client, served) = tokio::join!(
        link::connect(
            a,
            session::speaks(),
            Scope::LocalUi,
            held.of(Scope::LocalUi)
        ),
        link::serve_any(b, session::speaks(), &held)
    );
    tokio::spawn(session::serve(
        served.expect("the host admits the window"),
        purlis_host::Routed::new(host),
        None,
    ));
    client.expect("the window links to its host")
}

/// A pool of its own added on `machine` as the window adds one
/// ([`crate::dockerpools::add_on`]), its runner a container of `image` paired with this
/// desktop: its name, and what removes it, its container and its home however the test ends.
fn a_pool_on(machine: &crate::runnerlinks::Machine, image: &str) -> (String, RemovesThePool) {
    use purlis_core::dockerpool::Image;
    use purlis_core::runners::{Pool, Set};
    let name = format!(
        "parity-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("after 1970")
            .as_nanos()
    );
    let pool = Pool::new(
        &name,
        Image::pinned(image).expect("an image named by digest"),
        Set::default(),
    )
    .expect("a pool");
    crate::dockerpools::add_on(machine, &pool).expect("the pool is added and its runner paired");
    let removes = RemovesThePool {
        config: machine.config.clone(),
        docker: machine.docker.clone(),
        name: name.clone(),
    };
    (name, removes)
}

/// What the runner's host in `container` was started with: the environment of the
/// container's first process, which starts it.
fn environment_in(
    docker: &purlis_core::dockerpool::Docker,
    container: &str,
) -> Vec<(OsString, OsString)> {
    use std::os::unix::ffi::OsStringExt;
    let out = purlis_core::forklock::output(std::process::Command::new(docker.program()).args([
        "exec",
        container,
        "cat",
        "/proc/1/environ",
    ]))
    .expect("docker runs");
    assert!(out.status.success(), "the runner's environment: {out:?}");
    out.stdout
        .split(|byte| *byte == 0)
        .filter(|pair| !pair.is_empty())
        .filter_map(|pair| {
            let at = pair.iter().position(|byte| *byte == b'=')?;
            Some((
                OsString::from_vec(pair[..at].to_vec()),
                OsString::from_vec(pair[at + 1..].to_vec()),
            ))
        })
        .collect()
}

/// Removes a test's pool, its runner's container and its home, however the test ends.
struct RemovesThePool {
    config: PathBuf,
    docker: purlis_core::dockerpool::Docker,
    name: String,
}

impl Drop for RemovesThePool {
    fn drop(&mut self) {
        // A behaviour that failed on the runner is read with what its host said.
        if std::thread::panicking()
            && let Ok(kept) = purlis_core::runners::runner_for(&self.config, &self.name)
        {
            let said = purlis_core::forklock::output(
                std::process::Command::new(self.docker.program()).args([
                    "logs",
                    "--tail",
                    "200",
                    kept.made.container.as_str(),
                ]),
            );
            if let Ok(said) = said {
                use std::io::Write as _;
                let _ = writeln!(
                    std::io::stderr(),
                    "the runner's host said:\n{}{}",
                    String::from_utf8_lossy(&said.stdout),
                    String::from_utf8_lossy(&said.stderr)
                );
            }
        }
        let removed = purlis_core::pools::remove(&self.config, &self.docker, &self.name, None);
        if !removed
            .as_ref()
            .is_ok_and(|removed| removed.as_ref().is_none_or(|removed| removed.forgotten))
        {
            let said = format!("the pool {} was not removed whole: {removed:?}", self.name);
            // Said, not panicked, while the test already fails: a second panic aborts the run.
            if std::thread::panicking() {
                tracing::warn!("{said}");
            } else {
                panic!("{said}");
            }
        }
    }
}

/// A Docker pool's runner, as a test reaches into its container.
pub(crate) struct OnTheRunner {
    docker: purlis_core::dockerpool::Docker,
    container: String,
    /// The pool it is the runner of.
    pub pool: String,
}

/// A runner's host stopped, started again when this is dropped: a test that fails while it is
/// stopped still has a host to end its chats.
struct HostStopped<'a> {
    runner: &'a OnTheRunner,
    pid: String,
}

impl Drop for HostStopped<'_> {
    fn drop(&mut self) {
        // Not `prints`, whose assertion would be a panic inside the panic of a failed test.
        let _ = purlis_core::forklock::output(
            std::process::Command::new(self.runner.docker.program()).args([
                "exec",
                self.runner.container.as_str(),
                "kill",
                "-CONT",
                self.pid.as_str(),
            ]),
        );
    }
}

impl OnTheRunner {
    /// What `script` prints in the runner's container, run as its own user.
    pub(crate) fn prints(&self, script: &str) -> String {
        let out = purlis_core::forklock::output(
            std::process::Command::new(self.docker.program()).args([
                "exec",
                self.container.as_str(),
                "sh",
                "-c",
                script,
            ]),
        )
        .expect("docker runs");
        assert!(out.status.success(), "{script}: {out:?}");
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    /// Runs `script` in the runner's container, as its own user or as root, `given` on its
    /// standard input.
    fn runs(&self, as_root: bool, script: &str, given: &[u8]) {
        use std::io::Write as _;
        let mut command = std::process::Command::new(self.docker.program());
        command.args(["exec", "--interactive"]);
        if as_root {
            command.args(["--user", "0"]);
        }
        command
            .args([self.container.as_str(), "sh", "-c", script])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        let mut child = purlis_core::forklock::spawn(&mut command).expect("docker runs");
        child
            .stdin
            .take()
            .expect("piped")
            .write_all(given)
            .expect("given");
        let out = child.wait_with_output().expect("it ends");
        assert!(out.status.success(), "{script}: {out:?}");
    }

    /// Runs `argv` in the runner's container as its host was started: its user, and the
    /// environment `inherited` it was started with.
    fn runs_as_its_host(&self, inherited: &[(OsString, OsString)], argv: &[&str]) {
        let mut command = std::process::Command::new(self.docker.program());
        command.arg("exec");
        for (name, value) in inherited {
            let mut pair = name.clone();
            pair.push("=");
            pair.push(value);
            command.arg("--env").arg(pair);
        }
        command.arg(self.container.as_str()).args(argv);
        let out = purlis_core::forklock::output(&mut command).expect("docker runs");
        assert!(out.status.success(), "{argv:?}: {out:?}");
    }

    /// The one path in the runner's container that `pattern` names, the shell's globs and
    /// `$HOME` read there: `None` where it names none.
    fn path(&self, pattern: &str) -> Option<String> {
        let out = purlis_core::forklock::output(
            std::process::Command::new(self.docker.program()).args([
                "exec",
                self.container.as_str(),
                "sh",
                "-c",
                &format!("ls -d {pattern}"),
            ]),
        )
        .expect("docker runs");
        if !out.status.success() {
            return None;
        }
        let said = String::from_utf8_lossy(&out.stdout);
        let mut paths = said.lines().filter(|line| !line.is_empty());
        let path = paths.next()?.to_owned();
        assert!(paths.next().is_none(), "{pattern} names one path: {said}");
        Some(path)
    }

    /// What the runner's host has said in its log, the container's.
    fn said(&self) -> String {
        let out = purlis_core::forklock::output(
            std::process::Command::new(self.docker.program())
                .args(["logs", self.container.as_str()]),
        )
        .expect("docker runs");
        assert!(out.status.success(), "the runner's log: {out:?}");
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        )
    }

    /// A copy, on this machine, of what is at `path` in the runner's container: `None` where
    /// nothing is there.
    fn copied(&self, path: &str) -> Option<(tempfile::TempDir, PathBuf)> {
        let into = tempfile::tempdir().expect("a folder to copy it into");
        let at = into.path().join("copied");
        let out = purlis_core::forklock::output(
            std::process::Command::new(self.docker.program())
                .arg("cp")
                .arg(format!("{}:{path}", self.container))
                .arg(&at),
        )
        .expect("docker runs");
        out.status.success().then_some((into, at))
    }

    /// Where the runner's host keeps its event log: its device's folder in its data home.
    fn log(&self) -> String {
        self.path(&format!(
            "{}/{}/*",
            purlis_core::datahome::IN_A_RUNNER_CONTAINER,
            purlis_core::eventlog::EVENTS
        ))
        .expect("the runner's host keeps an event log")
    }

    /// Whether the lock `lock` in the runner's container is held by a process of its own.
    fn holds_the_lock(&self, lock: &str) -> bool {
        // `flock` says a lock it could not take with an exit code of its own, told apart from
        // one with no `flock` or no file to lock.
        const HELD: i32 = 75;
        let out = purlis_core::forklock::output(
            std::process::Command::new(self.docker.program()).args([
                "exec",
                self.container.as_str(),
                "flock",
                "--nonblock",
                "--conflict-exit-code",
                &HELD.to_string(),
                lock,
                "true",
            ]),
        )
        .expect("docker runs");
        assert!(
            out.status.success() || out.status.code() == Some(HELD),
            "the lock could not be tried: {out:?}"
        );
        out.status.code() == Some(HELD)
    }

    /// The runner's host stopped, until what this returns is dropped: the one child of the
    /// container's init, which `docker exec`'s processes are not.
    fn its_host_stopped(&self) -> HostStopped<'_> {
        let pid = self.prints("cat /proc/1/task/1/children").trim().to_owned();
        assert!(
            !pid.is_empty() && pid.chars().all(|c| c.is_ascii_digit()),
            "the container's init has one child: {pid:?}"
        );
        self.prints(&format!("kill -STOP {pid}"));
        HostStopped { runner: self, pid }
    }

    /// Whether the process `pid` of the runner's container is there.
    fn has(&self, pid: u32) -> bool {
        let out = purlis_core::forklock::output(
            std::process::Command::new(self.docker.program()).args([
                "exec",
                self.container.as_str(),
                "test",
                "-e",
                &format!("/proc/{pid}"),
            ]),
        )
        .expect("docker runs");
        out.status.success()
    }

    /// The runner has a profile of its own named `name`, of `harness` (`claude` or `codex`,
    /// whose stand-in the parity image runs, `tests/parity/runner.Dockerfile`), signed in with
    /// no key, its harness the program `program` is on this machine. A Codex profile is one
    /// for sandboxed chats only, its folder where purlis makes such a profile's (#2037): under
    /// the runner's data home, in use once its first browser has started.
    fn signs_in_as(&self, harness: &str, name: &str, program: &Path) {
        use purlis_core::profilestore::{Folders, MachineProfile};
        let folder = if harness == "codex" {
            let browsers = self.prints(&format!(
                "ls -d \"$HOME\"/.local/share/*/{} | head -n 1",
                purlis_core::datahome::BROWSERS
            ));
            let data = Path::new(browsers.trim())
                .parent()
                .expect("the runner's data home, where its browsers are")
                .to_path_buf();
            data.join(purlis_core::profilestore::DIR)
                .join(name)
                .join(purlis_core::profilestore::CONFIG)
        } else {
            PathBuf::from(format!("/home/purlis/.{harness}-{name}"))
        };
        let config = tempfile::tempdir().expect("a config home to write it in");
        purlis_core::profilestore::update(config.path(), |store| {
            store.profiles.push(MachineProfile {
                name: name.to_owned(),
                harness: harness.to_owned(),
                title: name.to_owned(),
                folders: Folders {
                    config: folder.clone(),
                    data: None,
                },
                adopted: None,
                imported: None,
                sign_in: None,
                pinned: None,
                // A sandboxed Codex chat runs only on a profile kept for sandboxed chats alone (#2037).
                sandbox_only: harness == "codex",
                kept_for: None,
                rest: Default::default(),
            });
        })
        .expect("the profile is kept");
        let store =
            std::fs::read(purlis_core::profilestore::path(config.path())).expect("the store reads");
        let at = format!(
            "\"$HOME\"/.config/{}",
            purlis_core::machine::dir(Path::new("")).display()
        );
        self.runs(
            false,
            &format!(
                "mkdir -p {at} && mkdir -p -m 0700 '{}' && umask 077 && cat > {at}/{}",
                folder.display(),
                purlis_core::profilestore::FILE
            ),
            &store,
        );
        if harness == "codex" {
            self.runs(
                false,
                &format!(
                    "printf '{{\"parity\":true}}' > '{}'/auth.json",
                    folder.display()
                ),
                b"",
            );
        }
        // The parity image's harness runs what is here (`tests/parity/runner.Dockerfile`).
        self.runs(
            false,
            &format!(
                "mkdir -p \"$HOME\"/.parity && cat > \"$HOME\"/.parity/{harness} && chmod 0755 \"$HOME\"/.parity/{harness}"
            ),
            &std::fs::read(program).expect("the program reads"),
        );
    }
}

/// Makes the project at `root` a repository with one commit, as a project the window holds on
/// a pool's runner is: its code is sent there as git sends it.
fn a_repository(root: &Path) {
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["add", "-A"]);
    git(
        root,
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@example.invalid",
            "commit",
            "-q",
            "-m",
            "a project",
        ],
    );
}

/// A session host as the window's binary composes it, keeping its kill switch in `folder`: its
/// starts composed from the window's launch, each in a real terminal, its chats' hooks heard on
/// their project's channel, each confined here, and the device's event log kept in `data`,
/// opened as it first holds a project (#2003).
pub(crate) fn a_host(folder: &Path, data: &Path) -> purlis_host::Hosting {
    let kept = {
        let data = data.to_path_buf();
        purlis_core::hookchannel::EventLog::opening(move || an_event_log(&data))
    };
    purlis_host::Hosting::kept(
        folder,
        crate::sessionhost::Composes::as_the_tests_ship(),
        purlis_host::Terminals,
        crate::sessionhost::hook_channels(&kept),
        crate::hostedconfinement::ConfinesHere,
    )
    .recording_with(kept)
    .saving_with(crate::hostsaving::InTheHost)
}

/// A link from host A to `host_b`, over the link between hosts on an in-memory stream: what
/// `host_b` serves as `remote-link`.
pub(crate) async fn linked_to(host_b: purlis_host::Hosting) -> link::Link {
    use purlis_session_protocol::hostlink::{self, Handshake, Identity, LinkKey, Liveness, Pinned};
    let identity = |device: &str| Identity {
        device: device.to_owned(),
        key: LinkKey::generate().expect("a link key"),
    };
    let pinned = |identity: &Identity| Pinned {
        device: identity.device.clone(),
        key: identity.key.public(),
    };
    let (a, b) = (
        identity("01JA0DESKT0P00000000000000"),
        identity("01JB0RUNNER0000000000000000"),
    );
    let (desktop, b_pinned) = (pinned(&a), pinned(&b));
    let (a_end, b_end) = tokio::io::duplex(1 << 20);
    tokio::spawn(async move {
        let answered = hostlink::answer(
            b_end,
            Handshake::THIS_BUILD,
            &b,
            &desktop,
            Liveness::default(),
        )
        .await
        .expect("host B admits host A");
        session::serve(answered.link, host_b, None).await
    });
    hostlink::connect(
        a_end,
        Handshake::THIS_BUILD,
        &a,
        &b_pinned,
        Liveness::default(),
    )
    .await
    .expect("host A links to host B")
}

/// The window, speaking the session protocol over an in-memory `local-ui` link to the host
/// `host` makes, on a runtime of the target's own, its own kill switch in a config home of its
/// own. The window opens no event log, and hands the host every write (#2003); the host keeps
/// it in `log`.
fn a_window_on<H: session::Host + Clone>(
    log: PathBuf,
    runs: Runs,
    host: impl FnOnce() -> H + Send + 'static,
) -> Target {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("a runtime");
    // Made on the runtime, as a host is made where it serves.
    let host = runtime.block_on(async move { host() });
    let (connection, _window) = crate::hostedsessions::tests::in_memory(host, &runtime);
    let mut target = a_window_over(connection, std::env::vars_os().collect(), log, runs);
    target._keeps.push(Box::new(runtime));
    target
}

/// The window, its chats run in the session host it reaches over `connection`, whose process
/// was started with `inherited` and keeps the device's event log in `log`: its project, its
/// hooks heard from the host, and its own switch in a config home of its own, held by the host.
fn a_window_over(
    connection: Arc<Connection>,
    inherited: Vec<(OsString, OsString)>,
    log: PathBuf,
    runs: Runs,
) -> Target {
    let config = tempfile::tempdir().expect("a config home");
    let (project, root) = a_project();
    let (hooks, moved) = hooks_of(&root, true);
    hooks.record_into(crate::hooks::Events::in_the_host({
        let connection = Arc::clone(&connection);
        move |entry| crate::hostedsessions::record_in_the_host(&connection, &entry)
    }));
    let switch = KillSwitch::kept_in(Some(config.path().to_path_buf()));
    switch.held_by(Arc::clone(&connection) as Arc<dyn crate::killswitch::Authority>);
    let mut sessions: Box<dyn SessionHost> = match runs {
        Runs::Here => Box::new(HostedSessions::new(Arc::clone(&connection), &root)),
        Runs::OnARunner => Box::new(HostedSessions::on_a_runner(Arc::clone(&connection), &root)),
        // Routed as the window routes a project's chats (`crate::routedsessions`), every start on
        // the pool's runner: this machine's host holds the project, keeps its board and deals
        // each chat's number, and the runner's lines are heard as that pool's.
        Runs::OnAPoolsRunner { pool, config } => Box::new(
            crate::routedsessions::RoutedSessions::of_the_project(
                Box::new(HostedSessions::new(Arc::clone(&connection), &root)),
                &connection,
                &config,
                &root,
            )
            .every_start_on(&pool),
        ),
    };
    sessions.hear_through(hooks.relayed_from_the_host().expect("heard from the host"));
    sessions.stopped_by(Arc::clone(&switch));
    Target {
        sessions,
        hooks,
        moved,
        root,
        switch,
        inherited,
        savers_here: 1,
        log,
        host: Some(connection),
        kill_the_host: None,
        runner: None,
        saved_by: None,
        _keeps: vec![Box::new(project), Box::new(config)],
    }
}

/// A chat in a session host of its own process (#2170): the window's binary, started as the
/// window starts it ([`crate::sessionhost::SessionHost`]), the window speaking the session
/// protocol to it over its socket. Its config home, data home and runtime directory are in
/// `home`, so it serves at a place of its own and keeps its kill switch and the device's event
/// log there; its `PATH` leads with a folder
/// the test's own does not, so what a chat inherits is seen to be the host's.
///
/// Run on Linux alone: on macOS the host's socket is in this user's own temporary directory,
/// which a test cannot move aside, and a window running there would already hold it. It is
/// built everywhere, so a change that breaks it is seen on every machine.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn in_a_host_process_at(home: &Path) -> Target {
    let HostHome {
        place,
        inherited,
        log,
    } = a_host_home(home);
    // Held to be ended from here on, so a host that never says it is ready is ended too when
    // the wait below fails the test.
    let host = Ends(crate::sessionhost::SessionHost::default());
    host.0.start_at(place, {
        let inherited = inherited.clone();
        move || {
            let mut command = crate::sessionhost::command(&charter_app());
            command.env_clear().envs(inherited.iter().cloned());
            Ok(command)
        }
    });
    let connection = host.0.connection();
    connection
        .wait(PATIENCE)
        .expect("the window connects to its host");
    let mut target = a_window_over(connection, inherited, log, Runs::Here);
    target.savers_here = 0;
    target.kill_the_host = Some(host.0.killer());
    // Ended before the folders it serves from go.
    target._keeps.insert(0, Box::new(host));
    target
}

/// Where a session host process of a test's serves from, and what it is started with: its
/// config home, data home and runtime directory in `home`, and a `PATH` that leads with a
/// folder the test's own does not.
pub(crate) struct HostHome {
    pub(crate) place: purlis_host::Place,
    pub(crate) inherited: Vec<(OsString, OsString)>,
    /// Where it keeps the device's event log.
    pub(crate) log: PathBuf,
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(crate) fn a_host_home(home: &Path) -> HostHome {
    use std::os::unix::fs::PermissionsExt;
    let (run, config, data) = (home.join("run"), home.join("config"), home.join("data"));
    for folder in [&run, &config, &data] {
        std::fs::create_dir_all(folder).expect("a folder of the host's");
    }
    std::fs::set_permissions(&run, std::fs::Permissions::from_mode(0o700))
        .expect("a runtime directory this user's alone");
    let path = std::env::join_paths(std::iter::once(home.join("bin")).chain(
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()),
    ))
    .expect("a PATH");
    let ours = [
        ("PATH", path),
        ("XDG_RUNTIME_DIR", run.clone().into_os_string()),
        (
            purlis_core::machine::HOME_VAR,
            config.clone().into_os_string(),
        ),
        (
            purlis_core::datahome::HOME_VAR,
            data.clone().into_os_string(),
        ),
    ];
    let inherited: Vec<(OsString, OsString)> = std::env::vars_os()
        .filter(|(name, _)| {
            let name = name.to_string_lossy();
            !ours
                .iter()
                .any(|(ours, _)| purlis_core::envvar::same(ours, &name))
        })
        .chain(ours.clone().map(|(name, value)| (name.into(), value)))
        .collect();
    // Where the host process keeps the device's event log (`Recorder::open`): its device's own
    // folder in its data home, the device named in its config home before it starts.
    let log = data
        .join(purlis_core::eventlog::EVENTS)
        .join(purlis_core::machine::device_id(&config).expect("the host's device"));
    // The place the host process names for itself: its runtime directory, and the name its
    // config home gives its socket (#2114).
    let place = purlis_host::Place::at(
        purlis_core::hostsocket::here_for(Some(run.as_os_str()), Some(&config)),
        purlis_core::machine::charterd(&config),
    );
    HostHome {
        place,
        inherited,
        log,
    }
}

/// [`in_a_host_process_at`] in a folder of its own.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(crate) fn in_a_host_process() -> Target {
    let home = tempfile::tempdir().expect("the host's home");
    let mut target = in_a_host_process_at(home.path());
    target._keeps.push(Box::new(home));
    target
}

/// Ends the host the test started, as the window's quit does, however the test ends: CI's
/// orphan check fails a run that leaves one alive.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
struct Ends(crate::sessionhost::SessionHost);

impl Drop for Ends {
    fn drop(&mut self) {
        self.0.end();
    }
}

/// The window's own binary, which it starts again as its session host, built once beside this
/// test's binary when it is not there yet.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn charter_app() -> PathBuf {
    static BUILT: OnceLock<PathBuf> = OnceLock::new();
    BUILT
        .get_or_init(|| built_beside("charter-app", "purlis-app"))
        .clone()
}

/// The `purlis` binary, whose `purlis hook` a chat's harness runs, built once beside this
/// test's binary when it is not there yet.
fn purlis() -> String {
    static BUILT: OnceLock<PathBuf> = OnceLock::new();
    BUILT
        .get_or_init(|| built_beside("purlis", "purlis-cli"))
        .display()
        .to_string()
}

/// The binary `name` of package `package`, built beside this test's binary when it is not
/// there yet.
pub(crate) fn built_beside(name: &str, package: &str) -> PathBuf {
    let deps = std::env::current_exe().expect("this test's path");
    let beside = deps
        .parent()
        .and_then(Path::parent)
        .expect("the target folder")
        .join(name);
    if !beside.is_file() {
        let status = purlis_core::forklock::status(
            std::process::Command::new(env!("CARGO"))
                .args(["build", "--quiet", "-p", package, "--bin", name]),
        )
        .expect("cargo runs");
        assert!(status.success(), "{name} did not build: {status}");
    }
    beside
}

/// The repo's fake harness, built once beside this test's binary when it is not there yet.
fn fake_harness() -> PathBuf {
    static BUILT: OnceLock<PathBuf> = OnceLock::new();
    BUILT
        .get_or_init(|| built_beside("fake-harness", "fake-harness"))
        .clone()
}

fn a_harness(args: &[&str]) -> Opening {
    let mut opening = a_program(&fake_harness().display().to_string(), args);
    // A harness the plane does not know: the fake one runs as a program, as a chat on a
    // profile's own command does.
    opening.harness = None;
    opening
}

fn a_shell(script: &str) -> Opening {
    a_program("/bin/sh", &["-c", script])
}

fn a_program(program: &str, args: &[&str]) -> Opening {
    Opening {
        program: Some(program.to_owned()),
        args: args.iter().map(|arg| (*arg).to_owned()).collect(),
        size: SIZE,
        ..Opening::default()
    }
}

/// A pane: what its view is sent, as it is sent, and the terminal of its own that draws it.
struct Pane {
    seen: Arc<Mutex<Seen>>,
}

/// What a pane's view has been sent, and its terminal drawing it.
struct Seen {
    sent: String,
    terminal: AlacrittyEngine,
}

impl Seen {
    fn new(size: Size) -> Seen {
        Seen {
            sent: String::new(),
            terminal: AlacrittyEngine::new(size, crate::sessions::SCROLLBACK as usize),
        }
    }

    fn take(&mut self, text: &str) {
        self.sent.push_str(text);
        self.terminal.advance(text.as_bytes());
    }

    /// Whether `wanted`, as text, is drawn: in what the pane was sent, or on its terminal,
    /// its scrollback included.
    fn draws(&mut self, wanted: &str) -> bool {
        drawn(&self.sent).contains(wanted)
            || drawn(&String::from_utf8_lossy(&self.terminal.snapshot())).contains(wanted)
    }
}

impl Pane {
    fn on(target: &Target, id: u32) -> Pane {
        Pane::of(target.sessions.as_ref(), id)
    }

    /// A pane of chat `id`, which `sessions` runs.
    fn of(sessions: &dyn SessionHost, id: u32) -> Pane {
        // A pane sized as the window's first is: the view says the size its screen was
        // drawn for only once it opens, after its first bytes may have been sent.
        let seen = Arc::new(Mutex::new(Seen::new(SIZE)));
        let sink: Sink = {
            let seen = Arc::clone(&seen);
            Box::new(move |text: String| seen.lock().expect("the pane").take(&text))
        };
        sessions.watch(id, sink).expect("a view opens");
        Pane { seen }
    }

    /// A pane that was sent `bytes`, on a screen of [`SIZE`].
    #[cfg(test)]
    fn sent(bytes: &str) -> Pane {
        let mut seen = Seen::new(SIZE);
        seen.take(bytes);
        Pane {
            seen: Arc::new(Mutex::new(seen)),
        }
    }

    /// Waits until the pane draws `text` followed by a number and the end of its line, and
    /// answers the number.
    fn number_after(&self, text: &str) -> u16 {
        let deadline = Instant::now() + PATIENCE;
        let after = |seen: &str| -> Option<u16> {
            let after = drawn(seen).split(text).nth(1)?.to_owned();
            let digits: String = after.chars().take_while(char::is_ascii_digit).collect();
            // A number is read only once its line has ended: one still being written, or
            // written in two pieces, has no line's end after it yet.
            let ended = after[digits.len()..].starts_with(['\r', '\n']);
            if digits.is_empty() || !ended {
                return None;
            }
            digits.parse().ok()
        };
        loop {
            let (sent, found) = {
                let mut seen = self.seen.lock().expect("the pane");
                let on_screen = String::from_utf8_lossy(&seen.terminal.snapshot()).into_owned();
                let found = after(&seen.sent).or_else(|| after(&on_screen));
                (seen.sent.clone(), found)
            };
            if let Some(number) = found {
                return number;
            }
            assert!(
                Instant::now() < deadline,
                "{text:?} and a number were never drawn, only {sent:?}"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Waits until the pane draws `text`, read as the text it draws. A view is sent the screen
    /// as it is first and then the output, so a line the program writes in two pieces can
    /// reach it with the rest of the screen and a cursor move between them (#2375): a
    /// terminal draws that as the line, and so does the pane's.
    fn shows(&self, text: &str) {
        self.shows_within(text, PATIENCE);
    }

    /// [`Self::shows`], for as long as `patience`.
    fn shows_within(&self, text: &str, patience: Duration) {
        let deadline = Instant::now() + patience;
        let wanted = drawn(text);
        loop {
            // The lock is let go before a failed wait panics: the view's sink takes it on the
            // host's thread, and a poisoned one would fail it there too.
            let sent = {
                let mut seen = self.seen.lock().expect("the pane");
                if seen.draws(&wanted) {
                    return;
                }
                seen.sent.clone()
            };
            assert!(
                Instant::now() < deadline,
                "{text:?} was never drawn, only {sent:?}"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

/// `seen` as text, its escape sequences left out: a control sequence (`ESC [` to its final
/// byte), a string (`ESC ]`, `P`, `^` or `_` to BEL or ST), or an escape with its intermediate
/// bytes and final byte, such as saving the cursor (`ESC 7`).
fn drawn(seen: &str) -> String {
    let mut text = String::with_capacity(seen.len());
    let mut chars = seen.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            text.push(c);
            continue;
        }
        match chars.next() {
            Some('[') => {
                for c in chars.by_ref() {
                    if ('\u{40}'..='\u{7e}').contains(&c) {
                        break;
                    }
                }
            }
            Some(']' | 'P' | '^' | '_') => {
                while let Some(c) = chars.next() {
                    if c == '\u{7}' {
                        break;
                    }
                    if c == '\u{1b}' && chars.peek() == Some(&'\\') {
                        chars.next();
                        break;
                    }
                }
            }
            Some(' '..='/') => {
                for c in chars.by_ref() {
                    if !(' '..='/').contains(&c) {
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    text
}

/// A pane opened while a chat writes a line is sent the screen with the line's first piece,
/// then the rest as the program writes it: the end of the screen it is sent moves the cursor
/// back between the two (#2375, as CI drew it).
#[test]
fn a_line_written_in_two_pieces_around_the_screen_a_pane_opens_on_is_shown_whole() {
    let pane = Pane::sent(
        "--name chat 1 without the sandbox\u{1b}[K\u{1b}[1;1H\u{1b}7\u{1b}[2 q\u{1b}[24;33H :END",
    );

    pane.shows("without the sandbox :END");
}

/// The same, with the line high on the screen: the screen a pane opens on draws the rows below
/// the line before its cursor goes back to the line's end.
#[test]
fn a_line_written_in_two_pieces_high_on_the_screen_a_pane_opens_on_is_shown_whole() {
    let mut chat = AlacrittyEngine::new(SIZE, 0);
    chat.advance(b"\r\nwithout the sandbox");
    let screen = String::from_utf8(chat.snapshot()).expect("the screen as text");
    let pane = Pane::sent(&format!("{screen} :END"));

    pane.shows("without the sandbox :END");
}

/// What a pane is told when its program ends carries escape sequences of its own, and is
/// shown as it is drawn.
#[test]
fn the_end_a_pane_is_told_is_shown_as_it_is_drawn() {
    let pane = Pane::sent(&format!("HARNESS-READY\u{1b}[K\r\n{ENDED}"));

    pane.shows(ENDED);
}

/// A number a chat says is read once its line is written, from what the pane was sent or from
/// the screen it was sent when it opened after the line (#3288).
#[test]
fn a_number_a_chat_says_is_read_from_its_pane() {
    assert_eq!(
        Pane::sent("serving on port 4242\r\n").number_after("serving on port "),
        4242
    );
    let mut chat = AlacrittyEngine::new(SIZE, 0);
    chat.advance(b"serving on port 4243\r\n");
    let screen = String::from_utf8(chat.snapshot()).expect("the screen as text");
    assert_eq!(Pane::sent(&screen).number_after("serving on port "), 4243);
}

/// How each chat ended, as the window is told.
fn told_of_ends(target: &Target) -> Arc<Mutex<Vec<(u32, Exit)>>> {
    let ends = Arc::new(Mutex::new(Vec::new()));
    target.sessions.when_one_ends({
        let ends = Arc::clone(&ends);
        Arc::new(move |id, exit| ends.lock().expect("the ends").push((id, exit)))
    });
    ends
}

fn until(what: &str, done: impl Fn() -> bool) {
    let deadline = Instant::now() + PATIENCE;
    while !done() {
        assert!(Instant::now() < deadline, "{what} never happened");
        std::thread::sleep(Duration::from_millis(10));
    }
}

// The behaviours.

fn a_chat_starts_and_its_tab_draws_what_it_wrote(target: &Target) {
    let id = target
        .sessions
        .open(
            None,
            &a_harness(&["--sentinel", "HARNESS-READY", "--interactive"]),
            &|_| {},
        )
        .expect("it starts");

    Pane::on(target, id).shows("HARNESS-READY");
    target.sessions.close(id).expect("it closes");
}

/// **A chat named alone is composed by the host** (#2171): a start with no launch, by profile,
/// persona, first message and the person's opt-out, runs the chat they describe, composed from
/// the project's own files with the code the window's start runs. The window's own process
/// composes no such start, and says so.
fn a_chat_named_alone_is_composed_by_the_host_as_its_profile_says(target: &Target) {
    let bin = tempfile::tempdir().expect("a folder for the harness");
    let harness = stand_in::claude_code(
        bin.path(),
        "claude",
        // The first message is the last argument. It is drawn alone on a line of its own, so the
        // pane holds it whole however long the arguments before it grow.
        "#!/bin/sh\nprintf 'COMPOSED as %s:' \"$PURLIS_PERSONA\"\nfor arg; do last=\"$arg\"; done\n\
         printf '\\n%s :END' \"$last\"\nread line\n",
    );
    a_profile(target, "work", &harness);
    if target.host.is_none() {
        let refused = target
            .sessions
            .open_named(
                None,
                &named("hello", false),
                purlis_core::sandbox::plan::Hearing::default(),
                &|_| {},
            )
            .unwrap_err();
        assert_eq!(refused, crate::host::NOT_COMPOSED_HERE);
        return;
    }

    let id = target
        .sessions
        .open_named(
            None,
            &named("carry on with the plan", false),
            purlis_core::sandbox::plan::Hearing::default(),
            &|_| {},
        )
        .expect("it starts")
        .number;
    Pane::on(target, id).shows("COMPOSED as steward:");
    Pane::on(target, id).shows("carry on with the plan :END");
    target.sessions.close(id).expect("it closes");

    // A profile this machine does not declare starts nothing.
    let mut elsewhere = named("hello", false);
    elsewhere.profile = Some("someone-elses".to_owned());
    let refused = target
        .sessions
        .open_named(
            None,
            &elsewhere,
            purlis_core::sandbox::plan::Hearing::default(),
            &|_| {},
        )
        .unwrap_err();
    assert!(refused.contains("someone-elses"), "{refused}");
}

/// **A harness chat started by its names is viewed, typed into, answered and stopped** (#3072):
/// the chat a person starts from the window, on a profile the machine it runs on has signed
/// in, composed by the host it runs in. Its tab draws it, what its tab types reaches it, the
/// ask its hook raises is held by the window and answered from there, and stopping every
/// program ends it and keeps its tab. The window's own process composes no such start.
fn a_harness_chat_started_by_its_names_is_viewed_typed_into_answered_and_stopped(target: &Target) {
    if target.host.is_none() {
        let refused = target
            .sessions
            .open_named(
                None,
                &named("start here", false),
                purlis_core::sandbox::plan::Hearing::default(),
                &|_| {},
            )
            .unwrap_err();
        assert_eq!(refused, crate::host::NOT_COMPOSED_HERE);
        return;
    }
    let Hooked {
        harness,
        payload,
        option,
        prints,
    } = &HOOKED_HARNESSES[0];
    let bin = tempfile::tempdir().expect("a folder for the harness");
    let program = stand_in::claude_code(
        bin.path(),
        "claude",
        &format!(
            "#!/bin/sh\nprintf 'READY.'\nread line\nprintf 'you said %s.' \"$line\"\n\
             printf '%s' '{payload}' | PURLIS_HARNESS={harness} '{}' hook permissionrequest\n\
             printf 'decided.'\nread line\n",
            purlis()
        ),
    );
    a_profile(target, "work", &program);
    let ends = told_of_ends(target);

    let id = target
        .sessions
        .open_named(
            None,
            &named("start here", false),
            purlis_core::sandbox::plan::Hearing::default(),
            &|id| {
                target.hooks.board().opened(id, None, None);
            },
        )
        .expect("it starts")
        .number;
    let pane = Pane::on(target, id);
    pane.shows("READY.");

    target.sessions.input(id, b"hello there\r").expect("typed");
    pane.shows("you said hello there.");

    answers_its_ask(target, id, harness, option);
    pane.shows(prints);
    pane.shows("decided.");

    assert_eq!(target.sessions.stop_every_program(), 1);
    until("the window is told it ended", || {
        ends.lock()
            .expect("ends")
            .iter()
            .any(|(ended, _)| *ended == id)
    });
    assert!(target.sessions.running().contains(&id), "its tab stays");
    target.sessions.close(id).expect("it closes");
}

/// **A person's opt-out of a chat the host composes is written in its event log before it
/// starts** (#2171, ADR 0067 §7), under the chat's own run.
fn an_opt_out_of_a_chat_the_host_composes_is_in_its_event_log_before_it_starts(target: &Target) {
    if target.host.is_none() {
        return;
    }
    let bin = tempfile::tempdir().expect("a folder for the harness");
    let harness = stand_in::claude_code(
        bin.path(),
        "claude",
        // The first message, alone on a line of its own (see the test above).
        "#!/bin/sh\nfor arg; do last=\"$arg\"; done\nprintf '\\n%s :END' \"$last\"\nread line\n",
    );
    a_profile(target, "work", &harness);

    let id = target
        .sessions
        .open_named(
            None,
            &named("without the sandbox", true),
            purlis_core::sandbox::plan::Hearing::default(),
            &|_| {},
        )
        .expect("it starts")
        .number;

    Pane::on(target, id).shows("without the sandbox :END");
    let events = purlis_core::eventlog::read(&target.log).expect("the log reads");
    assert!(
        events.iter().any(|event| event.kind == "trust.sandbox.off"),
        "{events:?}"
    );
    target.sessions.close(id).expect("it closes");
}

/// A chat named by the profile `work` and the persona `steward`, told `first_message`.
fn named(first_message: &str, without_sandbox: bool) -> crate::host::Named {
    crate::host::Named {
        tools_allowed: None,
        plane_skills: false,
        chat: None,
        run: None,
        profile: Some("work".to_owned()),
        persona: Some("steward".to_owned()),
        workspace: None,
        resume: None,
        first_message: Some(first_message.to_owned()),
        without_sandbox,
        alone: Vec::new(),
        size: SIZE,
    }
}

/// The target's project has the persona `steward` and the machine its chats run on the
/// profile `name`, of Claude Code, running `program`, as the person approved it there: in the
/// project's own local settings on this machine, and on a Docker pool's runner as a profile of
/// the runner's own, which the window's project settings never reach (they are this machine's
/// alone, #2046), its `claude` being `program`.
fn a_profile(target: &Target, name: &str, program: &Path) {
    a_profile_of(target, "claude", name, program);
}

/// [`a_profile`], of `harness` (`claude` or `codex`).
fn a_profile_of(target: &Target, harness: &str, name: &str, program: &Path) {
    let root = &target.root;
    let persona = root.join("personas").join("steward");
    std::fs::create_dir_all(&persona).expect("its persona");
    std::fs::write(persona.join("persona.md"), "# steward\n").expect("its persona");
    std::fs::write(
        root.join(purlis_core::profiles::LOCAL_FILE),
        format!(
            "[harness.{name}]\nkind = \"{harness}\"\ncommand = [{:?}]\n",
            program.display().to_string()
        ),
    )
    .expect("its profile");
    let declared = purlis_core::profiles::current(root);
    purlis_core::profiletrust::record_launched(
        root,
        name,
        &purlis_core::profiletrust::fingerprint(declared.get(name).expect("declared")),
    )
    .expect("approved");
    if let Some(runner) = &target.runner {
        runner.signs_in_as(harness, name, program);
    }
}

fn what_its_tab_types_reaches_it(target: &Target) {
    let id = target
        .sessions
        .open(
            None,
            &a_harness(&["--sentinel", "HARNESS-READY", "--interactive"]),
            &|_| {},
        )
        .expect("it starts");
    let pane = Pane::on(target, id);
    pane.shows("HARNESS-READY");

    target.sessions.input(id, b"hello there\r").expect("typed");

    pane.shows("you said: hello there");
    target.sessions.close(id).expect("it closes");
}

fn a_resize_reaches_its_terminal(target: &Target) {
    let id = target
        .sessions
        .open(
            None,
            &a_shell("printf 'size %s.' \"$(stty size)\"; read line; printf 'size %s.' \"$(stty size)\"; read line"),
            &|_| {},
        )
        .expect("it starts");
    let pane = Pane::on(target, id);
    pane.shows("size 24 80.");

    target
        .sessions
        .resize(
            id,
            Size {
                columns: 120,
                rows: 40,
            },
        )
        .expect("resized");
    target.sessions.input(id, b"\r").expect("typed");

    pane.shows("size 40 120.");
    target.sessions.close(id).expect("it closes");
}

fn a_chat_is_told_its_own_number(target: &Target) {
    let id = target
        .sessions
        .open(
            Some(7),
            &a_shell("printf 'chat %s.' \"$PURLIS_SESSION_ID\"; read line"),
            &|_| {},
        )
        .expect("it starts");

    assert_eq!(id, 7);
    Pane::on(target, id).shows("chat 7.");
    target.sessions.close(id).expect("it closes");
}

fn a_program_that_ends_is_told_with_its_code_and_its_tab_stays(target: &Target) {
    let ends = told_of_ends(target);
    let id = target
        .sessions
        .open(
            None,
            &a_harness(&[
                "--sentinel",
                "HARNESS-READY",
                "--interactive",
                "--exit-code",
                "7",
            ]),
            &|_| {},
        )
        .expect("it starts");
    let pane = Pane::on(target, id);
    pane.shows("HARNESS-READY");

    target.sessions.input(id, b"/quit\r").expect("typed");

    pane.shows(ENDED);
    until("the window is told", || {
        !ends.lock().expect("ends").is_empty()
    });
    assert_eq!(*ends.lock().expect("ends"), [(id, Exit::Code(7))]);
    assert!(target.sessions.running().contains(&id), "the tab stays");
    assert_eq!(target.sessions.process_id(id), None);
    target.sessions.close(id).expect("it closes");
}

fn stopping_every_program_ends_each_and_keeps_their_tabs(target: &Target) {
    let ends = told_of_ends(target);
    let ids: Vec<u32> = (0..2)
        .map(|_| {
            target
                .sessions
                .open(
                    None,
                    &a_harness(&["--sentinel", "HARNESS-READY", "--interactive"]),
                    &|_| {},
                )
                .expect("it starts")
        })
        .collect();
    for id in &ids {
        Pane::on(target, *id).shows("HARNESS-READY");
    }

    assert_eq!(target.sessions.stop_every_program(), 2);

    until("both are told", || ends.lock().expect("ends").len() == 2);
    for id in &ids {
        assert_eq!(target.sessions.process_id(*id), None);
        assert!(target.sessions.running().contains(id));
        target.sessions.close(*id).expect("it closes");
    }
}

fn a_closed_chat_is_ended_and_gone(target: &Target) {
    let id = target
        .sessions
        .open(
            None,
            &a_harness(&["--sentinel", "HARNESS-READY", "--interactive"]),
            &|_| {},
        )
        .expect("it starts");
    Pane::on(target, id).shows("HARNESS-READY");
    let pid = target.sessions.process_id(id).expect("it runs");

    target.sessions.close(id).expect("it closes");

    assert!(!target.sessions.running().contains(&id));
    assert!(target.sessions.input(id, b"x").is_err());
    until("its program is gone", || !target.alive(pid));
}

fn a_program_reading_a_line_is_not_yet_ready_to_be_typed_at_key_by_key(target: &Target) {
    let id = target
        .sessions
        .open(None, &a_shell("printf 'reading.'; read line"), &|_| {})
        .expect("it starts");
    Pane::on(target, id).shows("reading.");

    let readiness = target.sessions.readiness(id).expect("it answers");

    if let Some(edits_lines) = readiness.edits_lines {
        assert!(
            edits_lines,
            "a shell's `read` leaves the line discipline on"
        );
    }
    target.sessions.close(id).expect("it closes");
}

fn a_chat_starts_with_the_path_and_variables_the_window_gives_it(target: &Target) {
    // What the window's own composition gives a chat started with nothing of its own, from
    // what the process that starts it inherited: every target must give the same, whichever
    // process starts it, and a host gives what it inherited, not what the window has.
    let expected = purlis_core::chatspec::spec(
        target.inherited.iter().cloned(),
        &purlis_core::chatspec::Terminal {
            program: "/bin/sh",
            args: &[],
            cwd: None,
            size: SIZE,
            starting: purlis_core::chatenv::Starting {
                harness: None,
                operator: &[],
                strip: &[],
                set: &[],
                git_hooks: None,
                declared: &[],
            },
            number: 1,
            reporting: None,
            prompt_key: None,
        },
    );
    let value = |name: &str| {
        expected
            .env
            .iter()
            .rev()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.to_string_lossy().into_owned())
            .unwrap_or_default()
    };
    let (path, home, runtime) = (value("PATH"), value("HOME"), value("XDG_RUNTIME_DIR"));
    let inherited_path = target
        .inherited
        .iter()
        .find(|(name, _)| name == "PATH")
        .map(|(_, value)| value.to_string_lossy().into_owned());
    assert_eq!(
        Some(&path),
        inherited_path.as_ref(),
        "a chat is given the PATH its starter inherited"
    );
    let id = target
        .sessions
        .open(
            None,
            &a_shell(
                "read line; printf 'P[%s]H[%s]R[%s].' \"$PATH\" \"$HOME\" \"$XDG_RUNTIME_DIR\"; \
                 read line",
            ),
            &|_| {},
        )
        .expect("it starts");
    let pane = Pane::on(target, id);

    target.sessions.input(id, b"\r").expect("typed");

    pane.shows(&format!("P[{path}]H[{home}]R[{runtime}]."));
    target.sessions.close(id).expect("it closes");
}

/// Makes `root` a plane that commits by itself after a second of quiet: a repository with one
/// commit and its own identity, and no remote.
fn a_plane_saved_after_a_second(root: &Path) {
    std::fs::write(
        root.join(purlis_core::plane::MANIFEST),
        "[plane]\nmode = \"commit\"\nautosave_after = \"1s\"\n",
    )
    .expect("the plane's settings");
    std::fs::write(root.join(".gitignore"), ".charter/\n.purlis/\n").expect("its ignores");
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "user.name", "t"]);
    git(root, &["config", "user.email", "t@example.invalid"]);
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "-m", "one"]);
}

fn git(dir: &Path, args: &[&str]) {
    let mut command = std::process::Command::new("git");
    command
        .arg("-C")
        .arg(dir)
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1");
    let out = purlis_core::forklock::output(&mut command).expect("git runs");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// What the window was told each look did.
fn told_of_looks() -> (
    crate::autosave::Told,
    Arc<Mutex<Vec<crate::autosave::Looked>>>,
) {
    let looks = Arc::new(Mutex::new(Vec::new()));
    let told: crate::autosave::Told = {
        let looks = Arc::clone(&looks);
        Arc::new(move |looked| looks.lock().expect("the looks").push(looked))
    };
    (told, looks)
}

/// How long a save after a second of quiet may take: its looks are two seconds apart, and the
/// standing it decides on may be read again only after `planegit::SHARED_FOR`.
const A_SAVE_WITHIN: Duration = Duration::from_secs(60);

fn a_write_in_the_project_is_saved_after_its_quiet_period_and_the_window_is_told(target: &Target) {
    a_plane_saved_after_a_second(&target.root);
    let (told, looks) = told_of_looks();
    let _saving = target.saver().keep_saving(&target.root, told);

    std::fs::write(target.root.join("note.md"), "a note\n").expect("written");

    let deadline = Instant::now() + A_SAVE_WITHIN;
    while !looks
        .lock()
        .expect("the looks")
        .iter()
        .any(|looked| looked.saved)
    {
        assert!(
            Instant::now() < deadline,
            "the window was never told of a save"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    let journal = purlis_core::planegit::journal(&target.root);
    let saved = journal
        .iter()
        .find(|line| line["outcome"] == "committed")
        .expect("a save was journalled");
    assert_eq!(saved["trigger"], "quiet");
    let tracked = purlis_core::forklock::output(
        std::process::Command::new("git")
            .arg("-C")
            .arg(&target.root)
            .args(["ls-files", "note.md"]),
    )
    .expect("git runs");
    assert_eq!(String::from_utf8_lossy(&tracked.stdout).trim(), "note.md");
}

fn exactly_one_saver_runs_for_the_project_and_none_once_it_is_stopped(target: &Target) {
    a_plane_saved_after_a_second(&target.root);
    let (told, looks) = told_of_looks();
    let saving = target.saver().keep_saving(&target.root, told);
    std::fs::write(target.root.join("first.md"), "saved\n").expect("written");

    let deadline = Instant::now() + A_SAVE_WITHIN;
    while !looks
        .lock()
        .expect("the looks")
        .iter()
        .any(|looked| looked.saved)
    {
        assert!(Instant::now() < deadline, "no saver ever saved");
        std::thread::sleep(Duration::from_millis(50));
    }
    // Whatever was to start has had the time to. The window's own process runs a saver only
    // where the target's saver is in it: never one beside the host's.
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(
        crate::autosave::running::on(&target.root),
        target.savers_here,
        "the window and the host both save the project"
    );

    drop(saving);

    until("the saver stops", || {
        crate::autosave::running::on(&target.root) == 0
    });
    let told_before = looks.lock().expect("the looks").len();
    std::fs::write(target.root.join("second.md"), "not saved\n").expect("written");
    // Its quiet period and two of its looks, two seconds apart, go by with nothing saved.
    std::thread::sleep(Duration::from_secs(5));
    assert_eq!(
        looks.lock().expect("the looks").len(),
        told_before,
        "the window was told of a look after its saving stopped"
    );
    let tracked = purlis_core::forklock::output(
        std::process::Command::new("git")
            .arg("-C")
            .arg(&target.root)
            .args(["ls-files", "second.md"]),
    )
    .expect("git runs");
    assert_eq!(
        String::from_utf8_lossy(&tracked.stdout).trim(),
        "",
        "a write after the saving stopped was saved"
    );
}

/// A harness whose hooks report and ask through `purlis hook`: the word a chat's environment
/// names it with (`PURLIS_HARNESS`), a permission ask's payload as its hook is handed it, the
/// option the window answers it with, and what its hook then prints for the harness.
struct Hooked {
    harness: &'static str,
    payload: &'static str,
    option: &'static str,
    prints: &'static str,
}

const HOOKED_HARNESSES: [Hooked; 3] = [
    Hooked {
        harness: "claude-code",
        payload: r#"{"tool_name":"Bash","tool_input":{"command":"npm test"}}"#,
        option: "allow",
        prints: r#""behavior":"allow""#,
    },
    Hooked {
        harness: "codex",
        payload: r#"{"tool_name":"Bash","tool_input":{"command":"npm test"}}"#,
        option: "allow",
        prints: r#""behavior":"allow""#,
    },
    // opencode's shell line is answered only with a refusal from the window (#1691).
    Hooked {
        harness: "opencode",
        payload: r#"{"permission":"bash","patterns":["npm test"]}"#,
        option: "reject",
        prints: r#"{"reply":"reject"}"#,
    },
];

/// Started as the window starts a chat: its number on the board before its program runs, so
/// its first hook lands.
fn a_chat_on_the_board(target: &Target, opening: &Opening) -> u32 {
    target
        .sessions
        .open(None, opening, &|id| {
            target.hooks.board().opened(id, None, None);
        })
        .expect("it starts")
}

fn a_chat_whose_turn_ends_needs_you_on_every_harness(target: &Target) {
    for Hooked { harness, .. } in HOOKED_HARNESSES {
        let id = a_chat_on_the_board(
            target,
            &a_shell(&format!(
                "PURLIS_HARNESS={harness} '{}' hook stop </dev/null; printf 'reported.'; read line",
                purlis()
            )),
        );

        Pane::on(target, id).shows("reported.");

        until(
            &format!("{harness}: the window is told the chat needs you"),
            || {
                target.moved.lock().expect("the moves").iter().any(|moved| {
                    moved.session == id && moved.needs_you && moved.queue.contains(&id)
                })
            },
        );
        target.sessions.close(id).expect("it closes");
    }
}

/// Waits until the window holds an ask of chat `id`, on `harness`, and answers it `option`, as
/// the person does from Needs you.
fn answers_its_ask(target: &Target, id: u32, harness: &str, option: &str) {
    let ask = {
        let deadline = Instant::now() + PATIENCE;
        loop {
            let pending = target.hooks.asks().pending(Instant::now());
            if let Some(ask) = pending.into_iter().find(|ask| ask.chat == id.to_string()) {
                break ask;
            }
            assert!(
                Instant::now() < deadline,
                "{harness}: the window never held the ask"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    };
    target
        .hooks
        .answer(id, &ask.id.to_string(), option)
        .expect("answered in the window");
}

fn a_permission_ask_is_answered_from_the_window_on_every_harness(target: &Target) {
    for Hooked {
        harness,
        payload,
        option,
        prints,
    } in HOOKED_HARNESSES
    {
        let id = a_chat_on_the_board(
            target,
            &a_shell(&format!(
                "printf '%s' '{payload}' | PURLIS_HARNESS={harness} '{}' hook permissionrequest; \
                 printf 'decided.'; read line",
                purlis()
            )),
        );
        let pane = Pane::on(target, id);
        answers_its_ask(target, id, harness, option);

        pane.shows(prints);
        pane.shows("decided.");
        target.sessions.close(id).expect("it closes");
    }
}

/// The program the behaviours run on the machine a chat runs on, as a chat's shell names it: the
/// repo's fake harness, at the path a pool's runner has it too (`tests/parity/runner.Dockerfile`).
fn the_fake_harness() -> String {
    format!("'{}'", fake_harness().display())
}

/// The rival listener is started beside the chat on the machine it runs on, by its own shell:
/// below none of the chat's hooks, on every target, a pool's runner included (#3288).
fn a_hook_refuses_a_listener_that_is_not_its_chats_ancestor(target: &Target) {
    let payload = HOOKED_HARNESSES[0].payload;
    let id = a_chat_on_the_board(
        target,
        &a_shell(&format!(
            "{} --listen-at \"${}\" & rival=$!; \
             read line; printf '%s' '{payload}' | '{}' hook permissionrequest; \
             printf 'left to its own prompt.'; kill $rival 2>/dev/null; wait; read line",
            the_fake_harness(),
            purlis_core::hookwire::SOCKET_ENV,
            purlis()
        )),
    );
    let pane = Pane::on(target, id);
    pane.shows("listening beside the chat.");

    target.sessions.input(id, b"\r").expect("typed");

    pane.shows("is not the host that started this chat");
    pane.shows("left to its own prompt.");
    assert!(
        target.hooks.asks().pending(Instant::now()).is_empty(),
        "an ask reached the window through a listener that is not the chat's ancestor"
    );
    target.sessions.close(id).expect("it closes");
}

/// Stop all, as the title bar throws it (`Planes::stop_every_agent`): the switch first, so
/// nothing starts in the second the ending takes, then every program.
fn stop_all(target: &Target) -> usize {
    target
        .switch
        .stop(purlis_core::halt::Actor::Window)
        .expect("the stop is kept");
    target.sessions.stop_every_program()
}

fn stop_all_ends_every_chat_and_none_starts_until_the_window_re_arms(target: &Target) {
    let ends = told_of_ends(target);
    let ids: Vec<u32> = (0..2)
        .map(|_| {
            target
                .sessions
                .open(
                    None,
                    &a_harness(&["--sentinel", "HARNESS-READY", "--interactive"]),
                    &|_| {},
                )
                .expect("it starts")
        })
        .collect();
    for id in &ids {
        Pane::on(target, *id).shows("HARNESS-READY");
    }

    assert_eq!(stop_all(target), 2);

    until("both are told", || ends.lock().expect("ends").len() == 2);
    for id in &ids {
        assert_eq!(target.sessions.process_id(*id), None);
    }
    let refused = target.sessions.open(
        None,
        &a_harness(&["--sentinel", "HARNESS-READY", "--interactive"]),
        &|_| {},
    );
    assert_eq!(refused, Err(crate::sessions::STOPPED.to_owned()));

    assert!(target.switch.rearm().expect("re-armed"));

    let id = target
        .sessions
        .open(
            None,
            &a_harness(&["--sentinel", "HARNESS-READY", "--interactive"]),
            &|_| {},
        )
        .expect("it starts once re-armed");
    Pane::on(target, id).shows("HARNESS-READY");
    for id in ids.iter().chain([&id]) {
        target.sessions.close(*id).expect("it closes");
    }
}

fn while_stopped_the_operators_shell_opens(target: &Target) {
    stop_all(target);

    let id = target
        .sessions
        .open(
            None,
            &Opening {
                operator_shell: true,
                ..a_shell("printf 'looking around.'; read line")
            },
            &|_| {},
        )
        .expect("the operator's shell opens");

    Pane::on(target, id).shows("looking around.");
    target.sessions.close(id).expect("it closes");
}

// A sandboxed chat's network (#1999): its confinement, started where its chat runs.

/// A chat's plan listing `listed`, as the window compiles one for a harness it wraps, giving it
/// the services on this machine at `given`.
fn a_plan(listed: Vec<(String, By)>, given: Vec<u16>) -> Plan {
    Plan {
        reach: Reach::of(listed).written(),
        tunnel_ports: egress::TUNNEL_PORTS.to_vec(),
        plain_ports: egress::PLAIN_PORTS.to_vec(),
        any_port: false,
        beside: plan::Beside::Wrapped,
        given,
    }
}

/// A service on the machine `target`'s chats run on, which answers nothing, served by a chat of
/// its own beside them: that chat, and the port it serves on. What a chat may be given by its
/// port.
fn a_service(target: &Target) -> (u32, u16) {
    let id = target
        .sessions
        .open(None, &a_harness(&["--serve"]), &|_| {})
        .expect("it starts");
    let port = Pane::on(target, id).number_after("serving on port ");
    (id, port)
}

/// [`a_service`], which you let every chat of `target`'s project reach on that machine: a host
/// the window's plan reaches through a link gives a chat only what its own sandbox for the
/// project gives (#2445).
fn a_service_you_gave(target: &Target) -> (u32, u16) {
    let (id, port) = a_service(target);
    target.gives(port);
    (id, port)
}

/// What a chat's shell runs to ask its proxy, on its HTTP `port` on the chat's own machine, for
/// a tunnel to `to`: the chat says the status line it is answered with.
fn tunnel_through(port: u16, to: &str) -> String {
    format!(
        "{} --tunnel '{to}' --through 127.0.0.1:{port}",
        the_fake_harness()
    )
}

/// What the chat says its proxy answered a tunnel to `to` with.
fn tunnelled(to: &str, status: &str) -> String {
    format!("tunnel to {to}: {status}")
}

/// What a chat's proxy told the window, by kind.
#[derive(Default)]
struct HeardOfIt {
    refused: Mutex<Vec<String>>,
    asked: Mutex<Vec<String>>,
}

fn hearing(heard: &Arc<HeardOfIt>, asks: bool) -> Hearing {
    Hearing {
        refusals: egress::Refusals::telling({
            let heard = Arc::clone(heard);
            Arc::new(move |host: &str, port: u16| {
                heard
                    .refused
                    .lock()
                    .expect("heard")
                    .push(egress::host_and_port(host, port));
            })
        }),
        asks: asks.then(|| Asking {
            tell: {
                let heard = Arc::clone(heard);
                Arc::new(move |told| {
                    if let purlis_core::sandbox::asks::Heard::Asked(targets) = told {
                        heard.asked.lock().expect("heard").extend(targets);
                    }
                })
            },
            askable: Arc::new(|| true),
            known: Arc::new(|_| None),
        }),
        ..Hearing::default()
    }
}

/// A chat that runs in `confinement`, as the window starts one, and runs `script` once it says
/// it is confined: the chat, and its pane.
fn in_it(target: &Target, confinement: &Confinement, script: &str) -> (u32, Pane) {
    let mut opening = a_shell(&format!("printf 'confined.'; {script}; read line"));
    opening
        .env
        .push(("HTTPS_PROXY".into(), confinement.proxy_url()));
    opening.confinement = confinement.held_as().map(str::to_owned);
    let id = target
        .sessions
        .open(None, &opening, &|_| {})
        .expect("it starts");
    let pane = Pane::on(target, id);
    pane.shows("confined.");
    (id, pane)
}

/// The chat itself tunnels through its own proxy, on its own machine (#3288).
fn a_sandboxed_chats_proxy_carries_what_it_lists_and_refuses_the_rest(target: &Target) {
    let (service, port) = a_service_you_gave(target);
    let heard = Arc::new(HeardOfIt::default());
    let confinement = target
        .sessions
        .confine(
            &a_plan(vec![(format!("127.0.0.1:{port}"), By::You)], vec![port]),
            hearing(&heard, false),
        )
        .expect("confined");
    let proxy = confinement.proxy_port();
    let given = format!("127.0.0.1:{port}");
    let (id, pane) = in_it(
        target,
        &confinement,
        &format!(
            "{}; {}",
            tunnel_through(proxy, &given),
            tunnel_through(proxy, "refused.invalid:443")
        ),
    );

    pane.shows(&tunnelled(&given, "HTTP/1.1 200 Connection established"));
    pane.shows(&tunnelled("refused.invalid:443", "HTTP/1.1 403 Forbidden"));
    until("the window hears of the refusal", || {
        *heard.refused.lock().expect("heard") == ["refused.invalid:443"]
    });
    target.sessions.close(id).expect("it closes");
    target.sessions.close(service).expect("it closes");
}

fn a_port_withdrawn_while_a_chat_runs_is_carried_no_longer(target: &Target) {
    let (service, port) = a_service_you_gave(target);
    let confinement = target
        .sessions
        .confine(
            &a_plan(vec![(format!("127.0.0.1:{port}"), By::You)], vec![port]),
            Hearing::default(),
        )
        .expect("confined");
    let given = format!("127.0.0.1:{port}");
    let (id, pane) = in_it(
        target,
        &confinement,
        &format!(
            "read line; {}",
            tunnel_through(confinement.proxy_port(), &given)
        ),
    );

    confinement.withdraw_port(port).expect("withdrawn");
    target.sessions.input(id, b"\r").expect("typed");

    pane.shows(&tunnelled(&given, "HTTP/1.1 403 Forbidden"));
    target.sessions.close(id).expect("it closes");
    target.sessions.close(service).expect("it closes");
}

fn an_ask_raised_by_a_chats_proxy_is_answered_from_the_window(target: &Target) {
    let heard = Arc::new(HeardOfIt::default());
    let confinement = target
        .sessions
        .confine(&a_plan(Vec::new(), Vec::new()), hearing(&heard, true))
        .expect("confined");
    let (id, pane) = in_it(
        target,
        &confinement,
        &tunnel_through(confinement.proxy_port(), "held.invalid:443"),
    );
    until("the window is asked", || {
        *heard.asked.lock().expect("heard") == ["held.invalid:443"]
    });
    let board = confinement.asks().expect("its proxy asks live");
    assert!(board.asks_about(&Host::parse("held.invalid").expect("a host")));

    let retry = board.allow(&Host::parse("held.invalid").expect("a host"), By::Chat);

    // Carried: the proxy tried the host, which never resolves.
    pane.shows(&tunnelled("held.invalid:443", "HTTP/1.1 502 Bad Gateway"));
    assert!(retry.is_empty(), "nothing had given up");
    assert_eq!(
        board.allowed_live(),
        [("held.invalid".to_owned(), By::Chat)]
    );
    target.sessions.close(id).expect("it closes");
}

/// **The host's own targets only**: the window's own process has no host to start again, and it
/// is the host that keeps the stop across its restart and refuses a start the window did not
/// stop first (#2001). The window here is one that never heard of the stop. `a_host` starts a
/// host keeping what it keeps in the folder it is given, and a window on it.
fn the_host_refuses_a_start_while_stopped_and_holds_the_stop_across_its_restart(
    a_host: fn(&Path) -> Target,
) {
    let kept = tempfile::tempdir().expect("the host's folder");
    {
        let before = a_host(kept.path());
        let id = before
            .sessions
            .open(
                None,
                &a_harness(&["--sentinel", "HARNESS-READY", "--interactive"]),
                &|_| {},
            )
            .expect("it starts");
        Pane::on(&before, id).shows("HARNESS-READY");
        stop_all(&before);
        before.sessions.close(id).expect("it closes");
    }

    let after = a_host(kept.path());
    assert!(!after.switch.is_stopped(), "this window never heard of it");

    let refused = after
        .sessions
        .open(
            None,
            &a_harness(&["--sentinel", "HARNESS-READY", "--interactive"]),
            &|_| {},
        )
        .expect_err("the host starts nothing");

    assert!(
        refused.contains("kill switch"),
        "the host's words: {refused}"
    );
    assert!(
        after.switch.agree_with_its_host(),
        "the window learns of it"
    );
    assert!(after.switch.is_stopped());
    assert!(after.switch.rearm().expect("re-armed"));
    let id = after
        .sessions
        .open(
            None,
            &a_harness(&["--sentinel", "HARNESS-READY", "--interactive"]),
            &|_| {},
        )
        .expect("it starts once the window re-armed");
    Pane::on(&after, id).shows("HARNESS-READY");
    after.sessions.close(id).expect("it closes");
}

static HELD: std::sync::LazyLock<Credentials> =
    std::sync::LazyLock::new(|| Credentials::mint().expect("credentials"));

/// A `local-ui` link to `hosting` through a stream a test can cut, the cut kept in `cuts`.
pub(crate) async fn a_cuttable_link(
    hosting: purlis_host::Hosting,
    cuts: Arc<Mutex<Vec<tokio::task::AbortHandle>>>,
) -> Result<HostLink, String> {
    let (windows_end, near) = tokio::io::duplex(1 << 20);
    let (far, hosts_end) = tokio::io::duplex(1 << 20);
    let cut = tokio::spawn(async move {
        let (mut near, mut far) = (near, far);
        let _ = tokio::io::copy_bidirectional(&mut near, &mut far).await;
    })
    .abort_handle();
    cuts.lock().expect("the cuts").push(cut);
    let (client, served) = tokio::join!(
        link::connect(
            windows_end,
            session::speaks(),
            Scope::LocalUi,
            HELD.of(Scope::LocalUi)
        ),
        link::serve_any(hosts_end, session::speaks(), &HELD)
    );
    tokio::spawn(session::serve(
        served.map_err(|why| why.to_string())?,
        hosting,
        None,
    ));
    HostLink::over(
        client.map_err(|why| why.to_string())?,
        tokio::runtime::Handle::current(),
        || {},
    )
    .await
}

/// A window of the session host's own, over links a test can cut as a crash of the window
/// would: its hooks for the project, its chats, and what it is told.
struct AWindow {
    sessions: HostedSessions,
    hooks: Hooks,
    moved: Arc<Mutex<Vec<Moved>>>,
    /// The window's connection: its own link, and the project root's (#2178).
    connection: Arc<Connection>,
    root: PathBuf,
    /// What cuts each of its links.
    cuts: Arc<Mutex<Vec<tokio::task::AbortHandle>>>,
}

impl AWindow {
    /// A window on `hosting`, for the project at `root`.
    fn on(hosting: &purlis_host::Hosting, root: &Path, runtime: &tokio::runtime::Runtime) -> Self {
        let cuts: Arc<Mutex<Vec<tokio::task::AbortHandle>>> = Arc::default();
        let link = runtime
            .block_on(a_cuttable_link(hosting.clone(), Arc::clone(&cuts)))
            .expect("the window speaks to the host");
        let dial: crate::hostedsessions::Dial = {
            let (hosting, cuts, handle) =
                (hosting.clone(), Arc::clone(&cuts), runtime.handle().clone());
            Arc::new(move |within, _naming| {
                crate::hostedsessions::open_within(
                    &handle,
                    within,
                    a_cuttable_link(hosting.clone(), Arc::clone(&cuts)),
                )
            })
        };
        let connection = Arc::new(Connection::default());
        connection.ready(Arc::new(link), dial);
        let (hooks, moved) = hooks_of(root, true);
        let sessions = HostedSessions::new(Arc::clone(&connection), root);
        sessions.hear_through(hooks.relayed_from_the_host().expect("heard from the host"));
        AWindow {
            sessions,
            hooks,
            moved,
            connection,
            root: root.to_path_buf(),
            cuts,
        }
    }

    /// Cuts every link of the window's, as its crash would.
    fn cut(&self) {
        for cut in self.cuts.lock().expect("the cuts").iter() {
            cut.abort();
        }
    }

    /// A chat started as the window starts one: on the board before its program runs.
    fn starts(&self, script: &str) -> u32 {
        self.sessions
            .open(None, &a_shell(script), &|id| {
                self.hooks.board().opened(id, None, None);
            })
            .expect("it starts")
    }

    /// Whether the window was told chat `id` needs you.
    fn told_it_needs_you(&self, id: u32) -> bool {
        self.moved
            .lock()
            .expect("the moves")
            .iter()
            .any(|moved| moved.session == id && moved.needs_you && moved.queue.contains(&id))
    }
}

/// **The host's own target only** (#2194): the window's own process keeps no board that
/// outlives it. A window whose link is cut as a crash cuts it comes back to the host, which kept
/// the project's board meanwhile: every chat's state and Needs you are as the host kept them,
/// a chat that said something while the window was away included.
#[test]
fn a_window_killed_and_back_reads_every_chats_state_and_needs_you_as_the_host_kept_them() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("a runtime");
    let kept = tempfile::tempdir().expect("the host's folder");
    let data = tempfile::tempdir().expect("the host's data");
    let hosting = a_host(kept.path(), data.path());
    let (_project, root) = a_project();
    let away = tempfile::tempdir().expect("a folder for the chat's signals");
    let speak = away.path().join("speak");
    let spoke = away.path().join("spoke");

    let first = AWindow::on(&hosting, &root, &runtime);
    let quiet = first.starts("printf 'quiet.'; read line");
    let ended_its_turn = first.starts(&format!(
        "PURLIS_HARNESS=claude '{}' hook stop </dev/null; printf 'reported.'; read line",
        purlis()
    ));
    let speaks_while_away = first.starts(&format!(
        "while [ ! -e '{speak}' ]; do sleep 0.1; done; \
         PURLIS_HARNESS=claude '{purlis}' hook stop </dev/null; touch '{spoke}'; read line",
        speak = speak.display(),
        spoke = spoke.display(),
        purlis = purlis(),
    ));
    until("the first window is told the chat needs you", || {
        first.told_it_needs_you(ended_its_turn)
    });
    // Every move the first window made has reached the host: a read of the board follows them.
    let project = purlis_core::scaffold::planefile::id_to_hold_by(&root).expect("its id");
    first
        .connection
        .of_project(&first.root, PATIENCE)
        .expect("the root's link")
        .call(session::Command::Board { project })
        .expect("the host answers its board");

    first.cut();
    drop(first);
    std::fs::write(&speak, "").expect("the chat is told to speak");
    until("the chat spoke with no window there", || spoke.exists());

    let back = AWindow::on(&hosting, &root, &runtime);
    back.sessions.hold().expect("it holds the project again");

    let board = back.hooks.board();
    assert_eq!(
        board.chats().collect::<Vec<_>>(),
        [quiet, ended_its_turn, speaks_while_away]
    );
    assert_eq!(board.state(quiet), purlis_core::state::State::Unknown);
    assert_eq!(
        board.state(ended_its_turn),
        purlis_core::state::State::Waiting
    );
    assert_eq!(
        board.state(speaks_while_away),
        purlis_core::state::State::Waiting
    );
    assert_eq!(board.needs_you(), [ended_its_turn, speaks_while_away]);
    drop(board);
    assert!(back.told_it_needs_you(ended_its_turn));
    assert!(back.told_it_needs_you(speaks_while_away));
}

// ---------------------------------------------------------------------------------------
// a window killed and started again reattaches every chat (#2006, #646)
// ---------------------------------------------------------------------------------------

/// How many chats the reattach is measured with (#646, spec #1974's measured claim).
const TWENTY: usize = 20;

/// Twenty chats started by a window, each writing before the window goes and again once
/// `speak` exists, with nobody attached, then touching its number in `spoke`, then echoing a
/// line typed to it. Their numbers, in order.
fn twenty_chats_in(sessions: &dyn SessionHost, speak: &Path, spoke: &Path) -> Vec<u32> {
    let script = format!(
        "printf 'chat %s before.' \"$PURLIS_SESSION_ID\"; \
         while [ ! -e '{speak}' ]; do sleep 0.1; done; \
         printf ' while away.'; touch '{spoke}'/\"$PURLIS_SESSION_ID\"; \
         read line; printf ' got %s.' \"$line\"; read line",
        speak = speak.display(),
        spoke = spoke.display(),
    );
    let numbers: Vec<u32> = (0..TWENTY)
        .map(|_| {
            sessions
                .open(None, &a_shell(&script), &|_| {})
                .expect("it starts")
        })
        .collect();
    for id in &numbers {
        Pane::of(sessions, *id).shows(&format!("chat {id} before."));
    }
    numbers
}

/// The window is gone: each chat writes again with nobody attached.
fn they_speak_while_no_window_is_there(speak: &Path, spoke: &Path) {
    std::fs::write(speak, "").expect("the chats are told to speak");
    until("every chat spoke with no window there", || {
        std::fs::read_dir(spoke).map_or(0, Iterator::count) == TWENTY
    });
}

/// **Every chat comes back with nothing lost**: the window that came back reattaches each, under
/// its own number, without starting it again; its pane draws what it wrote before the window
/// went and while it was gone; and what is typed to it reaches it.
fn every_chat_comes_back_with_nothing_lost(back: &dyn SessionHost, numbers: &[u32]) {
    let started = Instant::now();
    for id in numbers {
        assert!(back.left_running(*id), "chat {id} is in the host");
        assert_eq!(back.reattach(*id, &|_| {}), Ok(*id));
    }
    for id in numbers {
        let pane = Pane::of(back, *id);
        pane.shows(&format!("chat {id} before. while away."));
        back.input(*id, b"hello\n").expect("typed to it");
        pane.shows(" got hello.");
    }
    assert_eq!(back.running(), numbers, "every tab, and no other");
    assert!(
        started.elapsed() < Duration::from_secs(30),
        "twenty chats came back in {:?}",
        started.elapsed()
    );
}

/// **The host's own code, over in-memory links** (#2006): a window whose link is cut as a
/// crash cuts it, and a window back on the same host.
#[test]
fn twenty_live_chats_reattach_after_the_window_is_killed_in_the_host() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("a runtime");
    let kept = tempfile::tempdir().expect("the host's folder");
    let data = tempfile::tempdir().expect("the host's data");
    let hosting = a_host(kept.path(), data.path());
    let (_project, root) = a_project();
    let signals = tempfile::tempdir().expect("a folder for the chats' signals");
    let (speak, spoke) = (signals.path().join("speak"), signals.path().join("spoke"));
    std::fs::create_dir(&spoke).expect("the folder each chat says it spoke in");

    let first = AWindow::on(&hosting, &root, &runtime);
    let numbers = twenty_chats_in(&first.sessions, &speak, &spoke);
    first.cut();
    std::mem::forget(first.sessions);
    they_speak_while_no_window_is_there(&speak, &spoke);

    let back = AWindow::on(&hosting, &root, &runtime);
    every_chat_comes_back_with_nothing_lost(&back.sessions, &numbers);
    back.sessions.end_all();
}

/// **A real host process** (#2006, #646): the window's binary started as its session host by a
/// window that is then killed outright (its link and the host's standard input both gone, with
/// no quit), and a window started again, which finds the host serving and reattaches to it. On
/// Linux, as the host process's target is.
#[cfg(target_os = "linux")]
#[test]
fn twenty_live_chats_reattach_after_the_window_is_killed_and_started_again() {
    let home = tempfile::tempdir().expect("the host's home");
    let HostHome {
        place, inherited, ..
    } = a_host_home(home.path());
    let (_project, root) = a_project();
    let signals = tempfile::tempdir().expect("a folder for the chats' signals");
    let (speak, spoke) = (signals.path().join("speak"), signals.path().join("spoke"));
    std::fs::create_dir(&spoke).expect("the folder each chat says it spoke in");
    let starts = {
        let inherited = inherited.clone();
        move || {
            let mut command = crate::sessionhost::command(&charter_app());
            command.env_clear().envs(inherited.iter().cloned());
            Ok(command)
        }
    };

    let first = KilledWindow::start(&place, &root, starts.clone());
    let numbers = twenty_chats_in(&first.sessions, &speak, &spoke);
    let first = first.killed();
    they_speak_while_no_window_is_there(&speak, &spoke);

    let back = Ends(crate::sessionhost::SessionHost::default());
    purlis_core::hostsocket::make_the_folder(&place.socket).expect("the socket's folder");
    back.0.start_at(place, starts);
    let sessions = HostedSessions::new(back.0.connection(), &root);
    every_chat_comes_back_with_nothing_lost(&sessions, &numbers);
    // The window quits: the host ends every chat and stops, and so ends the first one's host.
    drop(back);
    first.ended_by_the_quit();
}

/// **The way back after a crash** (#2004): a window running its chats in the session host is
/// killed outright, and the host keeps a chat for the grace period. The window started again
/// with its chats in its own process (`PURLIS_CHATS_IN_THE_HOST=0`) ends that host as a quit
/// before it puts any chat back: by the time its gate lets the chats back, the chat the host
/// kept has ended, so its conversation never has two programs at once. Then it starts a host of
/// its own. A host process's target only. The test ends every host it started.
#[cfg(target_os = "linux")]
#[test]
fn a_window_flipped_back_after_a_crash_ends_the_chats_the_host_kept_before_it_puts_any_back() {
    let home = tempfile::tempdir().expect("the host's home");
    let HostHome {
        place, inherited, ..
    } = a_host_home(home.path());
    let (_project, root) = a_project();
    let marks = tempfile::tempdir().expect("a folder for the chat's process");
    let pid_file = marks.path().join("pid");
    let starts = {
        let inherited = inherited.clone();
        move || {
            let mut command = crate::sessionhost::command(&charter_app());
            command.env_clear().envs(inherited.iter().cloned());
            Ok(command)
        }
    };
    let first = KilledWindow::start(&place, &root, starts.clone());
    let script = format!("echo $$ > '{}'; exec sleep 600", pid_file.display());
    first
        .sessions
        .open(None, &a_shell(&script), &|_| {})
        .expect("the chat starts in the host");
    until("the chat to say its process", || {
        std::fs::read_to_string(&pid_file).is_ok_and(|said| said.ends_with('\n'))
    });
    let pid: i32 = std::fs::read_to_string(&pid_file)
        .expect("its process")
        .trim()
        .parse()
        .expect("a pid");
    let first = first.killed();
    // Running, and not a zombie waiting for its parent.
    let alive = |pid: i32| {
        std::fs::read_to_string(format!("/proc/{pid}/stat"))
            .is_ok_and(|stat| stat.split_whitespace().nth(2) != Some("Z"))
    };
    assert!(
        alive(pid),
        "the host keeps the chat while no window is there"
    );

    let back = Ends(crate::sessionhost::SessionHost::default());
    back.0.ending_a_host_it_finds();
    purlis_core::hostsocket::make_the_folder(&place.socket).expect("the socket's folder");
    back.0.start_at(place, starts);
    let gate = back.0.before_chats_are_put_back(PATIENCE);

    assert_eq!(gate(), Ok(()), "the window may put its chats back");
    assert!(
        !alive(pid),
        "the chat the host kept still runs as the window puts chats back"
    );
    first.ended_by_the_quit();
    back.0
        .connection()
        .wait(PATIENCE)
        .expect("a host of the window's own serves");
}

/// **A change written while the window is gone is committed by the host process** (#2287,
/// #2002): a window started on a real git plane has the host save it, and is killed outright
/// (its links and the host's standard input gone, with no quit). A file then written in the
/// plane is committed by the host, well within the grace period it waits for a window in. A
/// host process's target only: there is a window to kill and a host that outlives it. The test
/// ends the host it started.
#[cfg(target_os = "linux")]
#[test]
fn a_change_written_while_the_window_is_gone_is_committed_by_the_host_process() {
    const _: () = assert!(A_SAVE_WITHIN.as_secs() < purlis_host::GRACE.as_secs());
    let home = tempfile::tempdir().expect("the host's home");
    let HostHome {
        place, inherited, ..
    } = a_host_home(home.path());
    let (_project, root) = a_project();
    a_plane_saved_after_a_second(&root);
    let window = KilledWindow::start(&place, &root, move || {
        let mut command = crate::sessionhost::command(&charter_app());
        command.env_clear().envs(inherited.iter().cloned());
        Ok(command)
    });
    let (told, looks) = told_of_looks();
    let saving = window.sessions.keep_saving(&root, told);
    // The host saves the plane before the window goes.
    std::fs::write(root.join("before.md"), "written with the window there\n").expect("written");
    let deadline = Instant::now() + A_SAVE_WITHIN;
    while !looks
        .lock()
        .expect("the looks")
        .iter()
        .any(|looked| looked.saved)
    {
        assert!(Instant::now() < deadline, "the host never saved the plane");
        std::thread::sleep(Duration::from_millis(50));
    }

    let mut window = window.killed();
    // A window killed outright stops nothing: its saver is never dropped.
    std::mem::forget(saving);
    std::fs::write(root.join("away.md"), "written with the window gone\n").expect("written");

    let deadline = Instant::now() + A_SAVE_WITHIN;
    while !committed(&root, "away.md") {
        assert!(
            Instant::now() < deadline,
            "the host did not commit a change written with the window gone within {A_SAVE_WITHIN:?}"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(
        window.host_runs(),
        "the host that committed it outlived the window"
    );
    window.end_its_host();
}

/// Whether `file` is in a commit of the repository at `root`.
#[cfg(target_os = "linux")]
fn committed(root: &Path, file: &str) -> bool {
    let out = purlis_core::forklock::output(
        std::process::Command::new("git")
            .arg("-C")
            .arg(root)
            .args(["log", "--format=%H", "--", file])
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1"),
    )
    .expect("git runs");
    out.status.success() && !out.stdout.trim_ascii().is_empty()
}

/// A window that starts the window's binary as its session host itself, as the window does,
/// over a link a test can cut, and is then killed outright.
#[cfg(target_os = "linux")]
struct KilledWindow {
    host: std::process::Child,
    stdin: Option<std::process::ChildStdin>,
    sessions: HostedSessions,
    /// What cuts each of its links.
    cuts: Arc<Mutex<Vec<tokio::task::AbortHandle>>>,
    _runtime: tokio::runtime::Runtime,
}

#[cfg(target_os = "linux")]
impl KilledWindow {
    /// A window on the project at `root`, whose host `starts` starts at `place`.
    fn start(
        place: &purlis_host::Place,
        root: &Path,
        starts: impl FnOnce() -> Result<std::process::Command, String>,
    ) -> KilledWindow {
        use std::io::BufRead;
        purlis_core::hostsocket::make_the_folder(&place.socket).expect("the socket's folder");
        let mut host = purlis_core::forklock::spawn(&mut starts().expect("a command"))
            .expect("the host starts");
        let stdin = host.stdin.take();
        let mut ready = String::new();
        std::io::BufReader::new(host.stdout.take().expect("its output"))
            .read_line(&mut ready)
            .expect("it says it is ready");
        assert_eq!(ready.trim_end(), purlis_host::READY);
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("a runtime");
        let credential =
            purlis_session_protocol::auth::Credential::read(&place.credentials, Scope::LocalUi)
                .expect("the window's credential");
        let cuts: Arc<Mutex<Vec<tokio::task::AbortHandle>>> = Arc::default();
        let linked = {
            let (socket, credential, cuts) = (place.socket.clone(), credential, Arc::clone(&cuts));
            move || {
                let (socket, credential, cuts) =
                    (socket.clone(), credential.clone(), Arc::clone(&cuts));
                async move {
                    let stream = tokio::net::UnixStream::connect(&socket)
                        .await
                        .map_err(|why| why.to_string())?;
                    let (windows_end, near) = tokio::io::duplex(1 << 20);
                    let cut = tokio::spawn(async move {
                        let (mut near, mut stream) = (near, stream);
                        let _ = tokio::io::copy_bidirectional(&mut near, &mut stream).await;
                    })
                    .abort_handle();
                    cuts.lock().expect("the cuts").push(cut);
                    let linked =
                        link::connect(windows_end, session::speaks(), Scope::LocalUi, &credential)
                            .await
                            .map_err(|why| why.to_string())?;
                    HostLink::over(linked, tokio::runtime::Handle::current(), || {}).await
                }
            }
        };
        let link = runtime
            .block_on(linked())
            .expect("the window speaks to the host");
        let dial: crate::hostedsessions::Dial = {
            let handle = runtime.handle().clone();
            Arc::new(move |within, _naming| {
                crate::hostedsessions::open_within(&handle, within, linked())
            })
        };
        let connection = Arc::new(Connection::default());
        connection.ready(Arc::new(link), dial);
        KilledWindow {
            host,
            stdin,
            sessions: HostedSessions::new(connection, root),
            cuts,
            _runtime: runtime,
        }
    }

    /// Killed outright: its link and the host's standard input go, and nothing says goodbye.
    fn killed(mut self) -> KilledWindow {
        for cut in self.cuts.lock().expect("the cuts").iter() {
            cut.abort();
        }
        drop(self.stdin.take());
        self
    }

    /// The host this window started ended once the window that came back quit.
    fn ended_by_the_quit(mut self) {
        let deadline = Instant::now() + PATIENCE;
        while !matches!(self.host.try_wait(), Ok(Some(_))) {
            assert!(Instant::now() < deadline, "the host outlived the quit");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// Whether the host this window started still runs.
    fn host_runs(&mut self) -> bool {
        matches!(self.host.try_wait(), Ok(None))
    }

    /// The host this window started, ended by the test and reaped.
    fn end_its_host(mut self) {
        self.host.kill().expect("the host is ended");
        self.host.wait().expect("the host is reaped");
    }
}

#[cfg(target_os = "linux")]
impl Drop for KilledWindow {
    fn drop(&mut self) {
        // However the test ends: CI's orphan check fails a run that leaves a host alive.
        if !matches!(self.host.try_wait(), Ok(Some(_))) {
            let _ = self.host.kill();
            let _ = self.host.wait();
        }
    }
}

mod the_host_refuses_a_start_while_stopped_and_holds_the_stop_across_its_restart {
    #[test]
    fn in_the_host() {
        super::the_host_refuses_a_start_while_stopped_and_holds_the_stop_across_its_restart(
            super::in_a_host_keeping_its_switch_in,
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn in_a_host_process() {
        super::the_host_refuses_a_start_while_stopped_and_holds_the_stop_across_its_restart(
            super::in_a_host_process_at,
        );
    }
}

/// **The session host killed outright** (#2007, ADR 0068 §3, FD-29's chaos test): with twenty
/// chats in it, the host's process is killed as `kill -9` kills it. Within thirty seconds every
/// chat has been told it ended with its host, and the window's supervisor has a new host
/// serving. What the window then says of each is read from its board as the chat went: the one
/// whose turn had ended, on a harness that resumes by id, is hibernated, and the rest, mid-turn,
/// failed. Each then starts again in the new host, within a bound of its own. Resume itself, the
/// chat's restart on its conversation, is the planes' test
/// (`a_chat_idle_when_its_session_host_ends_is_hibernated_and_resume_starts_it_on_its_conversation`).
/// A host process's target only: there is a process to kill. The test ends the host it started
/// (`Ends`), and the one it replaced ended its programs with its terminal.
#[cfg(target_os = "linux")]
#[test]
fn a_host_killed_outright_brings_twenty_chats_back_within_thirty_seconds() {
    const CHATS: usize = 20;
    let target = in_a_host_process();
    // Before any chat starts: each is told its end as it starts.
    let ends = told_of_ends(&target);
    // One chat whose turn has ended in a conversation of its own, as its harness's own hook
    // says.
    let idle = a_chat_on_the_board(
        &target,
        &a_shell(&format!(
            "printf '%s' '{{\"session_id\":\"0199a2f4-1c2b-7f00-8000-000000000007\",\
             \"hook_event_name\":\"Stop\"}}' | PURLIS_HARNESS=claude '{}' hook stop; \
             printf 'CHAT-IDLE-READY'; read line",
            purlis()
        )),
    );
    Pane::on(&target, idle).shows("CHAT-IDLE-READY");
    until("the idle chat's turn has ended in its conversation", || {
        let board = target.hooks.board();
        board.state(idle) == purlis_core::state::State::Waiting
            && board.conversation(idle).is_some()
    });
    let mut chats: Vec<u32> = vec![idle];
    chats.extend((1..CHATS).map(|n| {
        let id = target
            .sessions
            .open(
                None,
                &a_shell(&format!("printf 'CHAT-{n}-READY'; read line")),
                &|_| {},
            )
            .expect("it starts");
        Pane::on(&target, id).shows(&format!("CHAT-{n}-READY"));
        id
    }));
    let killed = Instant::now();

    (target.kill_the_host.as_ref().expect("a host process"))();

    let within = Duration::from_secs(30);
    let host = target.host.as_ref().expect("the host's target");
    while !(ends.lock().expect("the ends").len() == CHATS && host.wait(Duration::ZERO).is_ok()) {
        assert!(
            killed.elapsed() < within,
            "after {within:?}, {} of {CHATS} chats were told they ended, and the host serves: {}",
            ends.lock().expect("the ends").len(),
            host.wait(Duration::ZERO).is_ok()
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    let left_in = |chat: u32| {
        let (state, asking, has_a_conversation) = {
            let board = target.hooks.board();
            (
                board.state(chat),
                board.asking(chat),
                board.conversation(chat).is_some(),
            )
        };
        crate::hostlost::after_the_host(
            state,
            asking,
            purlis_core::harness::Harness::ClaudeCode.resumes_by_id(),
            has_a_conversation,
        )
    };
    for chat in &chats {
        assert!(
            target.sessions.lost_with_its_host(*chat),
            "chat {chat} did not end with its host"
        );
    }
    assert_eq!(left_in(idle), Some(crate::hostlost::LeftIn::Hibernated));
    for chat in &chats[1..] {
        assert_eq!(
            left_in(*chat),
            Some(crate::hostlost::LeftIn::Failed),
            "{chat}"
        );
    }
    // Each starts again, in the host that took the old one's place.
    let reopened = Instant::now();
    for (n, chat) in chats.iter().enumerate() {
        let again = target
            .sessions
            .open(
                None,
                &a_shell(&format!("printf 'CHAT-{n}-BACK'; read line")),
                &|_| {},
            )
            .expect("it starts again");
        Pane::on(&target, again).shows(&format!("CHAT-{n}-BACK"));
        target.sessions.close(*chat).expect("the lost one closes");
        target.sessions.close(again).expect("it closes");
    }
    assert!(
        reopened.elapsed() < Duration::from_secs(60),
        "twenty chats took {:?} to start again",
        reopened.elapsed()
    );
}

/// How many `hook.stop` lines the log of the machine chat `id` ran on holds, the chat closed.
///
/// A hook that hears nothing from its host within a quarter of a second spools the line as well
/// (ADR 0075 §7, #952), and a busy machine can take that long, so a copy is no fault. And a host
/// that reads the line once its hook has gone refuses it, so only the copy is left. A chat's end
/// drains its spool, so the count comes after that, the one moment that holds on every path and
/// the only one a second writing could show in. That a host in time leaves no copy is
/// `a_line_the_host_takes_is_answered_only_once_its_hearer_has_recorded_it`.
fn its_hook_lines_once_it_is_closed(target: &Target, id: u32) -> usize {
    target.sessions.close(id).expect("it closes");
    let stops = || {
        target
            .events_where_its_chats_run()
            .iter()
            .filter(|event| event.kind == "hook.stop")
            .count()
    };
    until("the hook's line is in the event log", || stops() >= 1);
    until("the copy of the line in its spool is drained", || {
        !target.spool_holds_lines_of(id)
    });
    stops()
}

/// **The event log has one writer** (#2003): what a chat's hook says is written once, by the
/// window where the chat runs in the window and by the host where it runs there, and while
/// chats run in the host the window holds no log of its own. On a pool's runner, the runner's
/// host writes it in the runner's own log, under the project as the runner holds it, and this
/// machine's log holds none of it (ADR 0078 §7, #3304).
fn the_event_log_has_one_writer_and_each_hook_line_is_written_once(target: &Target) {
    let id = a_chat_on_the_board(
        target,
        &a_shell(&format!(
            "PURLIS_HARNESS=claude '{}' hook stop </dev/null; printf 'reported.'; read line",
            purlis()
        )),
    );
    Pane::on(target, id).shows("reported.");
    let stops = its_hook_lines_once_it_is_closed(target, id);
    assert_eq!(stops, 1, "one line, written once");
    // Nothing its host heard failed to reach the log.
    let not_recorded = target.hook_lines_its_host_did_not_record();
    assert!(not_recorded.is_empty(), "{not_recorded:?}");
    assert!(
        target.its_log_has_its_writer(),
        "its one writer holds it, and no second is let in"
    );
    if target.runner.is_some() {
        // The desktop never copies a runner's events into its own.
        let here = purlis_core::eventlog::read(&target.log).expect("this machine's log reads");
        assert!(
            !here.iter().any(|event| event.kind == "hook.stop"),
            "{here:?}"
        );
    }
}

/// **A chat whose program ends with the window gone still has its end recorded, and its
/// dispatch report written** (#2003): the window lets go of its project and the host runs on;
/// the task's program ends by itself; its sub-agent's run ends with its own in the event log,
/// and the dispatch it worked ends as one that went without its report, owed to its asking
/// chat. The host's alone: the window's own process ends with its window.
#[test]
fn a_chat_ending_with_the_window_gone_still_writes_its_record_and_its_dispatch_report() {
    use purlis_core::dispatchrecord;
    use purlis_core::eventlog::{Attributes, Began, Entry};
    let target = in_the_host();
    let root = target.root.clone();
    let host = target.host.as_ref().expect("the host's target");
    let task = target
        .sessions
        .open(
            None,
            &a_shell("printf 'TASK-READY'; read line; exit 3"),
            &|_| {},
        )
        .expect("it starts");
    let pane = Pane::on(&target, task);
    pane.shows("TASK-READY");
    // The task's run, and a sub-agent of it, as the window tells the host's log.
    let (chat, run) = ("01K6W0RKER000000000000000B", "01K6W0RKRUN00000000000000B");
    let record = |entry: Entry| {
        crate::hostedsessions::record_in_the_host(host, &entry).expect("the host records it");
    };
    record(Entry::Begin {
        plane: root.clone(),
        number: task,
        chat: chat.into(),
        run: run.into(),
        cause: Began::Start,
        attributes: Attributes::default(),
        held: false,
        trust: None,
    });
    record(Entry::Tool {
        plane: root.clone(),
        call: purlis_core::hookwire::ToolCall {
            chat: task,
            tool_hook: "pretooluse".into(),
            tool: Some("Bash".into()),
            call: Some("toolu_01".into()),
            args: None,
            decision: purlis_core::hookwire::Decision::None,
            rule: None,
            hook_ms: 1,
            agent: Some("agent-1".into()),
            at_ms: 0,
            skill: None,
        },
    });
    let dispatched = dispatchrecord::open(
        &root,
        dispatchrecord::Opening {
            mode: dispatchrecord::Mode::Task,
            asker: dispatchrecord::Asker {
                chat: dispatchrecord::ChatRef {
                    chat: 99,
                    id: Some("01K6ASKER0000000000000000A".into()),
                    name: "steward 99".into(),
                    persona: Some("steward".into()),
                },
                workspace: None,
                by_person: false,
                session_record: None,
            },
            persona: Some("devops".into()),
            worker: dispatchrecord::Worker {
                chat: dispatchrecord::ChatRef {
                    chat: task,
                    id: Some(chat.into()),
                    name: "look at the queue".into(),
                    persona: Some("devops".into()),
                },
                harness: None,
                profile: None,
                session_record: None,
            },
            task: Some("look at the queue".into()),
            place: dispatchrecord::Place {
                workspace: None,
                folder: None,
                worktree: None,
            },
            brief: "Look at the queue.".into(),
            report_owed: true,
        },
        chrono::Utc::now(),
    )
    .expect("a running dispatch");

    // The window goes: its holds on the project go with it, and the host runs on. The project
    // is held on its root's own link (#2178).
    let link = host.of_project(&root, PATIENCE).expect("the root's link");
    let project = purlis_core::scaffold::planefile::id_to_hold_by(&root).expect("its id");
    link.call(session::Command::LetGoOfProject { project })
        .expect("let go of");
    // The task's program ends by itself.
    target.sessions.input(task, b"\n").expect("typed");

    until("the sub-agent's run ends with its own", || {
        purlis_core::eventlog::read(&target.log)
            .expect("the log reads")
            .iter()
            .any(|event| {
                event.kind == "run.ended"
                    && event.parent_run.as_deref() == Some(run)
                    && event.body["state"] == "failed"
                    && event.body["cause"] == "exited"
            })
    });
    until("its dispatch ends without its report", || {
        dispatchrecord::read(&root, &dispatched.id).is_some_and(|record| !record.running())
    });
    let ended = dispatchrecord::read(&root, &dispatched.id).expect("the record");
    assert_eq!(ended.ended_by, Some(dispatchrecord::EndedBy::Unreported));
    assert_eq!(
        ended.report.map(|report| report.text),
        Some(dispatchrecord::ENDED_WITHOUT_A_REPORT.to_owned())
    );
    assert!(ended.undelivered.is_some(), "owed to the chat that asked");
}

/// Each behaviour, as a test of its own on every target. Each group after `not yet through a
/// link` runs on the window's and the host's targets, and through a link and in a Docker pool
/// (whose runner is reached through a link) waits for what its reason names (#2042). Each
/// group after `not yet in a Docker pool` runs on every target but a Docker pool's, which waits
/// for what its reason names (#2045).
macro_rules! on_every_target {
    (@one $behaviour:ident, through_a_link: [$($link:meta)*], in_a_docker_pool: [$($docker:meta)*]) => {
        mod $behaviour {
            #[test]
            fn in_the_window() {
                super::$behaviour(&super::in_the_window());
            }

            #[test]
            fn in_the_host() {
                super::$behaviour(&super::in_the_host());
            }

            #[cfg(target_os = "linux")]
            #[test]
            fn in_a_host_process() {
                super::$behaviour(&super::in_a_host_process());
            }

            #[test]
            $(#[$link])*
            fn through_a_link() {
                super::$behaviour(&super::through_a_link());
            }

            /// Run by the parity job, which names a runner image; elsewhere it runs nothing.
            #[test]
            $(#[$docker])*
            fn in_a_docker_pool() {
                if let Some(target) = super::in_a_docker_pool() {
                    super::$behaviour(&target);
                }
            }
        }
    };
    (
        $($behaviour:ident),* $(,)?;
        $(not yet through a link ($why:literal): $($waiting:ident),+ $(,)?;)*
        $(not yet in a Docker pool ($docker_why:literal): $($docker_waiting:ident),+ $(,)?;)*
    ) => {
        $(
            on_every_target!(@one $behaviour, through_a_link: [], in_a_docker_pool: []);
        )*
        $($(
            on_every_target!(@one $waiting, through_a_link: [ignore = $why], in_a_docker_pool: [ignore = $why]);
        )+)*
        $($(
            on_every_target!(@one $docker_waiting, through_a_link: [], in_a_docker_pool: [ignore = $docker_why]);
        )+)*
    };
}

on_every_target!(
    a_chat_starts_and_its_tab_draws_what_it_wrote,
    a_chat_named_alone_is_composed_by_the_host_as_its_profile_says,
    a_harness_chat_started_by_its_names_is_viewed_typed_into_answered_and_stopped,
    what_its_tab_types_reaches_it,
    a_resize_reaches_its_terminal,
    a_chat_is_told_its_own_number,
    a_program_that_ends_is_told_with_its_code_and_its_tab_stays,
    stopping_every_program_ends_each_and_keeps_their_tabs,
    a_closed_chat_is_ended_and_gone,
    a_program_reading_a_line_is_not_yet_ready_to_be_typed_at_key_by_key,
    a_chat_starts_with_the_path_and_variables_the_window_gives_it,
    a_chat_whose_turn_ends_needs_you_on_every_harness,
    a_permission_ask_is_answered_from_the_window_on_every_harness,
    the_event_log_has_one_writer_and_each_hook_line_is_written_once,
    a_write_in_the_project_is_saved_after_its_quiet_period_and_the_window_is_told,
    exactly_one_saver_runs_for_the_project_and_none_once_it_is_stopped,
    a_hook_refuses_a_listener_that_is_not_its_chats_ancestor,
    a_sandboxed_chats_proxy_carries_what_it_lists_and_refuses_the_rest,
    a_port_withdrawn_while_a_chat_runs_is_carried_no_longer,
    an_ask_raised_by_a_chats_proxy_is_answered_from_the_window;
    not yet through a link ("an opt-out is asked again on the device a chat moves to, ADR 0078 §4"):
    an_opt_out_of_a_chat_the_host_composes_is_in_its_event_log_before_it_starts;
    not yet through a link ("the window's re-arm and its shell while stopped cross a link in #2220"):
    stop_all_ends_every_chat_and_none_starts_until_the_window_re_arms,
    while_stopped_the_operators_shell_opens;
);

/// **A sandboxed runner chat reaches neither its runner's browser engine nor its host** (#3338,
/// spec #2041): on a Docker pool's runner, the chat, the session host and the browser engine
/// share one container. A Codex chat the runner's own host composes, under purlis's Linux
/// sandbox as every sandboxed runner chat is, tries what would reach past it from in there, with
/// a browser of its project running beside it: the engine's data folder (the cookies and
/// storage of every site it opened), the engine's memory, the pipe the host drives the engine
/// over, the runner's link key in the host's own folder, the host's socket's folder, and a
/// namespace of its own, which only the engine may make there (D-2050-25). Each is refused;
/// reading its project is not.
///
/// The paths and process ids it tries are found first, as the runner's own user, outside the
/// sandbox, and each must be found, so no try passes by naming nothing: the engine is one whose
/// environment that user reads there, and the pipe one that user opens there, so each refusal
/// inside is the sandbox's. (The engine's memory is refused outside too where Yama holds
/// `ptrace` to a process's own descendants, so its environment, which Yama does not hold, is
/// tried beside it.)
fn a_sandboxed_runner_chat_reaches_neither_its_browser_engine_nor_its_host(target: &Target) {
    let Some(runner) = &target.runner else {
        return;
    };
    let (browsers, browser, past) = a_browser_and_what_lies_past_a_chat(target, runner);

    // The chat: Codex as the runner composes it, its program a stand-in that tries each in a
    // shell of its own and says what came of it.
    let bin = tempfile::tempdir().expect("a folder for the harness");
    let program = stand_in::program(
        bin.path(),
        "codex",
        &format!(
            "#!/bin/sh\n\
             [ \"$1\" = login ] && [ \"$2\" = status ] && exit 0\n\
             for word in \"$@\"; do [ \"$word\" = --version ] && {{ echo 'codex-cli 0.147.0'; exit 0; }}; done\n\
             {tries}\
             printf 'TRIED.\\n'\n\
             read line\n",
            tries = past.tries(),
        ),
    );
    a_profile_of(target, "codex", "work", &program);
    let id = target
        .sessions
        .open_named(
            None,
            &named("try what reaches past you", false),
            purlis_core::sandbox::plan::Hearing::default(),
            &|_| {},
        )
        .expect("the sandboxed chat starts on the runner")
        .number;
    let pane = Pane::on(target, id);
    pane.shows("TRIED.");
    for said in PAST_A_CHAT {
        pane.shows(said);
    }
    target.sessions.close(id).expect("it closes");
    browsers.close(&browser).expect("the browser ends");
}

/// **A real Claude Code chat on a runner is held at least as a Codex chat is** (#4083, ruled
/// 2026-10-30): the program Claude Code ships, armed by the runner's own host with purlis's
/// plugin, its own sandbox off and purlis's wrap around all of it, in a container with neither
/// bubblewrap nor socat. A stand-in model inside the chat asks its Bash tool for what
/// [`a_sandboxed_runner_chat_reaches_neither_its_browser_engine_nor_its_host`] tries, and for
/// what Claude Code's own sandbox used to keep from a command: its settings, its hooks and
/// `.claude.json`, and a connection past the proxy. Its hook reaches the host.
fn a_real_claude_code_chat_on_a_runner_is_held_as_a_codex_chat_is(target: &Target) {
    let Some(runner) = &target.runner else {
        return;
    };
    let outside = runner.prints("command -v bwrap socat || echo neither");
    assert_eq!(outside.trim(), "neither", "the image stays without them");
    let (browsers, browser, past) = a_browser_and_what_lies_past_a_chat(target, runner);
    let bin = tempfile::tempdir().expect("a folder for the harness");
    let program = stand_in::program(
        bin.path(),
        "claude",
        &format!(
            "#!/bin/sh\n\
             real={REAL_CLAUDE_CODE}\n\
             for word in \"$@\"; do [ \"$word\" = --version ] && exec \"$real\" --version; done\n\
             cat > \"$TMPDIR/probes.sh\" <<'PROBES'\n\
             {tries}\
             try TURN 'touch \"$CLAUDE_CONFIG_DIR/projects/turn\"'\n\
             try SETTINGS 'echo x >> \"$CLAUDE_CONFIG_DIR/settings.json\"'\n\
             try STATE 'echo x >> \"$CLAUDE_CONFIG_DIR/.claude.json\"'\n\
             try HOOKS 'mkdir \"$CLAUDE_CONFIG_DIR/hooks\"'\n\
             try DIRECT 'bash -c \"exec 3<>/dev/tcp/1.1.1.1/443\"'\n\
             PROBES\n\
             '{fake}' --model \"sh $TMPDIR/probes.sh\" --model-said \"$TMPDIR/said\" > \"$TMPDIR/port\" &\n\
             until [ -s \"$TMPDIR/port\" ]; do sleep 0.1; done\n\
             export ANTHROPIC_BASE_URL=\"http://127.0.0.1:$(cat \"$TMPDIR/port\")\"\n\
             export ANTHROPIC_AUTH_TOKEN=stand-in NO_PROXY=127.0.0.1 no_proxy=127.0.0.1\n\
             \"$real\" -p \"$@\" --permission-mode default --output-format json > \"$TMPDIR/out\" 2> \"$TMPDIR/err\" < /dev/null\n\
             printf 'CLAUDE-CODE ended %s.\\n' \"$?\"\n\
             cat \"$TMPDIR/said\" \"$TMPDIR/err\"; ls -A \"$CLAUDE_CONFIG_DIR\"\n\
             printf '\\nTRIED.\\n'\n\
             read line\n",
            tries = past.tries(),
            fake = fake_harness().display(),
        ),
    );
    a_profile_of(target, "claude", "work", &program);
    let chat_says = |direct: &str| {
        let id = target
            .sessions
            .open_named(
                None,
                &named("Run the probes.", false),
                purlis_core::sandbox::plan::Hearing::default(),
                &|_| {},
            )
            .expect("the sandboxed chat starts on the runner")
            .number;
        let pane = Pane::on(target, id);
        pane.shows("TRIED.");
        pane.shows("CLAUDE-CODE ended 0.");
        for said in PAST_A_CHAT {
            pane.shows(said);
        }
        for said in [
            "TURN yes.",
            "SETTINGS no.",
            "STATE no.",
            "HOOKS no.",
            direct,
        ] {
            pane.shows(said);
        }
        // Its plugin's hook ran inside the wrap and reached the host over the hook socket.
        until("its hook's line is in the event log", || {
            target
                .events_where_its_chats_run()
                .iter()
                .any(|event| event.kind == "hook.stop")
        });
        target.sessions.close(id).expect("it closes");
    };
    // At the floor a chat reaches other machines as it reaches them with no sandbox, as a
    // Codex chat there does; at the full sandbox it reaches them only through its proxy.
    chat_says("DIRECT yes.");
    runner.runs(
        false,
        &format!(
            "printf '\\n[sandbox]\\nmode = \"on\"\\negress = [\"model-providers\"]\\n' >> '{}/purlis.toml'",
            target.project_on_its_runner(runner)
        ),
        b"",
    );
    chat_says("DIRECT no.");
    browsers.close(&browser).expect("the browser ends");
}

/// Where the parity image keeps the program Claude Code ships (`tests/parity/runner.Dockerfile`).
const REAL_CLAUDE_CODE: &str = "/usr/local/lib/parity/claude-code";

/// What a chat says of [`PastAChat::tries`]: its project reached, and nothing past it.
const PAST_A_CHAT: [&str; 9] = [
    "PROJECT yes.",
    "ENGINE-FOLDER no.",
    "ENGINE-MEMORY no.",
    "ENGINE-ENVIRONMENT no.",
    "HOST-PIPE no.",
    "HOST-KEY no.",
    "HOST-SOCKET no.",
    // Only the browser engine may make namespaces on a runner (D-2050-25), refused by the
    // filter, not for want of a program.
    "NAMESPACE no:",
    "Operation not permitted",
];

/// What lies past a sandboxed chat on a runner, each found as the runner's own user, outside the
/// sandbox ([`a_browser_and_what_lies_past_a_chat`]).
struct PastAChat {
    folder: String,
    engine: String,
    host: String,
    pipe: String,
    socket: String,
    key: String,
}

impl PastAChat {
    /// A shell's `try` and what it tries of each, saying what came of it.
    fn tries(&self) -> String {
        let Self {
            folder,
            engine,
            host,
            pipe,
            socket,
            key,
        } = self;
        format!(
            "try() {{ if ( eval \"$2\" ) >/dev/null 2>&1; then printf '%s yes.\\n' \"$1\"; else printf '%s no.\\n' \"$1\"; fi; }}\n\
             try PROJECT 'ls .'\n\
             try ENGINE-FOLDER 'ls {folder}'\n\
             try ENGINE-MEMORY 'exec 3< /proc/{engine}/mem'\n\
             try ENGINE-ENVIRONMENT 'cat /proc/{engine}/environ'\n\
             try HOST-PIPE 'exec 3<> /proc/{host}/fd/{pipe}'\n\
             try HOST-KEY 'cat {key}'\n\
             try HOST-SOCKET 'ls {socket}'\n\
             if said=$(unshare --user true 2>&1); then printf 'NAMESPACE yes.\\n'; else printf 'NAMESPACE no: %s\\n' \"$said\"; fi\n"
        )
    }
}

/// A browser of the project running on `runner`, and what lies past a chat beside it, each found
/// outside the sandbox.
fn a_browser_and_what_lies_past_a_chat(
    target: &Target,
    runner: &OnTheRunner,
) -> (crate::browsers::WindowBrowsers, String, PastAChat) {
    use crate::chatbrowsers::Driver as _;
    let browsers = crate::browsers::WindowBrowsers::new(Arc::clone(
        target.host.as_ref().expect("the window's link to its host"),
    ));
    let browser = browsers
        .start(
            &target.root,
            Some(&runner.pool),
            purlis_session_protocol::browsing::BrowserReach::default(),
            None,
        )
        .unwrap_or_else(|why| panic!("a browser starts on the runner: {why:?}"));

    let found = runner.prints(&format!(
        r#"set -e
engine=; host=
for status in /proc/[0-9]*/status; do
  pid=${{status#/proc/}}; pid=${{pid%/status}}
  name=$(sed -n 's/^Name:[[:space:]]*//p' "$status" 2>/dev/null || true)
  case "$name" in
    chrome) [ -n "$engine" ] || ! cat /proc/$pid/environ >/dev/null 2>&1 || engine=$pid ;;
    purlis) tr '\0' ' ' < /proc/$pid/cmdline 2>/dev/null | grep -q ' serve' && host=$pid || true ;;
  esac
done
folder=$(ls -d "$HOME"/.local/share/*/{browsers}/*/ 2>/dev/null | head -n 1)
pipe=
for fd in $(ls -l /proc/$host/fd | sed -n 's/.* \([0-9][0-9]*\) -> pipe:.*/\1/p'); do
  [ "$fd" -ge 3 ] && [ -z "$pipe" ] && (exec 3<> /proc/$host/fd/$fd) 2>/dev/null && pipe=$fd || true
done
socket=$(ls -d /tmp/purlis-$(id -u) | head -n 1)
key=$(ls -d "$HOME"/.config/*/{daemon}/{key} "$HOME"/.config/*/{daemon_before}/{key} 2>/dev/null | head -n 1)
printf 'folder=%s\nengine=%s\nhost=%s\npipe=%s\nsocket=%s\nkey=%s\n' "$folder" "$engine" "$host" "$pipe" "$socket" "$key"
"#,
        daemon = purlis_core::names::DAEMON_DIR
            .spellings()
            .next()
            .expect("its name"),
        daemon_before = purlis_core::names::DAEMON_DIR
            .spellings()
            .last()
            .expect("its name"),
        key = purlis_core::runnerlink::KEY_FILE,
        browsers = purlis_core::datahome::BROWSERS,
    ));
    let value = |name: &str| {
        let value = found
            .lines()
            .find_map(|line| line.strip_prefix(&format!("{name}=")))
            .unwrap_or_default()
            .trim()
            .to_owned();
        assert!(!value.is_empty(), "the runner has no {name}: {found}");
        value
    };
    let (folder, engine, host, pipe, socket, key) = (
        value("folder"),
        value("engine"),
        value("host"),
        value("pipe"),
        value("socket"),
        value("key"),
    );

    // Outside the chat, the runner's user makes a user namespace: what the chat is refused is
    // the filter's refusal, not the runner's.
    let outside = runner.prints("unshare --user true 2>&1 && echo yes || echo no");
    assert_eq!(
        outside.trim(),
        "yes",
        "the runner refuses a user namespace to anyone: {outside}"
    );
    (
        browsers,
        browser,
        PastAChat {
            folder,
            engine,
            host,
            pipe,
            socket,
            key,
        },
    )
}

/// **A chat without the sandbox on a runner makes no namespace, and leaves its runner trusted with
/// no browser** (#3298, D-2050-25, D-2050-27): it runs under the filter the runner's host starts
/// every program with, and once it has run, the window starts no browser on that runner until
/// it is made again, in words that say why.
fn a_chat_without_the_sandbox_on_a_runner_makes_no_namespace_and_leaves_it_no_browser(
    target: &Target,
) {
    use crate::chatbrowsers::Driver as _;
    let Some(runner) = &target.runner else {
        return;
    };
    let outside = runner.prints("unshare --user true 2>&1 && echo yes || echo no");
    assert_eq!(
        outside.trim(),
        "yes",
        "the runner refuses a user namespace to anyone: {outside}"
    );
    let id = target
        .sessions
        .open(
            None,
            &a_shell("unshare --user true; printf 'CODE=%s.' $?; read line"),
            &|_| {},
        )
        .expect("a chat without the sandbox starts on the runner");
    let pane = Pane::on(target, id);
    pane.shows("Operation not permitted");
    pane.shows("CODE=1.");
    target.sessions.close(id).expect("it closes");

    let browsers = crate::browsers::WindowBrowsers::new(Arc::clone(
        target.host.as_ref().expect("the window's link to its host"),
    ));
    let refused = browsers
        .start(
            &target.root,
            Some(&runner.pool),
            purlis_session_protocol::browsing::BrowserReach::default(),
            None,
        )
        .expect_err("no browser starts on that runner");
    let said = format!("{refused:?}");
    assert!(said.contains("without the sandbox"), "{said}");
    assert!(said.contains("add it again"), "{said}");
}

/// **The person's own shell on a runner makes no namespace either** (D-2050-25), and leaves the
/// runner trusted with browsers: a shell is the person's, not a chat's.
fn the_persons_shell_on_a_runner_makes_no_namespace(target: &Target) {
    let Some(runner) = &target.runner else {
        return;
    };
    let outside = runner.prints("unshare --user true 2>&1 && echo yes || echo no");
    assert_eq!(
        outside.trim(),
        "yes",
        "the runner refuses a user namespace to anyone: {outside}"
    );
    let id = target
        .sessions
        .open(
            None,
            &Opening {
                operator_shell: true,
                ..a_shell("unshare --user true; printf 'CODE=%s.' $?; read line")
            },
            &|_| {},
        )
        .expect("the person's shell starts on the runner");
    let pane = Pane::on(target, id);
    pane.shows("Operation not permitted");
    pane.shows("CODE=1.");
    target.sessions.close(id).expect("it closes");
}

mod a_chat_without_the_sandbox_on_a_runner_makes_no_namespace_and_leaves_it_no_browser {
    /// Run by the parity job, which names a runner image; elsewhere it runs nothing.
    #[test]
    fn in_a_docker_pool() {
        if let Some(target) = super::in_a_docker_pool() {
            super::a_chat_without_the_sandbox_on_a_runner_makes_no_namespace_and_leaves_it_no_browser(
                &target,
            );
        }
    }
}

/// **A hook line its runner's host was too slow for is written once** (ADR 0075 §7, #952): the
/// host stopped while the hook runs, so the hook gives up on it, spools the line and ends. Started
/// again, the host refuses what it was sent by a process now gone, and the copy is what reaches
/// the log, once. A host and a copy that both record it is `hookwire`'s
/// `a_line_the_host_recorded_late_and_its_hook_spooled_is_recorded_once`.
fn a_hook_line_its_runners_host_was_too_slow_for_is_written_once(target: &Target) {
    let runner = target.runner.as_ref().expect("a runner's target");
    let project = target.project_on_its_runner(runner);
    let (go, told) = (
        format!("{project}/.hook-go"),
        format!("{project}/.hook-told"),
    );
    let id = a_chat_on_the_board(
        target,
        &a_shell(&format!(
            "printf 'waiting.'; until [ -e '{go}' ]; do sleep 0.05; done; \
             PURLIS_HARNESS=claude '{}' hook stop </dev/null; : > '{told}'; read line",
            purlis()
        )),
    );
    Pane::on(target, id).shows("waiting.");
    let a_copy_waits = {
        let _stopped = runner.its_host_stopped();
        runner.prints(&format!(": > '{go}'"));
        until("the hook gives up on its stopped host", || {
            runner.path(&told).is_some()
        });
        target.spool_holds_lines_of(id)
    };
    assert!(
        a_copy_waits,
        "the hook spooled the line its host did not take"
    );

    assert_eq!(
        its_hook_lines_once_it_is_closed(target, id),
        1,
        "written once"
    );
}

mod a_hook_line_its_runners_host_was_too_slow_for_is_written_once {
    /// Run by the parity job, which names a runner image; elsewhere it runs nothing.
    #[test]
    fn in_a_docker_pool() {
        if let Some(target) = super::in_a_docker_pool() {
            super::a_hook_line_its_runners_host_was_too_slow_for_is_written_once(&target);
        }
    }
}

mod the_persons_shell_on_a_runner_makes_no_namespace {
    /// Run by the parity job, which names a runner image; elsewhere it runs nothing.
    #[test]
    fn in_a_docker_pool() {
        if let Some(target) = super::in_a_docker_pool() {
            super::the_persons_shell_on_a_runner_makes_no_namespace(&target);
        }
    }
}

mod a_real_claude_code_chat_on_a_runner_is_held_as_a_codex_chat_is {
    /// Run by the parity job, which names a runner image; elsewhere it runs nothing.
    #[test]
    fn in_a_docker_pool() {
        if let Some(target) = super::in_a_docker_pool() {
            super::a_real_claude_code_chat_on_a_runner_is_held_as_a_codex_chat_is(&target);
        }
    }
}

mod a_sandboxed_runner_chat_reaches_neither_its_browser_engine_nor_its_host {
    /// Run by the parity job, which names a runner image; elsewhere it runs nothing: on this
    /// machine the chat, the host and the engine are not in one container.
    #[test]
    fn in_a_docker_pool() {
        if let Some(target) = super::in_a_docker_pool() {
            super::a_sandboxed_runner_chat_reaches_neither_its_browser_engine_nor_its_host(&target);
        }
    }
}

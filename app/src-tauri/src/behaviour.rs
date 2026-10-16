// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The behaviour suite: what a chat does, written once and run against every place a chat can
//! run (spec #1974, #1998). It is the gate each later step of moving chats into the session
//! host passes: a behaviour is added here once, and every target runs it.
//!
//! **The targets are the two ways the start-time switch runs a chat** (`crate::hostedsessions`):
//! in the window's own process ([`crate::sessions::Sessions`]), and in a session host the window
//! speaks the session protocol to ([`crate::hostedsessions::HostedSessions`], here over an
//! in-memory link to the host's own code, with real terminals); **and a third, through a link**:
//! the window's host relaying to another host over the link between hosts
//! ([`through_a_link`], #2042). Each behaviour speaks only what the window speaks to any of
//! them, [`SessionHost`], and checks only what the window can observe:
//! what its pane draws, what it is told, what ends. When the in-app path is deleted (#2005), its
//! target goes with it, and the host's stays; Docker and a cluster join as targets of their own
//! (spec #2041).
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
//! (`crates/host/tests/the_kill_switch_is_held_by_the_host.rs`).
//!
//! **The event log has one writer on each target** (#2003): the window, holding it in a data
//! home of the test's, or the host, holding it there while the window hands it each write and
//! never opens it.
//!
//! **To add a behaviour:** write `fn the_behaviour(target: &Target)`, and name it in the
//! [`on_every_target!`] list at the bottom.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use purlis_core::engine::Size;
use purlis_core::sandbox::hosts::Host;
use purlis_core::sandbox::plan::{self, Asking, Hearing, Plan};
use purlis_core::sandbox::reach::{By, Reach};
use purlis_core::sandbox::{Confinement, egress};
use purlis_core::session::Exit;
use purlis_session_protocol::{link, session};

use crate::hooks::{Hooks, Moved};
use crate::host::{Opening, SessionHost, Sink};
use crate::hostedsessions::{Connection, HostedSessions};
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
    /// Where the device's event log is: the window's, or that of the host its chats run in.
    pub log: PathBuf,
    /// The window's link to the host, on a host's target: what the host is asked directly.
    pub host: Option<Arc<Connection>>,
    /// What keeps the target running for as long as the test does.
    _keeps: Vec<Box<dyn std::any::Any>>,
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

/// A chat in the window's own process: the switch off, the default.
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
        log: data.path().to_path_buf(),
        host: None,
        _keeps: vec![Box::new(project), Box::new(config), Box::new(data)],
    }
}

/// A chat in the session host: the switch on. The host's own code, serving the session protocol
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
    let mut target = a_window_on(log.clone(), move || a_host(&folder, &log));
    target._keeps.push(Box::new(data));
    target
}

/// A chat on another session host, reached through this machine's (#2042): the window speaks to
/// host A, which relays to host B over the link between hosts, as the desktop's host reaches a
/// runner's (ADR 0078 §4). Host B is the host's own code with real terminals, composed as the
/// window's binary composes them; both hosts are on this machine, linked in memory over the
/// encrypted link, each end's link key pinned by the other. Host B takes the window's start
/// and its replies because host A relays them from the window's own link (ruled 2026-10-15).
pub(crate) fn through_a_link() -> Target {
    let folder = tempfile::tempdir().expect("host B's folder");
    // Host B runs the chats, so it writes their event log.
    let data = tempfile::tempdir().expect("host B's data home");
    let host_b = a_host(folder.path(), data.path());
    let mut target = a_window_on(data.path().to_path_buf(), move || {
        purlis_host::Relay::new(Arc::new(move || {
            let host_b = host_b.clone();
            Box::pin(async move { Ok(linked_to(host_b).await) })
        }))
    });
    target._keeps.push(Box::new(folder));
    target._keeps.push(Box::new(data));
    target
}

/// A session host as the window's binary composes it, keeping its kill switch in `folder`: its
/// starts composed from the window's launch, each in a real terminal, its chats' hooks heard on
/// their project's channel, each confined here, and the device's event log kept in `data`,
/// opened as it first holds a project (#2003).
fn a_host(folder: &Path, data: &Path) -> purlis_host::Hosting {
    let kept = {
        let data = data.to_path_buf();
        crate::sessionhost::Kept::opening(move || an_event_log(&data))
    };
    purlis_host::Hosting::kept(
        folder,
        purlis_host::FromTheWindow(crate::sessionhost::launched),
        purlis_host::Terminals,
        crate::sessionhost::HookChannels(kept.clone()),
        crate::hostedconfinement::ConfinesHere,
    )
    .recording_with(kept)
    .saving_with(crate::hostsaving::InTheHost)
}

/// A link from host A to `host_b`, over the link between hosts on an in-memory stream.
async fn linked_to(host_b: purlis_host::Hosting) -> link::Link {
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
    host: impl FnOnce() -> H + Send + 'static,
) -> Target {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("a runtime");
    let config = tempfile::tempdir().expect("a config home");
    // Made on the runtime, as a host is made where it serves.
    let host = runtime.block_on(async move { host() });
    let (connection, _window) = crate::hostedsessions::tests::in_memory(host, &runtime);
    let (project, root) = a_project();
    let (hooks, moved) = hooks_of(&root, true);
    hooks.record_into(crate::hooks::Events::in_the_host({
        let connection = Arc::clone(&connection);
        move |entry| crate::hostedsessions::record_in_the_host(&connection, &entry)
    }));
    let switch = KillSwitch::kept_in(Some(config.path().to_path_buf()));
    switch.held_by(Arc::clone(&connection) as Arc<dyn crate::killswitch::Authority>);
    let mut sessions = HostedSessions::new(Arc::clone(&connection), &root);
    sessions.hear_through(hooks.relayed_from_the_host().expect("heard from the host"));
    sessions.stopped_by(Arc::clone(&switch));
    Target {
        sessions: Box::new(sessions),
        hooks,
        moved,
        root,
        switch,
        log,
        host: Some(connection),
        _keeps: vec![Box::new(project), Box::new(config), Box::new(runtime)],
    }
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
fn built_beside(name: &str, package: &str) -> PathBuf {
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
        cwd: None,
        size: SIZE,
        env: Vec::new(),
        env_strip: Vec::new(),
        harness: None,
        declared_env: Vec::new(),
        env_pass: Vec::new(),
        operator_shell: false,
        git_hooks: None,
        handed: Default::default(),
        confinement: None,
    }
}

/// A pane: what its view is sent, as it is sent.
struct Pane {
    seen: Arc<Mutex<String>>,
}

impl Pane {
    fn on(target: &Target, id: u32) -> Pane {
        let seen = Arc::new(Mutex::new(String::new()));
        let sink: Sink = {
            let seen = Arc::clone(&seen);
            Box::new(move |text: String| seen.lock().expect("the pane").push_str(&text))
        };
        target.sessions.watch(id, sink).expect("a view opens");
        Pane { seen }
    }

    fn shows(&self, text: &str) {
        let deadline = Instant::now() + PATIENCE;
        while !self.seen.lock().expect("the pane").contains(text) {
            assert!(
                Instant::now() < deadline,
                "{text:?} was never drawn, only {:?}",
                self.seen.lock().expect("the pane")
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }
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
    until("its program is gone", || !purlis_core::process::alive(pid));
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
    // What the window's own composition gives a chat started with nothing of its own: every
    // target must give the same, whichever process starts it.
    let expected = purlis_core::chatspec::spec(
        std::env::vars_os(),
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
    let (path, home) = (value("PATH"), value("HOME"));
    assert!(!path.is_empty(), "a chat is given a PATH");
    let id = target
        .sessions
        .open(
            None,
            &a_shell("read line; printf 'P[%s]H[%s].' \"$PATH\" \"$HOME\"; read line"),
            &|_| {},
        )
        .expect("it starts");
    let pane = Pane::on(target, id);

    target.sessions.input(id, b"\r").expect("typed");

    pane.shows(&format!("P[{path}]H[{home}]."));
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
    let _saving = target.sessions.keep_saving(&target.root, told);

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
    let (told, _looks) = told_of_looks();
    let saving = target.sessions.keep_saving(&target.root, told);

    until("a saver runs", || {
        crate::autosave::running::on(&target.root) == 1
    });
    // Whatever was to start has had the time to.
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(
        crate::autosave::running::on(&target.root),
        1,
        "the window and the host both save the project"
    );

    drop(saving);

    until("the saver stops", || {
        crate::autosave::running::on(&target.root) == 0
    });
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

        pane.shows(prints);
        pane.shows("decided.");
        target.sessions.close(id).expect("it closes");
    }
}

/// Where [`a_listener_beside_the_chat`] binds, when this test binary is run as that helper.
const LISTEN_AT: &str = "PURLIS_BEHAVIOUR_LISTEN_AT";

/// Not a behaviour: this binary, run again as a child of the test, takes the hook socket's
/// path over with a listener of its own, which is beside every chat and below none, says so,
/// and waits to be killed.
#[test]
#[ignore = "a helper the behaviours start as a child"]
fn a_listener_beside_the_chat() {
    let Some(path) = std::env::var_os(LISTEN_AT) else {
        return;
    };
    let _ = std::fs::remove_file(&path);
    let _listener = std::os::unix::net::UnixListener::bind(&path).expect("bound");
    println!("up");
    std::thread::sleep(Duration::from_secs(60));
}

/// A listener at `socket` in a process of its own, beside the chats: ended as it is dropped.
struct Beside(std::process::Child);

impl Beside {
    fn listening_at(socket: &Path) -> Beside {
        let mut child = purlis_core::forklock::spawn(
            std::process::Command::new(std::env::current_exe().expect("this test"))
                .args([
                    "--exact",
                    "behaviour::a_listener_beside_the_chat",
                    "--ignored",
                    "--nocapture",
                ])
                .env(LISTEN_AT, socket)
                .stdout(std::process::Stdio::piped()),
        )
        .expect("the listener starts");
        let mut said = std::io::BufReader::new(child.stdout.take().expect("piped"));
        let mut line = String::new();
        while !line.contains("up") {
            line.clear();
            if std::io::BufRead::read_line(&mut said, &mut line).expect("it speaks") == 0 {
                break;
            }
        }
        Beside(child)
    }
}

impl Drop for Beside {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn a_hook_refuses_a_listener_that_is_not_its_chats_ancestor(target: &Target) {
    let payload = HOOKED_HARNESSES[0].payload;
    let id = a_chat_on_the_board(
        target,
        &a_shell(&format!(
            "read line; printf '%s' '{payload}' | '{}' hook permissionrequest; \
             printf 'left to its own prompt.'; read line",
            purlis()
        )),
    );
    let pane = Pane::on(target, id);
    let socket = target
        .hooks
        .socket()
        .expect("the project's hook socket")
        .to_path_buf();
    let _beside = Beside::listening_at(&socket);

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

/// A service on this machine, which answers nothing: what a chat may be given by its port.
fn a_service() -> (std::net::TcpListener, u16) {
    let service = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("a service");
    let port = service.local_addr().expect("its port").port();
    (service, port)
}

/// The status line a chat's proxy, on its HTTP `port`, answers a tunnel to `target` with.
fn tunnel_through(port: u16, target: &str) -> String {
    use std::io::{BufRead, Write};
    let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).expect("the proxy");
    stream
        .set_read_timeout(Some(Duration::from_secs(30)))
        .expect("a timeout");
    stream
        .write_all(format!("CONNECT {target} HTTP/1.1\r\nHost: {target}\r\n\r\n").as_bytes())
        .expect("sent");
    let mut line = String::new();
    std::io::BufReader::new(stream)
        .read_line(&mut line)
        .expect("a status line");
    line.trim_end().to_owned()
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

/// A chat that runs in `confinement`, as the window starts one.
fn in_it(target: &Target, confinement: &Confinement) -> u32 {
    let mut opening = a_shell("printf 'confined.'; read line");
    opening
        .env
        .push(("HTTPS_PROXY".into(), confinement.proxy_url()));
    opening.confinement = confinement.held_as().map(str::to_owned);
    let id = target
        .sessions
        .open(None, &opening, &|_| {})
        .expect("it starts");
    Pane::on(target, id).shows("confined.");
    id
}

fn a_sandboxed_chats_proxy_carries_what_it_lists_and_refuses_the_rest(target: &Target) {
    let (_service, port) = a_service();
    let heard = Arc::new(HeardOfIt::default());
    let confinement = target
        .sessions
        .confine(
            &a_plan(vec![(format!("127.0.0.1:{port}"), By::You)], vec![port]),
            hearing(&heard, false),
        )
        .expect("confined");
    let id = in_it(target, &confinement);

    assert_eq!(
        tunnel_through(confinement.proxy_port(), &format!("127.0.0.1:{port}")),
        "HTTP/1.1 200 Connection established"
    );
    assert_eq!(
        tunnel_through(confinement.proxy_port(), "refused.invalid:443"),
        "HTTP/1.1 403 Forbidden"
    );
    until("the window hears of the refusal", || {
        *heard.refused.lock().expect("heard") == ["refused.invalid:443"]
    });
    target.sessions.close(id).expect("it closes");
}

fn a_port_withdrawn_while_a_chat_runs_is_carried_no_longer(target: &Target) {
    let (_service, port) = a_service();
    let confinement = target
        .sessions
        .confine(
            &a_plan(vec![(format!("127.0.0.1:{port}"), By::You)], vec![port]),
            Hearing::default(),
        )
        .expect("confined");
    let id = in_it(target, &confinement);

    confinement.withdraw_port(port).expect("withdrawn");

    assert_eq!(
        tunnel_through(confinement.proxy_port(), &format!("127.0.0.1:{port}")),
        "HTTP/1.1 403 Forbidden"
    );
    target.sessions.close(id).expect("it closes");
}

fn an_ask_raised_by_a_chats_proxy_is_answered_from_the_window(target: &Target) {
    let heard = Arc::new(HeardOfIt::default());
    let confinement = target
        .sessions
        .confine(&a_plan(Vec::new(), Vec::new()), hearing(&heard, true))
        .expect("confined");
    let id = in_it(target, &confinement);
    let port = confinement.proxy_port();
    let held = std::thread::spawn(move || tunnel_through(port, "held.invalid:443"));
    until("the window is asked", || {
        *heard.asked.lock().expect("heard") == ["held.invalid:443"]
    });
    let board = confinement.asks().expect("its proxy asks live");
    assert!(board.asks_about(&Host::parse("held.invalid").expect("a host")));

    let retry = board.allow(&Host::parse("held.invalid").expect("a host"), By::Chat);

    // Carried: the proxy tried the host, which never resolves.
    assert_eq!(held.join().expect("answered"), "HTTP/1.1 502 Bad Gateway");
    assert!(retry.is_empty(), "nothing had given up");
    assert_eq!(
        board.allowed_live(),
        [("held.invalid".to_owned(), By::Chat)]
    );
    target.sessions.close(id).expect("it closes");
}

/// **The host's own target only**: the window's own process has no host to start again, and it
/// is the host that keeps the stop across its restart and refuses a start the window did not
/// stop first (#2001). The window here is one that never heard of the stop.
#[test]
fn the_host_refuses_a_start_while_stopped_and_holds_the_stop_across_its_restart() {
    let kept = tempfile::tempdir().expect("the host's folder");
    {
        let before = in_a_host_keeping_its_switch_in(kept.path());
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

    let after = in_a_host_keeping_its_switch_in(kept.path());
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

/// **The event log has one writer** (#2003): what a chat's hook says is written once, by the
/// window where the chat runs in the window and by the host where it runs there, and while
/// chats run in the host the window holds no log of its own.
fn the_event_log_has_one_writer_and_each_hook_line_is_written_once(target: &Target) {
    let id = a_chat_on_the_board(
        target,
        &a_shell(&format!(
            "PURLIS_HARNESS=claude '{}' hook stop </dev/null; printf 'reported.'; read line",
            purlis()
        )),
    );
    Pane::on(target, id).shows("reported.");
    let stops = || {
        purlis_core::eventlog::read(&target.log)
            .expect("the log reads")
            .iter()
            .filter(|event| event.kind == "hook.stop")
            .count()
    };
    until("the hook's line is in the event log", || stops() >= 1);
    // The hook was told its line was taken, so it spooled no copy that could be recorded later.
    let spool =
        purlis_core::hookwire::spool::dir_for(&crate::hooks::socket_for(Some(&target.root)).socket);
    assert!(
        !purlis_core::hookwire::spool::holds_lines(&spool, id),
        "no copy of the line waits to be drained"
    );

    assert_eq!(stops(), 1, "one line, written once");
    assert!(
        purlis_core::eventlog::Log::open(&target.log, DEVICE).is_err(),
        "its one writer holds it, and no second is let in"
    );
    target.sessions.close(id).expect("it closes");
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
    let project = purlis_core::scaffold::planefile::ensure_project_id(&root).expect("its id");
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
/// link` runs on the window's and the host's targets, and through a link waits for what its
/// reason names (#2042).
macro_rules! on_every_target {
    (
        $($behaviour:ident),* $(,)?;
        $(not yet through a link ($why:literal): $($waiting:ident),+ $(,)?;)*
    ) => {
        $(
            mod $behaviour {
                #[test]
                fn in_the_window() {
                    super::$behaviour(&super::in_the_window());
                }

                #[test]
                fn in_the_host() {
                    super::$behaviour(&super::in_the_host());
                }

                #[test]
                fn through_a_link() {
                    super::$behaviour(&super::through_a_link());
                }
            }
        )*
        $($(
            mod $waiting {
                #[test]
                fn in_the_window() {
                    super::$waiting(&super::in_the_window());
                }

                #[test]
                fn in_the_host() {
                    super::$waiting(&super::in_the_host());
                }

                #[test]
                #[ignore = $why]
                fn through_a_link() {
                    super::$waiting(&super::through_a_link());
                }
            }
        )+)*
    };
}

on_every_target!(
    a_chat_starts_and_its_tab_draws_what_it_wrote,
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
    a_hook_refuses_a_listener_that_is_not_its_chats_ancestor,
    the_event_log_has_one_writer_and_each_hook_line_is_written_once;
    not yet through a link ("a sandboxed chat starts on a runner through the link in #2217"):
    a_sandboxed_chats_proxy_carries_what_it_lists_and_refuses_the_rest,
    a_port_withdrawn_while_a_chat_runs_is_carried_no_longer,
    an_ask_raised_by_a_chats_proxy_is_answered_from_the_window;
    not yet through a link ("the window's re-arm and its shell while stopped cross a link in #2220"):
    stop_all_ends_every_chat_and_none_starts_until_the_window_re_arms,
    while_stopped_the_operators_shell_opens;
    not yet through a link ("saving a project on a runner comes with code over the link in #2046"):
    a_write_in_the_project_is_saved_after_its_quiet_period_and_the_window_is_told,
    exactly_one_saver_runs_for_the_project_and_none_once_it_is_stopped;
);

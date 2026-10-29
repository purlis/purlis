// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The window's side of a chat in the session host, over an in-memory link to a real host whose
//! programs are stand-ins, so none of this needs a terminal: what the window hands the host for
//! a start, and how it draws, drives, hears the end of and closes what the host runs. The same
//! over real terminals is the behaviour suite (`crate::behaviour`), on CI.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use purlis_core::engine::Size;
use purlis_host::Hosting;
use purlis_host::standin::{Savers, StandIns};
use purlis_session_protocol::session::{self, Exit};

use super::*;

#[cfg(unix)]
mod browsers_back;
mod movingcode;
mod upgrading;

/// The window's connection to the host `hosting` serves as, over in-memory links, run on
/// `runtime`: its own link, ready, and one more opened for each project root, as the window's
/// are over the host's socket.
pub(crate) fn in_memory<H: session::Host + Clone>(
    hosting: H,
    runtime: &tokio::runtime::Runtime,
) -> (Arc<Connection>, Arc<HostLink>) {
    let link = Arc::new(
        runtime
            .block_on(a_link_to(hosting.clone(), || {}))
            .expect("the window speaks to the host"),
    );
    let handle = runtime.handle().clone();
    let dial: Dial = Arc::new(move |within, naming| {
        open_within(&handle, within, a_named_link_to(hosting.clone(), naming))
    });
    let connection = Arc::new(Connection::default());
    connection.ready(Arc::clone(&link), dial);
    (connection, link)
}

/// [`in_memory`], whose links to a pool's runner reach `runner` (#2050): as a window whose host
/// relays them, seen apart from the relay, which `purlis-host`'s tests hold to its rules.
pub(crate) fn in_memory_with_a_runner<H: session::Host + Clone, R: session::Host + Clone>(
    hosting: H,
    runner: R,
    runtime: &tokio::runtime::Runtime,
) -> Arc<Connection> {
    let link = Arc::new(
        runtime
            .block_on(a_link_to(hosting.clone(), || {}))
            .expect("the window speaks to the host"),
    );
    let handle = runtime.handle().clone();
    let dial: Dial = Arc::new({
        let handle = handle.clone();
        move |within, naming| open_within(&handle, within, a_named_link_to(hosting.clone(), naming))
    });
    let relayed: RelayedDial = Arc::new(move |within, naming, _pool: &str| {
        open_within(&handle, within, a_named_link_to(runner.clone(), naming))
    });
    let connection = Arc::new(Connection::default());
    connection.ready_relaying(link, dial, relayed);
    connection
}

/// A `local-ui` link to `hosting` over an in-memory stream; `closed` is called once the host's
/// end of it ends.
async fn a_link_to<H: session::Host>(
    hosting: H,
    closed: impl FnOnce() + Send + 'static,
) -> Result<HostLink, String> {
    let client = a_bare_link_to(hosting, closed).await?;
    HostLink::over(client, tokio::runtime::Handle::current(), || {}).await
}

/// [`a_link_to`], the window naming itself on it first by `naming` (#2623), as the window's
/// binary dials each project root's link.
async fn a_named_link_to<H: session::Host>(
    hosting: H,
    naming: Option<purlis_session_protocol::browsing::Secret>,
) -> Result<HostLink, String> {
    let client = a_bare_link_to(hosting, || {}).await?;
    HostLink::over_named(client, tokio::runtime::Handle::current(), || {}, naming).await
}

/// The link [`a_link_to`] speaks over, bare: for a test that speaks the protocol itself.
async fn a_bare_link_to<H: session::Host>(
    hosting: H,
    closed: impl FnOnce() + Send + 'static,
) -> Result<Link, String> {
    use purlis_session_protocol::auth::{Credentials, Scope};
    use purlis_session_protocol::link;

    static HELD: std::sync::LazyLock<Credentials> =
        std::sync::LazyLock::new(|| Credentials::mint().expect("credentials"));

    let (a, b) = tokio::io::duplex(1 << 20);
    let (client, served) = tokio::join!(
        link::connect(
            a,
            session::speaks(),
            Scope::LocalUi,
            HELD.of(Scope::LocalUi)
        ),
        link::serve_any(b, session::speaks(), &HELD)
    );
    let served = served.map_err(|why| why.to_string())?;
    tokio::spawn(async move {
        let _ = session::serve(served, hosting, None).await;
        closed();
    });
    client.map_err(|why| why.to_string())
}

const SIZE: Size = Size {
    columns: 100,
    rows: 30,
};
const PATIENCE: Duration = Duration::from_secs(10);

struct Hosted {
    sessions: HostedSessions,
    link: Arc<HostLink>,
    runs: StandIns,
    savers: Savers,
    root: std::path::PathBuf,
    _dir: tempfile::TempDir,
    _runtime: tokio::runtime::Runtime,
}

fn a_project() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    std::fs::write(root.join(purlis_core::plane::MANIFEST), "").unwrap();
    (dir, root)
}

/// A project's chats in a session host whose programs are stand-ins, over an in-memory link.
fn hosted() -> Hosted {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let runs = StandIns::default();
    let savers = Savers::default();
    let hosting = Hosting::running_with(
        crate::sessionhost::Composes::as_the_tests_ship(),
        runs.clone(),
    )
    .saving_with(savers.clone());
    let (connection, link) = in_memory(hosting, &runtime);
    let (dir, root) = a_project();
    Hosted {
        sessions: HostedSessions::new(connection, &root),
        link,
        runs,
        savers,
        root,
        _dir: dir,
        _runtime: runtime,
    }
}

fn a_shell(script: &str) -> Opening {
    Opening {
        program: Some("/bin/sh".to_owned()),
        args: vec!["-c".to_owned(), script.to_owned()],
        cwd: Some("/tmp".to_owned()),
        size: SIZE,
        env: vec![("PURLIS_ROOT".to_owned(), "/home/me/project".to_owned())],
        ..Opening::default()
    }
}

fn value(spec: &purlis_core::session::Spec, name: &str) -> Option<String> {
    spec.env
        .iter()
        .rev()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.to_string_lossy().into_owned())
}

fn until(what: &str, done: impl Fn() -> bool) {
    let deadline = Instant::now() + PATIENCE;
    while !done() {
        assert!(Instant::now() < deadline, "{what} never happened");
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// What a pane is sent, as it is sent.
fn a_pane() -> (Sink, Arc<Mutex<String>>) {
    let seen = Arc::new(Mutex::new(String::new()));
    let sink = {
        let seen = Arc::clone(&seen);
        Box::new(move |text: String| seen.lock().unwrap().push_str(&text)) as Sink
    };
    (sink, seen)
}

#[test]
fn two_clones_of_one_project_in_one_window_both_start_chats() {
    // #2178: one project id, two folders, one window's connection to its host.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let runs = StandIns::default();
    let hosting = Hosting::running_with(
        crate::sessionhost::Composes::as_the_tests_ship(),
        runs.clone(),
    );
    let (connection, _window) = in_memory(hosting, &runtime);
    let clone = || {
        let (dir, root) = a_project();
        std::fs::write(
            root.join(purlis_core::plane::MANIFEST),
            "[project]\nid = \"01J9PRJCT00000000000000000\"\n",
        )
        .unwrap();
        (dir, root)
    };
    let ((_one, first), (_two, second)) = (clone(), clone());
    let first = HostedSessions::new(Arc::clone(&connection), &first);
    let second = HostedSessions::new(connection, &second);

    let started = (
        first.open(None, &a_shell("true"), &|_| {}),
        second.open(None, &a_shell("true"), &|_| {}),
    );

    assert_eq!(started, (Ok(1), Ok(1)), "each clone deals its own numbers");
    assert_eq!(runs.ran().len(), 2);
    drop((first, second));
    drop(runtime);
}

#[test]
fn chats_run_in_the_host_unless_the_switch_is_set_to_zero() {
    // #2004: the session host is the default, and `0` is the way back to the window's own
    // process until #2005 deletes it.
    assert!(!switched_on(Some("0")));
    for on in [None, Some(""), Some("1"), Some("true"), Some("no")] {
        assert!(switched_on(on), "{on:?}");
    }
}

#[test]
fn a_chat_its_profile_hands_credentials_is_never_started_in_the_host() {
    // #2036: what a profile hands its chat is set on a program the window starts, and never
    // goes over the link.
    let (_dir, root) = a_project();
    let sessions = HostedSessions::new(Arc::new(Connection::default()), &root);
    let announced = Mutex::new(Vec::new());
    let handed = Opening {
        handed: purlis_core::profilesignin::Handed::of(vec![(
            "AWS_REGION".to_owned(),
            "a-region-for-this-test".to_owned(),
        )]),
        ..a_shell("true")
    };

    let refused = sessions
        .open(None, &handed, &|id| announced.lock().unwrap().push(id))
        .unwrap_err();

    assert_eq!(refused, crate::hostedsessions::NOT_HANDED_IN_THE_HOST);
    assert!(announced.lock().unwrap().is_empty());
}

#[test]
fn with_no_host_a_chat_is_refused_with_why_and_never_announced() {
    let (_dir, root) = a_project();
    let connection = Arc::new(Connection::default());
    connection.gone("another session host already serves this user on this device");
    let sessions = HostedSessions::new(connection, &root);
    let announced = Mutex::new(Vec::new());

    let refused = sessions
        .open(None, &a_shell("true"), &|id| {
            announced.lock().unwrap().push(id)
        })
        .unwrap_err();

    assert!(
        refused.contains("it is not running: another session host already serves"),
        "{refused}"
    );
    assert!(announced.lock().unwrap().is_empty());
    assert!(sessions.running().is_empty());
}

#[test]
fn a_start_hands_the_host_what_the_window_composed_and_the_host_composes_it_as_the_window_would() {
    let hosted = hosted();
    let announced = Mutex::new(Vec::new());

    let id = hosted
        .sessions
        .open(None, &a_shell("echo hi"), &|id| {
            announced.lock().unwrap().push(id)
        })
        .expect("it starts");

    assert_eq!(id, 1);
    assert_eq!(*announced.lock().unwrap(), [1]);
    let (spec, _) = &hosted.runs.ran()[0];
    assert_eq!(spec.program, "/bin/sh");
    assert_eq!(spec.args, ["-c", "echo hi"]);
    assert_eq!(spec.cwd.as_deref(), Some(std::path::Path::new("/tmp")));
    assert_eq!(spec.size, SIZE);
    assert!(spec.env_clear);
    assert_eq!(value(spec, "PURLIS_SESSION_ID").as_deref(), Some("1"));
    assert_eq!(
        value(spec, "PURLIS_ROOT").as_deref(),
        Some("/home/me/project")
    );
    let in_the_window = purlis_core::chatspec::spec(
        std::env::vars_os(),
        &purlis_core::chatspec::Terminal {
            program: "/bin/sh",
            args: &["-c".to_owned(), "echo hi".to_owned()],
            cwd: Some("/tmp"),
            size: SIZE,
            starting: purlis_core::chatenv::Starting {
                harness: None,
                operator: &[],
                strip: &[],
                set: &[("PURLIS_ROOT".to_owned(), "/home/me/project".to_owned())],
                git_hooks: None,
                declared: &[],
            },
            number: 1,
            reporting: None,
        },
    );
    assert_eq!(spec.env, in_the_window.env, "the same environment and PATH");
    assert_eq!(
        purlis_core::scaffold::planefile::project_id(&hosted.root),
        None,
        "holding the project wrote no id into it (#2004)"
    );
}

#[test]
fn a_start_carries_the_run_it_begins_and_the_host_lists_it_and_names_it_as_it_ends() {
    // #2172: the run the window minted for this start (ADR 0066) is the host's too, so a
    // window that comes back finds each chat by the run it is in (#2006).
    let runtime = a_runtime();
    let runs = StandIns::default();
    let hosting = Hosting::running_with(
        crate::sessionhost::Composes::as_the_tests_ship(),
        runs.clone(),
    );
    let (connection, _window) = in_memory(hosting.clone(), &runtime);
    let (_dir, root) = a_project();
    let sessions = HostedSessions::new(connection, &root);
    let (watcher, mut told) = runtime.block_on(async {
        let link = a_bare_link_to(hosting, || {}).await.expect("a second link");
        let (client, _views, events) = Client::new(link);
        client
            .subscribe(session::Since::new())
            .await
            .expect("it subscribes");
        (client, events)
    });
    let run = "01JA2172RVN00000000000000A".to_owned();

    sessions
        .open(
            None,
            &Opening {
                run: Some(run.clone()),
                ..a_shell("true")
            },
            &|_| {},
        )
        .expect("it starts");

    let listed = runtime.block_on(watcher.list()).expect("the host lists");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].run.as_deref(), Some(run.as_str()));
    runs.last().ends(Exit::Code(0));
    let ended = runtime.block_on(async {
        loop {
            match tokio::time::timeout(PATIENCE, told.recv()).await {
                Ok(Some(Told::Pushed(Pushed::Ended { run, .. }))) => break run,
                Ok(Some(_)) => {}
                other => panic!("the end was never told: {other:?}"),
            }
        }
    });
    assert_eq!(ended.as_deref(), Some(run.as_str()));
    drop(sessions);
    drop(runtime);
}

/// #3274: a grade's chat ends with its window. The start says so to the host (1.42), and once
/// the window's links end without a quit the host ends that chat at once, while every other
/// waits for the window to come back.
#[test]
fn a_chat_that_ends_with_its_window_is_ended_by_the_host_once_the_window_goes() {
    let runtime = a_runtime();
    let runs = StandIns::default();
    let hosting = Hosting::running_with(
        crate::sessionhost::Composes::as_the_tests_ship(),
        runs.clone(),
    );
    let (connection, window) = in_memory(hosting, &runtime);
    let (_dir, root) = a_project();
    let sessions = HostedSessions::new(connection, &root);
    sessions
        .open(None, &a_shell("true"), &|_| {})
        .expect("it starts");
    let kept = runs.last();
    sessions
        .open(
            None,
            &Opening {
                ends_with_its_window: true,
                ..a_shell("true")
            },
            &|_| {},
        )
        .expect("it starts");
    let graded = runs.last();
    assert!(!graded.seen().stopped, "it runs while the window is there");

    drop(sessions);
    drop(window);

    let deadline = Instant::now() + PATIENCE;
    while !graded.seen().stopped {
        assert!(Instant::now() < deadline, "the host never ended it");
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(!kept.seen().stopped, "the other waits for its window");
    drop(runtime);
}

#[test]
fn a_chat_keeps_the_number_it_already_answers_to_and_numbers_are_dealt_past_it() {
    let hosted = hosted();

    let kept = hosted
        .sessions
        .open(Some(7), &a_shell("true"), &|_| {})
        .unwrap();
    let next = hosted
        .sessions
        .open(None, &a_shell("true"), &|_| {})
        .unwrap();

    assert_eq!((kept, next), (7, 8));
    assert_eq!(hosted.sessions.running(), [7, 8]);
    assert_eq!(hosted.sessions.dealt(), 8);
    assert_eq!(hosted.sessions.process_id(7), Some(4242));
}

#[test]
fn a_pane_draws_the_screen_then_what_follows_and_is_told_when_the_program_ends() {
    let hosted = hosted();
    let ends = Arc::new(Mutex::new(Vec::new()));
    hosted.sessions.when_one_ends({
        let ends = Arc::clone(&ends);
        Arc::new(move |id, exit| ends.lock().unwrap().push((id, exit)))
    });
    let id = hosted
        .sessions
        .open(None, &a_shell("true"), &|_| {})
        .unwrap();
    let program = hosted.runs.last();
    program.writes(b"ready> ");
    let (sink, seen) = a_pane();

    let watching = hosted.sessions.watch(id, sink).expect("a view opens");
    assert_eq!(watching.size, SIZE);
    until("the screen is drawn", || {
        seen.lock().unwrap().contains("ready> ")
    });
    hosted.sessions.input(id, b"hello").unwrap();
    until("the typing is drawn", || {
        seen.lock().unwrap().ends_with("ready> hello")
    });
    assert_eq!(program.seen().typed, b"hello");

    program.ends(Exit::Code(3));

    until("the pane is told", || seen.lock().unwrap().ends_with(ENDED));
    until("whoever asked is told", || !ends.lock().unwrap().is_empty());
    assert_eq!(
        *ends.lock().unwrap(),
        [(id, purlis_core::session::Exit::Code(3))]
    );
    assert_eq!(hosted.sessions.process_id(id), None);
    assert_eq!(hosted.sessions.running(), [id], "the tab stays");
}

#[test]
fn a_resize_and_readiness_go_to_the_host() {
    let hosted = hosted();
    let id = hosted
        .sessions
        .open(None, &a_shell("true"), &|_| {})
        .unwrap();

    hosted
        .sessions
        .resize(
            id,
            Size {
                columns: 120,
                rows: 40,
            },
        )
        .unwrap();
    let readiness = hosted.sessions.readiness(id).unwrap();

    assert_eq!(
        hosted.runs.last().seen().sizes,
        [Size {
            columns: 120,
            rows: 40
        }]
    );
    assert_eq!(
        readiness,
        Readiness {
            edits_lines: Some(false),
            quiet_for: Duration::from_millis(1500),
        }
    );
}

#[test]
fn a_closed_chat_is_ended_in_the_host_and_forgotten() {
    let hosted = hosted();
    let id = hosted
        .sessions
        .open(None, &a_shell("true"), &|_| {})
        .unwrap();

    hosted.sessions.close(id).unwrap();

    assert!(hosted.sessions.running().is_empty());
    let program = hosted.runs.last();
    until("the host ends it", || program.seen().stopped);
    assert!(hosted.sessions.input(id, b"x").is_err());
}

#[test]
fn stopping_every_program_stops_each_in_the_host_and_keeps_its_tab() {
    let hosted = hosted();
    for _ in 0..3 {
        hosted
            .sessions
            .open(None, &a_shell("true"), &|_| {})
            .unwrap();
    }

    let asked = hosted.sessions.stop_every_program();

    assert_eq!(asked, 3);
    assert!(hosted.runs.ran().iter().all(|(_, run)| run.seen().stopped));
    assert_eq!(hosted.sessions.running(), [1, 2, 3]);
}

#[test]
fn ending_every_chat_closes_each_in_the_host() {
    let hosted = hosted();
    for _ in 0..2 {
        hosted
            .sessions
            .open(None, &a_shell("true"), &|_| {})
            .unwrap();
    }

    hosted.sessions.end_all();

    assert!(hosted.sessions.running().is_empty());
    assert!(hosted.runs.ran().iter().all(|(_, run)| run.seen().stopped));
}

#[test]
fn a_closed_chats_end_does_not_end_the_chat_put_back_under_its_number() {
    let hosted = hosted();
    let ends = Arc::new(Mutex::new(Vec::new()));
    hosted.sessions.when_one_ends({
        let ends = Arc::clone(&ends);
        Arc::new(move |id, exit| ends.lock().unwrap().push((id, exit)))
    });
    hosted.runs.hold_on();
    hosted
        .sessions
        .open(Some(5), &a_shell("true"), &|_| {})
        .unwrap();
    let closed = hosted.runs.last();
    hosted.sessions.close(5).unwrap();

    let again = hosted
        .sessions
        .open(Some(5), &a_shell("true"), &|_| {})
        .unwrap();
    closed.ends(Exit::Code(0));
    std::thread::sleep(Duration::from_millis(200));

    assert_eq!(again, 5);
    assert_eq!(
        hosted.sessions.process_id(5),
        Some(4242),
        "the new one runs on"
    );
    assert!(
        ends.lock().unwrap().is_empty(),
        "{:?}",
        ends.lock().unwrap()
    );
}

#[test]
fn a_start_the_host_numbers_otherwise_fails_and_is_closed() {
    let hosted = hosted();
    let (link, project) = hosted.sessions.link().unwrap();
    // A program of the project's that the window does not know already has number 5.
    link.call(Command::StartProgram(Box::new(StartProgram {
        number: Some(5),
        launch: Some(launch_of(&a_shell("true"), "/bin/sh".to_owned())),
        ..StartProgram::of(project, ProgramKind::Shell)
    })))
    .unwrap();

    let refused = hosted
        .sessions
        .open(Some(5), &a_shell("true"), &|_| {})
        .unwrap_err();

    assert!(refused.contains("not 5"), "{refused}");
    assert!(hosted.sessions.running().is_empty());
    let started = hosted.runs.last();
    until("the host closes it", || started.seen().stopped);
    drop(hosted.link);
}

/// A project's hook channel standing in for the host's (#2000): what it hands on goes over the
/// link exactly as the real one's does, and a test hears as its listener would through
/// [`HooksHeard::hearing`], each line taken as admitted.
#[derive(Clone, Default)]
struct HooksHeard {
    relaying: Arc<Mutex<Option<Arc<purlis_core::hookwire::relay::Relaying>>>>,
    issued: Arc<Mutex<Vec<u32>>>,
    /// Each reply the window sent to a line the host handed on, as it came over the link.
    replies: Arc<Mutex<Vec<serde_json::Value>>>,
    /// The project's board, kept as the window's binary keeps it in the host (#2194).
    board: purlis_core::hookchannel::KeptBoard,
}

impl HooksHeard {
    /// What the project's listener would hear an admitted line with.
    fn hearing(&self) -> purlis_core::hookwire::Hearing {
        self.relaying
            .lock()
            .unwrap()
            .clone()
            .expect("the project is held")
            .hearing(Arc::new(purlis_core::hookwire::ChatTokens::default()))
    }
}

impl purlis_host::Hear for HooksHeard {
    fn open(
        &self,
        _project: &str,
        path: &std::path::Path,
        hand_on: purlis_host::HandOn,
    ) -> Result<Option<Arc<dyn purlis_host::Channel>>, String> {
        let relaying = purlis_core::hookwire::relay::Relaying::new(self.board.handing_on(hand_on));
        // As the host's own channel is told where it holds the project (#3400).
        relaying.of_project_at(path);
        *self.relaying.lock().unwrap() = Some(Arc::clone(&relaying));
        Ok(Some(Arc::new(StandInChannel {
            relaying,
            issued: Arc::clone(&self.issued),
            replies: Arc::clone(&self.replies),
            board: self.board.clone(),
        })))
    }
}

struct StandInChannel {
    relaying: Arc<purlis_core::hookwire::relay::Relaying>,
    issued: Arc<Mutex<Vec<u32>>>,
    replies: Arc<Mutex<Vec<serde_json::Value>>>,
    board: purlis_core::hookchannel::KeptBoard,
}

impl purlis_host::Channel for StandInChannel {
    fn report_to(&self, number: u32) -> Result<session::Reporting, String> {
        self.issued.lock().unwrap().push(number);
        Ok(session::Reporting {
            socket: "/the/project/hooks.sock".into(),
            token: format!("issued-by-the-host-{number}"),
        })
    }

    fn bind(&self, _number: u32, _pid: u32) {}

    fn ended(&self, _number: u32, _pid: u32) {}

    fn forget(&self, _number: u32) {}

    fn replied(&self, id: u64, reply: serde_json::Value) {
        self.replies.lock().unwrap().push(reply.clone());
        self.relaying
            .replied(id, serde_json::from_value(reply).expect("a reply"));
    }

    fn moved(&self, number: u32, moved: serde_json::Value) {
        self.board.moved(number, moved);
    }

    fn board(&self) -> Result<serde_json::Value, String> {
        self.board.whole()
    }

    fn ended_with_no_window(&self, number: u32, exit: &session::Exit) {
        self.board.ended(number, exit);
    }
}

/// A project's chats in a session host that owns its hook channel, heard by the window's own
/// hooks: what each chat's board says, as the window is told.
struct Heard {
    hosted: Hosted,
    hooks: crate::hooks::Hooks,
    channel: HooksHeard,
    moved: Arc<Mutex<Vec<crate::hooks::Moved>>>,
    hosting: Hosting,
}

fn heard_in_the_window() -> Heard {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let runs = StandIns::default();
    let channel = HooksHeard::default();
    let hosting = Hosting::hearing_with(
        crate::sessionhost::Composes::as_the_tests_ship(),
        runs.clone(),
        channel.clone(),
    );
    let (dir, root) = a_project();
    let (link, hooks, sessions, moved) = a_window_of(&hosting, &root, &runtime);
    Heard {
        hosted: Hosted {
            sessions,
            link,
            runs,
            savers: Savers::default(),
            root,
            _dir: dir,
            _runtime: runtime,
        },
        hooks,
        channel,
        moved,
        hosting,
    }
}

/// A window on `hosting`, over a link of its own, whose hooks for the project at `root` hear
/// what the host hands on.
fn a_window_of(
    hosting: &Hosting,
    root: &std::path::Path,
    runtime: &tokio::runtime::Runtime,
) -> (
    Arc<HostLink>,
    crate::hooks::Hooks,
    HostedSessions,
    Arc<Mutex<Vec<crate::hooks::Moved>>>,
) {
    let (connection, link) = in_memory(hosting.clone(), runtime);
    let moved: Arc<Mutex<Vec<crate::hooks::Moved>>> = Arc::default();
    let plane: crate::planes::PlaneId =
        serde_json::from_str(&serde_json::to_string(root).unwrap()).unwrap();
    let hooks = crate::hooks::Hooks::relayed(
        plane,
        &crate::hooks::socket_for(Some(root)),
        {
            let moved = Arc::clone(&moved);
            Arc::new(move |what| moved.lock().unwrap().push(what))
        },
        Arc::new(|_| {}),
    );
    let sessions = HostedSessions::new(connection, root);
    sessions.hear_through(hooks.relayed_from_the_host().expect("relayed"));
    (link, hooks, sessions, moved)
}

impl Heard {
    /// A chat started in the host, announced to the window's board as the window does.
    fn a_chat(&self) -> u32 {
        self.hosted
            .sessions
            .open(None, &a_shell("true"), &|id| {
                self.hooks.board().opened(id, None, None);
            })
            .expect("it starts")
    }
}

fn reported(chat: u32, event: purlis_core::state::Event) -> purlis_core::hookwire::Report {
    purlis_core::hookwire::Report {
        chat,
        event,
        conversation: purlis_core::hookwire::Conversation::Unknown,
        pid: None,
        agent: None,
        detail: purlis_core::state::Detail::default(),
    }
}

#[test]
fn the_windows_launch_never_says_where_a_chat_reports() {
    let launch = launch_of(&a_shell("true"), "/bin/sh".to_owned());

    assert_eq!(launch.reporting, None);
}

#[test]
fn the_host_issues_a_chat_its_token_and_the_window_never_holds_one() {
    let heard = heard_in_the_window();

    let id = heard.a_chat();

    assert_eq!(*heard.channel.issued.lock().unwrap(), [id]);
    let (spec, _) = &heard.hosted.runs.ran()[0];
    assert_eq!(
        value(spec, purlis_core::hookwire::TOKEN_ENV).as_deref(),
        Some(format!("issued-by-the-host-{id}").as_str())
    );
    assert_eq!(
        value(spec, purlis_core::hookwire::SOCKET_ENV).as_deref(),
        Some("/the/project/hooks.sock")
    );
    assert_eq!(
        heard.hosted.sessions.reports_to(),
        Some(crate::hooks::socket_for(Some(&heard.hosted.root)).socket),
        "the chat's sandbox lets it reach the socket the host binds"
    );
}

#[test]
fn a_report_the_host_hears_moves_the_windows_board_and_its_needs_you() {
    let heard = heard_in_the_window();
    let id = heard.a_chat();

    let taken = (heard.channel.hearing().each)(reported(id, purlis_core::state::Event::Stop));

    assert!(taken.is_ok(), "{taken:?}");
    let moved = heard.moved.lock().unwrap().clone();
    let last = moved.last().expect("the window is told");
    assert_eq!((last.session, last.needs_you), (id, true));
    assert_eq!(last.queue, [id]);
}

#[test]
fn a_hook_lines_record_goes_back_naming_no_path_to_a_host_that_said_it_uses_none() {
    // The host's push of the line says it records it under the project as it holds it (1.40,
    // #3400): the record the window's hearing sends back with its reply names no project path.
    let heard = heard_in_the_window();
    let id = heard.a_chat();

    let taken = (heard.channel.hearing().each)(reported(id, purlis_core::state::Event::Stop));

    assert!(taken.is_ok(), "{taken:?}");
    let replies = heard.channel.replies.lock().unwrap().clone();
    let records: Vec<String> = replies
        .iter()
        .filter(|reply| reply.get("line").is_some() || reply.get("record").is_some())
        .map(serde_json::Value::to_string)
        .collect();
    assert_eq!(records.len(), 1, "{replies:?}");
    assert!(records[0].starts_with(r#"{"line":"#), "{}", records[0]);
    assert!(!records[0].contains("plane"), "{}", records[0]);
    let root = heard.hosted.root.display().to_string();
    assert!(!records[0].contains(&root), "{}", records[0]);
}

#[test]
fn a_permission_ask_the_host_hears_is_held_in_the_window_and_answered_there() {
    let heard = heard_in_the_window();
    let id = heard.a_chat();

    let waiting = (heard.channel.hearing().permission)(purlis_core::hookwire::PermissionAsked {
        chat: id,
        permission_request: purlis_core::harness::hooked::Source::ClaudeCode,
        payload: serde_json::json!({"tool_name": "Bash",
            "tool_input": {"command": "npm test"}}),
        ran_for_ms: None,
    })
    .expect("the window holds it");
    let held = heard.hooks.asks().pending(Instant::now());
    let ask = held.first().expect("the window lists it");
    let allow = ask
        .ask
        .options
        .iter()
        .find(|option| option.kind == purlis_core::harness::model::ChoiceKind::Allow)
        .expect("an option to allow it")
        .clone();
    heard
        .hooks
        .answer(id, &ask.id.to_string(), &allow.id)
        .expect("answered in the window");

    assert_eq!(
        waiting
            .answered
            .recv_timeout(PATIENCE)
            .map(|chosen| chosen.id),
        Ok(allow.id)
    );
}

fn told_nothing() -> crate::autosave::Told {
    Arc::new(|_| {})
}

#[test]
fn the_project_is_saved_in_the_host_and_the_window_answers_who_is_mid_turn_from_its_board() {
    let hosted = hosted();
    let saving = hosted.sessions.keep_saving(&hosted.root, told_nothing());
    until("the host saves the project", || {
        hosted.savers.running() == 1
    });
    saving.asks_mid_turn_of(Arc::new(|workspace: &str| vec![format!("{workspace} 1")]));

    assert_eq!(
        hosted.savers.asks_the_window("alpha"),
        Some(vec!["alpha 1".to_owned()]),
        "the host did not hear the window's board"
    );
    assert_eq!(hosted.savers.started()[0].1, hosted.root);
}

#[test]
fn a_quit_that_the_host_does_not_answer_gives_up_on_it_by_the_deadline() {
    let hosted = hosted();
    let saving = hosted.sessions.keep_saving(&hosted.root, told_nothing());
    until("the host saves the project", || {
        hosted.savers.running() == 1
    });
    // Its save never lets go: the host never answers the stop.
    hosted.savers.mid_save();

    let started = Instant::now();
    let stopped = saving.stop_by(started + Duration::from_millis(500));

    assert!(!stopped, "a stop the host never answered was taken as done");
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "the quit waited {:?} for a host that does not answer",
        started.elapsed()
    );
    hosted.savers.finish_saving();
}

/// The board the host keeps, as a window takes it.
fn the_hosts_board(heard: &Heard) -> purlis_core::state::Board {
    let mut taken = purlis_core::state::Board::new();
    taken.keep(
        serde_json::from_value(
            heard
                .channel
                .board
                .whole()
                .expect("a board within an answer"),
        )
        .expect("a whole board"),
    );
    taken
}

#[test]
fn each_move_the_window_makes_on_its_board_is_made_on_the_hosts() {
    use crate::host::ChatBoard;
    use purlis_core::state::{Event, State};
    let heard = heard_in_the_window();
    let id = heard.a_chat();
    let other = heard.a_chat();

    (heard.channel.hearing().each)(reported(id, Event::Stop)).expect("taken");
    (heard.channel.hearing().each)(reported(other, Event::Stop)).expect("taken");
    heard.hooks.ignored(other);
    heard.hooks.task_failed(
        id,
        purlis_core::state::FailedTask::new(
            "task-1",
            "the task",
            purlis_core::state::HowFailed::Failed,
            "it broke",
        ),
    );

    until("the host's board follows the window's", || {
        the_hosts_board(&heard).failed_tasks(id).len() == 1
    });
    let host = the_hosts_board(&heard);
    assert_eq!(host.needs_you(), [id]);
    assert_eq!(host.needs_you(), heard.hooks.board().needs_you());
    assert_eq!(host.state(other), State::Waiting);

    heard.hooks.closed(other);
    until("the close reaches the host's board", || {
        the_hosts_board(&heard).chats().count() == 1
    });
}

#[test]
fn a_window_that_holds_the_project_again_reads_every_chat_as_the_host_kept_it() {
    use purlis_core::state::{Event, State};
    let heard = heard_in_the_window();
    let id = heard.a_chat();
    (heard.channel.hearing().each)(reported(id, Event::Stop)).expect("taken");
    until("the host's board has the chat's end of turn", || {
        the_hosts_board(&heard).needs_you() == [id]
    });

    let (_link, hooks, sessions, moved) =
        a_window_of(&heard.hosting, &heard.hosted.root, &heard.hosted._runtime);
    sessions.hold().expect("it holds the project");

    assert_eq!(hooks.board().state(id), State::Waiting);
    assert_eq!(hooks.board().needs_you(), [id]);
    let told = moved.lock().unwrap().clone();
    let told = told.last().expect("the window is told the chat");
    assert_eq!((told.session, told.needs_you), (id, true));
    assert_eq!(told.queue, [id]);
}

#[test]
fn a_host_before_held_trust_events_is_sent_the_trust_event_first_and_no_start_taken_back() {
    // #2252: a window and a host one version apart still record the sandbox coming back on,
    // before the start's program runs, as every start's was.
    use purlis_core::eventlog::{Attributes, Began, Entry, HeldTrust};
    use purlis_session_protocol::version::Version;
    let trust = HeldTrust::of(
        &purlis_core::sandbox::Change::On,
        Some(purlis_core::harness::Harness::ClaudeCode),
        None,
    );
    let begin = |trust: Option<HeldTrust>| Entry::Begin {
        plane: "/planes/one".into(),
        number: 3,
        chat: "01K6E8ZK6V4Q9T0N3M2B1C5D7E".into(),
        run: "01K6E8ZK6V4Q9T0N3M2B1C5D7F".into(),
        cause: Began::Start,
        attributes: Attributes::default(),
        held: true,
        trust: trust.map(Box::new),
    };
    let taken_back = Entry::TakenBack {
        plane: "/planes/one".into(),
        number: 3,
    };
    let older = Version { major: 1, minor: 8 };

    assert_eq!(
        as_the_host_reads(&begin(Some(trust.clone())), older),
        [
            Entry::Trust {
                chat: "01K6E8ZK6V4Q9T0N3M2B1C5D7E".into(),
                run: "01K6E8ZK6V4Q9T0N3M2B1C5D7F".into(),
                kind: "trust.sandbox.on".into(),
                body: trust.body().clone(),
            },
            begin(None),
        ]
    );
    assert!(as_the_host_reads(&taken_back, older).is_empty());
    assert_eq!(
        as_the_host_reads(&begin(Some(trust.clone())), session::VERSION),
        [begin(Some(trust))]
    );
    assert_eq!(
        as_the_host_reads(&taken_back, session::VERSION),
        [taken_back]
    );
}

// ----- a host killed outright, and the one that replaces it (#2007) --------------------------

/// One session host on in-memory links, whose serving of every link can be ended as a kill
/// ends it.
struct OneHost {
    link: Arc<HostLink>,
    dial: Dial,
    runs: StandIns,
    savers: Savers,
    serving: Arc<Mutex<Vec<tokio::task::AbortHandle>>>,
}

impl OneHost {
    /// The host is killed outright: it serves nothing more, and its end of every link is gone.
    fn killed(&self) {
        for serving in self.serving.lock().unwrap().drain(..) {
            serving.abort();
        }
    }
}

/// A `local-ui` link to `hosting` over an in-memory stream, whose serving is kept in `serving`.
async fn a_served_link(
    hosting: Hosting,
    serving: Arc<Mutex<Vec<tokio::task::AbortHandle>>>,
) -> Result<HostLink, String> {
    use purlis_session_protocol::auth::{Credentials, Scope};
    use purlis_session_protocol::link;
    static HELD: std::sync::LazyLock<Credentials> =
        std::sync::LazyLock::new(|| Credentials::mint().expect("credentials"));
    let (a, b) = tokio::io::duplex(256 * 1024);
    let (client, served) = tokio::join!(
        link::connect(
            a,
            session::speaks(),
            Scope::LocalUi,
            HELD.of(Scope::LocalUi)
        ),
        link::serve_any(b, session::speaks(), &HELD)
    );
    let served = served.map_err(|why| why.to_string())?;
    let task = tokio::spawn(async move {
        let _ = session::serve(served, hosting, None).await;
    });
    serving.lock().unwrap().push(task.abort_handle());
    let client = client.map_err(|why| why.to_string())?;
    HostLink::over(client, tokio::runtime::Handle::current(), || {}).await
}

/// A host started on `runtime`, as the window's supervisor starts one: the window's link to
/// it, and one more for each project root.
fn a_host_on(runtime: &tokio::runtime::Runtime) -> OneHost {
    let runs = StandIns::default();
    let savers = Savers::default();
    let hosting = Hosting::running_with(
        crate::sessionhost::Composes::as_the_tests_ship(),
        runs.clone(),
    )
    .saving_with(savers.clone());
    let serving: Arc<Mutex<Vec<tokio::task::AbortHandle>>> = Arc::default();
    let link = Arc::new(
        runtime
            .block_on(a_served_link(hosting.clone(), Arc::clone(&serving)))
            .unwrap(),
    );
    let dial: Dial = {
        let (handle, serving) = (runtime.handle().clone(), Arc::clone(&serving));
        Arc::new(move |within, _naming| {
            open_within(
                &handle,
                within,
                a_served_link(hosting.clone(), Arc::clone(&serving)),
            )
        })
    };
    OneHost {
        link,
        dial,
        runs,
        savers,
        serving,
    }
}

/// A project's chats in a host that is killed and replaced as the test says.
struct Supervised {
    sessions: HostedSessions,
    connection: Arc<Connection>,
    first: OneHost,
    root: std::path::PathBuf,
    runtime: tokio::runtime::Runtime,
    _dir: tempfile::TempDir,
}

impl Supervised {
    /// The host that replaces the first, once the window has seen it go.
    fn replaced(&self) -> OneHost {
        let next = a_host_on(&self.runtime);
        self.connection
            .ready(Arc::clone(&next.link), Arc::clone(&next.dial));
        next
    }
}

fn supervised() -> Supervised {
    let runtime = a_runtime();
    let first = a_host_on(&runtime);
    let connection = Arc::new(Connection::default());
    connection.ready(Arc::clone(&first.link), Arc::clone(&first.dial));
    let (dir, root) = a_project();
    Supervised {
        sessions: HostedSessions::new(Arc::clone(&connection), &root),
        connection,
        first,
        root,
        runtime,
        _dir: dir,
    }
}

#[test]
fn every_chat_in_a_host_killed_outright_ends_with_it_and_says_so() {
    let host = supervised();
    let ends = Arc::new(Mutex::new(Vec::new()));
    host.sessions.when_one_ends({
        let ends = Arc::clone(&ends);
        Arc::new(move |id, _exit| ends.lock().unwrap().push(id))
    });
    let chats: Vec<u32> = (0..20)
        .map(|_| {
            host.sessions
                .open(None, &a_shell("true"), &|_| {})
                .expect("it starts")
        })
        .collect();
    let (sink, pane) = a_pane();
    host.sessions.watch(chats[0], sink).expect("a view opens");
    let killed = Instant::now();

    host.first.killed();

    until("every chat is told it ended", || {
        ends.lock().unwrap().len() == chats.len()
    });
    assert!(killed.elapsed() < Duration::from_secs(30));
    let mut ended = ends.lock().unwrap().clone();
    ended.sort_unstable();
    assert_eq!(ended, chats, "each chat once");
    for chat in &chats {
        assert!(
            host.sessions.lost_with_its_host(*chat),
            "chat {chat} did not end with its host"
        );
        assert_eq!(host.sessions.process_id(*chat), None);
    }
    until("its pane says it ended", || {
        pane.lock().unwrap().ends_with(ENDED)
    });
    assert_eq!(host.sessions.running(), chats, "every tab stays");
}

#[test]
fn a_chat_that_ended_by_itself_did_not_end_with_its_host() {
    let host = supervised();
    let id = host
        .sessions
        .open(None, &a_shell("true"), &|_| {})
        .expect("it starts");
    host.first.runs.last().ends(Exit::Code(0));
    until("its end is heard", || {
        host.sessions.process_id(id).is_none()
    });

    host.first.killed();
    std::thread::sleep(Duration::from_millis(100));

    assert!(!host.sessions.lost_with_its_host(id));
}

#[test]
fn a_chat_lost_with_its_host_starts_again_in_the_host_that_replaces_it() {
    let host = supervised();
    let id = host
        .sessions
        .open(None, &a_shell("true"), &|_| {})
        .expect("it starts");
    host.first.killed();
    until("it ended with its host", || {
        host.sessions.lost_with_its_host(id)
    });
    host.connection.restarting();
    let next = host.replaced();

    // As Resume does: a new run of the chat starts, and then the one that was lost closes.
    let again = host
        .sessions
        .open(None, &a_shell("true"), &|_| {})
        .expect("it starts again");
    host.sessions.close(id).expect("the lost one closes");

    assert_eq!(next.runs.ran().len(), 1, "in the new host");
    assert_eq!(host.sessions.running(), [again]);
    assert!(!host.sessions.lost_with_its_host(again));
    assert!(!host.sessions.lost_with_its_host(id), "kept past its close");
    assert_eq!(host.sessions.process_id(again), Some(4242));
}

#[test]
fn a_start_while_the_host_is_started_again_waits_for_the_new_one() {
    let host = supervised();
    host.first.killed();
    host.connection.restarting();

    let starting = std::thread::scope(|scope| {
        let starting = scope.spawn(|| host.sessions.open(None, &a_shell("true"), &|_| {}));
        std::thread::sleep(Duration::from_millis(200));
        let next = host.replaced();
        (starting.join().unwrap(), next)
    });

    let (started, next) = starting;
    assert!(started.is_ok(), "{started:?}");
    assert_eq!(next.runs.ran().len(), 1);
}

#[test]
fn a_project_the_host_saved_is_saved_by_the_host_that_replaces_it() {
    let host = supervised();
    let _saving = host.sessions.keep_saving(&host.root, told_nothing());
    until("the first host saves the project", || {
        host.first.savers.running() == 1
    });

    host.first.killed();
    host.connection.restarting();
    let next = host.replaced();

    until("the new host saves the project", || {
        next.savers.running() == 1
    });
    assert_eq!(next.savers.started()[0].1, host.root);
}

pub(crate) fn a_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap()
}

#[test]
fn a_call_that_cannot_wait_never_waits_behind_another_roots_link_being_opened() {
    let runtime = a_runtime();
    let hosting = Hosting::running_with(
        crate::sessionhost::Composes::as_the_tests_ship(),
        StandIns::default(),
    );
    let (connection, window) = in_memory(hosting, &runtime);
    let ((_one, open), (_two, opening)) = (a_project(), a_project());
    connection
        .of_project(&open, PATIENCE)
        .expect("the first root's link opens");
    // From now on another link opens only once it is let go.
    let (entered, dialed) = std::sync::mpsc::channel();
    let (let_go, go) = std::sync::mpsc::channel::<()>();
    let go = Mutex::new(go);
    connection.ready(
        window,
        Arc::new(move |within, _naming| {
            let _ = entered.send(());
            let _ = go.lock().unwrap().recv_timeout(within);
            Err("held up".to_owned())
        }),
    );
    let dialer = {
        let (connection, opening) = (Arc::clone(&connection), opening.clone());
        std::thread::spawn(move || connection.of_project(&opening, PATIENCE).map(|_| ()))
    };
    dialed
        .recv_timeout(PATIENCE)
        .expect("the second root's link is being opened");

    let asked = Instant::now();
    let already = connection.of_project(&open, Duration::ZERO);
    let not_yet = connection.of_project(&opening, Duration::ZERO);
    let waited = asked.elapsed();

    let _ = let_go.send(());
    assert!(dialer.join().unwrap().is_err());
    assert!(already.is_ok(), "the open link was not answered");
    assert!(not_yet.is_err(), "a link still opening was answered");
    assert!(
        waited < Duration::from_secs(1),
        "it waited {waited:?} behind another root's link"
    );
}

#[test]
fn a_link_that_opens_after_nobody_waits_for_it_is_closed() {
    let runtime = a_runtime();
    let hosting = Hosting::running_with(
        crate::sessionhost::Composes::as_the_tests_ship(),
        StandIns::default(),
    );
    let (closed, ends) = std::sync::mpsc::channel();

    let opened = open_within(runtime.handle(), Duration::from_millis(50), async move {
        tokio::time::sleep(Duration::from_millis(300)).await;
        a_link_to(hosting, move || {
            let _ = closed.send(());
        })
        .await
    });

    assert!(opened.is_err(), "the link opened in time");
    ends.recv_timeout(PATIENCE)
        .expect("the link that opened late is still open");
}

// ---------------------------------------------------------------------------------------
// a window that comes back to a host that outlived it (#2006)
// ---------------------------------------------------------------------------------------

/// One host whose programs and confinements are stand-ins, and windows that come and go on it,
/// each over a link a test cuts as a crash of the window would.
struct Comeback {
    hosting: Hosting,
    runs: StandIns,
    /// Its browsers (#2025).
    browsers: purlis_host::standin::StandInBrowsers,
    confines: purlis_host::standin::StandInConfines,
    root: std::path::PathBuf,
    runtime: tokio::runtime::Runtime,
    _dir: tempfile::TempDir,
    _store: tempfile::TempDir,
}

/// A window on [`Comeback`]'s host: its chats, its browsers, and what cuts each of its links.
struct AWindow {
    sessions: HostedSessions,
    connection: Arc<Connection>,
    cuts: Arc<Mutex<Vec<tokio::task::AbortHandle>>>,
}

impl Comeback {
    fn new() -> Comeback {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();
        let runs = StandIns::default();
        let confines = purlis_host::standin::StandInConfines::default();
        let (dir, root) = a_project();
        let store = tempfile::tempdir().unwrap();
        let browsers =
            purlis_host::standin::StandInBrowsers::in_folder(store.path().join("browsers"));
        Comeback {
            hosting: Hosting::confining(
                crate::sessionhost::Composes::as_the_tests_ship(),
                confines.clone(),
                runs.clone(),
            )
            .with_browsers(browsers.clone()),
            runs,
            browsers,
            confines,
            root,
            runtime,
            _dir: dir,
            _store: store,
        }
    }

    fn window(&self) -> AWindow {
        let cuts: Arc<Mutex<Vec<tokio::task::AbortHandle>>> = Arc::default();
        let link = self
            .runtime
            .block_on(crate::behaviour::a_cuttable_link(
                self.hosting.clone(),
                Arc::clone(&cuts),
            ))
            .unwrap();
        let dial: Dial = {
            let (hosting, cuts, handle) = (
                self.hosting.clone(),
                Arc::clone(&cuts),
                self.runtime.handle().clone(),
            );
            Arc::new(move |within, _naming| {
                open_within(
                    &handle,
                    within,
                    crate::behaviour::a_cuttable_link(hosting.clone(), Arc::clone(&cuts)),
                )
            })
        };
        let connection = Arc::new(Connection::default());
        connection.ready(Arc::new(link), dial);
        AWindow {
            sessions: HostedSessions::new(Arc::clone(&connection), &self.root),
            connection,
            cuts,
        }
    }
}

impl AWindow {
    /// The window crashes: every link of its is cut, and nothing of it says goodbye.
    fn crashes(self) {
        for cut in self.cuts.lock().unwrap().iter() {
            cut.abort();
        }
        std::mem::forget(self.sessions);
    }
}

#[test]
fn a_chat_left_running_is_reattached_under_its_number_and_drawn_as_it_is_now() {
    let host = Comeback::new();
    let first = host.window();
    first
        .sessions
        .open(Some(3), &a_shell("true"), &|_| {})
        .unwrap();
    let program = host.runs.last();
    program.writes(b"before. ");
    first.crashes();
    program.writes(b"while away. ");

    let back = host.window();
    assert!(back.sessions.left_running(3));
    assert!(!back.sessions.left_running(4));
    let announced = Mutex::new(Vec::new());
    let id = back
        .sessions
        .reattach(3, &|id| announced.lock().unwrap().push(id))
        .expect("it is reattached");

    assert_eq!(id, 3);
    assert_eq!(*announced.lock().unwrap(), [3]);
    assert_eq!(back.sessions.running(), [3]);
    assert_eq!(back.sessions.process_id(3), Some(4242));
    assert!(!back.sessions.left_running(3), "reattached once");
    let (sink, seen) = a_pane();
    back.sessions.watch(3, sink).expect("a view opens");
    until("the screen as it is now is drawn", || {
        seen.lock().unwrap().contains("before. while away. ")
    });
    back.sessions.input(3, b"typed").unwrap();
    until("the typing reaches it", || program.seen().typed == b"typed");
    assert_eq!(host.runs.ran().len(), 1, "nothing was started again");
}

#[test]
fn a_chat_that_ended_with_no_window_there_is_reattached_and_told_ended() {
    let host = Comeback::new();
    let first = host.window();
    first
        .sessions
        .open(Some(2), &a_shell("true"), &|_| {})
        .unwrap();
    let program = host.runs.last();
    first.crashes();
    program.ends(Exit::Code(3));

    let back = host.window();
    let ends = Arc::new(Mutex::new(Vec::new()));
    back.sessions.when_one_ends({
        let ends = Arc::clone(&ends);
        Arc::new(move |id, exit| ends.lock().unwrap().push((id, exit)))
    });
    assert!(back.sessions.left_running(2));
    back.sessions
        .reattach(2, &|_| {})
        .expect("it is reattached");

    until("it is told ended", || !ends.lock().unwrap().is_empty());
    assert_eq!(
        *ends.lock().unwrap(),
        [(2, purlis_core::session::Exit::Code(3))]
    );
    assert_eq!(back.sessions.process_id(2), None);
    assert_eq!(back.sessions.running(), [2], "its tab stays");
}

#[test]
fn what_no_put_back_reattached_is_closed_and_what_it_did_runs_on() {
    let host = Comeback::new();
    let first = host.window();
    for number in [1, 2] {
        first
            .sessions
            .open(Some(number), &a_shell("true"), &|_| {})
            .unwrap();
    }
    let (kept, left) = (host.runs.ran()[0].1.clone(), host.runs.ran()[1].1.clone());
    first.crashes();

    let back = host.window();
    back.sessions
        .reattach(1, &|_| {})
        .expect("it is reattached");
    back.sessions.close_what_was_not_reattached();

    until("the one nothing reattached is ended", || {
        left.seen().stopped
    });
    assert!(!kept.seen().stopped);
    assert!(!back.sessions.left_running(2));
    assert_eq!(back.sessions.running(), [1]);
}

#[test]
fn a_chats_confinement_is_reattached_and_its_proxy_heard_by_the_window_that_came_back() {
    use purlis_core::sandbox::egress::{PLAIN_PORTS, Refusals, TUNNEL_PORTS};
    use purlis_core::sandbox::plan::{Beside, Hearing, Plan};
    use purlis_core::sandbox::reach::Reach;
    let refused_by = |heard: &Arc<Mutex<Vec<String>>>| Hearing {
        refusals: Refusals::telling({
            let heard = Arc::clone(heard);
            Arc::new(move |host: &str, port: u16| {
                heard.lock().unwrap().push(format!("{host}:{port}"));
            })
        }),
        more: None,
        reached: None,
        asks: None,
    };
    let plan = Plan {
        reach: Reach::of(Vec::new()).written(),
        tunnel_ports: TUNNEL_PORTS.to_vec(),
        plain_ports: PLAIN_PORTS.to_vec(),
        any_port: false,
        beside: Beside::Wrapped,
        given: Vec::new(),
    };
    let host = Comeback::new();
    let first = host.window();
    let heard_first = Arc::new(Mutex::new(Vec::new()));
    let confined = first
        .sessions
        .confine(&plan, refused_by(&heard_first))
        .expect("confined");
    let id = confined.held_as().expect("held in the host").to_owned();
    first
        .sessions
        .open(
            Some(4),
            &Opening {
                confinement: Some(id.clone()),
                ..a_shell("true")
            },
            &|_| {},
        )
        .unwrap();
    std::mem::forget(confined);
    first.crashes();

    let back = host.window();
    let heard_back = Arc::new(Mutex::new(Vec::new()));
    let again = back
        .sessions
        .confined_again(4, refused_by(&heard_back))
        .expect("it runs in one")
        .expect("it is reattached");
    host.confines.proxy_did(
        &id,
        purlis_session_protocol::session::SandboxTold::Refused {
            host: "tracker.example.com".into(),
            port: 443,
        },
    );

    assert_eq!(again.held_as(), Some(id.as_str()));
    until("the window that came back hears its proxy", || {
        !heard_back.lock().unwrap().is_empty()
    });
    assert_eq!(*heard_back.lock().unwrap(), ["tracker.example.com:443"]);
    assert!(!host.confines.ended(&id), "it runs on");
    assert!(
        back.sessions
            .confined_again(5, refused_by(&heard_back))
            .is_none()
    );
}

/// A window that comes back puts its record back as a launch does (#2006): each chat the host
/// still runs is reattached under its number and none is started again, a chat the host no
/// longer has is started, and what the host runs that the record does not name is closed.
#[test]
fn a_record_put_back_reattaches_what_the_host_still_runs_and_starts_the_rest() {
    let host = Comeback::new();
    let first = host.window();
    for number in [1, 2] {
        first
            .sessions
            .open(Some(number), &a_shell("true"), &|_| {})
            .unwrap();
    }
    let (kept, left) = (host.runs.ran()[0].1.clone(), host.runs.ran()[1].1.clone());
    first.crashes();
    let back = host.window();
    let chats =
        crate::chats::Chats::on_host(Box::new(|_| {}), Box::new(back.sessions), host.root.clone());
    let chat = |number: u32| purlis_core::reopen::Chat {
        program: "/bin/sh".to_owned(),
        args: vec!["-c".to_owned(), "true".to_owned()],
        name: format!("chat {number}"),
        number: Some(number),
        identity: purlis_core::reopen::Identity {
            id: Some(format!("01K6E8ZK6V4Q9T0N3M2B1C5D{number:02}")),
            ..Default::default()
        },
        ..Default::default()
    };
    let record = purlis_core::reopen::Record {
        chats: vec![chat(1), chat(3)],
        ..Default::default()
    };

    let open = chats.put_back(&record, SIZE);
    chats.close_what_was_not_reattached();

    let mut sessions: Vec<u32> = open.iter().map(|one| one.session).collect();
    sessions.sort_unstable();
    assert_eq!(sessions, [1, 3]);
    assert_eq!(host.runs.ran().len(), 3, "only chat 3 was started");
    until("the one the record does not name is ended", || {
        left.seen().stopped
    });
    assert!(!kept.seen().stopped, "the reattached one runs on");
}

/// Composes a start with no launch as a host's binary does, in a confinement it asks the host
/// for, and runs a stand-in (#2171).
struct ComposesConfined;

impl purlis_host::Compose for ComposesConfined {
    fn compose(
        &self,
        starting: &purlis_host::ToCompose<'_>,
    ) -> Result<purlis_core::session::Spec, String> {
        starting.beside.confine(&session::ConfinePlan {
            listed: Vec::new(),
            never: Vec::new(),
            withdrawn: Vec::new(),
            anything: false,
            tunnel_ports: vec![443],
            plain_ports: vec![80],
            any_port: false,
            beside: session::Beside::Wrapped,
            given: vec![5432],
        })?;
        Ok(purlis_core::session::Spec::new("/bin/true", starting.size))
    }
}

pub(crate) fn by_its_names() -> Named {
    Named {
        chat: None,
        run: None,
        profile: Some("work".to_owned()),
        persona: Some("steward".to_owned()),
        workspace: None,
        resume: None,
        first_message: None,
        without_sandbox: false,
        alone: Vec::new(),
        size: SIZE,
    }
}

/// **A chat the host composes by its names is heard by the window, and its asks answered from
/// it** (#2407): what its proxy refused and asks about reaches the window's hearing, the
/// window's answer and a port withdrawn reach its confinement, and closing it lets the host end
/// it, as for a confinement the window planned.
#[test]
fn a_chat_the_host_composes_by_its_names_is_heard_by_the_window_and_answered_from_it() {
    use purlis_core::sandbox::asks::Heard;
    use purlis_core::sandbox::egress::Refusals;
    use purlis_core::sandbox::hosts::Host;
    use purlis_core::sandbox::plan::{Asking, Hearing};
    use purlis_core::sandbox::reach::By;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let confines = purlis_host::standin::StandInConfines::default();
    let hosting = Hosting::confining(ComposesConfined, confines.clone(), StandIns::default());
    let (connection, _link) = in_memory(hosting, &runtime);
    let (_dir, root) = a_project();
    let sessions = HostedSessions::new(connection, &root);
    let refused = Arc::new(Mutex::new(Vec::new()));
    let asked = Arc::new(Mutex::new(Vec::new()));
    let hearing = Hearing {
        refusals: Refusals::telling({
            let refused = Arc::clone(&refused);
            Arc::new(move |host: &str, port: u16| {
                refused.lock().unwrap().push(format!("{host}:{port}"));
            })
        }),
        more: None,
        reached: None,
        asks: Some(Asking {
            tell: {
                let asked = Arc::clone(&asked);
                Arc::new(move |heard: Heard| {
                    if let Heard::Asked(targets) = heard {
                        asked.lock().unwrap().extend(targets);
                    }
                })
            },
            askable: Arc::new(|| true),
            known: Arc::new(|_| None),
        }),
    };

    let crate::host::Composed {
        confinement, ran, ..
    } = sessions
        .open_named(None, &by_its_names(), hearing, &|_| {})
        .expect("composed in the host");
    // How the host applied the sandbox to the run it composed, as its start answered (#2692).
    assert_eq!(ran, Some(session::RunSandbox::By("proxy".to_owned())));
    let confinement = confinement.expect("named to the window");
    let id = confinement.held_as().expect("held in the host").to_owned();
    confines.proxy_did(
        &id,
        session::SandboxTold::Refused {
            host: "tracker.example.com".into(),
            port: 443,
        },
    );
    confines.proxy_did(
        &id,
        session::SandboxTold::Asked {
            targets: vec!["pypi.example.com:443".into()],
        },
    );
    until("the window hears its proxy", || {
        !refused.lock().unwrap().is_empty() && !asked.lock().unwrap().is_empty()
    });
    confinement
        .asks()
        .expect("its asks are answered from the window")
        .allow(&Host::parse("pypi.example.com").unwrap(), By::Chat);
    confinement
        .withdraw_port(5432)
        .expect("withdrawn in the host");

    assert_eq!(*refused.lock().unwrap(), ["tracker.example.com:443"]);
    assert_eq!(*asked.lock().unwrap(), ["pypi.example.com:443"]);
    assert_eq!(
        confines.seen()[0].changes,
        [
            session::ReachChange::Allow {
                host: "pypi.example.com".into(),
                layer: session::Layer::Chat,
            },
            session::ReachChange::WithdrawPort { port: 5432 },
        ]
    );
    drop(confinement);
    until("the host ends it", || confines.ended(&id));
}

/// A chat started by its names whose hearing asks nobody has its confinement named to no link:
/// the window holds none of it (#2407).
#[test]
fn a_chat_started_by_its_names_with_a_hearing_that_asks_nobody_names_nothing_to_the_window() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let confines = purlis_host::standin::StandInConfines::default();
    let hosting = Hosting::confining(ComposesConfined, confines.clone(), StandIns::default());
    let (connection, _link) = in_memory(hosting, &runtime);
    let (_dir, root) = a_project();
    let sessions = HostedSessions::new(connection, &root);

    let crate::host::Composed { confinement, .. } = sessions
        .open_named(
            None,
            &by_its_names(),
            purlis_core::sandbox::plan::Hearing::default(),
            &|_| {},
        )
        .expect("composed in the host");

    assert!(confinement.is_none());
    assert!(!confines.seen()[0].confine.asks);
}

/// **A chat started by its names tells the host the services the person allowed it alone**
/// (#2590), where the host's confinement asks this window about each it leaves out; and none
/// where nobody would be asked.
#[test]
fn a_chat_started_by_its_names_tells_the_host_what_was_allowed_it_alone_where_it_asks_the_window() {
    use purlis_core::sandbox::plan::{Asking, Hearing};
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let confines = purlis_host::standin::StandInConfines::default();
    let hosting = Hosting::confining(ComposesConfined, confines.clone(), StandIns::default());
    let (connection, _link) = in_memory(hosting, &runtime);
    let (_dir, root) = a_project();
    let sessions = HostedSessions::new(connection, &root);
    let alone = Named {
        alone: vec![6379],
        ..by_its_names()
    };
    let asking = || Hearing {
        asks: Some(Asking {
            tell: Arc::new(|_| {}),
            askable: Arc::new(|| true),
            known: Arc::new(|_| None),
        }),
        ..Hearing::default()
    };

    let crate::host::Composed { confinement, .. } = sessions
        .open_named(None, &alone, asking(), &|_| {})
        .expect("composed in the host");
    let crate::host::Composed {
        confinement: unasked,
        ..
    } = sessions
        .open_named(None, &alone, Hearing::default(), &|_| {})
        .expect("composed in the host");

    assert!(confinement.is_some());
    assert!(unasked.is_none());
    let seen = confines.seen();
    assert_eq!(seen[0].planned, purlis_host::Planned::Composed);
    assert_eq!(seen[0].alone, [6379]);
    assert!(seen[1].alone.is_empty(), "nobody would be asked about it");
}

/// A `local-ui` link to `hosting` over a line `lines` can cut.
async fn a_cuttable_link_to<H: session::Host>(
    hosting: H,
    lines: purlis_host::standin::Lines,
) -> Result<Link, String> {
    use purlis_session_protocol::auth::{Credentials, Scope};
    use purlis_session_protocol::link;

    static HELD: std::sync::LazyLock<Credentials> =
        std::sync::LazyLock::new(|| Credentials::mint().expect("credentials"));

    let (ours, theirs) = lines.line();
    let (client, served) = tokio::join!(
        link::connect(
            ours,
            session::speaks(),
            Scope::LocalUi,
            HELD.of(Scope::LocalUi)
        ),
        link::serve_any(theirs, session::speaks(), &HELD)
    );
    let served = served.map_err(|why| why.to_string())?;
    tokio::spawn(session::serve(served, hosting, None));
    client.map_err(|why| why.to_string())
}

/// A host between the window and `inner`, which says what `inner` pushes, and what the test
/// pushes through `pushes`, and leaves out a `link_back` where it is told to: a window that
/// missed one. It keeps which of its subscriptions, numbered from 0 as they were made, the
/// window let go of.
#[derive(Clone)]
pub(crate) struct Between<H> {
    inner: H,
    drops_link_back: bool,
    pub(crate) pushes: tokio::sync::broadcast::Sender<session::Pushed>,
    subscriptions: Arc<Mutex<(usize, std::collections::BTreeSet<usize>)>>,
}

impl<H> Between<H> {
    pub(crate) fn new(inner: H, drops_link_back: bool) -> Self {
        Between {
            inner,
            drops_link_back,
            pushes: tokio::sync::broadcast::channel(4096).0,
            subscriptions: Arc::default(),
        }
    }

    /// Whether the window let go of subscription `number`.
    fn let_go(&self, number: usize) -> bool {
        self.subscriptions.lock().unwrap().1.contains(&number)
    }
}

impl<H: session::Host + Clone + Send + Sync + 'static> session::Host for Between<H> {
    async fn call(
        &self,
        command: session::Command,
        peer: &session::Peer,
    ) -> Result<Answer, session::Refusal> {
        self.inner.call(command, peer).await
    }

    async fn elicits_a_secret(&self, chat: &str, ask: &str) -> bool {
        self.inner.elicits_a_secret(chat, ask).await
    }

    async fn link_ended(&self, peer: &session::Peer) {
        self.inner.link_ended(peer).await;
    }

    async fn subscribe(
        &self,
        since: session::Since,
        peer: &session::Peer,
    ) -> Result<tokio::sync::mpsc::Receiver<session::Pushed>, session::Refusal> {
        let mut from = self.inner.subscribe(since, peer).await?;
        let mut pushed_here = self.pushes.subscribe();
        let drops_link_back = self.drops_link_back;
        let subscriptions = Arc::clone(&self.subscriptions);
        let number = {
            let mut subscriptions = subscriptions.lock().unwrap();
            subscriptions.0 += 1;
            subscriptions.0 - 1
        };
        let (tell, told) = tokio::sync::mpsc::channel(8192);
        tokio::spawn(async move {
            loop {
                let pushed = tokio::select! {
                    () = tell.closed() => break,
                    pushed = from.recv() => pushed,
                    pushed = pushed_here.recv() => pushed.ok(),
                };
                let Some(pushed) = pushed else { return };
                if drops_link_back && pushed == (session::Pushed::LinkBack {}) {
                    continue;
                }
                if tell.send(pushed).await.is_err() {
                    break;
                }
            }
            subscriptions.lock().unwrap().1.insert(number);
        });
        Ok(told)
    }
}

/// A runner whose programs are stand-ins, `runs`, reached through this machine's host, which
/// relays to it (#2042): each link the relay dials runs over a line `lines` cuts.
fn a_relay_to_a_runner(runs: &StandIns, lines: &purlis_host::standin::Lines) -> purlis_host::Relay {
    let runner = Hosting::running_with(
        crate::sessionhost::Composes::as_the_tests_ship(),
        runs.clone(),
    );
    let lines = lines.clone();
    purlis_host::Relay::new(Arc::new(move || {
        Box::pin(a_cuttable_link_to(runner.clone(), lines.clone()))
    }))
}

/// A project's chats in `host`, whose programs are `runs`, over in-memory links.
fn hosted_in<H: session::Host + Clone>(host: H, runs: StandIns) -> Hosted {
    let runtime = a_runtime();
    let (connection, link) = in_memory(host, &runtime);
    let (dir, root) = a_project();
    Hosted {
        sessions: HostedSessions::new(connection, &root),
        link,
        runs,
        savers: Savers::default(),
        root,
        _dir: dir,
        _runtime: runtime,
    }
}

/// Which chats the window was told are reconnecting or reconnected, in the order it was told.
type Reconnects = Arc<Mutex<Vec<(Vec<u32>, Reconnection)>>>;

fn told_of_reconnecting(hosted: &Hosted) -> Reconnects {
    let told: Reconnects = Arc::default();
    hosted.sessions.when_reconnecting({
        let told = Arc::clone(&told);
        Arc::new(move |chats, reconnection| told.lock().unwrap().push((chats, reconnection)))
    });
    told
}

/// Every chat the window was told is reconnected, whichever word said it.
fn reconnected(told: &Reconnects) -> std::collections::BTreeSet<u32> {
    told.lock()
        .unwrap()
        .iter()
        .filter(|(_, reconnection)| *reconnection == Reconnection::Reconnected)
        .flat_map(|(chats, _)| chats.iter().copied())
        .collect()
}

/// A chat whose pane is drawn, its program having written `ready> `.
fn a_drawn_chat(hosted: &Hosted) -> (u32, Arc<Mutex<String>>) {
    let id = hosted
        .sessions
        .open(None, &a_shell("true"), &|_| {})
        .unwrap();
    hosted.runs.last().writes(b"ready> ");
    let (sink, seen) = a_pane();
    hosted.sessions.watch(id, sink).expect("a view opens");
    until("the screen is drawn", || {
        seen.lock().unwrap().contains("ready> ")
    });
    (id, seen)
}

#[test]
fn every_chat_on_a_runner_is_reconnecting_while_its_link_is_down_and_each_is_reconnected_after() {
    let (runs, lines) = (StandIns::default(), purlis_host::standin::Lines::default());
    let hosted = hosted_in(a_relay_to_a_runner(&runs, &lines), runs);
    let told = told_of_reconnecting(&hosted);
    let (typed_in, seen) = a_drawn_chat(&hosted);
    let ends = hosted
        .sessions
        .open(None, &a_shell("true"), &|_| {})
        .unwrap();
    let ending = hosted.runs.last();

    lines.cut();
    until(
        "every chat of the project is said to be reconnecting",
        || *told.lock().unwrap() == [(vec![typed_in, ends], Reconnection::Reconnecting)],
    );
    // A pane drawn now starts dimmed.
    assert_eq!(hosted.sessions.reconnecting(), [typed_in, ends]);
    // One chat's program ends while the link is down.
    ending.ends(Exit::Code(0));

    // What the window sends next links again, and the chat draws again from its snapshot.
    until("the runner is linked again", || {
        hosted.sessions.input(typed_in, b"typed").is_ok()
    });
    until("each chat is said to be reconnected", || {
        reconnected(&told) == [typed_in, ends].into()
    });
    assert!(hosted.sessions.reconnecting().is_empty());
    until("the screen is drawn again", || {
        seen.lock().unwrap().ends_with("ready> typed")
    });
}

#[test]
fn a_chat_whose_link_back_went_missing_is_reconnected_once_it_draws_again() {
    let (runs, lines) = (StandIns::default(), purlis_host::standin::Lines::default());
    let hosted = hosted_in(Between::new(a_relay_to_a_runner(&runs, &lines), true), runs);
    let told = told_of_reconnecting(&hosted);
    let (id, _seen) = a_drawn_chat(&hosted);

    lines.cut();
    until("the chat is said to be reconnecting", || {
        *told.lock().unwrap() == [(vec![id], Reconnection::Reconnecting)]
    });
    until("the runner is linked again", || {
        hosted.sessions.input(id, b"typed").is_ok()
    });

    until(
        "the chat is said to be reconnected as it draws again",
        || reconnected(&told) == [id].into(),
    );
    assert!(hosted.sessions.reconnecting().is_empty());
}

#[test]
fn a_chat_with_no_pane_while_its_link_was_down_is_reconnected_once_a_pane_draws_it() {
    let (runs, lines) = (StandIns::default(), purlis_host::standin::Lines::default());
    let hosted = hosted_in(Between::new(a_relay_to_a_runner(&runs, &lines), true), runs);
    let told = told_of_reconnecting(&hosted);
    let id = hosted
        .sessions
        .open(None, &a_shell("true"), &|_| {})
        .unwrap();
    hosted.runs.last().writes(b"ready> ");

    lines.cut();
    until("the chat is said to be reconnecting", || {
        *told.lock().unwrap() == [(vec![id], Reconnection::Reconnecting)]
    });
    until("the runner is linked again", || {
        hosted.sessions.input(id, b"typed").is_ok()
    });
    // Its link_back went missing, and no view of it drew: it is still dimmed.
    assert_eq!(hosted.sessions.reconnecting(), [id]);

    let (sink, seen) = a_pane();
    hosted.sessions.watch(id, sink).expect("a view opens");
    until("the screen is drawn", || {
        seen.lock().unwrap().contains("ready> ")
    });
    until("the chat is said to be reconnected", || {
        reconnected(&told) == [id].into()
    });
    assert!(hosted.sessions.reconnecting().is_empty());
}

#[test]
fn a_window_that_fell_behind_its_host_leaves_no_chat_reconnecting() {
    let runs = StandIns::default();
    let host = Between::new(
        Hosting::running_with(
            crate::sessionhost::Composes::as_the_tests_ship(),
            runs.clone(),
        ),
        false,
    );
    let (pushes, between) = (host.pushes.clone(), host.clone());
    let hosted = hosted_in(host, runs);
    // The window takes its time over being told the link is down: meanwhile its host pushes
    // more than it holds, so it falls behind and subscribes again from now.
    let (go_on, held) = std::sync::mpsc::channel::<()>();
    let held = Mutex::new(held);
    let told: Reconnects = Arc::default();
    hosted.sessions.when_reconnecting({
        let told = Arc::clone(&told);
        Arc::new(move |chats, reconnection| {
            if reconnection == Reconnection::Reconnecting {
                // Off the worker it holds, so the link goes on reading meanwhile.
                tokio::task::block_in_place(|| {
                    let _ = held.lock().unwrap().recv_timeout(PATIENCE);
                });
            }
            told.lock().unwrap().push((chats, reconnection));
        })
    });
    let (id, _seen) = a_drawn_chat(&hosted);

    pushes
        .send(session::Pushed::LinkDown {
            why: "the link to it ended".into(),
            key_changed: false,
        })
        .unwrap();
    for seq in 1..=3000 {
        let _ = pushes.send(session::Pushed::Missed {
            device: "01J9ZZDEVICE0000000000000".into(),
            after: seq,
            resumes_at: seq + 1,
        });
    }
    // The project's link, the window's second, has let go of its subscription: it fell behind.
    until("the window falls behind", || between.let_go(1));
    go_on.send(()).unwrap();

    until("the chat is said to be reconnected", || {
        reconnected(&told) == [id].into()
    });
    assert!(hosted.sessions.reconnecting().is_empty());
}

/// A runner that answered the relay with another key than the one pinned for it (#2752): its
/// chats are dimmed as for any outage, and the window is told the key changed, so it says why.
#[test]
fn a_runner_whose_key_changed_leaves_its_chats_dimmed_and_says_so() {
    let runs = StandIns::default();
    let host = Between::new(
        Hosting::running_with(
            crate::sessionhost::Composes::as_the_tests_ship(),
            runs.clone(),
        ),
        false,
    );
    let pushes = host.pushes.clone();
    let hosted = hosted_in(host, runs);
    let told = told_of_reconnecting(&hosted);
    let (id, _seen) = a_drawn_chat(&hosted);

    pushes
        .send(session::Pushed::LinkDown {
            why: "the runner answered with another key".into(),
            key_changed: true,
        })
        .unwrap();
    until("the window is told the runner's key changed", || {
        *told.lock().unwrap()
            == [
                (vec![id], Reconnection::Reconnecting),
                (vec![id], Reconnection::KeyChanged),
            ]
    });
    assert_eq!(hosted.sessions.reconnecting(), [id]);
}

/// The window's link to its own host, which relays it to a runner whose host composes each
/// harness chat in a confinement of its own (#2446): the window's sessions there, the runner's
/// confinements, and what the runner ran.
fn on_a_runner() -> (
    HostedSessions,
    purlis_host::standin::StandInConfines,
    StandIns,
    tempfile::TempDir,
    tokio::runtime::Runtime,
) {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let confines = purlis_host::standin::StandInConfines::default();
    let runs = StandIns::default();
    let runner = Hosting::confining(ComposesConfined, confines.clone(), runs.clone());
    let relay = runtime.block_on(async move {
        purlis_host::Relay::new(Arc::new(move || {
            let runner = runner.clone();
            Box::pin(async move { Ok(crate::behaviour::linked_to(runner).await) })
        }))
    });
    let (connection, _link) = in_memory(relay, &runtime);
    let (dir, root) = a_project();
    let sessions = HostedSessions::on_a_runner(connection, &root);
    (sessions, confines, runs, dir, runtime)
}

/// **A harness chat on a runner is started there by its names, and its proxy's asks reach the
/// window** (#2446): the runner composes it and names its confinement to the window, through
/// the link its host relays; the window's answer and a port withdrawn reach it there, and
/// closing the chat lets the runner end it. The window's own launch of a harness is not sent to
/// a runner; a shell still starts from it.
#[test]
fn a_harness_chat_on_a_runner_is_started_there_by_its_names_and_its_asks_reach_the_window() {
    use purlis_core::sandbox::asks::Heard;
    use purlis_core::sandbox::hosts::Host;
    use purlis_core::sandbox::plan::{Asking, Hearing};
    use purlis_core::sandbox::reach::By;
    let (sessions, confines, runs, _dir, _runtime) = on_a_runner();
    let asked = Arc::new(Mutex::new(Vec::new()));
    let hearing = Hearing {
        asks: Some(Asking {
            tell: {
                let asked = Arc::clone(&asked);
                Arc::new(move |heard: Heard| {
                    if let Heard::Asked(targets) = heard {
                        asked.lock().unwrap().extend(targets);
                    }
                })
            },
            askable: Arc::new(|| true),
            known: Arc::new(|_| None),
        }),
        ..Hearing::default()
    };
    assert!(sessions.composes_its_harness_chats());

    let crate::host::Composed {
        number: id,
        confinement,
        ..
    } = sessions
        .open_named(None, &by_its_names(), hearing, &|_| {})
        .expect("composed on the runner");
    let confinement = confinement.expect("named to the window");
    let held = confinement
        .held_as()
        .expect("held on the runner")
        .to_owned();
    confines.proxy_did(
        &held,
        session::SandboxTold::Asked {
            targets: vec!["pypi.example.com:443".into()],
        },
    );
    until("the window is asked", || !asked.lock().unwrap().is_empty());
    confinement
        .asks()
        .expect("its asks are answered from the window")
        .allow(&Host::parse("pypi.example.com").unwrap(), By::Chat);
    confinement
        .withdraw_port(5432)
        .expect("withdrawn on the runner");
    until("the runner hears the window", || {
        confines.seen()[0].changes.len() == 2
    });

    assert_eq!(*asked.lock().unwrap(), ["pypi.example.com:443"]);
    assert_eq!(
        confines.seen()[0].changes,
        [
            session::ReachChange::Allow {
                host: "pypi.example.com".into(),
                layer: session::Layer::Chat,
            },
            session::ReachChange::WithdrawPort { port: 5432 },
        ]
    );
    drop(confinement);
    sessions.close(id).expect("it closes");
    until("the runner ends it", || confines.ended(&held));

    let opted_out = Named {
        without_sandbox: true,
        ..by_its_names()
    };
    let refused = sessions.open_named(None, &opted_out, Default::default(), &|_| {});
    assert_eq!(refused.err().as_deref(), Some(SANDBOXED_ON_A_RUNNER));
    let mut harness = a_shell("true");
    harness.harness = Some(purlis_core::harness::Harness::ClaudeCode);
    let refused = sessions.open(None, &harness, &|_| {});
    assert_eq!(refused, Err(COMPOSED_ON_THE_RUNNER.to_owned()));
    let ran = runs.ran().len();
    sessions
        .open(None, &a_shell("true"), &|_| {})
        .expect("a shell starts from the window's launch");
    until("the runner runs the shell", || runs.ran().len() == ran + 1);
}

/// A host between the window and `inner` that counts the holds the window sends, and refuses
/// each, in a runner's words, while the test says so.
#[derive(Clone)]
struct Holds<H> {
    inner: H,
    tried: Arc<Mutex<usize>>,
    refusing: Arc<std::sync::atomic::AtomicBool>,
}

/// What the runner says as it refuses a hold.
const NOT_THERE: &str = "that folder is not there";

impl<H> Holds<H> {
    fn new(inner: H) -> Self {
        Holds {
            inner,
            tried: Arc::default(),
            refusing: Arc::default(),
        }
    }

    fn tried(&self) -> usize {
        *self.tried.lock().unwrap()
    }
}

impl<H: session::Host + Clone + Send + Sync + 'static> session::Host for Holds<H> {
    async fn call(
        &self,
        command: session::Command,
        peer: &session::Peer,
    ) -> Result<Answer, session::Refusal> {
        if let session::Command::HoldProject { .. } = command {
            *self.tried.lock().unwrap() += 1;
            if self.refusing.load(Ordering::Relaxed) {
                return Err(session::Refusal {
                    code: session::NOT_ALLOWED.into(),
                    why: NOT_THERE.into(),
                });
            }
        }
        self.inner.call(command, peer).await
    }

    async fn link_ended(&self, peer: &session::Peer) {
        self.inner.link_ended(peer).await;
    }

    async fn subscribe(
        &self,
        since: session::Since,
        peer: &session::Peer,
    ) -> Result<tokio::sync::mpsc::Receiver<session::Pushed>, session::Refusal> {
        self.inner.subscribe(since, peer).await
    }
}

/// A project's chats whose host says what the test pushes, through `holds`, which counts the
/// window's holds: and the project's id, as its host knows it.
fn hosted_holding() -> (Hosted, Holds<Hosting>, Between<Holds<Hosting>>, String) {
    let runs = StandIns::default();
    let holds = Holds::new(Hosting::running_with(
        crate::sessionhost::Composes::as_the_tests_ship(),
        runs.clone(),
    ));
    let between = Between::new(holds.clone(), false);
    let hosted = hosted_in(between.clone(), runs);
    let project = purlis_core::scaffold::planefile::id_to_hold_by(&hosted.root).unwrap();
    (hosted, holds, between, project)
}

#[test]
fn a_project_a_re_link_could_not_hold_is_held_again_by_the_window() {
    let (hosted, holds, between, project) = hosted_holding();
    let (id, _seen) = a_drawn_chat(&hosted);
    let held = holds.tried();
    assert!(held > 0);

    between
        .pushes
        .send(session::Pushed::NotHeld {
            project: project.clone(),
            why: NOT_THERE.into(),
        })
        .unwrap();

    until("the window holds the project again", || {
        holds.tried() > held
    });
    // Its chats carry on, and a new one starts.
    hosted.sessions.input(id, b"typed").unwrap();
    hosted
        .sessions
        .open(None, &a_shell("true"), &|_| {})
        .expect("a chat starts in the project held again");
}

#[test]
fn a_project_the_runner_will_not_hold_again_is_said_to_be_not_held_in_the_runners_words() {
    let (hosted, holds, between, project) = hosted_holding();
    a_drawn_chat(&hosted);
    let held = holds.tried();
    holds.refusing.store(true, Ordering::Relaxed);

    between
        .pushes
        .send(session::Pushed::NotHeld {
            project,
            why: NOT_THERE.into(),
        })
        .unwrap();
    until("the window tries to hold the project again", || {
        holds.tried() > held
    });

    let refused = hosted
        .sessions
        .open(None, &a_shell("true"), &|_| {})
        .expect_err("no chat starts in a project the runner does not hold");
    assert!(refused.starts_with(NOT_HELD), "{refused}");
    assert!(refused.contains(NOT_THERE), "{refused}");
}

/// **A project the runner would not hold again is marked until a hold of it is taken** (#2846):
/// the Pools group draws the mark, in the runner's words.
#[test]
fn a_project_the_runner_will_not_hold_again_is_marked_until_it_is_held() {
    let (hosted, holds, between, project) = hosted_holding();
    a_drawn_chat(&hosted);
    let held = holds.tried();
    holds.refusing.store(true, Ordering::Relaxed);
    let marked = || {
        crate::notheld::now(None)
            .into_iter()
            .find(|one| std::path::Path::new(&one.root) == hosted.root)
    };
    assert_eq!(marked(), None);

    between
        .pushes
        .send(session::Pushed::NotHeld {
            project,
            why: NOT_THERE.into(),
        })
        .unwrap();
    until("the window tries to hold the project again", || {
        holds.tried() > held
    });
    until("the project is marked not held", || marked().is_some());
    let mark = marked().unwrap();
    assert!(mark.why.contains(NOT_THERE), "{mark:?}");
    assert!(
        !mark.why.starts_with(NOT_HELD),
        "the runner's words alone: {mark:?}"
    );

    // The runner holds it again: a chat starts, and the mark goes.
    holds.refusing.store(false, Ordering::Relaxed);
    hosted
        .sessions
        .open(None, &a_shell("true"), &|_| {})
        .expect("a chat starts in the project held again");
    assert_eq!(marked(), None);
}

#[test]
fn a_project_another_project_was_not_held_for_is_left_held() {
    let (hosted, holds, between, _project) = hosted_holding();
    a_drawn_chat(&hosted);
    let held = holds.tried();

    between
        .pushes
        .send(session::Pushed::NotHeld {
            project: "01J9PRJCTANOTHER0000000000".into(),
            why: NOT_THERE.into(),
        })
        .unwrap();
    std::thread::sleep(Duration::from_millis(300));
    hosted
        .sessions
        .open(None, &a_shell("true"), &|_| {})
        .expect("a chat starts");
    assert_eq!(holds.tried(), held);
}

/// Each command's word a host was told, in the order they came, with the name of each
/// `name_window`.
type HostTold = Arc<Mutex<Vec<(&'static str, Option<String>)>>>;

/// A host between the window and `inner` that keeps each name the window names itself by, and
/// the word of each command, in the order they came.
#[derive(Clone)]
struct Names<H> {
    inner: H,
    told: HostTold,
}

impl<H: session::Host + Clone + Send + Sync + 'static> session::Host for Names<H> {
    async fn call(
        &self,
        command: session::Command,
        peer: &session::Peer,
    ) -> Result<Answer, session::Refusal> {
        let name = match &command {
            session::Command::NameWindow { name } => Some(name.expose().to_owned()),
            _ => None,
        };
        self.told.lock().unwrap().push((command.word(), name));
        self.inner.call(command, peer).await
    }

    async fn link_ended(&self, peer: &session::Peer) {
        self.inner.link_ended(peer).await;
    }

    async fn subscribe(
        &self,
        since: session::Since,
        peer: &session::Peer,
    ) -> Result<tokio::sync::mpsc::Receiver<session::Pushed>, session::Refusal> {
        self.inner.subscribe(since, peer).await
    }
}

/// A window naming its links in `config`, on a host that keeps what it was told: its project's
/// chats, and what the host was told.
fn named_in(config: &Path, root: &Path) -> (HostedSessions, HostTold, tokio::runtime::Runtime) {
    let runtime = a_runtime();
    let host = Names {
        inner: Hosting::running_with(
            crate::sessionhost::Composes::as_the_tests_ship(),
            StandIns::default(),
        ),
        told: Arc::default(),
    };
    let told = Arc::clone(&host.told);
    let (connection, _link) = in_memory(host, &runtime);
    connection.naming_in(config.to_path_buf());
    (HostedSessions::new(connection, root), told, runtime)
}

#[test]
fn the_window_names_each_projects_link_first_by_the_name_it_keeps_across_restarts() {
    let config = tempfile::tempdir().unwrap();
    let (_dir, root) = a_project();
    let (sessions, told, _runtime) = named_in(config.path(), &root);
    sessions.hold().unwrap();
    let first = told.lock().unwrap().clone();
    let Some(("name_window", Some(name))) = first.first().cloned() else {
        panic!("the link's first command names the window: {first:?}");
    };
    assert_eq!(name.len(), 32);
    drop(sessions);

    // The window restarts: its new link names it the same way.
    let (sessions, told, _runtime) = named_in(config.path(), &root);
    sessions.hold().unwrap();
    assert_eq!(
        told.lock().unwrap().first().cloned(),
        Some(("name_window", Some(name.clone())))
    );

    // A window that keeps its names elsewhere names itself otherwise.
    let elsewhere = tempfile::tempdir().unwrap();
    let (sessions, told, _runtime) = named_in(elsewhere.path(), &root);
    sessions.hold().unwrap();
    let other = told.lock().unwrap().first().cloned();
    assert!(
        matches!(&other, Some(("name_window", Some(other))) if *other != name),
        "{other:?}"
    );
}

/// Everything logged while it is the thread's subscriber, as text.
#[derive(Clone, Default)]
struct Logged(Arc<Mutex<String>>);

impl tracing::field::Visit for Logged {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.0
            .lock()
            .unwrap()
            .push_str(&format!("{}={value:?}\n", field.name()));
    }
}

impl tracing::Subscriber for Logged {
    fn enabled(&self, _metadata: &tracing::Metadata<'_>) -> bool {
        true
    }

    fn new_span(&self, span: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        span.record(&mut self.clone());
        tracing::span::Id::from_u64(1)
    }

    fn record(&self, _span: &tracing::span::Id, values: &tracing::span::Record<'_>) {
        values.record(&mut self.clone());
    }

    fn record_follows_from(&self, _span: &tracing::span::Id, _follows: &tracing::span::Id) {}

    fn event(&self, event: &tracing::Event<'_>) {
        event.record(&mut self.clone());
    }

    fn enter(&self, _span: &tracing::span::Id) {}

    fn exit(&self, _span: &tracing::span::Id) {}
}

/// A host that refuses the window's name, as one that holds it for another link would.
#[derive(Clone)]
struct RefusesNames(Hosting);

impl session::Host for RefusesNames {
    async fn call(
        &self,
        command: session::Command,
        peer: &session::Peer,
    ) -> Result<Answer, session::Refusal> {
        if let session::Command::NameWindow { .. } = command {
            return Err(session::Refusal {
                code: session::NOT_ALLOWED.into(),
                why: "another window's link holds this name now".into(),
            });
        }
        self.0.call(command, peer).await
    }

    async fn subscribe(
        &self,
        since: session::Since,
        peer: &session::Peer,
    ) -> Result<tokio::sync::mpsc::Receiver<session::Pushed>, session::Refusal> {
        self.0.subscribe(since, peer).await
    }
}

#[test]
fn the_name_the_window_keeps_is_never_logged() {
    let config = tempfile::tempdir().unwrap();
    let (_dir, root) = a_project();
    let runtime = a_runtime();
    let (connection, _link) = in_memory(
        RefusesNames(Hosting::running_with(
            crate::sessionhost::Composes::as_the_tests_ship(),
            StandIns::default(),
        )),
        &runtime,
    );
    connection.naming_in(config.path().to_path_buf());
    let sessions = HostedSessions::new(connection, &root);

    let logged = Logged::default();
    tracing::subscriber::with_default(logged.clone(), || {
        sessions
            .hold()
            .expect("a refused name leaves the link as it was");
    });

    let name = purlis_core::windownames::name_for(config.path(), &root, &[]).unwrap();
    let logged = logged.0.lock().unwrap().clone();
    assert!(
        logged.contains("did not take the window's name"),
        "{logged}"
    );
    assert!(!logged.contains(name.expose()), "{logged}");
}

/// **A window that comes back takes back its runner chat** (#2623), over its own host's relay
/// as the window's binary links: each project root's link names the window first, before it
/// subscribes. The window that comes back names itself the same way on its new link, and acts on
/// its chat's confinement on the runner again; a window that names itself nowhere does not.
#[test]
fn a_window_that_comes_back_acts_on_its_runner_chats_confinement_again() {
    const CONFINEMENT: &str = "01J9CNFNMNT00000000000000W1";
    let runtime = a_runtime();
    let runner = Hosting::confining(
        ComposesConfined,
        purlis_host::standin::StandInConfines::default(),
        StandIns::default(),
    );
    let relay = runtime.block_on(async move {
        purlis_host::Relay::new(Arc::new(move || {
            let runner = runner.clone();
            Box::pin(async move { Ok(crate::behaviour::linked_to(runner).await) })
        }))
    });
    let config = tempfile::tempdir().unwrap();
    let (_dir, root) = a_project();
    let project = purlis_core::scaffold::planefile::id_to_hold_by(&root).unwrap();
    let hold = || session::Command::HoldProject {
        project: project.clone(),
        project_path: root.display().to_string(),
    };
    let reach = || session::Command::ReachOf {
        confinement: CONFINEMENT.into(),
    };
    // A window, naming its links or not, and its project root's link.
    let window = |named: bool| {
        let (connection, _link) = in_memory(relay.clone(), &runtime);
        if named {
            connection.naming_in(config.path().to_path_buf());
        }
        let link = connection.of_project(&root, PATIENCE).unwrap();
        link.call(hold()).expect("the runner holds the project");
        (connection, link)
    };

    let (first, link) = window(true);
    link.call(session::Command::StartProgram(Box::new(
        session::StartProgram {
            confinement: Some(CONFINEMENT.into()),
            ..session::StartProgram::of(&project, ProgramKind::Harness)
        },
    )))
    .expect("composed on the runner");
    link.call(reach())
        .expect("the window acts on its chat's confinement");
    // The window goes, and its links with it.
    drop((first, link));

    let (_unnamed, other) = window(false);
    assert!(
        other.call(reach()).is_err(),
        "a window that names itself nowhere takes nothing back"
    );
    let deadline = Instant::now() + PATIENCE;
    loop {
        // Until the host has let go of the link of the window that went, its name is held.
        let (_again, link) = window(true);
        if link.call(reach()).is_ok() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "the window that came back never took its chat back"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// A key's private half, kept so an identity can be made again for each link to the runner.
struct KeptKey {
    device: &'static str,
    private: purlis_session_protocol::hostlink::Zeroizing<[u8; 32]>,
}

impl KeptKey {
    fn new(device: &'static str) -> KeptKey {
        KeptKey {
            device,
            private: purlis_session_protocol::hostlink::LinkKey::generate()
                .unwrap()
                .private_half()
                .unwrap(),
        }
    }

    fn identity(&self) -> purlis_session_protocol::hostlink::Identity {
        purlis_session_protocol::hostlink::Identity {
            device: self.device.into(),
            key: purlis_session_protocol::hostlink::LinkKey::from_private(self.private.clone())
                .unwrap(),
        }
    }

    fn pinned(&self) -> purlis_session_protocol::hostlink::Pinned {
        let identity = self.identity();
        purlis_session_protocol::hostlink::Pinned {
            device: identity.device,
            key: identity.key.public(),
        }
    }
}

/// The runner's connector, in memory: each connection is answered by the runner's host.
#[derive(Clone)]
struct ToTheRunner {
    runner_host: Hosting,
    runner: Arc<KeptKey>,
    desktop: purlis_session_protocol::hostlink::Pinned,
}

impl purlis_session_protocol::hostlink::Connector for ToTheRunner {
    type Io = tokio::io::DuplexStream;

    fn connect(
        &self,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = std::io::Result<Self::Io>> + Send + 'static>,
    > {
        let this = self.clone();
        Box::pin(async move {
            let (a, b) = tokio::io::duplex(256 * 1024);
            tokio::spawn(async move {
                if let Ok(answered) = purlis_session_protocol::hostlink::answer(
                    b,
                    purlis_session_protocol::hostlink::Handshake::THIS_BUILD,
                    &this.runner.identity(),
                    &this.desktop,
                    purlis_session_protocol::hostlink::Liveness::default(),
                )
                .await
                {
                    let _ = session::serve(answered.link, this.runner_host.clone(), None).await;
                }
            });
            Ok(a)
        })
    }
}

/// The desktop's host, relaying to `runner_host` as the runner it keeps as `builds`.
fn a_desktop_relaying_to(runner_host: Hosting) -> Hosting {
    let (desktop, runner) = (
        Arc::new(KeptKey::new("01JA0DESKT0P00000000000000")),
        Arc::new(KeptKey::new("01JB0RUNNER0000000000000000")),
    );
    let pinned = runner.pinned();
    let connector = ToTheRunner {
        runner_host,
        runner,
        desktop: desktop.pinned(),
    };
    let relays: purlis_host::RelaysTo = Arc::new(move |name: &str| {
        if name != "builds" {
            return Err(format!("no runner named {name} is kept on this machine"));
        }
        let (connector, desktop, pinned) =
            (connector.clone(), Arc::clone(&desktop), pinned.clone());
        Ok((
            Arc::new(move || {
                purlis_session_protocol::hostlink::keep_linked(
                    connector.clone(),
                    purlis_session_protocol::hostlink::Handshake::THIS_BUILD,
                    desktop.identity(),
                    pinned.clone(),
                    purlis_session_protocol::hostlink::Liveness::default(),
                )
            }) as purlis_host::Keep,
            format!("{name}/{name}"),
        ))
    });
    Hosting::running_with(
        crate::sessionhost::Composes::as_the_tests_ship(),
        StandIns::default(),
    )
    .relaying_with(relays)
}

/// **A window that restarts before its host has let go of its old link is relayed on a second
/// try** (#2744), through the desktop's host as it serves each link (`Routed`): the old link
/// still holds the window's name, so the first `relay_to` is refused; the window sends it once
/// more a moment later, once the old link has ended, and takes its runner chat back on that
/// link.
#[test]
fn a_window_that_restarts_quickly_is_relayed_with_its_name_on_a_second_try() {
    const CONFINEMENT: &str = "01J9CNFNMNT00000000000000W2";
    let runtime = a_runtime();
    let runner = Hosting::confining(
        ComposesConfined,
        purlis_host::standin::StandInConfines::default(),
        StandIns::default(),
    );
    let desktop = runtime.block_on(async move { a_desktop_relaying_to(runner) });
    let config = tempfile::tempdir().unwrap();
    let (_dir, root) = a_project();
    let project = purlis_core::scaffold::planefile::id_to_hold_by(&root).unwrap();
    let name = purlis_core::windownames::name_for(config.path(), &root, &[]).unwrap();
    let naming = || {
        Some(purlis_session_protocol::browsing::Secret::new(
            name.expose().to_owned(),
        ))
    };
    let hold = || session::Command::HoldProject {
        project: project.clone(),
        project_path: root.display().to_string(),
    };
    let reach = || session::Command::ReachOf {
        confinement: CONFINEMENT.into(),
    };
    let relayed = |naming| {
        let desktop = desktop.clone();
        runtime.block_on(async move {
            let link = a_bare_link_to(purlis_host::Routed::new(desktop), || {}).await?;
            HostLink::over_relayed(
                link,
                tokio::runtime::Handle::current(),
                || {},
                naming,
                Some("builds"),
            )
            .await
        })
    };

    let first = relayed(naming()).expect("the window's link is relayed");
    first.call(hold()).expect("the runner holds the project");
    first
        .call(session::Command::StartProgram(Box::new(
            session::StartProgram {
                confinement: Some(CONFINEMENT.into()),
                ..session::StartProgram::of(&project, ProgramKind::Harness)
            },
        )))
        .expect("composed on the runner");

    // The window restarts: its old link ends a moment after the new one is first refused.
    let going = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(100));
        drop(first);
    });
    let again = relayed(naming()).expect("relayed on the second try");
    going.join().unwrap();
    again.call(hold()).expect("the runner holds the project");
    again
        .call(reach())
        .expect("the window that came back takes its chat back");
}

#[test]
fn a_hosts_origin_is_taken_only_as_an_origin_and_anything_else_is_none() {
    // #2300: a runner's host could send any text; the chip is purlis's own words.
    use super::an_origin;
    let one = |said: &str| an_origin(Some(said.to_owned()));
    assert_eq!(
        one("http://localhost:5173").as_deref(),
        Some("http://localhost:5173")
    );
    assert_eq!(
        one("https://[::1]:8443").as_deref(),
        Some("https://[::1]:8443")
    );
    for forged in [
        "https://a.test on this Mac",
        "https://bank.example\u{202e}",
        "https://a.test/login?token=s3cret",
        "https://ada@a.test",
        "HTTPS://A.TEST",
        "javascript:alert(1)",
        "",
    ] {
        assert_eq!(one(forged), None, "{forged:?}");
    }
    let long = format!("https://{}.test", "a".repeat(300));
    assert_eq!(one(&long), None, "an origin too long for any page");
    assert_eq!(an_origin(None), None);
}

/// A JPEG header saying `width` by `height`, and nothing more.
fn a_jpeg_of(width: u16, height: u16) -> Vec<u8> {
    let mut jpeg = vec![0xff, 0xd8, 0xff, 0xc0, 0x00, 0x11, 0x08];
    jpeg.extend_from_slice(&height.to_be_bytes());
    jpeg.extend_from_slice(&width.to_be_bytes());
    jpeg
}

/// #3297: a pool's own limits move the window's hold with them: a wider frame and a quicker
/// pace where the pool sets them.
#[test]
fn the_window_holds_a_runner_browsers_frames_to_its_pools_own_limits() {
    let mut held = HeldFrames::to(purlis_session_protocol::browsing::LinkFrames {
        frames: 30,
        width: 1920,
    });
    let start = Instant::now();
    assert!(held.draws(&a_jpeg_of(1920, 1080), start));
    assert!(
        held.draws(&a_jpeg_of(1920, 1080), start + Duration::from_millis(30)),
        "at 30 a second, 25 ms apart is drawn"
    );
    assert!(
        !held.draws(&a_jpeg_of(1921, 1080), start + Duration::from_millis(200)),
        "wider than the pool's widest"
    );
    assert_eq!(HeldFrames::default().at_least(), Duration::from_millis(50));
}

#[test]
fn the_window_draws_a_runner_browsers_frames_only_within_the_links_caps() {
    let mut held = HeldFrames::default();
    let start = Instant::now();

    assert!(
        held.draws(&a_jpeg_of(1280, 800), start),
        "a frame within the caps"
    );
    assert!(
        !held.draws(&a_jpeg_of(1280, 800), start + Duration::from_millis(10)),
        "one sooner than the link's rate"
    );
    let later = start + Duration::from_millis(200);
    assert!(
        !held.draws(&a_jpeg_of(1920, 1080), later),
        "wider than the link's widest"
    );
    assert!(
        !held.draws(&a_jpeg_of(1280, 60_000), later),
        "higher than drawn at most"
    );
    assert!(
        !held.draws(b"not a jpeg", later),
        "no image the window can read the size of"
    );
    let mut huge = a_jpeg_of(1280, 800);
    huge.resize(MOST_HELD_FRAME_BYTES + 1, 0);
    assert!(!held.draws(&huge, later), "larger than drawn at most");
    assert!(
        held.draws(&a_jpeg_of(390, 844), later),
        "and the next within them"
    );
}

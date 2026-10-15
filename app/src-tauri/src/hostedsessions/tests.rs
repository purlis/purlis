// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The window's side of a chat in the session host, over an in-memory link to a real host whose
//! programs are stand-ins, so none of this needs a terminal: what the window hands the host for
//! a start, and how it draws, drives, hears the end of and closes what the host runs. The same
//! over real terminals is the behaviour suite (`crate::behaviour`), on CI.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use purlis_core::engine::Size;
use purlis_host::standin::StandIns;
use purlis_host::{FromTheWindow, Hosting};
use purlis_session_protocol::auth::{Credentials, Scope};
use purlis_session_protocol::link;
use purlis_session_protocol::session::{self, Exit};

use super::*;

static HELD: std::sync::LazyLock<Credentials> =
    std::sync::LazyLock::new(|| Credentials::mint().unwrap());

const SIZE: Size = Size {
    columns: 100,
    rows: 30,
};
const PATIENCE: Duration = Duration::from_secs(10);

struct Hosted {
    sessions: HostedSessions,
    link: Arc<HostLink>,
    runs: StandIns,
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
    let hosting = Hosting::running_with(FromTheWindow(crate::sessionhost::launched), runs.clone());
    let link = runtime.block_on(async {
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
        tokio::spawn(session::serve(served.unwrap(), hosting, None));
        HostLink::over(client.unwrap(), tokio::runtime::Handle::current(), || {})
            .await
            .unwrap()
    });
    let link = Arc::new(link);
    let connection = Arc::new(Connection::default());
    connection.ready(Arc::clone(&link));
    let (dir, root) = a_project();
    Hosted {
        sessions: HostedSessions::new(connection, &root, None),
        link,
        runs,
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
        env_strip: Vec::new(),
        harness: None,
        declared_env: Vec::new(),
        env_pass: Vec::new(),
        operator_shell: false,
        git_hooks: None,
        handed: Default::default(),
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
fn only_the_switch_set_to_one_runs_chats_in_the_host() {
    assert!(switched_on(Some("1")));
    for off in [None, Some(""), Some("0"), Some("true"), Some("yes")] {
        assert!(!switched_on(off), "{off:?}");
    }
}

#[test]
fn a_chat_its_profile_hands_credentials_is_never_started_in_the_host() {
    // #2036: what a profile hands its chat is set on a program the window starts, and never
    // goes over the link.
    let (_dir, root) = a_project();
    let sessions = HostedSessions::new(Arc::new(Connection::default()), &root, None);
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
    let sessions = HostedSessions::new(connection, &root, None);
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
    assert!(
        purlis_core::scaffold::planefile::project_id(&hosted.root).is_some(),
        "the host names the project by its stable id"
    );
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
        launch: Some(launch_of(&a_shell("true"), "/bin/sh".to_owned(), None)),
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

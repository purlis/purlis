// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The behaviour suite: what a chat does, written once and run against every place a chat can
//! run (spec #1974, #1998). It is the gate each later step of moving chats into the session
//! host passes: a behaviour is added here once, and every target runs it.
//!
//! **The targets are the two ways the start-time switch runs a chat** (`crate::hostedsessions`):
//! in the window's own process ([`crate::sessions::Sessions`]), and in a session host the window
//! speaks the session protocol to ([`crate::hostedsessions::HostedSessions`], here over an
//! in-memory link to the host's own code, with real terminals). Each behaviour speaks only what
//! the window speaks to either, [`SessionHost`], and checks only what the window can observe:
//! what its pane draws, what it is told, what ends. When the in-app path is deleted (#2005), its
//! target goes with it, and the host's stays; Docker and a cluster join as targets of their own
//! (spec #2041).
//!
//! **The chat's program is the repo's fake harness** (`crates/fake-harness`), or a shell where a
//! behaviour needs the terminal itself (its size, its line discipline). These open terminals,
//! which a sandboxed agent cannot: they first run on CI.
//!
//! **To add a behaviour:** write `fn the_behaviour(target: &Target)`, and name it in the
//! [`on_every_target!`] list at the bottom.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use purlis_core::engine::Size;
use purlis_core::session::Exit;
use purlis_session_protocol::auth::{Credentials, Scope};
use purlis_session_protocol::{link, session};

use crate::host::{Opening, SessionHost, Sink};
use crate::hostedsessions::{Connection, HostLink, HostedSessions};
use crate::sessions::{ENDED, Sessions};

const SIZE: Size = Size {
    columns: 80,
    rows: 24,
};
const PATIENCE: Duration = Duration::from_secs(20);

/// One place a chat runs, as the window reaches it.
pub(crate) struct Target {
    pub sessions: Box<dyn SessionHost>,
    /// What keeps the target running for as long as the test does.
    _keeps: Vec<Box<dyn std::any::Any>>,
}

/// A chat in the window's own process: the switch off, the default.
pub(crate) fn in_the_window() -> Target {
    Target {
        sessions: Box::new(Sessions::new()),
        _keeps: Vec::new(),
    }
}

static HELD: std::sync::LazyLock<Credentials> =
    std::sync::LazyLock::new(|| Credentials::mint().expect("credentials"));

/// A chat in the session host: the switch on. The host's own code, serving the session protocol
/// over an in-memory link, its programs each in a real terminal, composed as the window's binary
/// composes them.
pub(crate) fn in_the_host() -> Target {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("a runtime");
    let hosting =
        purlis_host::Hosting::new(purlis_host::FromTheWindow(crate::sessionhost::launched));
    let link = runtime.block_on(async {
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
        tokio::spawn(session::serve(
            served.expect("the host admits the window"),
            hosting,
            None,
        ));
        HostLink::over(
            client.expect("the window is admitted"),
            tokio::runtime::Handle::current(),
            || {},
        )
        .await
        .expect("the window speaks to the host")
    });
    let connection = Arc::new(Connection::default());
    connection.ready(Arc::new(link));
    let project = tempfile::tempdir().expect("a project");
    let root = std::fs::canonicalize(project.path()).expect("a resolved project");
    std::fs::write(root.join(purlis_core::plane::MANIFEST), "").expect("a manifest");
    Target {
        sessions: Box::new(HostedSessions::new(connection, &root, None)),
        _keeps: vec![Box::new(project), Box::new(runtime)],
    }
}

/// The repo's fake harness, built once beside this test's binary when it is not there yet.
fn fake_harness() -> PathBuf {
    static BUILT: OnceLock<PathBuf> = OnceLock::new();
    BUILT
        .get_or_init(|| {
            let deps = std::env::current_exe().expect("this test's path");
            let beside = deps
                .parent()
                .and_then(Path::parent)
                .expect("the target folder")
                .join("fake-harness");
            if !beside.is_file() {
                let status = purlis_core::forklock::status(
                    std::process::Command::new(env!("CARGO")).args([
                        "build",
                        "--quiet",
                        "-p",
                        "fake-harness",
                    ]),
                )
                .expect("cargo runs");
                assert!(status.success(), "the fake harness did not build: {status}");
            }
            beside
        })
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

/// Each behaviour, as a test of its own on every target.
macro_rules! on_every_target {
    ($($behaviour:ident),* $(,)?) => {
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
            }
        )*
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
);

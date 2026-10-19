// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A session host that is killed outright ends nothing on its way out, so the next host to serve
//! this user settles what it left (#2007, ADR 0068 §3): each program the host record says was
//! started and never ended is told to the binary, which keeps its end as a crash's, and is ended
//! by what runs programs, where its process is still the one that was started. A host that ends
//! every program as it stops leaves nothing to settle. The programs are stand-ins, so none of
//! this needs a terminal; the terminals' own ending of what was left is the host's unit test.

#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use purlis_host::standin::StandIns;
use purlis_host::{
    Compose, ConfinesNothing, Ended, HearsNothing, Hosting, Record, Started, ToCompose,
};
use purlis_session_protocol::auth::{Credentials, Scope};
use purlis_session_protocol::link;
use purlis_session_protocol::session::{self, Client, Exit, ProgramKind, StartProgram};
use purlis_terminal::session::Spec;
use tokio::io::duplex;

const PROJECT: &str = "01J9PRJCT00000000000000000";

static HELD: std::sync::LazyLock<Credentials> =
    std::sync::LazyLock::new(|| Credentials::mint().unwrap());

struct Echo;

impl Compose for Echo {
    fn compose(&self, starting: &ToCompose<'_>) -> Result<Spec, String> {
        Ok(Spec::new("/bin/echo", starting.size))
    }
}

/// What the binary was told the last host left running.
#[derive(Clone, Default)]
struct Kept {
    left: Arc<Mutex<Vec<Started>>>,
}

impl Record for Kept {
    fn record(&self, _entry: serde_json::Value) -> Result<(), String> {
        Ok(())
    }

    fn ended(&self, _ended: &Ended<'_>) {}

    fn left_by_its_last_host(&self, left: &Started) {
        self.left.lock().unwrap().push(left.clone());
    }
}

/// A host keeping what it keeps in `folder`, as the window's and `purlis serve` keep it there.
fn a_host_in(folder: &Path, runs: &StandIns, kept: &Kept) -> Hosting {
    Hosting::kept(folder, Echo, runs.clone(), HearsNothing, ConfinesNothing)
        .recording_with(kept.clone())
}

/// The window's link to `hosting`, with the project at `path` held.
async fn the_window(hosting: &Hosting, path: &Path) -> Client {
    let (a, b) = duplex(256 * 1024);
    let (client, served) = tokio::join!(
        link::connect(
            a,
            session::speaks(),
            Scope::LocalUi,
            HELD.of(Scope::LocalUi)
        ),
        link::serve_any(b, session::speaks(), &HELD)
    );
    tokio::spawn(session::serve(served.unwrap(), hosting.clone(), None));
    let (client, _views, _events) = Client::new(client.unwrap());
    client
        .hold_project(PROJECT, path.to_str().unwrap())
        .await
        .expect("the project is held");
    client
}

/// A chat's start, beginning run `run`.
fn a_chat(run: &str) -> StartProgram {
    StartProgram {
        run: Some(run.to_owned()),
        ..StartProgram::of(PROJECT, ProgramKind::Harness)
    }
}

/// Starts three chats in a host kept in `folder`, the first of which ends by itself, and leaves
/// the host as a crash leaves it: nothing it ran is ended.
async fn a_host_that_crashed_with_two_chats_running(folder: &Path, project: &Path) {
    let runs = StandIns::default();
    let hosting = a_host_in(folder, &runs, &Kept::default());
    let window = the_window(&hosting, project).await;
    for run in ["run-1", "run-2", "run-3"] {
        window.start_program(a_chat(run)).await.expect("it starts");
    }
    runs.ran()[0].1.ends(Exit::Code(0));
    tokio::time::sleep(Duration::from_millis(50)).await;
}

#[tokio::test]
async fn each_program_the_last_host_left_running_is_told_to_the_binary() {
    let (folder, project) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    a_host_that_crashed_with_two_chats_running(folder.path(), project.path()).await;
    let kept = Kept::default();
    let next = a_host_in(folder.path(), &StandIns::default(), &kept);

    let settled = next.take_over_from_the_last_host();

    assert_eq!(settled, 2);
    let project = std::fs::canonicalize(project.path()).unwrap();
    let left: Vec<(PathBuf, u32, Option<String>)> = kept
        .left
        .lock()
        .unwrap()
        .iter()
        .map(|left| (left.project_path.clone(), left.number, left.run.clone()))
        .collect();
    assert_eq!(
        left,
        [
            (project.clone(), 2, Some("run-2".to_owned())),
            (project.clone(), 3, Some("run-3".to_owned())),
        ]
    );
}

#[tokio::test]
async fn each_program_the_last_host_left_running_is_ended_by_what_runs_programs() {
    let (folder, project) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    a_host_that_crashed_with_two_chats_running(folder.path(), project.path()).await;
    let runs = StandIns::default();
    let next = a_host_in(folder.path(), &runs, &Kept::default());

    next.take_over_from_the_last_host();

    let numbers: Vec<u32> = runs.ended_left().iter().map(|left| left.number).collect();
    assert_eq!(numbers, [2, 3]);
    assert!(
        runs.ended_left()
            .iter()
            .all(|left| left.process.is_some() && left.process_started.is_some()),
        "each was written down with its process and when it started"
    );
}

#[tokio::test]
async fn what_the_last_host_left_is_settled_once() {
    let (folder, project) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    a_host_that_crashed_with_two_chats_running(folder.path(), project.path()).await;
    a_host_in(folder.path(), &StandIns::default(), &Kept::default()).take_over_from_the_last_host();
    let kept = Kept::default();

    let settled =
        a_host_in(folder.path(), &StandIns::default(), &kept).take_over_from_the_last_host();

    assert_eq!(settled, 0);
    assert!(kept.left.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_host_that_ended_every_program_as_it_stopped_leaves_nothing_to_settle() {
    let (folder, project) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let runs = StandIns::default();
    runs.hold_on();
    let hosting = a_host_in(folder.path(), &runs, &Kept::default());
    let window = the_window(&hosting, project.path()).await;
    window
        .start_program(a_chat("run-1"))
        .await
        .expect("it starts");
    // Its program ignores being stopped, so its end is never told: the host still ended it.
    let ending = hosting.clone();
    tokio::task::spawn_blocking(move || ending.end_all())
        .await
        .unwrap();
    let kept = Kept::default();

    let settled =
        a_host_in(folder.path(), &StandIns::default(), &kept).take_over_from_the_last_host();

    assert_eq!(settled, 0);
    assert!(kept.left.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_host_kept_in_memory_keeps_no_record_and_settles_nothing() {
    let project = tempfile::tempdir().unwrap();
    let kept = Kept::default();
    let hosting = Hosting::with(Echo, StandIns::default(), HearsNothing, ConfinesNothing)
        .recording_with(kept.clone());
    let window = the_window(&hosting, project.path()).await;
    window
        .start_program(a_chat("run-1"))
        .await
        .expect("it starts");

    let settled = hosting.take_over_from_the_last_host();

    assert_eq!(settled, 0);
    assert!(kept.left.lock().unwrap().is_empty());
}

/// What the last host noted of a program whose process is `process`, started when `started`
/// says, in this boot of the machine.
fn left_as(process: u32, started: Option<String>) -> Started {
    left_in(process, started, purlis_host::this_boot())
}

/// [`left_as`], noted in the boot `boot`.
fn left_in(process: u32, started: Option<String>, boot: Option<String>) -> Started {
    Started {
        program: "01J9PRGRM00000000000000000".to_owned(),
        project: PROJECT.to_owned(),
        project_path: PathBuf::from("/home/me/project"),
        number: 1,
        run: None,
        process: Some(process),
        process_started: started,
        boot,
        sandbox: None,
    }
}

/// A program leading a process group of its own, as a terminal's does, with a child in it.
fn a_program_with_a_child() -> std::process::Child {
    use std::os::unix::process::CommandExt;
    std::process::Command::new("/bin/sh")
        .args(["-c", "sleep 30 & sleep 30"])
        .process_group(0)
        .spawn()
        .expect("a program starts")
}

/// Whether `program` ended within a few seconds, reaped if it did.
fn ends_soon(program: &mut std::process::Child) -> bool {
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while std::time::Instant::now() < deadline {
        if matches!(program.try_wait(), Ok(Some(_))) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

#[test]
fn a_terminals_program_the_last_host_left_running_is_ended_with_its_group() {
    use purlis_host::Run as _;
    let mut program = a_program_with_a_child();
    let started = purlis_same_user::Parents::started(program.id());
    assert!(started.is_some(), "the kernel says when it started");

    purlis_host::Terminals.end_what_was_left(&left_as(program.id(), started));

    assert!(ends_soon(&mut program), "it was left running");
}

#[test]
fn a_process_that_is_not_the_one_the_last_host_started_in_this_boot_is_left_alone() {
    use purlis_host::Run as _;
    let mut program = a_program_with_a_child();

    purlis_host::Terminals.end_what_was_left(&left_as(
        program.id(),
        Some("another process's start".to_owned()),
    ));
    purlis_host::Terminals.end_what_was_left(&left_as(program.id(), None));

    // Its start, as the kernel says it, but noted in another boot of the machine.
    purlis_host::Terminals.end_what_was_left(&left_in(
        program.id(),
        purlis_same_user::Parents::started(program.id()),
        Some("another boot".to_owned()),
    ));

    let ended = ends_soon(&mut program);
    // Ended here whatever happened above, with its group: the test leaves nothing running.
    let group = nix::unistd::Pid::from_raw(i32::try_from(program.id()).unwrap());
    let _ = nix::sys::signal::killpg(group, nix::sys::signal::Signal::SIGKILL);
    let _ = program.wait();
    assert!(!ended, "a process the host record does not name was ended");
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A host ending every program as it stops (`Hosting::end_all`, at a quit or a `SIGTERM`)
//! returns only once each chat's end is kept, or its time is up (#6700, ADR 0068 §3 as amended
//! for #6700). A terminal's program is reaped first, and what its end leaves (the spool drained,
//! `run.ended`, the host record's `ended` line) is written after, on a thread of its own: a host
//! that exited at the reaping would end that thread mid-write. Where an end is still unwritten
//! when the time is up, the host record keeps that chat's start, so the next host settles it as
//! the crash it is, rather than nobody settling it at all. And from the moment the host begins
//! ending every program, it starts none.
//!
//! The stand-ins here ignore being stopped, and each is ended by a thread of the test's once it
//! has been told to stop, as a terminal's reaper ends a real one.

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use purlis_host::standin::{StandIn, StandIns};
use purlis_host::{
    Compose, ConfinesNothing, ENDS_WITHIN, Ended, HearsNothing, Hosting, Record, Started, Stopped,
    ToCompose,
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

/// A program's number, and the host's own act that ended it.
type EndedBy = (u32, Option<Stopped>);

/// How each program the host ran ended, kept only once writing it has taken `slow`.
#[derive(Clone)]
struct Slow {
    slow: Duration,
    ended: Arc<Mutex<Vec<EndedBy>>>,
}

impl Slow {
    fn taking(slow: Duration) -> Self {
        Slow {
            slow,
            ended: Arc::default(),
        }
    }
}

impl Record for Slow {
    fn record(&self, _entry: serde_json::Value) -> Result<(), String> {
        Ok(())
    }

    fn ended(&self, ended: &Ended<'_>) {
        std::thread::sleep(self.slow);
        self.ended
            .lock()
            .unwrap()
            .push((ended.number, ended.stopped));
    }

    fn left_by_its_last_host(&self, _left: &Started) {}
}

/// Remembers what the last host left, for the next host.
#[derive(Clone, Default)]
struct Left(Arc<Mutex<Vec<Started>>>);

impl Record for Left {
    fn record(&self, _entry: serde_json::Value) -> Result<(), String> {
        Ok(())
    }

    fn ended(&self, _ended: &Ended<'_>) {}

    fn left_by_its_last_host(&self, left: &Started) {
        self.0.lock().unwrap().push(left.clone());
    }
}

async fn the_desktop(hosting: &Hosting, path: &Path) -> Client {
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

/// A host on stand-ins that ignore being stopped, with two chats started, whose ends `record`
/// keeps.
async fn two_chats(folder: &Path, project: &Path, record: Slow) -> (Hosting, StandIns, Client) {
    let runs = StandIns::default();
    runs.hold_on();
    let hosting = Hosting::kept(folder, Echo, runs.clone(), HearsNothing, ConfinesNothing)
        .recording_with(record);
    let desktop = the_desktop(&hosting, project).await;
    for run in ["run-1", "run-2"] {
        desktop
            .start_program(StartProgram {
                run: Some(run.to_owned()),
                ..StartProgram::of(PROJECT, ProgramKind::Harness)
            })
            .await
            .expect("it starts");
    }
    (hosting, runs, desktop)
}

/// Ends each stand-in on a thread of its own once it has been told to stop, as a terminal's
/// reaper tells a real program's end after its process is gone.
fn reaped_once_stopped(programs: Vec<StandIn>) {
    for program in programs {
        std::thread::spawn(move || {
            let until = Instant::now() + Duration::from_secs(30);
            while !program.seen().stopped && Instant::now() < until {
                std::thread::sleep(Duration::from_millis(5));
            }
            program.ends(Exit::Signal("Interrupt".into()));
        });
    }
}

fn the_stand_ins(runs: &StandIns) -> Vec<StandIn> {
    runs.ran().into_iter().map(|(_, program)| program).collect()
}

fn settled_by_the_next_host(folder: &Path) -> usize {
    Hosting::kept(
        folder,
        Echo,
        StandIns::default(),
        HearsNothing,
        ConfinesNothing,
    )
    .recording_with(Left::default())
    .take_over_from_the_last_host()
}

#[tokio::test]
async fn ending_every_program_returns_only_once_each_chats_end_is_kept() {
    let (folder, project) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let kept = Slow::taking(Duration::from_millis(300));
    let (hosting, runs, _desktop) = two_chats(folder.path(), project.path(), kept.clone()).await;
    reaped_once_stopped(the_stand_ins(&runs));

    let ending = hosting.clone();
    tokio::task::spawn_blocking(move || ending.end_all())
        .await
        .unwrap();

    let mut ended = kept.ended.lock().unwrap().clone();
    ended.sort_by_key(|(number, _)| *number);
    assert_eq!(
        ended,
        [(1, Some(Stopped::Quit)), (2, Some(Stopped::Quit))],
        "each chat's end was kept, as a quit, before the host was done ending them"
    );
    assert_eq!(
        settled_by_the_next_host(folder.path()),
        0,
        "nothing is left to settle as a crash"
    );
}

#[tokio::test]
async fn a_chat_whose_end_is_not_kept_in_time_is_left_for_the_next_host_to_settle() {
    let (folder, project) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    // Longer than the host gives every program to end: neither end is kept in time.
    let kept = Slow::taking(ENDS_WITHIN + Duration::from_secs(2));
    let (hosting, runs, _desktop) = two_chats(folder.path(), project.path(), kept).await;
    reaped_once_stopped(the_stand_ins(&runs));

    let ending = hosting.clone();
    let began = Instant::now();
    tokio::task::spawn_blocking(move || ending.end_all())
        .await
        .unwrap();

    assert!(
        began.elapsed() < ENDS_WITHIN + Duration::from_secs(1),
        "the host waits no longer than it gives every program"
    );
    assert_eq!(
        settled_by_the_next_host(folder.path()),
        2,
        "each chat whose end was never kept is settled by the next host as ended by its crash"
    );
}

#[tokio::test]
async fn a_start_that_lands_as_the_host_ends_every_program_is_refused() {
    let (folder, project) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let kept = Slow::taking(Duration::from_millis(800));
    let (hosting, runs, desktop) = two_chats(folder.path(), project.path(), kept).await;
    reaped_once_stopped(the_stand_ins(&runs));

    let ending = hosting.clone();
    let ended = tokio::task::spawn_blocking(move || ending.end_all());
    // Once the host is ending them: both are told to stop as it begins.
    let until = Instant::now() + Duration::from_secs(5);
    while !the_stand_ins(&runs)
        .iter()
        .all(|program| program.seen().stopped)
    {
        assert!(Instant::now() < until, "the host told each program to stop");
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    let late = desktop
        .start_program(StartProgram {
            run: Some("run-3".to_owned()),
            ..StartProgram::of(PROJECT, ProgramKind::Harness)
        })
        .await;
    ended.await.unwrap();

    assert!(
        late.is_err(),
        "a start as the host stops is refused: {late:?}"
    );
    assert_eq!(
        settled_by_the_next_host(folder.path()),
        0,
        "nothing it refused is left in the host record to settle as a crash"
    );
}

#[tokio::test]
async fn a_second_ending_after_a_quit_leaves_what_the_first_left_for_the_next_host() {
    let (folder, project) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    // Longer than both endings together give every program: neither end is kept in time.
    let kept = Slow::taking(3 * ENDS_WITHIN);
    let (hosting, runs, desktop) = two_chats(folder.path(), project.path(), kept).await;
    reaped_once_stopped(the_stand_ins(&runs));

    // The window quits, and then the host ends every program again as it stops serving.
    desktop.quit().await.expect("the quit is answered");
    let ending = hosting.clone();
    tokio::task::spawn_blocking(move || ending.end_all())
        .await
        .unwrap();

    assert_eq!(
        settled_by_the_next_host(folder.path()),
        2,
        "the second ending, finding no program, still leaves each end the first never kept"
    );
}

#[tokio::test]
async fn an_ending_begun_while_another_waits_leaves_what_neither_kept_for_the_next_host() {
    let (folder, project) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let kept = Slow::taking(3 * ENDS_WITHIN);
    let (hosting, runs, _desktop) = two_chats(folder.path(), project.path(), kept).await;
    reaped_once_stopped(the_stand_ins(&runs));

    // A quit's ending, and a SIGTERM's while the first still waits for the ends.
    let first = {
        let ending = hosting.clone();
        tokio::task::spawn_blocking(move || ending.end_all())
    };
    let until = Instant::now() + Duration::from_secs(5);
    while !the_stand_ins(&runs)
        .iter()
        .all(|program| program.seen().stopped)
    {
        assert!(Instant::now() < until, "the host told each program to stop");
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    let ending = hosting.clone();
    tokio::task::spawn_blocking(move || ending.end_all())
        .await
        .unwrap();
    assert_eq!(
        settled_by_the_next_host(folder.path()),
        2,
        "the second ending empties nothing while the first's ends are unkept"
    );
    first.await.unwrap();
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The event log is written in the session host, and a program's end is kept there, window or
//! no window (#2003), spoken over the session protocol on in-memory links: the window's `record`
//! reaches the binary's [`Record`] and no other link's does, and each program's end is told to
//! it with the host's own act that ended it and whether a window held its project then. The
//! programs are stand-ins, so none of this needs a terminal.

#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use purlis_host::standin::StandIns;
use purlis_host::{
    Compose, ConfinesNothing, Ended, HearsNothing, Hosting, Record, Stopped, ToCompose,
};
use purlis_session_protocol::auth::{Credentials, Scope};
use purlis_session_protocol::link;
use purlis_session_protocol::session::{self, Client, Command, Exit, ProgramKind, StartProgram};
use purlis_terminal::session::Spec;
use tokio::io::duplex;

const PROJECT: &str = "01J9PRJCT00000000000000000";
const PATIENCE: Duration = Duration::from_secs(5);

static HELD: std::sync::LazyLock<Credentials> =
    std::sync::LazyLock::new(|| Credentials::mint().unwrap());

struct Echo;

impl Compose for Echo {
    fn compose(&self, starting: &ToCompose<'_>) -> Result<Spec, String> {
        Ok(Spec::new("/bin/echo", starting.size))
    }
}

/// One program's end, as the binary was told it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Told {
    project_path: PathBuf,
    number: u32,
    exit: Exit,
    stopped: Option<Stopped>,
    watched: bool,
}

/// The binary's keeping: what it was asked to record, and each end it was told.
#[derive(Clone, Default)]
struct Kept {
    recorded: Arc<Mutex<Vec<serde_json::Value>>>,
    ended: Arc<Mutex<Vec<Told>>>,
}

impl Record for Kept {
    fn record(&self, entry: serde_json::Value) -> Result<(), String> {
        self.recorded.lock().unwrap().push(entry);
        Ok(())
    }

    fn ended(&self, ended: &Ended<'_>) {
        self.ended.lock().unwrap().push(Told {
            project_path: ended.project_path.to_owned(),
            number: ended.number,
            exit: ended.exit.clone(),
            stopped: ended.stopped,
            watched: ended.watched,
        });
    }
}

impl Kept {
    /// The ends told, once there are `n` of them.
    fn ends(&self, n: usize) -> Vec<Told> {
        let deadline = Instant::now() + PATIENCE;
        loop {
            let ended = self.ended.lock().unwrap().clone();
            if ended.len() >= n || Instant::now() > deadline {
                return ended;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

fn a_host(runs: &StandIns, kept: &Kept) -> Hosting {
    Hosting::with(Echo, runs.clone(), HearsNothing, ConfinesNothing).recording_with(kept.clone())
}

/// A link to `hosting` admitted as `scope`.
async fn a_link(hosting: &Hosting, scope: Scope) -> Client {
    let (a, b) = duplex(256 * 1024);
    let (client, served) = tokio::join!(
        link::connect(a, session::speaks(), scope, HELD.of(scope)),
        link::serve_any(b, session::speaks(), &HELD)
    );
    tokio::spawn(session::serve(served.unwrap(), hosting.clone(), None));
    let (client, _views, _events) = Client::new(client.unwrap());
    client
}

/// The window's link, with `project` held.
async fn the_window(hosting: &Hosting, project: &Path) -> Client {
    let window = a_link(hosting, Scope::LocalUi).await;
    window
        .hold_project(PROJECT, project.to_str().unwrap())
        .await
        .expect("the project is held");
    window
}

fn a_harness() -> StartProgram {
    StartProgram::of(PROJECT, ProgramKind::Harness)
}

fn an_entry() -> serde_json::Value {
    serde_json::json!({ "entry": "kill_switch_read" })
}

#[tokio::test]
async fn what_the_window_asks_recorded_reaches_the_binarys_log_in_the_order_it_was_sent() {
    let project = tempfile::tempdir().unwrap();
    let kept = Kept::default();
    let hosting = a_host(&StandIns::default(), &kept);
    let window = the_window(&hosting, project.path()).await;

    for n in 0..20 {
        window
            .call(Command::Record {
                entry: serde_json::json!({ "entry": "kill_switch_read", "n": n }),
            })
            .await
            .expect("recorded");
    }

    let recorded = kept.recorded.lock().unwrap().clone();
    let order: Vec<u64> = recorded
        .iter()
        .filter_map(|entry| entry["n"].as_u64())
        .collect();
    assert_eq!(order, (0..20).collect::<Vec<u64>>());
}

#[tokio::test]
async fn no_link_but_the_windows_writes_in_the_event_log() {
    let kept = Kept::default();
    let hosting = a_host(&StandIns::default(), &kept);

    // Every other scope's row is the protocol's own test; these are the ones a host admits.
    for scope in [Scope::Terminal, Scope::FleetMcp, Scope::Approval] {
        let other = a_link(&hosting, scope).await;

        let refused = other
            .call(Command::Record { entry: an_entry() })
            .await
            .unwrap_err();

        let refusal = refused.refusal().expect("refused by the host");
        assert_eq!(refusal.code, session::NOT_ALLOWED, "{scope}");
    }
    assert!(kept.recorded.lock().unwrap().is_empty(), "nothing written");
}

#[tokio::test]
async fn a_host_that_keeps_no_event_log_refuses_a_write_in_words() {
    let hosting = Hosting::with(Echo, StandIns::default(), HearsNothing, ConfinesNothing);
    let window = a_link(&hosting, Scope::LocalUi).await;

    let refused = window
        .call(Command::Record { entry: an_entry() })
        .await
        .unwrap_err();

    let refusal = refused.refusal().expect("refused by the host");
    assert_eq!(refusal.code, session::NOT_RECORDED);
    assert!(refusal.why.contains("event log"), "{}", refusal.why);
}

#[tokio::test]
async fn a_program_that_ends_by_itself_with_the_window_gone_is_kept_as_unwatched() {
    let project = tempfile::tempdir().unwrap();
    let (runs, kept) = (StandIns::default(), Kept::default());
    let hosting = a_host(&runs, &kept);
    let window = the_window(&hosting, project.path()).await;
    window.start_program(a_harness()).await.expect("it starts");

    // The window's link ends, and the host runs on.
    drop(window);
    let deadline = Instant::now() + PATIENCE;
    while hosting_holds(&hosting).await && Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(
        !hosting_holds(&hosting).await,
        "the window's holds went with its link"
    );
    runs.last().ends(Exit::Code(1));

    let ended = kept.ends(1);
    assert_eq!(
        ended,
        [Told {
            project_path: std::fs::canonicalize(project.path()).unwrap(),
            number: 1,
            exit: Exit::Code(1),
            stopped: None,
            watched: false,
        }]
    );
}

/// Whether any link still holds the project: a number is dealt only in a held project.
async fn hosting_holds(hosting: &Hosting) -> bool {
    let probe = a_link(hosting, Scope::LocalUi).await;
    probe.deal(PROJECT).await.is_ok()
}

#[tokio::test]
async fn a_program_the_host_ended_is_kept_with_the_hosts_own_act() {
    let project = tempfile::tempdir().unwrap();
    let (runs, kept) = (StandIns::default(), Kept::default());
    let hosting = a_host(&runs, &kept);
    let window = the_window(&hosting, project.path()).await;
    let stopped = window.start_program(a_harness()).await.expect("it starts");
    let closed = window.start_program(a_harness()).await.expect("it starts");

    window.stop(&stopped).await.expect("stopped");
    window.close(&closed).await.expect("closed");

    let mut ended: Vec<(u32, Option<Stopped>, bool)> = kept
        .ends(2)
        .into_iter()
        .map(|told| (told.number, told.stopped, told.watched))
        .collect();
    ended.sort();
    assert_eq!(
        ended,
        [
            (1, Some(Stopped::Operator), true),
            (2, Some(Stopped::Closed), true)
        ]
    );
}

#[tokio::test]
async fn every_program_the_kill_switch_ends_is_kept_as_killed() {
    let (folder, project) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (runs, kept) = (StandIns::default(), Kept::default());
    let hosting = Hosting::kept(
        folder.path(),
        Echo,
        runs.clone(),
        HearsNothing,
        ConfinesNothing,
    )
    .recording_with(kept.clone());
    let window = the_window(&hosting, project.path()).await;
    for _ in 0..3 {
        window.start_program(a_harness()).await.expect("it starts");
    }

    window.stop_all().await.expect("stopped");

    let ended = kept.ends(3);
    assert_eq!(ended.len(), 3);
    assert!(
        ended
            .iter()
            .all(|told| told.stopped == Some(Stopped::Killed) && told.watched),
        "{ended:?}"
    );
}

#[tokio::test]
async fn as_the_host_ends_every_program_each_is_kept_as_ended_by_the_quit() {
    let project = tempfile::tempdir().unwrap();
    let (runs, kept) = (StandIns::default(), Kept::default());
    let hosting = a_host(&runs, &kept);
    let window = the_window(&hosting, project.path()).await;
    window.start_program(a_harness()).await.expect("it starts");

    let ending = hosting.clone();
    tokio::task::spawn_blocking(move || ending.end_all())
        .await
        .unwrap();

    let ended = kept.ends(1);
    assert_eq!(ended.len(), 1);
    assert_eq!(ended[0].stopped, Some(Stopped::Quit));
}

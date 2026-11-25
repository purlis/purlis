// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A program hibernated at its window's ask (`hibernate`, 1.96, #7630, SC-4a) is ended and its
//! engine dropped: the host no longer holds it, so its memory is returned, which a stop (that
//! keeps it until it is closed) does not. Its end is told as the host's lowest act, so the
//! binary writes no `run.ended` for it, and an end that races it (a close, a stop) wins. Spoken
//! over the session protocol on in-memory links, with stand-in programs.

#![cfg(unix)]

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use purlis_host::standin::StandIns;
use purlis_host::{
    Compose, ConfinesNothing, Ended, HearsNothing, Hosting, Record, Stopped, ToCompose,
};
use purlis_session_protocol::auth::{Credentials, Scope};
use purlis_session_protocol::link;
use purlis_session_protocol::session::{self, Client, ProgramKind, StartProgram};
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

/// One end the binary was told: the program's number and the host's own act.
type End = (u32, Option<Stopped>);

/// The ends the binary was told.
#[derive(Clone, Default)]
struct Kept(Arc<Mutex<Vec<End>>>);

impl Record for Kept {
    fn record(&self, _entry: serde_json::Value) -> Result<(), String> {
        Ok(())
    }

    fn ended(&self, ended: &Ended<'_>) {
        self.0.lock().unwrap().push((ended.number, ended.stopped));
    }
}

impl Kept {
    fn ends(&self, n: usize) -> Vec<End> {
        let deadline = Instant::now() + PATIENCE;
        loop {
            let ended = self.0.lock().unwrap().clone();
            if ended.len() >= n || Instant::now() > deadline {
                return ended;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

async fn the_window(hosting: &Hosting, project: &Path) -> Arc<Client> {
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
    let (window, _views, _events) = Client::new(client.unwrap());
    window
        .hold_project(PROJECT, project.to_str().unwrap())
        .await
        .expect("the project is held");
    Arc::new(window)
}

fn a_harness() -> StartProgram {
    StartProgram::of(PROJECT, ProgramKind::Harness)
}

fn a_host(runs: &StandIns, kept: &Kept) -> Hosting {
    Hosting::with(Echo, runs.clone(), HearsNothing, ConfinesNothing).recording_with(kept.clone())
}

#[tokio::test]
async fn hibernating_a_program_ends_it_and_the_host_no_longer_holds_it() {
    let project = tempfile::tempdir().unwrap();
    let (runs, kept) = (StandIns::default(), Kept::default());
    let hosting = a_host(&runs, &kept);
    let window = the_window(&hosting, project.path()).await;
    let program = window.start_program(a_harness()).await.expect("it starts");
    let stays = window.start_program(a_harness()).await.expect("it starts");

    window.hibernate(&program).await.expect("hibernated");

    assert_eq!(kept.ends(1), [(1, Some(Stopped::Hibernated))]);
    let held: Vec<String> = window
        .programs_of(PROJECT)
        .await
        .unwrap()
        .into_iter()
        .map(|held| held.program)
        .collect();
    assert_eq!(
        held,
        [stays],
        "its engine is dropped: only the other program is held"
    );
    assert!(
        window.hibernate(&program).await.is_err(),
        "it is no longer there to hibernate"
    );
}

#[tokio::test]
async fn a_stop_that_came_first_stays_the_end_though_the_program_is_then_hibernated() {
    // The host keeps the highest act that ended a program, and a hibernation is the lowest.
    let project = tempfile::tempdir().unwrap();
    let (runs, kept) = (StandIns::default(), Kept::default());
    let hosting = a_host(&runs, &kept);
    let window = the_window(&hosting, project.path()).await;
    let program = window.start_program(a_harness()).await.expect("it starts");

    window.stop(&program).await.expect("stopped");
    window.hibernate(&program).await.expect("hibernated after");

    assert_eq!(kept.ends(1), [(1, Some(Stopped::Operator))]);
}

#[tokio::test]
async fn a_close_that_came_first_stays_the_end_and_the_hibernation_is_refused() {
    let project = tempfile::tempdir().unwrap();
    let (runs, kept) = (StandIns::default(), Kept::default());
    let hosting = a_host(&runs, &kept);
    let window = the_window(&hosting, project.path()).await;
    let program = window.start_program(a_harness()).await.expect("it starts");

    window.close(&program).await.expect("closed");

    assert!(window.hibernate(&program).await.is_err());
    assert_eq!(kept.ends(1), [(1, Some(Stopped::Closed))]);
}

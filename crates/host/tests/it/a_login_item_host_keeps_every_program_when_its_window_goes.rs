// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A session host that is a login item (FD-17, #655; ADR 0068 §2, X24), spoken to over the
//! session protocol on in-memory links.
//!
//! - **Its window going ends nothing**, with or without a word, however long it stays away:
//!   something keeps the host running (a trigger or a paired device), so there is no grace
//!   period, and a window that comes back finds every program as it was.
//! - **A quit ends every program as a quit and stops the host**: the tray's Stop, or a window
//!   quitting with nothing keeping the host any more.
//! - **A program started to end with its window still ends as the window goes** (1.42, #3274).
//!
//! The programs are stand-ins; the window's grace period is a fraction of a second where it is
//! compared. A real host process under launchd or systemd is the operator's check on hardware.

#![cfg(unix)]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use purlis_host::standin::StandIns;
use purlis_host::{
    Compose, ConfinesNothing, Ended, HearsNothing, Hosting, Record, Stopped, ToCompose,
};
use purlis_session_protocol::auth::{Credentials, Scope};
use purlis_session_protocol::link::{self, Acceptor};
use purlis_session_protocol::session::{self, Client, Events, ProgramKind, StartProgram};
use purlis_terminal::session::Spec;
use serde_json::Value;
use tokio::io::duplex;
use tokio::task::JoinHandle;

const PROJECT: &str = "01J9PRJCT00000000000000000";
/// Longer than the grace period a window's host gives, in the test that compares the two.
const AWAY: Duration = Duration::from_millis(600);
const PATIENCE: Duration = Duration::from_secs(5);

static HELD: std::sync::LazyLock<Credentials> =
    std::sync::LazyLock::new(|| Credentials::mint().unwrap());

struct Echo;

impl Compose for Echo {
    fn compose(&self, starting: &ToCompose<'_>) -> Result<Spec, String> {
        Ok(Spec::new("/bin/echo", starting.size))
    }
}

/// Each program's end the host kept, by number and the host's act that ended it.
#[derive(Clone, Default)]
struct Ends(Arc<Mutex<Vec<Kept>>>);

/// One end kept: the program's number and the host's act that ended it.
type Kept = (u32, Option<Stopped>);

impl Record for Ends {
    fn record(&self, _entry: Value) -> Result<(), String> {
        Ok(())
    }

    fn ended(&self, ended: &Ended<'_>) {
        self.0.lock().unwrap().push((ended.number, ended.stopped));
    }
}

impl Ends {
    fn kept(&self) -> Vec<Kept> {
        let mut kept = self.0.lock().unwrap().clone();
        kept.sort();
        kept
    }
}

struct Host {
    hosting: Hosting,
    runs: StandIns,
    ends: Ends,
    project: tempfile::TempDir,
}

fn a_login_item() -> Host {
    let (runs, ends) = (StandIns::default(), Ends::default());
    Host {
        hosting: Hosting::with(Echo, runs.clone(), HearsNothing, ConfinesNothing)
            .recording_with(ends.clone()),
        runs,
        ends,
        project: tempfile::tempdir().unwrap(),
    }
}

/// A window's link to the host, subscribed, as the window's is.
struct Window {
    client: Client,
    _views: Acceptor,
    _events: Events,
}

impl Host {
    /// The host's life as a login item's.
    fn lives(&self) -> JoinHandle<()> {
        let hosting = self.hosting.clone();
        tokio::spawn(async move { hosting.while_it_is_a_login_item().await })
    }

    async fn window(&self) -> Window {
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
        tokio::spawn(session::serve(served.unwrap(), self.hosting.clone(), None));
        let (client, views, events) = Client::new(client.unwrap());
        client.subscribe(session::Since::new()).await.unwrap();
        let window = Window {
            client,
            _views: views,
            _events: events,
        };
        window
            .client
            .hold_project(PROJECT, self.project.path().to_str().unwrap())
            .await
            .expect("the project is held");
        window
    }
}

impl Window {
    async fn starts(&self, number: u32, ends_with_its_window: bool) -> String {
        self.client
            .start_program(StartProgram {
                number: Some(number),
                ends_with_its_window,
                ..StartProgram::of(PROJECT, ProgramKind::Harness)
            })
            .await
            .expect("it starts")
    }
}

#[tokio::test]
async fn a_window_that_goes_without_a_word_ends_nothing_however_long_it_stays_away() {
    let host = a_login_item();
    let lives = host.lives();
    host.window().await.starts(1, false).await;

    tokio::time::sleep(AWAY).await;

    assert!(host.ends.kept().is_empty(), "nothing ended");
    assert!(!host.runs.last().seen().stopped, "the program runs on");
    assert!(!lives.is_finished(), "the host goes on");
    let back = host.window().await;
    assert_eq!(back.client.programs_of(PROJECT).await.unwrap().len(), 1);
}

#[tokio::test]
async fn where_a_windows_host_would_have_ended_them_a_login_items_programs_run_on() {
    // The same window, the same program, and the same time away: the window's host ends it as
    // a quit once its grace period has passed; the login item does not.
    let windows = a_login_item();
    let grace = AWAY / 3;
    let lives = {
        let hosting = windows.hosting.clone();
        tokio::spawn(async move {
            hosting
                .while_a_window_may_come_back(grace, std::future::pending())
                .await;
        })
    };
    windows.window().await.starts(1, false).await;
    let login_item = a_login_item();
    let kept = login_item.lives();
    login_item.window().await.starts(1, false).await;

    tokio::time::timeout(PATIENCE, lives)
        .await
        .expect("the window's host stops past its grace period")
        .unwrap();

    assert_eq!(windows.ends.kept(), [(1, Some(Stopped::Quit))]);
    assert!(login_item.ends.kept().is_empty());
    assert!(!kept.is_finished());
}

#[tokio::test]
async fn a_quit_ends_every_program_as_a_quit_and_stops_the_host() {
    let host = a_login_item();
    let lives = host.lives();
    let window = host.window().await;
    window.starts(1, false).await;
    window.starts(2, false).await;

    window
        .client
        .quit()
        .await
        .expect("the quit is answered once every program has ended");

    tokio::time::timeout(PATIENCE, lives)
        .await
        .expect("the host stops")
        .unwrap();
    assert_eq!(
        host.ends.kept(),
        [(1, Some(Stopped::Quit)), (2, Some(Stopped::Quit))]
    );
}

#[tokio::test]
async fn a_program_started_to_end_with_its_window_still_ends_as_the_window_goes() {
    let host = a_login_item();
    let _lives = host.lives();
    {
        let window = host.window().await;
        window.starts(1, true).await;
        window.starts(2, false).await;
    }

    let deadline = std::time::Instant::now() + PATIENCE;
    while host.ends.kept().is_empty() {
        assert!(
            std::time::Instant::now() < deadline,
            "the grade's chat never ended"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    tokio::time::sleep(AWAY).await;
    assert_eq!(host.ends.kept().len(), 1, "only the one that ends with it");
}

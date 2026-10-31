// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The window's session host outlives a window that went without a quit, for the grace period
//! (ADR 0068 §2, ten minutes; #2006), spoken to over the session protocol on in-memory links.
//!
//! - **A window back within it finds everything as it was**, and nothing ends.
//! - **Past it with no window, every program ends as a quit would**, its project's saving
//!   stops, and the host stops: no agent runs unseen for longer than the grace period.
//! - **A project no window holds again is let go of when its own grace period passes**, even
//!   while a window that did not open it is there (#2262): its programs end as a quit would,
//!   running or ended, with its hook channel and its board. A window back before then still
//!   reads how each program ended.
//! - **A program started to end with its window ends as soon as that window goes** without a
//!   quit (1.42, #3274), as a quit would end it: a grade's chat, which only the window that
//!   started it watches. Every other program waits out the grace period.
//! - **A host with nothing a window could come back for stops at once** once its window has
//!   gone; one whose window has not linked yet waits for it.
//!
//! The grace period is a fraction of a second here; the programs, savers and channels are
//! stand-ins. A real host process is the app's behaviour suite, on CI.

#![cfg(unix)]

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use purlis_host::standin::{Savers, StandIns};
use purlis_host::{
    Channel, Compose, ConfinesNothing, Ended, HandOn, Hear, Hosting, Record, Stopped, ToCompose,
};
use purlis_session_protocol::auth::{Credentials, Scope};
use purlis_session_protocol::link::{self, Acceptor};
use purlis_session_protocol::session::{
    self, Client, Events, Exit, ProgramKind, Reporting, StartProgram,
};
use purlis_terminal::session::Spec;
use serde_json::Value;
use tokio::io::duplex;
use tokio::task::JoinHandle;

const PROJECT: &str = "01J9PRJCT00000000000000000";
const OTHER: &str = "01J9PRJCT00000000000000001";
const GRACE: Duration = Duration::from_millis(400);
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

/// Which projects' hook channels are open now.
#[derive(Clone, Default)]
struct Channels(Arc<Mutex<Vec<String>>>);

impl Channels {
    fn open(&self) -> Vec<String> {
        self.0.lock().unwrap().clone()
    }
}

impl Hear for Channels {
    fn open(
        &self,
        project: &str,
        _path: &Path,
        _hand_on: HandOn,
    ) -> Result<Option<Arc<dyn Channel>>, String> {
        self.0.lock().unwrap().push(project.to_owned());
        Ok(Some(Arc::new(OneChannel(
            self.clone(),
            project.to_owned(),
            Mutex::default(),
        ))))
    }
}

/// A project's channel, and the board it keeps: the moves it was told.
struct OneChannel(Channels, String, Mutex<Vec<Value>>);

impl Channel for OneChannel {
    fn report_to(&self, number: u32) -> Result<Reporting, String> {
        Ok(Reporting {
            socket: "/the/project/hooks.sock".into(),
            token: format!("token-{number}"),
        })
    }
    fn bind(&self, _number: u32, _pid: u32) {}
    fn ended(&self, _number: u32, _pid: u32) {}
    fn forget(&self, _number: u32) {}
    fn replied(&self, _id: u64, _reply: Value) {}
    fn moved(&self, _number: u32, moved: Value) {
        self.2.lock().unwrap().push(moved);
    }
    fn board(&self) -> Result<Value, String> {
        Ok(Value::Array(self.2.lock().unwrap().clone()))
    }
}

impl Drop for OneChannel {
    fn drop(&mut self) {
        self.0.0.lock().unwrap().retain(|open| *open != self.1);
    }
}

struct Host {
    hosting: Hosting,
    runs: StandIns,
    ends: Ends,
    channels: Channels,
    savers: Savers,
    project: tempfile::TempDir,
    other: tempfile::TempDir,
}

fn a_host() -> Host {
    let (runs, ends, channels, savers) = (
        StandIns::default(),
        Ends::default(),
        Channels::default(),
        Savers::default(),
    );
    Host {
        hosting: Hosting::with(Echo, runs.clone(), channels.clone(), ConfinesNothing)
            .recording_with(ends.clone())
            .saving_with(savers.clone()),
        runs,
        ends,
        channels,
        savers,
        project: tempfile::tempdir().unwrap(),
        other: tempfile::tempdir().unwrap(),
    }
}

/// A window's link to the host, subscribed, as the window's is.
struct Window {
    client: Client,
    _views: Acceptor,
    _events: Events,
}

impl Host {
    /// The host's life with a window that may come back, as the window's host lives it, its
    /// starter still there.
    fn lives(&self) -> JoinHandle<()> {
        let hosting = self.hosting.clone();
        tokio::spawn(async move {
            hosting
                .while_a_window_may_come_back(GRACE, std::future::pending())
                .await;
        })
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
        Window {
            client,
            _views: views,
            _events: events,
        }
    }

    async fn holding(&self, project: &str) -> Window {
        let window = self.window().await;
        let path = if project == PROJECT {
            self.project.path()
        } else {
            self.other.path()
        };
        window
            .client
            .hold_project(project, path.to_str().unwrap())
            .await
            .expect("the project is held");
        window
    }
}

impl Window {
    async fn starts(&self, project: &str, number: u32) -> String {
        self.client
            .start_program(StartProgram {
                number: Some(number),
                ..StartProgram::of(project, ProgramKind::Harness)
            })
            .await
            .expect("it starts")
    }
}

impl Window {
    async fn starts_ending_with_it(&self, project: &str, number: u32) -> String {
        self.client
            .start_program(StartProgram {
                number: Some(number),
                ends_with_its_window: true,
                ..StartProgram::of(project, ProgramKind::Harness)
            })
            .await
            .expect("it starts")
    }
}

async fn until(what: &str, done: impl Fn() -> bool) {
    let deadline = Instant::now() + PATIENCE;
    while !done() {
        assert!(Instant::now() < deadline, "{what} never happened");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[tokio::test]
async fn a_window_back_within_the_grace_period_finds_every_program_running() {
    let host = a_host();
    let lives = host.lives();
    host.holding(PROJECT).await.starts(PROJECT, 1).await;
    tokio::time::sleep(GRACE / 4).await;

    let back = host.holding(PROJECT).await;
    tokio::time::sleep(GRACE * 2).await;

    assert!(!host.runs.last().seen().stopped, "nothing was ended");
    assert_eq!(back.client.programs_of(PROJECT).await.unwrap().len(), 1);
    assert!(!lives.is_finished(), "the host goes on");
}

#[tokio::test]
async fn past_the_grace_period_with_no_window_every_program_ends_as_a_quit_and_the_host_stops() {
    let host = a_host();
    let lives = host.lives();
    {
        let window = host.holding(PROJECT).await;
        window.starts(PROJECT, 1).await;
        window.starts(PROJECT, 2).await;
        window
            .client
            .start_saving(PROJECT)
            .await
            .expect("it is saved");
    }
    tokio::time::sleep(GRACE / 2).await;
    assert!(host.ends.kept().is_empty(), "nothing ends within it");
    assert_eq!(host.savers.running(), 1, "it is saved meanwhile");

    tokio::time::timeout(PATIENCE, lives)
        .await
        .expect("the host stops past the grace period")
        .unwrap();

    assert_eq!(
        host.ends.kept(),
        [(1, Some(Stopped::Quit)), (2, Some(Stopped::Quit))]
    );
    assert_eq!(host.savers.running(), 0, "its saving stopped");
    assert!(host.channels.open().is_empty(), "its hook channel closed");
}

#[tokio::test]
async fn a_project_no_window_holds_again_is_let_go_of_when_its_grace_period_passes() {
    let host = a_host();
    let lives = host.lives();
    host.holding(PROJECT).await.starts(PROJECT, 1).await;
    // A window comes back, and opens another project only.
    let back = host.holding(OTHER).await;
    back.starts(OTHER, 1).await;

    until("the project nobody took back is let go of", || {
        host.channels.open() == [OTHER]
    })
    .await;

    assert_eq!(host.ends.kept(), [(1, Some(Stopped::Quit))]);
    assert!(
        !host.runs.last().seen().stopped,
        "the other project's goes on"
    );
    assert!(!lives.is_finished(), "a window is there");
    let again = host.holding(PROJECT).await;
    assert!(again.client.programs_of(PROJECT).await.unwrap().is_empty());
}

#[tokio::test]
async fn a_window_back_before_then_reads_how_each_program_ended_and_then_it_is_let_go_of() {
    let host = a_host();
    let lives = host.lives();
    host.holding(PROJECT).await.starts(PROJECT, 1).await;
    host.runs.last().ends(Exit::Code(3));
    tokio::time::sleep(GRACE / 4).await;

    let exits: Vec<Option<Exit>> = {
        let back = host.holding(PROJECT).await;
        back.client
            .call(session::Command::BoardMoved {
                project: PROJECT.into(),
                chat: 1,
                moved: serde_json::json!({"move": "exited"}),
            })
            .await
            .expect("the board kept is moved");
        back.client
            .programs_of(PROJECT)
            .await
            .unwrap()
            .into_iter()
            .map(|program| program.exit)
            .collect()
    };
    assert_eq!(exits, [Some(Exit::Code(3))]);

    tokio::time::timeout(PATIENCE, lives)
        .await
        .expect("a project with only ended programs is let go of in time, and the host stops")
        .unwrap();
    assert!(host.channels.open().is_empty());
    // Its board went with it: a window that holds the project again reads none of it.
    let again = host.holding(PROJECT).await;
    let board = again
        .client
        .call(session::Command::Board {
            project: PROJECT.into(),
        })
        .await
        .expect("a board");
    assert_eq!(board, session::Answer::Board(serde_json::json!([])));
    assert!(again.client.programs_of(PROJECT).await.unwrap().is_empty());
}

#[tokio::test]
async fn a_host_whose_window_went_with_nothing_to_come_back_for_stops_at_once() {
    let host = a_host();
    let lives = host.lives();
    drop(host.holding(PROJECT).await);

    tokio::time::timeout(GRACE / 2, lives)
        .await
        .expect("it stops before the grace period would have passed")
        .unwrap();
}

#[tokio::test]
async fn a_host_whose_window_has_not_linked_yet_waits_for_it() {
    let host = a_host();
    let lives = host.lives();

    tokio::time::sleep(GRACE * 2).await;

    assert!(!lives.is_finished());
}

#[tokio::test]
async fn a_host_whose_starter_went_before_linking_stops_at_once() {
    let host = a_host();
    let (went, goes) = tokio::sync::oneshot::channel::<()>();
    let hosting = host.hosting.clone();
    let lives = tokio::spawn(async move {
        hosting
            .while_a_window_may_come_back(GRACE, async {
                let _ = goes.await;
            })
            .await;
    });
    drop(went);

    tokio::time::timeout(GRACE / 2, lives)
        .await
        .expect("nothing to come back for")
        .unwrap();
}

#[tokio::test]
async fn a_quit_stops_a_host_with_a_window_there() {
    let host = a_host();
    let lives = host.lives();
    let window = host.holding(PROJECT).await;
    window.starts(PROJECT, 1).await;

    window.client.quit().await.expect("it quits");

    tokio::time::timeout(PATIENCE, lives)
        .await
        .expect("the host stops")
        .unwrap();
    assert_eq!(host.ends.kept(), [(1, Some(Stopped::Quit))]);
}

#[tokio::test]
async fn a_program_that_ends_with_its_window_ends_once_the_window_goes_and_the_rest_wait() {
    let host = a_host();
    let lives = host.lives();
    {
        let window = host.holding(PROJECT).await;
        window.starts(PROJECT, 1).await;
        window.starts_ending_with_it(PROJECT, 2).await;
    }

    until("the program that ends with its window is ended", || {
        host.ends.kept() == [(2, Some(Stopped::Quit))]
    })
    .await;
    tokio::time::sleep(GRACE / 4).await;
    assert_eq!(
        host.ends.kept(),
        [(2, Some(Stopped::Quit))],
        "the other waits for its window"
    );

    let back = host.holding(PROJECT).await;
    let programs = back.client.programs_of(PROJECT).await.unwrap();
    assert_eq!(programs.len(), 1, "a window back finds the other alone");
    assert!(!lives.is_finished(), "a window is there");
}

#[tokio::test]
async fn a_window_that_quits_ends_a_program_that_ends_with_it_as_a_quit() {
    let host = a_host();
    let lives = host.lives();
    let window = host.holding(PROJECT).await;
    window.starts_ending_with_it(PROJECT, 1).await;

    window.client.quit().await.expect("it quits");

    tokio::time::timeout(PATIENCE, lives)
        .await
        .expect("the host stops at the quit")
        .unwrap();
    assert_eq!(host.ends.kept(), [(1, Some(Stopped::Quit))]);
}

#[tokio::test]
async fn a_host_whose_only_program_ends_with_its_window_ends_it_and_then_stops() {
    let host = a_host();
    let lives = host.lives();
    host.holding(PROJECT)
        .await
        .starts_ending_with_it(PROJECT, 1)
        .await;

    tokio::time::timeout(PATIENCE, lives)
        .await
        .expect("the host stops once nothing is left")
        .unwrap();
    assert_eq!(
        host.ends.kept(),
        [(1, Some(Stopped::Quit))],
        "its end was kept before the host stopped"
    );
    assert!(host.runs.last().seen().stopped);
}

#[tokio::test]
async fn a_window_link_that_ends_while_another_stays_ends_nothing() {
    let host = a_host();
    let _lives = host.lives();
    let stays = host.holding(PROJECT).await;
    host.holding(PROJECT)
        .await
        .starts_ending_with_it(PROJECT, 1)
        .await;
    tokio::time::sleep(GRACE / 4).await;

    assert!(host.ends.kept().is_empty(), "a window is still there");
    assert_eq!(stays.client.programs_of(PROJECT).await.unwrap().len(), 1);
}

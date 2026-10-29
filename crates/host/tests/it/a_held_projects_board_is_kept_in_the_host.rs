// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A held project's board is kept in the session host (#2194), spoken to over the session
//! protocol on an in-memory link: the window tells the host each move it makes on its own copy
//! (`board_moved`), in order, and reads the host's board whole (`board`). The project and its
//! board outlive a window whose link ends while the project has a program the window has not
//! closed, running or ended, so a window that holds it again reads every chat as the host kept
//! it, and a program that ends with no window there is told to the board. The board itself is a stand-in here; the real one is the core's
//! (`purlis_core::state::kept`), and the window and the host together are the app's behaviour
//! suite, on CI.

#![cfg(unix)]

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use purlis_host::standin::StandIns;
use purlis_host::{Channel, Compose, HandOn, Hear, Hosting, ToCompose};
use purlis_session_protocol::auth::{Credentials, Scope};
use purlis_session_protocol::link::{self, Acceptor};
use purlis_session_protocol::session::{
    self, Answer, Client, Command, Events, Exit, ProgramKind, Reporting, StartProgram,
};
use purlis_terminal::session::Spec;
use serde_json::{Value, json};
use tokio::io::duplex;

const PROJECT: &str = "01J9PRJCT00000000000000000";
const PATIENCE: Duration = Duration::from_secs(5);

static HELD: std::sync::LazyLock<Credentials> =
    std::sync::LazyLock::new(|| Credentials::mint().unwrap());

/// What the stand-in channels were told, in order, and what they hand lines on with.
#[derive(Clone, Default)]
struct Kept {
    said: Arc<Mutex<Vec<String>>>,
    moves: Arc<Mutex<Vec<Value>>>,
    hand_on: Arc<Mutex<Option<HandOn>>>,
    /// Its board is more than one answer carries, for this reason.
    too_large: Arc<Mutex<Option<String>>>,
}

impl Kept {
    fn said(&self) -> Vec<String> {
        self.said.lock().unwrap().clone()
    }

    fn say(&self, what: String) {
        self.said.lock().unwrap().push(what);
    }

    /// Hands a line on, as the channel would one its hooks wrote: whether a window took it.
    fn hands_on(&self, id: u64, line: Value) -> bool {
        let hand_on = self.hand_on.lock().unwrap().clone().expect("a channel");
        hand_on(id, line)
    }
}

struct Keeps(Kept);

impl Hear for Keeps {
    fn open(
        &self,
        project: &str,
        _path: &Path,
        hand_on: HandOn,
    ) -> Result<Option<Arc<dyn Channel>>, String> {
        self.0.say(format!("open {project}"));
        *self.0.hand_on.lock().unwrap() = Some(hand_on);
        Ok(Some(Arc::new(KeepingChannel(self.0.clone()))))
    }
}

/// A channel whose board is the list of moves it was told.
struct KeepingChannel(Kept);

impl Channel for KeepingChannel {
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

    fn moved(&self, number: u32, moved: Value) {
        self.0
            .moves
            .lock()
            .unwrap()
            .push(json!({"chat": number, "moved": moved}));
    }

    fn board(&self) -> Result<Value, String> {
        match self.0.too_large.lock().unwrap().clone() {
            Some(why) => Err(why),
            None => Ok(json!({ "moves": *self.0.moves.lock().unwrap() })),
        }
    }

    fn ended_with_no_window(&self, number: u32, exit: &Exit) {
        self.0
            .say(format!("{number} ended with no window: {exit:?}"));
    }

    fn no_window(&self) {
        self.0.say("no window".into());
    }
}

impl Drop for KeepingChannel {
    fn drop(&mut self) {
        self.0.say("closed".into());
    }
}

struct Echoes;

impl Compose for Echoes {
    fn compose(&self, starting: &ToCompose<'_>) -> Result<Spec, String> {
        Ok(Spec::new("/bin/echo", starting.size))
    }
}

struct Host {
    hosting: Hosting,
    runs: StandIns,
    kept: Kept,
    project: tempfile::TempDir,
}

fn a_host() -> Host {
    let runs = StandIns::default();
    let kept = Kept::default();
    Host {
        hosting: Hosting::hearing_with(Echoes, runs.clone(), Keeps(kept.clone())),
        runs,
        kept,
        project: tempfile::tempdir().unwrap(),
    }
}

/// A window's link to the host, subscribed, holding the project.
struct Window {
    client: Client,
    _events: Events,
    _views: Acceptor,
}

impl Host {
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
            _events: events,
            _views: views,
        }
    }

    async fn holding(&self) -> Window {
        let window = self.window().await;
        window
            .client
            .hold_project(PROJECT, self.project.path().to_str().unwrap())
            .await
            .expect("the project is held");
        window
    }

    /// Waits until no window takes a line of the project's: the last link holding it ended.
    async fn until_no_window_holds_it(&self) {
        let deadline = Instant::now() + PATIENCE;
        while self.kept.hands_on(99, json!({"doing": {}})) {
            assert!(
                Instant::now() < deadline,
                "a window still holds the project"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }
}

impl Window {
    async fn moves(&self, chat: u32, moved: Value) {
        self.client
            .call(Command::BoardMoved {
                project: PROJECT.into(),
                chat,
                moved,
            })
            .await
            .expect("the move is made");
    }

    async fn board(&self) -> Result<Value, session::CallError> {
        match self
            .client
            .call(Command::Board {
                project: PROJECT.into(),
            })
            .await?
        {
            Answer::Board(board) => Ok(board),
            other => panic!("a board was answered with {other:?}"),
        }
    }

    async fn starts(&self) -> String {
        self.client
            .start_program(StartProgram::of(PROJECT, ProgramKind::Harness))
            .await
            .expect("it starts")
    }
}

#[tokio::test]
async fn each_move_the_window_makes_is_made_on_the_hosts_board_in_order() {
    let host = a_host();
    let window = host.holding().await;

    window.moves(1, json!({"move": "opened"})).await;
    window.moves(1, json!({"move": "ignored"})).await;

    assert_eq!(
        window.board().await.unwrap(),
        json!({"moves": [
            {"chat": 1, "moved": {"move": "opened"}},
            {"chat": 1, "moved": {"move": "ignored"}},
        ]})
    );
}

#[tokio::test]
async fn a_window_back_after_its_link_ended_reads_the_board_as_the_host_kept_it() {
    let host = a_host();
    {
        let window = host.holding().await;
        window.starts().await;
        window.moves(1, json!({"move": "opened"})).await;
    }
    host.until_no_window_holds_it().await;

    // While no window is there, the program ends, and the board is told.
    host.runs.last().ends(Exit::Code(3));
    let deadline = Instant::now() + PATIENCE;
    while !host
        .kept
        .said()
        .iter()
        .any(|said| said.contains("ended with no window"))
    {
        assert!(
            Instant::now() < deadline,
            "the board was never told of the end"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let back = host.holding().await;

    assert_eq!(
        back.board().await.unwrap(),
        json!({"moves": [{"chat": 1, "moved": {"move": "opened"}}]})
    );
    assert_eq!(
        host.kept.said(),
        [
            format!("open {PROJECT}"),
            "no window".to_owned(),
            "1 ended with no window: Code(3)".to_owned()
        ],
        "one channel, kept open while the window was gone"
    );
}

#[tokio::test]
async fn a_program_that_ends_while_a_window_holds_its_project_is_the_windows_to_tell() {
    let host = a_host();
    let window = host.holding().await;
    window.starts().await;

    host.runs.last().ends(Exit::Code(0));
    tokio::time::sleep(Duration::from_millis(200)).await;

    assert_eq!(host.kept.said(), [format!("open {PROJECT}")]);
}

#[tokio::test]
async fn a_project_that_runs_nothing_is_let_go_of_with_the_last_link_that_held_it() {
    let host = a_host();
    {
        let window = host.holding().await;
        window.moves(1, json!({"move": "opened"})).await;
    }
    let deadline = Instant::now() + PATIENCE;
    while host.kept.said().last().map(String::as_str) != Some("closed") {
        assert!(Instant::now() < deadline, "the channel was never closed");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[tokio::test]
async fn a_link_that_does_not_hold_the_project_neither_moves_nor_reads_its_board() {
    let host = a_host();
    let window = host.holding().await;
    let other = host.window().await;

    let read = other.board().await.unwrap_err();
    let moved = other
        .client
        .call(Command::BoardMoved {
            project: PROJECT.into(),
            chat: 1,
            moved: json!({"move": "ignored"}),
        })
        .await
        .unwrap_err();

    for refused in [read, moved] {
        assert_eq!(
            refused.refusal().map(|r| r.code.as_str()),
            Some(session::NO_SUCH_PROJECT)
        );
    }
    assert_eq!(window.board().await.unwrap(), json!({"moves": []}));
}

#[tokio::test]
async fn a_project_whose_program_ended_but_was_not_closed_is_kept_after_its_link_ends() {
    let host = a_host();
    {
        let window = host.holding().await;
        window.starts().await;
        window.moves(1, json!({"move": "opened"})).await;
        host.runs.last().ends(Exit::Code(0));
        window.moves(1, json!({"move": "exited", "code": 0})).await;
    }
    host.until_no_window_holds_it().await;
    tokio::time::sleep(Duration::from_millis(200)).await;

    assert_eq!(
        host.kept.said(),
        [format!("open {PROJECT}"), "no window".to_owned()],
        "the channel stays: a window that comes back reads how the program ended"
    );
    let back = host.holding().await;
    assert_eq!(
        back.board().await.unwrap(),
        json!({"moves": [
            {"chat": 1, "moved": {"move": "opened"}},
            {"chat": 1, "moved": {"move": "exited", "code": 0}},
        ]})
    );
}

#[tokio::test]
async fn a_project_whose_programs_were_all_closed_is_let_go_of_with_the_last_link_that_held_it() {
    let host = a_host();
    {
        let window = host.holding().await;
        let program = window.starts().await;
        host.runs.last().ends(Exit::Code(0));
        window.client.close(&program).await.expect("it is closed");
    }
    let deadline = Instant::now() + PATIENCE;
    while host.kept.said().last().map(String::as_str) != Some("closed") {
        assert!(Instant::now() < deadline, "the channel was never closed");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

/// A kept board too large to hand over is refused in words (#2006), so the window can say so
/// rather than start from its own copy unawares.
#[tokio::test]
async fn a_board_too_large_to_hand_over_is_refused_saying_so() {
    let host = a_host();
    let window = host.holding().await;
    *host.kept.too_large.lock().unwrap() = Some("it is 70000 bytes".into());

    let refused = window.board().await.unwrap_err();

    let refusal = refused.refusal().expect("a refusal");
    assert_eq!(refusal.code, session::TOO_LARGE);
    assert!(refusal.why.contains("70000 bytes"), "{}", refusal.why);
}

/// The window's link ends while lines it was handed wait for its reply (#2006): the channel is
/// told no window holds the project now, so the lines are heard by the board it keeps. A link
/// that ends while another still holds the project tells it nothing.
#[tokio::test]
async fn a_channel_is_told_once_no_window_holds_its_project() {
    let host = a_host();
    let staying = host.holding().await;
    staying.starts().await;
    drop(host.holding().await);
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(host.kept.said(), [format!("open {PROJECT}")]);

    drop(staying);
    host.until_no_window_holds_it().await;

    let deadline = Instant::now() + PATIENCE;
    while host.kept.said().last().map(String::as_str) != Some("no window") {
        assert!(Instant::now() < deadline, "the channel was never told");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

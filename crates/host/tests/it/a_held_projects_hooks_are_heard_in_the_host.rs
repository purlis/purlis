// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A held project's hook channel lives in the session host (#2000), spoken to over the session
//! protocol on an in-memory link: the host opens the channel as it first holds the project,
//! deals each chat its number and has the channel issue it its token, records its program and
//! lets both go as it ends or closes, and pushes each line the channel hands on to the window
//! that holds the project, whose reply goes back to the channel. The channel is a stand-in here;
//! the real one, a socket and its admission checks, is the app's behaviour suite, on CI.

#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use purlis_host::standin::StandIns;
use purlis_host::{Channel, Compose, HandOn, Hear, Hosting, ToCompose};
use purlis_session_protocol::auth::{Credentials, Scope};
use purlis_session_protocol::link::{self, Acceptor};
use purlis_session_protocol::session::{
    self, Client, Command, Events, Exit, Launch, ProgramKind, Pushed, Reporting, StartProgram, Told,
};
use purlis_terminal::session::Spec;
use serde_json::json;
use tokio::io::duplex;
use tokio::time::timeout;

const PROJECT: &str = "01J9PRJCT00000000000000000";
const PATIENCE: Duration = Duration::from_secs(5);

static HELD: std::sync::LazyLock<Credentials> =
    std::sync::LazyLock::new(|| Credentials::mint().unwrap());

/// What the stand-in channels were asked, in order, and what each was handed to hand lines on.
#[derive(Clone, Default)]
struct Heard {
    said: Arc<Mutex<Vec<String>>>,
    hand_on: Arc<Mutex<Option<HandOn>>>,
    /// What each channel was handed, by its folder's name.
    hands_on_at: Arc<Mutex<std::collections::HashMap<String, HandOn>>>,
}

impl Heard {
    fn said(&self) -> Vec<String> {
        self.said.lock().unwrap().clone()
    }

    fn say(&self, what: String) {
        self.said.lock().unwrap().push(what);
    }

    /// Hands a line on, as the channel would one its hooks wrote: whether a window took it.
    fn hands_on(&self, id: u64, line: serde_json::Value) -> bool {
        let hand_on = self.hand_on.lock().unwrap().clone().expect("a channel");
        hand_on(id, line)
    }

    /// [`Self::hands_on`], from the channel of the place at `folder`.
    fn hands_on_at(&self, folder: &Path, id: u64, line: serde_json::Value) -> bool {
        let hand_on = self.hands_on_at.lock().unwrap()[&file_name(folder)].clone();
        hand_on(id, line)
    }
}

/// Opens a stand-in channel that keeps what it is asked in [`Heard`].
struct Hears(Heard);

impl Hear for Hears {
    fn open(
        &self,
        project: &str,
        path: &Path,
        hand_on: HandOn,
    ) -> Result<Option<Arc<dyn Channel>>, String> {
        self.0.say(format!("open {project} at {}", file_name(path)));
        self.0
            .hands_on_at
            .lock()
            .unwrap()
            .insert(file_name(path), hand_on.clone());
        *self.0.hand_on.lock().unwrap() = Some(hand_on);
        Ok(Some(Arc::new(StandInChannel(self.0.clone()))))
    }
}

fn file_name(path: &Path) -> String {
    path.file_name().unwrap().to_string_lossy().into_owned()
}

struct StandInChannel(Heard);

impl Channel for StandInChannel {
    fn report_to(&self, number: u32) -> Result<Reporting, String> {
        self.0.say(format!("token for {number}"));
        Ok(Reporting {
            socket: "/the/project/hooks.sock".into(),
            token: format!("token-{number}"),
        })
    }

    fn bind(&self, number: u32, pid: u32) {
        self.0.say(format!("bind {number} to {pid}"));
    }

    fn ended(&self, number: u32, pid: u32) {
        self.0.say(format!("{number}'s {pid} ended"));
    }

    fn forget(&self, number: u32) {
        self.0.say(format!("forget {number}"));
    }

    fn replied(&self, id: u64, reply: serde_json::Value) {
        self.0.say(format!("reply to {id}: {reply}"));
    }
}

impl Drop for StandInChannel {
    fn drop(&mut self) {
        self.0.say("closed".into());
    }
}

/// Composes a start as `/bin/echo <where it reports> <its token>`, from its launch.
struct Reports;

impl Compose for Reports {
    fn compose(&self, starting: &ToCompose<'_>) -> Result<Spec, String> {
        let reporting = starting
            .start
            .launch
            .as_ref()
            .and_then(|launch| launch.reporting.clone());
        let (socket, token) = reporting
            .map(|reporting| (reporting.socket, reporting.token))
            .unwrap_or_default();
        Ok(Spec::new("/bin/echo", starting.size).args([socket, token]))
    }
}

struct Linked {
    client: Client,
    events: Events,
    runs: StandIns,
    heard: Heard,
    hosting: Hosting,
    project: tempfile::TempDir,
    _views: Acceptor,
}

async fn linked() -> Linked {
    let runs = StandIns::default();
    let heard = Heard::default();
    let hosting = Hosting::hearing_with(Reports, runs.clone(), Hears(heard.clone()));
    let (client, views, events) = another_window(&hosting).await;
    client.subscribe(session::Since::new()).await.unwrap();
    Linked {
        client,
        events,
        runs,
        heard,
        hosting,
        project: tempfile::tempdir().unwrap(),
        _views: views,
    }
}

async fn another_window(hosting: &Hosting) -> (Client, Acceptor, Events) {
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
    Client::new(client.unwrap())
}

impl Linked {
    fn path(&self) -> &str {
        self.project.path().to_str().unwrap()
    }

    async fn hold(&self) {
        self.client
            .hold_project(PROJECT, self.path())
            .await
            .expect("the project is held");
    }

    fn folder(&self) -> String {
        file_name(&PathBuf::from(self.path()))
    }
}

/// A start the window composed, which says where the chat reports as a window of #1998 did.
fn the_windows_start(number: Option<u32>) -> StartProgram {
    StartProgram {
        number,
        launch: Some(Launch {
            program: "/bin/sh".into(),
            args: Vec::new(),
            cwd: None,
            set: Vec::new(),
            strip: Vec::new(),
            pass: Vec::new(),
            declared: Vec::new(),
            harness: None,
            git_hooks: None,
            reporting: Some(Reporting {
                socket: "/the/window/s/own.sock".into(),
                token: "the-window-s-token".into(),
            }),
            confinement: None,
            prompt_key: None,
        }),
        ..StartProgram::of(PROJECT, ProgramKind::Harness)
    }
}

#[tokio::test]
async fn holding_a_project_opens_its_hook_channel_once_at_its_folder() {
    let linked = linked().await;
    let (other, _views, _events) = another_window(&linked.hosting).await;

    linked.hold().await;
    linked.hold().await;
    other.hold_project(PROJECT, linked.path()).await.unwrap();

    assert_eq!(
        linked.heard.said(),
        [format!("open {PROJECT} at {}", linked.folder())]
    );
}

#[tokio::test]
async fn a_chat_is_issued_its_token_by_its_projects_channel_and_reports_nowhere_else() {
    let linked = linked().await;
    linked.hold().await;

    let started = linked
        .client
        .start_program_told(the_windows_start(Some(3)))
        .await
        .expect("it starts");

    assert_eq!(started.number, Some(3));
    let (spec, _) = &linked.runs.ran()[0];
    assert_eq!(spec.args, ["/the/project/hooks.sock", "token-3"]);
    assert_eq!(
        linked.heard.said()[1..],
        ["token for 3".to_owned(), "bind 3 to 4242".to_owned()]
    );
}

#[tokio::test]
async fn a_chats_program_that_ends_is_let_go_of_and_a_closed_chat_is_forgotten() {
    let mut linked = linked().await;
    linked.hold().await;
    let program = linked
        .client
        .start_program(the_windows_start(None))
        .await
        .unwrap();

    linked.runs.last().ends(Exit::Code(0));
    loop {
        match timeout(PATIENCE, linked.events.recv()).await {
            Ok(Some(Told::Pushed(Pushed::Ended { .. }))) => break,
            Ok(Some(_)) => {}
            other => panic!("the end was never told: {other:?}"),
        }
    }
    linked.client.close(&program).await.unwrap();

    assert_eq!(
        linked.heard.said()[1..],
        [
            "token for 1".to_owned(),
            "bind 1 to 4242".to_owned(),
            "1's 4242 ended".to_owned(),
            "forget 1".to_owned(),
        ]
    );
}

#[tokio::test]
async fn numbers_are_dealt_by_the_host_never_twice_and_never_one_that_went_out_before() {
    let linked = linked().await;
    linked.hold().await;
    linked
        .client
        .call(Command::DealtAlready {
            project: PROJECT.into(),
            dealt: 6,
        })
        .await
        .unwrap();

    let dealt = linked.client.deal(PROJECT).await.unwrap();
    let started = linked
        .client
        .start_program_told(the_windows_start(None))
        .await
        .unwrap();
    let again = linked.client.deal(PROJECT).await.unwrap();

    assert_eq!(dealt, 7);
    assert_eq!(started.number, Some(8));
    assert_eq!(again, 9);
}

#[tokio::test]
async fn past_the_last_number_none_is_dealt_and_nothing_starts() {
    let linked = linked().await;
    linked.hold().await;
    linked
        .client
        .call(Command::DealtAlready {
            project: PROJECT.into(),
            dealt: u32::MAX,
        })
        .await
        .unwrap();

    let dealt = linked.client.deal(PROJECT).await.unwrap_err();
    let started = linked
        .client
        .start_program_told(the_windows_start(None))
        .await
        .unwrap_err();

    for refused in [dealt, started] {
        assert_eq!(
            refused.refusal().map(|r| r.code.as_str()),
            Some(session::NOT_STARTED)
        );
    }
    assert!(linked.runs.ran().is_empty());
}

#[tokio::test]
async fn a_number_is_dealt_only_in_a_held_project() {
    let linked = linked().await;

    let refused = linked.client.deal(PROJECT).await.unwrap_err();

    assert_eq!(
        refused.refusal().map(|r| r.code.as_str()),
        Some(session::NO_SUCH_PROJECT)
    );
}

#[tokio::test]
async fn a_line_its_hooks_wrote_reaches_the_window_holding_it_and_the_reply_comes_back() {
    let mut linked = linked().await;
    linked.hold().await;

    let line = json!({"report": {"chat": 1, "event": "stop"}});
    assert!(linked.heard.hands_on(12, line.clone()), "a window holds it");
    let pushed = loop {
        match timeout(PATIENCE, linked.events.recv()).await {
            Ok(Some(Told::Pushed(pushed @ Pushed::Heard { .. }))) => break pushed,
            Ok(Some(_)) => {}
            other => panic!("the line was never pushed: {other:?}"),
        }
    };
    linked
        .client
        .call(Command::ReplyHeard {
            project: PROJECT.into(),
            heard: 12,
            reply: json!("taken"),
        })
        .await
        .unwrap();

    assert_eq!(
        pushed,
        Pushed::Heard {
            project: PROJECT.into(),
            heard: 12,
            line,
            // It records the line under the project as it holds it (1.40, #3400).
            pathless: true,
        }
    );
    assert_eq!(
        linked.heard.said().last().unwrap(),
        "reply to 12: \"taken\""
    );
}

#[tokio::test]
async fn a_window_that_does_not_hold_the_project_is_neither_told_its_lines_nor_heard_replying() {
    let linked = linked().await;
    linked.hold().await;
    let (other, _views, mut events) = another_window(&linked.hosting).await;
    other.subscribe(session::Since::new()).await.unwrap();
    linked.client.let_go_of_project(PROJECT).await.unwrap();
    linked.hold().await;

    let refused = other
        .call(Command::ReplyHeard {
            project: PROJECT.into(),
            heard: 12,
            reply: json!("taken"),
        })
        .await
        .unwrap_err();
    linked.heard.hands_on(13, json!({"doing": {}}));

    assert_eq!(
        refused.refusal().map(|r| r.code.as_str()),
        Some(session::NO_SUCH_PROJECT)
    );
    assert!(
        timeout(Duration::from_millis(200), events.recv())
            .await
            .is_err(),
        "the other window was told a line of a project it does not hold"
    );
    assert!(
        !linked
            .heard
            .said()
            .iter()
            .any(|said| said.starts_with("reply to 12")),
    );
}

#[tokio::test]
async fn each_clone_of_a_project_has_its_own_channel_and_its_lines_reach_only_its_link() {
    // #2178: two clones of one project, each held on a link of its own.
    let linked = linked().await;
    linked.hold().await;
    let clone = tempfile::tempdir().unwrap();
    let (other, _views, mut told) = another_window(&linked.hosting).await;
    other.subscribe(session::Since::new()).await.unwrap();
    other
        .hold_project(PROJECT, clone.path().to_str().unwrap())
        .await
        .unwrap();
    let mut events = linked.events;

    let line = json!({"report": {"chat": 1, "event": "stop"}});
    assert!(linked.heard.hands_on_at(clone.path(), 21, line.clone()));
    let pushed = loop {
        match timeout(PATIENCE, told.recv()).await {
            Ok(Some(Told::Pushed(pushed @ Pushed::Heard { .. }))) => break pushed,
            Ok(Some(_)) => {}
            other => panic!("the line was never pushed: {other:?}"),
        }
    };

    assert_eq!(
        linked.heard.said(),
        [
            format!("open {PROJECT} at {}", file_name(linked.project.path())),
            format!("open {PROJECT} at {}", file_name(clone.path())),
        ]
    );
    assert_eq!(
        pushed,
        Pushed::Heard {
            project: PROJECT.into(),
            heard: 21,
            line,
            // It records the line under the project as it holds it (1.40, #3400).
            pathless: true,
        }
    );
    let first_told = loop {
        match timeout(Duration::from_millis(200), events.recv()).await {
            Ok(Some(Told::Pushed(Pushed::Heard { .. }))) => break true,
            Ok(Some(_)) => {}
            _ => break false,
        }
    };
    assert!(
        !first_told,
        "the first clone's window was told the second's line"
    );
}

#[tokio::test]
async fn a_project_nobody_holds_any_more_closes_its_channel_and_hands_nothing_on() {
    let linked = linked().await;
    linked.hold().await;

    linked.client.let_go_of_project(PROJECT).await.unwrap();

    assert_eq!(linked.heard.said().last().unwrap(), "closed");
    assert!(!linked.heard.hands_on(12, json!({"doing": {}})));
}

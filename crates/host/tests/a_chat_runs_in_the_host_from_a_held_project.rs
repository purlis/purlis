// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What the session host does with the programs a client starts in it (#1998), spoken over the
//! session protocol on an in-memory link: it holds a project, deals each chat's number in it,
//! runs what its binary composed, and then views, drives, stops and closes it, telling a
//! subscriber when it ends. The program is a stand-in, so none of this needs a terminal; the
//! same over a real terminal is the app's behaviour suite, on CI.

#![cfg(unix)]

use std::time::Duration;

use purlis_host::standin::StandIns;
use purlis_host::{Compose, Hosting, ToCompose, Window};
use purlis_session_protocol::auth::{Credentials, Scope};
use purlis_session_protocol::link::{self, Acceptor};
use purlis_session_protocol::local::Chats;
use purlis_session_protocol::session::{
    self, Client, Events, Exit, ProgramKind, Pushed, Readiness, StartProgram, Told, ViewKind,
};
use purlis_session_protocol::view::{Chunk, Viewer};
use purlis_terminal::engine::Size;
use purlis_terminal::session::Spec;
use tokio::io::duplex;
use tokio::time::timeout;

const PROJECT: &str = "01J9PRJCT00000000000000000";
const PATIENCE: Duration = Duration::from_secs(5);

static HELD: std::sync::LazyLock<Credentials> =
    std::sync::LazyLock::new(|| Credentials::mint().unwrap());

/// Composes a start as `/bin/echo <project path> <number>`, at the size asked; refuses one whose
/// persona is `nobody`.
struct Echo;

impl Compose for Echo {
    fn compose(&self, starting: &ToCompose<'_>) -> Result<Spec, String> {
        if starting.start.persona.as_deref() == Some("nobody") {
            return Err("there is no persona nobody in this project".into());
        }
        Ok(Spec::new("/bin/echo", starting.size).args([
            starting.project_path.display().to_string(),
            starting.number.to_string(),
        ]))
    }
}

struct Linked {
    client: Client,
    views: Acceptor,
    events: Events,
    runs: StandIns,
    hosting: Hosting,
    project: tempfile::TempDir,
}

async fn linked() -> Linked {
    let runs = StandIns::default();
    let hosting = Hosting::running_with(Echo, runs.clone());
    let (client, views, events) = another_window(&hosting).await;
    Linked {
        client,
        views,
        events,
        runs,
        hosting,
        project: tempfile::tempdir().unwrap(),
    }
}

/// Another `local-ui` link to `hosting`.
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
    async fn hold(&self) {
        self.client
            .hold_project(PROJECT, self.project.path().to_str().unwrap())
            .await
            .expect("the project is held");
    }
}

fn a_harness() -> StartProgram {
    StartProgram {
        cols: Some(100),
        rows: Some(30),
        ..StartProgram::of(PROJECT, ProgramKind::Harness)
    }
}

#[tokio::test]
async fn a_start_in_a_project_the_host_does_not_hold_is_refused_and_runs_nothing() {
    let linked = linked().await;

    let refused = linked.client.start_program(a_harness()).await.unwrap_err();

    assert_eq!(
        refused.refusal().map(|r| r.code.as_str()),
        Some(session::NO_SUCH_PROJECT)
    );
    assert!(linked.runs.ran().is_empty());
}

#[tokio::test]
async fn a_start_runs_what_the_binary_composed_in_the_held_project_and_says_its_number_and_process()
{
    let linked = linked().await;
    linked.hold().await;

    let started = linked
        .client
        .start_program_told(a_harness())
        .await
        .expect("it starts");

    assert_eq!(started.number, Some(1));
    assert_eq!(started.process, Some(4242));
    let (spec, _) = &linked.runs.ran()[0];
    assert_eq!(spec.program, "/bin/echo");
    assert_eq!(
        spec.args,
        [linked.project.path().as_os_str().to_owned(), "1".into()]
    );
    assert_eq!(
        spec.size,
        Size {
            columns: 100,
            rows: 30
        }
    );
    let listed = linked.client.list().await.unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].chat, started.program);
    assert_eq!(listed[0].project, PROJECT);
    assert_eq!(listed[0].number, Some(1));
    assert_eq!(listed[0].state, "running");
}

#[tokio::test]
async fn a_chat_keeps_the_number_and_id_it_asks_for_and_a_number_in_use_is_dealt_past() {
    let linked = linked().await;
    linked.hold().await;

    let first = linked
        .client
        .start_program_told(StartProgram {
            number: Some(7),
            program: Some("01J9ZQ5000000000000000000A".into()),
            run: Some("01J9ZQ7000000000000000000C".into()),
            ..a_harness()
        })
        .await
        .unwrap();
    let again = linked
        .client
        .start_program_told(StartProgram {
            number: Some(7),
            ..a_harness()
        })
        .await
        .unwrap();
    let fresh = linked.client.start_program_told(a_harness()).await.unwrap();

    assert_eq!(first.program, "01J9ZQ5000000000000000000A");
    assert_eq!(first.number, Some(7));
    assert_eq!(again.number, Some(8), "7 is taken, so the next is dealt");
    assert_eq!(fresh.number, Some(9), "a number dealt is never dealt again");
    let listed = linked.client.list().await.unwrap();
    let kept = listed.iter().find(|chat| chat.number == Some(7)).unwrap();
    assert_eq!(kept.run.as_deref(), Some("01J9ZQ7000000000000000000C"));
}

#[tokio::test]
async fn a_start_its_binary_cannot_compose_is_refused_with_why_and_runs_nothing() {
    let linked = linked().await;
    linked.hold().await;

    let refused = linked
        .client
        .start_program(StartProgram {
            persona: Some("nobody".into()),
            ..a_harness()
        })
        .await
        .unwrap_err();

    let refusal = refused.refusal().expect("a refusal");
    assert_eq!(refusal.code, session::NOT_STARTED);
    assert_eq!(refusal.why, "there is no persona nobody in this project");
    assert!(linked.runs.ran().is_empty());
}

#[tokio::test]
async fn a_project_let_go_of_starts_nothing_more() {
    let linked = linked().await;
    linked.hold().await;
    linked.client.let_go_of_project(PROJECT).await.unwrap();

    let refused = linked.client.start_program(a_harness()).await.unwrap_err();

    assert_eq!(
        refused.refusal().map(|r| r.code.as_str()),
        Some(session::NO_SUCH_PROJECT)
    );
}

#[tokio::test]
async fn a_project_that_is_not_a_folder_here_is_not_held() {
    let linked = linked().await;

    let refused = linked
        .client
        .hold_project(PROJECT, "/no/such/folder/anywhere")
        .await
        .unwrap_err();

    assert_eq!(
        refused.refusal().map(|r| r.code.as_str()),
        Some(session::NO_SUCH_PROJECT)
    );
}

#[tokio::test]
async fn what_is_typed_reaches_the_program_and_its_view_draws_it_after_its_screen() {
    let mut linked = linked().await;
    linked.hold().await;
    let program = linked.client.start_program(a_harness()).await.unwrap();
    linked.runs.last().writes(b"ready> ");

    let attached = linked
        .client
        .attach_view(&program, ViewKind::Terminal)
        .await
        .unwrap();
    assert_eq!((attached.cols, attached.rows), (100, 30));
    let mut reader = Viewer::default()
        .accept(linked.views.accept().await.unwrap())
        .await
        .unwrap();
    linked.client.write(&program, b"hello").await.unwrap();

    let mut snapshot = Vec::new();
    let mut live = Vec::new();
    while live != b"hello" {
        match timeout(PATIENCE, reader.next())
            .await
            .expect("the view draws")
            .expect("the view is open")
            .unwrap()
        {
            Chunk::Snapshot(bytes) => snapshot.extend_from_slice(&bytes),
            Chunk::Live(bytes) => live.extend_from_slice(&bytes),
        }
    }
    assert_eq!(snapshot, b"ready> ");
    assert_eq!(linked.runs.last().seen().typed, b"hello");
}

#[tokio::test]
async fn a_resize_reaches_the_program_and_the_next_view_is_drawn_for_it() {
    let linked = linked().await;
    linked.hold().await;
    let program = linked.client.start_program(a_harness()).await.unwrap();

    linked.client.resize(&program, 120, 40).await.unwrap();

    assert_eq!(
        linked.runs.last().seen().sizes,
        [Size {
            columns: 120,
            rows: 40
        }]
    );
    let attached = linked
        .client
        .attach_view(&program, ViewKind::Terminal)
        .await
        .unwrap();
    assert_eq!((attached.cols, attached.rows), (120, 40));
}

#[tokio::test]
async fn readiness_is_the_terminals_and_the_clocks() {
    let linked = linked().await;
    linked.hold().await;
    let program = linked.client.start_program(a_harness()).await.unwrap();

    let readiness = linked.client.readiness(&program).await.unwrap();

    assert_eq!(
        readiness,
        Readiness {
            edits_lines: Some(false),
            quiet_for_ms: 1500
        }
    );
}

#[tokio::test]
async fn a_stopped_program_is_kept_with_its_last_screen_and_a_subscriber_is_told_how_it_ended() {
    let mut linked = linked().await;
    linked.hold().await;
    linked.client.subscribe(Default::default()).await.unwrap();
    let program = linked
        .client
        .start_program(StartProgram {
            run: Some("01J9ZQ7000000000000000000C".into()),
            ..a_harness()
        })
        .await
        .unwrap();

    linked.client.stop(&program).await.unwrap();

    assert!(linked.runs.last().seen().stopped);
    let told = timeout(PATIENCE, linked.events.recv())
        .await
        .expect("the subscriber is told")
        .unwrap();
    assert_eq!(
        told,
        Told::Pushed(Pushed::Ended {
            program: program.clone(),
            run: Some("01J9ZQ7000000000000000000C".into()),
            exit: Exit::Signal("Interrupt".into()),
        })
    );
    let listed = linked.client.list().await.unwrap();
    assert_eq!(listed[0].state, "ended", "stopped, and still there");
}

#[tokio::test]
async fn a_closed_program_is_ended_and_forgotten() {
    let linked = linked().await;
    linked.hold().await;
    let program = linked.client.start_program(a_harness()).await.unwrap();

    linked.client.close(&program).await.unwrap();

    let program_ran = linked.runs.last();
    let deadline = std::time::Instant::now() + PATIENCE;
    while !program_ran.seen().stopped {
        assert!(
            std::time::Instant::now() < deadline,
            "the host never ended it"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert!(linked.client.list().await.unwrap().is_empty());
    let refused = linked.client.write(&program, b"x").await.unwrap_err();
    assert_eq!(
        refused.refusal().map(|r| r.code.as_str()),
        Some(session::NO_SUCH_CHAT)
    );
}

#[tokio::test]
async fn every_hosted_program_is_inside_a_chat_to_the_admission_check() {
    let linked = linked().await;
    linked.hold().await;
    linked.client.start_program(a_harness()).await.unwrap();

    assert_eq!(linked.hosting.programs(), [4242]);
    assert_eq!(
        linked.hosting.holder(),
        Some(std::process::id()),
        "with no window, everything below the host is inside a chat"
    );
    let windowed = Hosting::running_with(Echo, StandIns::default()).started_by(Some(Window(77)));
    assert_eq!(windowed.holder(), Some(77));
}

#[tokio::test]
async fn a_closed_chats_number_is_free_once_the_close_is_answered() {
    let linked = linked().await;
    linked.hold().await;
    let first = linked
        .client
        .start_program_told(StartProgram {
            number: Some(5),
            ..a_harness()
        })
        .await
        .unwrap();

    linked.client.close(&first.program).await.unwrap();
    let again = linked
        .client
        .start_program_told(StartProgram {
            number: Some(5),
            ..a_harness()
        })
        .await
        .unwrap();

    assert_eq!(again.number, Some(5));
}

#[tokio::test]
async fn a_closed_programs_process_stays_inside_a_chat_to_the_admission_check() {
    let linked = linked().await;
    linked.hold().await;
    let program = linked.client.start_program(a_harness()).await.unwrap();

    linked.client.close(&program).await.unwrap();

    assert!(linked.client.list().await.unwrap().is_empty());
    assert_eq!(
        linked.hosting.programs(),
        [4242],
        "what it started may outlive it in its session"
    );
}

#[tokio::test]
async fn a_link_lets_go_only_of_its_own_hold_and_its_holds_end_with_it() {
    let linked = linked().await;
    linked.hold().await;
    let (other, _views, _events) = another_window(&linked.hosting).await;

    let refused = other.let_go_of_project(PROJECT).await.unwrap_err();
    assert_eq!(
        refused.refusal().map(|r| r.code.as_str()),
        Some(session::NO_SUCH_PROJECT)
    );
    linked
        .client
        .start_program(a_harness())
        .await
        .expect("still held");

    let Linked {
        client, hosting, ..
    } = linked;
    drop(client);
    let deadline = std::time::Instant::now() + PATIENCE;
    loop {
        match other.start_program(a_harness()).await {
            Err(refused)
                if refused.refusal().map(|r| r.code.as_str()) == Some(session::NO_SUCH_PROJECT) =>
            {
                break;
            }
            _ => assert!(
                std::time::Instant::now() < deadline,
                "the hold outlived its link"
            ),
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    drop(hosting);
}

#[tokio::test]
async fn a_held_project_is_not_moved_by_another_hold() {
    let linked = linked().await;
    linked.hold().await;
    let elsewhere = tempfile::tempdir().unwrap();

    let refused = linked
        .client
        .hold_project(PROJECT, elsewhere.path().to_str().unwrap())
        .await
        .unwrap_err();

    assert_eq!(
        refused.refusal().map(|r| r.code.as_str()),
        Some(session::NOT_ALLOWED)
    );
    let listed = {
        linked.client.start_program(a_harness()).await.unwrap();
        linked.client.list().await.unwrap()
    };
    assert_eq!(
        listed[0].project_path.as_deref(),
        linked.project.path().to_str()
    );
}

#[tokio::test]
async fn ending_the_host_ends_every_program_it_runs() {
    let linked = linked().await;
    linked.hold().await;
    for _ in 0..3 {
        linked.client.start_program(a_harness()).await.unwrap();
    }

    let hosting = linked.hosting.clone();
    tokio::task::spawn_blocking(move || hosting.end_all())
        .await
        .unwrap();

    assert!(linked.runs.ran().iter().all(|(_, run)| run.seen().stopped));
    assert!(linked.client.list().await.unwrap().is_empty());
}

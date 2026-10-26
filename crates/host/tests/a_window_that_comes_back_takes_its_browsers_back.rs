// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A window that comes back to a session host that outlived it takes its browsers back (#2025),
//! spoken to over the session protocol on in-memory links: `programs_of` answers each browser a
//! held project still has beside its chats, with who drives it and the note the window kept of
//! it in the host (1.42), so the window puts each back with its chat; and a view the window
//! attaches again draws from the next whole frame. The browsers are stand-ins; the window's side
//! is the app's tests.

#![cfg(unix)]

use std::time::Duration;

use purlis_host::standin::{StandInBrowsers, StandInScreen, StandIns};
use purlis_host::{ComposesNothing, Hosting};
use purlis_session_protocol::auth::{Credentials, Scope};
use purlis_session_protocol::browsing::Driver;
use purlis_session_protocol::link::{self, Acceptor};
use purlis_session_protocol::session::{
    self, Client, HeldProgram, ProgramKind, StartProgram, ViewRole,
};
use purlis_session_protocol::view::frames::Frames;
use purlis_session_protocol::view::{Chunk, Viewer};
use serde_json::json;
use tokio::io::duplex;
use tokio::time::timeout;

const PROJECT: &str = "01J9PRJCT00000000000000000";
const PATIENCE: Duration = Duration::from_secs(5);

static HELD: std::sync::LazyLock<Credentials> =
    std::sync::LazyLock::new(|| Credentials::mint().unwrap());

/// A window's link to `hosting`, holding the project.
async fn window(hosting: &Hosting, project: &std::path::Path) -> (Client, Acceptor) {
    let (a, b) = duplex(4 << 20);
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
    let (client, views, _events) = Client::new(client.unwrap());
    client
        .hold_project(PROJECT, project.to_str().unwrap())
        .await
        .expect("the project is held");
    (client, views)
}

struct Hosted {
    hosting: Hosting,
    browsers: StandInBrowsers,
    _store: tempfile::TempDir,
    project: tempfile::TempDir,
}

fn hosted() -> Hosted {
    let store = tempfile::tempdir().unwrap();
    let browsers = StandInBrowsers::in_folder(store.path().join("browsers"));
    let hosting =
        Hosting::running_with(ComposesNothing, StandIns::default()).with_browsers(browsers.clone());
    Hosted {
        hosting,
        browsers,
        _store: store,
        project: tempfile::tempdir().unwrap(),
    }
}

async fn a_browser_noted(
    client: &Client,
    browsers: &StandInBrowsers,
    note: serde_json::Value,
) -> (String, StandInScreen) {
    let program = client
        .start_program(StartProgram {
            note: Some(note),
            ..StartProgram::of(PROJECT, ProgramKind::Browser)
        })
        .await
        .expect("a browser starts");
    (program, browsers.started().last().unwrap().clone())
}

/// The browsers `programs_of` answers.
async fn browsers_of(client: &Client) -> Vec<HeldProgram> {
    client
        .programs_of(PROJECT)
        .await
        .expect("the programs are answered")
        .into_iter()
        .filter(|program| program.kind == ProgramKind::Browser)
        .collect()
}

#[tokio::test]
async fn a_browser_is_answered_to_a_window_that_comes_back_with_its_note_and_its_driver() {
    let hosted = hosted();
    let (first, _views) = window(&hosted.hosting, hosted.project.path()).await;
    let (program, _screen) =
        a_browser_noted(&first, &hosted.browsers, json!({"chat": 3, "name": "b1"})).await;
    first.take_over(&program).await.unwrap();

    // The window crashes: its link ends without a quit.
    drop(first);
    let (second, _views) = window(&hosted.hosting, hosted.project.path()).await;
    let browsers = browsers_of(&second).await;

    assert_eq!(browsers.len(), 1, "{browsers:?}");
    let browser = &browsers[0];
    assert_eq!(browser.program, program);
    assert_eq!(browser.note, Some(json!({"chat": 3, "name": "b1"})));
    assert_eq!(
        browser.driver,
        Some(Driver::Person),
        "the lease is as it was"
    );
    assert!(browser.process.is_some(), "it still runs");
    assert_eq!(browser.exit, None);
}

#[tokio::test]
async fn the_note_a_window_keeps_of_a_browser_is_the_last_it_sent() {
    let hosted = hosted();
    let (first, _views) = window(&hosted.hosting, hosted.project.path()).await;
    let (program, _screen) =
        a_browser_noted(&first, &hosted.browsers, json!({"chat": 3, "name": "b1"})).await;

    first
        .note_program(&program, json!({"chat": 3, "name": "b1", "pinned": true}))
        .await
        .expect("the note is kept");
    drop(first);
    let (second, _views) = window(&hosted.hosting, hosted.project.path()).await;

    let browsers = browsers_of(&second).await;
    assert_eq!(
        browsers[0].note,
        Some(json!({"chat": 3, "name": "b1", "pinned": true}))
    );
    assert_eq!(browsers[0].driver, Some(Driver::Agent));
}

#[tokio::test]
async fn a_note_past_its_bound_is_refused_and_the_one_before_is_kept() {
    let hosted = hosted();
    let (client, _views) = window(&hosted.hosting, hosted.project.path()).await;
    let (program, _screen) = a_browser_noted(&client, &hosted.browsers, json!({"chat": 1})).await;

    let refused = client
        .note_program(&program, json!({"padding": "x".repeat(session::NOTE_MOST)}))
        .await
        .unwrap_err();

    assert_eq!(refused.refusal().unwrap().code, session::TOO_LARGE);
    assert_eq!(browsers_of(&client).await[0].note, Some(json!({"chat": 1})));
}

#[tokio::test]
async fn a_note_for_no_program_the_host_holds_is_refused() {
    let hosted = hosted();
    let (client, _views) = window(&hosted.hosting, hosted.project.path()).await;

    let refused = client
        .note_program("01J9NOSUCHPROGRAM000000000", json!({}))
        .await
        .unwrap_err();

    assert_eq!(refused.refusal().unwrap().code, session::NO_SUCH_CHAT);
}

#[tokio::test]
async fn a_view_attached_again_after_the_window_came_back_draws_from_the_next_whole_frame() {
    let hosted = hosted();
    let (first, mut first_views) = window(&hosted.hosting, hosted.project.path()).await;
    let (program, screen) =
        a_browser_noted(&first, &hosted.browsers, json!({"chat": 3, "name": "b1"})).await;
    first
        .attach_browser_view(&program, ViewRole::Focused, None)
        .await
        .unwrap();
    let _stream = timeout(PATIENCE, first_views.accept())
        .await
        .unwrap()
        .unwrap();
    screen.makes(b"\xff\xd8 before the crash");

    drop(first);
    let (second, mut views) = window(&hosted.hosting, hosted.project.path()).await;
    second
        .attach_browser_view(&program, ViewRole::Focused, None)
        .await
        .expect("a view is attached again");
    let stream = timeout(PATIENCE, views.accept()).await.unwrap().unwrap();
    let mut reader = Viewer::default().accept(stream).await.unwrap();
    screen.makes(b"\xff\xd8 after it");

    let mut frames = Frames::default();
    let mut images = Vec::new();
    while !images
        .iter()
        .any(|image: &Vec<u8>| image == b"\xff\xd8 after it")
    {
        let chunk = timeout(PATIENCE, reader.next())
            .await
            .expect("a frame comes")
            .unwrap()
            .unwrap();
        let bytes = match chunk {
            Chunk::Snapshot(bytes) | Chunk::Live(bytes) => bytes,
        };
        reader.ack(bytes.len()).await.unwrap();
        for frame in frames.push(&bytes).expect("each frame is whole") {
            // An image; the page's origin rides the stream too (1.35).
            if let Some((_, image)) = frame.aimed() {
                images.push(image.to_vec());
            }
        }
    }
    assert!(
        images
            .iter()
            .all(|image| image == b"\xff\xd8 before the crash" || image == b"\xff\xd8 after it"),
        "only whole frames: {images:?}"
    );
}

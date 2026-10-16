// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The person takes a browser over and hands it back (#2014), over the session protocol on an
//! in-memory link: while they drive, every act of the agent's is refused in words and only
//! their own input reaches the browser; the lease is the host's, so it stays with them when the
//! window's link ends and comes back only by their hand-back; and a browser view's frames name
//! what they show, so input aimed at a page that has moved on is dropped. The browser is a
//! stand-in; the engine's own part (the call in flight cut, references void, the field blanked
//! before a key) is `purlis-browser-engine`'s tests.

#![cfg(unix)]

use std::time::{Duration, Instant};

use purlis_host::standin::{StandInBrowsers, StandInScreen, StandIns};
use purlis_host::{ComposesNothing, Hosting};
use purlis_session_protocol::auth::{Credentials, Scope};
use purlis_session_protocol::browsing::{Act, Aim, Driver, Input, KeyAction};
use purlis_session_protocol::link::{self, Acceptor};
use purlis_session_protocol::session::{self, Client, ProgramKind, StartProgram, ViewRole};
use purlis_session_protocol::view::frames::{AIMED_JPEG, Frames};
use purlis_session_protocol::view::{Chunk, Viewer};
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

async fn a_browser(client: &Client, browsers: &StandInBrowsers) -> (String, StandInScreen) {
    let program = client
        .start_program(StartProgram::of(PROJECT, ProgramKind::Browser))
        .await
        .expect("a browser starts");
    (program, browsers.started().last().unwrap().clone())
}

fn aim(document: u64) -> Aim {
    Aim {
        page: "p1".into(),
        document,
    }
}

fn a_key(document: u64) -> Input {
    Input::Key {
        aim: aim(document),
        action: KeyAction::Down,
        key: "a".into(),
        physical: "KeyA".into(),
        key_code: 65,
        modifiers: 0,
    }
}

#[tokio::test]
async fn while_the_person_drives_the_agents_acts_are_refused_in_words_until_they_hand_back() {
    let hosted = hosted();
    let (client, _views) = window(&hosted.hosting, hosted.project.path()).await;
    let (program, screen) = a_browser(&client, &hosted.browsers).await;

    assert_eq!(client.take_over(&program).await.unwrap(), Driver::Person);
    let refused = client
        .browse(&program, "c-1", Act::Pages)
        .await
        .unwrap_err();

    let refusal = refused.refusal().expect("a refusal");
    assert_eq!(refusal.code, session::BROWSER_REFUSED);
    assert!(
        refusal
            .why
            .contains("the person has taken this browser over"),
        "{}",
        refusal.why
    );
    assert!(screen.shown().acts.is_empty(), "no act reached the browser");
    assert!(screen.shown().taken_over);

    assert_eq!(client.hand_back(&program).await.unwrap(), Driver::Agent);
    client
        .browse(&program, "c-2", Act::Pages)
        .await
        .expect("the agent's act reaches the browser once it is handed back");
    let shown = screen.shown();
    assert_eq!(shown.acts.len(), 1);
    assert_eq!(
        shown.epochs,
        [shown.epoch],
        "the act is made under the lease the hand-back started"
    );
}

#[tokio::test]
async fn the_lease_stays_with_the_person_when_the_windows_link_ends() {
    let hosted = hosted();
    let (first, _views) = window(&hosted.hosting, hosted.project.path()).await;
    let (program, screen) = a_browser(&first, &hosted.browsers).await;
    first.take_over(&program).await.unwrap();

    // The window crashes: its link ends.
    drop(first);
    let (second, _views) = window(&hosted.hosting, hosted.project.path()).await;
    let start = Instant::now();
    let listed = loop {
        let listed = second.list().await.unwrap();
        let browser = listed.into_iter().find(|p| p.chat == program).unwrap();
        if browser.driver == Some(Driver::Person) || start.elapsed() > PATIENCE {
            break browser;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    };

    assert_eq!(listed.driver, Some(Driver::Person));
    let refused = second.browse(&program, "c-1", Act::Pages).await;
    assert!(refused.is_err(), "the agent is still refused");
    assert!(screen.shown().taken_over, "nothing handed it back");
    assert_eq!(second.hand_back(&program).await.unwrap(), Driver::Agent);
    assert!(!screen.shown().taken_over);
}

#[tokio::test]
async fn the_persons_input_reaches_the_browser_only_while_they_drive() {
    let hosted = hosted();
    let (client, _views) = window(&hosted.hosting, hosted.project.path()).await;
    let (program, screen) = a_browser(&client, &hosted.browsers).await;
    screen.shown().document = 1;

    let early = client.browser_input(&program, a_key(1)).await.unwrap_err();
    assert_eq!(early.refusal().unwrap().code, session::NOT_DRIVING);
    assert!(screen.shown().inputs.is_empty());

    client.take_over(&program).await.unwrap();
    client
        .browser_input(&program, a_key(1))
        .await
        .expect("the key reaches the browser");
    assert_eq!(screen.shown().inputs, [a_key(1)]);
}

#[tokio::test]
async fn input_aimed_at_a_page_that_has_moved_on_is_refused_as_missed() {
    let hosted = hosted();
    let (client, _views) = window(&hosted.hosting, hosted.project.path()).await;
    let (program, screen) = a_browser(&client, &hosted.browsers).await;
    client.take_over(&program).await.unwrap();
    screen.shown().document = 2;

    let missed = client.browser_input(&program, a_key(1)).await.unwrap_err();

    assert_eq!(missed.refusal().unwrap().code, session::INPUT_MISSED);
    assert!(screen.shown().inputs.is_empty());
}

#[tokio::test]
async fn the_person_sets_a_browsers_size_while_they_drive() {
    let hosted = hosted();
    let (client, _views) = window(&hosted.hosting, hosted.project.path()).await;
    let (program, screen) = a_browser(&client, &hosted.browsers).await;

    let early = client.size_browser(&program, 390, 844).await.unwrap_err();
    assert_eq!(early.refusal().unwrap().code, session::NOT_DRIVING);

    client.take_over(&program).await.unwrap();
    client.size_browser(&program, 390, 844).await.unwrap();

    assert_eq!(screen.shown().size, (390, 844));
}

#[tokio::test]
async fn a_browser_views_frames_name_the_page_and_document_they_show() {
    let hosted = hosted();
    let (client, mut views) = window(&hosted.hosting, hosted.project.path()).await;
    let (program, screen) = a_browser(&client, &hosted.browsers).await;
    client
        .attach_browser_view(&program, ViewRole::Focused, None)
        .await
        .unwrap();
    let stream = timeout(PATIENCE, views.accept()).await.unwrap().unwrap();
    let mut reader = Viewer::default().accept(stream).await.unwrap();
    screen.shown().document = 4;

    screen.makes(b"\xff\xd8 a page");

    let mut frames = Frames::default();
    let frame = loop {
        let chunk = timeout(PATIENCE, reader.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let bytes = match chunk {
            Chunk::Snapshot(bytes) | Chunk::Live(bytes) => bytes,
        };
        reader.ack(bytes.len()).await.unwrap();
        if let Some(frame) = frames.push(&bytes).unwrap().pop() {
            break frame;
        }
    };
    assert_eq!(frame.format, AIMED_JPEG);
    let (shown, image) = frame.aimed().unwrap();
    assert_eq!(shown, aim(4));
    assert_eq!(&image[..], b"\xff\xd8 a page");
}

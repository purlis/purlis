// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The person inspects a browser they drive (#2028), over the session protocol on an in-memory
//! link: DevTools open only while they drive, are watched as frames of their own, take the
//! person's input apart from the page's, and close as the browser is handed back. The browser
//! is a stand-in; the engine's own part (the front end, the tapped session, what it may reach)
//! is `purlis-browser-engine`'s tests.

#![cfg(unix)]

use std::time::Duration;

use purlis_host::standin::{StandInBrowsers, StandInScreen, StandIns};
use purlis_host::{ComposesNothing, Hosting};
use purlis_session_protocol::auth::{Credentials, Scope};
use purlis_session_protocol::browsing::{Aim, Input, KeyAction};
use purlis_session_protocol::link::{self, Acceptor};
use purlis_session_protocol::session::{self, Client, ProgramKind, StartProgram};
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

fn devtools_key() -> Input {
    Input::Key {
        aim: Aim {
            page: "devtools".into(),
            document: 1,
        },
        action: KeyAction::Down,
        key: "a".into(),
        physical: "KeyA".into(),
        key_code: 65,
        modifiers: 0,
    }
}

/// The next whole frame on `reader`, or `None` once its view ends.
async fn next_frame(
    reader: &mut purlis_session_protocol::view::Reader,
    frames: &mut Frames,
) -> Option<purlis_session_protocol::view::frames::Frame> {
    loop {
        let chunk = timeout(PATIENCE, reader.next()).await.unwrap()?.ok()?;
        let bytes = match chunk {
            Chunk::Snapshot(bytes) | Chunk::Live(bytes) => bytes,
        };
        reader.ack(bytes.len()).await.unwrap();
        if let Some(frame) = frames.push(&bytes).unwrap().pop() {
            return Some(frame);
        }
    }
}

#[tokio::test]
async fn devtools_open_only_for_the_person_driving_the_browser_at_the_size_asked() {
    let hosted = hosted();
    let (client, _views) = window(&hosted.hosting, hosted.project.path()).await;
    let (program, screen) = a_browser(&client, &hosted.browsers).await;

    let early = client.inspect(&program, 1000, 600).await.unwrap_err();
    assert_eq!(early.refusal().unwrap().code, session::NOT_DRIVING);
    assert_eq!(screen.shown().inspecting, None);

    client.take_over(&program).await.unwrap();
    client.inspect(&program, 1000, 600).await.unwrap();
    assert_eq!(screen.shown().inspecting, Some((1000, 600)));
    // Asked again while open, they are only sized.
    client.inspect(&program, 900, 500).await.unwrap();
    assert_eq!(screen.shown().inspecting, Some((900, 500)));
}

#[tokio::test]
async fn a_devtools_view_is_their_frames_and_ends_as_the_person_hands_the_browser_back() {
    let hosted = hosted();
    let (client, mut views) = window(&hosted.hosting, hosted.project.path()).await;
    let (program, screen) = a_browser(&client, &hosted.browsers).await;
    client.take_over(&program).await.unwrap();
    let closed = client.attach_inspector_view(&program).await.unwrap_err();
    assert!(closed.refusal().is_some(), "no DevTools are open to watch");

    client.inspect(&program, 1000, 600).await.unwrap();
    let attached = client.attach_inspector_view(&program).await.unwrap();
    assert_eq!(
        (attached.cols, attached.rows),
        (1000, 600),
        "drawn at DevTools' size"
    );
    let stream = timeout(PATIENCE, views.accept()).await.unwrap().unwrap();
    let mut reader = Viewer::default().accept(stream).await.unwrap();
    assert!(screen.shown().showing_devtools);
    screen.makes(b"\xff\xd8 the page");
    screen.devtools_make(b"\xff\xd8 devtools");
    let mut frames = Frames::default();
    let frame = next_frame(&mut reader, &mut frames).await.expect("a frame");
    assert_eq!(frame.format, AIMED_JPEG);
    let (shown, image) = frame.aimed().unwrap();
    assert_eq!(shown.page, "devtools");
    assert_eq!(&image[..], b"\xff\xd8 devtools");

    client.hand_back(&program).await.unwrap();
    assert!(
        next_frame(&mut reader, &mut frames).await.is_none(),
        "the view of DevTools ends with them"
    );
    assert!(!screen.shown().showing_devtools);
    assert_eq!(screen.shown().inspecting, None);
}

#[tokio::test]
async fn the_persons_input_reaches_devtools_only_while_they_drive_with_them_open() {
    let hosted = hosted();
    let (client, _views) = window(&hosted.hosting, hosted.project.path()).await;
    let (program, screen) = a_browser(&client, &hosted.browsers).await;

    let early = client
        .inspector_input(&program, devtools_key())
        .await
        .unwrap_err();
    assert_eq!(early.refusal().unwrap().code, session::NOT_DRIVING);
    client.take_over(&program).await.unwrap();
    let unopened = client
        .inspector_input(&program, devtools_key())
        .await
        .unwrap_err();
    assert_eq!(unopened.refusal().unwrap().code, session::INPUT_MISSED);

    client.inspect(&program, 1000, 600).await.unwrap();
    client
        .inspector_input(&program, devtools_key())
        .await
        .unwrap();
    assert_eq!(screen.shown().devtools_inputs, [devtools_key()]);
    assert!(
        screen.shown().inputs.is_empty(),
        "nothing of it reached the page"
    );
}

#[tokio::test]
async fn devtools_that_close_by_themselves_end_their_view_and_open_again_afresh() {
    let hosted = hosted();
    let (client, mut views) = window(&hosted.hosting, hosted.project.path()).await;
    let (program, screen) = a_browser(&client, &hosted.browsers).await;
    client.take_over(&program).await.unwrap();
    client.inspect(&program, 1000, 600).await.unwrap();
    client.attach_inspector_view(&program).await.unwrap();
    let stream = timeout(PATIENCE, views.accept()).await.unwrap().unwrap();
    let mut reader = Viewer::default().accept(stream).await.unwrap();

    screen.devtools_close();
    let mut frames = Frames::default();
    assert!(next_frame(&mut reader, &mut frames).await.is_none());

    client.inspect(&program, 1000, 600).await.unwrap();
    client
        .attach_inspector_view(&program)
        .await
        .expect("DevTools opened again are watched again");
}

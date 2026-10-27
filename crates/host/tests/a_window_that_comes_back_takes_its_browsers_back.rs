// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A window that comes back to a session host that outlived it takes its browsers back (#2025),
//! spoken to over the session protocol on in-memory links: `programs_of` answers each browser a
//! held project still has beside its chats, with who drives it and the note the window kept of
//! it in the host (1.39), so the window puts each back with its chat; and a view the window
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
    self, Client, HeldProgram, ProgramKind, ProgramNote, StartProgram, ViewRole,
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
            note: Some(ProgramNote(note)),
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
    assert_eq!(
        browser.note,
        Some(ProgramNote(json!({"chat": 3, "name": "b1"})))
    );
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
        .note_program(
            &program,
            ProgramNote(json!({"chat": 3, "name": "b1", "pinned": true})),
        )
        .await
        .expect("the note is kept");
    drop(first);
    let (second, _views) = window(&hosted.hosting, hosted.project.path()).await;

    let browsers = browsers_of(&second).await;
    assert_eq!(
        browsers[0].note,
        Some(ProgramNote(
            json!({"chat": 3, "name": "b1", "pinned": true})
        ))
    );
    assert_eq!(browsers[0].driver, Some(Driver::Agent));
}

#[tokio::test]
async fn a_note_past_its_bound_is_refused_and_the_one_before_is_kept() {
    let hosted = hosted();
    let (client, _views) = window(&hosted.hosting, hosted.project.path()).await;
    let (program, _screen) = a_browser_noted(&client, &hosted.browsers, json!({"chat": 1})).await;

    let refused = client
        .note_program(
            &program,
            ProgramNote(json!({"padding": "x".repeat(ProgramNote::MOST)})),
        )
        .await
        .unwrap_err();

    assert_eq!(refused.refusal().unwrap().code, session::TOO_LARGE);
    assert_eq!(
        browsers_of(&client).await[0].note,
        Some(ProgramNote(json!({"chat": 1})))
    );
}

#[tokio::test]
async fn a_note_for_no_program_the_host_holds_is_refused() {
    let hosted = hosted();
    let (client, _views) = window(&hosted.hosting, hosted.project.path()).await;

    let refused = client
        .note_program("01J9NOSUCHPROGRAM000000000", ProgramNote(json!({})))
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

const OTHER: &str = "01J9PRJCT00000000000000001";

/// A window's link to `hosting` holding project `project` at `path`.
async fn window_holding(hosting: &Hosting, project: &str, path: &std::path::Path) -> Client {
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
    let (client, _views, _events) = Client::new(client.unwrap());
    client
        .hold_project(project, path.to_str().unwrap())
        .await
        .expect("the project is held");
    client
}

#[tokio::test]
async fn a_note_of_a_browser_in_a_project_the_link_does_not_hold_is_refused() {
    let hosted = hosted();
    let (holder, _views) = window(&hosted.hosting, hosted.project.path()).await;
    let (program, _screen) = a_browser_noted(&holder, &hosted.browsers, json!({"chat": 1})).await;
    let elsewhere = tempfile::tempdir().unwrap();
    let other = window_holding(&hosted.hosting, OTHER, elsewhere.path()).await;

    let refused = other
        .note_program(&program, ProgramNote(json!({"chat": 9})))
        .await
        .unwrap_err();

    assert_eq!(refused.refusal().unwrap().code, session::NO_SUCH_CHAT);
    assert_eq!(
        browsers_of(&holder).await[0].note,
        Some(ProgramNote(json!({"chat": 1})))
    );
}

/// A login of `identity`, or of none named.
fn a_login(identity: Option<&str>) -> purlis_session_protocol::browsing::LogIn {
    purlis_session_protocol::browsing::LogIn {
        address: "https://shop.example.test/login".into(),
        user: None,
        password: purlis_session_protocol::browsing::Secret::new("not-a-real-one".into()),
        keep: None,
        identity: identity.map(str::to_owned),
    }
}

#[tokio::test]
async fn each_login_purlis_performed_in_a_browser_is_answered_whatever_its_note_says() {
    let hosted = hosted();
    let (first, _views) = window(&hosted.hosting, hosted.project.path()).await;
    let (program, _screen) = a_browser_noted(&first, &hosted.browsers, json!({"chat": 3})).await;
    assert!(browsers_of(&first).await[0].logins.is_empty());

    let _ = first.log_in(&program, a_login(Some("login-one"))).await;
    let _ = first.log_in(&program, a_login(Some("login-two"))).await;
    let _ = first.log_in(&program, a_login(Some("login-one"))).await;
    let _ = first.log_in(&program, a_login(None)).await;
    drop(first);
    let (second, _views) = window(&hosted.hosting, hosted.project.path()).await;

    let browsers = browsers_of(&second).await;
    assert_eq!(
        browsers[0].logins,
        ["login-one", "login-two", ""],
        "the host's own record, each once, one with no identity as an empty one"
    );
    assert_eq!(browsers[0].note, Some(ProgramNote(json!({"chat": 3}))));
}

#[tokio::test]
async fn a_browser_opened_from_a_kept_login_is_answered_with_it() {
    let hosted = hosted();
    let (client, _views) = window(&hosted.hosting, hosted.project.path()).await;
    client
        .start_program(StartProgram {
            kept: Some(purlis_session_protocol::browsing::KeptLogin {
                profile: "claude".into(),
                persona: "steward".into(),
                login: "shop-login".into(),
                identity: "kept-identity".into(),
                address: "https://shop.example.test/login".into(),
            }),
            ..StartProgram::of(PROJECT, ProgramKind::Browser)
        })
        .await
        .expect("a browser starts from its kept login");

    assert_eq!(browsers_of(&client).await[0].logins, ["kept-identity"]);
}

#[tokio::test]
async fn the_lease_comes_back_under_the_epoch_it_had() {
    let hosted = hosted();
    let (first, _views) = window(&hosted.hosting, hosted.project.path()).await;
    let (program, screen) = a_browser_noted(&first, &hosted.browsers, json!({"chat": 3})).await;
    first.take_over(&program).await.unwrap();
    first.hand_back(&program).await.unwrap();
    let epoch = screen.shown().epoch;

    drop(first);
    let (second, _views) = window(&hosted.hosting, hosted.project.path()).await;
    second
        .browse(
            &program,
            "c-after",
            purlis_session_protocol::browsing::Act::Pages,
        )
        .await
        .expect("the agent acts on it");

    let (now, last) = {
        let shown = screen.shown();
        (shown.epoch, shown.epochs.last().copied())
    };
    assert_eq!(now, epoch, "the engine's lease is as it was");
    assert_eq!(last, Some(epoch), "the act ran under it");
    assert_eq!(browsers_of(&second).await[0].driver, Some(Driver::Agent));
}

#[tokio::test]
async fn a_view_attached_again_draws_a_page_that_has_not_moved_since_from_its_snapshot() {
    let hosted = hosted();
    let (first, mut first_views) = window(&hosted.hosting, hosted.project.path()).await;
    let (program, screen) = a_browser_noted(&first, &hosted.browsers, json!({"chat": 3})).await;
    first
        .attach_browser_view(&program, ViewRole::Focused, None)
        .await
        .unwrap();
    let _stream = timeout(PATIENCE, first_views.accept())
        .await
        .unwrap()
        .unwrap();
    screen.makes(b"\xff\xd8 a still page");

    drop(first);
    let (second, mut views) = window(&hosted.hosting, hosted.project.path()).await;
    second
        .attach_browser_view(&program, ViewRole::Focused, None)
        .await
        .expect("a view is attached again");
    let stream = timeout(PATIENCE, views.accept()).await.unwrap().unwrap();
    let mut reader = Viewer::default().accept(stream).await.unwrap();

    let mut frames = Frames::default();
    let image = loop {
        let chunk = timeout(PATIENCE, reader.next())
            .await
            .expect("the snapshot comes with no new frame")
            .unwrap()
            .unwrap();
        let bytes = match chunk {
            Chunk::Snapshot(bytes) | Chunk::Live(bytes) => bytes,
        };
        reader.ack(bytes.len()).await.unwrap();
        if let Some(image) = frames
            .push(&bytes)
            .unwrap()
            .iter()
            .find_map(|frame| frame.aimed().map(|(_, image)| image.to_vec()))
        {
            break image;
        }
    };
    assert_eq!(image, b"\xff\xd8 a still page");
}

#[tokio::test]
async fn a_browser_no_window_comes_back_for_within_the_grace_period_is_ended_with_its_project() {
    const GRACE: Duration = Duration::from_millis(400);
    let hosted = hosted();
    let lives = {
        let hosting = hosted.hosting.clone();
        tokio::spawn(async move {
            hosting
                .while_a_window_may_come_back(GRACE, std::future::pending())
                .await;
        })
    };
    let (first, _views) = window(&hosted.hosting, hosted.project.path()).await;
    let (_program, screen) = a_browser_noted(&first, &hosted.browsers, json!({"chat": 3})).await;
    drop(first);
    // A window comes back, and opens another project only.
    let elsewhere = tempfile::tempdir().unwrap();
    let _other = window_holding(&hosted.hosting, OTHER, elsewhere.path()).await;

    let deadline = std::time::Instant::now() + PATIENCE;
    while !screen.shown().stopped {
        assert!(
            std::time::Instant::now() < deadline,
            "the browser never ended"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let (late, _views) = window(&hosted.hosting, hosted.project.path()).await;
    assert!(browsers_of(&late).await.is_empty(), "nothing to take back");
    lives.abort();
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A runner runs its window's browsers as its desktop's host relays them (#2050, spec #2041):
//! the window starts a browser through host A, its own, and it runs on host B, the runner, over
//! the link between hosts; the window watches it, its chat's acts reach it, the person takes it
//! over and types into it, and purlis logs it in, all as on host A. Host A sends each of these
//! on for its window alone, and never a kept login, which stays on the device it was made on.
//!
//! **Over the link a view is held to the link's frames** (spec #2041's defaults): at most 15
//! frames a second, at most 1280 pixels wide, one frame in flight. A view on the runner's own
//! host is not held to them.
//!
//! The browsers are stand-ins, so none of this needs an engine.

#![cfg(unix)]

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use purlis_host::standin::{StandInBrowsers, StandInScreen, StandIns};
use purlis_host::{ComposesNothing, Dial, Hosting, Relay};
use purlis_session_protocol::auth::{Credentials, Scope};
use purlis_session_protocol::browsing::{
    Act, Aim, Driver, Input, KeptLogin, KeyAction, LogIn, Secret,
};
use purlis_session_protocol::hostlink::{self, Handshake, Identity, LinkKey, Liveness, Pinned};
use purlis_session_protocol::link::{self, Acceptor, Link};
use purlis_session_protocol::session::{
    self, Client, Command, ProgramKind, StartProgram, ViewKind, ViewRole,
};
use purlis_session_protocol::view::frames::Frames;
use purlis_session_protocol::view::{Chunk, Viewer};
use tokio::io::duplex;
use tokio::time::timeout;

const PROJECT: &str = "01J9PRJCT00000000000000000";
const PATIENCE: Duration = Duration::from_secs(5);

static HELD: std::sync::LazyLock<Credentials> =
    std::sync::LazyLock::new(|| Credentials::mint().unwrap());

async fn as_scope(scope: Scope, host: impl session::Host) -> Link {
    let (a, b) = duplex(4 << 20);
    let (client, served) = tokio::join!(
        link::connect(a, session::speaks(), scope, HELD.of(scope)),
        link::serve_any(b, session::speaks(), &HELD)
    );
    tokio::spawn(session::serve(served.unwrap(), host, None));
    client.unwrap()
}

fn identity(device: &str) -> Identity {
    Identity {
        device: device.into(),
        key: LinkKey::generate().unwrap(),
    }
}

/// A link between the hosts: host B serves it as `remote-link`.
async fn between_hosts(b: Hosting) -> Link {
    let (a_end, b_end) = duplex(4 << 20);
    let host_a = identity("01JA0DESKT0P00000000000000");
    let host_b = identity("01JB0RUNNER0000000000000000");
    let pinned = |identity: &Identity| Pinned {
        device: identity.device.clone(),
        key: identity.key.public(),
    };
    let peers = pinned(&host_a);
    let b_pinned = pinned(&host_b);
    tokio::spawn(async move {
        let answered = hostlink::answer(
            b_end,
            Handshake::THIS_BUILD,
            &host_b,
            &peers,
            Liveness::default(),
        )
        .await
        .unwrap();
        session::serve(answered.link, b, None).await
    });
    hostlink::connect(
        a_end,
        Handshake::THIS_BUILD,
        &host_a,
        &b_pinned,
        Liveness::default(),
    )
    .await
    .unwrap()
}

/// Dials host B over the link between hosts, counting each dial.
fn over_the_link(b: Hosting, dials: Arc<AtomicUsize>) -> Dial {
    Arc::new(move || {
        let b = b.clone();
        dials.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move { Ok(between_hosts(b).await) })
    })
}

/// The runner: host B, whose browsers are stand-ins.
struct Runner {
    hosting: Hosting,
    browsers: StandInBrowsers,
    _store: tempfile::TempDir,
}

fn a_runner() -> Runner {
    let store = tempfile::tempdir().unwrap();
    let browsers = StandInBrowsers::in_folder(store.path().join("browsers"));
    let hosting = Hosting::running_with(ComposesNothing, StandIns::default())
        .with_browsers(browsers.clone());
    Runner {
        hosting,
        browsers,
        _store: store,
    }
}

/// The window on host A, which relays to the runner, holding the project there.
struct Window {
    client: Client,
    views: Acceptor,
    viewer: Viewer,
    _project: tempfile::TempDir,
}

async fn a_window(runner: &Runner, dials: &Arc<AtomicUsize>) -> Window {
    let relay = Relay::new(over_the_link(runner.hosting.clone(), Arc::clone(dials)));
    let (client, views, _events) = Client::new(as_scope(Scope::LocalUi, relay).await);
    let project = tempfile::tempdir().unwrap();
    client
        .hold_project(PROJECT, project.path().to_str().unwrap())
        .await
        .expect("the runner holds the project");
    Window {
        client,
        views,
        viewer: Viewer::default(),
        _project: project,
    }
}

impl Window {
    async fn browser(&self, runner: &Runner) -> (String, StandInScreen) {
        let program = self
            .client
            .start_program(StartProgram::of(PROJECT, ProgramKind::Browser))
            .await
            .expect("a browser starts on the runner");
        (program, runner.browsers.started().last().unwrap().clone())
    }
}

fn a_login(keep: Option<KeptLogin>) -> LogIn {
    LogIn {
        address: "https://shop.example.test/login".into(),
        user: None,
        password: Secret::new("fixture-word".into()),
        keep,
    }
}

fn a_key(document: u64) -> Input {
    Input::Key {
        aim: Aim {
            page: "p1".into(),
            document,
        },
        action: KeyAction::Down,
        key: "a".into(),
        physical: "KeyA".into(),
        key_code: 65,
        modifiers: 0,
    }
}

#[tokio::test]
async fn the_windows_browser_starts_on_the_runner_and_its_chats_acts_reach_it_there() {
    let runner = a_runner();
    let dials = Arc::new(AtomicUsize::new(0));
    let window = a_window(&runner, &dials).await;

    let (program, screen) = window.browser(&runner).await;
    let browsed = window
        .client
        .browse(&program, "c-1", Act::Pages)
        .await
        .expect("the act reaches the runner's browser");

    assert!(browsed.settled);
    assert_eq!(runner.browsers.started().len(), 1, "it runs on the runner");
    assert_eq!(screen.shown().acts.len(), 1);
}

#[tokio::test]
async fn the_window_watches_the_runners_browser_through_its_host() {
    let runner = a_runner();
    let dials = Arc::new(AtomicUsize::new(0));
    let mut window = a_window(&runner, &dials).await;
    let (program, screen) = window.browser(&runner).await;

    let attached = window
        .client
        .attach_browser_view(&program, ViewRole::Focused, None)
        .await
        .expect("the view attaches through host A");
    assert_eq!(attached.kind, Some(ViewKind::Browser));
    let stream = timeout(PATIENCE, window.views.accept())
        .await
        .unwrap()
        .unwrap();
    let mut reader = window.viewer.accept(stream).await.unwrap();
    screen.makes(b"the runner's page");

    let mut frames = Frames::default();
    let image = timeout(PATIENCE, async {
        loop {
            let (Chunk::Snapshot(bytes) | Chunk::Live(bytes)) =
                reader.next().await.unwrap().unwrap();
            reader.ack(bytes.len()).await.unwrap();
            if let Some(image) = frames
                .push(&bytes)
                .unwrap()
                .into_iter()
                .find_map(|frame| frame.aimed().map(|(_, image)| image))
                .filter(|image| !image.is_empty())
            {
                return image;
            }
        }
    })
    .await
    .expect("its frame is drawn in the window");

    assert_eq!(&image[..], b"the runner's page");
}

#[tokio::test]
async fn the_person_takes_the_runners_browser_over_types_into_it_and_hands_it_back() {
    let runner = a_runner();
    let dials = Arc::new(AtomicUsize::new(0));
    let window = a_window(&runner, &dials).await;
    let (program, screen) = window.browser(&runner).await;

    let took = window.client.take_over(&program).await.unwrap();
    window
        .client
        .browser_input(&program, a_key(0))
        .await
        .expect("the key reaches the runner's page");
    let gave = window.client.hand_back(&program).await.unwrap();

    assert_eq!((took, gave), (Driver::Person, Driver::Agent));
    assert_eq!(screen.shown().inputs, [a_key(0)]);
}

#[tokio::test]
async fn purlis_logs_the_runners_browser_in_for_the_window_and_says_no_value() {
    let runner = a_runner();
    let dials = Arc::new(AtomicUsize::new(0));
    let window = a_window(&runner, &dials).await;
    let (program, screen) = window.browser(&runner).await;

    let browsed = window
        .client
        .log_in(&program, a_login(None))
        .await
        .expect("the login reaches the runner's browser");

    assert!(browsed.settled);
    assert_eq!(screen.shown().logins, ["https://shop.example.test/login"]);
}

#[tokio::test]
async fn a_login_that_would_keep_its_login_is_not_sent_to_the_runner() {
    let runner = a_runner();
    let dials = Arc::new(AtomicUsize::new(0));
    let window = a_window(&runner, &dials).await;
    let (program, screen) = window.browser(&runner).await;

    let refused = window
        .client
        .log_in(
            &program,
            a_login(Some(KeptLogin {
                profile: "claude".into(),
                persona: "steward".into(),
                login: "shop-login".into(),
                identity: "0123456789abcdef0123456789abcdef".into(),
                address: "https://shop.example.test/login".into(),
            })),
        )
        .await
        .unwrap_err();

    let refusal = refused.refusal().expect("refused in words");
    assert_eq!(refusal.code, session::NOT_ALLOWED);
    assert!(!refusal.why.contains("fixture-word"), "{}", refusal.why);
    assert!(screen.shown().logins.is_empty(), "nothing was filled");
}

/// A client of host A that is not the window: another device's link to it.
async fn another_host_on(relay: Relay) -> Client {
    let (a, b) = duplex(256 * 1024);
    let (client, served) = tokio::join!(
        link::connect_as_a_device(a, session::speaks()),
        link::serve_as_a_device(b, session::speaks(), link::ProvenDevice::stand_in())
    );
    tokio::spawn(session::serve(served.unwrap(), relay, None));
    Client::new(client.unwrap()).0
}

#[tokio::test]
async fn host_a_sends_a_browser_or_a_login_on_for_its_window_alone() {
    let runner = a_runner();
    let dials = Arc::new(AtomicUsize::new(0));
    let relay = Relay::new(over_the_link(runner.hosting.clone(), Arc::clone(&dials)));
    let another = another_host_on(relay).await;

    let started = another
        .start_program(StartProgram::of(PROJECT, ProgramKind::Browser))
        .await;
    let logged_in = another.log_in("01J9ZQ8000000000000000000D", a_login(None)).await;

    for refused in [started.unwrap_err(), logged_in.unwrap_err()] {
        let refusal = refused.refusal().expect("host A refuses it");
        assert_eq!(refusal.code, session::NOT_ALLOWED);
    }
    assert!(runner.browsers.started().is_empty(), "nothing started");
}

/// A view of `program` attached by `client`, whose stream is read and drawn until it is let go.
async fn watched(client: &Client, views: &mut Acceptor, program: &str, role: ViewRole) {
    client
        .attach_browser_view(program, role, None)
        .await
        .expect("the view attaches");
    let stream = timeout(PATIENCE, views.accept()).await.unwrap().unwrap();
    let mut reader = Viewer::default().accept(stream).await.unwrap();
    tokio::spawn(async move {
        while let Some(Ok(Chunk::Snapshot(bytes) | Chunk::Live(bytes))) = reader.next().await {
            if reader.ack(bytes.len()).await.is_err() {
                return;
            }
        }
    });
}

#[tokio::test]
async fn over_the_link_a_focused_view_is_held_to_fifteen_frames_a_second_1280_pixels_wide() {
    let runner = a_runner();
    let dials = Arc::new(AtomicUsize::new(0));
    let mut window = a_window(&runner, &dials).await;
    let (program, screen) = window.browser(&runner).await;

    watched(&window.client, &mut window.views, &program, ViewRole::Focused).await;

    let shown = screen.shown();
    assert_eq!(
        shown.every,
        Some(Duration::from_secs(1) / 15),
        "no more than 15 frames a second"
    );
    assert_eq!(shown.most_width, Some(1280), "no wider than 1280 pixels");
}

#[tokio::test]
async fn over_the_link_a_thumbnail_keeps_its_own_slower_pace() {
    let runner = a_runner();
    let dials = Arc::new(AtomicUsize::new(0));
    let mut window = a_window(&runner, &dials).await;
    let (program, screen) = window.browser(&runner).await;

    watched(
        &window.client,
        &mut window.views,
        &program,
        ViewRole::Thumbnail,
    )
    .await;

    assert_eq!(screen.shown().every, Some(purlis_host::THUMBNAIL_EVERY));
}

#[tokio::test]
async fn a_view_on_the_runners_own_host_is_not_held_to_the_links_frames() {
    let runner = a_runner();
    let (client, mut views, _events) =
        Client::new(as_scope(Scope::LocalUi, runner.hosting.clone()).await);
    let project = tempfile::tempdir().unwrap();
    client
        .hold_project(PROJECT, project.path().to_str().unwrap())
        .await
        .unwrap();
    let program = client
        .start_program(StartProgram::of(PROJECT, ProgramKind::Browser))
        .await
        .unwrap();
    let screen = runner.browsers.started().last().unwrap().clone();

    watched(&client, &mut views, &program, ViewRole::Focused).await;

    let shown = screen.shown();
    assert_eq!(shown.every, Some(Duration::ZERO), "every frame");
    assert_eq!(shown.most_width, None, "as wide as its pages");
}

#[tokio::test]
async fn a_window_command_the_runner_refuses_is_still_sent_on_once() {
    // A browse of a browser the runner does not have is the runner's to refuse, not host A's.
    let runner = a_runner();
    let dials = Arc::new(AtomicUsize::new(0));
    let window = a_window(&runner, &dials).await;

    let refused = window
        .client
        .call(Command::Browse {
            program: "01J9ZQ8000000000000000000D".into(),
            call_id: "c-1".into(),
            act: Act::Pages,
            reach: None,
        })
        .await
        .unwrap_err();

    let refusal = refused.refusal().expect("the runner refuses it");
    assert_ne!(refusal.code, session::NOT_ALLOWED, "{}", refusal.why);
    assert_eq!(dials.load(Ordering::SeqCst), 1);
}

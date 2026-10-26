// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A browser is a hosted program of the session host (#2012), spoken to over the session
//! protocol on an in-memory link: it starts in a held project with a data folder of its own,
//! makes frames only while a view of it is attached (every frame for the focused view, about
//! one a second for a thumbnail), gives a view that fell behind the next whole frame, and ends
//! with a stop, with the host, and so with Stop all. The browser is a stand-in, so none of this
//! needs an engine; the same over a real engine runs on CI (`purlis-browser-engine`'s tests).

#![cfg(unix)]

use std::time::{Duration, Instant};

use purlis_host::standin::{StandInBrowsers, StandInScreen, StandIns};
use purlis_host::{ComposesNothing, Hosting};
use purlis_session_protocol::auth::{Credentials, Scope};
use purlis_session_protocol::browsing::{Act, Browsed, BrowserReach};
use purlis_session_protocol::link::{self, Acceptor};
use purlis_session_protocol::session::{
    self, Client, Events, Exit, ProgramKind, Pushed, StartProgram, Told, ViewKind, ViewRole,
};
use purlis_session_protocol::view::frames::{AIMED_JPEG, Frame, Frames, ORIGIN};
use purlis_session_protocol::view::{Chunk, Reader, Viewer};
use tokio::io::duplex;
use tokio::time::timeout;

const PROJECT: &str = "01J9PRJCT00000000000000000";
const PATIENCE: Duration = Duration::from_secs(5);

static HELD: std::sync::LazyLock<Credentials> =
    std::sync::LazyLock::new(|| Credentials::mint().unwrap());

struct Linked {
    client: Client,
    views: Acceptor,
    viewer: Viewer,
    events: Events,
    browsers: StandInBrowsers,
    hosting: Hosting,
    store: tempfile::TempDir,
    _project: tempfile::TempDir,
}

async fn linked() -> Linked {
    linked_speaking(session::speaks()).await
}

/// [`linked`], the window speaking `speaks`.
async fn linked_speaking(speaks: purlis_session_protocol::version::Speaks) -> Linked {
    let store = tempfile::tempdir().unwrap();
    let browsers = StandInBrowsers::in_folder(store.path().join("browsers"));
    let hosting =
        Hosting::running_with(ComposesNothing, StandIns::default()).with_browsers(browsers.clone());
    let (a, b) = duplex(4 << 20);
    let (client, served) = tokio::join!(
        link::connect(a, speaks, Scope::LocalUi, HELD.of(Scope::LocalUi)),
        link::serve_any(b, session::speaks(), &HELD)
    );
    tokio::spawn(session::serve(served.unwrap(), hosting.clone(), None));
    let (client, views, events) = Client::new(client.unwrap());
    let project = tempfile::tempdir().unwrap();
    client
        .hold_project(PROJECT, project.path().to_str().unwrap())
        .await
        .expect("the project is held");
    Linked {
        client,
        views,
        viewer: Viewer::default(),
        events,
        browsers,
        hosting,
        store,
        _project: project,
    }
}

impl Linked {
    async fn browser(&self) -> (String, StandInScreen) {
        let program = self
            .client
            .start_program(StartProgram::of(PROJECT, ProgramKind::Browser))
            .await
            .expect("a browser starts");
        (program, self.browsers.started().last().unwrap().clone())
    }

    async fn watch(&mut self, program: &str, role: ViewRole) -> (u32, Reader) {
        let attached = self
            .client
            .attach_browser_view(program, role, None)
            .await
            .expect("the view attaches");
        assert_eq!(attached.kind, Some(ViewKind::Browser));
        let stream = timeout(PATIENCE, self.views.accept())
            .await
            .unwrap()
            .unwrap();
        let reader = self.viewer.accept(stream).await.unwrap();
        (attached.view, reader)
    }
}

/// Every frame a view's stream carries until `until`, its snapshot's first.
async fn every_frame_until(reader: &mut Reader, until: Instant) -> Vec<Frame> {
    let mut frames = Frames::default();
    let mut all = Vec::new();
    while let Ok(Some(Ok(chunk))) = tokio::time::timeout_at(until.into(), reader.next()).await {
        let bytes = match chunk {
            Chunk::Snapshot(bytes) | Chunk::Live(bytes) => bytes,
        };
        reader.ack(bytes.len()).await.unwrap();
        all.extend(frames.push(&bytes).unwrap());
    }
    all
}

/// The frames a view's stream carries until `until`, as their images, its snapshot's first:
/// the page's origin between them is left out (1.35).
async fn frames_until(reader: &mut Reader, until: Instant) -> Vec<Vec<u8>> {
    every_frame_until(reader, until)
        .await
        .into_iter()
        .filter(|frame| frame.format != ORIGIN)
        .map(|frame| {
            // A 1.14 link's view is sent frames that name what they show (#2014).
            assert_eq!(frame.format, AIMED_JPEG);
            frame.aimed().expect("an aimed frame").1.to_vec()
        })
        .collect()
}

/// What a view's frames said, in order: `origin <it>` for an origin frame, the image's text
/// for an image.
fn said(frames: &[Frame]) -> Vec<String> {
    frames
        .iter()
        .map(|frame| match frame.origin() {
            Some(at) => format!("origin {}", at.unwrap_or_default()),
            None => {
                let image = frame
                    .aimed()
                    .map_or(frame.image.clone(), |(_, image)| image);
                String::from_utf8_lossy(&image).into_owned()
            }
        })
        .collect()
}

/// Makes a frame every `every` from another thread until `until`, numbered from 1.
fn making(screen: &StandInScreen, every: Duration, until: Instant) -> std::thread::JoinHandle<()> {
    let screen = screen.clone();
    std::thread::spawn(move || {
        let mut n: u32 = 0;
        while Instant::now() < until {
            n += 1;
            screen.makes(&n.to_be_bytes());
            std::thread::sleep(every);
        }
    })
}

#[tokio::test]
async fn a_browser_starts_with_a_fresh_data_folder_of_its_own_and_makes_no_frame_unwatched() {
    let linked = linked().await;

    let (program, screen) = linked.browser().await;
    screen.makes(b"nobody is looking");

    let shown = screen.shown();
    assert_eq!(
        shown.data,
        linked.store.path().join("browsers").join(&program),
        "its data folder is its own, under the host's store"
    );
    assert_eq!(
        shown.shows, 0,
        "no frames are asked for with no view attached"
    );
    assert_eq!(shown.made, 0);
}

#[tokio::test]
async fn a_browser_in_a_project_the_host_does_not_hold_is_refused_and_starts_nothing() {
    let linked = linked().await;

    let refused = linked
        .client
        .start_program(StartProgram::of(
            "01J9OTHER0000000000000000",
            ProgramKind::Browser,
        ))
        .await
        .unwrap_err();

    assert_eq!(
        refused.refusal().map(|r| r.code.as_str()),
        Some(session::NO_SUCH_PROJECT)
    );
    assert!(linked.browsers.started().is_empty());
}

#[tokio::test]
async fn a_focused_view_is_its_latest_frame_then_every_frame_and_frames_stop_when_it_detaches() {
    let mut linked = linked().await;
    let (program, screen) = linked.browser().await;
    let (view, mut reader) = linked.watch(&program, ViewRole::Focused).await;
    assert_eq!(screen.shown().shows, 1, "a view asks for frames");
    assert_eq!(screen.shown().every, Some(Duration::ZERO), "at full rate");

    screen.makes(b"one");
    screen.makes(b"two");
    let seen = frames_until(&mut reader, Instant::now() + Duration::from_millis(300)).await;
    assert_eq!(seen.last().map(Vec::as_slice), Some(&b"two"[..]));

    // A second view starts from the latest frame, whole.
    let (other, mut second) = linked.watch(&program, ViewRole::Focused).await;
    let first_seen = frames_until(&mut second, Instant::now() + Duration::from_millis(300)).await;
    assert_eq!(first_seen, [b"two".to_vec()]);
    assert!(second.snapshot_complete());

    linked.client.detach_view(&program, view).await.unwrap();
    linked.client.detach_view(&program, other).await.unwrap();
    timeout(PATIENCE, async {
        while screen.shown().showing {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("frames stop once no view is attached");
    screen.makes(b"unwatched");
    assert_eq!(screen.shown().made, 2);
}

#[tokio::test]
async fn a_thumbnail_is_sent_about_one_frame_a_second_while_the_focused_view_gets_every_one() {
    let mut linked = linked().await;
    let (program, screen) = linked.browser().await;
    let (_, mut thumbnail) = linked.watch(&program, ViewRole::Thumbnail).await;
    assert_eq!(
        screen.shown().every,
        Some(Duration::from_secs(1)),
        "with only a thumbnail, the engine is asked for a frame a second"
    );
    let (_, mut focused) = linked.watch(&program, ViewRole::Focused).await;
    assert_eq!(screen.shown().every, Some(Duration::ZERO));

    let until = Instant::now() + Duration::from_millis(2500);
    let maker = making(&screen, Duration::from_millis(20), until);
    let (to_thumbnail, to_focused) = tokio::join!(
        frames_until(&mut thumbnail, until + Duration::from_millis(200)),
        frames_until(&mut focused, until + Duration::from_millis(200)),
    );
    maker.join().unwrap();

    assert!(
        to_focused.len() >= 60,
        "the focused view gets every frame: {}",
        to_focused.len()
    );
    assert!(
        (2..=4).contains(&to_thumbnail.len()),
        "the thumbnail gets about one a second: {}",
        to_thumbnail.len()
    );
}

#[tokio::test]
async fn a_view_not_done_drawing_its_frame_is_sent_no_other_then_the_newest_whole() {
    // One frame in flight (#2050): a view slow to draw never falls behind into a queue of
    // frames; once it draws again it is given the newest frame, whole.
    let mut linked = linked().await;
    let (program, screen) = linked.browser().await;
    let (_, mut reader) = linked.watch(&program, ViewRole::Focused).await;
    let snapshot =
        every_frame_until(&mut reader, Instant::now() + Duration::from_millis(200)).await;
    assert!(snapshot.is_empty(), "nothing made yet");

    let big = vec![0x5a_u8; 200 * 1024];
    screen.makes(&big);
    // The frame in flight, read and drawn but for its last piece.
    let mut held = Vec::new();
    let mut last = 0;
    while let Ok(Some(Ok(Chunk::Live(bytes)))) =
        timeout(Duration::from_millis(300), reader.next()).await
    {
        if last > 0 {
            reader.ack(last).await.unwrap();
        }
        last = bytes.len();
        held.extend_from_slice(&bytes);
    }
    for n in 0..20u8 {
        screen.makes(&vec![n; 200 * 1024]);
    }
    let mut frames = Frames::default();
    let in_flight = frames.push(&held).unwrap();
    let images: Vec<_> = in_flight.iter().filter_map(Frame::aimed).collect();
    assert_eq!(
        images.len(),
        1,
        "one frame in flight: {:?}",
        said(&in_flight).len()
    );
    assert_eq!(images[0].1.len(), 200 * 1024, "whole");
    assert!(frames.is_between_frames());
    let nothing_more = timeout(Duration::from_millis(300), reader.next()).await;
    assert!(
        nothing_more.is_err(),
        "nothing more is sent until it is drawn"
    );

    reader.ack(last).await.unwrap();
    let next = every_frame_until(&mut reader, Instant::now() + Duration::from_millis(500)).await;
    assert_eq!(next.len(), 1, "one frame: the newest, none between");
    let (_, image) = next[0].aimed().expect("an aimed frame");
    assert_eq!(&image[..], &vec![19u8; 200 * 1024][..], "and it is whole");
    linked.hosting.end_all();
}

#[tokio::test]
async fn a_stopped_browser_ends_its_engine_and_its_fresh_data_folder_goes() {
    let mut linked = linked().await;
    linked
        .client
        .subscribe(session::Since::new())
        .await
        .unwrap();
    let (program, screen) = linked.browser().await;
    let data = screen.shown().data.clone();
    assert!(data.is_dir());

    linked.client.stop(&program).await.unwrap();

    assert!(screen.shown().stopped, "its engine is ended");
    let told = timeout(PATIENCE, async {
        loop {
            if let Some(Told::Pushed(Pushed::Ended { program, exit, .. })) =
                linked.events.recv().await
            {
                return (program, exit);
            }
        }
    })
    .await
    .unwrap();
    assert_eq!(told, (program, Exit::Code(0)));
    timeout(PATIENCE, async {
        while data.exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("a fresh browser's data folder is not kept");
}

#[tokio::test]
async fn every_browser_ends_with_the_host() {
    let linked = linked().await;
    let (_, one) = linked.browser().await;
    let (_, two) = linked.browser().await;

    linked.hosting.end_all();

    assert!(one.shown().stopped && two.shown().stopped);
}

#[tokio::test]
async fn a_browser_is_listed_as_one_to_a_link_that_knows_browsers() {
    let linked = linked().await;
    let (program, _) = linked.browser().await;

    let listed = linked.client.list().await.unwrap();

    let browser = listed.iter().find(|p| p.chat == program).unwrap();
    assert_eq!(browser.kind, Some(ProgramKind::Browser));
    assert_eq!(browser.number, None, "a browser has no chat's number");
}

#[tokio::test]
async fn stop_all_ends_every_browser_and_no_browser_starts_until_the_window_rearms() {
    let linked = linked().await;
    let (_, one) = linked.browser().await;

    linked.client.stop_all().await.expect("stopped");

    assert!(one.shown().stopped, "its engine is ended");
    let refused = linked
        .client
        .start_program(StartProgram::of(PROJECT, ProgramKind::Browser))
        .await
        .unwrap_err();
    assert_eq!(
        refused.refusal().map(|r| r.code.as_str()),
        Some(session::STOPPED)
    );
    assert_eq!(linked.browsers.started().len(), 1, "nothing more started");

    linked.client.rearm().await.expect("re-armed");
    linked.browser().await;
}

// ---- A chat's browser (#2013): what it may reach, and the acts the window hands on (1.11) ----

#[tokio::test]
async fn a_browser_is_started_with_the_reach_the_window_gives_it() {
    let linked = linked().await;
    let reach = BrowserReach {
        listed: vec![session::Listed {
            host: "docs.example.com".into(),
            layer: session::Layer::Open,
        }],
        never: Vec::new(),
        withdrawn: vec![5173],
        open_web: true,
    };

    linked
        .client
        .start_program(StartProgram {
            reach: Some(reach.clone()),
            ..StartProgram::of(PROJECT, ProgramKind::Browser)
        })
        .await
        .expect("a browser starts");

    let screen = linked.browsers.started().last().unwrap().clone();
    assert_eq!(screen.shown().reach, Some(reach));
}

#[tokio::test]
async fn an_act_reaches_its_browser_with_its_id_and_the_browser_s_answer_comes_back() {
    let linked = linked().await;
    let (program, screen) = linked.browser().await;
    screen.answers(Ok(Browsed {
        settled: true,
        text: Some("- link \"Docs\" [e3]".into()),
        ..Browsed::default()
    }));

    let answered = linked
        .client
        .browse(
            &program,
            "c-1",
            Act::Click {
                target: "e3".into(),
            },
        )
        .await
        .expect("the act is answered");

    assert_eq!(answered.text.as_deref(), Some("- link \"Docs\" [e3]"));
    assert_eq!(
        screen.shown().acts,
        [(
            "c-1".to_owned(),
            Act::Click {
                target: "e3".into()
            }
        )]
    );
}

#[tokio::test]
async fn an_act_the_browser_refuses_is_refused_in_its_words() {
    let linked = linked().await;
    let (program, screen) = linked.browser().await;
    screen.answers(Err(
        "e3 is not shown on the page now: take a new snapshot".into()
    ));

    let refused = linked
        .client
        .browse(
            &program,
            "c-2",
            Act::Click {
                target: "e3".into(),
            },
        )
        .await
        .unwrap_err();

    let refusal = refused.refusal().expect("a refusal");
    assert_eq!(refusal.code, session::BROWSER_REFUSED);
    assert_eq!(
        refusal.why,
        "e3 is not shown on the page now: take a new snapshot"
    );
}

#[tokio::test]
async fn an_act_on_a_browser_whose_engine_ended_is_refused_as_ended_by_its_code() {
    let linked = linked().await;
    let (program, screen) = linked.browser().await;
    screen.ends(Exit::Code(0));

    let refused = linked
        .client
        .browse(&program, "c-4", Act::Pages)
        .await
        .unwrap_err();

    let refusal = refused.refusal().expect("a refusal");
    assert_eq!(refusal.code, session::BROWSER_ENDED);
    assert_eq!(refusal.why, purlis_host::AN_ENDED_BROWSER);
}

#[tokio::test]
async fn an_act_for_a_program_the_host_does_not_hold_is_refused_as_no_such_chat() {
    let linked = linked().await;

    let refused = linked
        .client
        .browse("01J9NOSUCH000000000000000", "c-3", Act::Pages)
        .await
        .unwrap_err();

    assert_eq!(
        refused.refusal().map(|r| r.code.as_str()),
        Some(session::NO_SUCH_CHAT)
    );
}

#[tokio::test]
async fn a_view_is_told_its_pages_origin_alone_where_it_changes_and_in_its_snapshot() {
    // 1.35 (#2300): what the chip names, from the engine's record, cut to the origin here.
    let mut linked = linked().await;
    let (program, screen) = linked.browser().await;
    let (_, mut reader) = linked.watch(&program, ViewRole::Focused).await;

    screen.shown().address = "http://localhost:5173/login?next=%2Fadmin".into();
    screen.makes(b"one");
    let soon = || Instant::now() + Duration::from_millis(300);
    assert_eq!(
        said(&every_frame_until(&mut reader, soon()).await),
        ["origin http://localhost:5173", "one"]
    );
    // A view is sent the newest frame alone: each is read before the next is made.
    let mut seen = Vec::new();
    for (address, image) in [
        ("http://localhost:5173/account", "two"),
        ("https://ada:pw@docs.example.test/a#b", "three"),
        ("about:blank", "four"),
    ] {
        screen.shown().address = address.into();
        screen.makes(image.as_bytes());
        seen.extend(said(&every_frame_until(&mut reader, soon()).await));
    }
    assert_eq!(
        seen,
        [
            "two",
            "origin https://docs.example.test",
            "three",
            "origin ",
            "four"
        ],
        "the same origin is not said again; no path, user or fragment ever is"
    );

    // A view attached again is told where its page is before its first image.
    screen.shown().address = "http://localhost:5173/".into();
    screen.makes(b"five");
    let (_, mut again) = linked.watch(&program, ViewRole::Thumbnail).await;
    assert_eq!(
        said(&every_frame_until(&mut again, soon()).await),
        ["origin http://localhost:5173", "five"]
    );
}

#[tokio::test]
async fn a_view_of_a_link_before_1_35_is_sent_images_alone() {
    let before = purlis_session_protocol::version::Speaks::new([
        purlis_session_protocol::version::Version {
            major: 1,
            minor: 34,
        },
    ]);
    let mut linked = linked_speaking(before).await;
    let (program, screen) = linked.browser().await;
    let (_, mut reader) = linked.watch(&program, ViewRole::Focused).await;

    screen.shown().address = "http://localhost:5173/".into();
    screen.makes(b"one");

    let frames = every_frame_until(&mut reader, Instant::now() + Duration::from_millis(300)).await;
    assert_eq!(said(&frames), ["one"]);
    assert!(frames.iter().all(|frame| frame.format == AIMED_JPEG));
}

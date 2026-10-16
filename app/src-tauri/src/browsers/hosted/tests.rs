// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The window's browsers over an in-memory link to a real host whose browsers are stand-ins:
//! a browser opened in a project, a view of it drawn a whole frame at a time, and Stop all
//! ending every browser. The same over a real engine is `purlis-browser-engine`'s, on CI.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use purlis_host::standin::{StandInBrowsers, StandIns};
use purlis_host::{ComposesNothing, Hosting};

use super::WindowBrowsers;
use crate::browsers::BrowserViewRole;

struct Window {
    browsers: WindowBrowsers,
    started: StandInBrowsers,
    root: std::path::PathBuf,
    _dirs: (tempfile::TempDir, tempfile::TempDir),
    _runtime: tokio::runtime::Runtime,
}

fn window() -> Window {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let store = tempfile::tempdir().unwrap();
    let started = StandInBrowsers::in_folder(store.path().join("browsers"));
    let hosting =
        Hosting::running_with(ComposesNothing, StandIns::default()).with_browsers(started.clone());
    let (connection, _window) = crate::hostedsessions::tests::in_memory(hosting, &runtime);
    let project = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(project.path()).unwrap();
    std::fs::write(root.join(purlis_core::plane::MANIFEST), "").unwrap();
    Window {
        browsers: WindowBrowsers::new(connection),
        started,
        root,
        _dirs: (store, project),
        _runtime: runtime,
    }
}

fn until(what: &str, done: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !done() {
        assert!(Instant::now() < deadline, "{what} never happened");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn a_browser_the_window_opens_is_drawn_a_whole_frame_at_a_time() {
    let window = window();
    let program = window.browsers.open(&window.root).unwrap();
    let screen = window.started.started()[0].clone();
    let drawn: Arc<Mutex<Vec<Vec<u8>>>> = Arc::default();
    let drawing = Arc::clone(&drawn);

    let watching = window
        .browsers
        .watch(
            &program,
            BrowserViewRole::Focused,
            Box::new(move |image| drawing.lock().unwrap().push(image.to_vec())),
        )
        .unwrap();
    assert_eq!((watching.width, watching.height), (1280, 800));
    let big = vec![7_u8; 100 * 1024];
    screen.makes(b"\xff\xd8 first");
    until("the first frame is drawn", || {
        drawn.lock().unwrap().len() == 1
    });
    screen.makes(&big);
    until("the second frame is drawn whole", || {
        drawn.lock().unwrap().len() == 2
    });
    assert_eq!(drawn.lock().unwrap()[1], big);

    window.browsers.unwatch(&program, watching.view).unwrap();
    until("frames stop with the last view", || !screen.shown().showing);
}

#[test]
fn two_clones_of_one_project_each_open_a_browser() {
    // #2178: one project id, two folders.
    let window = window();
    let id = "[project]\nid = \"01J9PRJCT00000000000000000\"\n";
    std::fs::write(window.root.join(purlis_core::plane::MANIFEST), id).unwrap();
    let clone = tempfile::tempdir().unwrap();
    let other = std::fs::canonicalize(clone.path()).unwrap();
    std::fs::write(other.join(purlis_core::plane::MANIFEST), id).unwrap();

    window.browsers.open(&window.root).unwrap();
    window.browsers.open(&other).unwrap();

    assert_eq!(window.started.started().len(), 2);
}

#[test]
fn stop_all_stops_every_browser_the_host_runs() {
    let window = window();
    window.browsers.open(&window.root).unwrap();
    window.browsers.open(&window.root).unwrap();

    let stopped = window.browsers.stop_every_browser();

    assert_eq!(stopped, 2);
    assert!(
        window
            .started
            .started()
            .iter()
            .all(|screen| screen.shown().stopped)
    );
}

#[test]
fn letting_go_of_a_project_ends_the_browsers_the_window_started_in_it_and_its_hold() {
    let window = window();
    let one = window.browsers.open(&window.root).unwrap();
    window.browsers.open(&window.root).unwrap();
    window.browsers.close(&one).unwrap();

    let ended = window.browsers.let_go(&window.root);

    assert_eq!(ended, 1, "the one still running");
    // A close answers once the host has forgotten the browser, and ends it after.
    until("every browser the window started has stopped", || {
        window
            .started
            .started()
            .iter()
            .all(|screen| screen.shown().stopped)
    });
    let link = window.browsers.link().unwrap();
    assert!(
        matches!(
            link.call(purlis_session_protocol::session::Command::List),
            Ok(purlis_session_protocol::session::Answer::Chats(listed)) if listed.is_empty()
        ),
        "nothing is left in the host"
    );
    let held_again = window.browsers.open(&window.root);
    assert!(
        held_again.is_ok(),
        "the project is held again when asked: {held_again:?}"
    );
}

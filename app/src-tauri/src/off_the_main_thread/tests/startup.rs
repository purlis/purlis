// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The startup targets, held in the core** (#5326, the spec #5319, stories 12 and 13): at a
//! launch restoring three projects, the project already open answers the window's first reads
//! within 100 ms, and nothing the window asks holds its thread for 200 ms, the slow log's stall
//! (`app/src/slowLog.ts`'s `STALL_MS`), while the other two are slow to open.
//!
//! The window's frames stop while its thread is held, so a command that does a project's slow
//! work on the thread that asked is a stall the person feels; and a read that waits on another
//! project's open is a window that does not answer. Both were what a launch measured on
//! 2026-11-05 felt like. The window's own side is `app/src/StartupTargets.window.test.tsx`.
//!
//! **Real time, with wide margins** (ADR 0086 §1): each slow open is held for [`SLOW`], many
//! times both targets, and what is asked of a project already open is a lookup and a small
//! read, a few milliseconds on any runner. A read that waited on an open would wait [`SLOW`].

use std::path::Path;
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use serde_json::json;
use tauri::Manager;
use tauri::test::{MockRuntime, mock_builder};

use super::tests::{a_plane, ask, ask_without_waiting};
use crate::lifecycle::WINDOW;
use crate::planes::{PlaneId, Planes};

/// The window answers within this long of appearing (story 13).
const FIRST_ANSWER: Duration = Duration::from_millis(100);
/// A frame this long after the one before is a stall (story 12, `slowLog.ts`'s `STALL_MS`).
const STALL: Duration = Duration::from_millis(200);
/// How long a slow project takes to be built as it opens, unless let go sooner.
const SLOW: Duration = Duration::from_secs(3);

/// The app on Tauri's mock runtime with its window, holding the launch's project at `launch`,
/// and building each project in `slow` for [`SLOW`] or until a release comes, whichever is
/// first; `entered` hears each one begin to be built.
fn app_restoring(
    launch: &Path,
    slow: Vec<std::path::PathBuf>,
    entered: mpsc::Sender<()>,
    released: mpsc::Receiver<()>,
) -> (tauri::App<MockRuntime>, PlaneId) {
    let released = Mutex::new(released);
    let entered = Mutex::new(entered);
    let planes = Planes::telling(Arc::new(|_| {}), crate::Shipped::default(), None).building_with(
        Arc::new(move |root| {
            if slow.iter().any(|one| one == root) {
                let _ = entered.lock().expect("the entered sender").send(());
                let _ = released.lock().expect("the release").recv_timeout(SLOW);
            }
        }),
    );
    let plane = planes.open(launch);
    let app = mock_builder()
        .manage(planes)
        .invoke_handler(tauri::generate_handler![
            crate::opener::open_plane,
            crate::plane_sidebar,
            crate::workspace_panels,
            crate::plane_root_panels,
            crate::curation::curation_offers,
        ])
        .build(tauri_context!(test = true))
        .expect("the app builds");
    tauri::WebviewWindowBuilder::new(&app, WINDOW, tauri::WebviewUrl::default())
        .build()
        .expect("the main window");
    (app, plane)
}

#[test]
fn a_launch_restoring_three_projects_answers_within_100_ms_and_never_holds_the_window_200_ms() {
    let launch = a_plane();
    let slow = [a_plane(), a_plane()];
    let slow_roots: Vec<_> = slow
        .iter()
        .map(|dir| dir.path().canonicalize().expect("resolved"))
        .collect();
    let (entered, building) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let (app, front) = app_restoring(launch.path(), slow_roots.clone(), entered, released);

    // The window asks for the two other projects, as its restore does, side by side.
    let opening: Vec<_> = slow_roots
        .iter()
        .map(|root| ask_without_waiting(&app, "open_plane", json!({ "path": root })))
        .collect();
    for (root, asking) in slow_roots.iter().zip(&opening) {
        assert!(
            asking.held_for < STALL,
            "opening {} held the window's thread {:?}",
            root.display(),
            asking.held_for
        );
    }
    for _ in &slow_roots {
        building
            .recv_timeout(Duration::from_secs(30))
            .expect("each slow project has begun to be built");
    }

    // Meanwhile, the project in front answers what the window first asks of it.
    let reads = [
        ("plane_sidebar", json!({ "plane": front })),
        (
            "workspace_panels",
            json!({ "plane": front, "workspace": "alpha" }),
        ),
        ("plane_root_panels", json!({ "plane": front })),
        (
            "curation_offers",
            json!({ "plane": front, "subjects": ["workspace:alpha", "plane"] }),
        ),
    ];
    let mut slow_answers = Vec::new();
    for (command, args) in reads {
        let asked_at = Instant::now();
        let asked = ask(&app, command, args);
        let took = asked_at.elapsed();
        assert!(asked.answer.is_ok(), "{command} refused: {:?}", asked.answer);
        if took > FIRST_ANSWER || asked.held_for >= STALL {
            slow_answers.push(format!(
                "{command} answered in {took:?}, holding the window {:?}",
                asked.held_for
            ));
        }
    }
    // Still building: the reads above were answered while the others were opening.
    let still_opening = app.state::<Planes>().open_now().len() == 1;
    for _ in &slow_roots {
        let _ = release.send(());
    }
    assert!(still_opening, "the slow projects had opened before the reads were asked");
    assert!(slow_answers.is_empty(), "{slow_answers:#?}");

    for (root, asking) in slow_roots.iter().zip(opening) {
        let opened = asking.answered();
        assert!(
            opened.answer.is_ok(),
            "{} did not open: {:?}",
            root.display(),
            opened.answer
        );
    }
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **An ask answers while another project's open, or another project's burst, is held** (#7902,
//! FS-C2): the epic's slow log had an ask stuck in the core for more than 260 s. The registry of
//! projects is locked only to find one, and a burst of asks is one project's: neither holds an
//! unrelated ask.

use std::path::Path;
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

use tauri::Manager;
use tauri::test::{MockRuntime, mock_builder};

use crate::planes::{PlaneId, Planes};

/// The app on Tauri's mock runtime, with `planes`.
fn app_with(planes: Planes) -> tauri::AppHandle<MockRuntime> {
    mock_builder()
        .manage(planes)
        .build(tauri_context!(test = true))
        .expect("the app builds")
        .handle()
        .clone()
}

/// The sandbox's state of `plane`, as its command answers it.
fn sandbox_state(app: &tauri::AppHandle<MockRuntime>, plane: &PlaneId) -> crate::sandboxing::SandboxState {
    tauri::async_runtime::block_on(crate::sandboxing::sandbox_state(app.state(), plane.clone()))
        .expect("the state")
}

#[test]
fn an_ask_of_one_project_answers_while_another_projects_open_is_held() {
    let (quick, slow) = (super::costs::a_project(), super::costs::a_project());
    let held_at = slow.path().canonicalize().expect("resolved");
    let (entered, sweeping) = mpsc::channel::<()>();
    let (release, released) = mpsc::channel::<()>();
    let released = Mutex::new(released);
    let planes = Planes::telling(Arc::new(|_| {}), crate::Shipped::default(), None).sweeping_with(
        Arc::new(move |root: &Path| {
            if root == held_at {
                let _ = entered.send(());
                let _ = released
                    .lock()
                    .expect("the release")
                    .recv_timeout(Duration::from_secs(120));
            }
        }),
    );
    let app = app_with(planes);
    let plane = app.state::<Planes>().open(quick.path());
    let opening = std::thread::spawn({
        let (app, slow) = (app.clone(), slow.path().to_path_buf());
        move || app.state::<Planes>().open(&slow)
    });
    sweeping
        .recv_timeout(Duration::from_secs(60))
        .expect("the other project's open is under way");

    let (told, answered) = mpsc::channel();
    std::thread::spawn({
        let (app, plane) = (app.clone(), plane.clone());
        move || {
            let _ = told.send(sandbox_state(&app, &plane));
        }
    });
    // No read here takes a fraction of this: only an ask waiting on the held open gets there.
    let answer = answered.recv_timeout(Duration::from_secs(60));
    assert!(!opening.is_finished(), "the premise: the other open is held");
    release.send(()).expect("released");
    opening.join().expect("the other project opens");
    answer.expect("the ask answered while another project's open was held");
}

#[test]
fn an_ask_of_one_project_answers_while_another_projects_burst_holds_its_read() {
    let (busy, other) = (super::costs::a_project(), super::costs::a_project());
    let app = app_with(Planes::telling(
        Arc::new(|_| {}),
        crate::Shipped::default(),
        None,
    ));
    let busy = app.state::<Planes>().open(busy.path());
    let other = app.state::<Planes>().open(other.path());
    let answer = sandbox_state(&app, &busy);
    let held = app.state::<Planes>().held(&busy).expect("held");
    let burst = &held.bursts().sandbox_state;
    let (inside, started) = mpsc::channel::<()>();
    let (release, hold) = mpsc::channel::<()>();
    std::thread::scope(|scope| {
        // A read of the busy project held running, and an ask of it waiting for the next.
        let running = scope.spawn(move || {
            burst.ask(|| {
                inside.send(()).expect("told");
                hold.recv().expect("released");
                answer.clone()
            })
        });
        started.recv().expect("the read runs");
        let waiting = scope.spawn(|| sandbox_state(&app, &busy));
        while burst.waiting() < 1 {
            std::thread::yield_now();
        }
        // The other project's ask: answered, or caught joining the busy project's burst.
        let unrelated = scope.spawn(|| sandbox_state(&app, &other));
        while !unrelated.is_finished() && burst.waiting() < 2 {
            std::thread::yield_now();
        }
        let joined = burst.waiting();
        release.send(()).expect("released");
        running.join().expect("the running read");
        waiting.join().expect("the waiting ask");
        unrelated.join().expect("the other project's ask");
        assert_eq!(
            joined, 1,
            "the other project's ask waited for the busy project's burst"
        );
    });
}

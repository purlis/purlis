// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Through a real engine (#2973): at the person's first key, a frame of the page moves from the
//! page's own site to another, so the engine puts it in a process of its own. A move of the person's pointer aimed at what the
//! frame showed before is dropped; one on the page's own document still goes on.
//!
//! These need a real Chrome and the right to start it and to listen on a loopback port, which a
//! sandboxed agent does not have. They run where `PURLIS_TEST_ENGINE` names a Chrome
//! executable, and on CI against the runner's own Chrome. Anywhere else each test says why it
//! did not run and passes.

#![cfg(unix)]

use crate::common;

use std::time::{Duration, Instant};

use common::drive::{open, snapshot};
use common::site::started;
use purlis_browser_engine::drive::{MouseButton, MouseKind, PersonInput, Refusal};

fn move_to(aim: &purlis_browser_engine::drive::Aim, x: f64, y: f64) -> PersonInput {
    PersonInput::Mouse {
        aim: aim.clone(),
        kind: MouseKind::Moved,
        x,
        y,
        button: MouseButton::None,
        clicks: 0,
        modifiers: 0,
        delta_x: 0.0,
        delta_y: 0.0,
    }
}

#[test]
fn a_move_aimed_at_a_frame_from_before_it_moved_to_another_site_is_dropped() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    open(&browser, "a-frame-that-moves-to-another-site.html");
    // The frame is on the page's own site until the person's first key.
    assert!(
        snapshot(&browser).contains("The opened page"),
        "the frame shows the page's own site's document: {}",
        snapshot(&browser)
    );
    browser.take_over();
    let aim = browser.aim().expect("a page is shown");
    browser
        .person_input(&PersonInput::Key {
            aim: aim.clone(),
            down: true,
            key: "Escape".into(),
            code: "Escape".into(),
            key_code: 27,
            modifiers: 0,
        })
        .expect("the key reaches the page");
    let until = Instant::now() + Duration::from_secs(10);
    while browser.aim().as_ref() == Some(&aim) {
        assert!(Instant::now() < until, "the frame's move was never known");
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(
        browser.person_input(&move_to(&aim, 150.0, 175.0)),
        Err(Refusal::Missed),
        "a move onto the frame that moved"
    );
    assert!(
        browser.person_input(&move_to(&aim, 150.0, 20.0)).is_ok(),
        "a move on the page's own document"
    );
}

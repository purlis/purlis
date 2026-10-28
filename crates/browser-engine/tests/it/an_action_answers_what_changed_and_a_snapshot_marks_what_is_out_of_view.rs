// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What changed on a page, and what it has but does not show (#2137), through a real engine:
//! the lines new and gone since the page was last read, and an element laid out beyond what the
//! viewport shows marked out of view until it is scrolled to.
//!
//! These need a real Chrome and the right to start it and to listen on a loopback port, which a
//! sandboxed agent does not have. They run where `PURLIS_TEST_ENGINE` names a Chrome
//! executable, and on CI against the runner's own Chrome. Anywhere else each test says why it
//! did not run and passes.

#![cfg(unix)]

use crate::common;

use common::drive::{call, find, open, ref_on, snapshot, text};
use common::site::started;
use purlis_browser_engine::drive::{Call, Settled};

#[test]
fn an_element_beyond_the_viewport_is_marked_out_of_view_until_it_is_scrolled_to() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    assert_eq!(open(&browser, "long.html").settled, Settled::Yes);
    let far = find(&browser, "Reach me");
    assert!(far.contains("(out of view)"), "{far}");
    let near = find(&browser, "Not reached yet");
    assert!(!near.contains("(out of view)"), "{near}");
    let reach = ref_on(&far, "Reach me");
    call(
        &browser,
        Call::Scroll {
            page: None,
            at: Some(reach),
            x: 0.0,
            y: 0.0,
        },
    );
    let now = find(&browser, "Reach me");
    assert!(!now.contains("(out of view)"), "{now}");
}

#[test]
fn changes_after_a_click_are_the_lines_it_changed() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    open(&browser, "counter.html");
    let add = ref_on(&snapshot(&browser), "Add one");
    call(&browser, Call::Click { target: add });
    let changed = text(&call(&browser, Call::Changes { page: None }));
    assert!(
        changed
            .lines()
            .any(|l| l.starts_with("new: ") && l.contains("Count: 1")),
        "{changed}"
    );
    assert!(
        changed
            .lines()
            .any(|l| l.starts_with("gone: ") && l.contains("Count: 0")),
        "{changed}"
    );
    // The click also focused the button: a change of its line too.
    assert!(
        changed
            .lines()
            .any(|l| l.starts_with("new: ") && l.contains("Add one") && l.contains("focused")),
        "{changed}"
    );
}

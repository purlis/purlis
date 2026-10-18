// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What a box or a frame in view does not show (#2564), through a real engine: an item
//! scrolled out of a list that scrolls, and an element scrolled out of a frame on the same site
//! or on another, are marked out of view; what the list or the frame shows is not, and an item
//! scrolled to is not marked again.
//!
//! These need a real Chrome and the right to start it and to listen on a loopback port, which a
//! sandboxed agent does not have. They run where `PURLIS_TEST_ENGINE` names a Chrome
//! executable, and on CI against the runner's own Chrome. Anywhere else each test says why it
//! did not run and passes.

#![cfg(unix)]

mod common;

use common::drive::{call, find, open, ref_on};
use common::site::started;
use purlis_browser_engine::drive::{Call, Settled};

/// The lines of `found` that hold `words`.
fn lines_of<'a>(found: &'a str, words: &str) -> Vec<&'a str> {
    let lines: Vec<&str> = found.lines().filter(|l| l.contains(words)).collect();
    assert!(!lines.is_empty(), "no {words:?} in\n{found}");
    lines
}

#[test]
fn an_item_scrolled_out_of_a_list_is_marked_out_of_view_and_one_the_list_shows_is_not() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    assert_eq!(open(&browser, "scroll-boxes.html").settled, Settled::Yes);
    let shown = find(&browser, "Shown in the list");
    for line in lines_of(&shown, "Shown in the list") {
        assert!(!line.contains("(out of view)"), "{shown}");
    }
    let last = find(&browser, "Last in the list");
    assert!(
        lines_of(&last, "Last in the list")
            .iter()
            .all(|l| l.contains("(out of view)")),
        "{last}"
    );
    // Scrolled to, it is shown.
    call(
        &browser,
        Call::Scroll {
            page: None,
            at: Some(ref_on(&last, "Last in the list")),
            x: 0.0,
            y: 0.0,
        },
    );
    let now = find(&browser, "Last in the list");
    for line in lines_of(&now, "Last in the list") {
        assert!(!line.contains("(out of view)"), "{now}");
    }
}

#[test]
fn an_element_scrolled_out_inside_a_frame_in_view_is_marked_out_of_view() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    assert_eq!(open(&browser, "scroll-boxes.html").settled, Settled::Yes);
    // One frame on the page's own site, one on another site, read in a session of its own.
    let top = find(&browser, "Top of the frame");
    let tops = lines_of(&top, "Top of the frame");
    assert!(tops.len() >= 2, "both frames: {top}");
    for line in tops {
        assert!(!line.contains("(out of view)"), "{top}");
    }
    let far = find(&browser, "Far down in the frame");
    let fars = lines_of(&far, "Far down in the frame");
    assert!(fars.len() >= 2, "both frames: {far}");
    for line in fars {
        assert!(line.contains("(out of view)"), "{far}");
    }
}

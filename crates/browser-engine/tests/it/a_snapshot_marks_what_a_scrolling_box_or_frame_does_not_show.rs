// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What a box or a frame in view does not show (#2564), through a real engine: an item
//! scrolled out of a list that scrolls, and an element scrolled out of a frame, on the page's
//! own site or on another one read in a session of its own (#2689), are marked out of view;
//! what the list or the frame shows is not, and an item scrolled to is not marked again. A box
//! on the page that cuts off a frame from another site marks what it hides of the frame, placed
//! inside the frame's border and padding, where the page scales it, and through a frame further
//! out (#2840).
//!
//! These need a real Chrome and the right to start it and to listen on a loopback port, which a
//! sandboxed agent does not have. They run where `PURLIS_TEST_ENGINE` names a Chrome
//! executable, and on CI against the runner's own Chrome. Anywhere else each test says why it
//! did not run and passes.

#![cfg(unix)]

use crate::common;

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
    let top = find(&browser, "Top of the frame");
    let tops = lines_of(&top, "Top of the frame");
    for line in tops {
        assert!(!line.contains("(out of view)"), "{top}");
    }
    let far = find(&browser, "Far down in the frame");
    let fars = lines_of(&far, "Far down in the frame");
    for line in fars {
        assert!(line.contains("(out of view)"), "{far}");
    }
}

#[test]
fn an_element_scrolled_out_inside_a_frame_from_another_site_is_marked_out_of_view() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    let opened = open(&browser, "cross-site-boxes.html");
    assert_eq!(opened.settled, Settled::Yes, "{opened:?}");
    let top = find(&browser, "Top of the frame");
    for line in lines_of(&top, "Top of the frame") {
        assert!(!line.contains("(out of view)"), "{top}");
    }
    let far = find(&browser, "Far down in the frame");
    for line in lines_of(&far, "Far down in the frame") {
        assert!(line.contains("(out of view)"), "{far}");
    }
}

#[test]
fn a_box_on_the_page_that_cuts_off_a_frame_from_another_site_marks_what_it_hides() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    let opened = open(&browser, "cross-site-boxes.html");
    assert_eq!(opened.settled, Settled::Yes, "{opened:?}");
    // The box shows the frame's top 60 of its 150: its top line, not its button at 100.
    let top = find(&browser, "Top of the cut frame");
    for line in lines_of(&top, "Top of the cut frame") {
        assert!(!line.contains("(out of view)"), "{top}");
    }
    let low = find(&browser, "Low in the cut frame");
    for line in lines_of(&low, "Low in the cut frame") {
        assert!(line.contains("(out of view)"), "{low}");
    }
}

/// Opens `path`, and checks that `shown` is not marked out of view and `hidden` is.
fn marks_in(path: &str, shown: &str, hidden: &str) {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    let opened = open(&browser, path);
    assert_eq!(opened.settled, Settled::Yes, "{opened:?}");
    let found = find(&browser, shown);
    for line in lines_of(&found, shown) {
        assert!(!line.contains("(out of view)"), "{found}");
    }
    let found = find(&browser, hidden);
    for line in lines_of(&found, hidden) {
        assert!(line.contains("(out of view)"), "{found}");
    }
}

#[test]
fn a_box_cuts_off_a_padded_frame_inside_its_border_and_padding() {
    marks_in(
        "frame-boxes-padded.html",
        "Top of the padded frame",
        "Low in the padded frame",
    );
}

#[test]
fn a_box_cuts_off_a_frame_the_page_scales_where_the_page_draws_it() {
    marks_in(
        "frame-boxes-scaled.html",
        "Shown in the scaled frame",
        "Hidden in the scaled frame",
    );
}

#[test]
fn a_box_on_the_page_cuts_off_what_a_frame_nested_in_another_holds() {
    marks_in(
        "frame-boxes-nested.html",
        "Top of the inner frame",
        "Low in the inner frame",
    );
}

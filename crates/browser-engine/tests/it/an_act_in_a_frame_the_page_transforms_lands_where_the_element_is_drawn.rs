// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A click, a hover or a drag the agent aims at an element inside a frame from another site
//! lands where the element is drawn, however the page lays the frame out: turned and scaled, or
//! zoomed, or taller than the page's window so that the element is below the fold (#2557), or
//! nested in a frame from a third site below the fold of the frame around it (#2690).
//!
//! These need a real Chrome and the right to start it and to listen on a loopback port, which a
//! sandboxed agent does not have. They run where `PURLIS_TEST_ENGINE` names a Chrome
//! executable, and on CI against the runner's own Chrome. Anywhere else each test says why it
//! did not run and passes.

#![cfg(unix)]

use crate::common;

use common::drive::{call, find, next_id, open, ref_on, sees};
use common::site::started;
use purlis_browser_engine::drive::{Call, DragTo};

/// Opens `path`, whose frame from another site holds an "Add one" button, clicks the button,
/// and answers whether its count went up.
fn the_click_counts(path: &str) -> bool {
    let Some((browser, _data, _site)) = started() else {
        return true;
    };
    open(&browser, path);
    let add = ref_on(&find(&browser, "Add one"), "Add one");
    call(&browser, Call::Click { target: add });
    sees(&browser, "Count: 1")
}

/// Opens `path`, whose frame from another site holds a "Start here" and an "End here" square,
/// drags the first onto the second, and answers whether the frame saw the drag start on the one
/// and end on the other.
fn the_drag_lands(path: &str) -> bool {
    let Some((browser, _data, _site)) = started() else {
        return true;
    };
    open(&browser, path);
    let from = ref_on(&find(&browser, "Start here"), "Start here");
    let to = ref_on(&find(&browser, "End here"), "End here");
    call(
        &browser,
        Call::Drag {
            from,
            to: DragTo::Element(to),
        },
    );
    sees(&browser, "Dragged from start to end")
}

/// Opens `path`, whose frame from another site holds a "Point here" square, hovers over it,
/// and answers whether the frame saw the pointer come onto it.
fn the_hover_lands(path: &str) -> bool {
    let Some((browser, _data, _site)) = started() else {
        return true;
    };
    open(&browser, path);
    let target = ref_on(&find(&browser, "Point here"), "Point here");
    call(&browser, Call::Hover { target });
    sees(&browser, "Pointed at")
}

#[test]
fn a_hover_in_a_frame_the_page_turns_and_scales_comes_onto_its_element() {
    assert!(the_hover_lands("turned-pad.html"));
}

#[test]
fn a_click_in_a_frame_the_page_turns_and_scales_presses_the_button() {
    assert!(the_click_counts("turned-pad.html"));
}

#[test]
fn a_click_in_a_frame_taller_than_the_window_presses_a_button_below_the_fold() {
    assert!(the_click_counts("tall-pad.html"));
}

#[test]
fn a_click_in_a_frame_the_page_zooms_presses_the_button() {
    assert!(the_click_counts("zoomed-pad.html"));
}

#[test]
fn a_drag_in_a_frame_the_page_turns_and_scales_starts_and_ends_on_its_elements() {
    assert!(the_drag_lands("turned-pad.html"));
}

#[test]
fn a_drag_in_a_frame_the_page_zooms_starts_and_ends_on_its_elements() {
    assert!(the_drag_lands("zoomed-pad.html"));
}

#[test]
fn a_click_in_a_cross_site_frame_nested_below_a_middle_frames_fold_presses_the_button() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    // Each time from a page just opened, its frames not yet scrolled.
    for time in 1..=3 {
        open(&browser, "nested-pad.html");
        let found = find(&browser, "Add one");
        let add = ref_on(&found, "Add one");
        let clicked = browser.call(&next_id(), Call::Click { target: add });
        assert!(clicked.is_ok(), "click {time} answered {clicked:?}");
        let seen = sees(&browser, "Count: 1");
        // What the inner frame reads now: a count of 0 is a press that missed, no count at all
        // a frame two deep that is not read.
        let counted = find(&browser, "Count:");
        assert!(
            seen,
            "after click {time} the inner frame reads:\n{counted}\nthe button was found as:\n{found}"
        );
    }
}

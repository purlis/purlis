// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A blanked field inside a frame from another site is painted over in every screenshot,
//! however the page lays the frame out: turned and scaled, or zoomed (#2143).
//!
//! These need a real Chrome and the right to start it and to listen on a loopback port, which a
//! sandboxed agent does not have. They run where `PURLIS_TEST_ENGINE` names a Chrome
//! executable, and on CI against the runner's own Chrome. Anywhere else each test says why it
//! did not run and passes.

#![cfg(unix)]

use crate::common;

use common::drive::{filled_and_blanked, find, open, red_pixels, ref_on, screenshot};
use common::site::started;

/// Opens `path`, whose frame holds a red "Card number" field, fills the field and blanks it as a
/// login does, and answers how much of the field's red the screenshots show before and after.
fn red_before_and_after(path: &str) -> (usize, usize) {
    let Some((browser, _data, _site)) = started() else {
        return (1, 0);
    };
    open(&browser, path);
    let field = ref_on(&find(&browser, "Card number"), "Card number");
    let before = red_pixels(&screenshot(&browser));
    filled_and_blanked(&browser, field, "4000 1234 5678 9010");
    (before, red_pixels(&screenshot(&browser)))
}

#[test]
fn a_blanked_field_in_a_frame_the_page_turns_and_scales_shows_none_of_its_pixels() {
    let (before, after) = red_before_and_after("turned-frame.html");
    assert!(before > 0, "the field shows before");
    assert_eq!(after, 0, "the field is painted over");
}

#[test]
fn a_blanked_field_in_a_frame_the_page_zooms_shows_none_of_its_pixels() {
    let (before, after) = red_before_and_after("zoomed-frame.html");
    assert!(before > 0, "the field shows before");
    assert_eq!(after, 0, "the field is painted over");
}

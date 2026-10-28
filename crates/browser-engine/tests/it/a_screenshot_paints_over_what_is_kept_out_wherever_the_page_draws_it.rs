// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A text a page keeps out (what purlis filled and blanked, as for a login, and so what the
//! person typed, which is kept out the same way), drawn by the page somewhere other than in its
//! field, is painted over in every screenshot the agent gets (#2303): drawn into a canvas and an
//! image made from it, shown in a copy the page hides from its tree, and drawn by the page's
//! style.
//!
//! These need a real Chrome and the right to start it and to listen on a loopback port, which a
//! sandboxed agent does not have. They run where `PURLIS_TEST_ENGINE` names a Chrome
//! executable, and on CI against the runner's own Chrome. Anywhere else each test says why it
//! did not run and passes.

#![cfg(unix)]

use crate::common;

use common::drive::{filled_and_blanked, find, open, red_pixels, ref_on, screenshot};
use common::site::started;
use purlis_browser_engine::drive::{Browser, Ref};

/// The reference of the field labelled "Say something" on `path`, opened.
fn open_at_the_field(browser: &Browser, path: &str) -> Ref {
    open(browser, path);
    ref_on(&find(browser, "Say something"), "Say something")
}

#[test]
fn a_kept_text_drawn_into_a_canvas_and_an_image_is_painted_over() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    let field = open_at_the_field(&browser, "echo-drawn.html");
    assert!(
        red_pixels(&screenshot(&browser)) > 1000,
        "the canvas and its image show before"
    );

    filled_and_blanked(&browser, field, "qzwvxk");

    assert_eq!(
        red_pixels(&screenshot(&browser)),
        0,
        "the canvas and the image are painted over"
    );
}

#[test]
fn a_kept_text_shown_where_the_tree_does_not_show_it_is_painted_over() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    let field = open_at_the_field(&browser, "echo-hidden.html");
    assert!(
        red_pixels(&screenshot(&browser)) > 200,
        "the copies show before"
    );

    filled_and_blanked(&browser, field, "qzwvxk");

    assert_eq!(
        red_pixels(&screenshot(&browser)),
        0,
        "the hidden copy and the text the style draws are painted over"
    );
}

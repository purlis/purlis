// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A text a page keeps out (what purlis filled and blanked, as for a login, and so what the
//! person typed, which is kept out the same way), drawn by the page somewhere other than in its
//! field, is painted over in every screenshot the agent gets (#2303): drawn into a canvas and an
//! image made from it, copied by the canvas's shadow, shown in a copy the page hides from its
//! tree, and drawn by the page's style.
//!
//! These need a real Chrome and the right to start it and to listen on a loopback port, which a
//! sandboxed agent does not have. They run where `PURLIS_TEST_ENGINE` names a Chrome
//! executable, and on CI against the runner's own Chrome. Anywhere else each test says why it
//! did not run and passes.

#![cfg(unix)]

use crate::common;

use common::drive::{call, filled_and_blanked, find, open, red_pixels, ref_on, screenshot};
use common::site::started;
use purlis_browser_engine::drive::{Browser, Call, Content, PageId, Ref};

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

/// A canvas drawn through a `drop-shadow(…)` filter shows a copy of what it draws outside its
/// box: the copy is painted over with it.
#[test]
fn a_kept_text_drawn_into_a_canvas_with_a_drop_shadow_is_painted_over_with_its_shadow() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    let field = open_at_the_field(&browser, "echo-drawn-shadowed.html");
    assert!(
        red_pixels(&screenshot(&browser)) > 400,
        "the canvas and its shadow show before"
    );

    filled_and_blanked(&browser, field, "qzwvxk");

    assert_eq!(
        red_pixels(&screenshot(&browser)),
        0,
        "the canvas and its shadow are painted over"
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

/// The browser's page titled `title`, once it shows, within ten seconds.
fn page_titled(browser: &Browser, title: &str) -> PageId {
    let start = std::time::Instant::now();
    loop {
        let Content::Pages(pages) = call(browser, Call::Pages).content else {
            panic!("its pages");
        };
        if let Some(page) = pages.into_iter().find(|page| page.title == title) {
            return page.id;
        }
        assert!(
            start.elapsed() < std::time::Duration::from_secs(10),
            "no page titled {title}"
        );
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

/// A page opened with `noopener` on another site, which cannot reach the page that opened it,
/// is still that page's: once that page keeps a text, the opened page's canvas and image are
/// painted over too.
#[test]
fn a_page_opened_with_noopener_from_a_page_that_keeps_a_text_is_painted_over() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    open(&browser, "opens-apart.html");
    let opener = page_titled(&browser, "Opens a page apart");
    let apart = ref_on(&find(&browser, "Open apart"), "Open apart");
    call(&browser, Call::Click { target: apart });
    let opened = page_titled(&browser, "Drawn apart");
    call(
        &browser,
        Call::SwitchPage {
            page: opened.clone(),
        },
    );
    assert!(
        red_pixels(&screenshot(&browser)) > 1000,
        "the canvas and its image show before"
    );

    call(&browser, Call::SwitchPage { page: opener });
    let field = ref_on(&find(&browser, "Say something"), "Say something");
    filled_and_blanked(&browser, field, "qzwvxk");
    call(&browser, Call::SwitchPage { page: opened });

    assert_eq!(
        red_pixels(&screenshot(&browser)),
        0,
        "the opened page's canvas and image are painted over"
    );
}

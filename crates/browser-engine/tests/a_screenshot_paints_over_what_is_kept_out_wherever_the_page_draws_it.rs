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

mod common;

use std::sync::atomic::{AtomicU32, Ordering};

use common::site::started;
use purlis_browser_engine::WebAddress;
use purlis_browser_engine::drive::{Answer, Browser, Call, CallId, Content, Field, Ref};

fn next_id() -> CallId {
    static NEXT: AtomicU32 = AtomicU32::new(1);
    CallId::new(format!("call-{}", NEXT.fetch_add(1, Ordering::Relaxed))).unwrap()
}

fn call(browser: &Browser, call: Call) -> Answer {
    let what = format!("{call:?}");
    browser
        .call(&next_id(), call)
        .unwrap_or_else(|why| panic!("{what}: {why}"))
}

/// The reference of the field labelled "Say something" on `path`, opened.
fn open_at_the_field(browser: &Browser, path: &str) -> Ref {
    let address = WebAddress::parse(&format!("http://fixture.test/{path}")).unwrap();
    call(browser, Call::Open { address });
    let found = match call(
        browser,
        Call::Find {
            page: None,
            text: Some("Say something".into()),
            role: None,
            within: None,
        },
    )
    .content
    {
        Content::Text(text) => text,
        other => panic!("a find is text: {other:?}"),
    };
    let line = found
        .lines()
        .find(|l| l.contains("Say something") && l.contains('['))
        .unwrap_or_else(|| panic!("no field in\n{found}"));
    let start = line.find('[').unwrap() + 1;
    let end = line[start..].find(']').unwrap() + start;
    Ref::new(&line[start..end])
}

/// `text` is typed into `field`, and the field blanked, as a login fills one: what it holds is
/// kept out from then on.
fn filled_and_blanked(browser: &Browser, field: Ref, text: &str) {
    call(
        browser,
        Call::Type {
            page: None,
            fields: vec![Field {
                into: Some(field.clone()),
                text: text.into(),
                replace: true,
            }],
        },
    );
    browser.blank(&field).unwrap();
}

/// How many pixels of a PNG are the fixture's pure red.
fn red_pixels(png_bytes: &[u8]) -> usize {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(png_bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut buf).unwrap();
    let channels = info.color_type.samples();
    buf[..info.line_size * info.height as usize]
        .chunks(channels)
        .filter(|p| p.len() >= 3 && p[0] > 200 && p[1] < 60 && p[2] < 60)
        .count()
}

fn screenshot(browser: &Browser) -> Vec<u8> {
    match call(browser, Call::Screenshot { page: None }).content {
        Content::Image(png) => png,
        other => panic!("a screenshot is an image: {other:?}"),
    }
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

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Driving the fixture site through a real engine with the agent's calls: each call with an id
//! of its own, what it answers, and how much of a screenshot is the fixture's red.

use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use purlis_browser_engine::WebAddress;
use purlis_browser_engine::drive::{Answer, Browser, Call, CallId, Content, Ref};

pub fn next_id() -> CallId {
    static NEXT: AtomicU32 = AtomicU32::new(1);
    CallId::new(format!("call-{}", NEXT.fetch_add(1, Ordering::Relaxed))).unwrap()
}

pub fn call(browser: &Browser, call: Call) -> Answer {
    let what = format!("{call:?}");
    browser
        .call(&next_id(), call)
        .unwrap_or_else(|why| panic!("{what}: {why}"))
}

pub fn open(browser: &Browser, path: &str) -> Answer {
    let address = WebAddress::parse(&format!("http://fixture.test/{path}")).unwrap();
    call(browser, Call::Open { address })
}

pub fn text(answer: &Answer) -> String {
    match &answer.content {
        Content::Text(text) => text.clone(),
        other => panic!("not text: {other:?}"),
    }
}

pub fn snapshot(browser: &Browser) -> String {
    text(&call(
        browser,
        Call::Snapshot {
            page: None,
            within: None,
            from: 1,
            lines: None,
        },
    ))
}

pub fn find(browser: &Browser, words: &str) -> String {
    text(&call(
        browser,
        Call::Find {
            page: None,
            text: Some(words.into()),
            role: None,
            within: None,
        },
    ))
}

/// The reference on the first line of `text` that holds `words`.
pub fn ref_on(text: &str, words: &str) -> Ref {
    let line = text
        .lines()
        .find(|l| l.contains(words) && l.contains('['))
        .unwrap_or_else(|| panic!("no element with {words:?} in\n{text}"));
    let start = line.find('[').unwrap() + 1;
    let end = line[start..].find(']').unwrap() + start;
    Ref::new(&line[start..end])
}

pub fn sees(browser: &Browser, words: &str) -> bool {
    let answer = call(
        browser,
        Call::WaitForText {
            page: None,
            text: words.into(),
            gone: false,
            within: Duration::from_secs(10),
        },
    );
    answer.content == Content::Seen(true)
}

/// How many pixels of a PNG are the fixture's pure red.
pub fn red_pixels(png_bytes: &[u8]) -> usize {
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

pub fn screenshot(browser: &Browser) -> Vec<u8> {
    match call(browser, Call::Screenshot { page: None }).content {
        Content::Image(png) => png,
        other => panic!("a screenshot is an image: {other:?}"),
    }
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A real engine's frames (#2012): nothing is painted for anyone until the browser is shown,
//! each frame shown is a JPEG of the current page, a paced browser is sent fewer, and none
//! comes once it is hidden.
//!
//! These need a real Chrome and the right to start it and to listen on a loopback port (the
//! site's stand-in reach proxy), which a sandboxed agent does not have. They run where
//! `PURLIS_TEST_ENGINE` names a Chrome executable, and on CI against the runner's own Chrome.
//! Anywhere else each test says why it did not run and passes.

#![cfg(unix)]

mod common;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::site::started;
use purlis_browser_engine::WebAddress;
use purlis_browser_engine::drive::{Call, CallId};

type Frames = Arc<Mutex<Vec<Vec<u8>>>>;

fn collecting(frames: &Frames) -> Arc<dyn Fn(Vec<u8>) + Send + Sync> {
    let frames = frames.clone();
    Arc::new(move |image| frames.lock().unwrap().push(image))
}

#[test]
fn a_shown_engine_paints_jpeg_frames_and_a_hidden_one_paints_none() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    browser
        .call(
            &CallId::new("open").unwrap(),
            Call::Open {
                address: WebAddress::parse("http://fixture.test/long.html").unwrap(),
            },
        )
        .unwrap();
    let frames: Frames = Arc::default();

    std::thread::sleep(Duration::from_millis(500));
    browser.show(collecting(&frames));
    let start = Instant::now();
    while frames.lock().unwrap().is_empty() {
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "no frame came once shown"
        );
        // Scrolling changes what the page shows, so the engine paints.
        let _ = browser.call(
            &CallId::new(format!("scroll-{}", start.elapsed().as_millis())).unwrap(),
            Call::Scroll {
                page: None,
                at: None,
                x: 0.0,
                y: 200.0,
            },
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    let first = frames.lock().unwrap()[0].clone();
    assert_eq!(&first[..2], &[0xff, 0xd8], "a frame is a JPEG");

    browser.hide();
    std::thread::sleep(Duration::from_millis(500));
    let before = frames.lock().unwrap().len();
    for n in 0..5 {
        let _ = browser.call(
            &CallId::new(format!("after-{n}")).unwrap(),
            Call::Scroll {
                page: None,
                at: None,
                x: 0.0,
                y: 200.0,
            },
        );
    }
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(
        frames.lock().unwrap().len(),
        before,
        "a hidden browser paints nothing"
    );
}

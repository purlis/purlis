// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A real engine's DevTools (#2028): the front end the engine ships opens for the person who
//! drives the browser, speaks to the page through purlis's stand-in socket over the one pipe
//! (it asks the page for its document), is watched as JPEG frames of its own, never becomes one
//! of the browser's pages, and closes as the browser is handed back.
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
use purlis_browser_engine::drive::{Call, CallId, Content, InspectEnded, Painted};

fn wait_for(what: &str, within: Duration, mut done: impl FnMut() -> bool) {
    let start = Instant::now();
    while !done() {
        assert!(start.elapsed() < within, "{what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn the_engines_own_devtools_open_for_the_person_and_reach_the_page_through_purlis() {
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
    browser.take_over();
    let ends: Arc<Mutex<Vec<InspectEnded>>> = Arc::default();
    let told = Arc::clone(&ends);
    browser
        .inspect(
            1000,
            700,
            Box::new(move |why| told.lock().unwrap().push(why)),
        )
        .expect("DevTools open for the person driving the browser");
    let frames: Arc<Mutex<Vec<Painted>>> = Arc::default();
    let seen = Arc::clone(&frames);
    browser.show_inspector(Arc::new(move |painted| seen.lock().unwrap().push(painted)));

    // The front end speaks to the page through the stand-in: it asks for its document.
    wait_for(
        "the front end asked the page for its document",
        Duration::from_secs(30),
        || {
            browser
                .inspector_heard()
                .iter()
                .any(|method| method == "DOM.getDocument")
        },
    );
    wait_for("DevTools were painted", Duration::from_secs(20), || {
        !frames.lock().unwrap().is_empty()
    });
    let first = frames.lock().unwrap()[0].clone();
    assert_eq!(
        &first.jpeg[..2],
        &[0xff, 0xd8],
        "a frame of DevTools is a JPEG"
    );
    assert_eq!(first.aim.page.as_str(), "devtools");
    assert!(
        ends.lock().unwrap().is_empty(),
        "{:?}",
        ends.lock().unwrap()
    );

    browser.hand_back();
    assert!(!browser.inspecting());
    assert_eq!(*ends.lock().unwrap(), vec![InspectEnded::Closed]);
    // DevTools were never one of the browser's pages.
    let listed = browser
        .call(&CallId::new("pages").unwrap(), Call::Pages)
        .unwrap();
    let Content::Pages(pages) = listed.content else {
        panic!("the pages are listed");
    };
    assert_eq!(pages.len(), 1, "{pages:?}");
    assert!(
        pages[0].address.starts_with("http://fixture.test/"),
        "{pages:?}"
    );
}

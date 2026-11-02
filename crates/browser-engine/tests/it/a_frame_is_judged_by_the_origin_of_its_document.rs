// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What is typed into a frame is judged by the origin of the frame's document, by the web's
//! rules for its address: a srcdoc frame has its page's origin, unless it is sandboxed, a blob
//! frame the origin of the document that made it, and a sandboxed or data frame none, so
//! nothing is typed into it. A page's own document is judged by its origin the same way: a
//! blank page another page wrote has the writer's origin, and a blob page its maker's.
//!
//! These need a real Chrome and a loopback port for the fixture site, which a sandboxed agent
//! does not have: they run where `PURLIS_TEST_ENGINE` names a Chrome executable, and on CI.

#![cfg(unix)]

use crate::common;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::drive::{call, find, next_id, open, ref_on, snapshot};
use common::site::started_judged;
use purlis_browser_engine::WebAddress;
use purlis_browser_engine::drive::{
    Answer, Browser, Call, Field, Gate, Judge, Outbound, Refusal, Why,
};

/// A judge that keeps every address it is told, and asks about other.test unless the page
/// only reads: other.test's page loads in its frame and makes the blob frame, and what is typed
/// there is asked about.
fn judge() -> (Judge, Arc<Mutex<Vec<String>>>) {
    let told = Arc::new(Mutex::new(Vec::new()));
    let keep = Arc::clone(&told);
    let judge: Judge = Arc::new(move |out: &Outbound| {
        keep.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(out.address.clone());
        if out.address.starts_with("http://other.test/") && out.why != Why::Reads {
            Gate::Ask {
                site: "other.test:80".to_owned(),
            }
        } else {
            Gate::Go
        }
    });
    (judge, told)
}

/// Waits for the field labelled `label` to be found, and types into it.
fn typed_into(browser: &Browser, label: &str) -> Result<Answer, Refusal> {
    let start = Instant::now();
    let found = loop {
        let found = find(browser, label);
        if found.contains('[') {
            break found;
        }
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "{label} never showed: {found}"
        );
        std::thread::sleep(Duration::from_millis(100));
    };
    browser.call(
        &next_id(),
        Call::Type {
            page: None,
            fields: vec![Field {
                into: Some(ref_on(&found, label)),
                text: "typed".into(),
                replace: true,
            }],
        },
    )
}

#[test]
fn each_frame_is_judged_by_its_documents_origin() {
    let (judge, told) = judge();
    let Some((browser, _data, _site)) = started_judged(judge) else {
        return;
    };
    open(&browser, "frame-origins.html");

    let srcdoc = typed_into(&browser, "Srcdoc field").expect("the srcdoc field takes text");
    assert!(srcdoc.asked.is_empty(), "the page's own origin: {srcdoc:?}");

    assert_eq!(
        typed_into(&browser, "Sandboxed field").err(),
        Some(Refusal::FrameOpaque),
        "a sandboxed document has no web origin"
    );

    assert_eq!(
        typed_into(&browser, "Script-free field").err(),
        Some(Refusal::FrameOpaque),
        "a document sandboxed with no scripts has no web origin"
    );

    let blob = typed_into(&browser, "Blob field").expect("the blob field is judged");
    assert_eq!(
        blob.asked,
        ["other.test:80"],
        "the origin that made it; the judge was told {:?}",
        told.lock().unwrap()
    );

    assert_eq!(
        typed_into(&browser, "Data field").err(),
        Some(Refusal::FrameOpaque),
        "no web origin"
    );
}

/// Opens other.test's page that writes pages, and clicks `button` on it: the page it opens is
/// the current one.
fn opened_by_other_test(browser: &Browser, button: &str) {
    let address = WebAddress::parse("http://other.test/opens-pages.html").unwrap();
    call(browser, Call::Open { address });
    let target = ref_on(&snapshot(browser), button);
    let clicked = call(browser, Call::Click { target });
    assert_eq!(clicked.opened.len(), 1, "{clicked:?}");
    assert!(clicked.opened[0].current, "{clicked:?}");
}

#[test]
fn a_blank_page_another_page_wrote_is_judged_by_the_writers_origin() {
    let (judge, told) = judge();
    let Some((browser, _data, _site)) = started_judged(judge) else {
        return;
    };
    opened_by_other_test(&browser, "Open a blank page");
    let written = typed_into(&browser, "Written page field").expect("the written page is judged");
    assert_eq!(
        written.asked,
        ["other.test:80"],
        "the origin that wrote it; the judge was told {:?}",
        told.lock().unwrap()
    );
}

#[test]
fn a_blob_page_is_judged_by_the_origin_that_made_it() {
    let (judge, told) = judge();
    let Some((browser, _data, _site)) = started_judged(judge) else {
        return;
    };
    opened_by_other_test(&browser, "Open a blob page");
    let made = typed_into(&browser, "Blob page field").expect("the blob page is judged");
    assert_eq!(
        made.asked,
        ["other.test:80"],
        "the origin that made it; the judge was told {:?}",
        told.lock().unwrap()
    );
}

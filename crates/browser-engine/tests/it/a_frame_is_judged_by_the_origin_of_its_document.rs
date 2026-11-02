// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What is typed into a frame is judged by the origin of the frame's document, by the web's
//! rules for its address: a srcdoc frame has its page's origin, unless it is sandboxed, a blob
//! frame the origin of the document that made it, and a sandboxed or data frame none, so
//! nothing is typed into it.
//!
//! These need a real Chrome and a loopback port for the fixture site, which a sandboxed agent
//! does not have: they run where `PURLIS_TEST_ENGINE` names a Chrome executable, and on CI.

#![cfg(unix)]

use crate::common;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::drive::{find, next_id, open, ref_on};
use common::site::started_judged;
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

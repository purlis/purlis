// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What is typed into a frame is judged by the origin of the frame's document, by the web's
//! rules for its address: a srcdoc frame has its page's origin, unless it is sandboxed, a blob
//! frame the origin of the document that made it, and a sandboxed or data frame none, so
//! nothing is typed into it. A page's own document is judged by its origin the same way: a
//! blank page another page wrote has the writer's origin. A blob page another page opens does
//! not stay open. Keys on a fresh blank tab are refused as on a blank tab. A frame its page
//! sends to a blank document is judged by the page's origin once that document is known.
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
    Answer, Browser, Call, Content, Field, Gate, Judge, Outbound, Refusal, Why,
};

/// A judge that keeps every address it is told, and asks about other.test only for what is
/// sent there: other.test's pages load, in a frame or opened by the agent, and what is typed
/// into a document of its origin is asked about.
fn judge() -> (Judge, Arc<Mutex<Vec<String>>>) {
    let told = Arc::new(Mutex::new(Vec::new()));
    let keep = Arc::clone(&told);
    let judge: Judge = Arc::new(move |out: &Outbound| {
        keep.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(out.address.clone());
        if out.address.starts_with("http://other.test/") && out.why == Why::Sends {
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

#[test]
fn a_frame_its_page_sends_to_a_blank_document_is_judged_by_the_pages_origin() {
    let (judge, told) = judge();
    let Some((browser, _data, _site)) = started_judged(judge) else {
        return;
    };
    open(&browser, "frame-sent-to-blank.html");
    let target = ref_on(&snapshot(&browser), "Send the frame away");
    call(&browser, Call::Click { target });
    // Until the frame's new document is known, what would reach it is refused.
    let start = Instant::now();
    let typed = loop {
        let typed = typed_into(&browser, "Blank document field");
        if !matches!(typed, Err(Refusal::FrameUnread)) || start.elapsed() > Duration::from_secs(10)
        {
            break typed;
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    let typed = typed.expect("the blank document is judged once it is known");
    assert!(
        typed.asked.is_empty(),
        "the page's own origin, not the one of the document the frame showed before: {typed:?}; \
         the judge was told {:?}",
        told.lock().unwrap()
    );
}

/// Opens other.test's page that writes pages, and clicks `button` on it.
fn clicked_on_other_test(browser: &Browser, button: &str) -> Answer {
    let address = WebAddress::parse("http://other.test/opens-pages.html").unwrap();
    call(browser, Call::Open { address });
    let target = ref_on(&snapshot(browser), button);
    call(browser, Call::Click { target })
}

#[test]
fn a_blank_page_another_page_wrote_is_judged_by_the_writers_origin() {
    let (judge, told) = judge();
    let Some((browser, _data, _site)) = started_judged(judge) else {
        return;
    };
    let clicked = clicked_on_other_test(&browser, "Open a blank page");
    assert_eq!(clicked.opened.len(), 1, "{clicked:?}");
    assert!(clicked.opened[0].current, "{clicked:?}");
    let written = typed_into(&browser, "Written page field").expect("the written page is judged");
    assert_eq!(
        written.asked,
        ["other.test:80"],
        "the origin that wrote it; the judge was told {:?}",
        told.lock().unwrap()
    );
}

#[test]
fn a_blob_page_another_page_opens_does_not_stay_open() {
    let (judge, _) = judge();
    let Some((browser, _data, _site)) = started_judged(judge) else {
        return;
    };
    let clicked = clicked_on_other_test(&browser, "Open a blob page");
    assert!(clicked.opened.is_empty(), "{clicked:?}");
    std::thread::sleep(Duration::from_secs(1));
    let Content::Pages(listed) = call(&browser, Call::Pages).content else {
        panic!("no pages listed");
    };
    assert!(
        listed.iter().all(|page| !page.address.starts_with("blob:")),
        "{listed:?}"
    );
    let current = listed
        .iter()
        .find(|page| page.current)
        .expect("a current page");
    assert_eq!(
        current.address, "http://other.test/opens-pages.html",
        "{listed:?}"
    );
}

#[test]
fn keys_on_a_fresh_blank_tab_are_refused_as_a_blank_tab() {
    let (judge, _) = judge();
    let Some((browser, _data, _site)) = started_judged(judge) else {
        return;
    };
    let start = Instant::now();
    let refused = loop {
        let refused = browser
            .call(
                &next_id(),
                Call::Press {
                    page: None,
                    keys: vec!["a".into()],
                },
            )
            .err();
        // Until the blank document's own context has said its origin, it is not known.
        if refused != Some(Refusal::FrameUnread) || start.elapsed() > Duration::from_secs(10) {
            break refused;
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    assert_eq!(refused, Some(Refusal::BlankTab));
}

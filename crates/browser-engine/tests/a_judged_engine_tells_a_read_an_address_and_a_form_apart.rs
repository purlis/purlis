// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Where a browser may go (#2018), on a real engine: every request a page is about to send is
//! held until the browser's judge has heard what it is. A link the page offered is a read; an
//! address the agent writes is the agent's; a form submitted, by GET or by POST, sends. What the
//! judge asks about is not sent, and the act's answer names the site.
//!
//! These need a real Chrome and a loopback port for the fixture site, which a sandboxed agent
//! does not have: they run where `PURLIS_TEST_ENGINE` names a Chrome executable, and on CI.

#![cfg(unix)]

mod common;

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use common::site::started_judged;
use purlis_browser_engine::WebAddress;
use purlis_browser_engine::drive::{
    Answer, Browser, Call, CallId, Content, Field, Gate, Judge, Outbound, Ref, Why,
};

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

/// What a judge was told: each address and why it was sent.
type Told = Arc<Mutex<Vec<(String, Why)>>>;

/// A judge that keeps what it is told, and asks about other.test unless the page only reads.
fn judge() -> (Judge, Told) {
    let told = Arc::new(Mutex::new(Vec::new()));
    let keep = Arc::clone(&told);
    let judge: Judge = Arc::new(move |out: &Outbound| {
        keep.lock().unwrap().push((out.address.clone(), out.why));
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

fn snapshot(browser: &Browser) -> String {
    match call(
        browser,
        Call::Snapshot {
            page: None,
            within: None,
            from: 1,
            lines: None,
        },
    )
    .content
    {
        Content::Text(text) => text,
        other => panic!("not text: {other:?}"),
    }
}

/// The reference on the first line of `text` that holds `words`.
fn ref_on(text: &str, words: &str) -> Ref {
    let line = text
        .lines()
        .find(|l| l.contains(words) && l.contains('['))
        .unwrap_or_else(|| panic!("no element with {words:?} in\n{text}"));
    let start = line.find('[').unwrap() + 1;
    let end = line[start..].find(']').unwrap() + start;
    Ref::new(&line[start..end])
}

fn open_the_way_out(browser: &Browser) {
    let address = WebAddress::parse("http://fixture.test/leaving.html").unwrap();
    call(browser, Call::Open { address });
}

fn told_about(told: &Mutex<Vec<(String, Why)>>, address: &str) -> Vec<Why> {
    told.lock()
        .unwrap()
        .iter()
        .filter(|(at, _)| at.starts_with(address))
        .map(|(_, why)| *why)
        .collect()
}

#[test]
fn a_link_the_page_offered_is_a_read_and_loads() {
    let (judge, told) = judge();
    let Some((browser, _data, _site)) = started_judged(judge) else {
        return;
    };
    open_the_way_out(&browser);
    assert_eq!(
        told_about(&told, "http://fixture.test/leaving.html"),
        [Why::Addressed]
    );
    let link = ref_on(&snapshot(&browser), "Read the other site");
    let answer = call(&browser, Call::Click { target: link });
    assert!(answer.asked.is_empty(), "{answer:?}");
    assert_eq!(
        answer.page.unwrap().address,
        "http://other.test/deep.html",
        "the link loaded"
    );
    assert_eq!(
        told_about(&told, "http://other.test/deep.html"),
        [Why::Reads]
    );
}

#[test]
fn an_address_the_agent_writes_for_the_site_is_asked_about_and_not_loaded() {
    let (judge, told) = judge();
    let Some((browser, _data, _site)) = started_judged(judge) else {
        return;
    };
    open_the_way_out(&browser);
    let answer = call(
        &browser,
        Call::Navigate {
            page: None,
            address: WebAddress::parse("http://other.test/deep.html?said=1").unwrap(),
        },
    );
    assert_eq!(answer.asked, ["other.test:80"]);
    assert!(
        !snapshot(&browser).contains("Deep button"),
        "the other site's page did not load"
    );
    assert_eq!(
        told_about(&told, "http://other.test/deep.html"),
        [Why::Addressed]
    );
}

#[test]
fn a_form_submitted_to_the_site_sends_whether_by_get_or_by_post() {
    let (judge, told) = judge();
    let Some((browser, _data, _site)) = started_judged(judge) else {
        return;
    };
    for (field, button) in [
        ("Search", "Search the other site"),
        ("Note", "Post to the other site"),
    ] {
        open_the_way_out(&browser);
        let page = snapshot(&browser);
        call(
            &browser,
            Call::Type {
                page: None,
                fields: vec![Field {
                    into: Some(ref_on(&page, field)),
                    text: "what was typed".into(),
                    replace: true,
                }],
            },
        );
        let answer = call(
            &browser,
            Call::Click {
                target: ref_on(&page, button),
            },
        );
        assert_eq!(answer.asked, ["other.test:80"], "{button}: {answer:?}");
        assert!(
            !snapshot(&browser).contains("Deep button"),
            "{button}: nothing was sent, so the other site's page did not load"
        );
    }
    let sent = told_about(&told, "http://other.test/deep.html");
    assert_eq!(sent, [Why::Sends, Why::Sends], "{sent:?}");
}

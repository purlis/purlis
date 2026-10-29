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

use crate::common;

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use common::site::{Site, started_judged};
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

/// One judged engine at a time in this binary: each test waits on its engine's answers, and
/// several engines starting at once on a busy runner make those answers late.
fn one_engine_at_a_time() -> std::sync::MutexGuard<'static, ()> {
    static ONE: Mutex<()> = Mutex::new(());
    ONE.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// What a judge was told: each address and why it was sent.
type Told = Arc<Mutex<Vec<(String, Why)>>>;

/// A judge that keeps what it is told, and asks about other.test unless the page only reads.
fn judge() -> (Judge, Told) {
    let told = Arc::new(Mutex::new(Vec::new()));
    let keep = Arc::clone(&told);
    let judge: Judge = Arc::new(move |out: &Outbound| {
        keep.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push((out.address.clone(), out.why));
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

/// Opens the page of ways out; a failure says what the judge was told by then.
fn open_the_way_out(browser: &Browser, told: &Told) {
    let address = WebAddress::parse("http://fixture.test/leaving.html").unwrap();
    if let Err(why) = browser.call(&next_id(), Call::Open { address }) {
        let heard = format!("{:?}", told.lock().unwrap());
        panic!("the page of ways out did not open: {why}; the judge was told {heard}");
    }
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
    let _one = one_engine_at_a_time();
    let Some((browser, _data, _site)) = started_judged(judge) else {
        return;
    };
    open_the_way_out(&browser, &told);
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
    let _one = one_engine_at_a_time();
    let Some((browser, _data, site)) = started_judged(judge) else {
        return;
    };
    open_the_way_out(&browser, &told);
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
    assert!(
        !site.heard_from("other.test"),
        "the other site was sent nothing"
    );
    assert_eq!(
        told_about(&told, "http://other.test/deep.html"),
        [Why::Addressed]
    );
}

/// A form on the page of ways out, typed into and submitted: it sends, and nothing reaches the
/// other site.
fn a_form_sends(browser: &Browser, told: &Told, site: &Site, field: &str, button: &str) {
    open_the_way_out(browser, told);
    let page = snapshot(browser);
    call(
        browser,
        Call::Type {
            page: None,
            fields: vec![Field {
                into: Some(ref_on(&page, field)),
                text: "what was typed".into(),
                replace: true,
            }],
        },
    );
    let before = told_about(told, "http://other.test/deep.html").len();
    let answer = call(
        browser,
        Call::Click {
            target: ref_on(&page, button),
        },
    );
    assert_eq!(answer.asked, ["other.test:80"], "{button}: {answer:?}");
    assert!(
        !snapshot(browser).contains("Deep button"),
        "{button}: nothing was sent, so the other site's page did not load"
    );
    assert!(
        !site.heard_from("other.test"),
        "{button}: the other site was sent nothing"
    );
    let sent = told_about(told, "http://other.test/deep.html");
    assert_eq!(sent[before..], [Why::Sends], "{button}: {sent:?}");
}

/// After a form it held back, the same browser opens its next page (#2442).
fn the_next_page_opens(browser: &Browser, told: &Told, after: &str) {
    let address = WebAddress::parse("http://fixture.test/counter.html").unwrap();
    let answer = browser
        .call(&next_id(), Call::Open { address })
        .unwrap_or_else(|why| {
            let heard = format!("{:?}", told.lock().unwrap());
            panic!("after {after}, the next page did not open: {why}; the judge was told {heard}")
        });
    assert_eq!(
        answer.page.map(|page| page.address).as_deref(),
        Some("http://fixture.test/counter.html"),
        "after {after}"
    );
    assert!(
        snapshot(browser).contains("Add one"),
        "after {after}, the next page loaded"
    );
}

/// Both forms in one browser: each is held back, and the browser opens its next page after
/// each.
#[test]
fn a_form_submitted_to_the_site_sends_and_the_browser_opens_its_next_page() {
    let (judge, told) = judge();
    let _one = one_engine_at_a_time();
    let Some((browser, _data, site)) = started_judged(judge) else {
        return;
    };
    let by_get = "button \"Search the other site\"";
    a_form_sends(&browser, &told, &site, "textbox \"Search\"", by_get);
    the_next_page_opens(&browser, &told, by_get);
    let by_post = "button \"Post to the other site\"";
    a_form_sends(&browser, &told, &site, "textbox \"Note\"", by_post);
    the_next_page_opens(&browser, &told, by_post);
}

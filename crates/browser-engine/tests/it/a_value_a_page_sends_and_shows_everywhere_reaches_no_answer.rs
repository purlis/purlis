// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A filled value reaches no answer, whatever the page does with it** (#5310), through a real
//! engine: a page that sends the value in requests that stay open and writes it into its address,
//! its title, its console, a download's name and its text, in each form a page commonly writes
//! it. Every answer and every refusal is read whole, by its `Debug`, so a field added to either
//! is read too.
//!
//! These need a real Chrome and a loopback port, which a sandboxed agent does not have: they run
//! where `PURLIS_TEST_ENGINE` names a Chrome executable, and on CI.

#![cfg(unix)]

use crate::common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use base64::Engine as _;
use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD};
use common::drive::{next_id, ref_on, snapshot};
use common::site::started_limited;
use purlis_browser_engine::WebAddress;
use purlis_browser_engine::drive::{
    Answer, Browser, Call, CallId, Content, FillHold, Limits, Ref, Refusal, SecretFill, Settled,
};

/// A space, and marks that each form writes its own way.
const VALUE: &str = "fixture key+5310/!x";

/// Bytes either side of the value, as a page's base64 of a larger text has them.
const AROUND: [&str; 3] = ["q=", "qu=", "que="];

struct Holds;

impl FillHold for Holds {
    fn hold(&self) -> Result<(), String> {
        Ok(())
    }

    fn released(&self) {}
}

/// Every form the value may be written in, each named: a failing test names the form, never the
/// value's own text in another form than it found.
fn forms() -> Vec<(&'static str, String)> {
    let component = VALUE
        .replace('%', "%25")
        .replace(' ', "%20")
        .replace('+', "%2B")
        .replace('/', "%2F");
    let form = component.replace("%20", "+").replace('!', "%21");
    vec![
        ("plain", VALUE.to_owned()),
        ("as a file name", VALUE.replace('/', "_")),
        ("percent-encoded", component.clone()),
        ("percent-encoded in lower case", component.to_lowercase()),
        ("form-encoded", form),
        ("encoded twice", component.replace('%', "%25")),
        ("base64", STANDARD.encode(VALUE)),
        ("base64 with no padding", STANDARD_NO_PAD.encode(VALUE)),
        ("url-safe base64", URL_SAFE.encode(VALUE)),
        ("url-safe base64 with no padding", URL_SAFE_NO_PAD.encode(VALUE)),
        // What of a larger text's base64 only the value decides, at each of its three alignments.
        ("base64 inside a larger text", inside(AROUND[0])),
        ("base64 inside a larger text, shifted", inside(AROUND[1])),
        ("base64 inside a larger text, shifted twice", inside(AROUND[2])),
    ]
}

fn inside(before: &str) -> String {
    let start = (before.len() * 8).div_ceil(6);
    let end = ((before.len() + VALUE.len()) * 8) / 6;
    let whole = STANDARD_NO_PAD.encode(format!("{before}{VALUE}&page=1"));
    whole[start..end].to_owned()
}

/// The form of the value `said` holds, if any.
fn holds(said: &str) -> Option<&'static str> {
    let said = said.to_lowercase();
    forms()
        .into_iter()
        .find(|(_, form)| said.contains(&form.to_lowercase()))
        .map(|(name, _)| name)
}

#[track_caller]
fn holds_none(of: &str, said: &str) {
    if let Some(form) = holds(said) {
        panic!("{of} holds the value, {form}");
    }
}

fn asked(browser: &Browser, id: &CallId, what: &str, call: Call) -> Result<Answer, Refusal> {
    let outcome = browser.call(id, call);
    match &outcome {
        Ok(answer) => holds_none(what, &format!("{answer:?}")),
        Err(refusal) => {
            holds_none(what, &format!("{refusal:?}"));
            holds_none(what, &refusal.to_string());
        }
    }
    outcome
}

fn read(browser: &Browser, what: &str, call: Call) -> Result<Answer, Refusal> {
    asked(browser, &next_id(), what, call)
}

/// Every read a chat has of the page, each held to hold none of the value.
fn every_read_holds_none(browser: &Browser, target: &Ref) {
    let page = None;
    for words in [VALUE, "Results", "fixture key", "Plain"] {
        let _ = read(
            browser,
            "a find",
            Call::Find {
                page: page.clone(),
                text: Some(words.to_owned()),
                role: None,
                within: None,
            },
        );
    }
    let _ = read(
        browser,
        "a snapshot",
        Call::Snapshot {
            page: page.clone(),
            within: None,
            from: 1,
            lines: Some(500),
        },
    );
    let _ = read(browser, "the changes", Call::Changes { page: page.clone() });
    let _ = read(browser, "the pages", Call::Pages);
    let _ = read(browser, "the downloads", Call::Downloads);
    let _ = read(
        browser,
        "a field's description",
        Call::DescribeField {
            target: target.clone(),
        },
    );
    let _ = read(browser, "a screenshot", Call::Screenshot { page: page.clone() });
    // What it echoes of what it was asked.
    let waited = read(
        browser,
        "a wait for the value",
        Call::WaitForText {
            page,
            text: VALUE.to_owned(),
            gone: false,
            within: Duration::from_millis(200),
        },
    );
    assert!(
        matches!(waited.map(|answer| answer.content), Ok(Content::Seen(false))),
        "the page shows the value, and a wait for it sees nothing"
    );
    for refused in [
        Call::Click {
            target: Ref::new(VALUE),
        },
        Call::SwitchPage {
            page: purlis_browser_engine::drive::PageId::new(VALUE),
        },
    ] {
        let _ = read(browser, "a refusal", refused);
    }
    for form in forms() {
        let _ = read(
            browser,
            "a click on a reference spelled as the value",
            Call::Click {
                target: Ref::new(form.1),
            },
        );
    }
}

/// Filled into a page that sends and shows it everywhere, on a browser that does not wait long
/// for its requests.
fn filled() -> Option<(Browser, tempfile::TempDir, common::site::Site, Ref, Answer)> {
    let (browser, data, site) = started_limited(|limits| Limits {
        settle_within: Duration::from_secs(3),
        ..limits
    })?;
    browser
        .hold_downloads_in(&data.path().join("held"))
        .unwrap();
    let address = WebAddress::parse("http://fixture.test/echoes-everywhere.html").unwrap();
    browser
        .call(&next_id(), Call::Open { address })
        .unwrap_or_else(|why| panic!("opened: {why}"));
    let target = ref_on(&snapshot(&browser), "textbox \"Search for\"");
    let fill = SecretFill {
        target: target.clone(),
        origin: "http://fixture.test".to_owned(),
        key: "search-key".to_owned(),
        value: VALUE.to_owned().into(),
        multi_line: false,
    };
    let answer = browser
        .fill_secret(browser.epoch(), &fill, Arc::new(Holds))
        .map_err(|why| holds_none("the fill's refusal", &format!("{why:?}")))
        .expect("filled");
    Some((browser, data, site, target, answer))
}

#[test]
fn the_requests_a_filled_page_leaves_open_are_named_by_host_and_count() {
    let Some((browser, _data, _site, _target, answer)) = filled() else {
        return;
    };
    holds_none("the fill's answer", &format!("{answer:?}"));
    let Settled::No(waiting) = &answer.settled else {
        panic!("the page's requests stay open, so it has not settled");
    };
    let line = waiting.join("; ");
    assert!(
        line.contains("2 requests still loading: fixture.test"),
        "named by count and host: {}",
        holds(&line).unwrap_or("it holds no form of the value, and reads otherwise"),
    );
    assert!(
        !line.contains("stays-open"),
        "a pending request's path is never said"
    );
    drop(browser);
}

#[test]
fn nothing_a_chat_reads_of_a_page_holds_what_it_sent_or_showed() {
    let Some((browser, _data, _site, target, _answer)) = filled() else {
        return;
    };
    // The download's name, once the browser has told of it.
    let start = Instant::now();
    while browser.downloads().is_empty() && start.elapsed() < Duration::from_secs(10) {
        std::thread::sleep(Duration::from_millis(100));
    }
    let downloads = browser.downloads();
    assert!(!downloads.is_empty(), "the page began its download");
    holds_none("a download", &format!("{downloads:?}"));
    // Whatever the engine made of the name's marks: nothing of the value is read in it.
    assert!(
        downloads[0].name.contains("(blanked)") && !downloads[0].name.contains("fixture"),
        "a download's name holds the value's place"
    );
    every_read_holds_none(&browser, &target);
}

#[test]
fn a_retry_answers_as_the_first_did_and_holds_none_of_it() {
    let Some((browser, _data, _site, target, _answer)) = filled() else {
        return;
    };
    let id = next_id();
    let call = Call::Hover {
        target: target.clone(),
    };
    let _ = asked(&browser, &id, "an act", call.clone());
    if let Ok(again) = asked(&browser, &id, "the act's retry", call) {
        assert!(again.repeated, "a retry acts on nothing");
    }
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The hold a secret purlis fills puts on its browser (#5117; the person's ruling SF-3 and
//! decision SF-9 on #5096), through a real engine on the fixture site. Its holder is told just
//! before the value goes in; a route change that loads no document keeps it; a new document in
//! the page's main frame, or the page's close, releases it, once. Meanwhile the field gives
//! nothing to a copy, a cut or a drag, and a page that was left doesn't come back with the value.
//!
//! These need a real Chrome and the right to start it and to listen on a loopback port (the
//! site's stand-in reach proxy), which a sandboxed agent does not have. They run where
//! `PURLIS_TEST_ENGINE` names a Chrome executable, and on CI against the runner's own Chrome.
//! Anywhere else each test says why it did not run and passes.

#![cfg(unix)]

use crate::common;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::drive::{call, find, open, ref_on, sees, snapshot};
use common::site::started;
use purlis_browser_engine::drive::{Browser, Call, FillHold, Ref, SecretFill};

const VALUE: &str = "fixture-key-5117";

/// What the browser's holder was told, in order.
#[derive(Default)]
struct Told(Mutex<Vec<&'static str>>);

impl FillHold for Told {
    fn hold(&self) -> Result<(), String> {
        self.0.lock().unwrap().push("held");
        Ok(())
    }

    fn released(&self) {
        self.0.lock().unwrap().push("released");
    }
}

impl Told {
    fn said(&self) -> Vec<&'static str> {
        self.0.lock().unwrap().clone()
    }

    /// Waits up to ten seconds for it to have been told `words`.
    fn comes_to(&self, words: &[&str]) {
        let start = Instant::now();
        while self.said() != words {
            assert!(
                start.elapsed() < Duration::from_secs(10),
                "told {:?}, never {words:?}",
                self.said()
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

/// The reference of the element on line `line`, waiting for it to show.
fn ref_of(browser: &Browser, line: &str) -> Ref {
    let label = line.split('"').nth(1).unwrap_or(line);
    let start = Instant::now();
    loop {
        let found = find(browser, label);
        if found.lines().any(|l| l.contains(line) && l.contains('[')) {
            return ref_on(&found, line);
        }
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "{label} never showed: {found}"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// The dashboard's key filled, held by `told`.
fn filled(browser: &Browser, told: &Arc<Told>) {
    let key = ref_of(browser, "textbox \"API key\"");
    let fill = SecretFill {
        target: key,
        origin: "http://fixture.test".to_owned(),
        key: "dash-api-key".to_owned(),
        value: VALUE.to_owned().into(),
    };
    let filled = browser.fill_secret(
        browser.epoch(),
        &fill,
        Arc::clone(told) as Arc<dyn FillHold>,
    );
    // A refusal may carry what its page showed: only that there was one is said.
    assert!(filled.is_ok(), "the key was not filled");
    assert!(
        sees(
            browser,
            &format!("The key holds {} characters", VALUE.len())
        ),
        "the page has the value"
    );
}

fn click(browser: &Browser, line: &str) {
    let target = ref_of(browser, line);
    call(browser, Call::Click { target });
}

#[test]
fn a_route_change_keeps_the_hold_and_a_new_document_ends_it_once() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    open(&browser, "hold.html");
    let told = Arc::new(Told::default());
    filled(&browser, &told);
    assert_eq!(told.said(), ["held"]);

    click(&browser, "button \"Open the settings route\"");
    assert!(sees(&browser, "Settings route"), "the route changed");
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(told.said(), ["held"], "a route change keeps the hold");

    click(&browser, "link \"Next page\"");
    assert!(sees(&browser, "The next page"));
    told.comes_to(&["held", "released"]);
    call(&browser, Call::Back { page: None });
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(told.said(), ["held", "released"], "released once");
}

#[test]
fn a_closed_page_ends_the_hold() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    open(&browser, "hold-opener.html");
    click(&browser, "link \"Open the dashboard\"");
    let told = Arc::new(Told::default());
    filled(&browser, &told);

    click(&browser, "button \"Close this page\"");

    told.comes_to(&["held", "released"]);
}

#[test]
fn the_field_gives_nothing_to_a_copy_a_cut_or_a_drag_once_filled() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    open(&browser, "hold.html");
    click(&browser, "button \"Try to take the key\"");
    assert!(
        sees(
            &browser,
            "Tried: copy went, cut went, dragstart went, trusted copy went"
        ),
        "before a fill, the page's own copy goes: {}",
        snapshot(&browser)
    );
    let told = Arc::new(Told::default());
    filled(&browser, &told);

    click(&browser, "button \"Try to take the key\"");

    assert!(
        sees(
            &browser,
            "Tried: copy stopped, cut stopped, dragstart stopped, trusted copy stopped"
        ),
        "{}",
        snapshot(&browser)
    );
}

#[test]
fn a_page_that_was_left_comes_back_without_the_value() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    open(&browser, "hold.html");
    let told = Arc::new(Told::default());
    filled(&browser, &told);
    click(&browser, "link \"Next page\"");
    assert!(sees(&browser, "The next page"));

    call(&browser, Call::Back { page: None });

    assert!(sees(&browser, "Dashboard home"), "back on the dashboard");
    assert!(
        sees(&browser, "The key holds 0 characters"),
        "the field came back empty: {}",
        snapshot(&browser)
    );
}

/// The control for the test above: a value typed into the same field, which purlis marks
/// nothing on, comes back when the browser goes back, so it is the fill's mark that keeps a
/// filled value from coming back.
#[test]
fn a_value_typed_by_hand_comes_back_when_the_browser_goes_back() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    open(&browser, "hold.html");
    let key = ref_of(&browser, "textbox \"API key\"");
    call(
        &browser,
        Call::Type {
            page: None,
            fields: vec![purlis_browser_engine::drive::Field {
                into: Some(key),
                text: "typed-5117".into(),
                replace: true,
            }],
        },
    );
    assert!(sees(&browser, "The key holds 10 characters"));
    click(&browser, "link \"Next page\"");
    assert!(sees(&browser, "The next page"));

    call(&browser, Call::Back { page: None });

    assert!(sees(&browser, "Dashboard home"), "back on the dashboard");
    assert!(
        sees(&browser, "The key holds 10 characters"),
        "the browser restored what was typed: {}",
        snapshot(&browser)
    );
}

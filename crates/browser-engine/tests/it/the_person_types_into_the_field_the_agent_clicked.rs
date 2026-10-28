// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The person takes a browser over after the agent clicked a plain text field, and types: each
//! key reaches the field, which is blanked before the first one, so what they typed is kept out
//! of every answer after they hand the browser back (#2294).
//!
//! These need a real Chrome and the right to start it and to listen on a loopback port, which a
//! sandboxed agent does not have. They run where `PURLIS_TEST_ENGINE` names a Chrome
//! executable, and on CI against the runner's own Chrome. Anywhere else each test says why it
//! did not run and passes.

#![cfg(unix)]

use crate::common;

use common::drive::{call, find, open, ref_on, snapshot};
use common::site::started;
use purlis_browser_engine::drive::{Browser, Call, PersonInput};

const TYPED: &str = "zqvname";

/// The person presses and lets go of `key`, aimed at what the browser shows now.
fn press(browser: &Browser, key: &str, code: &str, key_code: u32) {
    let aim = browser.aim().expect("the browser shows a page");
    for down in [true, false] {
        browser
            .person_input(&PersonInput::Key {
                aim: aim.clone(),
                down,
                key: key.to_owned(),
                code: code.to_owned(),
                key_code,
                modifiers: 0,
            })
            .unwrap_or_else(|why| panic!("{key:?} did not reach the page: {why}"));
    }
}

#[test]
fn the_persons_keys_reach_the_field_the_agent_clicked_and_stay_out_of_every_answer() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    open(&browser, "login-stays.html");
    let name = ref_on(&find(&browser, "User name"), "User name");
    call(&browser, Call::Click { target: name });

    browser.take_over();
    for letter in TYPED.chars() {
        let upper = letter.to_ascii_uppercase();
        press(
            &browser,
            &letter.to_string(),
            &format!("Key{upper}"),
            u32::from(upper),
        );
    }
    // On to the password field, which counts what it holds where the page says so.
    press(&browser, "Tab", "Tab", 9);
    press(&browser, "x", "KeyX", 88);
    browser.hand_back();

    let read = snapshot(&browser);
    assert!(
        read.contains("the password field holds 1 characters"),
        "the keys reached the page: {read}"
    );
    assert!(!read.contains(TYPED), "{read}");
    // A find of the field the person typed into answers it blanked.
    let found = find(&browser, "User name");
    assert!(!found.contains(TYPED), "{found}");
}

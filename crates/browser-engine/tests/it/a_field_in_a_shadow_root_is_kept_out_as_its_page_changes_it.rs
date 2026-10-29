// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Through a real engine (#2972): the person types into a field inside an open shadow root, then
//! one inside a closed shadow root. Each field changes what was typed (every letter becomes the
//! next), and on Enter posts what it holds outside its host. What each field held is read from
//! the field itself, not from its host, so the posted text is in no later answer either.
//!
//! These need a real Chrome and the right to start it and to listen on a loopback port, which a
//! sandboxed agent does not have. They run where `PURLIS_TEST_ENGINE` names a Chrome
//! executable, and on CI against the runner's own Chrome. Anywhere else each test says why it
//! did not run and passes.

#![cfg(unix)]

use crate::common;

use common::drive::{open, sees, snapshot};
use common::site::started;
use purlis_browser_engine::drive::{Aim, Browser, PersonInput};

fn press(browser: &Browser, aim: &Aim, key: &str, code: &str, key_code: u32) {
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
            .unwrap_or_else(|why| panic!("{key}: {why}"));
    }
}

fn type_in(browser: &Browser, aim: &Aim, letters: &str) {
    for letter in letters.chars() {
        press(
            browser,
            aim,
            &letter.to_string(),
            &format!("Key{}", letter.to_ascii_uppercase()),
            u32::from(letter.to_ascii_uppercase()),
        );
    }
}

#[test]
fn what_fields_in_open_and_closed_shadow_roots_held_is_in_no_later_answer() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    open(&browser, "shadow-fields.html");
    browser.take_over();
    let aim = browser.aim().expect("a page is shown");
    // Typed "kept", the open field holds "lfqu"; typed "note", the closed one holds "opuf".
    type_in(&browser, &aim, "kept");
    press(&browser, &aim, "Enter", "Enter", 13);
    type_in(&browser, &aim, "note");
    press(&browser, &aim, "Enter", "Enter", 13);
    browser.hand_back();
    assert!(sees(&browser, "Sent: 2"), "{}", snapshot(&browser));
    let read = snapshot(&browser);
    for held in ["lfqu", "opuf"] {
        assert!(!read.contains(held), "{held} in\n{read}");
    }
}

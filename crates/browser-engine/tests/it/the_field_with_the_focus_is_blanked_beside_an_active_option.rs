// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Through a real engine (#2972): the person types into a combo box whose input names an
//! active option (`aria-activedescendant`). The input, which has the document's focus and which
//! the keys go to, is the field blanked and read, whatever the tree marks focused, and what was
//! typed is in no later answer.
//!
//! These need a real Chrome and the right to start it and to listen on a loopback port, which a
//! sandboxed agent does not have. They run where `PURLIS_TEST_ENGINE` names a Chrome
//! executable, and on CI against the runner's own Chrome. Anywhere else each test says why it
//! did not run and passes.

#![cfg(unix)]

use crate::common;

use common::drive::{open, sees, snapshot};
use common::site::started;
use purlis_browser_engine::drive::PersonInput;

#[test]
fn what_the_person_types_into_a_combo_box_is_kept_out_whatever_option_is_active() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    open(&browser, "combo-box.html");
    browser.take_over();
    let aim = browser.aim().expect("a page is shown");
    let typed = "gyumrisecret";
    for letter in typed.chars() {
        for down in [true, false] {
            browser
                .person_input(&PersonInput::Key {
                    aim: aim.clone(),
                    down,
                    key: letter.to_string(),
                    code: format!("Key{}", letter.to_ascii_uppercase()),
                    key_code: u32::from(letter.to_ascii_uppercase()),
                    modifiers: 0,
                })
                .unwrap_or_else(|why| panic!("{letter}: {why}"));
        }
    }
    browser.hand_back();
    assert!(
        sees(&browser, &format!("Typed: {}", typed.len())),
        "{}",
        snapshot(&browser)
    );
    let read = snapshot(&browser);
    assert!(!read.contains(typed), "{read}");
    assert!(
        read.contains("Yerevan"),
        "the active option is not what was blanked: {read}"
    );
}

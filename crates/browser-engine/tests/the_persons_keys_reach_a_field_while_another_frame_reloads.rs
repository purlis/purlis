// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Through a real engine (#2638): the person types into a field of a page whose advert, a frame
//! from another site, loads another document every 100 ms. Each key is aimed at what a frame
//! showed before the advert moved on, and each still reaches the field: a change in another
//! frame drops nothing.
//!
//! These need a real Chrome and the right to start it and to listen on a loopback port, which a
//! sandboxed agent does not have. They run where `PURLIS_TEST_ENGINE` names a Chrome
//! executable, and on CI against the runner's own Chrome. Anywhere else each test says why it
//! did not run and passes.

#![cfg(unix)]

mod common;

use std::time::Duration;

use common::drive::{open, sees, snapshot};
use common::site::started;
use purlis_browser_engine::drive::PersonInput;

#[test]
fn the_persons_keys_reach_a_field_while_another_frame_of_the_page_reloads() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    open(&browser, "typing-beside-a-reloading-frame.html");
    browser.take_over();
    let aim = browser.aim().expect("a page is shown");
    for letter in ["a", "b", "c", "d", "e"] {
        // The advert has moved on since the aim was taken.
        std::thread::sleep(Duration::from_millis(150));
        for down in [true, false] {
            let key = PersonInput::Key {
                aim: aim.clone(),
                down,
                key: letter.to_owned(),
                code: format!("Key{}", letter.to_ascii_uppercase()),
                key_code: u32::from(letter.to_ascii_uppercase().chars().next().unwrap()),
                modifiers: 0,
            };
            browser
                .person_input(&key)
                .unwrap_or_else(|why| panic!("{letter} ({down}): {why}"));
        }
    }
    assert_ne!(browser.aim(), Some(aim), "the advert never moved on");
    browser.hand_back();
    assert!(sees(&browser, "Typed: 5"), "{}", snapshot(&browser));
}

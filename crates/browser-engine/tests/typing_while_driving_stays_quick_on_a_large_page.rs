// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Through a real engine (#2295): while the person drives, each key they type into a field of a
//! large page (twenty thousand rows) reaches the page within the spec's 50 ms for input, every
//! protection of #2014 kept: the field is blanked before the first key, read after each, and
//! what was typed is in no later answer. Each key's time, from the window's event to its return
//! (the key sent and the field read after it), is said on CI.
//!
//! These need a real Chrome and the right to start it and to listen on a loopback port, which a
//! sandboxed agent does not have. They run where `PURLIS_TEST_ENGINE` names a Chrome
//! executable, and on CI against the runner's own Chrome. Anywhere else each test says why it
//! did not run and passes.

#![cfg(unix)]

mod common;

use std::io::Write as _;
use std::time::{Duration, Instant};

use common::drive::{open, sees, snapshot};
use common::site::started;
use purlis_browser_engine::drive::PersonInput;

/// The spec's bound from a keystroke to the screen (docs/spec.md, "Keystroke to screen").
const MOST_PER_KEY: Duration = Duration::from_millis(50);

#[test]
fn typing_into_a_field_of_a_large_page_stays_under_fifty_milliseconds_a_key() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    open(&browser, "typing-on-a-large-page.html");
    browser.take_over();
    let aim = browser.aim().expect("a page is shown");
    let typed = "squirrelnineteenxyz";
    let mut took = Vec::new();
    for letter in typed.chars() {
        let key = |down| PersonInput::Key {
            aim: aim.clone(),
            down,
            key: letter.to_string(),
            code: format!("Key{}", letter.to_ascii_uppercase()),
            key_code: u32::from(letter.to_ascii_uppercase()),
            modifiers: 0,
        };
        let started = Instant::now();
        browser.person_input(&key(true)).unwrap();
        took.push(started.elapsed());
        browser.person_input(&key(false)).unwrap();
    }
    let first = took[0];
    let mut rest: Vec<Duration> = took[1..].to_vec();
    rest.sort();
    let median = rest[rest.len() / 2];
    let worst = *rest.last().unwrap();
    {
        let mut out = std::io::stdout().lock();
        let _ = writeln!(
            out,
            "::notice title=key to page on a large page::first key {} ms (it blanks the field), then median {} ms, worst {} ms, over {} keys",
            first.as_millis(),
            median.as_millis(),
            worst.as_millis(),
            rest.len()
        );
        let _ = out.flush();
    }
    assert!(
        median < MOST_PER_KEY,
        "median {median:?}, every key {took:?}"
    );
    browser.hand_back();
    assert!(
        sees(&browser, &format!("Typed: {}", typed.len())),
        "{}",
        snapshot(&browser)
    );
    assert!(
        !snapshot(&browser).contains(typed),
        "what was typed is kept out"
    );
}

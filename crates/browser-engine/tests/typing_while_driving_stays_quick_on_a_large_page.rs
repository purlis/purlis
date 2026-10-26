// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Through a real engine (#2295): while the person drives, each key they type into a field of a
//! large page (twenty thousand rows) reaches the page within the spec's 50 ms for input, every
//! protection of #2014 kept: the field is blanked before the first key, read after each, and
//! what was typed is in no later answer. Each key's time, from the window's event to its return
//! (the key sent and the field read after it), is said on CI; the first key's, which blanks the
//! field from the tree around it (#2967), is said and not held to the bound.
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
use purlis_browser_engine::drive::{Aim, PersonInput, Refusal};

/// The spec's bound from a keystroke to the screen (docs/spec.md, "Keystroke to screen").
const MOST_PER_KEY: Duration = Duration::from_millis(50);

/// How long the window waits for a key's answer (`LINK_WITHIN` in the app's browsers).
const THE_WINDOW_WAITS: Duration = Duration::from_secs(30);

/// One key of the person's, `letter` a character or a key's name.
fn key(aim: &Aim, letter: &str, key_code: u32, down: bool) -> PersonInput {
    let code = if letter.chars().count() == 1 {
        format!("Key{}", letter.to_ascii_uppercase())
    } else {
        letter.to_owned()
    };
    PersonInput::Key {
        aim: aim.clone(),
        down,
        key: letter.to_owned(),
        code,
        key_code,
        modifiers: 0,
    }
}

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
        let (name, code) = (letter.to_string(), u32::from(letter.to_ascii_uppercase()));
        let started = Instant::now();
        browser.person_input(&key(&aim, &name, code, true)).unwrap();
        took.push(started.elapsed());
        browser
            .person_input(&key(&aim, &name, code, false))
            .unwrap();
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
            "::notice title=key to page on a large page::first key {} ms (it blanks the field from the tree around it), then median {} ms, worst {} ms, over {} keys",
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

#[test]
fn a_key_into_a_second_field_of_a_large_page_is_held_in_words_until_the_page_is_read() {
    // #3043: the second field's key reads the whole tree, which on this page may take longer
    // than the key waits. It is held back and said so, never answered as unknown, and the read
    // goes on for the key typed again.
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    open(&browser, "typing-on-a-large-page.html");
    browser.take_over();
    let aim = browser.aim().expect("a page is shown");
    browser.person_input(&key(&aim, "a", 65, true)).unwrap();
    browser.person_input(&key(&aim, "a", 65, false)).unwrap();
    browser.person_input(&key(&aim, "Tab", 9, true)).unwrap();
    browser.person_input(&key(&aim, "Tab", 9, false)).unwrap();
    let typed_at = Instant::now();
    let mut held = 0;
    loop {
        let started = Instant::now();
        let answered = browser.person_input(&key(&aim, "b", 66, true));
        let took = started.elapsed();
        assert!(
            took < THE_WINDOW_WAITS,
            "the key answered after {took:?}, past the window's wait"
        );
        let _ = browser.person_input(&key(&aim, "b", 66, false));
        match answered {
            Ok(()) => break,
            Err(Refusal::PageUnreadInTime) => held += 1,
            Err(other) => panic!("the key was answered {other:?}"),
        }
        assert!(
            typed_at.elapsed() < Duration::from_secs(240),
            "the page was never read"
        );
    }
    {
        let mut out = std::io::stdout().lock();
        let _ = writeln!(
            out,
            "::notice title=a second field of a large page::its key reached the page after {} ms, held back {held} times",
            typed_at.elapsed().as_millis()
        );
        let _ = out.flush();
    }
    browser.hand_back();
    assert!(sees(&browser, "Typed: 1"), "{}", snapshot(&browser));
}

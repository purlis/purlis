// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! An agent's read of a large page (#3277): it finishes inside the call's limit, or the call says
//! in words that the page took too long, never that whether it acted is not known.

use std::sync::Arc;
use std::time::{Duration, Instant};

use super::{browser_with, id, limits, ref_of, snapshot};
use crate::drive::{Call, Content, Refusal};

/// A browser over a large page: a read of each frame's tree takes `takes`, an agent's call waits
/// `call_waits` for the read, and the read itself may take `read_may_take` a frame.
fn a_large_page(
    takes: Duration,
    call_waits: Duration,
    read_may_take: Duration,
) -> (super::Browser, Arc<super::Engine>) {
    let (browser, engine) = browser_with(crate::drive::Limits {
        page_reads_within: call_waits,
        tree_reads_within: read_may_take,
        ..limits()
    });
    super::super::lock(&engine.model).ax_takes = Some(takes);
    (browser, engine)
}

fn snapshot_call() -> Call {
    Call::Snapshot {
        page: None,
        within: None,
        from: 1,
        lines: None,
    }
}

/// How many reads of a frame's whole tree the engine was asked for.
fn tree_reads(engine: &super::Engine) -> usize {
    engine.sent("Accessibility.getFullAXTree").len()
}

/// Sleeps past a slow read's end, with room for a busy machine.
fn past_the_read() {
    std::thread::sleep(Duration::from_millis(1500));
}

#[test]
fn a_snapshot_of_a_page_read_past_the_calls_limit_says_so_in_words() {
    let (browser, _engine) = a_large_page(
        Duration::from_millis(600),
        Duration::from_millis(150),
        Duration::from_secs(30),
    );

    let started = Instant::now();
    let answer = browser.call(&id(1), snapshot_call());

    assert_eq!(answer, Err(Refusal::PageReadTooLong));
    assert!(
        started.elapsed() < Duration::from_millis(500),
        "the call waited {:?}",
        started.elapsed()
    );
    let said = Refusal::PageReadTooLong.to_string();
    assert!(said.contains("too large to read"), "{said}");
    assert!(said.contains("again in a moment"), "{said}");
}

#[test]
fn the_read_a_call_left_running_answers_the_next_call() {
    let (browser, engine) = a_large_page(
        Duration::from_millis(300),
        Duration::from_millis(100),
        Duration::from_secs(30),
    );
    assert_eq!(
        browser.call(&id(1), snapshot_call()),
        Err(Refusal::PageReadTooLong)
    );
    past_the_read();
    let read = tree_reads(&engine);

    let answer = browser.call(&id(2), snapshot_call()).unwrap();

    assert!(
        matches!(&answer.content, Content::Text(text) if text.contains("Pay now")),
        "{answer:?}"
    );
    assert_eq!(tree_reads(&engine), read, "the page was not read again");
    let note = answer.note.unwrap_or_default();
    assert!(note.contains("a read that began"), "{note}");
}

#[test]
fn a_read_past_its_own_limit_is_said_in_words_never_unknown() {
    let (browser, _engine) = a_large_page(
        Duration::from_millis(400),
        Duration::from_secs(5),
        Duration::from_millis(100),
    );

    let answer = browser.call(&id(1), snapshot_call());

    assert_eq!(answer, Err(Refusal::PageReadTooLong));
}

#[test]
fn a_find_and_a_wait_on_a_page_read_past_the_calls_limit_say_so_in_words() {
    let (browser, _engine) = a_large_page(
        Duration::from_millis(600),
        Duration::from_millis(150),
        Duration::from_secs(30),
    );

    let found = browser.call(
        &id(1),
        Call::Find {
            page: None,
            text: Some("Pay".into()),
            role: None,
            within: None,
        },
    );
    past_the_read();
    // The read the find left is used once, by the next call that reads.
    let _ = browser.call(&id(2), snapshot_call());
    let waited = browser.call(
        &id(3),
        Call::WaitForText {
            page: None,
            text: "Pay now".into(),
            gone: false,
            within: Duration::from_millis(10),
        },
    );

    assert_eq!(found, Err(Refusal::PageReadTooLong));
    assert_eq!(waited, Err(Refusal::PageReadTooLong));
}

#[test]
fn a_call_that_acts_leaves_no_earlier_read_for_the_next_call() {
    let (browser, engine) = a_large_page(
        Duration::from_millis(1),
        Duration::from_millis(100),
        Duration::from_secs(30),
    );
    let text = snapshot(&browser, 1);
    let pay = ref_of(&text, "- button \"Pay now\"");
    super::super::lock(&engine.model).ax_takes = Some(Duration::from_millis(300));
    // A snapshot whose read outlives it, then a click: the read is of the page before the click.
    assert_eq!(
        browser.call(&id(2), snapshot_call()),
        Err(Refusal::PageReadTooLong)
    );
    past_the_read();
    super::super::lock(&engine.model).ax_takes = None;
    browser.call(&id(3), Call::Click { target: pay }).unwrap();
    let read = tree_reads(&engine);

    snapshot(&browser, 4);

    assert!(tree_reads(&engine) > read, "the page was read anew");
}

#[test]
fn a_wait_whose_page_was_read_once_answers_what_that_read_showed() {
    let (browser, _engine) = a_large_page(
        Duration::from_millis(200),
        Duration::from_millis(500),
        Duration::from_secs(30),
    );

    let waited = browser
        .call(
            &id(1),
            Call::WaitForText {
                page: None,
                text: "Nowhere on the page".into(),
                gone: false,
                within: Duration::from_millis(600),
            },
        )
        .unwrap();

    assert_eq!(waited.content, Content::Seen(false));
}

#[test]
fn a_call_waiting_on_a_slow_read_answers_at_once_when_the_person_takes_the_browser_over() {
    let (browser, _engine) = a_large_page(
        Duration::from_secs(2),
        Duration::from_secs(10),
        Duration::from_secs(30),
    );
    let browser = Arc::new(browser);
    let taking = Arc::clone(&browser);
    let taker = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(200));
        taking.take_over();
    });

    let started = Instant::now();
    let answer = browser.call(&id(1), snapshot_call());
    taker.join().unwrap();

    assert_eq!(answer, Err(Refusal::Interrupted));
    assert!(
        started.elapsed() < Duration::from_millis(1500),
        "the call waited {:?}",
        started.elapsed()
    );
}

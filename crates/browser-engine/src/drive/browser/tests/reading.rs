// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! An agent's read of a large page (#3277): it finishes inside the call's limit, or the call says
//! in words that the page took too long, never that whether it acted is not known.

use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::json;

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

    assert_eq!(answer, Err(Refusal::PageReadPastItsLimit));
    let said = Refusal::PageReadPastItsLimit.to_string();
    assert!(said.contains("did not finish"), "{said}");
    assert!(!said.contains("goes on"), "{said}");
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
    let (browser, engine) = a_large_page(
        Duration::from_millis(1),
        Duration::from_millis(500),
        Duration::from_secs(30),
    );
    snapshot(&browser, 1);
    let one_read = tree_reads(&engine);
    // The wait's first read comes. Its next takes past the wait's end, however slow the machine.
    engine.on(move |sent, engine| {
        if sent.method == "Accessibility.getFullAXTree"
            && engine.sent("Accessibility.getFullAXTree").len() > 2 * one_read
        {
            super::super::lock(&engine.model).ax_takes = Some(Duration::from_secs(5));
        }
        None
    });

    let waited = browser
        .call(
            &id(2),
            Call::WaitForText {
                page: None,
                text: "Nowhere on the page".into(),
                gone: false,
                within: Duration::from_millis(1500),
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

/// A browser over a large page as [`a_large_page`], a read under way or come used for
/// `kept_for`.
fn a_large_page_kept_for(
    takes: Duration,
    call_waits: Duration,
    kept_for: Duration,
) -> (super::Browser, Arc<super::Engine>) {
    let (browser, engine) = browser_with(crate::drive::Limits {
        page_reads_within: call_waits,
        page_read_kept_for: kept_for,
        ..limits()
    });
    super::super::lock(&engine.model).ax_takes = Some(takes);
    (browser, engine)
}

#[test]
fn an_act_made_while_a_read_settles_leaves_that_read_for_no_later_call() {
    let (browser, engine) = browser_with(crate::drive::Limits {
        page_reads_within: Duration::from_millis(150),
        settle_within: Duration::from_millis(600),
        ..limits()
    });
    let text = snapshot(&browser, 1);
    let pay = ref_of(&text, "- button \"Pay now\"");
    // The page asks for data it never gets: a call settles for all its settling's time.
    engine.emit(
        "Network.requestWillBeSent",
        json!({"requestId":"9.1","loaderId":"L9","frameId":"PAGE-1","request":{"url":"http://fixture.test/slow-data"},"type":"Fetch"}),
        Some("S-PAGE-1"),
    );
    super::super::lock(&engine.model).ax_takes = Some(Duration::from_millis(300));
    let browser = Arc::new(browser);
    let reading = Arc::clone(&browser);
    // A snapshot settles, then begins its read, which outlives it.
    let snapshot_made = std::thread::spawn(move || reading.call(&id(2), snapshot_call()));
    std::thread::sleep(Duration::from_millis(200));
    // A click is made while the snapshot still settles, and acts once it has answered.
    browser.call(&id(3), Call::Click { target: pay }).unwrap();
    assert_eq!(snapshot_made.join().unwrap(), Err(Refusal::PageReadTooLong));
    past_the_read();
    super::super::lock(&engine.model).ax_takes = None;
    let read = tree_reads(&engine);

    let _ = browser.call(&id(4), snapshot_call());

    assert!(
        tree_reads(&engine) > read,
        "the read from before the click was used"
    );
}

#[test]
fn a_wait_uses_no_read_that_began_before_it() {
    let (browser, engine) = a_large_page(
        Duration::from_millis(300),
        Duration::from_millis(150),
        Duration::from_secs(30),
    );
    assert_eq!(
        browser.call(&id(1), snapshot_call()),
        Err(Refusal::PageReadTooLong)
    );
    past_the_read();
    let read = tree_reads(&engine);

    let waited = browser.call(
        &id(2),
        Call::WaitForText {
            page: None,
            text: "Pay now".into(),
            gone: true,
            within: Duration::from_millis(10),
        },
    );

    assert_eq!(waited, Err(Refusal::PageReadTooLong));
    assert!(tree_reads(&engine) > read, "the wait read the page anew");
}

#[test]
fn a_read_still_under_way_long_after_it_began_is_not_used() {
    // A read under way is used for as long as a call waits for one and a read is kept for after
    // it: here 1.2 s and 800 ms. This one takes 3 s, over two frames: it ends well after the
    // second call and well before the third gives up, on a slow machine too.
    let (browser, engine) = a_large_page_kept_for(
        Duration::from_millis(1500),
        Duration::from_millis(1200),
        Duration::from_millis(800),
    );
    assert_eq!(
        browser.call(&id(1), snapshot_call()),
        Err(Refusal::PageReadTooLong)
    );
    // Still under way, begun 1.2 s ago: used.
    assert_eq!(
        browser.call(&id(2), snapshot_call()),
        Err(Refusal::PageReadTooLong)
    );

    // Still under way, begun 2.4 s ago: past what a read is used for.
    let next = browser.call(&id(3), snapshot_call());

    assert_eq!(next, Err(Refusal::PageReadTooLong));
    std::thread::sleep(Duration::from_millis(300));
    assert!(
        tree_reads(&engine) > 2,
        "the page was read anew once the old read ended"
    );
}

#[test]
fn a_read_left_by_a_call_is_not_joined_by_another_while_it_runs() {
    let (browser, engine) = a_large_page(
        Duration::from_millis(1),
        Duration::from_millis(150),
        Duration::from_secs(30),
    );
    let text = snapshot(&browser, 1);
    let pay = ref_of(&text, "- button \"Pay now\"");
    // Long enough that the read is still running when the last call below ends, on a slow machine.
    super::super::lock(&engine.model).ax_takes = Some(Duration::from_secs(5));
    assert_eq!(
        browser.call(&id(2), snapshot_call()),
        Err(Refusal::PageReadTooLong)
    );
    // The click leaves that read for no later call; the engine still reads it.
    browser.call(&id(3), Call::Click { target: pay }).unwrap();
    let clicked = tree_reads(&engine);

    let next = browser.call(&id(4), snapshot_call());

    assert_eq!(next, Err(Refusal::PageReadTooLong));
    assert_eq!(
        tree_reads(&engine),
        clicked,
        "a second read of the page's tree was asked for while the first ran"
    );
}

#[test]
fn a_read_that_holds_more_than_a_read_may_is_refused_as_too_large() {
    let (browser, _engine) = browser_with(crate::drive::Limits {
        most_tree_read_bytes: 512,
        ..limits()
    });

    let answer = browser.call(&id(1), snapshot_call());

    assert_eq!(answer, Err(Refusal::TooLarge));
}

#[test]
fn a_read_under_way_is_left_for_no_call_once_the_person_takes_the_browser_over() {
    let (browser, _engine) = a_large_page(
        Duration::from_millis(300),
        Duration::from_millis(100),
        Duration::from_secs(30),
    );
    assert_eq!(
        browser.call(&id(1), snapshot_call()),
        Err(Refusal::PageReadTooLong)
    );

    browser.take_over();

    assert!(!super::super::lock(&browser.reading).keeps());
}

#[test]
fn a_read_from_before_a_frame_of_the_page_loaded_another_document_is_not_used() {
    let (browser, engine) = a_large_page(
        Duration::from_millis(300),
        Duration::from_millis(100),
        Duration::from_secs(30),
    );
    assert_eq!(
        browser.call(&id(1), snapshot_call()),
        Err(Refusal::PageReadTooLong)
    );
    engine.emit(
        "Page.frameNavigated",
        json!({"frame":{"id":"PAYMENT-FRAME","parentId":"PAGE-1","loaderId":"LOADER-P2","url":"http://other.test/pay.html?again","securityOrigin":"http://other.test","mimeType":"text/html"}}),
        Some("S-PAYMENT"),
    );
    past_the_read();
    super::super::lock(&engine.model).ax_takes = None;
    let read = tree_reads(&engine);

    let _ = browser.call(&id(2), snapshot_call());

    assert!(tree_reads(&engine) > read, "the page was read anew");
}

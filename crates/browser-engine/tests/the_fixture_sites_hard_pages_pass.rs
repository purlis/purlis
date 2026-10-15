// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The fixture site's hard pages, driven through a real engine with the agent's calls only: a
//! page longer than one snapshot, a double-click edit, a drag by an offset, nested frames,
//! shadow content, a new window and late-loading data; and a retried call that does not act
//! twice.
//!
//! These need a real Chrome and the right to start it and to listen on a loopback port (the
//! site's stand-in reach proxy), which a sandboxed agent does not have. They run where
//! `PURLIS_TEST_ENGINE` names a Chrome executable, and on CI against the runner's own Chrome.
//! Anywhere else each test says why it did not run and passes.

#![cfg(unix)]

mod common;

use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use common::site::started;
use purlis_browser_engine::WebAddress;
use purlis_browser_engine::drive::{
    Answer, Browser, Call, CallId, Content, DragTo, Field, Ref, Settled,
};

fn next_id() -> CallId {
    static NEXT: AtomicU32 = AtomicU32::new(1);
    CallId::new(format!("call-{}", NEXT.fetch_add(1, Ordering::Relaxed))).unwrap()
}

fn call(browser: &Browser, call: Call) -> Answer {
    let what = format!("{call:?}");
    browser
        .call(&next_id(), call)
        .unwrap_or_else(|why| panic!("{what}: {why}"))
}

fn open(browser: &Browser, path: &str) -> Answer {
    let address = WebAddress::parse(&format!("http://fixture.test/{path}")).unwrap();
    call(browser, Call::Open { address })
}

fn text(answer: &Answer) -> String {
    match &answer.content {
        Content::Text(text) => text.clone(),
        other => panic!("not text: {other:?}"),
    }
}

fn snapshot(browser: &Browser) -> String {
    text(&call(
        browser,
        Call::Snapshot {
            page: None,
            within: None,
            from: 1,
            lines: None,
        },
    ))
}

fn find(browser: &Browser, words: &str) -> String {
    text(&call(
        browser,
        Call::Find {
            page: None,
            text: Some(words.into()),
            role: None,
            within: None,
        },
    ))
}

/// The reference on the first line of `text` that holds `words`.
fn ref_on(text: &str, words: &str) -> Ref {
    let line = text
        .lines()
        .find(|l| l.contains(words) && l.contains('['))
        .unwrap_or_else(|| panic!("no element with {words:?} in\n{text}"));
    let start = line.find('[').unwrap() + 1;
    let end = line[start..].find(']').unwrap() + start;
    Ref::new(&line[start..end])
}

fn sees(browser: &Browser, words: &str) -> bool {
    let answer = call(
        browser,
        Call::WaitForText {
            page: None,
            text: words.into(),
            gone: false,
            within: Duration::from_secs(10),
        },
    );
    answer.content == Content::Seen(true)
}

#[test]
fn a_page_longer_than_one_snapshot_is_read_in_pages_and_reached_past_the_cut() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    assert_eq!(open(&browser, "long.html").settled, Settled::Yes);
    let first = text(&call(
        &browser,
        Call::Snapshot {
            page: None,
            within: None,
            from: 1,
            lines: Some(100),
        },
    ));
    assert!(
        first.contains("(lines 1 to 100 of") && first.contains("the next page starts at line 101"),
        "{first}"
    );
    assert!(!first.contains("Reach me"), "{first}");
    let second = text(&call(
        &browser,
        Call::Snapshot {
            page: None,
            within: None,
            from: 101,
            lines: Some(100),
        },
    ));
    assert!(second.contains("Item 120"), "{second}");
    let reach = ref_on(&find(&browser, "Reach me"), "Reach me");
    let clicked = call(&browser, Call::Click { target: reach });
    assert_eq!(clicked.settled, Settled::Yes);
    assert!(sees(&browser, "Reached the end"));
}

#[test]
fn a_double_click_edits_a_todo_in_place() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    open(&browser, "todo.html");
    let item = ref_on(&find(&browser, "Buy milk"), "Buy milk");
    call(&browser, Call::DoubleClick { target: item });
    let edit = ref_on(&find(&browser, "Edit todo"), "Edit todo");
    call(
        &browser,
        Call::Type {
            page: None,
            fields: vec![Field {
                into: Some(edit),
                text: "Buy bread".into(),
                replace: true,
            }],
        },
    );
    call(
        &browser,
        Call::Press {
            page: None,
            keys: vec!["Enter".into()],
        },
    );
    assert!(sees(&browser, "Buy bread"));
    let after = snapshot(&browser);
    assert!(!after.contains("textbox"), "{after}");
    assert!(!after.contains("Buy milk"), "{after}");
}

#[test]
fn a_drag_by_an_offset_moves_a_handle_that_far() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    open(&browser, "slider.html");
    let before = snapshot(&browser);
    assert!(before.contains("value=\"0\""), "{before}");
    let handle = ref_on(&before, "slider \"Level\"");
    call(
        &browser,
        Call::Drag {
            from: handle,
            to: DragTo::Offset { x: 100.0, y: 0.0 },
        },
    );
    let after = snapshot(&browser);
    assert!(
        after
            .lines()
            .any(|l| l.contains("slider \"Level\"") && l.contains("value=\"50\"")),
        "{after}"
    );
}

#[test]
fn nested_frames_are_named_and_their_elements_are_acted_on_in_place() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    open(&browser, "frames.html");
    let tree = snapshot(&browser);
    assert!(tree.contains("- frame \"inner\" ["), "{tree}");
    assert!(
        tree.lines()
            .any(|l| l.trim_start().starts_with("- frame \"") && l.contains("eeper")),
        "{tree}"
    );
    let found = find(&browser, "Deep button");
    assert!(found.contains(", in frame \""), "{found}");
    let deep = ref_on(&found, "Deep button");
    call(&browser, Call::Click { target: deep });
    assert!(sees(&browser, "Deep clicked"));
}

#[test]
fn shadow_content_is_in_the_snapshot_and_acted_on() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    open(&browser, "shadow.html");
    let tree = snapshot(&browser);
    assert!(tree.contains("Closed shadow text"), "{tree}");
    let button = ref_on(&tree, "Shadow button");
    call(&browser, Call::Click { target: button });
    assert!(sees(&browser, "Shadow clicked"));
}

#[test]
fn an_action_that_opens_a_window_answers_with_the_new_page() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    open(&browser, "window.html");
    let link = ref_on(&snapshot(&browser), "Open in a new window");
    let clicked = call(&browser, Call::Click { target: link });
    assert_eq!(clicked.opened.len(), 1, "{clicked:?}");
    let opened = &clicked.opened[0];
    assert!(opened.address.ends_with("/opened.html"), "{opened:?}");
    assert!(opened.current);
    assert_eq!(clicked.settled, Settled::Yes);
    assert!(snapshot(&browser).contains("The opened page"));
}

#[test]
fn late_loading_data_is_there_once_the_page_has_settled() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    let opened = open(&browser, "late.html");
    assert_eq!(opened.settled, Settled::Yes);
    // No wait: the open answered once the data had arrived.
    let tree = snapshot(&browser);
    assert!(tree.contains("Data arrived: 42"), "{tree}");
}

#[test]
fn a_retried_click_with_the_same_id_clicks_once() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    open(&browser, "counter.html");
    let add = ref_on(&snapshot(&browser), "Add one");
    let id = next_id();
    let first = browser
        .call(
            &id,
            Call::Click {
                target: add.clone(),
            },
        )
        .unwrap();
    let again = browser.call(&id, Call::Click { target: add }).unwrap();
    assert!(!first.repeated);
    assert!(again.repeated);
    let tree = snapshot(&browser);
    assert!(tree.contains("Count: 1"), "{tree}");
}

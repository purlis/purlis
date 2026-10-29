// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The fixture site's hard pages, driven through a real engine with the agent's calls only: a
//! page longer than one snapshot, a double-click edit, a drag by an offset, nested frames,
//! shadow content, a link that opens a page and late-loading data; a retried call that does
//! not act twice; and a blanked field painted over in a screenshot.
//!
//! These need a real Chrome and the right to start it and to listen on a loopback port (the
//! site's stand-in reach proxy), which a sandboxed agent does not have. They run where
//! `PURLIS_TEST_ENGINE` names a Chrome executable, and on CI against the runner's own Chrome.
//! Anywhere else each test says why it did not run and passes.

#![cfg(unix)]

use crate::common;

use common::drive::{
    call, find, next_id, open, red_pixels, ref_on, screenshot, sees, snapshot, text,
};
use common::site::started;
use purlis_browser_engine::drive::{Call, Content, DragTo, Field, Settled};

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
fn an_action_that_opens_a_page_answers_with_the_new_page() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    open(&browser, "opens-a-page.html");
    let link = ref_on(&snapshot(&browser), "Open in a new page");
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

#[test]
fn a_blanked_field_at_the_edge_of_a_scrolling_page_is_painted_over_whole() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    open(&browser, "blanked.html");
    let card = ref_on(&find(&browser, "Card number"), "Card number");
    assert!(
        red_pixels(&screenshot(&browser)) > 1000,
        "the field shows before"
    );
    browser.blank(&card).unwrap();
    let shot = screenshot(&browser);
    assert_eq!(
        red_pixels(&shot),
        0,
        "every pixel of the field is painted over"
    );
    let tree = snapshot(&browser);
    assert!(!tree.contains("4000 1234"), "{tree}");
    let found = browser.call(
        &next_id(),
        Call::Find {
            page: None,
            text: Some("5678".into()),
            role: None,
            within: None,
        },
    );
    assert!(
        matches!(&found, Ok(a) if matches!(&a.content, Content::Text(t) if t.ends_with("Nothing on the page matches.\n"))),
        "{found:?}"
    );
}

#[test]
fn a_captcha_page_shows_the_agent_a_field_and_never_the_letters_it_asks_for() {
    // #2015: the page the agent asks the person to take over for. Its letters are drawn in a
    // picture, so no answer of the agent's tools reads them.
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    open(&browser, "captcha.html");
    let seen = snapshot(&browser);
    assert!(seen.contains("Prove you are a person"), "{seen}");
    assert!(seen.contains("Letters"), "{seen}");
    assert!(!seen.contains("Q7XK"), "{seen}");
}

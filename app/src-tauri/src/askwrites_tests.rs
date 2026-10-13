// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use purlis_core::harness::hooked::{Source, ask};

fn asked(tool: &str, input: serde_json::Value) -> Ask {
    ask(
        Source::ClaudeCode,
        &serde_json::json!({"tool_name": tool, "tool_input": input}),
    )
}

#[test]
fn a_write_is_drawn_as_the_whole_file_against_nothing() {
    let shown = of(&asked(
        "Write",
        serde_json::json!({"file_path": "/w/a.rs", "content": "one\ntwo\n"}),
    ));

    assert_eq!(
        shown,
        Some(WritesShown::Whole {
            path: "/w/a.rs".to_owned(),
            changes: vec![ChangeShown {
                says: WRITES_THE_FILE.to_owned(),
                before: String::new(),
                after: "one\ntwo\n".to_owned(),
                hunks: vec![GitHunk {
                    old_start: 0,
                    old_lines: 0,
                    new_start: 1,
                    new_lines: 2
                }],
            }],
        })
    );
}

#[test]
fn an_edit_is_drawn_as_the_text_it_replaces_beside_what_it_puts_there() {
    let shown = of(&asked(
        "MultiEdit",
        serde_json::json!({"file_path": "/w/a.rs", "edits": [
            {"old_string": "a\nb", "new_string": "c", "replace_all": true},
            {"old_string": "d", "new_string": ""},
        ]}),
    ));
    let Some(WritesShown::Whole { changes, .. }) = shown else {
        panic!("shown whole: {shown:?}");
    };

    assert_eq!(changes[0].says, REPLACES_EVERY);
    assert_eq!(
        changes[0].hunks,
        [GitHunk {
            old_start: 1,
            old_lines: 2,
            new_start: 1,
            new_lines: 1
        }]
    );
    assert_eq!(changes[1].says, REPLACES);
    assert_eq!(
        changes[1].hunks,
        [GitHunk {
            old_start: 1,
            old_lines: 1,
            new_start: 0,
            new_lines: 0
        }]
    );
}

#[test]
fn a_line_that_ends_in_crlf_is_drawn_with_its_carriage_return_marked_and_says_so() {
    // #1146: an editor draws CRLF and LF alike, so a change of line endings would look like no
    // change. Each CRLF's carriage return is drawn as a mark, and the change says what it is.
    let shown = of(&asked(
        "Edit",
        serde_json::json!({"file_path": "/w/a.bat", "old_string": "one\r\ntwo\r\n",
            "new_string": "one\ntwo\n"}),
    ));

    assert_eq!(
        shown,
        Some(WritesShown::Whole {
            path: "/w/a.bat".to_owned(),
            changes: vec![ChangeShown {
                says: format!("{REPLACES} ({CRLF_MARKED})"),
                before: "one\u{240D}\ntwo\u{240D}\n".to_owned(),
                after: "one\ntwo\n".to_owned(),
                hunks: vec![GitHunk {
                    old_start: 1,
                    old_lines: 2,
                    new_start: 1,
                    new_lines: 2
                }],
            }],
        })
    );
    assert!(CRLF_MARKED.contains(purlis_core::harness::asked::edit::CARRIAGE_RETURN_DRAWN));
}

#[test]
fn only_a_change_with_a_crlf_says_how_one_is_drawn() {
    let shown = of(&asked(
        "MultiEdit",
        serde_json::json!({"file_path": "/w/a.rs", "edits": [
            {"old_string": "a", "new_string": "b"},
            {"old_string": "c", "new_string": "c\r\n", "replace_all": true},
        ]}),
    ));
    let Some(WritesShown::Whole { changes, .. }) = shown else {
        panic!("shown whole: {shown:?}");
    };

    assert_eq!(changes[0].says, REPLACES);
    assert_eq!(changes[1].says, format!("{REPLACES_EVERY} ({CRLF_MARKED})"));
    assert_eq!(changes[1].after, "c\u{240D}\n");
}

#[test]
fn a_carriage_return_that_ends_no_line_is_said_to_be_why_a_change_is_not_shown() {
    let shown = of(&asked(
        "Write",
        serde_json::json!({"file_path": "/w/a", "content": "a\rb"}),
    ));

    assert_eq!(
        shown,
        Some(WritesShown::NotShown {
            why: format!(
                "This change holds characters the Inbox cannot show as they are, such as a \
                 carriage return that ends no line. {IN_ITS_PANE}"
            )
        })
    );
}

#[test]
fn an_edit_not_shown_says_why_and_where_it_is_answered() {
    let shown = of(&asked(
        "Write",
        serde_json::json!({"file_path": "/w/a", "content": "a\0b"}),
    ));

    assert_eq!(
        shown,
        Some(WritesShown::NotShown {
            why: format!("This change is not text. {IN_ITS_PANE}")
        })
    );
}

#[test]
fn any_other_ask_draws_no_change() {
    assert_eq!(
        of(&asked("Bash", serde_json::json!({"command": "ls"}))),
        None
    );
}

#[test]
fn what_a_link_is_served_for_an_edits_change_is_this_windows_own_not_shown() {
    // #1146: the UI RPC puts it where the change was, so a link's Inbox draws the reason and
    // offers no Allow beside it, as for any change not shown.
    let served = purlis_session_protocol::ui::an_edit_drawn_in_the_window_only();
    let ours = serde_json::to_value(WritesShown::NotShown {
        why: served["why"].as_str().expect("a sentence").to_owned(),
    })
    .expect("serialised");
    assert_eq!(served, ours);
}

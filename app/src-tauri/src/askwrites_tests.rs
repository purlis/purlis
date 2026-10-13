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

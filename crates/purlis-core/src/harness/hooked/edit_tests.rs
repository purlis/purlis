// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Claude Code's `Write`, `Edit` and `MultiEdit` asks, shown whole in the window before they
//! may be allowed from it (#1146).

use super::*;
use crate::harness::asked::edit::MOST_LINES;
use crate::harness::model::{Unshown, Writes, Written};

fn asked(tool: &str, input: Value) -> Value {
    serde_json::json!({
        "session_id": "abc",
        "hook_event_name": "PermissionRequest",
        "tool_name": tool,
        "tool_input": input,
    })
}

fn ids(ask: &Ask) -> Vec<&str> {
    ask.options
        .iter()
        .map(|option| option.id.as_str())
        .collect()
}

fn not_shown(ask: &Ask) -> Option<Unshown> {
    match &ask.writes {
        Some(Writes::NotShown { why }) => Some(*why),
        _ => None,
    }
}

#[test]
fn a_write_shows_the_whole_file_it_writes_and_may_be_allowed_from_the_window() {
    let payload = asked(
        "Write",
        serde_json::json!({"file_path": "/work/a.txt", "content": "one\ntwo\n"}),
    );
    let ask = ask(Source::ClaudeCode, &payload);

    assert_eq!(
        ask.writes,
        Some(Writes::Whole {
            changes: vec![Written::File {
                content: "one\ntwo\n".to_owned()
            }]
        })
    );
    assert_eq!(ask.summary.as_str(), "Change /work/a.txt");
    assert!(shown_in_full(&ask));
    assert_eq!(ids(&ask), ["allow", "deny"]);
}

#[test]
fn an_allowed_edit_runs_with_its_own_input_and_nothing_changed() {
    let payload = asked(
        "Write",
        serde_json::json!({"file_path": "/work/a.txt", "content": "one\n"}),
    );

    let line = decision(Source::ClaudeCode, &payload, "allow").expect("a decision");
    let printed: Value = serde_json::from_str(&line).expect("json");

    assert_eq!(
        printed["hookSpecificOutput"]["decision"],
        serde_json::json!({"behavior": "allow"})
    );
}

#[test]
fn an_edit_shows_the_text_it_replaces_and_what_it_puts_there() {
    let payload = asked(
        "Edit",
        serde_json::json!({"file_path": "/work/a.rs", "old_string": "fn one()",
            "new_string": "fn two()", "replace_all": true}),
    );
    let ask = ask(Source::ClaudeCode, &payload);

    assert_eq!(
        ask.writes,
        Some(Writes::Whole {
            changes: vec![Written::Replace {
                before: "fn one()".to_owned(),
                after: "fn two()".to_owned(),
                every: true,
            }]
        })
    );
    assert_eq!(ids(&ask), ["allow", "deny"]);
}

#[test]
fn a_multi_edit_shows_each_replacement_in_the_order_it_makes_them() {
    let payload = asked(
        "MultiEdit",
        serde_json::json!({"file_path": "/work/a.rs", "edits": [
            {"old_string": "a", "new_string": "b"},
            {"old_string": "c", "new_string": "", "replace_all": false},
        ]}),
    );
    let ask = ask(Source::ClaudeCode, &payload);

    assert_eq!(
        ask.writes,
        Some(Writes::Whole {
            changes: vec![
                Written::Replace {
                    before: "a".to_owned(),
                    after: "b".to_owned(),
                    every: false
                },
                Written::Replace {
                    before: "c".to_owned(),
                    after: String::new(),
                    every: false
                },
            ]
        })
    );
    assert_eq!(ids(&ask), ["allow", "deny"]);
}

#[test]
fn an_edit_is_allowed_from_the_window_once_and_never_for_the_session() {
    // A rule for the session would let later edits run that nobody was shown.
    let mut payload = asked(
        "Edit",
        serde_json::json!({"file_path": "/work/a.rs", "old_string": "a", "new_string": "b"}),
    );
    payload["permission_suggestions"] = serde_json::json!([
        {"type": "addRules", "rules": [{"toolName": "Edit"}], "behavior": "allow",
         "destination": "session"},
        {"type": "addDirectories", "directories": ["/elsewhere"], "destination": "session"},
        {"type": "addRules", "rules": [{"toolName": "Edit"}], "behavior": "deny",
         "destination": "session"},
    ]);
    let ask = ask(Source::ClaudeCode, &payload);

    assert_eq!(ids(&ask), ["allow", "suggestion:2", "deny"]);
    assert_eq!(decision(Source::ClaudeCode, &payload, "suggestion:0"), None);
}

#[test]
fn an_edit_that_cannot_be_shown_whole_says_why_and_is_only_denied() {
    let long = "line\n".repeat(MOST_LINES + 1);
    // A forge token's shape, put together here so no file holds one.
    let token = format!("{}_{}", "ghp", "0123456789abcdefghijklmnopqrstuvwxyzAB");
    let cases = [
        (
            "Write",
            serde_json::json!({"file_path": "/w/a", "content": long}),
            Unshown::TooBig,
        ),
        (
            "Write",
            serde_json::json!({"file_path": "/w/a", "content": "a\0b"}),
            Unshown::Binary,
        ),
        (
            // A carriage return that ends no line: an editor breaks the line there or draws
            // over it.
            "Write",
            serde_json::json!({"file_path": "/w/a", "content": "a\rb"}),
            Unshown::DrawnOtherwise,
        ),
        (
            "Write",
            serde_json::json!({"file_path": "/w/a", "content": "a\r\nb\r"}),
            Unshown::DrawnOtherwise,
        ),
        (
            "Edit",
            serde_json::json!({"file_path": "/w/a", "old_string": "a", "new_string": "a\r\r\nb"}),
            Unshown::DrawnOtherwise,
        ),
        (
            // The mark the window draws a CRLF's carriage return as, written in the text
            // itself beside a line that ends in CRLF: the two would read the same.
            "Edit",
            serde_json::json!({"file_path": "/w/a", "old_string": "a\u{240D}\nb",
                "new_string": "a\r\nb"}),
            Unshown::DrawnOtherwise,
        ),
        (
            "Edit",
            serde_json::json!({"file_path": "/w/a", "old_string": "a", "new_string": "a\u{200B}b"}),
            Unshown::DrawnOtherwise,
        ),
        (
            // Marks stacked past ordinary text draw over the lines around them.
            "Write",
            serde_json::json!({"file_path": "/w/a", "content": "a\u{301}\u{302}\u{303}\u{304}\u{305}\n"}),
            Unshown::DrawnOtherwise,
        ),
        (
            "Write",
            serde_json::json!({"file_path": "/w/a", "content": format!("token = {token}\n")}),
            Unshown::Credential,
        ),
        (
            "Write",
            serde_json::json!({"file_path": "/w/a", "content": "a", "mode": "append"}),
            Unshown::Unread,
        ),
        (
            "Write",
            serde_json::json!({"file_path": "/w/a"}),
            Unshown::Unread,
        ),
        (
            "Edit",
            serde_json::json!({"file_path": "/w/a", "old_string": "", "new_string": "b"}),
            Unshown::Unread,
        ),
        (
            "Edit",
            serde_json::json!({"file_path": "/w/a", "old_string": "a", "new_string": "b",
                "replace_all": "yes"}),
            Unshown::Unread,
        ),
        (
            "MultiEdit",
            serde_json::json!({"file_path": "/w/a", "edits": []}),
            Unshown::Unread,
        ),
        (
            "NotebookEdit",
            serde_json::json!({"notebook_path": "/w/a.ipynb", "new_source": "x"}),
            Unshown::Unread,
        ),
    ];
    for (tool, input, why) in cases {
        let payload = asked(tool, input.clone());
        let ask = ask(Source::ClaudeCode, &payload);

        assert_eq!(not_shown(&ask), Some(why), "{tool} {input}");
        assert_eq!(ids(&ask), ["deny"], "{tool} {input}");
        assert_eq!(decision(Source::ClaudeCode, &payload, "allow"), None);
    }
}

#[test]
fn a_change_whose_lines_end_in_crlf_is_shown_whole_and_may_be_allowed() {
    // #1146: a CRLF file's edit carries its line endings, which the window draws visibly
    // (`asked::edit::drawn`), so the change is shown whole and its Allow is offered.
    for (tool, input) in [
        (
            "Write",
            serde_json::json!({"file_path": "/w/a.bat", "content": "echo one\r\necho two\r\n"}),
        ),
        (
            // From CRLF to LF, and a file that mixes the two.
            "Edit",
            serde_json::json!({"file_path": "/w/a.bat", "old_string": "one\r\ntwo\nthree",
                "new_string": "one\ntwo\nthree"}),
        ),
    ] {
        let payload = asked(tool, input.clone());
        let ask = ask(Source::ClaudeCode, &payload);

        assert!(
            matches!(ask.writes, Some(Writes::Whole { .. })),
            "{tool} {input}: {:?}",
            ask.writes
        );
        assert_eq!(ids(&ask), ["allow", "deny"], "{tool} {input}");
        // What runs is the tool's own input, carriage returns and all.
        let line = decision(Source::ClaudeCode, &payload, "allow").expect("a decision");
        let printed: Value = serde_json::from_str(&line).expect("json");
        assert_eq!(
            printed["hookSpecificOutput"]["decision"],
            serde_json::json!({"behavior": "allow"})
        );
    }
    // What the change holds is kept as it came: only the window's drawing marks it.
    let ask = ask(
        Source::ClaudeCode,
        &asked(
            "Write",
            serde_json::json!({"file_path": "/w/a", "content": "a\r\nb"}),
        ),
    );
    assert_eq!(
        ask.writes,
        Some(Writes::Whole {
            changes: vec![Written::File {
                content: "a\r\nb".to_owned()
            }]
        })
    );
}

#[test]
fn a_crlf_s_carriage_return_is_drawn_as_a_mark_before_its_line_break() {
    use crate::harness::asked::edit::{CARRIAGE_RETURN_DRAWN, drawn, ends_a_line_in_crlf};

    assert_eq!(CARRIAGE_RETURN_DRAWN, '\u{240D}');
    assert_eq!(drawn("a\r\nb\nc\r\n"), "a\u{240D}\nb\nc\u{240D}\n");
    assert_eq!(drawn("a\nb"), "a\nb");
    assert!(ends_a_line_in_crlf("a\r\nb"));
    assert!(!ends_a_line_in_crlf("a\nb"));
}

#[test]
fn the_mark_written_in_a_text_with_no_crlf_is_drawn_as_itself() {
    let payload = asked(
        "Write",
        serde_json::json!({"file_path": "/w/notes.md", "content": "CR is drawn \u{240D}\n"}),
    );

    assert_eq!(ids(&ask(Source::ClaudeCode, &payload)), ["allow", "deny"]);
}

#[test]
fn a_change_with_a_joiner_doing_its_job_may_be_allowed_and_one_between_latin_letters_may_not() {
    // #3220: the window judges a change's text by purlis's one rule (D-1301), not one
    // character at a time, so a family emoji and a Persian word read as the text they are.
    for content in [
        "family: \u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\n",
        "\u{645}\u{6CC}\u{200C}\u{62E}\u{648}\u{627}\u{647}\u{645}\n",
    ] {
        let payload = asked(
            "Write",
            serde_json::json!({"file_path": "/w/notes.md", "content": content}),
        );
        assert_eq!(
            ids(&ask(Source::ClaudeCode, &payload)),
            ["allow", "deny"],
            "{content:?}"
        );
    }
    let payload = asked(
        "Edit",
        serde_json::json!({"file_path": "/w/a", "old_string": "ab", "new_string": "a\u{200D}b"}),
    );
    let ask = ask(Source::ClaudeCode, &payload);
    assert_eq!(not_shown(&ask), Some(Unshown::DrawnOtherwise));
    assert_eq!(ids(&ask), ["deny"]);
}

#[test]
fn a_tab_and_a_newline_are_drawn_as_they_are() {
    let payload = asked(
        "Write",
        serde_json::json!({"file_path": "/w/Makefile", "content": "all:\n\tmake it\n"}),
    );

    assert_eq!(ids(&ask(Source::ClaudeCode, &payload)), ["allow", "deny"]);
}

#[test]
fn an_edit_whose_path_the_window_cannot_show_whole_is_only_denied() {
    for path in [
        "/w/a\u{202E}txt.rs".to_owned(),
        format!("/w/{}", "a".repeat(300)),
    ] {
        let payload = asked(
            "Write",
            serde_json::json!({"file_path": path, "content": "one\n"}),
        );
        let ask = ask(Source::ClaudeCode, &payload);

        assert!(matches!(ask.writes, Some(Writes::Whole { .. })));
        assert!(!shown_in_full(&ask));
        assert_eq!(ids(&ask), ["deny"]);
    }
}

#[test]
fn another_harness_s_edit_shows_nothing_and_is_never_allowed_from_the_window() {
    let payload = asked(
        "Write",
        serde_json::json!({"file_path": "/work/a.txt", "content": "one\n"}),
    );
    let ask = ask(Source::Codex, &payload);

    assert_eq!(ask.writes, None);
    assert_eq!(ids(&ask), ["deny"]);
}

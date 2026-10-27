// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **An ask never shows a value purlis knows to be a secret** (#1047): one this process read
//! from a vault or stored in one ([`crate::secrets::seen`]), whatever its shape. Where an ask
//! carries one, the window is shown a mask in its place, and nothing that lets the call run,
//! since what is shown is then not what runs.
//!
//! The store is the process's, so each test remembers a value no other test uses.

use super::*;
use crate::harness::model::{Summary, Unshown, Writes};
use crate::secrets::seen;

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

#[test]
fn a_summary_masks_a_vault_value_that_has_no_credential_shape() {
    seen::remember("correct horse battery 1047a");

    let summary = Summary::of("mysql -u app -pcorrect horse battery 1047a prod");

    assert_eq!(summary.as_str(), "mysql -u app -p[vault value] prod");
    // Read back off the wire, it is masked again.
    let forged: Summary =
        serde_json::from_value(serde_json::json!("say correct horse battery 1047a")).unwrap();
    assert_eq!(forged.as_str(), "say [vault value]");
}

#[test]
fn a_vault_value_written_over_several_lines_is_masked_on_the_one_line_shown() {
    // A summary is one line: white space is collapsed before it is read, and the value is
    // looked for both in the text as it came and on the line as it is shown.
    seen::remember("line-one-1047b\nline-two-1047b");

    let summary = Summary::of("printf 'line-one-1047b\nline-two-1047b' > key");

    assert!(!summary.as_str().contains("1047b"), "{}", summary.as_str());
}

#[test]
fn a_vault_value_across_the_lines_a_command_is_shown_on_is_masked_on_each() {
    // #3502: a command over several lines is shown line by line. A value written over those
    // lines, with its own line end or with a space where the command breaks the line, shows
    // no part of itself on any line shown.
    seen::remember("first-half-3502a\nsecond-half-3502a");
    seen::remember("left-3502b right-3502b");

    for text in [
        "Run a command: printf 'first-half-3502a\nsecond-half-3502a' > key",
        "Run a command: echo left-3502b\nright-3502b > key",
        "Run a command: echo left-3502b\r\n  right-3502b\nls",
    ] {
        let lines = Summary::lines_of(text).expect("it runs over several lines");

        assert!(lines.count > 1, "{text:?}");
        assert!(
            !lines.shown.iter().any(|line| line.contains("3502")),
            "{text:?}: {:?}",
            lines.shown
        );
        assert!(!Summary::of(text).as_str().contains("3502"), "{text:?}");
    }
}

#[test]
fn a_vault_value_that_overlaps_a_credential_shape_leaves_no_part_of_either() {
    // One that ends inside the shape, and one that begins inside it: each is masked with it
    // as one, so the part of the value outside the shape is not left beside the mask.
    let token = crate::secretshape::escaped::token();
    let (head, tail) = (&token[..6], &token[token.len() - 6..]);
    seen::remember(&format!("before-1047c:{head}"));
    seen::remember(&format!("{tail}!after-1047c"));

    let summary = Summary::of(&format!(
        "curl -H before-1047c:{token}!after-1047c https://x.test"
    ));

    assert_eq!(
        summary.as_str().matches('[').count(),
        1,
        "{}",
        summary.as_str()
    );
    assert!(
        summary.as_str().starts_with("curl -H [") && summary.as_str().ends_with("] https://x.test"),
        "{}",
        summary.as_str()
    );
}

#[test]
fn a_command_that_carries_a_vault_value_is_masked_and_only_denied_from_the_window() {
    seen::remember("hunter2-of-the-vault-1047d");
    let payload = asked(
        "Bash",
        serde_json::json!({"command": "psql postgres://app:hunter2-of-the-vault-1047d@db/prod"}),
    );

    let ask = ask(Source::ClaudeCode, &payload);

    assert_eq!(
        ask.summary.as_str(),
        "Run a command: psql postgres://app:[vault value]@db/prod"
    );
    assert!(!shown_in_full(&ask));
    assert_eq!(ids(&ask), ["deny"]);
    // The same command without it is allowed from the window, as before.
    let plain = asked(
        "Bash",
        serde_json::json!({"command": "psql postgres://db/prod"}),
    );
    assert_eq!(
        ids(&super::ask(Source::ClaudeCode, &plain)),
        ["allow", "deny"]
    );
}

#[test]
fn an_option_whose_words_carry_a_vault_value_is_not_offered() {
    seen::remember("rule-value-1047e");
    let mut payload = asked("Bash", serde_json::json!({"command": "npm test"}));
    payload["permission_suggestions"] = serde_json::json!([
        {"type": "addRules", "behavior": "deny", "destination": "session",
         "rules": [{"toolName": "Bash", "ruleContent": "curl -u app:rule-value-1047e"}]},
        {"type": "addRules", "behavior": "deny", "destination": "session",
         "rules": [{"toolName": "Bash", "ruleContent": "npm publish"}]},
    ]);

    let ask = ask(Source::ClaudeCode, &payload);

    assert_eq!(ids(&ask), ["allow", "suggestion:1", "deny"]);
    assert!(
        ask.options
            .iter()
            .all(|option| !option.label.contains("1047e"))
    );
}

#[test]
fn an_edit_that_writes_a_vault_value_is_not_shown_and_only_denied() {
    seen::remember("edit-value-of-the-vault-1047f");
    for input in [
        serde_json::json!({"file_path": "/w/.env",
            "content": "DB_PASSWORD=edit-value-of-the-vault-1047f\n"}),
        serde_json::json!({"file_path": "/w/.env", "old_string": "DB_PASSWORD=",
            "new_string": "DB_PASSWORD=edit-value-of-the-vault-1047f"}),
    ] {
        let tool = if input.get("content").is_some() {
            "Write"
        } else {
            "Edit"
        };
        let ask = ask(Source::ClaudeCode, &asked(tool, input));

        assert_eq!(
            ask.writes,
            Some(Writes::NotShown {
                why: Unshown::Credential
            })
        );
        assert_eq!(ids(&ask), ["deny"]);
    }
}

#[test]
fn a_question_answer_that_carries_a_vault_value_is_answered_in_the_pane() {
    seen::remember("answer-value-1047g");
    let payload = asked(
        "AskUserQuestion",
        serde_json::json!({"questions": [{
            "question": "Which password?",
            "header": "Password",
            "multiSelect": false,
            "options": [
                {"label": "answer-value-1047g", "description": "the first"},
                {"label": "another", "description": "the second"},
            ],
        }]}),
    );

    let ask = ask(Source::ClaudeCode, &payload);

    assert!(
        ask.options
            .iter()
            .all(|option| option.kind != ChoiceKind::Allow),
        "{:?}",
        ask.options
    );
    assert!(
        ask.options
            .iter()
            .all(|option| !option.label.contains("1047g"))
    );
}

#[test]
fn a_vault_value_across_a_kept_blank_and_a_line_end_is_masked_on_the_line_shown() {
    // #3470: a vertical tab (or a no-break space) stays on the summary's line while the line
    // end beside it is collapsed. The value, put on one line the same way, is still found.
    for (value, said) in [
        ("ab-1047d\u{0B}\ncd-1047d", "echo ab-1047d\u{0B}\ncd-1047d"),
        (
            "ab-1047e\u{00A0}\ncd-1047e",
            "echo ab-1047e\u{00A0}\ncd-1047e",
        ),
    ] {
        seen::remember(value);

        let summary = Summary::of(said);

        assert!(!summary.as_str().contains("1047"), "{}", summary.as_str());
    }
}

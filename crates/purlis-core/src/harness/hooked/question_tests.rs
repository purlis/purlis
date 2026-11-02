// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Claude Code's `AskUserQuestion`, answered from the Inbox on its `PermissionRequest` hook
//! (#1703): one question, one choice, sent back as the tool's own `answers`.

use super::*;
use crate::harness::asks::Admitted;
use crate::harness::model::{ChoiceKind, ChoiceScope};

fn asked(questions: Value) -> Value {
    serde_json::json!({
        "session_id": "abc",
        "hook_event_name": "PermissionRequest",
        "tool_name": "AskUserQuestion",
        "tool_input": { "questions": questions },
    })
}

fn one(question: &str, labels: &[&str]) -> Value {
    let options: Vec<Value> = labels
        .iter()
        .map(|label| serde_json::json!({ "label": label, "description": "why it fits" }))
        .collect();
    serde_json::json!({
        "question": question,
        "header": "Pick",
        "options": options,
        "multiSelect": false,
    })
}

fn ids(ask: &Ask) -> Vec<&str> {
    ask.options
        .iter()
        .map(|option| option.id.as_str())
        .collect()
}

#[test]
fn one_question_is_asked_in_its_own_words_with_each_answer_an_option() {
    let payload = asked(serde_json::json!([one(
        "Which database should the tests use?",
        &["SQLite", "Postgres"]
    )]));
    let ask = ask(Source::ClaudeCode, &payload);

    assert_eq!(
        ask.action,
        Action::Question {
            question: "Which database should the tests use?".to_owned()
        }
    );
    assert_eq!(
        ask.summary.as_str(),
        "Asks you: Which database should the tests use?"
    );
    assert!(shown_in_full(&ask));
    assert_eq!(ids(&ask), ["answer:0", "answer:1"]);
    let labels: Vec<&str> = ask.options.iter().map(|o| o.label.as_str()).collect();
    assert_eq!(labels, ["SQLite", "Postgres"]);
    assert!(
        ask.options
            .iter()
            .all(|o| o.kind == ChoiceKind::Allow && o.scope == ChoiceScope::Once)
    );
}

#[test]
fn an_answer_goes_back_as_the_tools_own_answers_beside_its_input() {
    let question = one(
        "Which database should the tests use?",
        &["SQLite", "Postgres"],
    );
    let payload = asked(serde_json::json!([question.clone()]));

    let line = decision(Source::ClaudeCode, &payload, "answer:1").expect("a decision");
    let printed: Value = serde_json::from_str(&line).expect("json");

    assert_eq!(
        printed,
        serde_json::json!({
            "hookSpecificOutput": {
                "hookEventName": "PermissionRequest",
                "decision": {
                    "behavior": "allow",
                    "updatedInput": {
                        "questions": [question],
                        "answers": { "Which database should the tests use?": "Postgres" },
                    },
                },
            }
        })
    );
}

#[test]
fn a_question_is_never_simply_allowed_nor_answered_by_a_suggestion() {
    // An allow with no answers would let the tool run with nothing chosen.
    let mut payload = asked(serde_json::json!([one("Go on?", &["Yes", "No"])]));
    payload["permission_suggestions"] = serde_json::json!([
        {"type": "addRules", "rules": [{"toolName": "AskUserQuestion"}],
         "behavior": "allow", "destination": "session"}
    ]);

    assert_eq!(
        ids(&ask(Source::ClaudeCode, &payload)),
        ["answer:0", "answer:1"]
    );
    assert_eq!(decision(Source::ClaudeCode, &payload, "allow"), None);
    assert_eq!(decision(Source::ClaudeCode, &payload, "suggestion:0"), None);
    assert_eq!(decision(Source::ClaudeCode, &payload, "answer:2"), None);
}

#[test]
fn more_than_one_question_or_more_than_one_choice_is_answered_in_its_pane() {
    let mut many = one("Which?", &["A", "B"]);
    many["multiSelect"] = Value::Bool(true);
    let shapes = [
        asked(serde_json::json!([
            one("First?", &["A"]),
            one("Second?", &["B"])
        ])),
        asked(serde_json::json!([many])),
        asked(serde_json::json!([])),
        asked(Value::Null),
    ];

    for payload in shapes {
        let ask = ask(Source::ClaudeCode, &payload);
        assert!(ask.options.is_empty(), "{payload}");
        assert_eq!(decision(Source::ClaudeCode, &payload, "answer:0"), None);
    }
}

#[test]
fn a_question_the_window_cannot_show_whole_offers_no_answer() {
    // Cut at the summary's width, on two lines, or drawn otherwise: the person would answer a
    // question they did not read.
    let long = format!("Should I go on with {}?", "this ".repeat(60));
    for question in [
        long.as_str(),
        "Keep the tests?\nAnd drop the database",
        "Keep the tests?\u{202E}",
        "",
    ] {
        let payload = asked(serde_json::json!([one(question, &["Yes", "No"])]));
        let ask = ask(Source::ClaudeCode, &payload);
        assert!(ask.options.is_empty(), "{question:?}");
    }
}

#[test]
fn answers_that_cannot_be_told_apart_or_read_whole_offer_none() {
    let long = "x".repeat(300);
    for labels in [
        vec!["Yes", "Yes"],
        vec!["Yes", ""],
        vec!["Yes", "No\u{200B}"],
        vec!["Yes", "No\nreally"],
        vec!["Yes", long.as_str()],
        // The same to the eye: one as a letter and its mark, one as a single character; case
        // alone; a full-width letter.
        vec!["Caf\u{E9}", "Cafe\u{301}"],
        vec!["Yes", "yes"],
        vec!["No", "\u{FF2E}o"],
        // Marks stacked past ordinary text, drawn over the rows around it.
        vec!["Yes", "No\u{301}\u{302}\u{303}\u{304}\u{305}"],
        vec![],
    ] {
        let payload = asked(serde_json::json!([one("Go on?", &labels)]));
        assert!(
            ask(Source::ClaudeCode, &payload).options.is_empty(),
            "{labels:?}"
        );
    }
    let unlabeled = asked(serde_json::json!([{
        "question": "Go on?", "options": [{"label": "Yes"}, {"description": "no label"}]
    }]));
    assert!(ask(Source::ClaudeCode, &unlabeled).options.is_empty());
}

#[test]
fn answers_in_ordinary_accented_text_are_offered() {
    // A few marks on a letter are ordinary text, and two accented words that differ are told
    // apart.
    let payload = asked(serde_json::json!([one(
        "Which?",
        &["Cafe\u{301}", "The\u{301}", "Vie\u{302}\u{323}t"]
    )]));
    assert_eq!(
        ids(&ask(Source::ClaudeCode, &payload)),
        ["answer:0", "answer:1", "answer:2"]
    );
}

#[test]
fn the_window_s_answer_reaches_the_hook_as_the_chosen_answer() {
    let hooks = HookAsks::new(Arc::new(Asks::new()));
    let payload = asked(serde_json::json!([one("Go on?", &["Yes", "No"])]));
    let now = Instant::now();
    let held = hooks
        .raise("7", Source::ClaudeCode, &payload, now)
        .expect("raised");
    let window = Answerer::admitted(Admitted::LocalUi).expect("the window answers");

    hooks
        .answer("7", &held.raised.id, "answer:0", window, now)
        .expect("answered");
    let chosen = held.answered.recv().expect("the hook hears it");

    let line = decision(Source::ClaudeCode, &payload, &chosen.id).expect("a decision");
    let printed: Value = serde_json::from_str(&line).expect("json");
    assert_eq!(
        printed["hookSpecificOutput"]["decision"]["updatedInput"]["answers"],
        serde_json::json!({ "Go on?": "Yes" })
    );
}

#[test]
fn only_claude_code_reads_a_question() {
    // Codex's hook shares the tool names but has no such tool: it is any other tool there.
    let payload = asked(serde_json::json!([one("Go on?", &["Yes", "No"])]));

    assert!(matches!(
        ask(Source::Codex, &payload).action,
        Action::Tool { .. }
    ));
}

#[test]
fn a_question_for_a_task_s_asking_chat_reads_as_one_message_with_what_it_offers() {
    let payload = asked(serde_json::json!([
        one("Which host?", &["blue", "green"]),
        one("And when?", &["now"]),
    ]));

    assert_eq!(
        asked_text(Source::ClaudeCode, &payload).as_deref(),
        Some(
            "Which host?\n- blue: why it fits\n- green: why it fits\n\n\
             And when?\n- now: why it fits"
        )
    );
}

#[test]
fn a_question_with_none_in_it_and_any_other_tool_are_not_redirected() {
    let empty = asked(serde_json::json!([]));
    assert_eq!(asked_text(Source::ClaudeCode, &empty), None);
    let other = serde_json::json!({ "tool_name": "Bash", "tool_input": { "command": "ls" } });
    assert_eq!(asked_text(Source::ClaudeCode, &other), None);
}

#[test]
fn a_harness_with_no_adapter_keeps_its_prompt() {
    let payload = asked(serde_json::json!([one("Which host?", &["blue"])]));
    for source in [Source::Codex, Source::Opencode] {
        assert_eq!(asked_text(source, &payload), None);
        assert_eq!(refused_with(source, "sent"), None);
    }
}

#[test]
fn a_redirected_question_is_refused_in_the_harness_s_own_words_and_its_turn_goes_on() {
    let said: Value = serde_json::from_str(
        &refused_with(Source::ClaudeCode, "It went to your asking chat.").expect("a decision"),
    )
    .expect("json");

    assert_eq!(
        said["hookSpecificOutput"],
        serde_json::json!({
            "hookEventName": "PermissionRequest",
            "decision": { "behavior": "deny", "message": "It went to your asking chat." },
        })
    );
}

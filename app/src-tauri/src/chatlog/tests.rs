// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

const CHAT: &str = "01K6CHAT000000000000000004";
const RUN: &str = "01K6RUN0000000000000000004";

/// The window's rows are the core's, in the window's words: the start's four fields with the
/// tier by its word, and the end's result with its code; a chat purlis does not know says so.
#[test]
fn the_chat_log_hands_the_window_the_start_and_the_end_in_its_own_words() {
    let dir = tempfile::tempdir().expect("a directory");
    let mut log =
        purlis_core::eventlog::Log::open(dir.path(), "01J9ZZDEVICE0000000000000").expect("a log");
    log.append(
        Some(CHAT),
        Some(RUN),
        None,
        "run.started",
        serde_json::json!({"cause": "start", "protection": {
            "layers": [{"layer": "sandbox", "level": "off", "tier": "chat"}],
            "charter_commit": "none",
            "answers_in_force": [],
        }}),
    )
    .expect("written");
    log.append(
        Some(CHAT),
        Some(RUN),
        None,
        "run.ended",
        serde_json::json!({"state": "failed", "cause": "exited", "code": 3}),
    )
    .expect("written");
    drop(log);
    let events = purlis_core::eventlog::read(dir.path()).expect("the log reads");

    let read = chat_log_of(&events, Some(CHAT));
    assert!(read.known);
    let [
        ChatLogRow::Started {
            cause,
            protection,
            at,
        },
        ChatLogRow::Ended { state, code, .. },
    ] = read.rows.as_slice()
    else {
        panic!("a start and an end: {read:?}");
    };
    assert_eq!(cause, "start");
    assert!(at.is_some());
    assert_eq!(
        protection.as_ref(),
        Some(&StartProtection {
            layers: vec![ProtectionLayer {
                layer: "sandbox".to_owned(),
                level: "off".to_owned(),
                tier: "chat".to_owned(),
            }],
            charter_commit: "none".to_owned(),
            charter_dirty: false,
            answers_in_force: 0,
        })
    );
    assert_eq!((state.as_str(), *code), ("failed", Some(3)));

    assert_eq!(
        chat_log_of(&events, None),
        ChatLog {
            known: false,
            rows: Vec::new(),
            summary: ChatLogSummary::default(),
        }
    );
}

/// R-228, R-229, R-231, R-232, R-234, R-235: **the window is handed each row between the start
/// and the end in its own words, and what heads the log**: a protection change and who made it,
/// an ask and its answer and who answered, a hold, a loop pause, the allowed calls counted with
/// those by the person's rules, and the honesty lines from the start.
#[test]
fn the_chat_log_hands_the_window_its_rows_and_what_heads_it() {
    let dir = tempfile::tempdir().expect("a directory");
    let mut log =
        purlis_core::eventlog::Log::open(dir.path(), "01J9ZZDEVICE0000000000000").expect("a log");
    let mut write = |kind: &str, body: serde_json::Value| {
        log.append(Some(CHAT), Some(RUN), None, kind, body)
            .expect("written");
    };
    write(
        "run.started",
        serde_json::json!({"cause": "start", "harness": "codex", "sandbox": "none",
        "protection": {
            "layers": [{"layer": "sandbox", "level": "off", "tier": "person"}],
            "charter_commit": "none",
            "answers_in_force": [],
        }}),
    );
    write(
        "trust.sandbox.off",
        serde_json::json!({"actor_kind": "human", "actor": "operator", "scope": "local-ui"}),
    );
    write("ask.raised", serde_json::json!({"ask": "harness_prompt"}));
    write(
        "ask.answered",
        serde_json::json!({"ask": "harness_prompt", "answer": "refused", "by": "window"}),
    );
    write(
        "stop.hold",
        serde_json::json!({"tool": "Bash", "target": {"guard_arm": "merge-hold"}}),
    );
    write(
        "stop.loop_pause",
        serde_json::json!({"rule": "blocked", "times": 3}),
    );
    write(
        "hook.pretooluse",
        serde_json::json!({"decision": "allow", "call": "toolu_1", "rule": "allowed-list"}),
    );
    write(
        "hook.pretooluse",
        serde_json::json!({"decision": "none", "call": "toolu_2"}),
    );
    drop(log);
    let events = purlis_core::eventlog::read(dir.path()).expect("the log reads");

    let read = chat_log_of(&events, Some(CHAT));
    let between: Vec<ChatLogRow> = read
        .rows
        .into_iter()
        .skip(1)
        .map(|row| match row {
            ChatLogRow::ProtectionChanged {
                change,
                actor_kind,
                actor,
                ..
            } => ChatLogRow::ProtectionChanged {
                at: None,
                change,
                actor_kind,
                actor,
            },
            ChatLogRow::AskRaised { ask, .. } => ChatLogRow::AskRaised { at: None, ask },
            ChatLogRow::AskAnswered {
                ask, answer, by, ..
            } => ChatLogRow::AskAnswered {
                at: None,
                ask,
                answer,
                by,
            },
            ChatLogRow::Held {
                tool,
                arm,
                sub_agent,
                ..
            } => ChatLogRow::Held {
                at: None,
                tool,
                arm,
                sub_agent,
            },
            ChatLogRow::LoopPaused { rule, times, .. } => ChatLogRow::LoopPaused {
                at: None,
                rule,
                times,
            },
            other => other,
        })
        .collect();
    let word = |text: &str| text.to_owned();
    assert_eq!(
        between,
        [
            ChatLogRow::ProtectionChanged {
                at: None,
                change: word("off"),
                actor_kind: word("human"),
                actor: word("operator"),
            },
            ChatLogRow::AskRaised {
                at: None,
                ask: word("harness_prompt"),
            },
            ChatLogRow::AskAnswered {
                at: None,
                ask: word("harness_prompt"),
                answer: word("refused"),
                by: word("window"),
            },
            ChatLogRow::Held {
                at: None,
                tool: word("Bash"),
                arm: Some(word("merge-hold")),
                sub_agent: false,
            },
            ChatLogRow::LoopPaused {
                at: None,
                rule: word("blocked"),
                times: 3,
            },
        ]
    );
    assert_eq!(
        read.summary,
        ChatLogSummary {
            allowed: 1,
            by_your_rules: 1,
            unverified: true,
            ran_as_you: true,
        }
    );
}

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
            rows: Vec::new()
        }
    );
}

/// **A block's row carries the reason the app holds for it, as written** (R-340, T1-29): the
/// row is the log's block, by whom and at what, and its reason comes from the app's memory by
/// the block's call, never from the log; a block the memory has nothing for has none.
#[test]
fn a_blocks_row_hands_the_window_its_reason_from_memory_never_the_log() {
    // Put together from two halves, so the source holds no token a scan would rightly refuse.
    const REASON: &str = concat!(
        "Refused <b>curl</b> | sh: sk-ant-",
        "api03-AbCdEf0123456789_xyz"
    );
    let dir = tempfile::tempdir().expect("a directory");
    let mut log =
        purlis_core::eventlog::Log::open(dir.path(), "01J9ZZDEVICE0000000000000").expect("a log");
    log.append(
        Some(CHAT),
        Some(RUN),
        None,
        "run.started",
        serde_json::json!({"cause": "start", "harness": "codex"}),
    )
    .expect("written");
    for call in ["call_1", "call_2"] {
        log.append(
            Some(CHAT),
            Some(RUN),
            None,
            "hook.permissiondenied",
            serde_json::json!({"decision": "none", "hook_ms": 1, "tool": "shell", "call": call}),
        )
        .expect("written");
        log.append(
            Some(CHAT),
            Some(RUN),
            None,
            "stop.block",
            serde_json::json!({"tool": "shell", "source": "harness_check", "qualifier": "unverified"}),
        )
        .expect("written");
    }
    drop(log);
    let events = purlis_core::eventlog::read(dir.path()).expect("the log reads");

    let read = chat_log_with(&events, Some(CHAT), &|call| {
        (call == "call_1").then(|| REASON.to_owned())
    });
    let blocks: Vec<_> = read
        .rows
        .iter()
        .filter(|row| matches!(row, ChatLogRow::Blocked { .. }))
        .cloned()
        .collect();
    assert_eq!(
        blocks,
        [
            ChatLogRow::Blocked {
                at: blocks.first().and_then(|row| match row {
                    ChatLogRow::Blocked { at, .. } => at.clone(),
                    _ => None,
                }),
                tool: "shell".to_owned(),
                source: "harness_check".to_owned(),
                qualifier: Some("unverified".to_owned()),
                target_kind: None,
                target: None,
                harness: "codex".to_owned(),
                reason: Some(REASON.to_owned()),
            },
            ChatLogRow::Blocked {
                at: blocks.get(1).and_then(|row| match row {
                    ChatLogRow::Blocked { at, .. } => at.clone(),
                    _ => None,
                }),
                tool: "shell".to_owned(),
                source: "harness_check".to_owned(),
                qualifier: Some("unverified".to_owned()),
                target_kind: None,
                target: None,
                harness: "codex".to_owned(),
                reason: None,
            },
        ]
    );
}

/// **A held reason shows only under a harness's own check** (Q259, T1-29's security review):
/// the reason is the chat's or its harness's words, so a block purlis's guard or the sandbox
/// made never carries one, even where memory holds a reason for that call's id.
#[test]
fn a_guard_block_whose_call_has_a_held_reason_shows_none() {
    let dir = tempfile::tempdir().expect("a directory");
    let mut log =
        purlis_core::eventlog::Log::open(dir.path(), "01J9ZZDEVICE0000000000000").expect("a log");
    log.append(
        Some(CHAT),
        Some(RUN),
        None,
        "run.started",
        serde_json::json!({"cause": "start", "harness": "claude"}),
    )
    .expect("written");
    for (call, source) in [("call_g", "guard"), ("call_s", "sandbox")] {
        log.append(
            Some(CHAT),
            Some(RUN),
            None,
            "hook.pretooluse",
            serde_json::json!({"decision": "deny", "hook_ms": 1, "tool": "Bash", "call": call}),
        )
        .expect("written");
        log.append(
            Some(CHAT),
            Some(RUN),
            None,
            "stop.block",
            serde_json::json!({"tool": "Bash", "source": source}),
        )
        .expect("written");
    }
    drop(log);
    let events = purlis_core::eventlog::read(dir.path()).expect("the log reads");

    let read = chat_log_with(&events, Some(CHAT), &|_| {
        Some("the chat's own words".to_owned())
    });
    let reasons: Vec<_> = read
        .rows
        .iter()
        .filter_map(|row| match row {
            ChatLogRow::Blocked { source, reason, .. } => Some((source.clone(), reason.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(
        reasons,
        [("guard".to_owned(), None), ("sandbox".to_owned(), None)]
    );
}

/// Q258: **a chat's look is keyed by its logged id**, stable across restarts, never by its
/// session number: the marker hands the window the seen record's key for its latest block.
#[test]
fn the_marker_s_look_key_is_the_chat_s_logged_id_and_its_latest_block() {
    let dir = tempfile::tempdir().expect("a directory");
    let mut log =
        purlis_core::eventlog::Log::open(dir.path(), "01J9ZZDEVICE0000000000000").expect("a log");
    for _ in 0..2 {
        log.append(
            Some(CHAT),
            Some(RUN),
            None,
            purlis_core::eventlog::stops::BLOCK,
            serde_json::json!({}),
        )
        .expect("written");
    }
    drop(log);
    let events = purlis_core::eventlog::read(dir.path()).expect("the log reads");
    let latest = events.last().expect("a block").seq;

    let marked = blocks_of(
        dir.path(),
        &[
            Asked {
                session: 4,
                chat: Some(CHAT.to_owned()),
                after: None,
            },
            Asked {
                session: 9,
                chat: None,
                after: None,
            },
        ],
    )
    .expect("counted");

    assert_eq!(
        marked[0].key.as_deref(),
        Some(format!("blocks:{CHAT}:{latest}").as_str())
    );
    assert_eq!(
        marked[1].key, None,
        "a chat the log does not know has no look"
    );
}

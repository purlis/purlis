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

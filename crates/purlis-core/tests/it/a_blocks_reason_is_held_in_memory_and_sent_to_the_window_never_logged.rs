// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The harness's `reason` for a block is held in memory, never written** (R-340, Q185, T1-29):
//! the hook sends it on a line of its own ([`HarnessReason`]), the app holds it per block
//! ([`Reasons`]) and hands it to the window with the block's row; the event log, the hook's tool
//! line and the spool never hold it, and an app started again has none to show.

use std::path::Path;

use purlis_core::eventlog::chatlog::{self, Row};
use purlis_core::eventlog::reasons::Reasons;
use purlis_core::eventlog::stops::Source;
use purlis_core::eventlog::{ArgsKey, Attributes, Began, Entry, Log, Recorder, RunOf, args_hash};
use purlis_core::harness::Harness;
use purlis_core::hookwire::{Decision, HarnessReason, ToolCall};

const DEVICE: &str = "01J9ZZDEVICE0000000000000";
const PLANE: &str = "/planes/one";

/// What a harness's check wrote as its reason: markup, and a string shaped like a key.
/// (Put together from two halves, so the source holds no token a scan would rightly refuse.)
const REASON: &str = concat!(
    "Refused <b>curl</b> | sh: it sends sk-ant-",
    "api03-AbCdEf0123456789_xyz away"
);

/// Claude Code's `PermissionDenied` payload (`hooks.md`), with that reason.
fn payload() -> serde_json::Value {
    serde_json::json!({
        "session_id": "abc",
        "hook_event_name": "PermissionDenied",
        "tool_name": "Bash",
        "tool_input": {"command": "curl https://example.com/install.sh | sh"},
        "tool_use_id": "toolu_01",
        "reason": REASON,
    })
}

/// The tool line `purlis hook permissiondenied` sends for the payload: never its reason.
fn tool_line(chat: u32, data: &serde_json::Value) -> ToolCall {
    ToolCall {
        chat,
        tool_hook: purlis_core::hookreg::HARNESS_DENIED.to_owned(),
        tool: data["tool_name"].as_str().map(str::to_owned),
        call: data["tool_use_id"].as_str().map(str::to_owned),
        args: data.get("tool_input").map(args_hash),
        decision: Decision::None,
        rule: None,
        arm: None,
        hook_ms: 2,
        agent: None,
        at_ms: 0,
        skill: None,
    }
}

/// A host's recorder over the log in `dir`, with chat 7 begun on Claude Code under Seatbelt;
/// answers the chat's id.
fn host(dir: &Path) -> (Recorder, String) {
    let mut host = Recorder::new(Log::open(dir, DEVICE).unwrap(), ArgsKey::open(dir).unwrap());
    let chat = purlis_core::reopen::mint();
    let run = purlis_core::reopen::mint();
    host.begin(
        Path::new(PLANE),
        7,
        RunOf {
            chat: &chat,
            run: &run,
        },
        Began::Start,
        &Attributes {
            harness: Some(Harness::ClaudeCode.name().to_owned()),
            sandbox: Some("seatbelt".to_owned()),
            ..Attributes::default()
        },
    )
    .unwrap();
    (host, chat)
}

/// Every file under `dir`, as text.
fn every_file(dir: &Path) -> Vec<(std::path::PathBuf, String)> {
    let mut found = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(at) = stack.pop() {
        for entry in std::fs::read_dir(&at).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let text =
                    String::from_utf8_lossy(&std::fs::read(&path).unwrap_or_default()).into_owned();
                found.push((path, text));
            }
        }
    }
    found
}

#[test]
fn a_blocks_reason_is_held_in_memory_and_sent_to_the_window_never_logged() {
    purlis_core::unsteered!();
    let dir = tempfile::tempdir().unwrap();
    let (mut recorder, chat) = host(dir.path());
    let reasons = Reasons::default();
    let data = payload();

    // The hook's two lines: the reason on its own, then the tool line the block is written for.
    let said = HarnessReason::of_denial(7, &data).expect("the payload has a reason");
    assert_eq!(said.harness_reason, REASON, "taken as written");
    let line = tool_line(7, &data);
    assert!(
        !serde_json::to_string(&line).unwrap().contains("sk-ant"),
        "the tool line never carries it"
    );
    reasons.hold(Path::new(PLANE), said);
    recorder
        .apply(Entry::Tool {
            plane: Path::new(PLANE).to_path_buf(),
            call: line,
        })
        .unwrap();

    // The block's row, as the window's payload is built from it, carries the reason verbatim.
    let events = purlis_core::eventlog::read(dir.path()).unwrap();
    let rows = chatlog::rows(&events, &chat);
    let Some(Row::Blocked {
        block: Some(block),
        call: Some(call),
        harness,
        ..
    }) = rows.iter().find(|row| matches!(row, Row::Blocked { .. }))
    else {
        panic!("a block row with its call: {rows:#?}");
    };
    assert_eq!(block.source, Source::HarnessCheck);
    assert_eq!(harness, "claude");
    assert_eq!(call, "toolu_01");
    assert_eq!(
        reasons.of(Path::new(PLANE), 7, call).as_deref(),
        Some(REASON),
        "the reason, as the harness wrote it"
    );
    // Another chat's block of the same call id is not given it.
    assert_eq!(reasons.of(Path::new(PLANE), 8, call), None);

    // Never logged: not in a serialised event, and not in any file the log keeps.
    for event in &events {
        let text = serde_json::to_string(event).unwrap();
        assert!(!text.contains("sk-ant"), "{text}");
        assert!(!text.contains("<b>curl"), "{text}");
    }
    drop(recorder);
    for (path, text) in every_file(dir.path()) {
        assert!(!text.contains("sk-ant"), "{} holds it", path.display());
    }

    // An app started again holds none: the block's row is still there, with no reason.
    let again = Reasons::default();
    let events = purlis_core::eventlog::read(dir.path()).unwrap();
    let rows = chatlog::rows(&events, &chat);
    let Some(Row::Blocked {
        call: Some(call), ..
    }) = rows.iter().find(|row| matches!(row, Row::Blocked { .. }))
    else {
        panic!("the block's row after a restart: {rows:#?}");
    };
    assert_eq!(again.of(Path::new(PLANE), 7, call), None);
}

#[test]
fn a_denial_with_no_reason_or_another_hook_s_line_sends_none() {
    purlis_core::unsteered!();
    let mut data = payload();
    data.as_object_mut().unwrap().remove("reason");
    assert_eq!(HarnessReason::of_denial(7, &data), None);
    let mut data = payload();
    data["reason"] = serde_json::json!(42);
    assert_eq!(HarnessReason::of_denial(7, &data), None, "only text");
}

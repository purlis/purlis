// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The commit gate reads the attendance fact** (T1-21, #6412; R-112, Q54): whether the
//! commitment gate on `userpromptsubmit` tells a chat to ask the person is keyed on the chat's
//! attendance as purlis holds it ([`purlis_core::attendance::Fact`]), never on the
//! `permission_mode` the payload names. The payload is the harness's, relayed beside the chat's
//! own commands, so a mode in it can never make an unattended chat read as attended (R-318,
//! Q271).

use purlis_core::commitgate;
use purlis_core::dispatchunattended::Attendance;
use purlis_core::toolhooks::Hook;
use serde_json::{Value, json};

/// A prompt with a fork in it, on a session's first prompt: the gate fires on it whenever the
/// chat is attended.
const FORKED: &str = "build something like the importer, and drop the old one";

/// Every `permission_mode` a payload can carry: each one Claude Code was measured to send
/// (Q270), a string nobody measured, and none at all.
const MODES: [Option<&str>; 7] = [
    Some("default"),
    Some("acceptEdits"),
    Some("plan"),
    Some("auto"),
    Some("bypassPermissions"),
    Some("a-mode-nobody-measured"),
    None,
];

/// What the gate says to `FORKED`, on a fresh session's first prompt, in a plane, for a chat
/// whose payload names `mode` and whose attendance is `attendance`.
fn gate(attendance: Attendance, mode: Option<&str>) -> Option<String> {
    let plane = tempfile::tempdir().expect("a plane");
    let mut payload = json!({"session_id": "first-prompt", "prompt": FORKED});
    if let Some(mode) = mode {
        payload["permission_mode"] = Value::from(mode);
    }
    let hook = Hook {
        root: plane.path(),
        in_plane: true,
        payload: &payload,
        env: &|_| None,
        cwd: plane.path(),
        now: chrono::Utc::now(),
        host: "a-machine",
    };
    commitgate::nudge(&hook, attendance)
}

/// R-112: flipping only attendance changes the gate's answer, and flipping only the payload's
/// mode does not. An attended chat is told to scout and ask whatever mode its payload names,
/// `bypassPermissions` included; an unattended one is never told, whatever its payload names,
/// `default` included, so no payload raises a chat's attendance.
///
/// Red on the gate as it was: it took no attendance and read `permission_mode` alone, so an
/// attended chat whose payload said `bypassPermissions` was silent, and an unattended chat
/// whose payload said `default` was told to ask the person.
#[test]
fn the_commit_gate_keys_on_attendance() {
    purlis_core::unsteered!();
    for mode in MODES {
        let attended = gate(Attendance::Attended, mode);
        assert!(
            attended
                .as_deref()
                .is_some_and(|said| said.contains("Commitment point")),
            "an attended chat whose payload names {mode:?} is told to scout and ask: {attended:?}"
        );
        let unattended = gate(Attendance::Unattended, mode);
        assert_eq!(
            unattended, None,
            "an unattended chat whose payload names {mode:?} is never told to ask anyone"
        );
    }
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The briefing reads the attendance fact** (R-111, T1-20, #6411): what a chat is told at its
//! start about whether anyone is there to ask follows [`purlis_core::attendance::Fact`], never
//! the `permission_mode` its harness's payload names. Every harness's briefing reads the same
//! fact, and a resume carries an unattended fact forward (Q198, Q270, Q271).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use purlis_core::attendance::{Begun, Fact};
use purlis_core::briefing::{Ask, parts};
use purlis_core::dispatchunattended::Attendance;
use purlis_core::harness::Harness;
use serde_json::Value;

/// A plane with two workspaces and none confirmed, so the workspace gate speaks.
fn plane() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("a temporary folder");
    let root = std::fs::canonicalize(dir.path()).expect("it resolves");
    std::fs::write(root.join("charter.toml"), "schema = 1\n").expect("the plane's file");
    for ws in ["alpha", "beta"] {
        let at = root.join("workspaces").join(ws);
        std::fs::create_dir_all(&at).expect("a workspace");
        std::fs::write(at.join("workspace.md"), format!("# {ws}\n")).expect("its charter");
    }
    (dir, root)
}

/// What a chat on the harness named `harness` is told, whose payload names `mode` and whose
/// attendance is `attendance`.
fn told(root: &Path, harness: &str, mode: &str, attendance: Attendance) -> Vec<String> {
    let env: HashMap<&str, String> = [
        ("PURLIS_SESSION_ID", "s1".to_owned()),
        ("PURLIS_HARNESS", harness.to_owned()),
    ]
    .into_iter()
    .collect();
    let lookup = move |name: &str| env.get(name).cloned();
    let payload: Value = serde_json::json!({"permission_mode": mode});
    parts(
        &Ask {
            root,
            cwd: root.parent().expect("the plane has a parent"),
            payload: &payload,
            env: &lookup,
            now: chrono::Utc::now(),
            attendance,
        },
        None,
    )
}

/// Each harness by its registry's name, and the harness it is.
const HARNESSES: [(&str, Harness); 3] = [
    ("claude-code", Harness::ClaudeCode),
    ("codex", Harness::Codex),
    ("opencode", Harness::Opencode),
];

/// The modes a payload may name: one that asks, one with the prompts off.
const MODES: [&str; 2] = ["default", "bypassPermissions"];

/// **R-111**: two chats in the same mode with different attendance are told different things,
/// and two with the same attendance in different modes are told the same, on every harness.
/// An unattended chat is never told to ask a person, whatever its payload says.
///
/// Mutation that shows it red: read `permission_mode` from the payload again in
/// `Ask::unattended`.
#[test]
fn the_briefing_follows_attendance_not_the_harness_mode() {
    purlis_core::unsteered!();
    let (_dir, root) = plane();
    for (name, harness) in HARNESSES {
        let attended = Fact::at_start(Begun::Person, Some(harness), &[]).attendance();
        let unattended = Fact::at_start(Begun::Task, Some(harness), &[]).attendance();
        assert_eq!(attended, Attendance::Attended, "{name}");
        assert_eq!(unattended, Attendance::Unattended, "{name}");
        for mode in MODES {
            let watched = told(&root, name, mode, attended);
            let alone = told(&root, name, mode, unattended);
            assert_ne!(
                watched, alone,
                "{name} in {mode}: attendance changes the text"
            );
            assert!(
                watched[0].starts_with("⬢ **Confirm the workspace before any repo work.**"),
                "{name} in {mode}, attended: {}",
                watched[0]
            );
            assert!(
                alone[0].starts_with("⬢ **STOP — this run has no workspace and nobody to ask.**"),
                "{name} in {mode}, unattended: {}",
                alone[0]
            );
            assert!(
                !alone.iter().any(|part| part.contains("Ask the person")),
                "{name} in {mode}: an unattended chat is never told to ask a person: {alone:?}"
            );
        }
        for attendance in [attended, unattended] {
            assert_eq!(
                told(&root, name, MODES[0], attendance),
                told(&root, name, MODES[1], attendance),
                "{name}, {attendance:?}: the mode the payload names changes nothing"
            );
        }
    }
}

/// **Q271**: a chat resumed from an unattended fact is briefed as unattended, though the
/// resume is a person's start and its payload names a mode that asks.
///
/// Mutation that shows it red: have the CLI or the briefing rebuild the fact with
/// [`Fact::at_start`] at a resume in place of [`Fact::resumed`].
#[test]
fn a_resumed_unattended_chat_is_briefed_as_unattended() {
    purlis_core::unsteered!();
    let (_dir, root) = plane();
    for (name, harness) in HARNESSES {
        let prior = Fact::at_start(Begun::Task, Some(harness), &[]);
        let resumed = Fact::resumed(prior, Begun::Person, Some(harness), &[]);
        let got = told(&root, name, "default", resumed.attendance());
        assert!(
            got[0].starts_with("⬢ **STOP — this run has no workspace and nobody to ask.**"),
            "{name}: {}",
            got[0]
        );
    }
}

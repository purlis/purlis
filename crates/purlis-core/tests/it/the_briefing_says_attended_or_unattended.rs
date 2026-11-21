// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The briefing says whether anyone is at the chat** (R-111, T1.1-40, #6887): every harness's
//! briefing names the attendance fact in a block of its own, `attended` or `unattended`, so a
//! chat and the transcript it leaves both carry it. The words follow the fact, never the
//! `permission_mode` the payload names.

use std::collections::HashMap;
use std::path::PathBuf;

use purlis_core::briefing::{Ask, parts};
use purlis_core::dispatchunattended::Attendance;

/// A plane with one workspace the chat is pinned to, so the briefing has no gate to say.
fn plane() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("a temporary folder");
    let root = std::fs::canonicalize(dir.path()).expect("it resolves");
    std::fs::write(root.join("charter.toml"), "schema = 1\n").expect("the plane's file");
    let at = root.join("workspaces").join("alpha");
    std::fs::create_dir_all(&at).expect("a workspace");
    std::fs::write(at.join("workspace.md"), "# alpha\n").expect("its charter");
    (dir, root)
}

/// What a chat on `harness`, pinned to `alpha`, whose payload names `mode`, is told when its
/// attendance is `attendance`.
fn told(harness: &str, mode: &str, attendance: Attendance) -> Vec<String> {
    let (_dir, root) = plane();
    let env: HashMap<&str, String> = [
        ("PURLIS_SESSION_ID", "s1".to_owned()),
        ("PURLIS_HARNESS", harness.to_owned()),
        ("PURLIS_WORKSPACE", "alpha".to_owned()),
    ]
    .into_iter()
    .collect();
    let lookup = move |name: &str| env.get(name).cloned();
    let payload = serde_json::json!({"permission_mode": mode});
    parts(
        &Ask {
            root: &root,
            cwd: root.parent().expect("the plane has a parent"),
            payload: &payload,
            env: &lookup,
            now: jiff::Timestamp::now(),
            attendance,
        },
        None,
    )
}

/// The blocks that speak of attendance, by their opening.
fn attendance_blocks(told: &[String]) -> Vec<&str> {
    told.iter()
        .map(String::as_str)
        .filter(|block| block.starts_with("⬢ **Attended") || block.starts_with("⬢ **Unattended"))
        .collect()
}

#[test]
fn the_briefing_says_attended_or_unattended() {
    for harness in ["claude-code", "codex", "opencode"] {
        // The mode asks and bypasses in turn: the words never follow it.
        for mode in ["default", "bypassPermissions"] {
            let attended = told(harness, mode, Attendance::Attended);
            let blocks = attendance_blocks(&attended);
            assert_eq!(blocks.len(), 1, "{harness}/{mode} attended: {attended:?}");
            assert!(blocks[0].starts_with("⬢ **Attended"), "{harness}/{mode}");

            let unattended = told(harness, mode, Attendance::Unattended);
            let blocks = attendance_blocks(&unattended);
            assert_eq!(
                blocks.len(),
                1,
                "{harness}/{mode} unattended: {unattended:?}"
            );
            assert!(blocks[0].starts_with("⬢ **Unattended"), "{harness}/{mode}");
        }
    }
}

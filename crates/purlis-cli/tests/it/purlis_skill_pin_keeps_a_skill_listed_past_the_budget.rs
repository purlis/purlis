// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis skill pin` on a temporary project (SK-15, #2778; ADR 0102 §4): past the budget the
//! router hides every plane skill that is not pinned, and `purlis skill list` says which are
//! pinned and why each other one is hidden. Behind the `skill-studio` flag.

use crate::support;

use std::path::{Path, PathBuf};
use std::process::Output;

/// A project with a workspace `alpha` holding 40 skills, `extra-00` to `extra-39`.
fn project() -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let at = tmp.path().canonicalize().unwrap();
    let root = at.join("plane");
    std::fs::create_dir_all(at.join("home")).unwrap();
    std::fs::create_dir_all(root.join("workspaces/alpha")).unwrap();
    std::fs::write(root.join("charter.toml"), "[plane]\n").unwrap();
    for at in 0..70 {
        let name = format!("extra-{at:02}");
        let dir = root.join("workspaces/alpha/skills").join(&name);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: Does {name}.\n---\n\nbody\n"),
        )
        .unwrap();
    }
    (tmp, at)
}

fn run(at: &Path, args: &[&str]) -> Output {
    let root = at.join("plane");
    let mut command = support::purlis();
    command
        .args(args)
        .current_dir(&root)
        .env("CHARTER_ROOT", &root)
        .env("HOME", at.join("home"))
        .env("PURLIS_CONFIG_HOME", at.join("home/config"))
        .env("PURLIS_SKILL_STUDIO", "1")
        .env("NO_COLOR", "1");
    // Run from a terminal, even where this test runs inside a chat.
    for spelling in purlis_core::envvar::spellings("PURLIS_SESSION_ID") {
        command.env_remove(spelling);
    }
    command.output().unwrap()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

#[test]
fn a_workspace_pins_a_skill_and_past_the_budget_its_chats_still_list_it() {
    let (_tmp, at) = project();
    let root = at.join("plane");

    let out = run(&at, &["skill", "pin", "extra-07", "-w", "alpha"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let manifest = std::fs::read_to_string(root.join("workspaces/alpha/workspace.json")).unwrap();
    assert!(manifest.contains("\"pinned\""), "{manifest}");

    let out = run(&at, &["skill", "list", "-w", "alpha", "--json"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let said: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let row = |name: &str| {
        said["skills"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["name"] == name)
            .cloned()
            .unwrap_or_else(|| panic!("no {name} in {said}"))
    };
    assert_eq!(row("extra-07")["pinned"], true);
    assert_eq!(row("extra-07")["hidden"], serde_json::json!([]));
    let hidden = row("extra-08")["hidden"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(!hidden.is_empty(), "{}", row("extra-08"));
    assert_eq!(hidden[0]["router"], "built-in");
    assert_eq!(hidden[0]["why"], "over-budget");
    assert!(
        hidden[0]["said"]
            .as_str()
            .is_some_and(|why| why.contains("index skill"))
    );
    assert_eq!(row("extra-08")["on"], true, "hidden, never left out");

    let none = run(&at, &["skill", "pin", "ghost", "-w", "alpha"]);
    assert!(!none.status.success());
    assert!(
        stderr(&none).contains("no skill is named"),
        "{}",
        stderr(&none)
    );
    let unpinned = run(&at, &["skill", "pin", "extra-07", "-w", "alpha", "--off"]);
    assert!(unpinned.status.success(), "{}", stderr(&unpinned));
}

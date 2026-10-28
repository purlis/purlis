// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis profile create`, `import` and `remove` (#2034, spec #2030): a profile of this
//! machine's, made in purlis's data home with its settings imported from the machine's harness
//! settings files and no sign-in file, listed in every project, and removed only once what goes
//! has been said.

use crate::support;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;

/// A machine of the test's own: a home, Claude Code's settings folder, a config home, a data
/// home and a project, each in one temporary folder.
struct Machine {
    dir: tempfile::TempDir,
}

impl Machine {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        for one in ["home", "config-home", "data", "claude-settings", "project"] {
            fs::create_dir_all(dir.path().join(one)).unwrap();
        }
        fs::write(dir.path().join("project/charter.toml"), "").unwrap();
        Self { dir }
    }

    fn path(&self, under: &str) -> PathBuf {
        self.dir.path().join(under)
    }

    fn purlis(&self, args: &[&str]) -> Output {
        support::purlis()
            .args(args)
            .current_dir(self.path("project"))
            .env("CHARTER_ROOT", self.path("project"))
            .env("HOME", self.path("home"))
            .env("PURLIS_DATA_HOME", self.path("data"))
            .env("PURLIS_CONFIG_HOME", self.path("config-home"))
            .env("CLAUDE_CONFIG_DIR", self.path("claude-settings"))
            .env_remove("PURLIS_CHAT")
            .env_remove("CHARTER_CHAT")
            .output()
            .expect("the binary runs")
    }

    fn said(&self, args: &[&str]) -> String {
        let out = self.purlis(args);
        assert!(out.status.success(), "{out:?}");
        String::from_utf8_lossy(&out.stderr).into_owned()
    }

    /// The profile `name`'s folder, as the harness is pointed at it.
    fn folder(&self, name: &str) -> PathBuf {
        self.path("data").join("profiles").join(name)
    }
}

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

#[test]
fn a_created_profile_has_the_machines_settings_and_not_its_sign_in() {
    let machine = Machine::new();
    let settings = machine.path("claude-settings");
    write(&settings.join("settings.json"), "{\"theme\": \"dark\"}\n");
    write(&settings.join(".credentials.json"), "not imported\n");
    write(&settings.join("history.jsonl"), "not imported\n");

    let said = machine.said(&["profile", "create", "work", "--harness", "claude"]);

    assert!(said.contains("made the profile 'work' (claude)"), "{said}");
    assert!(said.contains(": settings.json\n"), "{said}");
    let config = machine.folder("work").join("config");
    assert_eq!(
        fs::read_to_string(config.join("settings.json")).unwrap(),
        "{\"theme\": \"dark\"}\n"
    );
    assert!(!config.join(".credentials.json").exists());
    assert!(!config.join("history.jsonl").exists());
}

#[test]
fn a_created_profile_is_listed_in_a_project_and_points_its_harness_at_its_folder() {
    let machine = Machine::new();
    machine.said(&["profile", "create", "work", "--harness", "claude"]);

    let listed = machine.said(&["profile", "list"]);

    let row = listed
        .lines()
        .find(|line| line.trim_start().starts_with("work "))
        .unwrap_or_else(|| panic!("{listed}"));
    let config = machine.folder("work").join("config");
    assert!(
        row.contains(&format!("CLAUDE_CONFIG_DIR={} claude", config.display())),
        "{row}"
    );
    assert!(row.trim_end().ends_with("this machine"), "{row}");
}

#[test]
fn removing_a_profile_says_what_goes_and_removes_it_only_when_told_to() {
    let machine = Machine::new();
    machine.said(&["profile", "create", "work", "--harness", "claude"]);
    let folder = machine.folder("work");
    write(&folder.join("config/projects/-a/one.jsonl"), "one\n");

    let asked = machine.purlis(&["profile", "remove", "work"]);

    assert!(!asked.status.success());
    let said = String::from_utf8_lossy(&asked.stderr);
    assert!(said.contains("its one conversation"), "{said}");
    assert!(said.contains("config/projects/-a/one.jsonl"), "{said}");
    assert!(said.contains("purlis profile remove work --yes"), "{said}");
    assert!(folder.is_dir());

    let said = machine.said(&["profile", "remove", "work", "--yes"]);

    assert!(said.contains("removed the profile 'work'"), "{said}");
    assert!(!folder.exists());
    let listed = machine.said(&["profile", "list"]);
    assert!(
        !listed.lines().any(|l| l.trim_start().starts_with("work ")),
        "{listed}"
    );
}

#[test]
fn a_chat_may_not_make_a_profile() {
    let machine = Machine::new();

    let out = support::purlis()
        .args(["profile", "create", "work", "--harness", "claude"])
        .current_dir(machine.path("project"))
        .env("HOME", machine.path("home"))
        .env("PURLIS_DATA_HOME", machine.path("data"))
        .env("PURLIS_CONFIG_HOME", machine.path("config-home"))
        .env("PURLIS_CHAT", "7")
        .output()
        .expect("the binary runs");

    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("a chat may not"),
        "{out:?}"
    );
    assert!(!machine.folder("work").exists());
}

#[test]
fn a_profile_for_sandboxed_chats_only_is_made_with_nothing_imported() {
    // #2037, ruled 2026-10-15: what a sandboxed Codex chat runs on.
    let machine = Machine::new();
    write(
        &machine.path("claude-settings").join("settings.json"),
        "{\"theme\": \"dark\"}\n",
    );

    let said = machine.said(&[
        "profile",
        "create",
        "claude-sandbox",
        "--harness",
        "claude",
        "--sandbox-only",
    ]);

    assert!(
        said.contains("made the profile 'claude-sandbox' (claude)"),
        "{said}"
    );
    let config = machine.folder("claude-sandbox").join("config");
    assert!(config.is_dir());
    assert_eq!(
        fs::read_dir(&config).unwrap().count(),
        0,
        "something was imported"
    );
    // The store, under whichever spelling of purlis's folder the config home has.
    let store = fs::read_dir(machine.path("config-home"))
        .unwrap()
        .filter_map(Result::ok)
        .find_map(|dir| fs::read_to_string(dir.path().join("profiles.json")).ok())
        .expect("the store");
    assert!(store.contains("\"sandbox_only\": true"), "{store}");
}

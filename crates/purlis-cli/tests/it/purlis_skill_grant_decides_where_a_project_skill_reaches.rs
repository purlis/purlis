// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis skill grant | share` on a temporary project (SK-4, #2767): a grant decides which
//! workspaces' chats get a project skill, a workspace turns a granted one off and never on one it
//! was not granted, and Share makes a local skill shared. Behind the `skill-studio` flag.

use crate::support;

use std::path::{Path, PathBuf};
use std::process::Output;

struct Project {
    _tmp: tempfile::TempDir,
    at: PathBuf,
}

impl Project {
    /// A project with a project skill `deploy` and two workspaces, `alpha` and `beta`. No git
    /// repository, so a local skill in it is read.
    fn new() -> Project {
        let tmp = tempfile::tempdir().unwrap();
        let at = tmp.path().canonicalize().unwrap();
        let root = at.join("plane");
        for ws in ["alpha", "beta"] {
            std::fs::create_dir_all(root.join("workspaces").join(ws)).unwrap();
        }
        std::fs::create_dir_all(at.join("home")).unwrap();
        std::fs::write(root.join("charter.toml"), "[plane]\n").unwrap();
        write_skill(&root.join("skills"), "deploy");
        Project { _tmp: tmp, at }
    }

    fn root(&self) -> PathBuf {
        self.at.join("plane")
    }

    fn run(&self, args: &[&str]) -> Output {
        let root = self.root();
        let mut command = support::purlis();
        command
            .args(args)
            .current_dir(&root)
            .env("CHARTER_ROOT", &root)
            .env("HOME", self.at.join("home"))
            .env("PURLIS_CONFIG_HOME", self.at.join("home/config"))
            .env("PURLIS_SKILL_STUDIO", "1")
            .env("NO_COLOR", "1");
        // Run from a terminal, even where this test runs inside a chat: a chat changes no skill
        // itself (#2770).
        for spelling in purlis_core::envvar::spellings("PURLIS_SESSION_ID") {
            command.env_remove(spelling);
        }
        command.output().unwrap()
    }

    /// The state `purlis skill list -w <ws>` gives `name`, at its first row.
    fn state_in(&self, workspace: &str, name: &str) -> String {
        let out = self.run(&["skill", "list", "-w", workspace, "--json"]);
        assert!(out.status.success(), "{}", stderr(&out));
        let said: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        let row = said["skills"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["name"] == name)
            .cloned()
            .unwrap_or_else(|| panic!("no {name} in {said}"));
        match row["left_out"].as_str() {
            None => "on".to_owned(),
            Some(why) => why.to_owned(),
        }
    }
}

fn write_skill(dir: &Path, name: &str) {
    let dir = dir.join(name);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: Does {name}.\n---\n\nbody\n"),
    )
    .unwrap();
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

#[test]
fn skill_grant_to_a_workspace_reaches_its_chats_and_no_others() {
    let project = Project::new();
    assert!(
        project
            .state_in("alpha", "deploy")
            .starts_with("not granted")
    );

    let out = project.run(&["skill", "grant", "deploy", "alpha"]);

    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(
        std::fs::read_to_string(project.root().join("skills/deploy.toml")).unwrap(),
        "grant = [\"alpha\"]\n"
    );
    assert_eq!(project.state_in("alpha", "deploy"), "on");
    assert!(
        project
            .state_in("beta", "deploy")
            .starts_with("not granted")
    );

    let out = project.run(&["skill", "grant", "deploy", "all"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(project.state_in("beta", "deploy"), "on");

    let out = project.run(&["skill", "grant", "deploy", "root"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(!project.root().join("skills/deploy.toml").exists());
    assert!(
        project
            .state_in("alpha", "deploy")
            .starts_with("not granted")
    );
}

#[test]
fn skill_grant_refuses_a_workspace_the_project_does_not_have() {
    let project = Project::new();

    let out = project.run(&["skill", "grant", "deploy", "alpah"]);

    assert_eq!(out.status.code(), Some(1));
    assert!(
        stderr(&out).contains("no workspace \"alpah\""),
        "{}",
        stderr(&out)
    );
    assert!(!project.root().join("skills/deploy.toml").exists());
}

#[test]
fn skill_grant_without_where_it_reaches_says_how_to_say_it() {
    let project = Project::new();

    let out = project.run(&["skill", "grant", "deploy"]);

    assert_eq!(out.status.code(), Some(2));
    assert!(!stderr(&out).contains("master"));
    assert!(
        stderr(&out).contains("root (the project's root only)"),
        "{}",
        stderr(&out)
    );
}

#[test]
fn a_workspace_turns_a_granted_skill_off_and_on_but_never_on_one_it_was_not_granted() {
    let project = Project::new();

    let out = project.run(&["skill", "grant", "deploy", "-w", "alpha", "--on"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(
        stderr(&out).contains("deploy is not granted to alpha, so it cannot be turned on there"),
        "{}",
        stderr(&out)
    );
    assert!(
        !project
            .root()
            .join("workspaces/alpha/workspace.json")
            .exists()
    );

    assert!(
        project
            .run(&["skill", "grant", "deploy", "all"])
            .status
            .success()
    );
    let out = project.run(&["skill", "grant", "deploy", "-w", "alpha", "--off"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(
        project.state_in("alpha", "deploy"),
        "turned off in this workspace"
    );
    assert_eq!(project.state_in("beta", "deploy"), "on");

    let out = project.run(&["skill", "grant", "deploy", "-w", "alpha", "--on"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(project.state_in("alpha", "deploy"), "on");
}

#[test]
fn skill_share_moves_a_local_skill_into_the_shared_folder() {
    let project = Project::new();
    write_skill(
        &project.root().join("workspaces/alpha/skills.local"),
        "notes",
    );
    assert_eq!(project.state_in("alpha", "notes"), "on");

    let out = project.run(&["skill", "share", "notes", "-w", "alpha"]);

    assert!(out.status.success(), "{}", stderr(&out));
    assert!(
        project
            .root()
            .join("workspaces/alpha/skills/notes/SKILL.md")
            .is_file()
    );
    assert!(
        !project
            .root()
            .join("workspaces/alpha/skills.local/notes")
            .exists()
    );
    let listed = project.run(&["skill", "list", "-w", "alpha"]);
    let text = String::from_utf8_lossy(&listed.stdout).into_owned();
    let line = text.lines().find(|l| l.starts_with("notes")).unwrap();
    assert!(
        line.contains("workspace") && !line.contains("local"),
        "{line}"
    );
}

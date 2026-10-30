// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis skill new | edit | mv | rm | restore` on a temporary project (SK-7, #2770): the verbs
//! the Skill studio calls, from the command line. Behind the `skill-studio` flag. The project is
//! no git repository, so each change is made on disk and left for the next save; that each one
//! is one commit is the core's seam test (`skills/verbs/tests.rs`).

use crate::support;

use std::path::PathBuf;
use std::process::Output;

struct Project {
    _tmp: tempfile::TempDir,
    at: PathBuf,
}

impl Project {
    /// A project with a workspace `alpha` and a persona `steward` that names `deploy`.
    fn new() -> Project {
        let tmp = tempfile::tempdir().unwrap();
        let at = tmp.path().canonicalize().unwrap();
        let root = at.join("plane");
        std::fs::create_dir_all(root.join("workspaces/alpha")).unwrap();
        std::fs::create_dir_all(root.join("personas/steward")).unwrap();
        std::fs::write(
            root.join("personas/steward/persona.md"),
            "---\nname: steward\nskills: deploy\n---\n\nSteward.\n",
        )
        .unwrap();
        std::fs::create_dir_all(at.join("home")).unwrap();
        std::fs::write(root.join("charter.toml"), "[plane]\n").unwrap();
        // The project's app state, made before purlis was ever used there (#3718), in the data
        // home every run below is given: nothing of it waits for an Accept.
        purlis_core::appstate::made_in_for_tests(&at.join("data"), &root).unwrap();
        Project { _tmp: tmp, at }
    }

    fn root(&self) -> PathBuf {
        self.at.join("plane")
    }

    fn run_with(&self, args: &[&str], env: &[(&str, &str)]) -> Output {
        let root = self.root();
        let mut command = support::purlis();
        command
            .args(args)
            .current_dir(&root)
            .env("CHARTER_ROOT", &root)
            .env("HOME", self.at.join("home"))
            .env("PURLIS_CONFIG_HOME", self.at.join("home/config"))
            .env("PURLIS_DATA_HOME", self.at.join("data"))
            .env("PURLIS_SKILL_STUDIO", "1")
            .env("NO_COLOR", "1")
            .env_remove("VISUAL")
            .env_remove("EDITOR");
        // Run from a terminal, even where this test runs inside a chat.
        for spelling in purlis_core::envvar::spellings("PURLIS_SESSION_ID") {
            command.env_remove(spelling);
        }
        for (key, value) in env {
            command.env(key, value);
        }
        command.output().unwrap()
    }

    fn run(&self, args: &[&str]) -> Output {
        self.run_with(args, &[])
    }

    /// The names a chat at the root (`None`) or in a workspace gets.
    fn gets(&self, workspace: Option<&str>) -> Vec<String> {
        self.gets_as(workspace, None)
    }

    /// The names a chat at the root (`None`) or in a workspace gets, as `persona`.
    fn gets_as(&self, workspace: Option<&str>, persona: Option<&str>) -> Vec<String> {
        let mut args = vec!["skill", "list", "--json"];
        if let Some(ws) = workspace {
            args.extend(["-w", ws]);
        }
        if let Some(persona) = persona {
            args.extend(["-p", persona]);
        }
        let out = self.run(&args);
        assert!(out.status.success(), "{}", stderr(&out));
        let said: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        said["skills"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|row| row["on"] == true && row["level"] != "built-in")
            .map(|row| row["name"].as_str().unwrap().to_owned())
            .collect()
    }
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

#[test]
fn skill_new_makes_a_skill_from_a_template_at_the_level_named() {
    let project = Project::new();

    let out = project.run(&[
        "skill",
        "new",
        "triage",
        "-d",
        "Sorts new issues.",
        "--template",
        "steps",
        "-w",
        "alpha",
    ]);

    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stderr(&out).contains("triage is made"), "{}", stderr(&out));
    assert_eq!(project.gets(Some("alpha")), ["triage"]);
    assert!(project.gets(None).is_empty());
    let text = std::fs::read_to_string(
        project
            .root()
            .join("workspaces/alpha/skills/triage/SKILL.md"),
    )
    .unwrap();
    assert!(text.contains("## Steps"), "{text}");

    let out = project.run(&["skill", "new", "Bad Name", "-d", "x."]);
    assert_eq!(out.status.code(), Some(1));
    assert!(
        stderr(&out).contains("cannot be a skill's name"),
        "{}",
        stderr(&out)
    );
}

#[test]
fn skill_edit_saves_a_whole_skill_and_refuses_one_the_lint_finds_wrong() {
    let project = Project::new();
    assert!(
        project
            .run(&["skill", "new", "deploy", "-d", "Ships it."])
            .status
            .success()
    );
    let draft = project.at.join("draft.md");

    std::fs::write(
        &draft,
        "---\nname: deploy\ndescription: Ships it, carefully.\n---\n\nGo.\n",
    )
    .unwrap();
    let out = project.run(&["skill", "edit", "deploy", "--from", draft.to_str().unwrap()]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(
        std::fs::read_to_string(project.root().join("skills/deploy/SKILL.md"))
            .unwrap()
            .contains("Ships it, carefully.")
    );

    std::fs::write(&draft, "---\nname: deploy\n---\n").unwrap();
    let out = project.run(&["skill", "edit", "deploy", "--from", draft.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("no description"), "{}", stderr(&out));
}

#[test]
fn skill_mv_renames_everywhere_and_a_move_into_the_project_asks_for_a_grant() {
    let project = Project::new();
    assert!(
        project
            .run(&["skill", "new", "deploy", "-d", "Ships it.", "-w", "alpha"])
            .status
            .success()
    );

    let out = project.run(&["skill", "mv", "deploy", "-w", "alpha", "--name", "ship"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(project.gets(Some("alpha")), ["ship"]);
    assert!(
        std::fs::read_to_string(project.root().join("personas/steward/persona.md"))
            .unwrap()
            .contains("skills: ship")
    );

    let out = project.run(&["skill", "mv", "ship", "-w", "alpha", "--to", "root"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("--grant"), "{}", stderr(&out));

    let out = project.run(&[
        "skill", "mv", "ship", "-w", "alpha", "--to", "root", "--grant", "alpha",
    ]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(
        std::fs::read_to_string(project.root().join("skills/ship.toml")).unwrap(),
        "grant = [\"alpha\"]\n"
    );
    assert_eq!(project.gets(None), ["ship"]);
    assert_eq!(project.gets(Some("alpha")), ["ship"]);
}

#[test]
fn skill_rm_says_who_uses_it_and_restore_brings_it_back() {
    let project = Project::new();
    assert!(
        project
            .run(&["skill", "new", "deploy", "-d", "Ships it."])
            .status
            .success()
    );

    let out = project.run(&["skill", "rm", "deploy"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let said = stderr(&out);
    assert!(said.contains("chats at the project's root"), "{said}");
    assert!(said.contains("Named by 1 persona: steward"), "{said}");
    assert!(project.gets(None).is_empty());

    let out = project.run(&["skill", "restore"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(String::from_utf8_lossy(&out.stdout).contains("deploy  project"));

    let out = project.run(&["skill", "restore", "deploy"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(project.gets(None), ["deploy"]);
}

#[test]
fn a_chat_does_not_change_a_skill_itself() {
    let project = Project::new();

    let out = project.run_with(
        &["skill", "new", "deploy", "-d", "Ships it."],
        &[("PURLIS_SESSION_ID", "a-chat")],
    );

    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("draft"), "{}", stderr(&out));
    assert!(!project.root().join("skills/deploy").exists());
}

#[test]
fn a_personas_own_skill_is_made_moved_deleted_restored_and_shared_with_p() {
    // SK-32 (#3003): every verb at a persona's level, named with `-p`.
    let project = Project::new();
    let made = project.run(&["skill", "new", "lint", "-d", "Lints it.", "-p", "steward"]);
    assert!(made.status.success(), "{}", stderr(&made));
    assert!(
        project
            .root()
            .join("personas/steward/skills/lint/SKILL.md")
            .exists()
    );
    assert_eq!(project.gets_as(Some("alpha"), Some("steward")), ["lint"]);
    assert!(project.gets(Some("alpha")).is_empty());

    let out = project.run(&["skill", "new", "x", "-d", "Does x.", "-p", "nobody"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("no persona"), "{}", stderr(&out));

    // Promoting it asks for a grant; the persona keeps it in every workspace by its list.
    let out = project.run(&["skill", "mv", "lint", "-p", "steward", "--to", "root"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("--grant"), "{}", stderr(&out));
    let out = project.run(&[
        "skill", "mv", "lint", "-p", "steward", "--to", "root", "--grant", "root",
    ]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(project.root().join("skills/lint/SKILL.md").exists());
    assert!(
        std::fs::read_to_string(project.root().join("personas/steward/persona.md"))
            .unwrap()
            .contains("skills: deploy, lint")
    );
    assert_eq!(project.gets_as(Some("alpha"), Some("steward")), ["lint"]);
    assert!(project.gets(Some("alpha")).is_empty());

    // And back: the persona's own again, and out of its list.
    let out = project.run(&["skill", "mv", "lint", "--to-persona", "steward"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(
        project
            .root()
            .join("personas/steward/skills/lint/SKILL.md")
            .exists()
    );
    assert!(
        std::fs::read_to_string(project.root().join("personas/steward/persona.md"))
            .unwrap()
            .contains("skills: deploy\n")
    );

    let out = project.run(&["skill", "rm", "lint", "-p", "steward"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(
        stderr(&out).contains("chats as steward, in every workspace"),
        "{}",
        stderr(&out)
    );
    assert!(project.gets_as(None, Some("steward")).is_empty());
    let out = project.run(&["skill", "restore"]);
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("lint  persona steward"),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    let out = project.run(&["skill", "restore", "lint"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(project.gets_as(None, Some("steward")), ["lint"]);

    let out = project.run(&[
        "skill",
        "new",
        "notes",
        "-d",
        "Keeps notes.",
        "-p",
        "steward",
        "--local",
    ]);
    assert!(out.status.success(), "{}", stderr(&out));
    let out = project.run(&["skill", "share", "notes", "-p", "steward"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(
        project
            .root()
            .join("personas/steward/skills/notes/SKILL.md")
            .exists()
    );
}

/// Spec #3679 S12: `purlis skill` refuses a built-in skill's name as the window does, in the
/// core's words, whether or not a bundle lies beside the binary.
#[test]
fn skill_new_and_mv_refuse_a_built_in_skills_name() {
    let project = Project::new();
    let reserved = "That name belongs to a built-in skill; pick another, or make a copy.";

    let out = project.run(&["skill", "new", "secrets", "-d", "Stands in.", "-w", "alpha"]);
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert!(stderr(&out).contains(reserved), "{}", stderr(&out));
    assert!(
        !project
            .root()
            .join("workspaces/alpha/skills/secrets")
            .exists()
    );

    assert!(
        project
            .run(&["skill", "new", "deploy", "-d", "Ships it.", "-w", "alpha"])
            .status
            .success()
    );
    let out = project.run(&["skill", "mv", "deploy", "-w", "alpha", "--name", "browser"]);
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert!(stderr(&out).contains(reserved), "{}", stderr(&out));
    assert_eq!(project.gets(Some("alpha")), ["deploy"]);
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis skill list | show` beside the studio (SK-34, #3008): the skills each harness finds
//! itself, in the person's own folders and in a workspace's clones, with whose each is and
//! whether a chat gets it, read only, as the Skill studio lists them (spec #2762 decision 26).

mod support;

use std::path::{Path, PathBuf};
use std::process::Output;

struct Project {
    _tmp: tempfile::TempDir,
    /// The temporary folder, as the kernel names it (`/private/tmp` on macOS).
    at: PathBuf,
}

impl Project {
    /// A project with one project skill and a workspace `alpha` holding a clone `app`; a home
    /// whose Claude Code folder holds two skills of the person's own, one of them turned off for
    /// the project's Claude Code chats; and the clone carrying a skill in its `.claude/skills`.
    fn new() -> Project {
        let tmp = tempfile::tempdir().unwrap();
        let at = tmp.path().canonicalize().unwrap();
        let root = at.join("plane");
        std::fs::create_dir_all(root.join("workspaces/alpha/app/.git")).unwrap();
        std::fs::write(
            root.join("charter.toml"),
            "[plane]\n\n[harness_skills.claude]\nquiet = false\n",
        )
        .unwrap();
        write_skill(&root.join("skills"), "deploy", "Deploys.");
        let home = at.join("home");
        write_skill(&home.join(".claude/skills"), "mine", "Mine alone.");
        write_skill(&home.join(".claude/skills"), "quiet", "Says little.");
        write_skill(
            &root.join("workspaces/alpha/app/.claude/skills"),
            "tool",
            "The repo's tool.",
        );
        Project { _tmp: tmp, at }
    }

    fn run(&self, args: &[&str]) -> Output {
        let root = self.at.join("plane");
        support::purlis()
            .args(args)
            .current_dir(&root)
            .env("CHARTER_ROOT", &root)
            .env("HOME", self.at.join("home"))
            .env("PURLIS_CONFIG_HOME", self.at.join("home/config"))
            .env("PURLIS_SKILL_STUDIO", "1")
            .env("NO_COLOR", "1")
            .env_remove("CLAUDE_CONFIG_DIR")
            .env_remove("CODEX_HOME")
            .env_remove("OPENCODE_CONFIG_DIR")
            .env_remove("XDG_CONFIG_HOME")
            .output()
            .unwrap()
    }
}

fn write_skill(dir: &Path, name: &str, description: &str) {
    let dir = dir.join(name);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: {description}\n---\n\nbody\n"),
    )
    .unwrap();
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

/// The lines of `text`'s section headed `heading`, up to the next blank line.
fn section<'a>(text: &'a str, heading: &str) -> Vec<&'a str> {
    text.lines()
        .skip_while(|line| *line != heading)
        .skip(1)
        .take_while(|line| !line.is_empty())
        .collect()
}

/// The row of `name` in `lines`, its columns joined by one space.
fn row(lines: &[&str], name: &str) -> String {
    lines
        .iter()
        .find(|line| line.split_whitespace().next() == Some(name))
        .unwrap_or_else(|| panic!("{name} listed: {lines:#?}"))
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn skill_list_shows_each_harness_s_own_skills_with_whose_and_whether_on() {
    let project = Project::new();

    let out = project.run(&["skill", "list", "-w", "alpha"]);

    assert!(out.status.success(), "{}", stderr(&out));
    let text = stdout(&out);
    let claude = section(&text, "Claude Code finds these itself, read only:");
    assert_eq!(row(&claude, "SKILL"), "SKILL LEVEL OWNER STATE");
    assert_eq!(row(&claude, "mine"), "mine installed you on");
    assert_eq!(
        row(&claude, "quiet"),
        "quiet installed you left out: turned off for this project's chats"
    );
    assert_eq!(row(&claude, "tool"), "tool repo repo app on");
    let codex = section(&text, "Codex finds these itself, read only:");
    assert!(
        codex.is_empty(),
        "Codex finds none of these, so it has no section: {text}"
    );
}

#[test]
fn skill_list_at_the_root_lists_no_repo_skills() {
    let project = Project::new();

    let out = project.run(&["skill", "list"]);

    assert!(out.status.success(), "{}", stderr(&out));
    let text = stdout(&out);
    let claude = section(&text, "Claude Code finds these itself, read only:");
    assert_eq!(row(&claude, "mine"), "mine installed you on");
    assert!(
        !claude.iter().any(|line| line.starts_with("tool")),
        "a chat at the root works in no clone: {text}"
    );
}

#[test]
fn skill_list_json_gives_each_harness_its_skills_with_owner_and_switch() {
    let project = Project::new();

    let out = project.run(&["skill", "list", "-w", "alpha", "--json"]);

    assert!(out.status.success(), "{}", stderr(&out));
    let doc: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
    let claude = doc["harnesses"]
        .as_array()
        .unwrap()
        .iter()
        .find(|one| one["harness"] == "claude")
        .expect("Claude Code listed");
    assert_eq!(claude["title"], "Claude Code");
    let skill = |name: &str| {
        claude["skills"]
            .as_array()
            .unwrap()
            .iter()
            .find(|skill| skill["name"] == name)
            .unwrap_or_else(|| panic!("{name} listed: {claude}"))
            .clone()
    };
    let tool = skill("tool");
    assert_eq!(tool["level"], "repo");
    assert_eq!(tool["owner"], serde_json::json!({"kind": "repo", "name": "app"}));
    assert_eq!(tool["switch"], serde_json::json!({"kind": "alone"}));
    assert_eq!(tool["on"], true);
    let quiet = skill("quiet");
    assert_eq!(quiet["owner"], serde_json::json!({"kind": "you"}));
    assert_eq!(quiet["on"], false);
    assert_eq!(quiet["left_out"], "turned off for this project's chats");
    assert!(
        doc["skills"]
            .as_array()
            .unwrap()
            .iter()
            .any(|skill| skill["name"] == "deploy"),
        "the project's own still listed: {doc}"
    );
}

#[test]
fn skill_show_covers_a_repo_skill_with_its_harness_and_owner() {
    let project = Project::new();

    let out = project.run(&["skill", "show", "tool", "-w", "alpha"]);

    assert!(out.status.success(), "{}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("  level: repo\n"), "{text}");
    assert!(text.contains("  harness: Claude Code\n"), "{text}");
    assert!(text.contains("  owner: repo app\n"), "{text}");
    assert!(
        text.contains("  folder: workspaces/alpha/app/.claude/skills/tool\n"),
        "{text}"
    );
    assert!(text.contains("  in this chat's skills: on\n"), "{text}");
}

#[test]
fn skill_show_covers_a_skill_of_the_person_s_own_on_each_harness_that_finds_it() {
    let project = Project::new();

    let out = project.run(&["skill", "show", "quiet", "--json"]);

    assert!(out.status.success(), "{}", stderr(&out));
    let doc: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
    let copies = doc["copies"].as_array().unwrap();
    let on_claude = copies
        .iter()
        .find(|copy| copy["harness"] == "claude")
        .unwrap_or_else(|| panic!("Claude Code's copy shown: {doc}"));
    assert_eq!(on_claude["level"], "installed");
    assert_eq!(on_claude["owner"], serde_json::json!({"kind": "you"}));
    assert_eq!(on_claude["on"], false);
}

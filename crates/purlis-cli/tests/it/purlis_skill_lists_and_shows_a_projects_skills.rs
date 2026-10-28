// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis skill list | show` on a temporary project (SK-2, #2765): the project's skills with
//! their level, and every one left out with why, behind the `skill-studio` flag.

use crate::support;

use std::path::{Path, PathBuf};
use std::process::Output;

struct Project {
    _tmp: tempfile::TempDir,
    /// The temporary folder, as the kernel names it (`/private/tmp` on macOS).
    at: PathBuf,
}

impl Project {
    /// A project with two skills in `skills/`, a folder that is not one, and a workspace.
    fn new() -> Project {
        let tmp = tempfile::tempdir().unwrap();
        let at = tmp.path().canonicalize().unwrap();
        let root = at.join("plane");
        std::fs::create_dir_all(root.join("workspaces/alpha")).unwrap();
        std::fs::create_dir_all(at.join("home")).unwrap();
        std::fs::write(root.join("charter.toml"), "[plane]\n").unwrap();
        for name in ["deploy", "review"] {
            write_skill(&root, name, &format!("Does {name}."));
        }
        std::fs::create_dir_all(root.join("skills/half")).unwrap();
        std::fs::write(root.join("skills/half/SKILL.md"), "---\nname: half\n---\n").unwrap();
        Project { _tmp: tmp, at }
    }

    fn root(&self) -> PathBuf {
        self.at.join("plane")
    }

    fn run(&self, args: &[&str], studio: &str) -> Output {
        let root = self.root();
        support::purlis()
            .args(args)
            .current_dir(&root)
            .env("CHARTER_ROOT", &root)
            .env("HOME", self.at.join("home"))
            .env("PURLIS_CONFIG_HOME", self.at.join("home/config"))
            .env("PURLIS_SKILL_STUDIO", studio)
            .env("NO_COLOR", "1")
            .output()
            .unwrap()
    }
}

fn write_skill(root: &Path, name: &str, description: &str) {
    let dir = root.join("skills").join(name);
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

#[test]
fn skill_list_shows_each_project_skill_with_its_level_at_the_plane_root() {
    let project = Project::new();

    let out = project.run(&["skill", "list"], "1");

    assert!(out.status.success(), "{}", stderr(&out));
    let text = stdout(&out);
    let line = |name: &str| {
        text.lines()
            .find(|line| line.split_whitespace().next() == Some(name))
            .unwrap_or_else(|| panic!("{name} listed: {text}"))
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    };
    assert_eq!(line("deploy"), "deploy project on");
    assert_eq!(line("review"), "review project on");
    assert_eq!(
        line("half"),
        "half project left out: not a skill: its SKILL.md names no `description`"
    );
}

#[test]
fn skill_list_for_a_workspace_says_a_project_skill_was_not_granted_there() {
    let project = Project::new();

    let out = project.run(&["skill", "list", "-w", "alpha", "--json"], "1");

    assert!(out.status.success(), "{}", stderr(&out));
    let doc: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
    let deploy = doc["skills"]
        .as_array()
        .unwrap()
        .iter()
        .find(|skill| skill["name"] == "deploy")
        .expect("deploy listed");
    assert_eq!(deploy["level"], "project");
    assert_eq!(deploy["on"], false);
    assert_eq!(
        deploy["left_out"],
        "not granted to this workspace: a project skill is for chats at the project's root until \
         the project grants it"
    );
}

#[test]
fn skill_show_prints_one_skill_and_exits_2_for_a_name_no_skill_has() {
    let project = Project::new();

    let out = project.run(&["skill", "show", "deploy"], "1");
    assert!(out.status.success(), "{}", stderr(&out));
    let text = stdout(&out);
    assert!(text.starts_with("deploy\n"), "{text}");
    assert!(text.contains("level: project"), "{text}");
    assert!(text.contains("folder: skills/deploy"), "{text}");
    assert!(text.contains("description: Does deploy."), "{text}");
    assert!(text.contains("in this chat's skills: on"), "{text}");

    let missing = project.run(&["skill", "show", "nothing"], "1");
    assert_eq!(missing.status.code(), Some(2));
    assert!(stderr(&missing).contains("purlis skill list"));
}

#[test]
fn skill_show_shows_every_copy_of_a_name_and_why_the_hidden_one_is_left_out() {
    let project = Project::new();
    // A second folder naming `deploy`, after `deploy/` by folder name.
    let dir = project.root().join("skills/deploy-copy");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("SKILL.md"),
        "---\nname: deploy\ndescription: Deploys again.\n---\n",
    )
    .unwrap();

    let out = project.run(&["skill", "show", "deploy", "--json"], "1");

    assert!(out.status.success(), "{}", stderr(&out));
    let doc: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
    let copies: Vec<(&str, bool, &str)> = doc["copies"]
        .as_array()
        .unwrap()
        .iter()
        .map(|copy| {
            (
                copy["folder"].as_str().unwrap(),
                copy["on"].as_bool().unwrap(),
                copy["left_out"].as_str().unwrap_or(""),
            )
        })
        .collect();
    assert_eq!(
        copies,
        [
            ("skills/deploy", true, ""),
            (
                "skills/deploy-copy",
                false,
                "hidden by another project skill of this name"
            ),
        ]
    );
}

#[test]
fn skill_list_says_the_studio_is_not_on_while_its_flag_is_off_and_writes_nothing() {
    let project = Project::new();

    let out = project.run(&["skill", "list"], "0");

    assert_eq!(out.status.code(), Some(1));
    assert!(
        stderr(&out).contains("Settings › You › Previews"),
        "{}",
        stderr(&out)
    );
    assert!(
        stderr(&out).contains("PURLIS_SKILL_STUDIO=1"),
        "{}",
        stderr(&out)
    );
    assert!(stdout(&out).is_empty());
    assert!(!project.root().join(".claude").exists());
}

/// SK-47: the project's own skills table pads a name as it is shown. The padding escapes it, so
/// a name that reorders or hides its own characters cannot disguise its row, and the next column
/// still starts where every other row's does.
#[test]
fn a_project_skill_named_with_bidi_and_zero_width_characters_is_listed_escaped_and_lined_up() {
    let project = Project::new();
    let dir = project.root().join("skills/sly");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("SKILL.md"),
        "---\nname: sly\u{202e}e\u{200b}man\ndescription: Sly.\n---\n\nbody\n",
    )
    .unwrap();

    let out = project.run(&["skill", "list"], "1");

    assert!(out.status.success(), "{}", stderr(&out));
    let text = stdout(&out);
    assert!(!text.contains(['\u{202e}', '\u{200b}']), "{text:?}");
    let shown = "sly⟦U+202E⟧e⟦U+200B⟧man";
    let find = |start: &str| {
        text.lines()
            .find(|line| line.starts_with(start))
            .unwrap_or_else(|| panic!("{start} listed: {text}"))
    };
    let level = |line: &str, name: &str| {
        let name = name.chars().count();
        name + line.chars().skip(name).position(|c| c != ' ').unwrap()
    };
    assert_eq!(
        level(find(shown), shown),
        level(find("deploy "), "deploy"),
        "{text}"
    );
}

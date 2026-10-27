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
        self.command(args).output().unwrap()
    }

    /// [`Project::run`], with Claude Code's folder at `claude-config` in place of the home's.
    fn run_with_config(&self, args: &[&str]) -> Output {
        self.command(args)
            .env("CLAUDE_CONFIG_DIR", self.at.join("claude-config"))
            .output()
            .unwrap()
    }

    fn command(&self, args: &[&str]) -> std::process::Command {
        let root = self.at.join("plane");
        let mut command = support::purlis();
        command
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
            .env_remove("XDG_CONFIG_HOME");
        command
    }

    fn clone_skills(&self) -> PathBuf {
        self.at.join("plane/workspaces/alpha/app/.claude/skills")
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
    assert_eq!(
        tool["owner"],
        serde_json::json!({"kind": "repo", "name": "app"})
    );
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

#[test]
fn a_plugins_skill_is_listed_as_the_plugins_and_show_says_it_goes_only_with_its_plugin() {
    let project = Project::new();
    let config = project.at.join("claude-config");
    let cache = project.at.join("cache/figma/2.2");
    write_skill(&cache.join("skills"), "frame", "Frames.");
    std::fs::create_dir_all(config.join("plugins")).unwrap();
    std::fs::write(
        config.join("plugins/installed_plugins.json"),
        serde_json::json!({
            "version": 2,
            "plugins": { "figma@official": [{ "scope": "user", "installPath": cache }] }
        })
        .to_string(),
    )
    .unwrap();

    let list = project.run_with_config(&["skill", "list"]);
    let show = project.run_with_config(&["skill", "show", "figma:frame"]);

    assert!(list.status.success(), "{}", stderr(&list));
    let text = stdout(&list);
    let claude = section(&text, "Claude Code finds these itself, read only:");
    assert_eq!(
        row(&claude, "figma:frame"),
        "figma:frame installed figma on"
    );
    assert!(show.status.success(), "{}", stderr(&show));
    let text = stdout(&show);
    assert!(text.contains("  owner: figma\n"), "{text}");
    assert!(
        text.contains(
            "  switch: it comes with the plugin figma, which Claude Code turns on or off itself, so its skills are shown read-only\n"
        ),
        "{text}"
    );
}

#[cfg(unix)]
#[test]
fn a_repo_skill_folder_that_is_a_link_is_listed_by_its_name_and_never_read_through() {
    let project = Project::new();
    let elsewhere = project.at.join("elsewhere");
    write_skill(&elsewhere, "secret", "What lies elsewhere.");
    std::os::unix::fs::symlink(
        elsewhere.join("secret"),
        project.clone_skills().join("linked"),
    )
    .unwrap();

    let list = project.run(&["skill", "list", "-w", "alpha"]);
    let show = project.run(&["skill", "show", "linked", "-w", "alpha"]);

    assert!(list.status.success(), "{}", stderr(&list));
    let text = stdout(&list);
    let claude = section(&text, "Claude Code finds these itself, read only:");
    assert!(
        row(&claude, "linked").starts_with("linked repo repo app "),
        "{text}"
    );
    assert!(show.status.success(), "{}", stderr(&show));
    let text = stdout(&show);
    assert!(text.contains("does not follow"), "{text}");
    assert!(
        !text.contains("What lies elsewhere."),
        "what the link leads to is never read: {text}"
    );
}

#[test]
fn a_repo_skills_control_and_bidi_characters_are_escaped_in_list_and_show() {
    let project = Project::new();
    let dir = project.clone_skills().join("sly");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("SKILL.md"),
        "---\nname: sly\u{1b}[31m\u{202e}eman\ndescription: Says\u{200b} hi\u{1b}[2J.\n---\n\nbody\n",
    )
    .unwrap();
    let name = "sly\u{1b}[31m\u{202e}eman";

    let list = project.run(&["skill", "list", "-w", "alpha"]);
    let show = project.run(&["skill", "show", name, "-w", "alpha"]);

    for out in [&list, &show] {
        assert!(out.status.success(), "{}", stderr(out));
        let text = stdout(out);
        assert!(
            !text.contains(['\u{1b}', '\u{202e}', '\u{200b}']),
            "nothing hidden reaches the terminal: {text:?}"
        );
        assert!(text.contains("sly⟦U+001B⟧[31m⟦U+202E⟧eman"), "{text}");
    }
    assert!(
        stdout(&show).contains("  description: Says⟦U+200B⟧ hi⟦U+001B⟧[2J.\n"),
        "{}",
        stdout(&show)
    );
}

/// SK-47 (#3150): the table pads a name as it is shown, so a name that reorders or hides its own
/// characters is escaped by the padding itself, and its row still lines up with the others.
#[test]
fn a_skill_named_with_bidi_and_zero_width_characters_is_listed_escaped_and_lined_up() {
    let project = Project::new();
    write_skill(&project.clone_skills(), "sly", "Sly.");
    let path = project.clone_skills().join("sly/SKILL.md");
    std::fs::write(
        &path,
        "---\nname: sly\u{202e}e\u{200b}man\ndescription: Sly.\n---\n\nbody\n",
    )
    .unwrap();

    let out = project.run(&["skill", "list", "-w", "alpha"]);

    assert!(out.status.success(), "{}", stderr(&out));
    let text = stdout(&out);
    assert!(
        !text.contains(['\u{202e}', '\u{200b}']),
        "no character reorders or hides the table: {text:?}"
    );
    let claude = section(&text, "Claude Code finds these itself, read only:");
    let shown = "sly⟦U+202E⟧e⟦U+200B⟧man";
    let sly = claude
        .iter()
        .find(|line| line.starts_with(shown))
        .unwrap_or_else(|| panic!("{shown} listed: {claude:#?}"));
    let tool = claude
        .iter()
        .find(|line| line.starts_with("tool "))
        .unwrap_or_else(|| panic!("tool listed: {claude:#?}"));
    // Every character here is one column wide, so a column starts at the same character count:
    // the width was measured on the escaped name, not on the one the file holds.
    let level = |line: &str, name: &str| {
        let name = name.chars().count();
        name + line.chars().skip(name).position(|c| c != ' ').unwrap()
    };
    assert_eq!(
        level(sly, shown),
        level(tool, "tool"),
        "the LEVEL column lines up:\n{sly}\n{tool}"
    );
}

#[test]
fn a_repo_skills_folder_purlis_cannot_list_whole_is_one_row_that_says_why() {
    // SK-44 (#3133): past the most names purlis reads of one folder, none is read.
    let project = Project::new();
    for n in 0..2000 {
        std::fs::write(project.clone_skills().join(format!("f{n}")), "").unwrap();
    }

    let out = project.run(&["skill", "list", "-w", "alpha", "--json"]);

    assert!(out.status.success(), "{}", stderr(&out));
    let text = stdout(&out);
    assert!(!text.contains("left out: left out"), "{text}");
    let doc: serde_json::Value = serde_json::from_str(&text).unwrap();
    let claude = doc["harnesses"]
        .as_array()
        .unwrap()
        .iter()
        .find(|one| one["harness"] == "claude")
        .expect("Claude Code listed");
    let repo: Vec<(&str, &str)> = claude["skills"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|skill| skill["level"] == "repo")
        .map(|skill| {
            (
                skill["name"].as_str().unwrap(),
                skill["description"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        repo,
        [(
            "skills",
            "A folder of more than 2000 names, the most purlis reads of one, so none of its \
             skills is read here. A chat still finds them."
        )]
    );
}

#[test]
fn skill_list_says_under_codex_and_opencode_which_skills_a_repo_leaves_out_and_why() {
    // SK-53 (#3213), decision 41: a Codex or opencode chat in `app` gets the repo's `deploy` in
    // place of the workspace's (decision 51), and the list says so under each, once.
    let project = Project::new();
    let root = project.at.join("plane");
    write_skill(
        &root.join("workspaces/alpha/skills"),
        "deploy",
        "Deploys alpha.",
    );
    write_skill(
        &root.join("workspaces/alpha/app/.agents/skills"),
        "deploy",
        "The repo's deploy.",
    );

    let out = project.run(&["skill", "list", "-w", "alpha"]);
    let json = project.run(&["skill", "list", "-w", "alpha", "--json"]);

    assert!(out.status.success(), "{}", stderr(&out));
    let text = stdout(&out);
    assert!(!text.contains("left out: left out"), "{text}");
    for title in ["Codex", "opencode"] {
        let lost = section(
            &text,
            &format!("{title} chats in a repo here start without these:"),
        );
        assert_eq!(row(&lost, "SKILL"), "SKILL LEVEL STATE", "{text}");
        assert_eq!(
            row(&lost, "deploy"),
            "deploy workspace left out: hidden by the skill of this name in the repo app",
            "{text}"
        );
    }
    assert!(
        section(
            &text,
            "Claude Code chats in a repo here start without these:"
        )
        .is_empty(),
        "Claude Code's chats get the workspace's copy too: {text}"
    );
    assert!(json.status.success(), "{}", stderr(&json));
    let doc: serde_json::Value = serde_json::from_str(&stdout(&json)).unwrap();
    let codex = doc["harnesses"]
        .as_array()
        .unwrap()
        .iter()
        .find(|one| one["harness"] == "codex")
        .expect("Codex listed");
    assert_eq!(codex["repo_left_out"][0]["name"], "deploy");
    assert_eq!(codex["repo_left_out"][0]["level"], "workspace");
    assert_eq!(
        codex["repo_left_out"][0]["left_out"],
        "hidden by the skill of this name in the repo app"
    );
}

#[test]
fn skill_list_marks_a_project_skill_a_repo_has_a_copy_of_as_the_studio_does() {
    // SK-46 (#3149), decision 51: the clone `app` has its own `deploy` for every program, so the
    // project's `deploy` clashes on each, and the list says so on its row, in the studio's words.
    let project = Project::new();
    let root = project.at.join("plane");
    // Granted to `alpha`, so a chat there gets the project's copy.
    std::fs::write(root.join("skills/deploy.toml"), "grant = \"all\"\n").unwrap();
    write_skill(&project.clone_skills(), "deploy", "The repo's deploy.");
    write_skill(
        &root.join("workspaces/alpha/app/.agents/skills"),
        "deploy",
        "The repo's deploy, for Codex and opencode.",
    );

    let out = project.run(&["skill", "list", "-w", "alpha"]);
    let json = project.run(&["skill", "list", "-w", "alpha", "--json"]);
    let show = project.run(&["skill", "show", "deploy", "-w", "alpha"]);

    assert!(out.status.success(), "{}", stderr(&out));
    let text = stdout(&out);
    let deploy = row(&text.lines().collect::<Vec<_>>(), "deploy");
    let both = "the repo app has a skill of this name too, and a chat there gets both";
    let alone = |title: &str| {
        format!(
            "the repo app has a skill of this name too, and {title} chats there get the repo's copy alone"
        )
    };
    let squeezed = |said: &str| said.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        deploy.starts_with(&format!("deploy project on; also: {both}")),
        "{text}"
    );
    for title in ["Codex", "opencode"] {
        assert!(deploy.contains(&squeezed(&alone(title))), "{text}");
    }

    assert!(json.status.success(), "{}", stderr(&json));
    let doc: serde_json::Value = serde_json::from_str(&stdout(&json)).unwrap();
    let deploy = doc["skills"]
        .as_array()
        .unwrap()
        .iter()
        .find(|skill| skill["name"] == "deploy")
        .expect("the project's deploy listed");
    let clashes = deploy["clashes"].as_array().unwrap();
    let claude = clashes
        .iter()
        .find(|one| one["harness"] == "claude")
        .unwrap_or_else(|| panic!("a Claude Code clash: {deploy}"));
    assert_eq!(claude["outcome"], "both-load");
    assert_eq!(claude["outside"], "repo");
    assert_eq!(
        claude["owner"],
        serde_json::json!({"kind": "repo", "name": "app"})
    );
    assert_eq!(claude["said"], both);
    let codex = clashes
        .iter()
        .find(|one| one["harness"] == "codex")
        .unwrap_or_else(|| panic!("a Codex clash: {deploy}"));
    assert_eq!(codex["outcome"], "repo-wins");
    assert_eq!(codex["said"], alone("Codex"));
    assert!(
        doc["skills"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|skill| skill["name"] != "deploy")
            .all(|skill| skill["clashes"] == serde_json::json!([])),
        "only deploy clashes: {doc}"
    );

    assert!(show.status.success(), "{}", stderr(&show));
    assert!(
        stdout(&show).contains(&format!("  also: {both}\n")),
        "{}",
        stdout(&show)
    );
}

#[test]
fn a_plane_copy_a_repo_hides_carries_its_clash_in_list_and_show_json() {
    // #3461: the project's `deploy` is left out of Codex's chats in the clone `app`, whose own
    // `deploy` wins there. The left-out row says whose copy won in data too, as purlis's own
    // copy's row does, so `--json` never has to be read from the row's sentence.
    let project = Project::new();
    let root = project.at.join("plane");
    std::fs::write(root.join("skills/deploy.toml"), "grant = \"all\"\n").unwrap();
    write_skill(
        &root.join("workspaces/alpha/app/.agents/skills"),
        "deploy",
        "The repo's deploy, for Codex and opencode.",
    );

    let list = project.run(&["skill", "list", "-w", "alpha", "--json"]);
    let show = project.run(&["skill", "show", "deploy", "-w", "alpha", "--json"]);

    let repo_wins = |row: &serde_json::Value, harness: &str| {
        let clashes = row["clashes"]
            .as_array()
            .unwrap_or_else(|| panic!("clashes on the left-out row: {row}"));
        assert_eq!(clashes.len(), 1, "{row}");
        assert_eq!(clashes[0]["harness"], harness, "{row}");
        assert_eq!(clashes[0]["outcome"], "repo-wins", "{row}");
        assert_eq!(clashes[0]["outside"], "repo", "{row}");
        assert_eq!(
            clashes[0]["owner"],
            serde_json::json!({"kind": "repo", "name": "app"}),
            "{row}"
        );
    };
    assert!(list.status.success(), "{}", stderr(&list));
    let doc: serde_json::Value = serde_json::from_str(&stdout(&list)).unwrap();
    for harness in ["codex", "opencode"] {
        let one = doc["harnesses"]
            .as_array()
            .unwrap()
            .iter()
            .find(|one| one["harness"] == harness)
            .unwrap_or_else(|| panic!("{harness} listed: {doc}"));
        assert_eq!(one["repo_left_out"][0]["name"], "deploy", "{doc}");
        repo_wins(&one["repo_left_out"][0], harness);
    }

    assert!(show.status.success(), "{}", stderr(&show));
    let doc: serde_json::Value = serde_json::from_str(&stdout(&show)).unwrap();
    let copies = doc["copies"].as_array().unwrap();
    let hidden: Vec<&serde_json::Value> = copies
        .iter()
        .filter(|copy| copy["level"] == "project" && copy["on"] == false)
        .collect();
    assert_eq!(
        hidden.len(),
        2,
        "one left-out copy on Codex and opencode: {doc}"
    );
    for copy in hidden {
        repo_wins(copy, copy["harness"].as_str().unwrap());
    }
    // A copy the harness finds itself, the repo's own, is no plane copy and has no clashes.
    assert!(
        copies
            .iter()
            .filter(|copy| copy["level"] == "repo")
            .all(|copy| copy.get("clashes").is_none()),
        "{doc}"
    );
}

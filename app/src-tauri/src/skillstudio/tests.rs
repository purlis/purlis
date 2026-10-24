// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

/// An environment in which no harness has anything of its own: no home, and nothing read from
/// this process.
fn no_harness() -> purlis_core::harness_plugin::Env<'static> {
    purlis_core::harness_plugin::Env {
        chat: &[],
        home: None,
        process: false,
    }
}

fn skill(dir: &std::path::Path, folder: &str, text: &str) {
    std::fs::create_dir_all(dir.join(folder)).unwrap();
    std::fs::write(dir.join(folder).join("SKILL.md"), text).unwrap();
}

#[test]
fn the_studio_lists_built_in_and_project_skills_with_their_level_and_why_one_is_left_out() {
    let tmp = tempfile::tempdir().unwrap();
    let built_in = tmp.path().join("plugin/skills");
    let root = tmp.path().join("plane");
    skill(
        &built_in,
        "secrets",
        "---\nname: secrets\ndescription: Keeps secrets.\n---\n",
    );
    skill(
        &root.join("skills"),
        "deploy",
        "---\nname: deploy\ndescription: Deploys.\n---\n",
    );
    skill(&root.join("skills"), "half", "---\nname: half\n---\n");

    let studio = studio(&root, Some(&built_in), true, None, None, &no_harness());

    assert!(studio.on);
    let rows: Vec<(&str, Level, bool, Option<&str>, &str)> = studio
        .skills
        .iter()
        .map(|s| {
            (
                s.name.as_str(),
                s.level,
                s.on,
                s.left_out.as_deref(),
                s.folder.as_str(),
            )
        })
        .collect();
    assert_eq!(
        rows[..2],
        [
            ("deploy", Level::Project, true, None, "skills/deploy"),
            (
                "half",
                Level::Project,
                false,
                Some("not a skill: its SKILL.md names no `description`"),
                "skills/half"
            ),
        ]
    );
    assert_eq!(rows[2].0, "secrets");
    assert_eq!(rows[2].1, Level::BuiltIn);
}

#[test]
fn the_studio_shows_nothing_while_its_flag_is_off() {
    let tmp = tempfile::tempdir().unwrap();
    skill(
        &tmp.path().join("skills"),
        "deploy",
        "---\nname: deploy\ndescription: Deploys.\n---\n",
    );

    let studio = studio(tmp.path(), None, false, None, None, &no_harness());

    assert_eq!(
        studio,
        Studio {
            on: false,
            workspace: None,
            persona: None,
            workspaces: Vec::new(),
            levels: Vec::new(),
            skills: Vec::new(),
            harnesses: Vec::new(),
            clashes: Vec::new(),
        }
    );
}

#[test]
fn the_studio_shows_a_workspaces_chat_with_its_own_skills_and_each_hidden_copy() {
    // #2767: filtered to a workspace, the studio is that workspace's chats' set: its own skill,
    // the project skill granted to it, and the project copy its own hides, with the reason.
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("plane");
    let project = root.join("skills");
    skill(
        &project,
        "deploy",
        "---\nname: deploy\ndescription: Deploys.\n---\n",
    );
    std::fs::write(project.join("deploy.toml"), "grant = \"all\"\n").unwrap();
    skill(
        &project,
        "lint",
        "---\nname: lint\ndescription: Lints.\n---\n",
    );
    std::fs::write(project.join("lint.toml"), "grant = \"all\"\n").unwrap();
    skill(
        &root.join("workspaces/alpha/skills"),
        "lint",
        "---\nname: lint\ndescription: Lints here.\n---\n",
    );
    std::fs::create_dir_all(root.join("workspaces/beta")).unwrap();

    let studio = studio(&root, None, true, Some("alpha"), None, &no_harness());

    assert_eq!(studio.workspace.as_deref(), Some("alpha"));
    assert_eq!(studio.workspaces, ["alpha", "beta"]);
    type Row<'a> = (&'a str, Level, Option<&'a str>, bool, Option<&'a str>);
    let rows: Vec<Row<'_>> = studio
        .skills
        .iter()
        .map(|s| {
            (
                s.name.as_str(),
                s.level,
                s.grant.as_deref(),
                s.on,
                s.left_out.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        rows,
        [
            ("deploy", Level::Project, Some("all"), true, None),
            ("lint", Level::Workspace, None, true, None),
            (
                "lint",
                Level::Project,
                Some("all"),
                false,
                Some("hidden by the workspace skill of this name")
            ),
        ]
    );
}

#[test]
fn the_studio_as_a_persona_lists_its_own_skills_and_one_it_names_whatever_was_granted() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("plane");
    std::fs::create_dir_all(root.join("workspaces/alpha")).unwrap();
    std::fs::create_dir_all(root.join("personas/ops")).unwrap();
    std::fs::write(
        root.join("personas/ops/persona.md"),
        "---\nskills: deploy\n---\n",
    )
    .unwrap();
    skill(
        &root.join("skills"),
        "deploy",
        "---\nname: deploy\ndescription: Deploys.\n---\n",
    );
    skill(
        &root.join("personas/ops/skills"),
        "triage",
        "---\nname: triage\ndescription: Triages.\n---\n",
    );

    let studio = studio(&root, None, true, Some("alpha"), Some("ops"), &no_harness());

    assert_eq!(studio.persona.as_deref(), Some("ops"));
    let rows: Vec<(&str, Level, bool, &str)> = studio
        .skills
        .iter()
        .map(|s| (s.name.as_str(), s.level, s.on, s.label.as_str()))
        .collect();
    assert_eq!(
        rows,
        [
            ("deploy", Level::Project, true, "project"),
            ("triage", Level::Persona, true, "persona"),
        ]
    );
}

#[test]
fn the_studio_lists_each_harnesss_own_skills_and_plugins_and_marks_a_clash_with_a_project_skill() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("plane");
    skill(
        &root.join("skills"),
        "review",
        "---\nname: review\ndescription: Reviews.\n---\n",
    );
    let claude = tmp.path().join("claude-config");
    skill(
        &claude.join("skills"),
        "review",
        "---\nname: review\ndescription: Mine.\n---\n",
    );
    let figma = tmp.path().join("cache/figma");
    skill(
        &figma.join("skills"),
        "frame",
        "---\nname: frame\ndescription: Frames.\n---\n",
    );
    std::fs::create_dir_all(claude.join("plugins")).unwrap();
    std::fs::write(
        claude.join("plugins/installed_plugins.json"),
        serde_json::json!({"version": 2, "plugins": {
            "figma@official": [{"scope": "user", "installPath": figma}]
        }})
        .to_string(),
    )
    .unwrap();
    let chat = [("CLAUDE_CONFIG_DIR".to_owned(), claude.display().to_string())];
    let env = purlis_core::harness_plugin::Env {
        chat: &chat,
        home: None,
        process: false,
    };

    let studio = studio(&root, None, true, None, None, &env);

    let on_claude = &studio.harnesses[0];
    assert_eq!(on_claude.title, "Claude Code");
    let rows: Vec<(&str, Level, Option<&Owner>, Option<&SkillSwitch>)> = on_claude
        .skills
        .iter()
        .map(|s| {
            (
                s.name.as_str(),
                s.level,
                s.owner.as_ref(),
                s.switch.as_ref(),
            )
        })
        .collect();
    assert_eq!(
        rows,
        [
            (
                "review",
                Level::Installed,
                Some(&Owner::You),
                Some(&SkillSwitch::Alone)
            ),
            (
                "figma:frame",
                Level::Installed,
                Some(&Owner::Plugin {
                    name: "figma".to_owned()
                }),
                Some(&SkillSwitch::Not {
                    why: "Claude Code turns a plugin's skill off only with the whole plugin, \
                          figma"
                        .to_owned()
                })
            ),
        ]
    );
    let figma_plugin = on_claude
        .plugins
        .iter()
        .find(|p| p.id == "figma@official")
        .expect("listed");
    assert_eq!(
        (figma_plugin.state, figma_plugin.pinned.is_none()),
        (PluginState::NotSet, true)
    );
    assert_eq!(
        studio.clashes,
        [StudioClash {
            name: "review".to_owned(),
            level: Level::Project,
            local: false,
            harness: "Claude Code".to_owned(),
            outside: Level::Installed,
            owner: Owner::You,
            outcome: ClashOutcome::BothLoad,
        }]
    );
    let codex = &studio.harnesses[2];
    assert_eq!(codex.unsupported, None);
    assert!(codex.only_off.is_some(), "{:?}", codex.only_off);
    assert!(
        studio.harnesses[1].unsupported.is_some(),
        "opencode's plugins"
    );
}

#[test]
fn the_runs_panel_is_told_to_the_window_by_the_word_the_window_names_it_by() {
    let told = serde_json::to_value(SkillPanelKey::Runs).unwrap();
    assert_eq!(told["kind"], RUNS_PANEL);
}

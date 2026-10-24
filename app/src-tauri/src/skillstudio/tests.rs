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
            first: StudioFirst::default(),
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
fn a_skills_totals_name_what_it_ran_on_as_a_person_calls_it_and_say_a_start_that_does_not_say() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    std::fs::write(root.join(purlis_core::plane::MANIFEST), "schema = 2\n").unwrap();
    let log = root.join("personas/_skills");
    std::fs::create_dir_all(&log).unwrap();
    std::fs::write(
        log.join("2026-10.box.jsonl"),
        "{\"harness\": \"claude\", \"persona\": \"ops\", \"run\": \"01JB0000000000000000000001\", \
         \"skill\": \"deploy\", \"started_by\": \"curation\", \"ts\": \"2026-10-01T09:00:00+00:00\", \
         \"workspace\": null}\n\
         {\"asks\": 1, \"ended\": \"turn\", \"ms\": 900, \"refusals\": 0, \"run\": \
         \"01JB0000000000000000000001\", \"tokens\": 40, \"tool_calls\": 2, \"tool_errors\": 0, \
         \"ts\": \"2026-10-01T09:00:01+00:00\"}\n\
         {\"persona\": null, \"skill\": \"deploy\", \"ts\": \"2026-03-02T09:00:00+00:00\"}\n",
    )
    .unwrap();

    let stats = stats(root, "deploy");

    assert_eq!(
        stats.by_harness,
        [
            StatsRow {
                group: StatsGroup::Named("Claude Code".to_owned()),
                runs: 1,
                ended: 1,
                tool_calls: 2,
                tool_errors: 0,
                refusals: 0,
                asks: 1,
                tokens: Some(40),
                ms: 900,
            },
            StatsRow {
                group: StatsGroup::NotRecorded,
                runs: 1,
                ended: 0,
                tool_calls: 0,
                tool_errors: 0,
                refusals: 0,
                asks: 0,
                tokens: None,
                ms: 0,
            },
        ]
    );
    let workspaces: Vec<&StatsGroup> = stats.by_workspace.iter().map(|row| &row.group).collect();
    assert_eq!(workspaces, [&StatsGroup::None, &StatsGroup::NotRecorded]);
    let runs = runs(root, "deploy", 10);
    assert_eq!(runs[0].started_by, Some(RunStart::Curation));
    assert_eq!(runs[0].ran_on.as_deref(), Some("Claude Code"));
}

#[test]
fn a_studio_with_only_built_in_skills_lists_them_and_offers_the_starts_and_the_sources() {
    // SK-22 (#2785): the built-in skills are listed, but they are not the project's, so the
    // three starts are offered beside them, with every offered source the project lacks.
    let tmp = tempfile::tempdir().unwrap();
    let built_in = tmp.path().join("plugin/skills");
    let root = tmp.path().join("plane");
    std::fs::create_dir_all(&root).unwrap();
    skill(
        &built_in,
        "secrets",
        "---\nname: secrets\ndescription: Keeps secrets.\n---\n",
    );

    let studio = studio(&root, Some(&built_in), true, None, None, &no_harness());

    assert_eq!(studio.skills.len(), 1);
    assert!(studio.first.starts);
    let anthropic = studio
        .first
        .sources
        .iter()
        .find(|one| one.source.remote.url == "https://github.com/anthropics/skills")
        .expect("Anthropic's skills are offered");
    assert_eq!(anthropic.offering, SourceOffering::WellKnown);
    assert_eq!(anthropic.source.folder.as_deref(), Some("skills"));
    assert_eq!(
        anthropic.source.kept,
        crate::skillsources::SourceKept::Shared
    );
    assert!(!anthropic.why.is_empty());
    assert_eq!(studio.first.unmoved, None);

    skill(
        &root.join(".claude/skills"),
        "deploy",
        "---\nname: deploy\ndescription: Deploys.\n---\n",
    );
    skill(
        &root.join("skills"),
        "notify",
        "---\nname: notify\ndescription: Tells.\n---\n",
    );
    let studio = super::studio(&root, Some(&built_in), true, None, None, &no_harness());
    assert!(!studio.first.starts);
    assert_eq!(
        studio.first.unmoved,
        Some(StudioUnmoved {
            skills: 1,
            fix: "project-skills".to_owned(),
        })
    );
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The harness adapters (charter-app#274, ADR 0050 as amended by #3467): purlis's pins, each
//! adapter's record read from a fixture directory, the harness's own skills and what a file may
//! not say of them. Every expected answer is written out.

use super::*;
use crate::extension::project::Source;

// -------------------------------------------------------------------------------------
// purlis's own plugin: the pins, and nothing a project file says
// -------------------------------------------------------------------------------------

fn pins_of(adapter: &dyn Adapter) -> Vec<(String, bool)> {
    adapter
        .pinned()
        .iter()
        .map(|pin| (pin.id.to_owned(), pin.on))
        .collect()
}

#[test]
fn claude_codes_pins_are_charters_own_on_and_the_two_it_replaced_off() {
    assert_eq!(
        pins_of(&CLAUDE_CODE).into_iter().collect::<BTreeMap<_, _>>(),
        BTreeMap::from([
            ("purlis@inline".to_owned(), true),
            ("charter@charter".to_owned(), false),
            ("charter-app@inline".to_owned(), false),
            ("charter@inline".to_owned(), false),
            ("charter@charter-app".to_owned(), false),
        ])
    );
    assert_eq!(pins_of(&CLAUDE_CODE).len(), 5);
}

#[test]
fn codex_and_opencode_pin_nothing_since_they_load_purlis_s_plugin_their_own_way() {
    assert!(pins_of(&CODEX).is_empty());
    assert!(pins_of(&OPENCODE).is_empty());
    assert_eq!(pins(&CODEX), Chosen::new());
}

/// A plane whose files are `files`, for a chat's start.
fn plane_with(files: &[(&str, &str)]) -> tempfile::TempDir {
    let plane = tempfile::tempdir().unwrap();
    for (name, text) in files {
        let path = plane.path().join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    plane
}

#[test]
fn a_chat_is_handed_the_pins_alone_whatever_the_old_keys_say() {
    // #3467: the old [harness_plugins] keys are ignored on every layer, the pins included.
    let plane = plane_with(&[
        (
            "charter.toml",
            "[harness_plugins.claude]\n\"figma@official\" = false\n\"purlis@inline\" = false\n",
        ),
        (
            "charter.local.toml",
            "[harness_plugins.claude]\n\"charter@charter\" = true\n\n\
             [harness_plugins.codex]\n\"a@b\" = false\n",
        ),
        (
            "workspaces/alpha/workspace.json",
            r#"{"settings": {"harness_plugins": {"claude": {"serena@x": true}}}}"#,
        ),
    ]);
    for studio in [false, true] {
        for workspace in [None, Some("alpha")] {
            assert_eq!(
                for_start_as(studio, "claude", plane.path(), workspace).plugins,
                pins_of(&CLAUDE_CODE).into_iter().collect::<BTreeMap<_, _>>(),
                "{studio} {workspace:?}"
            );
            assert_eq!(
                for_start_as(studio, "codex", plane.path(), workspace),
                Chosen::new(),
                "{studio} {workspace:?}"
            );
        }
    }
}

#[test]
fn the_old_table_is_refused_nothing_so_a_file_that_holds_it_saves_as_it_did() {
    assert_eq!(
        refusals(
            "[harness_plugins.claude]\n\"purlis@inline\" = false\n\n[harness_plugins.vim]\nx = 3\n",
            "charter.toml"
        ),
        Vec::<String>::new()
    );
    assert_eq!(refusals("harness_plugins = 3\n", "charter.toml"), Vec::<String>::new());
}

#[test]
fn a_chat_on_no_known_harness_is_handed_nothing() {
    let plane = plane_with(&[]);
    assert_eq!(for_start_as(true, "vim", plane.path(), None), Chosen::new());
}

// -------------------------------------------------------------------------------------
// What a harness records it has installed: what a Try it's scratch chat turns off
// -------------------------------------------------------------------------------------

fn env_of(pairs: &[(&str, &std::path::Path)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(name, path)| ((*name).to_owned(), path.display().to_string()))
        .collect()
}

#[test]
fn claude_code_lists_every_plugin_its_install_record_holds_once() {
    let home = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(home.path().join("plugins")).unwrap();
    std::fs::write(
        home.path().join("plugins/installed_plugins.json"),
        r#"{"version": 2, "plugins": {
            "figma@claude-plugins-official": [{"scope": "user", "installPath": "/x", "version": "2.2.1"}],
            "superpowers@claude-plugins-official": [
                {"scope": "project", "projectPath": "/a", "installPath": "/y", "version": "5.0.0"},
                {"scope": "project", "projectPath": "/c", "installPath": "/y", "version": "5.0.0"},
                {"scope": "local", "projectPath": "/b", "installPath": "/y", "version": "5.0.0"}
            ]
        }}"#,
    )
    .unwrap();
    let env = env_of(&[("CLAUDE_CONFIG_DIR", home.path())]);

    let got = CLAUDE_CODE.installed(&Env::of(&env)).expect("it reads");

    assert_eq!(
        got,
        [
            Plugin {
                id: "figma@claude-plugins-official".to_owned(),
                harness: "claude",
                name: "figma".to_owned(),
                source: "claude-plugins-official, user".to_owned(),
            },
            Plugin {
                id: "superpowers@claude-plugins-official".to_owned(),
                harness: "claude",
                name: "superpowers".to_owned(),
                source: "claude-plugins-official, project, local".to_owned(),
            },
        ]
    );
}

#[test]
fn claude_code_with_nothing_installed_lists_nothing() {
    let home = tempfile::tempdir().unwrap();
    let env = env_of(&[("CLAUDE_CONFIG_DIR", home.path())]);
    assert_eq!(CLAUDE_CODE.installed(&Env::of(&env)), Ok(Vec::new()));
}

#[test]
fn claude_code_says_so_when_its_install_record_is_not_json() {
    let home = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(home.path().join("plugins")).unwrap();
    std::fs::write(home.path().join("plugins/installed_plugins.json"), "{").unwrap();
    let env = env_of(&[("CLAUDE_CONFIG_DIR", home.path())]);

    let why = CLAUDE_CODE
        .installed(&Env::of(&env))
        .expect_err("it refuses");
    assert!(why.contains("installed_plugins.json"), "{why}");
}

#[test]
fn claude_code_reads_under_home_when_the_chat_names_no_config_dir() {
    let home = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(home.path().join(".claude/plugins")).unwrap();
    std::fs::write(
        home.path().join(".claude/plugins/installed_plugins.json"),
        r#"{"version": 2, "plugins": {"a@m": [{"scope": "user"}]}}"#,
    )
    .unwrap();
    let env = Env {
        chat: &[],
        home: Some(home.path().to_path_buf()),
        process: false,
    };

    let got = CLAUDE_CODE.installed(&env).expect("it reads");
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].id, "a@m");
}

#[test]
fn codex_lists_the_plugins_its_config_names() {
    let home = tempfile::tempdir().unwrap();
    std::fs::write(
        home.path().join("config.toml"),
        "model = \"o3\"\n\n[plugins.\"charter@charter\"]\nenabled = true\n\n\
         [plugins.\"deep-research@openai-curated\"]\nenabled = false\n",
    )
    .unwrap();
    let env = env_of(&[("CODEX_HOME", home.path())]);

    let got = CODEX.installed(&Env::of(&env)).expect("it reads");

    assert_eq!(
        got,
        [
            Plugin {
                id: "charter@charter".to_owned(),
                harness: "codex",
                name: "charter".to_owned(),
                source: "charter, on in config.toml".to_owned(),
            },
            Plugin {
                id: "deep-research@openai-curated".to_owned(),
                harness: "codex",
                name: "deep-research".to_owned(),
                source: "openai-curated, off in config.toml".to_owned(),
            },
        ]
    );
}

#[test]
fn opencode_lists_nothing_since_it_cannot_turn_one_plugin_off() {
    let home = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(home.path().join("xdg/opencode/plugin")).unwrap();
    std::fs::write(home.path().join("xdg/opencode/plugin/a.js"), "").unwrap();
    let chat = [(
        "XDG_CONFIG_HOME".to_owned(),
        home.path().join("xdg").display().to_string(),
    )];
    let env = Env {
        chat: &chat,
        home: Some(home.path().to_path_buf()),
        process: false,
    };
    assert_eq!(OPENCODE.installed(&env), Ok(Vec::new()));
}

#[test]
fn claude_code_says_so_when_its_install_record_cannot_be_read() {
    let home = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(home.path().join("plugins/installed_plugins.json")).unwrap();
    let env = env_of(&[("CLAUDE_CONFIG_DIR", home.path())]);

    let why = CLAUDE_CODE
        .installed(&Env::of(&env))
        .expect_err("a record that is a directory read as no record");
    assert!(why.starts_with("purlis could not read "), "{why}");
}

#[test]
fn claude_code_is_called_claude_code() {
    assert_eq!(CLAUDE_CODE.title(), "Claude Code");
}

/// A title is written once, in the harness's built-in declaration (qw312, qw323): each plugin
/// adapter is called what the harness of its `kind` is.
#[test]
fn every_plugin_adapter_is_called_what_its_harnesss_declaration_calls_it() {
    for word in ["claude", "codex", "opencode"] {
        let adapter = adapter(word).expect("an adapter for every harness purlis ships");
        let harness = crate::harness::Harness::of_kind(adapter.harness()).expect("a harness");
        assert_eq!(adapter.title(), harness.declared().title, "{word}");
    }
}

#[test]
fn a_plugin_id_is_one_short_line_with_nothing_in_it_that_draws_as_nothing() {
    for good in ["figma@official", "a", &"a".repeat(MOST_ID_BYTES)] {
        assert!(id_ok(good), "{good}");
    }
    for bad in [
        "",
        "   ",
        &"a".repeat(MOST_ID_BYTES + 1),
        "a\nb",
        "a\u{200b}b",
    ] {
        assert!(!id_ok(bad), "{bad:?}");
    }
}

// -------------------------------------------------------------------------------------
// The harness's own skills (SK-6, #2769)
// -------------------------------------------------------------------------------------

#[test]
fn a_harness_skills_table_is_refused_what_its_harness_cannot_switch_alone() {
    // #3467: a plugin's skill is the plugin's on every harness, Codex's included.
    let got = refusals(
        "[harness_skills.claude]\nlangfuse = false\n\"figma:frame\" = false\n\n\
         [harness_skills.codex]\n\"zp:zp-skill\" = false\nreview = \"off\"\n\n\
         [harness_skills.vim]\nx = false\n",
        "charter.toml",
    );
    assert_eq!(
        got,
        [
            "harness_skills.claude.\"figma:frame\" in charter.toml cannot be set: it comes with \
             the plugin figma, which Claude Code turns on or off itself: purlis shows a plugin's \
             skills and does not switch them"
                .to_owned(),
            "harness_skills.codex.\"zp:zp-skill\" in charter.toml cannot be set: it comes with \
             the plugin zp, which Codex turns on or off itself: purlis shows a plugin's skills and \
             does not switch them"
                .to_owned(),
            "harness_skills.codex.review in charter.toml is not true or false".to_owned(),
            "[harness_skills.vim] in charter.toml is not a harness purlis knows — one of: \
             claude, opencode, codex"
                .to_owned(),
        ]
    );
}

#[test]
fn a_harness_skills_that_is_not_a_table_is_refused() {
    assert_eq!(
        refusals("harness_skills = 3\n", "charter.toml"),
        [
            "harness_skills in charter.toml is not a table — each harness is \
          [harness_skills.<harness>], holding \"<skill name>\" = true or false"
                .to_owned()
        ]
    );
}

#[test]
fn a_skill_the_harness_knows_is_on_or_off_by_the_layer_with_the_last_word() {
    let choices = Choices::from_text(
        Some("[harness_skills.opencode]\nreview = false\nlint = false\n"),
        Some("[harness_skills.opencode]\nlint = true\n"),
    )
    .in_workspace(Some(
        r#"{"settings": {"harness_skills": {"opencode": {"deploy": false}}}}"#,
    ));
    assert_eq!(
        choices.skill("opencode", "review"),
        (Some(false), Source::Shared)
    );
    assert_eq!(
        choices.skill("opencode", "lint"),
        (Some(true), Source::Local)
    );
    assert_eq!(
        choices.skill("opencode", "deploy"),
        (Some(false), Source::Workspace)
    );
    assert_eq!(choices.skill("codex", "review"), (None, Source::Default));
    assert_eq!(
        choices.skills_off(&OPENCODE),
        BTreeSet::from(["deploy".to_owned(), "review".to_owned()])
    );
}

#[test]
fn a_plugins_skill_a_file_turned_off_is_handed_to_no_chat() {
    // #3467: read-only in the studio, and never handed over, on any harness.
    let choices = Choices::from_text(
        Some("[harness_skills.codex]\n\"zp:zp-skill\" = false\nlangfuse = false\n"),
        None,
    );
    assert_eq!(
        choices.skills_off(&CODEX),
        BTreeSet::from(["langfuse".to_owned()])
    );
}

#[test]
fn a_chat_is_handed_the_harness_skills_the_project_turned_off_only_while_the_studio_is_on() {
    let plane = plane_with(&[
        (
            "charter.toml",
            "[harness_skills.codex]\nlangfuse = false\nreview = false\n",
        ),
        ("charter.local.toml", "[harness_skills.codex]\nreview = true\n"),
    ]);
    assert_eq!(
        for_start_as(true, "codex", plane.path(), None).skills_off,
        BTreeSet::from(["langfuse".to_owned()]),
        "Local turned review back on, over Shared"
    );
    assert_eq!(
        for_start_as(false, "codex", plane.path(), None),
        Chosen::new(),
        "behind the Skill studio's flag"
    );
}

// The Skill studio's switch (SK-6, #2769)
// -------------------------------------------------------------------------------------

/// The built-in skills' names, as the window hands them over.
fn built_in() -> Vec<String> {
    vec!["secrets".to_owned(), "handoff".to_owned()]
}

#[test]
fn a_built_in_skills_name_is_refused_in_harness_skills_on_every_harness() {
    // SK-6 (#2769): opencode's and Claude Code's switches go by the name alone, so a key named as
    // a built-in skill would turn purlis's own off.
    let top: toml::Table = "[harness_skills.opencode]\nsecrets = false\nreview = false\n\n\
                            [harness_skills.codex]\nhandoff = true\n"
        .parse()
        .unwrap();
    let got: Vec<String> = built_in_refused(&top, "charter.toml", "", &built_in())
        .into_iter()
        .map(|one| one.why)
        .collect();
    assert_eq!(
        got,
        [
            "harness_skills.opencode.secrets in charter.toml cannot be set: secrets is one of \
             purlis's built-in skills, which no project turns off"
                .to_owned(),
            "harness_skills.codex.handoff in charter.toml cannot be set: handoff is one of \
             purlis's built-in skills, which no project turns off"
                .to_owned(),
        ]
    );
}

fn text_of(path: &std::path::Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

#[test]
fn the_studio_turns_a_harness_skill_off_at_the_root_in_this_machines_own_file_and_back_by_removing_it()
 {
    let plane = tempfile::tempdir().unwrap();
    let local = plane.path().join("charter.local.toml");

    switch_in_the_studio(
        plane.path(),
        None,
        crate::harness::Harness::Codex,
        "langfuse",
        false,
        &built_in(),
    )
    .expect("turned off");
    assert_eq!(
        Choices::read(plane.path()).skill("codex", "langfuse"),
        (Some(false), Source::Local)
    );
    assert!(
        text_of(&local).contains("langfuse = false"),
        "{}",
        text_of(&local)
    );

    switch_in_the_studio(
        plane.path(),
        None,
        crate::harness::Harness::Codex,
        "langfuse",
        true,
        &built_in(),
    )
    .expect("turned on");
    assert_eq!(
        Choices::read(plane.path()).skill("codex", "langfuse"),
        (None, Source::Default),
        "nothing else named it, so the harness decides again"
    );
    assert!(!text_of(&local).contains("langfuse"), "{}", text_of(&local));
}

#[test]
fn the_studio_turns_one_on_over_a_shared_off_by_writing_it_on() {
    let plane = tempfile::tempdir().unwrap();
    std::fs::write(
        plane.path().join("charter.toml"),
        "[harness_skills.claude]\nlangfuse = false\n",
    )
    .unwrap();

    switch_in_the_studio(
        plane.path(),
        None,
        crate::harness::Harness::ClaudeCode,
        "langfuse",
        true,
        &built_in(),
    )
    .expect("turned on");

    assert_eq!(
        Choices::read(plane.path()).skill("claude", "langfuse"),
        (Some(true), Source::Local)
    );
}

#[test]
fn the_studio_switches_for_a_workspace_in_its_own_settings() {
    let plane = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(plane.path().join("workspaces/alpha")).unwrap();
    // A manifest purlis wrote and stamped, as a workspace's is.
    std::fs::write(
        plane.path().join("workspaces/alpha/workspace.json"),
        include_str!("../../../../tests/fixtures/planes/daily/workspaces/alpha/workspace.json"),
    )
    .unwrap();

    switch_in_the_studio(
        plane.path(),
        Some("alpha"),
        crate::harness::Harness::Opencode,
        "review",
        false,
        &built_in(),
    )
    .expect("turned off");

    assert_eq!(
        Choices::read_in(plane.path(), Some("alpha")).skill("opencode", "review"),
        (Some(false), Source::Workspace)
    );
    assert_eq!(
        Choices::read_in(plane.path(), None).skill("opencode", "review"),
        (None, Source::Default),
        "the project's root is untouched"
    );

    switch_in_the_studio(
        plane.path(),
        Some("alpha"),
        crate::harness::Harness::Opencode,
        "review",
        true,
        &built_in(),
    )
    .expect("turned back on");
    assert_eq!(
        Choices::read_in(plane.path(), Some("alpha")).skill("opencode", "review"),
        (None, Source::Default),
        "its key is taken out again"
    );
    let manifest = text_of(&plane.path().join("workspaces/alpha/workspace.json"));
    assert!(!manifest.contains("review"), "{manifest}");
}

#[test]
fn the_studio_refuses_what_a_harness_cannot_switch_and_writes_nothing() {
    let plane = tempfile::tempdir().unwrap();
    let refused = [
        (
            switch_in_the_studio(
                plane.path(),
                None,
                crate::harness::Harness::ClaudeCode,
                        "figma:frame",
                false,
                &built_in(),
            ),
            "it comes with the plugin figma, which Claude Code turns on or off itself: purlis \
             shows a plugin's skills and does not switch them, so nothing was changed",
        ),
        (
            switch_in_the_studio(
                plane.path(),
                None,
                crate::harness::Harness::Opencode,
                        "secrets",
                false,
                &built_in(),
            ),
            "secrets is one of purlis's built-in skills, which no project turns off, so nothing \
             was changed",
        ),
    ];
    for (got, said) in refused {
        let why = got.expect_err("refused");
        assert!(why.starts_with(said), "{why}");
    }
    assert!(!plane.path().join("charter.local.toml").exists());
}


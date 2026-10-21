// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What each harness has on this machine, read from folders in a temporary directory that the
//! chat's own variables point each harness at, never from this machine's.

use std::path::{Path, PathBuf};

use purlis_skills::OnMachine;

use super::super::{Adapter, CLAUDE_CODE, CODEX, Env, OPENCODE};

fn skill(dir: &Path, folder: &str, name: &str) {
    std::fs::create_dir_all(dir.join(folder)).unwrap();
    std::fs::write(
        dir.join(folder).join("SKILL.md"),
        format!("---\nname: {name}\ndescription: Does it.\n---\n"),
    )
    .unwrap();
}

fn own(name: &str) -> OnMachine {
    OnMachine::Skill {
        plugin: None,
        name: name.to_owned(),
    }
}

fn of(plugin: &str, name: &str) -> OnMachine {
    OnMachine::Skill {
        plugin: Some(plugin.to_owned()),
        name: name.to_owned(),
    }
}

fn env(chat: &[(String, String)], home: PathBuf) -> Env<'_> {
    Env {
        chat,
        home: Some(home),
        process: false,
    }
}

#[test]
fn claude_code_has_the_persons_skills_and_each_installed_plugins_skills() {
    let tmp = tempfile::tempdir().unwrap();
    let config = tmp.path().join("claude-config");
    skill(&config.join("skills"), "langfuse", "langfuse");
    skill(&config.join("skills/synced"), "docx", "docx");
    let plugin = tmp.path().join("cache/mattpocock-skills/1.0");
    skill(&plugin.join("skills"), "grilling-folder", "grilling");
    std::fs::create_dir_all(config.join("plugins")).unwrap();
    std::fs::write(
        config.join("plugins/installed_plugins.json"),
        serde_json::json!({
            "version": 2,
            "plugins": {
                "mattpocock-skills@market": [{"scope": "user", "installPath": plugin}],
                "superpowers@market": [{"scope": "user"}],
            }
        })
        .to_string(),
    )
    .unwrap();
    let chat = [("CLAUDE_CONFIG_DIR".to_owned(), config.display().to_string())];

    let mut got = CLAUDE_CODE.skills(&env(&chat, tmp.path().join("home")));
    got.sort_by_key(|one| format!("{one:?}"));

    let mut want = vec![
        own("langfuse"),
        own("docx"),
        of("anthropic-skills", "docx"),
        of("mattpocock-skills", "grilling-folder"),
        of("mattpocock-skills", "grilling"),
        OnMachine::Plugin("superpowers".to_owned()),
    ];
    want.sort_by_key(|one| format!("{one:?}"));
    assert_eq!(got, want);
}

#[test]
fn codex_has_its_homes_skills_and_each_plugin_its_config_records_as_a_whole() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("codex-home");
    skill(&home.join("skills"), "review", "review");
    std::fs::write(
        home.join("config.toml"),
        "[plugins.\"superpowers@market\"]\nenabled = true\n",
    )
    .unwrap();
    let chat = [("CODEX_HOME".to_owned(), home.display().to_string())];

    let got = CODEX.skills(&env(&chat, tmp.path().join("home")));

    assert_eq!(
        got,
        [own("review"), OnMachine::Plugin("superpowers".to_owned())]
    );
}

#[test]
fn opencode_has_the_skills_of_its_config_folders() {
    let tmp = tempfile::tempdir().unwrap();
    let xdg = tmp.path().join("xdg");
    skill(&xdg.join("opencode/skill"), "one", "one");
    let extra = tmp.path().join("extra");
    skill(&extra.join("skills"), "two", "two");
    let chat = [
        ("XDG_CONFIG_HOME".to_owned(), xdg.display().to_string()),
        (
            "OPENCODE_CONFIG_DIR".to_owned(),
            extra.display().to_string(),
        ),
    ];

    let got = OPENCODE.skills(&env(&chat, tmp.path().join("home")));

    assert_eq!(got, [own("one"), own("two")]);
}

#[test]
fn a_harness_with_nothing_installed_has_no_skills() {
    let tmp = tempfile::tempdir().unwrap();
    let chat = [
        (
            "CLAUDE_CONFIG_DIR".to_owned(),
            tmp.path().join("a").display().to_string(),
        ),
        (
            "CODEX_HOME".to_owned(),
            tmp.path().join("b").display().to_string(),
        ),
        (
            "XDG_CONFIG_HOME".to_owned(),
            tmp.path().join("c").display().to_string(),
        ),
    ];
    let env = env(&chat, tmp.path().join("home"));
    for adapter in [&CLAUDE_CODE as &dyn Adapter, &CODEX, &OPENCODE] {
        assert!(adapter.skills(&env).is_empty(), "{}", adapter.harness());
    }
}

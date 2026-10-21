// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Where each harness says its skills are, read from records in a temporary directory that the
//! chat's own variables point each harness at, never from this machine's.

use std::path::{Path, PathBuf};

use super::super::{Adapter, CLAUDE_CODE, CODEX, Env, OPENCODE};
use super::SkillsAt;

fn env(chat: &[(String, String)], home: PathBuf) -> Env<'_> {
    Env {
        chat,
        home: Some(home),
        process: false,
    }
}

fn folder(dir: &Path, plugin: Option<&str>) -> SkillsAt {
    SkillsAt::Folder {
        dir: dir.to_path_buf(),
        plugin: plugin.map(str::to_owned),
    }
}

#[test]
fn claude_code_looks_in_the_persons_skills_the_synced_ones_and_each_installed_plugin() {
    let tmp = tempfile::tempdir().unwrap();
    let config = tmp.path().join("claude-config");
    let plugin = tmp.path().join("cache/mattpocock-skills/1.0");
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

    let at = CLAUDE_CODE.skills_at(&env(&chat, tmp.path().join("home")));

    let skills = config.join("skills");
    assert_eq!(
        at,
        [
            folder(&skills.join("synced"), Some("anthropic-skills")),
            folder(&skills.join("synced"), None),
            folder(&skills, None),
            SkillsAt::Plugin {
                plugin: "mattpocock-skills".to_owned(),
                dir: plugin,
                manifest: ".claude-plugin/plugin.json",
                versions: false,
            },
            SkillsAt::Unlisted("superpowers".to_owned()),
        ]
    );
}

#[test]
fn codex_looks_in_its_homes_skills_and_each_plugins_cache() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("codex-home");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(
        home.join("config.toml"),
        "[plugins.\"superpowers@market\"]\nenabled = true\n[plugins.\"loose\"]\n",
    )
    .unwrap();
    let chat = [("CODEX_HOME".to_owned(), home.display().to_string())];
    let person = tmp.path().join("home");

    let at = CODEX.skills_at(&env(&chat, person.clone()));

    assert_eq!(
        at,
        [
            folder(&home.join("skills"), None),
            folder(&person.join(".agents/skills"), None),
            SkillsAt::Unlisted("loose".to_owned()),
            SkillsAt::Plugin {
                plugin: "superpowers".to_owned(),
                dir: home.join("plugins/cache/market/superpowers"),
                manifest: ".codex-plugin/plugin.json",
                versions: true,
            },
        ]
    );
}

#[test]
fn opencode_looks_in_the_folders_it_shares_and_its_config_folders() {
    let tmp = tempfile::tempdir().unwrap();
    let xdg = tmp.path().join("xdg");
    let extra = tmp.path().join("extra");
    let chat = [
        ("XDG_CONFIG_HOME".to_owned(), xdg.display().to_string()),
        (
            "OPENCODE_CONFIG_DIR".to_owned(),
            extra.display().to_string(),
        ),
    ];
    let person = tmp.path().join("home");

    let at = OPENCODE.skills_at(&env(&chat, person.clone()));

    let mut want = vec![
        folder(&person.join(".claude/skills"), None),
        folder(&person.join(".agents/skills"), None),
    ];
    for config in [person.join(".opencode"), xdg.join("opencode"), extra] {
        want.push(folder(&config.join("skill"), None));
        want.push(folder(&config.join("skills"), None));
    }
    assert_eq!(at, want);
}

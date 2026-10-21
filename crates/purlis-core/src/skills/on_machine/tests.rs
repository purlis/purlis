// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The skills in the places a harness names, laid out in a temporary directory.

use std::path::Path;

use super::*;

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

#[test]
fn a_folder_lists_each_skill_by_its_folder_and_its_own_name() {
    let tmp = tempfile::tempdir().unwrap();
    skill(tmp.path(), "langfuse", "langfuse");
    skill(tmp.path(), "grill-folder", "grilling");
    std::fs::create_dir_all(tmp.path().join("empty")).unwrap();

    let got = listed(&[SkillsAt::Folder {
        dir: tmp.path().to_path_buf(),
        plugin: None,
    }]);

    assert_eq!(got, [own("grill-folder"), own("grilling"), own("langfuse")]);
}

#[test]
fn a_plugins_skills_are_where_its_manifest_says_else_in_skills() {
    let tmp = tempfile::tempdir().unwrap();
    let plain = tmp.path().join("plain");
    skill(&plain.join("skills"), "tdd", "tdd");
    let named = tmp.path().join("named");
    skill(&named.join("lib/skills"), "review", "review");
    std::fs::create_dir_all(named.join(".claude-plugin")).unwrap();
    std::fs::write(
        named.join(".claude-plugin/plugin.json"),
        r#"{"name": "named", "skills": ["./lib/skills"]}"#,
    )
    .unwrap();
    let at = |plugin: &str, dir: &Path| SkillsAt::Plugin {
        plugin: plugin.to_owned(),
        dir: dir.to_path_buf(),
        manifest: ".claude-plugin/plugin.json",
        versions: false,
    };

    let got = listed(&[at("plain", &plain), at("named", &named)]);

    assert_eq!(got, [of("plain", "tdd"), of("named", "review")]);
}

#[test]
fn a_plugin_kept_by_version_lists_each_versions_skills() {
    let tmp = tempfile::tempdir().unwrap();
    let cache = tmp.path().join("cache/market/plugin-management");
    skill(&cache.join("0.1.0/skills"), "manage", "manage");
    std::fs::create_dir_all(cache.join("0.1.0/.codex-plugin")).unwrap();
    std::fs::write(
        cache.join("0.1.0/.codex-plugin/plugin.json"),
        r#"{"name": "x"}"#,
    )
    .unwrap();

    let got = listed(&[SkillsAt::Plugin {
        plugin: "plugin-management".to_owned(),
        dir: cache,
        manifest: ".codex-plugin/plugin.json",
        versions: true,
    }]);

    assert_eq!(got, [of("plugin-management", "manage")]);
}

#[test]
fn a_plugin_whose_folder_is_not_there_is_unlisted_never_empty() {
    let tmp = tempfile::tempdir().unwrap();
    let gone = tmp.path().join("gone");

    let got = listed(&[
        SkillsAt::Plugin {
            plugin: "a".to_owned(),
            dir: gone.clone(),
            manifest: ".codex-plugin/plugin.json",
            versions: true,
        },
        SkillsAt::Plugin {
            plugin: "b".to_owned(),
            dir: gone,
            manifest: ".claude-plugin/plugin.json",
            versions: false,
        },
        SkillsAt::Unlisted("c".to_owned()),
    ]);

    assert_eq!(
        got,
        [
            OnMachine::Unlisted("a".to_owned()),
            OnMachine::Unlisted("b".to_owned()),
            OnMachine::Unlisted("c".to_owned()),
        ]
    );
}

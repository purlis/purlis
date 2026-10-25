// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The skills from outside the plane each harness finds itself (SK-6, #2769), from fixtures:
//! what is listed, whose it is, whether it switches alone, and what the project turned off.

use std::path::{Path, PathBuf};

use super::*;
use crate::harness_plugin::{CLAUDE_CODE, CODEX, OPENCODE};

fn skill(dir: &Path, folder: &str, name: &str) {
    std::fs::create_dir_all(dir.join(folder)).unwrap();
    std::fs::write(
        dir.join(folder).join("SKILL.md"),
        format!("---\nname: {name}\ndescription: Does {name}.\n---\n\nbody\n"),
    )
    .unwrap();
}

fn env<'a>(chat: &'a [(String, String)], home: &Path) -> Env<'a> {
    Env {
        chat,
        home: Some(home.to_path_buf()),
        process: false,
    }
}

fn rows(found: &[OutsideSkill]) -> Vec<(&str, Origin, String, bool, bool)> {
    found
        .iter()
        .map(|f| {
            let owner = match &f.owner {
                Owner::You => "you".to_owned(),
                Owner::Plugin(name) | Owner::Repo(name) => name.clone(),
            };
            (
                f.id.as_str(),
                f.origin,
                owner,
                f.switch == Switch::Alone,
                f.off,
            )
        })
        .collect()
}

fn clone_at(name: &str, path: &Path) -> crate::repos::Repo {
    crate::repos::Repo {
        name: name.to_owned(),
        path: path.to_path_buf(),
    }
}

#[test]
fn claude_code_finds_the_persons_own_skills_and_each_installed_plugins_by_its_namespaced_name() {
    let tmp = tempfile::tempdir().unwrap();
    let config = tmp.path().join("claude-config");
    skill(&config.join("skills"), "langfuse", "langfuse");
    std::fs::create_dir_all(config.join("skills/synced")).unwrap();
    let cache = tmp.path().join("cache/figma/2.2");
    skill(&cache.join("skills"), "frame", "frame");
    std::fs::create_dir_all(config.join("plugins")).unwrap();
    std::fs::write(
        config.join("plugins/installed_plugins.json"),
        serde_json::json!({
            "version": 2,
            "plugins": {
                "figma@official": [
                    {"scope": "project", "installPath": tmp.path().join("nowhere")},
                    {"scope": "user", "installPath": cache},
                ]
            }
        })
        .to_string(),
    )
    .unwrap();
    let chat = [("CLAUDE_CONFIG_DIR".to_owned(), config.display().to_string())];
    let choices = Choices::from_text(Some("[harness_skills.claude]\nlangfuse = false\n"), None);

    let found = harness_skills(&CLAUDE_CODE, &env(&chat, tmp.path()), &choices);

    assert_eq!(
        rows(&found),
        [
            ("langfuse", Origin::Harness, "you".to_owned(), true, true),
            (
                "figma:frame",
                Origin::Harness,
                "figma".to_owned(),
                false,
                false
            ),
        ],
        "a folder that is no whole skill is not listed"
    );
    assert_eq!(
        found[1].switch,
        Switch::Not(
            "Claude Code turns a plugin's skill off only with the whole plugin, figma".to_owned()
        )
    );
    assert_eq!(
        found[0].skill.path,
        config.join("skills/langfuse/SKILL.md"),
        "said where it lies"
    );
}

#[test]
fn codex_finds_the_persons_own_and_each_plugins_newest_version() {
    let tmp = tempfile::tempdir().unwrap();
    let codex = tmp.path().join("codex-home");
    skill(&codex.join("skills"), "notes", "notes");
    skill(&codex.join("skills/.system"), "imagegen", "imagegen");
    skill(
        &codex.join("plugins/cache/zmkt/zp/0.0.1/skills"),
        "old",
        "zp-old",
    );
    skill(
        &codex.join("plugins/cache/zmkt/zp/0.0.2/skills"),
        "zp-skill",
        "zp-skill",
    );
    std::fs::write(
        codex.join("config.toml"),
        "[plugins.\"zp@zmkt\"]\nenabled = true\n",
    )
    .unwrap();
    let chat = [("CODEX_HOME".to_owned(), codex.display().to_string())];
    let choices = Choices::from_text(
        Some("[harness_skills.codex]\n\"zp:zp-skill\" = false\n"),
        None,
    );

    let found = harness_skills(&CODEX, &env(&chat, tmp.path()), &choices);

    assert_eq!(
        rows(&found),
        [
            ("notes", Origin::Harness, "you".to_owned(), true, false),
            ("zp:zp-skill", Origin::Harness, "zp".to_owned(), true, true),
        ]
    );
}

#[test]
fn opencode_finds_the_skills_of_its_own_config_folder() {
    let tmp = tempfile::tempdir().unwrap();
    let config = tmp.path().join("xdg");
    skill(&config.join("opencode/skill"), "one", "one");
    skill(&config.join("opencode/skills"), "two", "two");
    let chat = [("XDG_CONFIG_HOME".to_owned(), config.display().to_string())];

    let found = harness_skills(&OPENCODE, &env(&chat, tmp.path()), &Choices::default());

    assert_eq!(
        rows(&found),
        [
            ("one", Origin::Harness, "you".to_owned(), true, false),
            ("two", Origin::Harness, "you".to_owned(), true, false),
        ]
    );
}

/// A clone `api` in a temporary directory, holding `.claude/skills/review`.
fn a_clone() -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let clone = tmp.path().join("api");
    skill(&clone.join(".claude/skills"), "review", "review");
    (tmp, clone)
}

#[test]
fn a_repo_skill_is_found_where_the_harness_reads_a_clone_and_owned_by_the_repo() {
    let (_tmp, clone) = a_clone();
    skill(&clone.join(".agents/skills"), "ship", "ship");
    let clones = [clone_at("api", &clone)];
    let choices = Choices::from_text(Some("[harness_skills.codex]\nship = false\n"), None);

    let claude = repo_skills(&CLAUDE_CODE, &Choices::default(), &clones);
    let codex = repo_skills(&CODEX, &choices, &clones);

    assert_eq!(
        rows(&claude),
        [("review", Origin::Repo, "api".to_owned(), true, false)]
    );
    assert_eq!(
        rows(&codex),
        [("ship", Origin::Repo, "api".to_owned(), true, true)]
    );
}

#[cfg(unix)]
#[test]
fn a_link_in_a_clone_is_named_and_never_read_through() {
    let (tmp, clone) = a_clone();
    let elsewhere = tmp.path().join("elsewhere");
    skill(&elsewhere, "secret", "secret");
    std::os::unix::fs::symlink(
        elsewhere.join("secret"),
        clone.join(".claude/skills/linked"),
    )
    .unwrap();
    let other = tmp.path().join("other");
    skill(&other.join(".claude/skills"), "theirs", "theirs");
    let linked_top = tmp.path().join("linked-top");
    std::fs::create_dir_all(&linked_top).unwrap();
    std::os::unix::fs::symlink(other.join(".claude"), linked_top.join(".claude")).unwrap();

    let found = repo_skills(
        &CLAUDE_CODE,
        &Choices::default(),
        &[clone_at("api", &clone), clone_at("linked-top", &linked_top)],
    );

    assert_eq!(
        rows(&found),
        [
            ("linked", Origin::Repo, "api".to_owned(), true, false),
            ("review", Origin::Repo, "api".to_owned(), true, false),
        ],
        "a `.claude` that is a link lists nothing, and a skill folder that is one is named"
    );
    assert!(
        found[0].skill.description.contains("does not follow"),
        "{}",
        found[0].skill.description
    );
}

#[cfg(unix)]
#[test]
fn a_repo_skill_whose_folder_is_swapped_for_a_link_after_it_was_found_is_never_loaded_through() {
    // SK-36: a repo skill's folder is held from the clone's folder down, so a `skills/` swapped
    // for a link to one outside the clone after discovery is refused, naming nothing outside.
    let tmp = tempfile::tempdir().unwrap();
    let clone = tmp.path().join("ws/api");
    skill(&clone.join(".claude/skills"), "review", "review");
    let outside = tmp.path().join("outside");
    skill(&outside, "review", "review");
    std::fs::write(outside.join("review/secret.txt"), "no\n").unwrap();
    let found = repo_skills(
        &CLAUDE_CODE,
        &Choices::default(),
        &[clone_at("api", &clone)],
    );
    let set = purlis_skills::SkillSet::default().with_outside(purlis_skills::Route::Mcp, found);
    let review = purlis_skills::by_name(&set, "review").expect("found");
    let honours = crate::harness::Harness::ClaudeCode.skill_frontmatter();
    assert!(
        purlis_skills::load(review, &honours).is_ok(),
        "loaded before the swap"
    );

    let at = clone.join(".claude/skills");
    std::fs::rename(&at, clone.join(".claude/was-skills")).unwrap();
    std::os::unix::fs::symlink(&outside, &at).unwrap();

    let refused = purlis_skills::load(review, &honours).unwrap_err();
    assert!(
        refused.ends_with(&purlis_skills::Refusal::FolderLink.to_string()),
        "{refused}"
    );
}

#[test]
fn a_folder_of_more_names_than_purlis_reads_lists_none_of_its_skills_and_says_so() {
    // SK-44 (#3133): a clone's skills folder is the chat's to fill, and `purlis skill list`
    // lints each skill found. Past the most names purlis reads of one folder, it reads none of
    // them and lists the folder once, saying why.
    let (_tmp, clone) = a_clone();
    let dir = clone.join(".claude/skills");
    for n in 0..crate::harness_plugin::MOST_NAMES {
        std::fs::write(dir.join(format!("f{n}")), "").unwrap();
    }

    let found = repo_skills(
        &CLAUDE_CODE,
        &Choices::default(),
        &[clone_at("api", &clone)],
    );

    assert_eq!(found.len(), 1, "{:?}", rows(&found));
    assert_eq!(found[0].skill.name, "skills");
    assert!(
        found[0].skill.description.contains(&format!(
            "more than {} names",
            crate::harness_plugin::MOST_NAMES
        )),
        "{}",
        found[0].skill.description
    );
}

#[test]
fn a_repo_skill_whose_skill_md_is_larger_than_purlis_reads_of_a_skills_file_is_not_listed() {
    // SK-44 (#3133): one limit for a skill's file, wherever it is read from.
    let (_tmp, clone) = a_clone();
    let big = clone.join(".claude/skills/big");
    std::fs::create_dir_all(&big).unwrap();
    let pad = "x".repeat(usize::try_from(purlis_skills::MOST_BYTES).unwrap());
    std::fs::write(
        big.join("SKILL.md"),
        format!("---\nname: big\ndescription: Too big.\n---\n\n{pad}\n"),
    )
    .unwrap();

    let found = repo_skills(
        &CLAUDE_CODE,
        &Choices::default(),
        &[clone_at("api", &clone)],
    );

    assert_eq!(
        rows(&found),
        [("review", Origin::Repo, "api".to_owned(), true, false)]
    );
}

#[cfg(unix)]
#[test]
fn a_repo_skill_whose_skill_md_has_a_second_name_is_not_listed() {
    // SK-44 (#3133): read as every skill's own file is, a hard link reaching past it refused.
    let (tmp, clone) = a_clone();
    let elsewhere = tmp.path().join("elsewhere");
    skill(&elsewhere, "twin", "twin");
    std::fs::create_dir_all(clone.join(".claude/skills/twin")).unwrap();
    std::fs::hard_link(
        elsewhere.join("twin/SKILL.md"),
        clone.join(".claude/skills/twin/SKILL.md"),
    )
    .unwrap();

    let found = repo_skills(
        &CLAUDE_CODE,
        &Choices::default(),
        &[clone_at("api", &clone)],
    );

    assert_eq!(
        rows(&found),
        [("review", Origin::Repo, "api".to_owned(), true, false)]
    );
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;

use super::*;
use crate::{Honours, ManualOnly, Origin, Route, Skill};

/// A harness that reads nothing of a frontmatter purlis does not read itself.
const PLAIN: Honours = Honours {
    reads: &[],
    manual: ManualOnly::Listing,
    agents: &["general-purpose"],
};

fn skill(dir: &Path, name: &str, description: &str, more: &str) -> Effective {
    let folder = dir.join(name);
    std::fs::create_dir_all(&folder).unwrap();
    let path = folder.join("SKILL.md");
    std::fs::write(
        &path,
        format!("---\nname: {name}\ndescription: {description}\n{more}---\n\n# {name}\n"),
    )
    .unwrap();
    Effective {
        skill: Skill {
            name: name.to_owned(),
            description: description.to_owned(),
            path,
        },
        route: Route::Mcp,
        origin: Origin::Project,
        local: false,
        deprecated: None,
    }
}

fn set(effective: Vec<Effective>) -> SkillSet {
    SkillSet {
        effective,
        ..SkillSet::default()
    }
}

#[test]
fn find_answers_the_skills_a_word_is_in_name_first_and_nothing_for_no_match() {
    let dir = tempfile::tempdir().unwrap();
    let set = set(vec![
        skill(dir.path(), "deploy", "Ship the service to staging.", ""),
        skill(
            dir.path(),
            "review",
            "Review a change before you deploy it.",
            "",
        ),
        skill(dir.path(), "notes", "Keep meeting notes.", ""),
    ]);

    let found: Vec<&str> = find(&set, "Deploy!")
        .iter()
        .map(|e| e.skill.name.as_str())
        .collect();
    assert_eq!(found, ["deploy", "review"], "a name match ranks first");
    assert!(find(&set, "kubernetes").is_empty());
    assert!(
        find(&set, "  ").is_empty(),
        "an empty query matches nothing"
    );
}

#[test]
fn find_answers_at_most_ten() {
    let dir = tempfile::tempdir().unwrap();
    let set = set((0..15)
        .map(|n| skill(dir.path(), &format!("tool-{n:02}"), "A tool.", ""))
        .collect());
    assert_eq!(find(&set, "tool").len(), MOST_FOUND);
}

#[test]
fn a_skill_is_named_by_its_name_or_after_a_namespace_and_never_outside_the_set() {
    let dir = tempfile::tempdir().unwrap();
    let set = set(vec![skill(dir.path(), "deploy", "Ship it.", "")]);
    assert!(by_name(&set, "deploy").is_some());
    assert!(by_name(&set, "skills:deploy").is_some());
    assert!(by_name(&set, " deploy ").is_some());
    // On disk, and not in the set: never found.
    skill(dir.path(), "hidden", "Not granted.", "");
    assert!(by_name(&set, "hidden").is_none());
}

#[test]
fn a_load_hands_back_the_skill_md_and_its_other_files_by_path() {
    let dir = tempfile::tempdir().unwrap();
    let deploy = skill(dir.path(), "deploy", "Ship it.", "");
    let folder = dir.path().join("deploy");
    std::fs::create_dir_all(folder.join("scripts")).unwrap();
    std::fs::write(folder.join("scripts/ship.sh"), "echo ship\n").unwrap();
    std::fs::write(folder.join("reference.md"), "more\n").unwrap();
    // A nested SKILL.md is no file of this skill's.
    std::fs::create_dir_all(folder.join("inner")).unwrap();
    std::fs::write(folder.join("inner/SKILL.md"), "---\nname: inner\n---\n").unwrap();

    let loaded = load(&deploy, &PLAIN).unwrap();
    assert!(loaded.text.contains("# deploy"), "{}", loaded.text);
    assert_eq!(
        loaded.files,
        [folder.join("reference.md"), folder.join("scripts/ship.sh")]
    );
    assert!(!loaded.narrowing.any());
}

#[cfg(unix)]
#[test]
fn a_load_follows_no_link_out_of_the_skill() {
    let dir = tempfile::tempdir().unwrap();
    let outside = dir.path().join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("secret.txt"), "no\n").unwrap();
    let deploy = skill(dir.path(), "deploy", "Ship it.", "");
    let folder = dir.path().join("deploy");
    std::os::unix::fs::symlink(&outside, folder.join("linked")).unwrap();
    std::os::unix::fs::symlink(outside.join("secret.txt"), folder.join("secret.txt")).unwrap();

    let loaded = load(&deploy, &PLAIN).unwrap();
    assert!(loaded.files.is_empty(), "{:?}", loaded.files);

    // A SKILL.md that is itself a link is not loaded.
    let linked = skill(dir.path(), "linked-md", "Linked.", "");
    std::fs::remove_file(&linked.skill.path).unwrap();
    std::os::unix::fs::symlink(outside.join("secret.txt"), &linked.skill.path).unwrap();
    let refused = load(&linked, &PLAIN).unwrap_err();
    assert!(refused.contains("not a plain file"), "{refused}");
}

#[test]
fn what_a_skill_narrows_is_read_from_its_frontmatter() {
    let dir = tempfile::tempdir().unwrap();
    let narrow = skill(
        dir.path(),
        "audit",
        "Read only.",
        "disallowed-tools: Write, Edit,, Bash\ncontext: fork\n",
    );
    let loaded = load(&narrow, &PLAIN).unwrap();
    assert_eq!(loaded.narrowing.tools_taken, ["Write", "Edit", "Bash"]);
    assert!(loaded.narrowing.fork);
    assert_eq!(
        loaded.narrowing.said(),
        "takes away Write, Edit and Bash, and runs in a fork of the chat"
    );
    assert!(!narrowing("---\nname: x\ncontext: inline\n---\n").any());
}

#[test]
fn a_load_hands_back_the_narrowed_skill_md_and_refuses_one_purlis_hands_no_chat() {
    let dir = tempfile::tempdir().unwrap();
    let wide = skill(dir.path(), "wide", "Wide.", "allowed-tools: Bash\n");
    let loaded = load(&wide, &PLAIN).unwrap();
    assert!(!loaded.text.contains("allowed-tools"), "{}", loaded.text);
    let respelled = skill(dir.path(), "respelled", "Wide.", "allowedTools: Bash\n");
    let refused = load(&respelled, &PLAIN).unwrap_err();
    assert!(refused.contains("could not be loaded"), "{refused}");
}

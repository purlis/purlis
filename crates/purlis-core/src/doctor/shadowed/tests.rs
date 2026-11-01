// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::doctor::Status;

#[test]
fn the_shipped_skills_are_the_plugin_s_own() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../app/src-tauri/plugin/skills");
    let mut on_disk: Vec<String> = std::fs::read_dir(&dir)
        .expect("the plugin's skills")
        .flatten()
        .filter(|e| e.path().join("SKILL.md").is_file())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    on_disk.sort();
    assert_eq!(on_disk, crate::skills::SHIPPED);
}

#[test]
fn a_project_with_no_copy_has_nothing_shadowed() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("docs")).unwrap();
    std::fs::write(dir.path().join("docs/runbook.md"), "# ours\n").unwrap();
    let hit = shadowed(dir.path()).unwrap();
    assert_eq!(hit, Shadowed::default());
    assert_eq!(said(&hit).status, Status::Ok);
}

#[test]
fn a_copy_of_a_shipped_page_is_named() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::create_dir_all(root.join("docs")).unwrap();
    std::fs::write(root.join("docs/secrets.md"), "# ours\n").unwrap();
    std::fs::write(root.join("docs/runbook.md"), "# ours\n").unwrap();
    let hit = shadowed(root).unwrap();
    assert_eq!(hit.docs, ["secrets"]);
    assert!(hit.skills.is_empty(), "{hit:?}");
    let r = said(&hit);
    assert_eq!(r.status, Status::Warn);
    assert_eq!(r.detail, "docs/secrets.md duplicate what purlis ships");
    assert!(!r.hint.to_lowercase().contains("delete"), "{r:?}");
}

#[test]
fn a_copy_of_a_shipped_skill_is_named() {
    let dir = tempfile::tempdir().unwrap();
    let skills = dir.path().join(".claude/skills");
    std::fs::create_dir_all(skills.join("purlis-secrets")).unwrap();
    std::fs::write(
        skills.join("purlis-secrets/SKILL.md"),
        "---\nname: purlis-secrets\n---\n",
    )
    .unwrap();
    // A directory is a copy only when it holds a skill, and only of a skill purlis ships.
    std::fs::create_dir_all(skills.join("purlis-browser")).unwrap();
    std::fs::create_dir_all(skills.join("ours")).unwrap();
    std::fs::write(skills.join("ours/SKILL.md"), "---\nname: ours\n---\n").unwrap();
    // And one kept from before the prefix.
    std::fs::create_dir_all(skills.join("handoff")).unwrap();
    std::fs::write(skills.join("handoff/SKILL.md"), "---\nname: handoff\n---\n").unwrap();
    assert_eq!(
        shadowed(dir.path()).unwrap().skills,
        ["handoff", "purlis-secrets"]
    );
}

#[test]
fn skills_and_pages_are_said_together() {
    let r = said(&Shadowed {
        skills: vec!["browser".into(), "secrets".into()],
        docs: vec!["hooks".into()],
    });
    assert_eq!(
        r.detail,
        "skills browser, secrets · docs/hooks.md duplicate what purlis ships"
    );
}

#[test]
fn purlis_s_own_repository_shadows_nothing() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    assert_eq!(shadowed(&repo).unwrap(), Shadowed::default());
}

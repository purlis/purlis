// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A skill ships an artifact template (#5974): placed by name from the chat's own skill set,
//! copied without its data, never over another page, and left as it is on a run again.

use super::*;
use crate::artifact::{DIR, PAGE};

/// A plane with workspace alpha and a skill `backlog-sync` of alpha shipping `backlog-state`,
/// a page with a sample data file beside it.
fn plane() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    std::fs::write(root.join(crate::plane::MANIFEST), "schema = 2\n").unwrap();
    std::fs::create_dir_all(root.join("workspaces/alpha")).unwrap();
    let skill = root.join("workspaces/alpha/skills/backlog-sync");
    std::fs::create_dir_all(skill.join("artifacts/backlog-state")).unwrap();
    std::fs::write(
        skill.join("SKILL.md"),
        "---\nname: backlog-sync\ndescription: Sync the backlog.\n---\n\n# Sync\n",
    )
    .unwrap();
    let template = skill.join("artifacts/backlog-state");
    std::fs::write(template.join(PAGE), "<title>Backlog state</title>").unwrap();
    std::fs::write(template.join("style.css"), "p { margin: 0 }").unwrap();
    std::fs::write(template.join("data.json"), "{\"sample\": true}").unwrap();
    (dir, root)
}

fn set(root: &Path) -> crate::skills::SkillSet {
    crate::skills::of_a_chat(None, root, Some("alpha"), None, crate::skills::Route::Mcp)
}

fn placed(root: &Path, skill: &str, name: &str) -> Result<Placed, String> {
    place_from_skill(
        root,
        &root.join("workspaces/alpha"),
        &set(root),
        skill,
        name,
    )
}

fn copy(root: &Path) -> PathBuf {
    root.join("workspaces/alpha")
        .join(DIR)
        .join("backlog-state")
}

#[test]
fn a_skill_s_template_is_copied_into_the_workspace_without_its_data() {
    let (_dir, root) = plane();
    assert_eq!(
        placed(&root, "backlog-sync", "backlog-state"),
        Ok(Placed::Copied)
    );
    assert_eq!(
        std::fs::read_to_string(copy(&root).join(PAGE)).unwrap(),
        "<title>Backlog state</title>"
    );
    assert!(copy(&root).join("style.css").is_file());
    assert!(
        !copy(&root).join("data.json").exists(),
        "data is each person's own, never the skill's sample"
    );
}

#[test]
fn a_run_again_leaves_the_same_template_and_its_data_as_they_are() {
    let (_dir, root) = plane();
    placed(&root, "backlog-sync", "backlog-state").unwrap();
    std::fs::write(copy(&root).join("data.json"), "{\"mine\": 1}").unwrap();
    assert_eq!(
        placed(&root, "backlog-sync", "backlog-state"),
        Ok(Placed::There)
    );
    assert_eq!(
        std::fs::read_to_string(copy(&root).join("data.json")).unwrap(),
        "{\"mine\": 1}"
    );
}

#[test]
fn a_folder_holding_another_page_is_never_written_over() {
    let (_dir, root) = plane();
    std::fs::create_dir_all(copy(&root)).unwrap();
    std::fs::write(copy(&root).join(PAGE), "<title>Someone else's</title>").unwrap();
    let why = placed(&root, "backlog-sync", "backlog-state").unwrap_err();
    assert!(why.contains("already a folder 'backlog-state'"), "{why}");
    assert_eq!(
        std::fs::read_to_string(copy(&root).join(PAGE)).unwrap(),
        "<title>Someone else's</title>"
    );
}

#[test]
fn a_skill_not_in_the_chat_s_set_or_a_template_it_does_not_ship_places_nothing() {
    let (_dir, root) = plane();
    let why = placed(&root, "nobody", "backlog-state").unwrap_err();
    assert!(why.contains("no skill named 'nobody'"), "{why}");
    let why = placed(&root, "backlog-sync", "missing").unwrap_err();
    assert!(why.contains("has no folder artifacts/missing"), "{why}");
    let why = placed(&root, "backlog-sync", "../../../workspaces").unwrap_err();
    assert!(why.contains("never a path"), "{why}");
    assert!(!root.join("workspaces/alpha").join(DIR).exists());
}

#[test]
fn a_template_holding_a_secret_or_linking_out_of_its_skill_places_nothing() {
    let (_dir, root) = plane();
    let template = root.join("workspaces/alpha/skills/backlog-sync/artifacts/backlog-state");
    std::fs::write(
        template.join(PAGE),
        format!(
            "<title>x</title><p>{}</p>",
            ["ghp", "_0123456789abcdefABCDEFghij0123456789"].concat()
        ),
    )
    .unwrap();
    let why = placed(&root, "backlog-sync", "backlog-state").unwrap_err();
    assert!(why.contains("shaped like a secret"), "{why}");
    assert!(!copy(&root).exists());

    #[cfg(unix)]
    {
        let outside = root.join("elsewhere");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join(PAGE), "<title>out</title>").unwrap();
        std::os::unix::fs::symlink(
            &outside,
            root.join("workspaces/alpha/skills/backlog-sync/artifacts/linked"),
        )
        .unwrap();
        let why = placed(&root, "backlog-sync", "linked").unwrap_err();
        assert!(why.contains("not a folder inside its skill"), "{why}");
    }
}

#[cfg(unix)]
#[test]
fn a_committed_artifacts_link_out_of_the_workspace_is_never_written_through() {
    let (_dir, root) = plane();
    let outside = root.join("elsewhere");
    std::fs::create_dir_all(&outside).unwrap();
    std::os::unix::fs::symlink(&outside, root.join("workspaces/alpha").join(DIR)).unwrap();
    let touched = || std::fs::metadata(&outside).unwrap().modified().unwrap();
    let before = touched();
    std::thread::sleep(std::time::Duration::from_millis(20));
    let why = placed(&root, "backlog-sync", "backlog-state").unwrap_err();
    assert!(why.contains("is a link"), "{why}");
    assert_eq!(
        touched(),
        before,
        "no folder was made and removed through the link"
    );
    assert!(
        std::fs::read_dir(&outside).unwrap().next().is_none(),
        "nothing is made through the link"
    );
}

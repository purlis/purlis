// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The seam for what the Skill studio offers when it opens (SK-22, #2785): the three starts, the
//! offered sources less the project's own, and the move SK-21 left for here, on planes made in a
//! temporary folder.

use std::path::Path;

use super::*;
use crate::skills::{in_the_studio, studio_rows};

/// A plane with a workspace `ops`.
fn plane() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("charter.toml"), "schema = 1\n").unwrap();
    std::fs::create_dir_all(tmp.path().join("workspaces/ops")).unwrap();
    tmp
}

/// A skill folder `name` under `dir` of the plane at `root`.
fn skill(root: &Path, dir: &str, name: &str) {
    let folder = root.join(dir).join(name);
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(
        folder.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: Does {name}.\n---\n\nDo it.\n"),
    )
    .unwrap();
}

/// What the studio of the plane at `root` offers, for a chat in `workspace`, as `persona`.
fn opened(root: &Path, workspace: Option<&str>, persona: Option<&str>) -> FirstOpen {
    let set = in_the_studio(None, root, workspace, persona);
    first_open(root, &studio_rows(&set, root, &[]))
}

#[test]
fn the_committed_list_reads_whole_and_every_source_on_it_is_one_a_team_could_name() {
    let all = read(OFFERED).expect("offered.toml reads");
    assert!(!all.is_empty());
    for one in &all {
        assert!(
            one.source.remote.url.starts_with("https://"),
            "{}",
            one.name
        );
        assert!(one.source.check().is_ok(), "{}", one.name);
        assert!(one.source.sign_in.is_none(), "{}", one.name);
        assert!(!one.why.trim().is_empty(), "{}", one.name);
    }
    assert_eq!(offered(), {
        let mut sorted = all.clone();
        sorted.sort_by_key(|one| one.offering != Offering::Catalogue);
        sorted
    });
    assert!(
        all.iter()
            .any(|one| one.source.remote.url == "https://github.com/anthropics/skills")
    );
}

#[test]
fn a_list_with_a_key_purlis_does_not_read_or_a_source_no_team_could_name_offers_none() {
    let good = "[[source]]\noffering = \"well-known\"\nname = \"A\"\nurl = \
                \"https://example.com/a\"\nfolder = \"skills\"\nwhy = \"Because.\"\n";
    assert_eq!(read(good).unwrap().len(), 1);
    for bad in [
        format!("{good}extra = 1\n"),
        good.replace("well-known", "sponsored"),
        good.replace("https://example.com/a", "/srv/skills.git"),
        good.replace("https://example.com/a", "http://example.com/a"),
        good.replace("\"skills\"", "\"../up\""),
        good.replace("Because.", " "),
        format!("{good}[[source]]\noffering = \"well-known\"\n"),
    ] {
        assert!(read(&bad).is_err(), "{bad}");
    }
}

#[test]
fn a_project_with_no_skill_of_its_own_is_offered_the_three_starts() {
    let tmp = plane();
    assert!(opened(tmp.path(), None, None).starts);
    assert!(opened(tmp.path(), Some("ops"), None).starts);
}

#[test]
fn a_skill_of_the_projects_own_at_any_level_takes_the_starts_away() {
    let project = plane();
    skill(project.path(), "skills", "deploy");
    assert!(!opened(project.path(), None, None).starts);

    let workspace = plane();
    skill(workspace.path(), "workspaces/ops/skills", "triage");
    assert!(!opened(workspace.path(), Some("ops"), None).starts);
}

#[test]
fn every_offered_source_is_offered_until_the_project_browses_it() {
    let tmp = plane();
    let all = offered();
    assert_eq!(opened(tmp.path(), None, None).sources, all);

    let first = &all[0].source;
    // Listed with the URL spelt another way: a trailing `.git`, a capital, a slash.
    let spelt = format!("{}.git/", first.remote.url.replace("github", "GitHub"));
    std::fs::write(
        tmp.path().join("charter.toml"),
        format!("schema = 1\n\n[[skill_sources]]\nurl = \"{spelt}\"\nfolder = \"skills\"\n"),
    )
    .unwrap();
    let left = opened(tmp.path(), None, None).sources;
    assert_eq!(left.len(), all.len() - 1);
    assert!(left.iter().all(|one| one.source.remote != first.remote));
}

#[test]
fn a_project_whose_sources_cannot_be_read_is_offered_none() {
    let tmp = plane();
    std::fs::write(
        tmp.path().join("charter.toml"),
        "schema = 1\n\n[[skill_sources]]\nurl = \"https://example.com/a\"\nbogus = 1\n",
    )
    .unwrap();
    assert!(opened(tmp.path(), None, None).sources.is_empty());
}

#[test]
fn skills_still_in_the_old_folder_are_offered_the_move_and_only_whole_folders_count() {
    let tmp = plane();
    assert_eq!(opened(tmp.path(), None, None).unmoved, None);

    skill(tmp.path(), ".claude/skills", "deploy");
    skill(tmp.path(), ".claude/skills", "notify");
    // A folder with no SKILL.md and a loose file are not skills.
    std::fs::create_dir_all(tmp.path().join(".claude/skills/notes")).unwrap();
    std::fs::write(tmp.path().join(".claude/skills/README.md"), "hi").unwrap();
    assert_eq!(
        opened(tmp.path(), None, None).unmoved,
        Some(Unmoved {
            skills: 2,
            fix: FixId::ProjectSkills,
        })
    );
}

#[cfg(unix)]
#[test]
fn an_old_folder_reached_through_a_link_is_never_read() {
    let tmp = plane();
    let elsewhere = tempfile::tempdir().unwrap();
    skill(elsewhere.path(), "skills", "deploy");
    std::fs::create_dir_all(tmp.path().join(".claude")).unwrap();
    std::os::unix::fs::symlink(
        elsewhere.path().join("skills"),
        tmp.path().join(".claude/skills"),
    )
    .unwrap();
    assert_eq!(opened(tmp.path(), None, None).unmoved, None);

    // And a skill folder inside it that is a link is not counted.
    let other = plane();
    std::fs::create_dir_all(other.path().join(".claude/skills")).unwrap();
    std::os::unix::fs::symlink(
        elsewhere.path().join("skills/deploy"),
        other.path().join(".claude/skills/deploy"),
    )
    .unwrap();
    assert_eq!(opened(other.path(), None, None).unmoved, None);
}

#[test]
fn a_project_that_moved_its_skills_is_not_offered_the_move_again() {
    let tmp = plane();
    skill(tmp.path(), ".claude/skills", "deploy");
    std::fs::write(
        tmp.path().join("charter.toml"),
        "schema = 1\nrequires = [{ feature = \"project-skills\" }]\n",
    )
    .unwrap();
    assert_eq!(opened(tmp.path(), None, None).unmoved, None);
}

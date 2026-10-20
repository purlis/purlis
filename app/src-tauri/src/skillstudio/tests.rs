// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

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

    let studio = studio(&root, Some(&built_in), true);

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

    let studio = studio(tmp.path(), None, false);

    assert_eq!(
        studio,
        Studio {
            on: false,
            skills: Vec::new()
        }
    );
}

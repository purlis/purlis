// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::path::PathBuf;

use super::listed_folder;

/// A project with a skill of its own, `deploy`; a clone `api` of workspace `alpha` with a
/// skill of its own, `lint`; and a built-in skill, `purlis-secrets`, in the app's folder.
fn a_project() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(tmp.path()).unwrap();
    let plane = root.join("plane");
    std::fs::create_dir_all(&plane).unwrap();
    std::fs::write(plane.join("charter.toml"), "[plane]\n").unwrap();
    std::fs::create_dir_all(plane.join("workspaces/alpha/api/.git")).unwrap();
    let built_in = root.join("bundle/skills");
    for (dir, name) in [
        (plane.join("skills"), "deploy"),
        (plane.join("workspaces/alpha/api/.agents/skills"), "lint"),
        (built_in.clone(), "purlis-secrets"),
    ] {
        std::fs::create_dir_all(dir.join(name)).unwrap();
        std::fs::write(
            dir.join(name).join("SKILL.md"),
            format!("---\nname: {name}\ndescription: Does {name}.\n---\n"),
        )
        .unwrap();
    }
    (tmp, plane, built_in)
}

fn env(tmp: &tempfile::TempDir) -> crate::harness_plugin::Env<'static> {
    crate::harness_plugin::Env {
        chat: &[],
        home: Some(tmp.path().join("home")),
        process: false,
    }
}

#[test]
fn the_folder_of_a_skill_the_studio_lists_is_found_inside_the_project_or_out_of_it() {
    let (tmp, plane, built_in) = a_project();
    let found = |folder: &str| {
        listed_folder(
            Some(&built_in),
            &plane,
            Some("alpha"),
            None,
            &env(&tmp),
            folder,
        )
    };
    assert_eq!(found("skills/deploy"), Ok(plane.join("skills/deploy")));
    assert_eq!(
        found("workspaces/alpha/api/.agents/skills/lint"),
        Ok(plane.join("workspaces/alpha/api/.agents/skills/lint"))
    );
    let secrets = built_in.join("purlis-secrets");
    assert_eq!(found(&secrets.to_string_lossy()), Ok(secrets));
}

#[test]
fn a_folder_no_skill_the_studio_lists_is_in_is_refused() {
    let (tmp, plane, built_in) = a_project();
    let home = tmp.path().join("home").to_string_lossy().into_owned();
    for folder in [
        "",
        "skills",
        "workspaces/alpha/api",
        "skills/deploy/..",
        "/etc",
        &home,
    ] {
        assert_eq!(
            listed_folder(
                Some(&built_in),
                &plane,
                Some("alpha"),
                None,
                &env(&tmp),
                folder
            ),
            Err(format!(
                "'{folder}' is not the folder of a skill the studio lists here"
            )),
            "{folder}"
        );
    }
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! #5371: every scanner that finds a workspace's clones finds one in `repos/<name>` as it finds
//! one at the root (ADR 0114), against real planes on disk and real git. Each asks the scanner
//! itself, never [`purlis_core::repofolder`], which they all read now.

use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use purlis_core::{codelink, doctor, guest, repos, sandbox, skills, wslayer};

use crate::support;

/// The two places a clone named `api` may be, below the workspace's folder.
const LAYOUTS: [&str; 2] = ["api", "repos/api"];

/// The `workspace clones` row of doctor, run on `plane`.
fn clones_row(plane: &std::path::Path) -> doctor::Row {
    doctor::Doctor::at(plane, plane, false, false)
        .run()
        .into_iter()
        .find(|row| row.name == "workspace clones")
        .expect("doctor checks the workspaces' clones")
}

#[test]
fn the_repo_list_holds_a_clone_in_either_layout() {
    purlis_core::unsteered!();
    for below in LAYOUTS {
        let f = support::plane_with_clone(below);

        let found = repos::clones(&f.plane, "alpha").unwrap();

        let rows: Vec<(String, PathBuf)> = found
            .repos
            .into_iter()
            .map(|repo| (repo.name, repo.path))
            .collect();
        assert_eq!(rows, [("api".to_string(), f.clone.clone())], "{below}");
        assert!(found.refused.is_empty(), "{below}: {:?}", found.refused);
    }
}

#[test]
fn a_clone_at_the_root_beside_its_name_in_repos_is_listed_once_from_repos() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("repos/api");
    let old = f.workspace().join("api");
    std::fs::create_dir_all(&old).unwrap();
    support::git(&old, &["init", "-q", "-b", "main", "."]);

    let found = repos::clones(&f.plane, "alpha").unwrap();

    let paths: Vec<PathBuf> = found.repos.into_iter().map(|repo| repo.path).collect();
    assert_eq!(paths, [f.clone.clone()]);
}

#[test]
fn a_link_where_a_clones_git_would_be_is_said_in_either_layout() {
    purlis_core::unsteered!();
    for below in LAYOUTS {
        let f = support::plane_with_clone("web");
        let linked = f.workspace().join(below);
        std::fs::create_dir_all(&linked).unwrap();
        std::os::unix::fs::symlink(f.clone.join(".git"), linked.join(".git")).unwrap();

        let found = repos::clones(&f.plane, "alpha").unwrap();

        let refused: Vec<&str> = found
            .refused
            .iter()
            .map(|(name, _)| name.as_str())
            .collect();
        assert_eq!(refused, ["api"], "{below}");
    }
}

#[test]
fn the_code_links_clones_are_found_in_either_layout() {
    purlis_core::unsteered!();
    for below in LAYOUTS {
        let f = support::plane_with_clone(below);

        assert_eq!(
            codelink::clones_in(&f.plane, "alpha").unwrap(),
            ["api"],
            "{below}"
        );
        assert_eq!(
            codelink::clones_in(&f.plane, "nowhere").unwrap(),
            Vec::<String>::new(),
            "a workspace with no folder holds no clone"
        );
    }
}

#[test]
fn the_code_link_leaves_a_clone_with_no_commit_where_it_is_in_either_layout() {
    purlis_core::unsteered!();
    for below in LAYOUTS {
        let f = support::plane_with_clone("web");
        let fresh = f.workspace().join(below);
        std::fs::create_dir_all(&fresh).unwrap();
        support::git(&fresh, &["init", "-q", "-b", "main", "."]);

        assert_eq!(
            codelink::clones_in(&f.plane, "alpha").unwrap(),
            ["web"],
            "{below}"
        );
    }
}

#[test]
fn a_clone_in_either_layout_gets_its_guest_exclude_block() {
    purlis_core::unsteered!();
    for below in LAYOUTS {
        let f = support::plane_with_clone(below);
        f.give_the_plane_a_layer();

        let rows = wslayer::wire(&f.plane, &f.workspace());

        let exclude =
            std::fs::read_to_string(f.clone.join(".git/info/exclude")).unwrap_or_default();
        assert!(
            exclude.contains(guest::EXCLUDE_BEGIN),
            "{below}: no exclude block in {exclude:?}: {rows:?}"
        );
    }
}

#[test]
fn doctor_counts_a_clone_in_either_layout() {
    purlis_core::unsteered!();
    for below in LAYOUTS {
        let f = support::plane_with_clone(below);

        let row = clones_row(&f.plane);

        assert!(
            row.detail.starts_with("1 clone across all workspaces"),
            "{below}: {}",
            row.detail
        );
    }
}

#[test]
fn doctor_says_a_repos_folder_it_cannot_read_and_still_counts_the_root() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("api");
    let shut = f.workspace().join("repos");
    std::fs::create_dir_all(shut.join("web")).unwrap();
    std::fs::set_permissions(&shut, std::fs::Permissions::from_mode(0o000)).unwrap();
    let readable = std::fs::read_dir(&shut).is_ok();

    let row = clones_row(&f.plane);

    std::fs::set_permissions(&shut, std::fs::Permissions::from_mode(0o755)).unwrap();
    if readable {
        // Root reads every folder, so there is nothing it cannot check.
        return;
    }
    assert!(
        row.detail.starts_with("1 clone across 1 of 1 workspace"),
        "{}",
        row.detail
    );
    assert!(
        row.detail
            .contains("workspaces/alpha/repos cannot be checked"),
        "{}",
        row.detail
    );
}

#[test]
fn the_sandbox_holds_the_git_folder_of_a_clone_in_either_layout() {
    purlis_core::unsteered!();
    for below in LAYOUTS {
        let f = support::plane_with_clone(below);

        let held = sandbox::planted::clones_purlis_works_in(&f.plane);

        assert!(held.contains(&f.clone), "{below}: {held:?}");
    }
}

#[test]
fn a_chat_in_a_clone_in_either_layout_reads_that_clones_skills() {
    purlis_core::unsteered!();
    for below in LAYOUTS {
        let f = support::plane_with_clone(below);

        let found = skills::clones_of_chat(&f.plane, "alpha", &f.clone);

        let names: Vec<String> = found.into_iter().map(|repo| repo.name).collect();
        assert_eq!(names, ["api"], "{below}");
    }
}

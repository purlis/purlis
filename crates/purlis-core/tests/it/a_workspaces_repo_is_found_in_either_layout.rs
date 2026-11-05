// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Where a workspace's repo is, in the layout before structure version 6 (`<ws>/<repo>`), the
//! one after it (`<ws>/repos/<repo>`) and a workspace holding both (ADR 0113), against real
//! planes on disk and real git.
//!
//! Asked only of [`purlis_core::repofolder`], the one place that knows both layouts: what a
//! repo's folder is, which repos a workspace holds, and where a new one goes.

use std::path::{Path, PathBuf};

use purlis_core::repofolder::{self, Layout, Refused};

use crate::support;

/// A plane with one empty workspace, `alpha`.
struct Plane {
    _dir: tempfile::TempDir,
    root: PathBuf,
}

impl Plane {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        // Resolved, as `support::plane_with_clone` does: macOS's temp dir is behind a link.
        let root = std::fs::canonicalize(dir.path()).unwrap();
        std::fs::write(root.join("charter.toml"), "schema = 1\n").unwrap();
        std::fs::create_dir_all(root.join("workspaces/alpha")).unwrap();
        Self { _dir: dir, root }
    }

    fn workspace(&self) -> PathBuf {
        self.root.join("workspaces/alpha")
    }

    /// A git repository at `below` the workspace's folder, made by `git init` as a clone is.
    fn repo_at(&self, below: &str) -> PathBuf {
        let at = self.workspace().join(below);
        std::fs::create_dir_all(&at).unwrap();
        support::git(&at, &["init", "-q", "-b", "main", "."]);
        at
    }

    fn folder(&self, repo: &str) -> PathBuf {
        repofolder::folder(&self.root, "alpha", repo).expect("a repo's name")
    }

    /// The repos the workspace holds, as `(name, folder below the workspace, layout)`.
    fn repos(&self) -> Vec<(String, PathBuf, Layout)> {
        let ws = self.workspace();
        repofolder::repos(&self.root, "alpha")
            .expect("the workspace reads")
            .into_iter()
            .map(|repo| {
                let below = repo.path.strip_prefix(&ws).unwrap().to_path_buf();
                (repo.name, below, repo.layout)
            })
            .collect()
    }
}

fn row(name: &str, below: &str, layout: Layout) -> (String, PathBuf, Layout) {
    (name.to_string(), Path::new(below).to_path_buf(), layout)
}

#[test]
fn an_old_layout_workspace_holds_its_repos_at_its_root() {
    purlis_core::unsteered!();
    let plane = Plane::new();
    plane.repo_at("api");
    plane.repo_at("web");

    assert_eq!(plane.folder("api"), plane.workspace().join("api"));
    assert_eq!(
        plane.repos(),
        [
            row("api", "api", Layout::Root),
            row("web", "web", Layout::Root),
        ]
    );
}

#[test]
fn a_new_layout_workspace_holds_its_repos_in_repos() {
    purlis_core::unsteered!();
    let plane = Plane::new();
    plane.repo_at("repos/api");
    plane.repo_at("repos/web");

    assert_eq!(plane.folder("api"), plane.workspace().join("repos/api"));
    // `repos/` itself is the folder they are in, never a repo of its own.
    assert_eq!(
        plane.repos(),
        [
            row("api", "repos/api", Layout::Repos),
            row("web", "repos/web", Layout::Repos),
        ]
    );
}

#[test]
fn a_mixed_workspace_holds_repos_in_both_places() {
    purlis_core::unsteered!();
    let plane = Plane::new();
    plane.repo_at("old");
    plane.repo_at("repos/new");

    assert_eq!(plane.folder("old"), plane.workspace().join("old"));
    assert_eq!(plane.folder("new"), plane.workspace().join("repos/new"));
    assert_eq!(
        plane.repos(),
        [
            row("new", "repos/new", Layout::Repos),
            row("old", "old", Layout::Root),
        ]
    );
}

#[test]
fn a_repo_in_repos_wins_over_one_of_the_same_name_at_the_root() {
    purlis_core::unsteered!();
    let plane = Plane::new();
    plane.repo_at("api");
    plane.repo_at("repos/api");

    assert_eq!(plane.folder("api"), plane.workspace().join("repos/api"));
    assert_eq!(plane.repos(), [row("api", "repos/api", Layout::Repos)]);
}

#[test]
fn a_repo_cloned_nowhere_yet_is_answered_where_an_old_one_would_be() {
    purlis_core::unsteered!();
    // A caller that joins `<ws>/<repo>` by hand today and then asks whether it exists gets the
    // same answer through the resolver: nothing a person sees moves until a clone is in `repos/`.
    let plane = Plane::new();
    std::fs::create_dir_all(plane.workspace().join("repos")).unwrap();

    assert_eq!(plane.folder("api"), plane.workspace().join("api"));
}

#[test]
fn an_old_repo_named_repos_is_read_as_a_repo_and_no_new_repo_goes_in_it() {
    purlis_core::unsteered!();
    let plane = Plane::new();
    // A `repos/` with `.git` directly in it is an old clone, and what is inside it is that
    // repository's own files, even a nested repository: never a repo of the workspace's.
    plane.repo_at("repos");
    plane.repo_at("repos/vendored");

    assert_eq!(plane.folder("repos"), plane.workspace().join("repos"));
    assert_eq!(plane.folder("vendored"), plane.workspace().join("vendored"));
    assert_eq!(plane.repos(), [row("repos", "repos", Layout::Root)]);

    let refused = repofolder::new_folder(&plane.root, "alpha", "api")
        .expect_err("a new repo would land inside the repo named repos");
    assert!(
        matches!(refused, Refused::RepoNamedRepos { .. }),
        "{refused:?}"
    );
    assert!(
        refused.to_string().contains("rename"),
        "the refusal says what to do: {refused}"
    );
}

#[test]
fn a_new_repo_goes_in_repos_whatever_the_workspace_already_holds() {
    purlis_core::unsteered!();
    let plane = Plane::new();
    plane.repo_at("old");

    let at = repofolder::new_folder(&plane.root, "alpha", "api").expect("a place for it");

    assert_eq!(at, plane.workspace().join("repos/api"));
}

#[test]
fn a_workspaces_repos_leave_out_dot_names_branch_folders_and_its_own_stores() {
    purlis_core::unsteered!();
    let plane = Plane::new();
    plane.repo_at("api");
    plane.repo_at(".hidden");
    plane.repo_at("repos/.hidden");
    // A branch folder is a linked worktree under `.worktrees/`, never a repo.
    plane.repo_at(".worktrees/api/piece");
    // A store the workspace keeps of its own, even one somebody ran `git init` in.
    plane.repo_at("memory");
    std::fs::create_dir_all(plane.workspace().join("todos")).unwrap();
    std::fs::write(plane.workspace().join("workspace.json"), "{}\n").unwrap();
    // A folder of the person's own, with no repository in it.
    std::fs::create_dir_all(plane.workspace().join("notes")).unwrap();

    assert_eq!(plane.repos(), [row("api", "api", Layout::Root)]);
}

#[test]
fn a_name_that_is_not_one_folder_is_refused() {
    purlis_core::unsteered!();
    let plane = Plane::new();

    for name in ["../beta/api", "", "..", "a/b"] {
        assert!(
            matches!(
                repofolder::folder(&plane.root, "alpha", name),
                Err(Refused::NotARepoName(_))
            ),
            "{name:?}"
        );
        assert!(
            matches!(
                repofolder::new_folder(&plane.root, "alpha", name),
                Err(Refused::NotARepoName(_))
            ),
            "{name:?}"
        );
    }
}

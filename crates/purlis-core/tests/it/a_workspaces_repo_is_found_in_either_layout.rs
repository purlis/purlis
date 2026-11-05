// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Where a workspace's repo is, in the layout before structure version 6 (`<ws>/<repo>`), the
//! one after it (`<ws>/repos/<repo>`) and a workspace holding both (ADR 0113), against real
//! planes on disk and real git.
//!
//! Asked only of [`purlis_core::repofolder`], the one place that knows both layouts: what a
//! repo's folder is, which repos a workspace holds, and where a new one goes.

use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};

use purlis_core::repofolder::{self, Git, Holdings, Layout, Refused, Row, Trouble};
use purlis_core::repos;
use purlis_core::worktree::confine::Outside;

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

    fn holdings(&self) -> Holdings {
        repofolder::holdings(&self.root, "alpha").expect("the workspace reads")
    }

    /// A folder with a `.git` file in it, as a linked worktree has.
    fn linked_at(&self, below: &str) {
        let at = self.workspace().join(below);
        std::fs::create_dir_all(&at).unwrap();
        std::fs::write(at.join(".git"), "gitdir: /nowhere\n").unwrap();
    }

    /// A folder whose `.git` is a link to `target`.
    fn git_link_at(&self, below: &str, target: &Path) {
        let at = self.workspace().join(below);
        std::fs::create_dir_all(&at).unwrap();
        symlink(target, at.join(".git")).unwrap();
    }

    /// The repos the workspace holds, as `(name, folder below the workspace, layout)`.
    fn repos(&self) -> Vec<(String, PathBuf, Layout)> {
        let ws = self.workspace();
        repofolder::repos(&self.root, "alpha")
            .expect("the workspace reads")
            .into_iter()
            .map(|row| {
                let below = row.repo.path.strip_prefix(&ws).unwrap().to_path_buf();
                (row.repo.name, below, row.layout)
            })
            .collect()
    }
}

/// A row's name, where it is below the workspace, and what its `.git` is.
fn facts(plane: &Plane, rows: &[Row]) -> Vec<(String, PathBuf, Layout, Git)> {
    let ws = plane.workspace();
    rows.iter()
        .map(|row| {
            let below = row.repo.path.strip_prefix(&ws).unwrap().to_path_buf();
            (row.repo.name.clone(), below, row.layout, row.git)
        })
        .collect()
}

fn fact(name: &str, below: &str, layout: Layout, git: Git) -> (String, PathBuf, Layout, Git) {
    (name.to_string(), PathBuf::from(below), layout, git)
}

/// A git repository outside any workspace.
fn elsewhere() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let at = std::fs::canonicalize(dir.path()).unwrap();
    support::git(&at, &["init", "-q", "-b", "main", "."]);
    (dir, at)
}

/// Gives a folder its permissions back when the test ends, so the temp dir can be removed.
struct Shut(PathBuf);

impl Shut {
    fn folder(path: PathBuf) -> Self {
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();
        Self(path)
    }
}

impl Drop for Shut {
    fn drop(&mut self) {
        let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(0o755));
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

// ---------------------------------------------------------------------------------------
// One rule for what wins                                                                  #
// ---------------------------------------------------------------------------------------

#[test]
fn only_a_real_repo_in_repos_wins_over_a_repo_at_the_root() {
    purlis_core::unsteered!();
    let plane = Plane::new();
    let (_keep, outside) = elsewhere();
    for name in ["empty", "file", "dangling", "linked", "gitlink", "real"] {
        plane.repo_at(name);
    }
    let repos = plane.workspace().join("repos");
    std::fs::create_dir_all(repos.join("empty")).unwrap();
    std::fs::write(repos.join("file"), "a file\n").unwrap();
    symlink(repos.join("nothing-here"), repos.join("dangling")).unwrap();
    plane.linked_at("repos/linked");
    plane.git_link_at("repos/gitlink", &outside.join(".git"));
    plane.repo_at("repos/real");

    for name in ["empty", "file", "dangling", "linked", "gitlink"] {
        assert_eq!(plane.folder(name), plane.workspace().join(name), "{name}");
    }
    assert_eq!(plane.folder("real"), repos.join("real"));
    // And the listing says the same of every name, which is the rule being one rule.
    for (name, below, _) in plane.repos() {
        assert_eq!(plane.folder(&name), plane.workspace().join(below), "{name}");
    }
}

// ---------------------------------------------------------------------------------------
// One rule for what a repo's name is, and one anchor                                      #
// ---------------------------------------------------------------------------------------

#[test]
fn a_name_the_workspace_keeps_is_no_repos_name_in_any_question() {
    purlis_core::unsteered!();
    let plane = Plane::new();
    std::fs::create_dir_all(plane.workspace().join("memory")).unwrap();
    plane.repo_at("repos/todos");

    for name in [
        "memory",
        "Memory",
        "workspace.md",
        "todos",
        ".worktrees",
        "a b",
    ] {
        assert!(
            matches!(
                repofolder::folder(&plane.root, "alpha", name),
                Err(Refused::NotARepoName(_))
            ),
            "folder {name:?}"
        );
        assert!(
            matches!(
                repofolder::new_folder(&plane.root, "alpha", name),
                Err(Refused::NotARepoName(_))
            ),
            "new_folder {name:?}"
        );
    }
    // In `repos/` it is still no repo, so the listing and the lookup cannot disagree about it.
    assert_eq!(plane.repos(), []);
    assert_eq!(
        facts(&plane, &plane.holdings().others),
        [fact("todos", "repos/todos", Layout::Repos, Git::Directory)]
    );
}

#[test]
fn no_new_repo_takes_the_name_of_the_folder_it_goes_in() {
    purlis_core::unsteered!();
    let plane = Plane::new();

    for name in ["repos", "Repos"] {
        assert!(
            matches!(
                repofolder::new_folder(&plane.root, "alpha", name),
                Err(Refused::NotARepoName(_))
            ),
            "{name}"
        );
    }
}

#[test]
fn a_workspace_reached_through_a_link_anchors_no_question() {
    purlis_core::unsteered!();
    let plane = Plane::new();
    let (_keep, outside) = elsewhere();
    symlink(&outside, plane.root.join("workspaces/ghost")).unwrap();

    assert!(matches!(
        repofolder::folder(&plane.root, "ghost", "api"),
        Err(Refused::Trouble(Trouble::Outside(_)))
    ));
    assert!(matches!(
        repofolder::holdings(&plane.root, "ghost"),
        Err(Trouble::Outside(_))
    ));
    assert!(matches!(
        repofolder::new_folder(&plane.root, "ghost", "api"),
        Err(Refused::Trouble(Trouble::Outside(_)))
    ));
}

// ---------------------------------------------------------------------------------------
// The facts each scanner needs: said, not dropped                                         #
// ---------------------------------------------------------------------------------------

#[test]
fn a_row_says_what_its_git_is_in_either_place() {
    purlis_core::unsteered!();
    let plane = Plane::new();
    let (_keep, outside) = elsewhere();
    plane.repo_at("clone");
    plane.linked_at("linked");
    plane.git_link_at("gitlink", &outside.join(".git"));
    plane.repo_at("repos/clone2");
    plane.linked_at("repos/linked2");
    plane.git_link_at("repos/gitlink2", &outside.join(".git"));

    let found = plane.holdings();

    assert_eq!(
        facts(&plane, &found.repos),
        [
            fact("clone", "clone", Layout::Root, Git::Directory),
            fact("clone2", "repos/clone2", Layout::Repos, Git::Directory),
        ]
    );
    assert_eq!(
        facts(&plane, &found.others),
        [
            fact("gitlink", "gitlink", Layout::Root, Git::Link),
            fact("gitlink2", "repos/gitlink2", Layout::Repos, Git::Link),
            fact("linked", "linked", Layout::Root, Git::File),
            fact("linked2", "repos/linked2", Layout::Repos, Git::File),
        ]
    );
}

#[test]
fn a_repo_in_a_name_the_workspace_keeps_is_a_row_and_not_a_repo() {
    purlis_core::unsteered!();
    let plane = Plane::new();
    plane.repo_at("memory");

    let found = plane.holdings();

    assert_eq!(found.repos, []);
    assert_eq!(
        facts(&plane, &found.others),
        [fact("memory", "memory", Layout::Root, Git::Directory)]
    );
}

#[test]
fn a_name_that_is_no_repos_is_said_in_either_place() {
    purlis_core::unsteered!();
    let plane = Plane::new();
    std::fs::create_dir_all(plane.workspace().join("a b")).unwrap();
    std::fs::create_dir_all(plane.workspace().join("repos/c d")).unwrap();

    let refused = plane.holdings().refused;

    let said: Vec<_> = refused
        .iter()
        .map(|one| (one.name.as_str(), one.layout))
        .collect();
    assert_eq!(said, [("a b", Layout::Root), ("c d", Layout::Repos)]);
    assert!(refused[0].why.contains("not a name"), "{refused:?}");
}

#[test]
fn what_could_not_be_looked_at_is_said_and_the_rest_still_answers() {
    purlis_core::unsteered!();
    let plane = Plane::new();
    plane.repo_at("old");
    plane.repo_at("repos/new");
    plane.repo_at("repos/locked");
    let _shut = Shut::folder(plane.workspace().join("repos/locked"));

    let found = plane.holdings();

    let names: Vec<_> = found
        .repos
        .iter()
        .map(|row| row.repo.name.as_str())
        .collect();
    assert_eq!(names, ["new", "old"]);
    let said: Vec<_> = found
        .unreadable
        .iter()
        .map(|one| (one.name.as_str(), one.layout))
        .collect();
    assert_eq!(said, [("locked", Layout::Repos)]);
}

#[test]
fn a_repos_folder_that_cannot_be_read_is_said_and_the_root_still_answers() {
    purlis_core::unsteered!();
    let plane = Plane::new();
    plane.repo_at("old");
    plane.repo_at("repos/new");
    let _shut = Shut::folder(plane.workspace().join("repos"));

    let found = plane.holdings();

    let names: Vec<_> = found
        .repos
        .iter()
        .map(|row| row.repo.name.as_str())
        .collect();
    assert_eq!(names, ["old"]);
    let said: Vec<_> = found
        .unreadable
        .iter()
        .map(|one| (one.name.as_str(), one.layout))
        .collect();
    assert_eq!(said, [("repos", Layout::Root)]);
}

// ---------------------------------------------------------------------------------------
// The scanner already there sits on this one                                              #
// ---------------------------------------------------------------------------------------

#[test]
fn the_repo_list_reads_the_same_walk_and_still_answers_only_the_root_until_it_migrates() {
    purlis_core::unsteered!();
    let plane = Plane::new();
    let (_keep, outside) = elsewhere();
    plane.repo_at("twice");
    plane.repo_at("repos/twice");
    plane.repo_at("repos/new");
    plane.git_link_at("repos/gitlink", &outside.join(".git"));
    std::fs::create_dir_all(plane.workspace().join("repos/c d")).unwrap();
    // As `repos::clones` has always answered: it never held a repository in a kept name back.
    plane.repo_at("memory");
    plane.git_link_at("gitlink", &outside.join(".git"));
    std::fs::create_dir_all(plane.workspace().join("a b")).unwrap();

    let clones = repos::clones(&plane.root, "alpha").expect("the workspace reads");

    let found: Vec<_> = clones.repos.iter().map(|repo| repo.path.clone()).collect();
    assert_eq!(
        found,
        [
            plane.workspace().join("memory"),
            plane.workspace().join("twice")
        ]
    );
    let refused: Vec<_> = clones
        .refused
        .iter()
        .map(|(name, _)| name.as_str())
        .collect();
    assert_eq!(refused, ["a b", "gitlink"]);
    assert!(
        clones.refused[1].1.contains("symlink"),
        "{:?}",
        clones.refused
    );
}

// ---------------------------------------------------------------------------------------
// Duplicates                                                                              #
// ---------------------------------------------------------------------------------------

#[test]
fn a_repo_at_the_root_beside_one_in_repos_is_reported_as_shadowed() {
    purlis_core::unsteered!();
    let plane = Plane::new();
    plane.repo_at("api");
    plane.repo_at("repos/api");
    plane.linked_at("web");
    plane.repo_at("repos/web");
    plane.repo_at("alone");

    let found = plane.holdings();

    assert_eq!(
        facts(&plane, &found.repos),
        [
            fact("alone", "alone", Layout::Root, Git::Directory),
            fact("api", "repos/api", Layout::Repos, Git::Directory),
            fact("web", "repos/web", Layout::Repos, Git::Directory),
        ]
    );
    assert_eq!(
        facts(&plane, &found.shadowed),
        [
            fact("api", "api", Layout::Root, Git::Directory),
            fact("web", "web", Layout::Root, Git::File),
        ]
    );
}

#[test]
fn a_folder_of_the_persons_own_beside_a_repo_of_that_name_is_no_duplicate() {
    purlis_core::unsteered!();
    let plane = Plane::new();
    std::fs::create_dir_all(plane.workspace().join("docs")).unwrap();
    plane.repo_at("repos/docs");

    let found = plane.holdings();

    assert_eq!(found.shadowed, []);
    assert_eq!(
        facts(&plane, &found.repos),
        [fact("docs", "repos/docs", Layout::Repos, Git::Directory)]
    );
}

// ---------------------------------------------------------------------------------------
// A workspace with no folder                                                              #
// ---------------------------------------------------------------------------------------

#[test]
fn a_workspace_with_no_folder_holds_nothing_and_a_repos_folder_is_where_it_would_be() {
    purlis_core::unsteered!();
    let plane = Plane::new();

    assert_eq!(
        repofolder::holdings(&plane.root, "nothing").expect("nothing to read"),
        Holdings::default()
    );
    assert_eq!(
        repofolder::folder(&plane.root, "nothing", "api").expect("a place"),
        plane.root.join("workspaces/nothing/api")
    );
    // A new clone needs a workspace to go in, so that is still the caller's to hear.
    assert!(matches!(
        repofolder::new_folder(&plane.root, "nothing", "api"),
        Err(Refused::Trouble(Trouble::Outside(
            Outside::NoWorkspace { .. }
        )))
    ));
}

#[test]
fn a_workspace_that_is_a_file_is_refused_and_not_read_as_empty() {
    purlis_core::unsteered!();
    let plane = Plane::new();
    std::fs::write(plane.root.join("workspaces/flat"), "a file\n").unwrap();

    assert!(repofolder::holdings(&plane.root, "flat").is_err());
    assert!(repofolder::folder(&plane.root, "flat", "api").is_err());
}

// ---------------------------------------------------------------------------------------
// From a path back to the repo                                                            #
// ---------------------------------------------------------------------------------------

#[test]
fn a_path_inside_a_repo_names_its_workspace_repo_and_layout_in_either_place() {
    purlis_core::unsteered!();
    let plane = Plane::new();
    let old = plane.repo_at("old");
    let new = plane.repo_at("repos/new");
    std::fs::create_dir_all(old.join("src/deep")).unwrap();
    std::fs::create_dir_all(new.join("src/deep")).unwrap();

    for (path, name, layout, folder) in [
        (old.clone(), "old", Layout::Root, &old),
        (old.join("src/deep"), "old", Layout::Root, &old),
        (new.clone(), "new", Layout::Repos, &new),
        (new.join("src/deep"), "new", Layout::Repos, &new),
    ] {
        let found = repofolder::locate(&plane.root, &path).expect("a repo holds it");
        assert_eq!(found.plane, plane.root);
        assert_eq!(found.ws, "alpha");
        assert_eq!(found.repo.name, name);
        assert_eq!(&found.repo.path, folder);
        assert_eq!(found.layout, layout);
    }
}

#[test]
fn a_path_in_no_repo_of_the_workspace_locates_nothing() {
    purlis_core::unsteered!();
    let plane = Plane::new();
    plane.repo_at("old");
    plane.repo_at("memory");
    plane.repo_at("repos/new");
    plane.repo_at(".worktrees/old/piece");
    let notes = plane.workspace().join("notes");
    std::fs::create_dir_all(&notes).unwrap();
    let plain = plane.workspace().join("repos/plain");
    std::fs::create_dir_all(&plain).unwrap();
    let (_keep, outside) = elsewhere();
    symlink(&outside, plane.workspace().join("out")).unwrap();

    for path in [
        plane.root.clone(),
        plane.workspace(),
        plane.workspace().join("repos"),
        notes,
        plain,
        plane.workspace().join("memory"),
        plane.workspace().join(".worktrees/old/piece"),
        plane.workspace().join("out"),
    ] {
        assert_eq!(repofolder::locate(&plane.root, &path), None, "{path:?}");
    }
}

#[test]
fn a_path_in_an_old_repo_named_repos_is_in_that_repo_whatever_is_nested_in_it() {
    purlis_core::unsteered!();
    let plane = Plane::new();
    plane.repo_at("repos");
    let nested = plane.repo_at("repos/vendored");

    let found = repofolder::locate(&plane.root, &nested).expect("the old repo holds it");

    assert_eq!(found.repo.name, "repos");
    assert_eq!(found.repo.path, plane.workspace().join("repos"));
    assert_eq!(found.layout, Layout::Root);
}

#[test]
fn a_path_in_a_shadowed_folder_is_answered_where_it_stands() {
    purlis_core::unsteered!();
    let plane = Plane::new();
    let root = plane.repo_at("api");
    plane.repo_at("repos/api");

    let found = repofolder::locate(&plane.root, &root).expect("a repository holds it");

    assert_eq!(found.repo.path, root);
    assert_eq!(found.layout, Layout::Root);
    // Which is how a caller tells it from the repo of that name.
    assert_ne!(found.repo.path, plane.folder("api"));
}

#[test]
fn a_path_inside_a_repo_finds_its_plane_with_no_plane_given() {
    purlis_core::unsteered!();
    let plane = Plane::new();
    let new = plane.repo_at("repos/new");
    // A plane inside a repo's own tree, as a fixture is: it holds no repo, so the repo holds it.
    let fixture = new.join("fixtures/plane");
    std::fs::create_dir_all(fixture.join("workspaces/inner/notes")).unwrap();
    std::fs::write(fixture.join("charter.toml"), "schema = 1\n").unwrap();

    for path in [new.clone(), fixture.join("workspaces/inner/notes")] {
        let found = repofolder::locate_from(&path).expect("a repo holds it");
        assert_eq!(found.plane, plane.root, "{path:?}");
        assert_eq!(found.ws, "alpha");
        assert_eq!(found.repo.name, "new");
        assert_eq!(found.layout, Layout::Repos);
    }
    assert_eq!(repofolder::locate_from(&plane.root), None);
}

// ---------------------------------------------------------------------------------------
// `Repos`, however it is cased                                                            #
// ---------------------------------------------------------------------------------------

#[test]
fn an_old_repo_named_repos_in_capitals_is_read_as_a_repo() {
    purlis_core::unsteered!();
    let plane = Plane::new();
    plane.repo_at("Repos");

    assert_eq!(plane.folder("Repos"), plane.workspace().join("Repos"));
    assert_eq!(plane.repos(), [row("Repos", "Repos", Layout::Root)]);
}

// ---------------------------------------------------------------------------------------
// Nothing here follows a link                                                             #
// ---------------------------------------------------------------------------------------

#[test]
fn a_repos_that_is_a_link_is_said_and_nothing_is_read_or_placed_through_it() {
    purlis_core::unsteered!();
    let plane = Plane::new();
    let (_keep, outside) = elsewhere();
    let beyond = outside.join("beyond");
    std::fs::create_dir_all(beyond.join("api/.git")).unwrap();
    symlink(&beyond, plane.workspace().join("repos")).unwrap();

    assert_eq!(plane.folder("api"), plane.workspace().join("api"));
    let found = plane.holdings();
    assert_eq!(found.repos, []);
    let said: Vec<_> = found.refused.iter().map(|one| one.name.as_str()).collect();
    assert_eq!(said, ["repos"]);
    assert!(matches!(
        repofolder::new_folder(&plane.root, "alpha", "api"),
        Err(Refused::ReposNotAFolder { .. })
    ));
}

#[test]
fn an_entry_that_is_a_link_is_said_in_either_place_and_is_no_repo() {
    purlis_core::unsteered!();
    let plane = Plane::new();
    let (_keep, outside) = elsewhere();
    symlink(&outside, plane.workspace().join("rootlink")).unwrap();
    std::fs::create_dir_all(plane.workspace().join("repos")).unwrap();
    symlink(&outside, plane.workspace().join("repos/inlink")).unwrap();

    assert_eq!(plane.folder("inlink"), plane.workspace().join("inlink"));
    let found = plane.holdings();
    assert_eq!(found.repos, []);
    let said: Vec<_> = found
        .refused
        .iter()
        .map(|one| (one.name.as_str(), one.layout))
        .collect();
    assert_eq!(
        said,
        [("inlink", Layout::Repos), ("rootlink", Layout::Root)]
    );
    assert!(
        found.refused[0].why.contains("symlink"),
        "{:?}",
        found.refused
    );
}

#[test]
fn a_git_that_is_a_link_is_no_repo_and_loses_to_a_real_one() {
    purlis_core::unsteered!();
    let plane = Plane::new();
    let (_keep, outside) = elsewhere();
    plane.git_link_at("repos/api", &outside.join(".git"));
    let root = plane.repo_at("api");

    assert_eq!(plane.folder("api"), root);
    assert_eq!(plane.repos(), [row("api", "api", Layout::Root)]);
    assert_eq!(
        repofolder::locate(&plane.root, &plane.workspace().join("repos/api")),
        None
    );
}

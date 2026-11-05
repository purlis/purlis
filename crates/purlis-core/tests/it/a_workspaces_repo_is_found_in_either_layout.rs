// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Where a workspace's repo is, in the layout before structure version 6 (`<ws>/<repo>`), the
//! one after it (`<ws>/repos/<repo>`) and a workspace holding both (ADR 0114), against real
//! planes on disk and real git.
//!
//! Asked only of [`purlis_core::repofolder`], the one place that knows both layouts: what a
//! repo's folder is, which repos a workspace holds, and where a new one goes.

use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};

use purlis_core::repofolder::{self, Git, Holdings, Layout, Refused, Row, Trouble};
use purlis_core::repos;
use purlis_core::worktree;
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
fn a_name_the_workspace_keeps_is_no_new_repos_and_no_repos_in_repos() {
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
                repofolder::new_folder(&plane.root, "alpha", name),
                Err(Refused::NotARepoName(_))
            ),
            "new_folder {name:?}"
        );
    }
    for name in [".worktrees", "a b"] {
        assert!(
            matches!(
                repofolder::folder(&plane.root, "alpha", name),
                Err(Refused::NotARepoName(_))
            ),
            "folder {name:?}"
        );
    }
    // In `repos/` it is no repo, so the listing and the lookups cannot disagree about it.
    assert_eq!(plane.repos(), []);
    assert_eq!(
        facts(&plane, &plane.holdings().others),
        [fact("todos", "repos/todos", Layout::Repos, Git::Directory)]
    );
    assert_eq!(plane.folder("todos"), plane.workspace().join("todos"));
    assert_eq!(
        repofolder::locate(&plane.root, &plane.workspace().join("repos/todos")),
        None
    );
}

#[test]
fn an_old_clone_in_a_name_the_workspace_keeps_goes_on_working() {
    purlis_core::unsteered!();
    // ADR 0114, decision 3: a clone at the root keeps working with nothing done, whatever it is
    // named, and `repos::clones` and `repos::clone_at` have always taken these.
    let plane = Plane::new();
    for name in ["work", "themes", "memory"] {
        let clone = plane.repo_at(name);
        std::fs::create_dir_all(clone.join("src")).unwrap();

        assert_eq!(plane.folder(name), clone, "folder {name}");
        assert_eq!(
            repos::clone_at(&plane.root, "alpha", name).map(|repo| repo.path),
            Some(clone.clone()),
            "clone_at {name}"
        );
        for path in [clone.clone(), clone.join("src")] {
            let found = repofolder::locate(&plane.root, &path).expect("the old clone holds it");
            assert_eq!(found.repo.name, name);
            assert_eq!(found.repo.path, clone);
            assert_eq!(found.layout, Layout::Root);
            assert_eq!(found.git, Git::Directory);
        }
    }
    // Still not one of the repos the workspace lists, as the walk has it.
    assert_eq!(plane.repos(), []);
}

#[test]
fn a_repo_in_repos_never_wins_for_a_name_the_workspace_keeps() {
    purlis_core::unsteered!();
    let plane = Plane::new();
    let old = plane.repo_at("memory");
    plane.repo_at("repos/memory");

    assert_eq!(plane.folder("memory"), old);
    assert_eq!(plane.holdings().shadowed, []);
}

#[test]
fn a_kept_name_with_no_clone_there_answers_the_hand_joined_path_and_implies_no_repo() {
    purlis_core::unsteered!();
    let plane = Plane::new();
    let memory = plane.workspace().join("memory");
    std::fs::create_dir_all(&memory).unwrap();

    assert_eq!(plane.folder("memory"), memory);
    assert_eq!(plane.folder("sessions"), plane.workspace().join("sessions"));
    // The question "is there a repo" is the walk's, and a kept folder with no `.git` is no row.
    assert_eq!(plane.holdings(), Holdings::default());
    assert_eq!(repofolder::locate(&plane.root, &memory), None);
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
    std::fs::create_dir_all(plane.workspace().join("memory")).unwrap();
    plane.repo_at("repos/new");
    plane.repo_at("repos/todos");
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
        plane.workspace().join("repos/todos"),
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
    // A plane inside a repo's own tree, as a fixture is. Where it holds no repo, the outer repo
    // holds the path. Where it holds one, that is the nearest, and the farthest would be wrong.
    let fixture = new.join("fixtures/plane");
    std::fs::create_dir_all(fixture.join("workspaces/inner/notes")).unwrap();
    std::fs::write(fixture.join("charter.toml"), "schema = 1\n").unwrap();
    let held = fixture.join("workspaces/inner/held");
    std::fs::create_dir_all(&held).unwrap();
    support::git(&held, &["init", "-q", "-b", "main", "."]);
    // A folder that is only called `workspaces`, with no manifest above it, is no plane, whatever
    // is under it.
    let unplaned = new.join("vendor/workspaces/inner/thing");
    std::fs::create_dir_all(&unplaned).unwrap();
    support::git(&unplaned, &["init", "-q", "-b", "main", "."]);

    for path in [
        new.clone(),
        fixture.join("workspaces/inner/notes"),
        unplaned.clone(),
    ] {
        let found = repofolder::locate_from(&path).expect("a repo holds it");
        assert_eq!(found.plane, plane.root, "{path:?}");
        assert_eq!(found.ws, "alpha");
        assert_eq!(found.repo.name, "new");
        assert_eq!(found.layout, Layout::Repos);
    }

    let nearest = repofolder::locate_from(&held).expect("the fixture's repo holds it");
    assert_eq!(nearest.plane, fixture);
    assert_eq!(nearest.ws, "inner");
    assert_eq!(nearest.repo.name, "held");
    assert_eq!(nearest.layout, Layout::Root);

    assert_eq!(repofolder::locate_from(&plane.root), None);
}

#[test]
fn a_branch_folder_is_located_as_a_piece_and_not_as_a_repo() {
    purlis_core::unsteered!();
    // `locate_from` answers `None` here, and `worktree::locate` is what a caller asks instead:
    // it names the workspace, the repo and the piece, and `folder` then names the clone.
    let plane = Plane::new();
    let clone = plane.repo_at("repos/api");
    let piece = plane.workspace().join(".worktrees/api/piece");
    std::fs::create_dir_all(piece.join("src")).unwrap();
    std::fs::write(piece.join(".git"), "gitdir: /nowhere\n").unwrap();

    assert_eq!(repofolder::locate_from(&piece), None);
    assert_eq!(repofolder::locate(&plane.root, &piece.join("src")), None);

    let found = worktree::locate(&plane.root, &piece.join("src")).expect("a piece holds it");
    assert_eq!(found.workspace, "alpha");
    assert_eq!(found.piece, "piece");
    assert_eq!(plane.folder(&found.repo), clone);
}

// ---------------------------------------------------------------------------------------
// A path that is not there, and what stands where `.git` would be                         #
// ---------------------------------------------------------------------------------------

#[test]
fn a_path_not_there_yet_inside_a_repo_is_in_that_repo() {
    purlis_core::unsteered!();
    let plane = Plane::new();
    let old = plane.repo_at("old");
    let new = plane.repo_at("repos/new");

    for (path, name) in [
        (old.join("nofile"), "old"),
        (old.join("not/there/yet"), "old"),
        (new.join("nofile"), "new"),
    ] {
        let found = repofolder::locate(&plane.root, &path).expect("a repo holds it");
        assert_eq!(found.repo.name, name, "{path:?}");
    }
}

#[test]
fn a_path_that_walks_up_and_is_not_there_is_in_no_repo() {
    purlis_core::unsteered!();
    let plane = Plane::new();
    let api = plane.repo_at("api");
    plane.repo_at("repos/new");

    for path in [
        api.join("../../../../etc/nofile"),
        plane.workspace().join("repos/new/../../api/nofile"),
    ] {
        assert_eq!(repofolder::locate(&plane.root, &path), None, "{path:?}");
    }
}

#[test]
fn a_path_through_a_link_that_goes_nowhere_or_out_is_in_no_repo() {
    purlis_core::unsteered!();
    let plane = Plane::new();
    let api = plane.repo_at("api");
    let (_keep, outside) = elsewhere();
    symlink(api.join("nothing-here"), api.join("dang")).unwrap();
    symlink(&outside, api.join("out")).unwrap();

    for path in [
        api.join("dang"),
        api.join("dang/deeper"),
        api.join("out/nofile"),
    ] {
        assert_eq!(repofolder::locate(&plane.root, &path), None, "{path:?}");
    }
}

#[test]
fn a_path_is_located_in_a_folder_with_a_git_of_any_kind_and_says_which() {
    purlis_core::unsteered!();
    // `chatpiece::clone_at` takes any `.git`, so #5370 keeps that by not asking `git`, and a caller
    // who wants a repository asks for `Git::Directory`.
    let plane = Plane::new();
    let (_keep, outside) = elsewhere();
    plane.repo_at("clone");
    plane.linked_at("linked");
    plane.git_link_at("gitlink", &outside.join(".git"));
    plane.repo_at("repos/clone2");
    plane.linked_at("repos/linked2");
    plane.git_link_at("repos/gitlink2", &outside.join(".git"));
    std::fs::create_dir_all(plane.workspace().join("empty")).unwrap();

    for (below, name, layout, git) in [
        ("clone", "clone", Layout::Root, Git::Directory),
        ("linked", "linked", Layout::Root, Git::File),
        ("gitlink", "gitlink", Layout::Root, Git::Link),
        ("repos/clone2", "clone2", Layout::Repos, Git::Directory),
        ("repos/linked2", "linked2", Layout::Repos, Git::File),
        ("repos/gitlink2", "gitlink2", Layout::Repos, Git::Link),
    ] {
        let path = plane.workspace().join(below);
        let found = repofolder::locate(&plane.root, &path).expect("a .git is there");
        assert_eq!(found.repo.name, name, "{below}");
        assert_eq!(found.repo.path, path, "{below}");
        assert_eq!(found.layout, layout, "{below}");
        assert_eq!(found.git, git, "{below}");
    }
    assert_eq!(
        repofolder::locate(&plane.root, &plane.workspace().join("empty")),
        None
    );
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

#[test]
fn a_plain_repos_folder_in_capitals_on_a_disk_that_folds_case_is_the_folder_in_every_question() {
    purlis_core::unsteered!();
    let plane = Plane::new();
    let upper = plane.workspace().join("Repos");
    std::fs::create_dir_all(upper.join("new/src")).unwrap();
    if !plane.workspace().join("repos").exists() {
        return; // A disk that tells the two apart: `Repos/` is a folder of the person's own.
    }
    support::git(&upper.join("new"), &["init", "-q", "-b", "main", "."]);

    let folder = plane.workspace().join("repos/new");
    assert_eq!(plane.folder("new"), folder);
    assert_eq!(plane.repos(), [row("new", "repos/new", Layout::Repos)]);
    for path in [
        upper.join("new"),
        upper.join("new/src"),
        upper.join("new/nofile"),
        folder.join("src"),
    ] {
        let found = repofolder::locate(&plane.root, &path).expect("the repo holds it");
        assert_eq!(found.repo.name, "new", "{path:?}");
        assert_eq!(found.repo.path, folder, "{path:?}");
        assert_eq!(found.layout, Layout::Repos, "{path:?}");
    }
    assert_eq!(repofolder::locate(&plane.root, &upper), None);
}

#[test]
fn a_plain_folder_in_capitals_beside_repos_on_a_disk_that_keeps_case_is_the_persons_own() {
    purlis_core::unsteered!();
    let plane = Plane::new();
    let upper = plane.workspace().join("Repos");
    std::fs::create_dir_all(&upper).unwrap();
    if plane.workspace().join("repos").exists() {
        return; // A disk that folds case: there is one folder, asked of above.
    }
    plane.repo_at("repos/new");
    plane.repo_at("Repos/mine");

    assert_eq!(plane.repos(), [row("new", "repos/new", Layout::Repos)]);
    assert_eq!(repofolder::locate(&plane.root, &upper.join("mine")), None);
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
    // The same link in `repos/`, with something at the root: the link beats nothing.
    let beside = plane.repo_at("beside");
    symlink(&outside, plane.workspace().join("repos/beside")).unwrap();

    assert_eq!(plane.folder("inlink"), plane.workspace().join("inlink"));
    assert_eq!(plane.folder("beside"), beside);
    // `folder` is a hand-join with one precedence question: it gates only what it prefers, so a
    // link at the root comes back as the path it is, for the caller to gate as before.
    assert_eq!(plane.folder("rootlink"), plane.workspace().join("rootlink"));
    let found = plane.holdings();
    assert_eq!(plane.repos(), [row("beside", "beside", Layout::Root)]);
    let said: Vec<_> = found
        .refused
        .iter()
        .map(|one| (one.name.as_str(), one.layout))
        .collect();
    assert_eq!(
        said,
        [
            ("beside", Layout::Repos),
            ("inlink", Layout::Repos),
            ("rootlink", Layout::Root)
        ]
    );
    assert!(
        found.refused[0].why.contains("symlink"),
        "{:?}",
        found.refused
    );
}

#[test]
fn a_folder_whose_git_is_a_link_is_answered_by_folder_as_it_stands_and_refused_as_a_repo() {
    purlis_core::unsteered!();
    let plane = Plane::new();
    let (_keep, outside) = elsewhere();
    plane.git_link_at("gitlink", &outside.join(".git"));

    assert_eq!(plane.folder("gitlink"), plane.workspace().join("gitlink"));
    assert_eq!(plane.repos(), []);
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
    let found = repofolder::locate(&plane.root, &plane.workspace().join("repos/api"))
        .expect("a .git is there, and it says what kind");
    assert_eq!(found.git, Git::Link);
}

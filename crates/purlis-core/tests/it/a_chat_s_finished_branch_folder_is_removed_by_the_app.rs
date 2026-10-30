// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! #3879: a chat's finished branch folder is removed by the app, never forced, only where
//! nothing would be lost, and only one of the chat's own workspace, from above it.
//!
//! A real clone in a workspace, branch folders cut from it by the terminal's own command, and
//! the app's broker (`gitbroker::answer`) asked as a chat in the workspace asks it.

use crate::support;

use std::path::{Path, PathBuf};

use purlis_core::gitbroker::{self, Asker};
use purlis_core::hookwire::{Answer, GitAsk, GitWork};
use purlis_core::pieces::Who;
use purlis_core::repocmd::Say;
use purlis_core::worktree::git;

use support::git;

const WS: &str = "alpha";
const REPO: &str = "widget";

struct World {
    _dir: tempfile::TempDir,
    plane: PathBuf,
}

impl World {
    /// A world whose clone is of a forge (a bare repository by `file://` address), and a
    /// second checkout of the forge, someone else's, that merges on it.
    fn with_a_forge() -> (World, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let base = std::fs::canonicalize(dir.path()).unwrap();
        let plane = base.join("plane");
        std::fs::create_dir_all(plane.join("workspaces").join(WS)).unwrap();
        std::fs::write(plane.join("charter.toml"), "schema = 1\n").unwrap();
        let forge = base.join("forge.git");
        git(
            &base,
            &[
                "init",
                "-q",
                "--bare",
                "-b",
                "main",
                forge.to_str().unwrap(),
            ],
        );
        let theirs = base.join("theirs");
        let url = format!("file://{}", forge.display());
        git(&base, &["clone", "-q", &url, theirs.to_str().unwrap()]);
        std::fs::write(theirs.join("README.md"), "one\n").unwrap();
        git(&theirs, &["add", "README.md"]);
        git(&theirs, &["commit", "-q", "-m", "one"]);
        git(&theirs, &["push", "-q", "origin", "HEAD:main"]);
        let clone = plane.join("workspaces").join(WS).join(REPO);
        git(&base, &["clone", "-q", &url, clone.to_str().unwrap()]);
        (World { _dir: dir, plane }, theirs)
    }

    fn new() -> World {
        let dir = tempfile::tempdir().unwrap();
        let base = std::fs::canonicalize(dir.path()).unwrap();
        let plane = base.join("plane");
        std::fs::create_dir_all(plane.join("workspaces").join(WS)).unwrap();
        std::fs::write(plane.join("charter.toml"), "schema = 1\n").unwrap();
        let clone = plane.join("workspaces").join(WS).join(REPO);
        std::fs::create_dir_all(&clone).unwrap();
        git(&clone, &["init", "-q", "-b", "main"]);
        std::fs::write(clone.join("README.md"), "one\n").unwrap();
        git(&clone, &["add", "README.md"]);
        git(&clone, &["commit", "-q", "-m", "one"]);
        World { _dir: dir, plane }
    }

    fn workspace(&self) -> PathBuf {
        self.plane.join("workspaces").join(WS)
    }

    fn piece(&self, name: &str) -> PathBuf {
        self.workspace().join(".worktrees").join(REPO).join(name)
    }

    /// A branch folder cut from the clone's HEAD, as the terminal cuts one.
    fn cut(&self, name: &str) -> PathBuf {
        let code = purlis_core::piececmd::add(
            &self.plane,
            WS,
            REPO,
            name,
            None,
            Some(purlis_core::piececmd::HEAD),
            None,
            None,
            &Who::here(None, None, None),
            chrono::Utc::now(),
            &mut |_| {},
        );
        assert_eq!(code, 0, "{name} was not cut");
        self.piece(name)
    }

    /// What the app answers chat 4, standing at `cwd`, asking for `piece`'s removal, and
    /// what it said, as one text.
    fn removed(&self, cwd: &Path, piece: &str, delete_branch: bool) -> (Answer, String) {
        let answer = gitbroker::answer(
            &self.plane,
            &Asker {
                chat: 4,
                cwd: Some(cwd.to_path_buf()),
                persona: None,
                harnessed: true,
                unsandboxed: false,
                config: None,
                id: None,
                live: None,
            },
            &GitAsk {
                chat: 4,
                workspace: WS.to_owned(),
                work: GitWork::WorktreeRemove {
                    repo: REPO.to_owned(),
                    piece: piece.to_owned(),
                    delete_branch,
                },
            },
            &git::Isolated::identity(Some("Tester"), Some("t@e.invalid")),
            chrono::Utc::now(),
        );
        let said = match &answer {
            Answer::Said { lines, .. } => lines
                .iter()
                .map(|line| match line {
                    Say::Info(t) | Say::Done(t) | Say::Warn(t) | Say::Fail(t) | Say::Plain(t) => {
                        t.to_string()
                    }
                    Say::Out(t) => t.clone(),
                })
                .collect::<Vec<_>>()
                .join("\n"),
            Answer::No { why } => why.clone(),
            other => format!("{other:?}"),
        };
        (answer, said)
    }
}

#[test]
fn a_clean_folder_holding_no_commit_of_its_own_is_removed_with_its_merged_branch() {
    purlis_core::unsteered!();
    let world = World::new();
    let done = world.cut("done");
    let (answer, said) = world.removed(&world.workspace(), "done", true);
    assert!(matches!(answer, Answer::Said { code: 0, .. }), "{said}");
    assert!(said.contains("Removed widget · done"), "{said}");
    assert!(said.contains("Deleted branch done"), "{said}");
    assert!(!done.exists());
    // And from the project's top.
    let other = world.cut("other");
    let (answer, said) = world.removed(&world.plane, "other", false);
    assert!(matches!(answer, Answer::Said { code: 0, .. }), "{said}");
    assert!(!other.exists());
}

#[test]
fn a_folder_with_a_change_or_a_commit_of_its_own_is_kept_and_what_would_be_lost_is_named() {
    purlis_core::unsteered!();
    let world = World::new();
    let dirty = world.cut("dirty");
    std::fs::write(dirty.join("README.md"), "changed\n").unwrap();
    let (answer, said) = world.removed(&world.workspace(), "dirty", false);
    assert!(matches!(answer, Answer::Said { code: 1, .. }), "{said}");
    assert!(said.contains("README.md"), "{said}");
    assert!(dirty.join("README.md").is_file());

    let ahead = world.cut("ahead");
    std::fs::write(ahead.join("new.md"), "new\n").unwrap();
    git(&ahead, &["add", "new.md"]);
    git(&ahead, &["commit", "-q", "-m", "only here"]);
    let (answer, said) = world.removed(&world.workspace(), "ahead", false);
    assert!(matches!(answer, Answer::Said { code: 1, .. }), "{said}");
    assert!(ahead.join("new.md").is_file(), "{said}");
}

#[test]
fn a_folder_is_never_removed_for_a_chat_standing_in_it_nor_the_clone_itself() {
    purlis_core::unsteered!();
    let world = World::new();
    let piece = world.cut("here");
    let (answer, said) = world.removed(&piece, "here", false);
    assert!(matches!(answer, Answer::No { .. }), "{said}");
    assert!(piece.is_dir());
    // No name reaches above `.worktrees/<repo>`.
    for name in ["..", ".", "../widget", ""] {
        let (answer, said) = world.removed(&world.workspace(), name, false);
        assert!(
            !matches!(answer, Answer::Said { code: 0, .. }),
            "{name:?}: {said}"
        );
    }
    assert!(world.workspace().join(REPO).join(".git").is_dir());
}

#[test]
fn a_folder_whose_branch_landed_by_a_squash_merge_is_removed_and_its_branch_kept() {
    purlis_core::unsteered!();
    // #3879, on review: the forge squash-merges a lane and deletes its branch, and the clone's
    // fetch prunes it. Its commits are on no other ref, yet nothing in the folder is lost.
    let (world, theirs) = World::with_a_forge();
    let clone = world.workspace().join(REPO);
    let code = purlis_core::piececmd::add(
        &world.plane,
        WS,
        REPO,
        "lane",
        None,
        Some("main"),
        None,
        None,
        &Who::here(None, None, None),
        chrono::Utc::now(),
        &mut |_| {},
    );
    assert_eq!(code, 0, "the lane was not cut");
    let lane = world.piece("lane");
    std::fs::write(lane.join("feature.md"), "a feature\n").unwrap();
    git(&lane, &["add", "feature.md"]);
    git(&lane, &["commit", "-q", "-m", "lane: one"]);
    std::fs::write(lane.join("feature.md"), "a feature, reviewed\n").unwrap();
    git(&lane, &["commit", "-q", "-am", "lane: two"]);
    git(&clone, &["push", "-q", "origin", "lane:lane"]);
    // Squash-merged on the forge, and the branch deleted there.
    git(&theirs, &["fetch", "-q", "origin", "lane"]);
    git(&theirs, &["merge", "-q", "--squash", "FETCH_HEAD"]);
    git(&theirs, &["commit", "-q", "-m", "A lane (#1)"]);
    git(&theirs, &["push", "-q", "origin", "HEAD:main"]);
    git(&theirs, &["push", "-q", "origin", ":lane"]);
    // The clone fetches and prunes; its own main is left where it was.
    git(&clone, &["fetch", "-q", "--prune", "origin"]);

    let (answer, said) = world.removed(&world.workspace(), "lane", true);
    assert!(matches!(answer, Answer::Said { code: 0, .. }), "{said}");
    assert!(said.contains("landed by its files"), "{said}");
    assert!(said.contains("Its branch was kept"), "{said}");
    assert!(!lane.exists(), "{said}");
    // The branch stays: the one place its commits are.
    git(&clone, &["rev-parse", "--verify", "-q", "refs/heads/lane"]);
}

#[test]
fn a_squashed_branch_whose_folder_changed_since_is_kept() {
    purlis_core::unsteered!();
    let (world, theirs) = World::with_a_forge();
    let clone = world.workspace().join(REPO);
    let code = purlis_core::piececmd::add(
        &world.plane,
        WS,
        REPO,
        "lane",
        None,
        Some("main"),
        None,
        None,
        &Who::here(None, None, None),
        chrono::Utc::now(),
        &mut |_| {},
    );
    assert_eq!(code, 0);
    let lane = world.piece("lane");
    std::fs::write(lane.join("feature.md"), "a feature\n").unwrap();
    git(&lane, &["add", "feature.md"]);
    git(&lane, &["commit", "-q", "-m", "lane: one"]);
    git(&clone, &["push", "-q", "origin", "lane:lane"]);
    git(&theirs, &["fetch", "-q", "origin", "lane"]);
    git(&theirs, &["merge", "-q", "--squash", "FETCH_HEAD"]);
    git(&theirs, &["commit", "-q", "-m", "A lane (#1)"]);
    git(&theirs, &["push", "-q", "origin", "HEAD:main"]);
    git(&clone, &["fetch", "-q", "--prune", "origin"]);
    // A commit after the merge, which the forge does not have.
    std::fs::write(lane.join("feature.md"), "more\n").unwrap();
    git(&lane, &["commit", "-q", "-am", "lane: after"]);
    let (answer, said) = world.removed(&world.workspace(), "lane", false);
    assert!(matches!(answer, Answer::Said { code: 1, .. }), "{said}");
    assert!(lane.join("feature.md").is_file());
}

#[test]
fn a_clean_folder_holding_a_file_git_ignores_is_kept_for_a_chat_and_the_file_named() {
    purlis_core::unsteered!();
    // #3879, on review: git's own guard does not count ignored files, and deletes them.
    let world = World::new();
    let clone = world.workspace().join(REPO);
    std::fs::write(clone.join(".gitignore"), ".env\nresults/\n").unwrap();
    git(&clone, &["add", ".gitignore"]);
    git(&clone, &["commit", "-q", "-m", "ignore"]);
    let lane = world.cut("lane");
    std::fs::write(lane.join(".env"), "TOKEN=only-here\n").unwrap();
    let (answer, said) = world.removed(&world.workspace(), "lane", false);
    assert!(matches!(answer, Answer::Said { code: 1, .. }), "{said}");
    assert!(said.contains("`.env`"), "{said}");
    assert!(said.contains("files git ignores"), "{said}");
    assert!(lane.join(".env").is_file());
    // Once moved out, the folder goes.
    std::fs::remove_file(lane.join(".env")).unwrap();
    let (answer, said) = world.removed(&world.workspace(), "lane", false);
    assert!(matches!(answer, Answer::Said { code: 0, .. }), "{said}");
    assert!(!lane.exists());
}

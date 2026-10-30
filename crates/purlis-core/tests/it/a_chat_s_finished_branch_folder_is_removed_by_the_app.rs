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

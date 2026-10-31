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
                alive: false,
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

/// A lane cut from `main` with two commits to `feature.md`, squash-merged on the forge, and its
/// branch deleted there; the clone has not fetched yet. Answers the lane's folder and the clone.
fn squash_merged(world: &World, theirs: &Path) -> (PathBuf, PathBuf) {
    let clone = world.workspace().join(REPO);
    let lane = cut_from(world, "main");
    std::fs::write(lane.join("feature.md"), "a feature\nof lines\n").unwrap();
    git(&lane, &["add", "feature.md"]);
    git(&lane, &["commit", "-q", "-m", "lane: one"]);
    std::fs::write(lane.join("feature.md"), "a feature, reviewed\nof lines\n").unwrap();
    git(&lane, &["commit", "-q", "-am", "lane: two"]);
    land_by_squash(&clone, theirs);
    (lane, clone)
}

/// The folder `lane`, cut from `base`.
fn cut_from(world: &World, base: &str) -> PathBuf {
    let code = purlis_core::piececmd::add(
        &world.plane,
        WS,
        REPO,
        "lane",
        None,
        Some(base),
        None,
        None,
        &Who::here(None, None, None),
        chrono::Utc::now(),
        &mut |_| {},
    );
    assert_eq!(code, 0, "the lane was not cut");
    world.piece("lane")
}

/// `lane` pushed, squash-merged into the forge's main, and its branch deleted there.
fn land_by_squash(clone: &Path, theirs: &Path) {
    git(clone, &["push", "-q", "origin", "lane:lane"]);
    git(theirs, &["fetch", "-q", "origin", "lane"]);
    git(theirs, &["merge", "-q", "--squash", "FETCH_HEAD"]);
    git(theirs, &["commit", "-q", "-m", "A lane (#1)"]);
    git(theirs, &["push", "-q", "origin", "HEAD:main"]);
    git(theirs, &["push", "-q", "origin", ":lane"]);
}

/// Someone changes `feature.md` again on the forge, and the clone fetches it.
fn main_moves_on(theirs: &Path, clone: &Path) {
    std::fs::write(theirs.join("feature.md"), "rewritten by someone else\n").unwrap();
    git(
        theirs,
        &["commit", "-q", "-am", "Someone else's change (#2)"],
    );
    git(theirs, &["push", "-q", "origin", "HEAD:main"]);
    git(clone, &["fetch", "-q", "--prune", "origin"]);
}

#[test]
fn a_squashed_branch_goes_though_main_changed_the_same_file_since() {
    purlis_core::unsteered!();
    // #4237: "carried" asks whether main still reads as the branch, which a later change to the
    // same file ends. The squash commit is still in main's history.
    let (world, theirs) = World::with_a_forge();
    let (lane, clone) = squash_merged(&world, &theirs);
    main_moves_on(&theirs, &clone);

    let (answer, said) = world.removed(&world.workspace(), "lane", false);

    assert!(matches!(answer, Answer::Said { code: 0, .. }), "{said}");
    assert!(!lane.exists(), "{said}");
    git(&clone, &["rev-parse", "--verify", "-q", "refs/heads/lane"]);
}

#[test]
fn a_squashed_branch_goes_though_the_branch_it_was_cut_from_is_gone() {
    purlis_core::unsteered!();
    // A stacked lane: its recorded base was deleted once the stack landed on the default branch.
    let (world, theirs) = World::with_a_forge();
    let clone = world.workspace().join(REPO);
    git(&clone, &["branch", "stack-base", "main"]);
    let lane = cut_from(&world, "stack-base");
    std::fs::write(lane.join("feature.md"), "a feature\n").unwrap();
    git(&lane, &["add", "feature.md"]);
    git(&lane, &["commit", "-q", "-m", "lane: one"]);
    land_by_squash(&clone, &theirs);
    main_moves_on(&theirs, &clone);
    git(&clone, &["branch", "-q", "-D", "stack-base"]);

    let (answer, said) = world.removed(&world.workspace(), "lane", false);

    assert!(matches!(answer, Answer::Said { code: 0, .. }), "{said}");
    assert!(!lane.exists(), "{said}");
}

#[test]
fn a_commit_made_after_the_squash_is_kept_though_main_changed_the_file_since() {
    purlis_core::unsteered!();
    let (world, theirs) = World::with_a_forge();
    let (lane, clone) = squash_merged(&world, &theirs);
    main_moves_on(&theirs, &clone);
    std::fs::write(lane.join("later.md"), "later\n").unwrap();
    git(&lane, &["add", "later.md"]);
    git(&lane, &["commit", "-q", "-m", "lane: after"]);

    let (answer, said) = world.removed(&world.workspace(), "lane", false);

    assert!(matches!(answer, Answer::Said { code: 1, .. }), "{said}");
    assert!(said.contains("nowhere else"), "{said}");
    assert!(lane.join("later.md").is_file());
}

#[test]
fn a_commit_after_the_squash_that_changes_only_whitespace_or_a_mode_is_kept() {
    purlis_core::unsteered!();
    // `git patch-id --stable` would read the first as no change at all.
    let (world, theirs) = World::with_a_forge();
    let (lane, clone) = squash_merged(&world, &theirs);
    main_moves_on(&theirs, &clone);
    std::fs::write(lane.join("feature.md"), "a  feature, reviewed\nof lines\n").unwrap();
    git(&lane, &["commit", "-q", "-am", "lane: spacing"]);
    let (answer, said) = world.removed(&world.workspace(), "lane", false);
    assert!(matches!(answer, Answer::Said { code: 1, .. }), "{said}");

    std::fs::write(lane.join("feature.md"), "a feature, reviewed\nof lines\n").unwrap();
    git(&lane, &["commit", "-q", "-am", "lane: spacing back"]);
    git(&lane, &["update-index", "--chmod=+x", "feature.md"]);
    git(&lane, &["commit", "-q", "-m", "lane: executable"]);
    let (answer, said) = world.removed(&world.workspace(), "lane", false);
    assert!(matches!(answer, Answer::Said { code: 1, .. }), "{said}");
    assert!(lane.is_dir());
}

#[test]
fn a_binary_file_is_never_read_as_landed_by_its_patch() {
    purlis_core::unsteered!();
    // A text diff says only that a binary file differs, so two different files read the same.
    let (world, theirs) = World::with_a_forge();
    let clone = world.workspace().join(REPO);
    let lane = cut_from(&world, "main");
    std::fs::write(lane.join("blob.bin"), [0u8, 1, 2, 3]).unwrap();
    git(&lane, &["add", "blob.bin"]);
    git(&lane, &["commit", "-q", "-m", "lane: my blob"]);
    std::fs::write(theirs.join("blob.bin"), [0u8, 9, 9, 9]).unwrap();
    git(&theirs, &["add", "blob.bin"]);
    git(&theirs, &["commit", "-q", "-m", "their blob"]);
    git(&theirs, &["push", "-q", "origin", "HEAD:main"]);
    git(&clone, &["fetch", "-q", "--prune", "origin"]);

    let (answer, said) = world.removed(&world.workspace(), "lane", false);

    assert!(matches!(answer, Answer::Said { code: 1, .. }), "{said}");
    assert!(lane.is_dir());
}

#[test]
fn a_closed_request_whose_commits_landed_nowhere_is_kept() {
    purlis_core::unsteered!();
    // Pushed, then the branch deleted on the forge without a merge: nothing in main is it.
    let (world, theirs) = World::with_a_forge();
    let clone = world.workspace().join(REPO);
    let lane = cut_from(&world, "main");
    std::fs::write(lane.join("feature.md"), "a feature\n").unwrap();
    git(&lane, &["add", "feature.md"]);
    git(&lane, &["commit", "-q", "-m", "lane: one"]);
    git(&clone, &["push", "-q", "origin", "lane:lane"]);
    git(&theirs, &["push", "-q", "origin", ":lane"]);
    git(&clone, &["fetch", "-q", "--prune", "origin"]);

    let (answer, said) = world.removed(&world.workspace(), "lane", false);

    assert!(matches!(answer, Answer::Said { code: 1, .. }), "{said}");
    assert!(lane.join("feature.md").is_file());
}

#[test]
fn a_branch_name_reused_after_its_merge_is_kept_for_its_new_commits() {
    purlis_core::unsteered!();
    let (world, theirs) = World::with_a_forge();
    let (lane, clone) = squash_merged(&world, &theirs);
    git(&clone, &["fetch", "-q", "--prune", "origin"]);
    // The same name, now at main with new work on it.
    git(&lane, &["reset", "-q", "--hard", "origin/main"]);
    std::fs::write(lane.join("second.md"), "second round\n").unwrap();
    git(&lane, &["add", "second.md"]);
    git(&lane, &["commit", "-q", "-m", "lane: second round"]);

    let (answer, said) = world.removed(&world.workspace(), "lane", false);

    assert!(matches!(answer, Answer::Said { code: 1, .. }), "{said}");
    assert!(lane.join("second.md").is_file());
}

#[test]
fn uncommitted_work_in_a_squashed_branch_still_refuses_it() {
    purlis_core::unsteered!();
    let (world, theirs) = World::with_a_forge();
    let (lane, clone) = squash_merged(&world, &theirs);
    main_moves_on(&theirs, &clone);
    std::fs::write(lane.join("wip.md"), "unsaved\n").unwrap();

    let (answer, said) = world.removed(&world.workspace(), "lane", false);

    assert!(matches!(answer, Answer::Said { code: 1, .. }), "{said}");
    assert!(said.contains("wip.md"), "{said}");
    assert!(lane.join("wip.md").is_file());
}

/// A folder of a repo committing `tracked` and ignoring `rules`, holding `files`.
fn holding(rules: &str, tracked: &[&str], files: &[&str]) -> (World, PathBuf) {
    let world = World::new();
    let clone = world.workspace().join(REPO);
    std::fs::write(clone.join(".gitignore"), rules).unwrap();
    git(&clone, &["add", ".gitignore"]);
    for file in tracked {
        let at = clone.join(file);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(at, "x\n").unwrap();
        git(&clone, &["add", file]);
    }
    git(&clone, &["commit", "-q", "-m", "ignore"]);
    let lane = world.cut("lane");
    for file in files {
        let at = lane.join(file);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(at, "x\n").unwrap();
    }
    (world, lane)
}

#[test]
fn what_only_the_harness_or_a_build_regenerates_does_not_keep_a_folder() {
    purlis_core::unsteered!();
    // #4237, #4245: every folder that was built or worked in holds these, and a sandboxed chat
    // cannot delete `.claude/.cc-writes/` at all.
    let (world, lane) = holding(
        "node_modules/\ntarget/\ndist/\n__pycache__/\n.cc-writes/\n\
         **/gen/schemas/\n**/permissions/autogenerated/\n",
        &["Cargo.toml", "app/package.json"],
        &[
            ".claude/.cc-writes/a",
            "app/.claude/.cc-writes/b",
            "app/src-tauri/gen/schemas/c.json",
            "app/src-tauri/permissions/autogenerated/d.toml",
            "app/node_modules/e/index.js",
            "tools/__pycache__/f.pyc",
            "target/debug/g",
            "app/dist/h.js",
        ],
    );

    let (answer, said) = world.removed(&world.workspace(), "lane", false);

    assert!(matches!(answer, Answer::Said { code: 0, .. }), "{said}");
    assert!(!lane.exists(), "{said}");
}

#[test]
fn an_ignored_file_a_person_might_want_still_keeps_a_folder_beside_regenerated_ones() {
    purlis_core::unsteered!();
    let (world, lane) = holding(
        ".env\nnode_modules/\n",
        &[],
        &[".env", "node_modules/a/index.js"],
    );
    let (answer, said) = world.removed(&world.workspace(), "lane", false);
    assert!(matches!(answer, Answer::Said { code: 1, .. }), "{said}");
    assert!(said.contains("`.env`"), "{said}");
    assert!(
        !said.contains("node_modules"),
        "named only what matters: {said}"
    );
    assert!(lane.join(".env").is_file());
}

#[test]
fn a_folder_merely_named_like_a_regenerated_one_still_keeps_a_folder() {
    purlis_core::unsteered!();
    // No build manifest beside it, not under `.claude`, not the generated path, or not a folder.
    for (rules, file, named) in [
        ("target/\n", "target/results.csv", "`target/`"),
        ("dist/\n", "dist/release.tgz", "`dist/`"),
        (
            ".cc-writes/\n",
            "notes/.cc-writes/mine",
            "`notes/.cc-writes/`",
        ),
        ("schemas/\n", "gen/schemas/mine.json", "`gen/schemas/`"),
        ("node_modules\n", "lib/node_modules", "`lib/node_modules`"),
    ] {
        let (world, lane) = holding(rules, &[], &[file]);
        let (answer, said) = world.removed(&world.workspace(), "lane", false);
        assert!(
            matches!(answer, Answer::Said { code: 1, .. }),
            "{file}: {said}"
        );
        assert!(said.contains(named), "{file}: {said}");
        assert!(lane.join(file).is_file(), "{file}");
    }
}

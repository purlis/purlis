// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A clone in the workspace's `repos/` folder works as a clone at its root does (ADR 0114,
//! #5370): its branch folders are cut, a chat started in it is a writing chat, the app commits
//! for a chat in its branch folder, Changes reads it, and the plane is found from inside it.
//!
//! Every test runs against both layouts, against real planes on disk and real git, so the old
//! layout's answers are the ones the new layout is held to.

use crate::support;

use std::path::Path;

use purlis_core::chatpiece::{self, Naming};
use purlis_core::files::{self, Branch, Mark};
use purlis_core::gitbroker::{self, Asker, commit};
use purlis_core::hookwire::{Answer, CommitAsk, Stage};
use purlis_core::repocmd::Say;
use purlis_core::repofolder::{self, Git, Layout};
use purlis_core::worktree::{self, git};

/// The two places a clone can be: at the workspace's root, and in its `repos/` folder.
fn both() -> [(Layout, support::Fixture); 2] {
    [
        (Layout::Root, support::plane_with_clone("api")),
        (Layout::Repos, support::plane_with_clone_in_repos("api")),
    ]
}

/// The bounded reader, as the app starts it, but this test binary run again, picking
/// [`reader_child`].
fn reader() -> files::Reader {
    files::Reader::new(
        std::env::current_exe().expect("the test binary"),
        [
            "a_clone_in_repos_works_as_one_at_the_root::reader_child",
            "--exact",
            "--nocapture",
            "--test-threads=1",
            files::READ_ARG,
        ]
        .map(std::ffi::OsString::from),
    )
}

/// The reader's child: answers the one question it was asked and exits; in any other run it
/// does nothing.
#[test]
fn reader_child() {
    purlis_core::unsteered!();
    if let Some(code) = files::serve_if_asked() {
        std::process::exit(code);
    }
}

#[test]
fn a_branch_folder_is_cut_from_a_clone_in_either_place() {
    purlis_core::unsteered!();
    for (layout, f) in both() {
        let added = worktree::add(&f.plane, &f.ws, &f.repo, "piece", None)
            .unwrap_or_else(|why| panic!("{layout:?}: no branch folder was cut: {why}"));

        // Branch folders stay where they are, whichever place the clone is in.
        assert_eq!(
            added.path,
            worktree::path_for(&f.plane, &f.ws, &f.repo, "piece").unwrap(),
            "{layout:?}"
        );
        assert!(added.path.join("README.md").is_file(), "{layout:?}");
        let listed = worktree::list(&f.plane, &f.ws, &f.repo)
            .unwrap_or_else(|why| panic!("{layout:?}: the clone's branches: {why}"));
        assert!(
            listed.iter().any(|piece| piece.piece == "piece"),
            "{layout:?}: {listed:?}"
        );
    }
}

#[test]
fn a_chat_started_in_a_clone_in_either_place_gets_a_branch_of_its_own() {
    purlis_core::unsteered!();
    for (layout, f) in both() {
        assert_eq!(
            chatpiece::clone_at(&f.plane, &f.clone),
            Some((f.ws.clone(), f.repo.clone())),
            "{layout:?}"
        );

        let cut = chatpiece::cut(&f.plane, &f.ws, &f.repo, &Naming::After(None))
            .unwrap_or_else(|why| panic!("{layout:?}: no branch was cut: {why}"));
        assert_eq!(cut.branch, "chat-1", "{layout:?}");
        assert!(cut.path.join("README.md").is_file(), "{layout:?}");
    }
}

#[test]
fn a_folder_in_a_clone_or_the_repos_folder_itself_is_no_clone_a_chat_starts_in() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone_in_repos("api");
    std::fs::create_dir_all(f.clone.join("src")).unwrap();

    for not_a_clone in [f.workspace().join("repos"), f.clone.join("src")] {
        assert_eq!(
            chatpiece::clone_at(&f.plane, &not_a_clone),
            None,
            "{} is not a repo's clone",
            not_a_clone.display()
        );
    }
}

#[test]
fn a_clone_at_the_root_that_one_in_repos_shadows_is_no_clone_a_chat_starts_in() {
    purlis_core::unsteered!();
    // `repos/api` is the workspace's `api`, so a chat started in an old `api` beside it would
    // cut its branch from the other one.
    let f = support::plane_with_clone_in_repos("api");
    let shadowed = f.workspace().join("api");
    std::fs::create_dir_all(&shadowed).unwrap();
    support::git(&shadowed, &["init", "-q", "-b", "main", "."]);

    assert_eq!(chatpiece::clone_at(&f.plane, &shadowed), None);
    assert_eq!(
        chatpiece::clone_at(&f.plane, &f.clone),
        Some((f.ws.clone(), f.repo.clone()))
    );
}

/// What the app answers chat 4, standing in `cwd`, asking for `stage` to be committed.
fn commit_asked(f: &support::Fixture, cwd: &Path, stage: Stage) -> Answer {
    let asker = Asker {
        chat: 4,
        cwd: Some(cwd.to_path_buf()),
        persona: Some("steward".to_owned()),
        harnessed: true,
        unsandboxed: false,
        roots_git_held: false,
        unreadable: Vec::new(),
        config: None,
        id: None,
        live: None,
    };
    let ask = CommitAsk {
        chat: 4,
        message: "piece: one change\n\nWhy it was made.".to_owned(),
        stage,
        folder: None,
    };
    let identity = git::Isolated::identity(Some("The Person"), Some("person@example.invalid"));
    commit::answer(&f.plane, &asker, &ask, &identity)
}

#[test]
fn the_app_commits_for_a_chat_in_a_branch_folder_of_a_clone_in_either_place() {
    purlis_core::unsteered!();
    for (layout, f) in both() {
        std::fs::write(
            f.plane.join("charter.toml"),
            "schema = 1\n[sandbox]\nmode = \"on\"\n",
        )
        .unwrap();
        let piece = worktree::add(&f.plane, &f.ws, &f.repo, "piece", None)
            .unwrap_or_else(|why| panic!("{layout:?}: no branch folder was cut: {why}"))
            .path;
        std::fs::write(piece.join("change.txt"), "a change\n").unwrap();

        let answer = commit_asked(&f, &piece, Stage::Paths(vec!["change.txt".to_owned()]));

        let Answer::Said { lines, code: 0 } = &answer else {
            panic!("{layout:?}: not committed: {answer:?}");
        };
        assert!(
            matches!(lines.first(), Some(Say::Done(first)) if first.to_string().starts_with("committed ")),
            "{layout:?}: {lines:?}"
        );
        let log = support::git(&piece, &["log", "-1", "--format=%s"]);
        assert_eq!(
            String::from_utf8_lossy(&log.stdout).trim_end(),
            "piece: one change",
            "{layout:?}"
        );
    }
}

#[test]
fn the_broker_runs_git_in_a_clone_in_either_place_and_its_branch_folder() {
    purlis_core::unsteered!();
    for (layout, f) in both() {
        worktree::add(&f.plane, &f.ws, &f.repo, "piece", None)
            .unwrap_or_else(|why| panic!("{layout:?}: no branch folder was cut: {why}"));
        let isolation = git::Isolated::operators();

        let ran = gitbroker::in_a_checked_clone(&f.plane, &f.ws, &f.repo, &isolation, || 1);
        assert_eq!(ran, Ok(1), "{layout:?}");

        let ran =
            gitbroker::in_a_checked_folder(&f.plane, &f.ws, &f.repo, "piece", &isolation, || 2);
        assert_eq!(ran, Ok(2), "{layout:?}");
    }
}

#[test]
fn changes_reads_a_clone_in_either_place_and_its_branch_folder() {
    purlis_core::unsteered!();
    for (layout, f) in both() {
        let piece = worktree::add(&f.plane, &f.ws, &f.repo, "piece", None)
            .unwrap_or_else(|why| panic!("{layout:?}: no branch folder was cut: {why}"))
            .path;
        std::fs::write(f.clone.join("README.md"), "one, changed\n").unwrap();
        std::fs::write(piece.join("new.txt"), "new\n").unwrap();

        let in_the_clone = files::status(&reader(), &f.plane, Branch::repo(&f.ws, &f.repo))
            .unwrap_or_else(|why| panic!("{layout:?}: the clone's changes: {why}"));
        let in_the_piece =
            files::status(&reader(), &f.plane, Branch::piece(&f.ws, &f.repo, "piece"))
                .unwrap_or_else(|why| panic!("{layout:?}: the branch's changes: {why}"));

        let marks = |status: &files::Status| -> Vec<(String, Mark)> {
            status
                .changes
                .iter()
                .map(|change| (change.path.clone(), change.mark))
                .collect()
        };
        assert_eq!(
            marks(&in_the_clone),
            [("README.md".to_owned(), Mark::Changed)],
            "{layout:?}"
        );
        assert_eq!(
            marks(&in_the_piece),
            [("new.txt".to_owned(), Mark::Added)],
            "{layout:?}"
        );
    }
}

#[test]
fn the_plane_is_found_from_inside_a_clone_in_either_place() {
    purlis_core::unsteered!();
    for (layout, f) in both() {
        let deep = f.clone.join("src/inner");
        std::fs::create_dir_all(&deep).unwrap();

        assert_eq!(
            purlis_core::plane::find_root(&deep).ok(),
            Some(f.plane.clone()),
            "{layout:?}"
        );
        let found = repofolder::locate_from(&deep)
            .unwrap_or_else(|| panic!("{layout:?}: the clone is not found from inside it"));
        assert_eq!(
            (found.plane, found.ws, found.repo.name, found.repo.path),
            (
                f.plane.clone(),
                f.ws.clone(),
                f.repo.clone(),
                f.clone.clone()
            ),
            "{layout:?}"
        );
        assert_eq!((found.layout, found.git), (layout, Git::Directory));
    }
}

#[test]
fn a_clone_is_found_through_a_plane_path_that_is_not_resolved() {
    purlis_core::unsteered!();
    // As macOS's `/var` is a link to `/private/var`: the plane named through a link is still
    // the plane its clones are in.
    for (layout, f) in both() {
        let elsewhere = tempfile::tempdir().unwrap();
        let link = elsewhere.path().join("plane");
        std::os::unix::fs::symlink(&f.plane, &link).unwrap();

        assert_eq!(
            chatpiece::clone_at(&link, &f.clone),
            Some((f.ws.clone(), f.repo.clone())),
            "{layout:?}"
        );
    }
}

#[test]
fn a_dispatch_from_a_root_folder_that_one_in_repos_shadows_is_cut_from_the_workspace() {
    purlis_core::unsteered!();
    // Standing in the old `api` is standing in the workspace's folder, not in a clone: the
    // dispatch is cut from the workspace's `api`, which is `repos/api`, as `purlis worktree
    // add` would cut it, never from where the chat stands.
    let f = support::plane_with_clone_in_repos("api");
    let shadowed = f.workspace().join("api");
    std::fs::create_dir_all(&shadowed).unwrap();
    support::git(&shadowed, &["init", "-q", "-b", "main", "."]);

    let from_clone = purlis_core::dispatchplace::repo_of(&f.plane, Some(&f.clone), None)
        .expect("a chat in repos/api works in api");
    assert!(!from_clone.from_default, "{from_clone:?}");

    match purlis_core::dispatchplace::repo_of(&f.plane, Some(&shadowed), None) {
        Ok(repo) => {
            assert_eq!(repo.repo, "api");
            assert!(
                repo.from_default,
                "cut as from the workspace's folder: {repo:?}"
            );
        }
        Err(refused) => {
            panic!("a chat in the workspace's folder works in its one repo: {refused:?}")
        }
    }
}

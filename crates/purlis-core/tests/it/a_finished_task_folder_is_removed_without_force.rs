// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! ADR 0110 (#5168): a finished task folder is removed by `purlis worktree remove` without
//! `--force`, from a terminal or a chat. One that is not finished is still sent to its task's
//! Changes tab, and the refusal says why.
//!
//! A real clone, folders cut for tasks as a dispatch cuts them, their records in the store,
//! and the terminal's removal (`piececmd::remove`) and the app's broker asked as a chat asks.

use crate::support;

use purlis_core::chatpiece::Cut;
use purlis_core::dispatchplace;
use purlis_core::gitbroker::{self, Asker};
use purlis_core::hookwire::{Answer, GitAsk, GitWork};
use purlis_core::piececmd;
use purlis_core::worktree::git::Isolated;

use support::folders::{excluding, has_branch, spoken, task_folder};
use support::git;

/// The terminal's own removal of `cut`, and what it said.
fn from_a_terminal(f: &support::Fixture, cut: &Cut, force: bool, delete: bool) -> (u8, String) {
    let mut said = Vec::new();
    let code = piececmd::remove(
        &f.plane,
        &f.ws,
        &f.repo,
        &cut.piece,
        force,
        delete,
        &mut |line| said.push(spoken(&line)),
    );
    (code, said.join("\n"))
}

/// What the app answers a chat standing in the workspace that asks for `cut`'s removal.
fn from_a_chat(f: &support::Fixture, cut: &Cut, delete: bool) -> (Answer, String) {
    let answer = gitbroker::answer(
        &f.plane,
        &Asker {
            chat: 4,
            cwd: Some(f.plane.join("workspaces").join(&f.ws)),
            persona: None,
            harnessed: true,
            unsandboxed: false,
            config: None,
            id: None,
            live: None,
        },
        &GitAsk {
            chat: 4,
            workspace: f.ws.clone(),
            alive: false,
            work: GitWork::WorktreeRemove {
                repo: f.repo.clone(),
                piece: cut.piece.clone(),
                delete_branch: delete,
            },
        },
        &Isolated::identity(Some("Tester"), Some("t@e.invalid")),
        chrono::Utc::now(),
    );
    let said = match &answer {
        Answer::Said { lines, .. } => lines.iter().map(spoken).collect::<Vec<_>>().join("\n"),
        Answer::No { why } => why.clone(),
        other => format!("{other:?}"),
    };
    (answer, said)
}

#[test]
fn a_finished_task_folder_is_removed_from_a_terminal_without_force_and_with_its_branch() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("api");
    let cut = task_folder(&f, "check the queue", true);

    let (code, said) = from_a_terminal(&f, &cut, false, true);

    assert_eq!(code, 0, "{said}");
    assert!(!cut.path.exists(), "{said}");
    assert!(
        said.contains(&format!("Deleted branch {}", cut.branch)),
        "{said}"
    );
    assert!(!has_branch(&f.clone, &cut.branch));
}

#[test]
fn a_finished_task_folder_is_removed_for_a_chat() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("api");
    let cut = task_folder(&f, "check the queue", true);
    // Its task committed, and the commit is on its base.
    std::fs::write(cut.path.join("fix.md"), "fixed\n").unwrap();
    git(&cut.path, &["add", "fix.md"]);
    git(&cut.path, &["commit", "-q", "-m", "fix"]);
    git(&f.clone, &["merge", "-q", "--ff-only", &cut.branch]);

    let (answer, said) = from_a_chat(&f, &cut, false);

    assert!(matches!(answer, Answer::Said { code: 0, .. }), "{said}");
    assert!(!cut.path.exists(), "{said}");
}

#[test]
fn a_task_folder_whose_task_has_not_ended_is_sent_to_its_changes_tab() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("api");
    let cut = task_folder(&f, "check the queue", false);

    let (code, said) = from_a_terminal(&f, &cut, false, false);

    assert_eq!(code, 1, "{said}");
    assert!(said.contains("its task has not ended"), "{said}");
    assert!(said.contains("Changes tab"), "{said}");
    assert!(cut.path.is_dir());
}

#[test]
fn a_task_folder_that_would_lose_something_is_sent_to_its_changes_tab_saying_what() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("api");
    let dirty = task_folder(&f, "dirty", true);
    std::fs::write(dirty.path.join("README.md"), "changed\n").unwrap();
    let ahead = task_folder(&f, "ahead", true);
    std::fs::write(ahead.path.join("new.md"), "new\n").unwrap();
    git(&ahead.path, &["add", "new.md"]);
    git(&ahead.path, &["commit", "-q", "-m", "only here"]);
    let ignored = task_folder(&f, "ignored", true);
    excluding(&f, ".env");
    std::fs::write(ignored.path.join(".env"), "TOKEN=only-here\n").unwrap();

    // A chat's removal names an ignored file in its own words first (#3879); each names it.
    for (cut, what, named) in [
        (&dirty, "uncommitted changes (`README.md`)", "README.md"),
        (&ahead, "commits its base does not have", "commits"),
        (
            &ignored,
            "files git ignores that are not a build's (`.env`)",
            "`.env`",
        ),
    ] {
        let (code, said) = from_a_terminal(&f, cut, false, false);
        assert_eq!(code, 1, "{said}");
        assert!(said.contains("it is not finished"), "{said}");
        assert!(said.contains(what), "{said}");
        assert!(said.contains("Changes tab"), "{said}");
        assert!(cut.path.is_dir());
        let (answer, said) = from_a_chat(&f, cut, false);
        assert!(matches!(answer, Answer::Said { code: 1, .. }), "{said}");
        assert!(said.contains(named), "{said}");
        assert!(cut.path.is_dir());
    }
}

#[test]
fn a_task_folder_another_folder_links_into_is_kept_until_that_folder_goes() {
    purlis_core::unsteered!();
    // A lane shares an install by linking its `node_modules` into the task's folder.
    let f = support::plane_with_clone("api");
    excluding(&f, "node_modules");
    let shared = task_folder(&f, "shares its install", true);
    std::fs::create_dir_all(shared.path.join("node_modules/left-pad")).unwrap();
    std::fs::write(shared.path.join("package.json"), "{}\n").unwrap();
    git(&shared.path, &["add", "package.json"]);
    git(&shared.path, &["commit", "-q", "-m", "package"]);
    git(&f.clone, &["merge", "-q", "--ff-only", &shared.branch]);
    let linking = task_folder(&f, "links to it", true);
    std::os::unix::fs::symlink(
        shared.path.join("node_modules"),
        linking.path.join("node_modules"),
    )
    .unwrap();

    let (code, said) = from_a_terminal(&f, &shared, false, false);
    assert_eq!(code, 1, "{said}");
    assert!(
        said.contains(&format!("api · {} links into it", linking.piece)),
        "{said}"
    );
    assert!(shared.path.join("node_modules/left-pad").is_dir());

    let (code, said) = from_a_terminal(&f, &linking, false, false);
    assert_eq!(code, 0, "{said}");
    let (code, said) = from_a_terminal(&f, &shared, false, false);
    assert_eq!(code, 0, "{said}");
    assert!(!shared.path.exists());
}

#[test]
fn force_is_still_refused_for_a_task_folder_finished_or_not() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("api");
    let cut = task_folder(&f, "check the queue", true);

    let (code, said) = from_a_terminal(&f, &cut, true, false);

    assert_eq!(code, 1, "{said}");
    assert!(said.contains("Changes tab"), "{said}");
    assert!(cut.path.is_dir());
}

#[test]
fn the_explorer_s_remove_reads_a_finished_task_folder_as_the_command_does() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("api");
    let cut = task_folder(&f, "check the queue", true);
    let kept = |merging: bool, force: bool| {
        dispatchplace::kept_from_the_explorer(
            &f.plane,
            &f.ws,
            &f.repo,
            &cut.piece,
            merging,
            force,
            || false,
        )
    };

    assert_eq!(kept(false, false), None, "a finished task folder goes");
    assert!(kept(false, true).is_some(), "never forced");
    assert!(
        kept(true, false).is_some(),
        "never merged from the explorer"
    );
    std::fs::write(cut.path.join("README.md"), "changed\n").unwrap();
    let said = kept(false, false).expect("not finished");
    assert!(said.contains("uncommitted changes"), "{said}");
}

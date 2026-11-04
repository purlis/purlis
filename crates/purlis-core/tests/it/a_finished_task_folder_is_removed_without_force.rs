// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! ADR 0110 (#5168): a finished task folder is removed by `purlis worktree remove` without
//! `--force`, from a terminal or a chat. One that is not finished is still sent to its task's
//! Changes tab, and the refusal says why.
//!
//! A real clone, folders cut for tasks as a dispatch cuts them, their records in the store,
//! and the terminal's removal (`piececmd::remove`) and the app's broker asked as a chat asks.

use crate::support;

use std::path::Path;

use purlis_core::chatpiece::Cut;
use purlis_core::dispatchplace::{self, Repo};
use purlis_core::dispatchrecord;
use purlis_core::gitbroker::{self, Asker};
use purlis_core::hookwire::{Answer, GitAsk, GitWork};
use purlis_core::piececmd;
use purlis_core::repocmd::Say;
use purlis_core::worktree::git::Isolated;

use support::git;

/// A folder cut for the task `task`, its record stored, ended or still running.
fn task_folder(f: &support::Fixture, task: &str, ended: bool) -> Cut {
    let id = dispatchrecord::mint();
    let repo = Repo {
        workspace: f.ws.clone(),
        repo: f.repo.clone(),
        piece: None,
        from_default: false,
    };
    let cut = dispatchplace::cut(&f.plane, &repo, task, &id, &Isolated::default()).expect("cut");
    use dispatchrecord::{Asker, ChatRef, Mode, Opening, Place, Worker, Worktree};
    let chat = |number: u32, name: &str| ChatRef {
        chat: number,
        id: None,
        name: name.to_owned(),
        persona: None,
    };
    let now = chrono::Utc::now();
    dispatchrecord::open_as(
        &f.plane,
        id.clone(),
        Opening {
            mode: Mode::Task,
            asker: Asker {
                chat: chat(1, "steward 1"),
                workspace: Some(cut.workspace.clone()),
                by_person: false,
                session_record: None,
            },
            persona: None,
            worker: Worker {
                chat: chat(7, task),
                harness: None,
                profile: None,
                session_record: None,
            },
            task: Some(task.to_owned()),
            place: Place {
                workspace: Some(cut.workspace.clone()),
                folder: None,
                worktree: Some(Worktree {
                    repo: cut.repo.clone(),
                    piece: cut.piece.clone(),
                    branch: Some(cut.branch.clone()),
                    removed: None,
                }),
            },
            brief: "b".to_owned(),
            report_owed: true,
        },
        now,
    )
    .expect("a record");
    if ended {
        dispatchrecord::close(&f.plane, &id, dispatchrecord::Ending::default(), now).unwrap();
    }
    cut
}

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

fn spoken(line: &Say) -> String {
    match line {
        Say::Info(t) | Say::Done(t) | Say::Warn(t) | Say::Fail(t) | Say::Plain(t) => t.to_string(),
        Say::Out(t) => t.clone(),
    }
}

/// `rule` added to what the clone's git ignores, in every folder of it.
fn excluding(f: &support::Fixture, rule: &str) {
    use std::io::Write;
    std::fs::create_dir_all(f.clone.join(".git/info")).unwrap();
    let mut exclude = std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(f.clone.join(".git/info/exclude"))
        .unwrap();
    writeln!(exclude, "{rule}").unwrap();
}

fn has_branch(clone: &Path, branch: &str) -> bool {
    let mut ask = support::unsigned();
    ask.arg("-C").arg(clone).args([
        "show-ref",
        "--verify",
        "--quiet",
        &format!("refs/heads/{branch}"),
    ]);
    purlis_core::forklock::output(&mut ask)
        .expect("git runs")
        .status
        .success()
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

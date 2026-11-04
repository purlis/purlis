// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Branch folders for the tests of finished folders (ADR 0110): one cut for a task with its
//! dispatch record, what git ignores in every folder of a clone, whether a branch is there,
//! and a line of what a command said as text.

use std::path::Path;

use purlis_core::chatpiece::Cut;
use purlis_core::dispatchplace::{self, Repo};
use purlis_core::dispatchrecord;
use purlis_core::repocmd::Say;
use purlis_core::worktree::git::Isolated;

use super::Fixture;

/// A folder cut for the task `task` as a dispatch cuts one, its record stored, ended or still
/// running.
pub fn task_folder(f: &Fixture, task: &str, ended: bool) -> Cut {
    use dispatchrecord::{Asker, ChatRef, Mode, Opening, Place, Worker, Worktree};
    let id = dispatchrecord::mint();
    let repo = Repo {
        workspace: f.ws.clone(),
        repo: f.repo.clone(),
        piece: None,
        from_default: false,
    };
    let cut = dispatchplace::cut(&f.plane, &repo, task, &id, &Isolated::default()).expect("cut");
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

/// `rule` added to what the clone's git ignores, in every folder of it.
pub fn excluding(f: &Fixture, rule: &str) {
    use std::io::Write;
    std::fs::create_dir_all(f.clone.join(".git/info")).unwrap();
    let mut exclude = std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(f.clone.join(".git/info/exclude"))
        .unwrap();
    writeln!(exclude, "{rule}").unwrap();
}

/// Whether the clone at `clone` has the local branch `branch`.
pub fn has_branch(clone: &Path, branch: &str) -> bool {
    let mut ask = super::unsigned();
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

/// A line of what a command said, as its text.
pub fn spoken(line: &Say) -> String {
    match line {
        Say::Info(t) | Say::Done(t) | Say::Warn(t) | Say::Fail(t) | Say::Plain(t) => t.to_string(),
        Say::Out(t) => t.clone(),
    }
}

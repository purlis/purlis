// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! ADR 0110 (#5168): `purlis worktree prune` removes every finished folder of a workspace,
//! task folders and plain ones, a folder that links into another before it, and keeps every
//! other folder, saying why.
//!
//! A real clone, plain folders cut as the terminal cuts them, task folders cut as a dispatch
//! cuts them with their records stored, and the core's prune run as a terminal and as the app
//! runs it for a chat.

use crate::support;

use std::path::{Path, PathBuf};

use purlis_core::dispatchplace::{self, Repo};
use purlis_core::dispatchrecord;
use purlis_core::piececlaim::Live;
use purlis_core::pieces::{self, Event, Who};
use purlis_core::prune::{self, Asked, Asker};
use purlis_core::repocmd::Say;
use purlis_core::worktree::git::Isolated;

use support::git;

/// A plain folder `name`, cut from the clone's HEAD as the terminal cuts one.
fn plain(f: &support::Fixture, name: &str) -> PathBuf {
    let code = purlis_core::piececmd::add(
        &f.plane,
        &f.ws,
        &f.repo,
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
    f.workspace().join(".worktrees").join(&f.repo).join(name)
}

/// A folder cut for the task `task`, its record stored, ended or still running: its folder,
/// piece and branch.
fn task(f: &support::Fixture, task: &str, ended: bool) -> purlis_core::chatpiece::Cut {
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

/// Runs the prune from the person's terminal, and what it said.
fn pruned(f: &support::Fixture, dry_run: bool, keep_branches: bool) -> (u8, String) {
    pruned_by(
        f,
        dry_run,
        keep_branches,
        &Who::here(None, None, None),
        None,
    )
}

fn pruned_by(
    f: &support::Fixture,
    dry_run: bool,
    keep_branches: bool,
    who: &Who,
    live: Option<&[Live]>,
) -> (u8, String) {
    let mut said = Vec::new();
    let code = prune::prune(
        &Asked {
            ws: &f.ws,
            repo: None,
            dry_run,
            keep_branches,
        },
        &Asker {
            who,
            live,
            brokered: false,
        },
        &f.plane,
        chrono::Utc::now(),
        &mut |line| said.push(spoken(&line)),
    );
    (code, said.join("\n"))
}

fn spoken(line: &Say) -> String {
    match line {
        Say::Info(t) | Say::Done(t) | Say::Warn(t) | Say::Fail(t) | Say::Plain(t) => t.to_string(),
        Say::Out(t) => t.clone(),
    }
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

#[test]
fn every_finished_folder_goes_with_its_branch_and_every_other_stays_saying_why() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("api");
    let done = plain(&f, "done");
    let ahead = plain(&f, "ahead");
    f.commit(&ahead, "only-here");
    let dirty = plain(&f, "dirty");
    std::fs::write(dirty.join("README.md"), "changed\n").unwrap();
    let ended = task(&f, "ended task", true);
    let running = task(&f, "running task", false);

    let (code, said) = pruned(&f, false, false);

    assert_eq!(code, 0, "{said}");
    assert!(!done.exists(), "{said}");
    assert!(!has_branch(&f.clone, "done"), "{said}");
    assert!(!ended.path.exists(), "{said}");
    assert!(!has_branch(&f.clone, &ended.branch), "{said}");
    assert!(
        said.contains("Kept api · ahead: its branch holds commits"),
        "{said}"
    );
    assert!(
        said.contains("Kept api · dirty: it holds uncommitted changes"),
        "{said}"
    );
    assert!(
        said.contains(&format!(
            "Kept api · {}: its task 'running task' has not ended",
            running.piece
        )),
        "{said}"
    );
    assert!(ahead.is_dir() && dirty.is_dir() && running.path.is_dir());
    assert!(
        said.contains("Removed 2 finished folders, kept 3."),
        "{said}"
    );
}

#[test]
fn a_dry_run_says_what_would_go_and_removes_nothing() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("api");
    let done = plain(&f, "done");
    let dirty = plain(&f, "dirty");
    std::fs::write(dirty.join("README.md"), "changed\n").unwrap();

    let (code, said) = pruned(&f, true, false);

    assert_eq!(code, 0, "{said}");
    assert!(
        said.contains("Would remove api · done, and delete its branch"),
        "{said}"
    );
    assert!(said.contains("Kept api · dirty"), "{said}");
    assert!(said.contains("Nothing was removed."), "{said}");
    assert!(done.is_dir() && has_branch(&f.clone, "done"));
}

#[test]
fn keep_branches_removes_the_folders_and_leaves_their_branches() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("api");
    let done = plain(&f, "done");

    let (code, said) = pruned(&f, false, true);

    assert_eq!(code, 0, "{said}");
    assert!(!done.exists(), "{said}");
    assert!(has_branch(&f.clone, "done"), "{said}");
}

#[test]
fn a_squash_landed_folder_goes_and_its_branch_with_it() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("api");
    let lane = plain(&f, "lane");
    f.commit(&lane, "one");
    f.commit(&lane, "two");
    f.commit(&f.clone, "theirs");
    git(&f.clone, &["merge", "-q", "--squash", "lane"]);
    git(&f.clone, &["commit", "-q", "-m", "squashed lane"]);

    let (code, said) = pruned(&f, false, false);

    assert_eq!(code, 0, "{said}");
    assert!(!lane.exists(), "{said}");
    assert!(!has_branch(&f.clone, "lane"), "{said}");
}

#[test]
fn a_folder_on_no_branch_at_a_commit_of_its_base_goes() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("api");
    let loose = plain(&f, "loose");
    git(&loose, &["checkout", "-q", "--detach"]);

    let (code, said) = pruned(&f, false, false);

    assert_eq!(code, 0, "{said}");
    assert!(!loose.exists(), "{said}");
}

#[test]
fn a_linking_folder_goes_before_the_folder_it_links_into_and_one_that_stays_keeps_it() {
    purlis_core::unsteered!();
    // The live case: a plain lane links its `app/node_modules` into a task's folder.
    let f = support::plane_with_clone("api");
    excluding(&f, "node_modules");
    std::fs::create_dir_all(f.clone.join("app")).unwrap();
    std::fs::write(f.clone.join("app/package.json"), "{}\n").unwrap();
    git(&f.clone, &["add", "app/package.json"]);
    git(&f.clone, &["commit", "-q", "-m", "app"]);
    let shared = task(&f, "shares its install", true);
    std::fs::create_dir_all(shared.path.join("app/node_modules/left-pad")).unwrap();
    let flakes = plain(&f, "flakes");
    std::os::unix::fs::symlink(
        shared.path.join("app/node_modules"),
        flakes.join("app/node_modules"),
    )
    .unwrap();
    // A second pair, whose linking folder is not finished: both stay.
    let kept_into = task(&f, "kept for a link", true);
    std::fs::create_dir_all(kept_into.path.join("app/node_modules")).unwrap();
    let dirty = plain(&f, "dirty");
    std::fs::write(dirty.join("README.md"), "changed\n").unwrap();
    std::os::unix::fs::symlink(
        kept_into.path.join("app/node_modules"),
        dirty.join("app/node_modules"),
    )
    .unwrap();

    let (code, said) = pruned(&f, false, false);

    assert_eq!(code, 0, "{said}");
    assert!(!flakes.exists(), "{said}");
    assert!(!shared.path.exists(), "{said}");
    let first = said.find("Removed api · flakes").expect(&said);
    let then = said
        .find(&format!("Removed api · {}", shared.piece))
        .expect(&said);
    assert!(first < then, "the linking folder goes first: {said}");
    assert!(kept_into.path.is_dir() && dirty.is_dir());
    assert!(
        said.contains(&format!(
            "Kept api · {}: api · dirty links into it",
            kept_into.piece
        )),
        "{said}"
    );
}

#[test]
fn a_folder_an_open_chat_holds_stays_until_it_declares_done() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("api");
    let held = plain(&f, "held");
    let holder = Who::here(None, Some("9".to_owned()), None).for_chat(Some("chat-id-9".to_owned()));
    pieces::record(
        &f.plane,
        &f.ws,
        Event::Claimed,
        &f.repo,
        "held",
        None,
        &holder,
        chrono::Utc::now(),
    )
    .unwrap();
    let person = Who::here(None, None, None);
    let open = [Live {
        id: "chat-id-9".to_owned(),
        chat: Some(9),
        back: true,
    }];

    // No app to say whether chat 9 is open: kept.
    let (_, said) = pruned_by(&f, true, false, &person, None);
    assert!(
        said.contains("Kept api · held: chat 9 holds it, and whether"),
        "{said}"
    );
    // Open: kept.
    let (_, said) = pruned_by(&f, true, false, &person, Some(&open));
    assert!(
        said.contains(
            "Kept api · held: chat 9 holds it and has declared neither done nor abandoned"
        ),
        "{said}"
    );
    // Closed: it goes.
    let (code, said) = pruned_by(&f, false, false, &person, Some(&[]));
    assert_eq!(code, 0, "{said}");
    assert!(!held.exists(), "{said}");
}

/// What the app answers a chat standing at `cwd` that asks for a prune.
fn pruned_for_a_chat(
    f: &support::Fixture,
    cwd: &Path,
    dry_run: bool,
) -> (purlis_core::hookwire::Answer, String) {
    use purlis_core::hookwire::{Answer, GitAsk, GitWork};
    let answer = purlis_core::gitbroker::answer(
        &f.plane,
        &purlis_core::gitbroker::Asker {
            chat: 4,
            cwd: Some(cwd.to_path_buf()),
            persona: None,
            harnessed: true,
            unsandboxed: false,
            config: None,
            id: None,
            live: Some(Vec::new()),
        },
        &GitAsk {
            chat: 4,
            workspace: f.ws.clone(),
            alive: false,
            work: GitWork::WorktreePrune {
                repo: None,
                dry_run,
                keep_branches: false,
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
fn the_app_prunes_for_a_chat_from_the_workspace_and_never_from_inside_a_branch_folder() {
    purlis_core::unsteered!();
    use purlis_core::hookwire::Answer;
    let f = support::plane_with_clone("api");
    let done = plain(&f, "done");
    let here = plain(&f, "here");

    let (answer, said) = pruned_for_a_chat(&f, &here, false);
    assert!(matches!(answer, Answer::No { .. }), "{said}");
    assert!(
        said.contains("stands in branch folder api · here"),
        "{said}"
    );
    assert!(done.is_dir() && here.is_dir());

    let (answer, said) = pruned_for_a_chat(&f, &f.workspace(), false);
    assert!(matches!(answer, Answer::Said { code: 0, .. }), "{said}");
    assert!(!done.exists() && !here.exists(), "{said}");
    assert!(!has_branch(&f.clone, "done"), "{said}");
}

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

use purlis_core::finished::{Forge, Request};
use purlis_core::piececlaim::Live;
use purlis_core::pieces::{self, Event, Who};
use purlis_core::prune::{self, Asked, Asker};
use purlis_core::worktree::git::Isolated;

use support::folders::{excluding, has_branch, spoken, task_folder};
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
            forge: purlis_core::finished::Forge::none(),
        },
        &Asker {
            who,
            live,
            brokered: false,
            stands_in: None,
        },
        &f.plane,
        chrono::Utc::now(),
        &mut |line| said.push(spoken(&line)),
    );
    (code, said.join("\n"))
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
    let ended = task_folder(&f, "ended task", true);
    let running = task_folder(&f, "running task", false);

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
    let shared = task_folder(&f, "shares its install", true);
    std::fs::create_dir_all(shared.path.join("app/node_modules/left-pad")).unwrap();
    let flakes = plain(&f, "flakes");
    std::os::unix::fs::symlink(
        shared.path.join("app/node_modules"),
        flakes.join("app/node_modules"),
    )
    .unwrap();
    // A second pair, whose linking folder is not finished: both stay.
    let kept_into = task_folder(&f, "kept for a link", true);
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
            roots_git_held: false,
            unreadable: Vec::new(),
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
fn the_app_prunes_for_a_chat_and_keeps_the_folder_the_chat_stands_in() {
    purlis_core::unsteered!();
    use purlis_core::hookwire::Answer;
    let f = support::plane_with_clone("api");
    let done = plain(&f, "done");
    let here = plain(&f, "here");

    let (answer, said) = pruned_for_a_chat(&f, &here, false);
    assert!(matches!(answer, Answer::Said { code: 0, .. }), "{said}");
    assert!(!done.exists(), "{said}");
    assert!(!has_branch(&f.clone, "done"), "{said}");
    assert!(
        said.contains("Kept api · here: this chat stands in it"),
        "{said}"
    );
    assert!(here.is_dir());

    let (answer, said) = pruned_for_a_chat(&f, &f.workspace(), false);
    assert!(matches!(answer, Answer::Said { code: 0, .. }), "{said}");
    assert!(!here.exists(), "{said}");
}

#[test]
fn a_folder_already_gone_goes_where_its_branch_loses_nothing_and_else_is_listed_with_how_to_clear_it()
 {
    purlis_core::unsteered!();
    // #5439: only git's record of it is left; its branch is read alone.
    let f = support::plane_with_clone("api");
    let gone = plain(&f, "gone");
    std::fs::remove_dir_all(&gone).unwrap();
    let ahead = plain(&f, "ahead");
    f.commit(&ahead, "only-here");
    std::fs::remove_dir_all(&ahead).unwrap();

    let (code, said) = pruned(&f, true, false);
    assert_eq!(code, 0, "{said}");
    assert!(
        said.contains("Would remove api · gone, and delete its branch"),
        "{said}"
    );
    assert!(
        said.contains("Kept api · ahead: its folder is already gone")
            && said.contains("`purlis worktree remove api ahead`")
            && said.contains("and its branch holds commits its base does not have"),
        "{said}"
    );

    let (code, said) = pruned(&f, false, false);
    assert_eq!(code, 0, "{said}");
    assert!(!has_branch(&f.clone, "gone"), "{said}");
    assert!(has_branch(&f.clone, "ahead"), "{said}");
}

#[test]
fn a_link_inside_an_untracked_folder_still_keeps_what_it_links_into() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("api");
    let into = plain(&f, "into");
    let linking = plain(&f, "linking");
    // An untracked folder, so the linking folder is not finished, with a link one level down.
    std::fs::create_dir_all(linking.join("scratch")).unwrap();
    std::fs::write(linking.join("scratch/notes.txt"), "mine\n").unwrap();
    std::os::unix::fs::symlink(into.join("README.md"), linking.join("scratch/pointer")).unwrap();

    let (_, said) = pruned(&f, false, false);

    assert!(into.is_dir(), "{said}");
    assert!(
        said.contains("Kept api · into: api · linking links into it"),
        "{said}"
    );
}

/// The commit `rev` names in `dir`.
fn sha(dir: &Path, rev: &str) -> String {
    String::from_utf8(git(dir, &["rev-parse", rev]).stdout)
        .unwrap()
        .trim()
        .to_owned()
}

/// The prune from the person's terminal, the forge answering `forge`.
fn pruned_asking(
    f: &support::Fixture,
    dry_run: bool,
    forge: &(dyn Fn(&str, &str) -> Request + Sync),
) -> (u8, String) {
    let mut said = Vec::new();
    let code = prune::prune(
        &Asked {
            ws: &f.ws,
            repo: None,
            dry_run,
            keep_branches: false,
            forge: Forge::asking(forge),
        },
        &Asker {
            who: &Who::here(None, None, None),
            live: None,
            brokered: false,
            stands_in: None,
        },
        &f.plane,
        chrono::Utc::now(),
        &mut |line| said.push(spoken(&line)),
    );
    (code, said.join("\n"))
}

/// A lane whose request got one more commit after its tip, changing a file the tip has, and
/// squash-merged as that (#5439): its tip is neither on its base nor carried. Answers the lane's folder and the head commit,
/// which the clone has.
fn a_lane_inside_a_merged_head(f: &support::Fixture) -> (PathBuf, String) {
    let lane = plain(f, "lane");
    f.commit(&lane, "one");
    f.commit(&lane, "two");
    // The request's head: one commit more, made where the lane's folder does not see it.
    git(&f.clone, &["branch", "-q", "pr-head", "lane"]);
    let head_tree = f.workspace().join("pr-head-tree");
    let at = head_tree.to_str().unwrap();
    git(&f.clone, &["worktree", "add", "-q", at, "pr-head"]);
    // It changes a file the tip has, so the tip's files are not what landed.
    std::fs::write(head_tree.join("two"), "two, reviewed\n").unwrap();
    f.commit(&head_tree, "three");
    let head = sha(&head_tree, "HEAD");
    git(&f.clone, &["merge", "-q", "--squash", "pr-head"]);
    git(&f.clone, &["commit", "-q", "-m", "squashed request 7"]);
    git(&f.clone, &["worktree", "remove", "--force", at]);
    git(&f.clone, &["branch", "-q", "-D", "pr-head"]);
    (lane, head)
}

fn merged_at(head: &str) -> impl Fn(&str, &str) -> Request + Sync + '_ {
    move |repo, branch| {
        assert_eq!(repo, "api");
        if branch == "lane" {
            Request::Merged {
                number: 7,
                head: head.to_owned(),
                head_ref: "refs/pull/7/head".to_owned(),
            }
        } else {
            Request::NotMerged
        }
    }
}

#[test]
fn a_tip_inside_a_merged_requests_head_goes_and_its_branch_with_it() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("api");
    let (lane, head) = a_lane_inside_a_merged_head(&f);
    let forge = merged_at(&head);

    let (code, said) = pruned_asking(&f, true, &forge);
    assert_eq!(code, 0, "{said}");
    assert!(
        said.contains(
            "Would remove api · lane, and delete its branch (its commits are in pull request 7, \
             which merged)"
        ),
        "{said}"
    );

    let (code, said) = pruned_asking(&f, false, &forge);
    assert_eq!(code, 0, "{said}");
    assert!(!lane.exists(), "{said}");
    assert!(!has_branch(&f.clone, "lane"), "{said}");
}

#[test]
fn a_merged_head_the_clone_lacks_is_fetched_by_its_requests_ref() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("api");
    let origin = f.workspace().join("origin.git");
    let (clone_at, origin_at) = (f.clone.to_str().unwrap(), origin.to_str().unwrap());
    git(
        &f.workspace(),
        &["clone", "-q", "--bare", clone_at, "origin.git"],
    );
    git(&f.clone, &["remote", "add", "origin", origin_at]);
    let lane = plain(&f, "lane");
    f.commit(&lane, "one");
    git(&lane, &["push", "-q", "origin", "lane"]);
    // The request's last commit is made elsewhere, and only the forge has it.
    let elsewhere = f.workspace().join("elsewhere");
    git(&f.workspace(), &["clone", "-q", origin_at, "elsewhere"]);
    git(&elsewhere, &["switch", "-q", "lane"]);
    f.commit(&elsewhere, "two");
    git(
        &elsewhere,
        &["push", "-q", "origin", "HEAD:refs/pull/7/head"],
    );
    let head = sha(&elsewhere, "HEAD");
    git(&elsewhere, &["push", "-q", "origin", "--delete", "lane"]);
    assert!(
        !git_ok(&f.clone, &["cat-file", "-e", &format!("{head}^{{commit}}")]),
        "the clone must not have the head yet"
    );
    let forge = merged_at(&head);

    let (code, said) = pruned_asking(&f, false, &forge);

    assert_eq!(code, 0, "{said}");
    assert!(!lane.exists(), "{said}");
    assert!(!has_branch(&f.clone, "lane"), "{said}");
}

#[test]
fn a_merged_request_whose_head_does_not_hold_the_tip_keeps_it() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("api");
    let (lane, head) = a_lane_inside_a_merged_head(&f);
    f.commit(&lane, "after the merge");
    let forge = merged_at(&head);

    let (_, said) = pruned_asking(&f, false, &forge);

    assert!(lane.is_dir(), "{said}");
    assert!(
        said.contains("Kept api · lane: its branch holds commits its base does not have"),
        "{said}"
    );
}

#[test]
fn offline_a_branch_ahead_of_its_base_is_kept_saying_the_forge_could_not_be_asked() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("api");
    let (lane, _) = a_lane_inside_a_merged_head(&f);
    let offline = |_: &str, _: &str| Request::Unasked("the forge did not answer".to_owned());

    let (_, said) = pruned_asking(&f, false, &offline);

    assert!(lane.is_dir(), "{said}");
    assert!(
        said.contains(
            "Kept api · lane: its branch holds commits its base does not have, and whether a \
             merged pull request holds them could not be told (the forge did not answer)"
        ),
        "{said}"
    );
}

/// Whether git in `dir` exits 0.
fn git_ok(dir: &Path, args: &[&str]) -> bool {
    let mut git = std::process::Command::new("git");
    git.arg("-C").arg(dir).args(args);
    purlis_core::forklock::output(&mut git).is_ok_and(|out| out.status.success())
}

#[test]
fn a_plain_remove_lets_a_lane_inside_a_merged_head_go_and_deletes_its_branch_when_asked() {
    purlis_core::unsteered!();
    // #5439: `flakes-4672-5126` was refused, listing commits "that exist nowhere else", though
    // they were part of a request that merged.
    let f = support::plane_with_clone("api");
    let (lane, head) = a_lane_inside_a_merged_head(&f);
    let forge = merged_at(&head);
    let mut said = Vec::new();

    let code = purlis_core::piececmd::remove(
        &f.plane,
        &f.ws,
        &f.repo,
        "lane",
        false,
        true,
        Forge::asking(&forge),
        &mut |line| said.push(spoken(&line)),
    );

    let said = said.join("\n");
    assert_eq!(code, 0, "{said}");
    assert!(!lane.exists(), "{said}");
    assert!(!has_branch(&f.clone, "lane"), "{said}");
    assert!(
        said.contains("Deleted branch lane, whose commits merged in pull request 7"),
        "{said}"
    );
}

#[test]
fn a_task_folder_inside_a_merged_head_is_finished_for_its_removal() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("api");
    let cut = task_folder(&f, "lands through a request", true);
    f.commit(&cut.path, "one");
    // The request's last commit, made where the folder does not see it, changes the tip's file.
    git(&f.clone, &["branch", "-q", "pr-head", &cut.branch]);
    let head_tree = f.workspace().join("pr-head-tree");
    let at = head_tree.to_str().unwrap();
    git(&f.clone, &["worktree", "add", "-q", at, "pr-head"]);
    std::fs::write(head_tree.join("one"), "one, reviewed\n").unwrap();
    f.commit(&head_tree, "two");
    let head = sha(&head_tree, "HEAD");
    git(&f.clone, &["merge", "-q", "--squash", "pr-head"]);
    git(&f.clone, &["commit", "-q", "-m", "squashed request 7"]);
    git(&f.clone, &["worktree", "remove", "--force", at]);
    git(&f.clone, &["branch", "-q", "-D", "pr-head"]);
    let branch = cut.branch.clone();
    let forge = move |_: &str, asked: &str| {
        if asked == branch {
            Request::Merged {
                number: 7,
                head: head.clone(),
                head_ref: "refs/pull/7/head".to_owned(),
            }
        } else {
            Request::NotMerged
        }
    };
    let kept = |forge: Forge<'_>| {
        purlis_core::dispatchplace::kept_from_removal(
            &f.plane,
            &f.ws,
            &f.repo,
            &cut.piece,
            false,
            true,
            || false,
            forge,
        )
    };

    let offline = kept(Forge::none()).expect("not finished without the forge");
    assert!(
        offline.contains("whether a merged pull request holds them could not be told"),
        "{offline}"
    );
    assert_eq!(kept(Forge::asking(&forge)), None);
}

/// The clone's config lines for `branch`, as `git config --get-regexp` prints them.
fn branch_config(clone: &Path, branch: &str) -> String {
    let mut git = std::process::Command::new("git");
    git.arg("-C")
        .arg(clone)
        .args(["config", "--get-regexp", &format!(r"^branch\.{branch}\.")]);
    String::from_utf8(purlis_core::forklock::output(&mut git).unwrap().stdout).unwrap()
}

/// Takes `folder` away as a removal by hand does, leaving its branch.
fn gone_by_hand(f: &support::Fixture, folder: &Path) {
    git(
        &f.clone,
        &["worktree", "remove", "--force", folder.to_str().unwrap()],
    );
}

#[test]
fn a_branch_purlis_cut_whose_folder_is_gone_is_deleted_with_its_config_where_nothing_is_lost() {
    purlis_core::unsteered!();
    // #5439: a removal by another chat left both branches behind, and nothing deleted them.
    let f = support::plane_with_clone("api");
    let on_base = plain(&f, "on-base");
    gone_by_hand(&f, &on_base);
    let carried = plain(&f, "carried");
    f.commit(&carried, "one");
    git(&f.clone, &["merge", "-q", "--squash", "carried"]);
    git(&f.clone, &["commit", "-q", "-m", "squashed carried"]);
    gone_by_hand(&f, &carried);
    let ahead = plain(&f, "ahead");
    f.commit(&ahead, "only-here");
    gone_by_hand(&f, &ahead);
    assert!(branch_config(&f.clone, "carried").contains("charterbase"));

    let (code, said) = pruned(&f, true, false);
    assert_eq!(code, 0, "{said}");
    assert!(
        said.contains(
            "Would delete branch on-base of api, whose folder is gone (it is on its base)"
        ),
        "{said}"
    );
    assert!(
        said.contains(
            "Would delete branch carried of api, whose folder is gone (it is carried by a squash merge)"
        ),
        "{said}"
    );
    assert!(
        said.contains("Kept branch ahead of api: its branch holds commits its base does not have"),
        "{said}"
    );
    assert!(has_branch(&f.clone, "on-base") && has_branch(&f.clone, "carried"));

    let (code, said) = pruned(&f, false, false);
    assert_eq!(code, 0, "{said}");
    assert!(!has_branch(&f.clone, "on-base"), "{said}");
    assert!(!has_branch(&f.clone, "carried"), "{said}");
    assert!(has_branch(&f.clone, "ahead"), "{said}");
    assert_eq!(branch_config(&f.clone, "carried"), "", "{said}");
    assert!(
        said.contains("Deleted 2 branches whose folders are gone, kept 1."),
        "{said}"
    );
}

#[test]
fn keep_branches_leaves_every_branch_whose_folder_is_gone() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("api");
    let on_base = plain(&f, "on-base");
    gone_by_hand(&f, &on_base);

    let (_, said) = pruned(&f, false, true);

    assert!(has_branch(&f.clone, "on-base"), "{said}");
    assert!(!said.contains("on-base"), "{said}");
}

#[test]
fn the_app_deletes_a_branch_whose_folder_is_gone_for_a_chat_config_and_all() {
    purlis_core::unsteered!();
    use purlis_core::hookwire::Answer;
    let f = support::plane_with_clone("api");
    let carried = plain(&f, "carried");
    f.commit(&carried, "one");
    git(&f.clone, &["merge", "-q", "--squash", "carried"]);
    git(&f.clone, &["commit", "-q", "-m", "squashed carried"]);
    gone_by_hand(&f, &carried);

    let (answer, said) = pruned_for_a_chat(&f, &f.workspace(), false);

    assert!(matches!(answer, Answer::Said { code: 0, .. }), "{said}");
    assert!(!has_branch(&f.clone, "carried"), "{said}");
    assert_eq!(branch_config(&f.clone, "carried"), "", "{said}");
}

#[test]
fn remove_delete_branch_on_a_gone_task_folder_deletes_its_carried_branch_and_keeps_an_ahead_one() {
    purlis_core::unsteered!();
    // #5439: it refused with "Run this again without --delete-branch to remove the folder",
    // and the folder was already gone.
    let f = support::plane_with_clone("api");
    let removed = |piece: &str| {
        let mut said = Vec::new();
        let code = purlis_core::piececmd::remove(
            &f.plane,
            &f.ws,
            &f.repo,
            piece,
            false,
            true,
            Forge::none(),
            &mut |line| said.push(spoken(&line)),
        );
        (code, said.join("\n"))
    };
    let carried = task_folder(&f, "a carried task", true);
    f.commit(&carried.path, "one");
    git(&f.clone, &["merge", "-q", "--squash", &carried.branch]);
    git(&f.clone, &["commit", "-q", "-m", "squashed the task"]);
    gone_by_hand(&f, &carried.path);
    let ahead = task_folder(&f, "an unfinished task", true);
    f.commit(&ahead.path, "only-here");
    gone_by_hand(&f, &ahead.path);

    let (code, said) = removed(&carried.piece);
    assert_eq!(code, 0, "{said}");
    assert!(!has_branch(&f.clone, &carried.branch), "{said}");
    assert!(
        said.contains(&format!(
            "Deleted branch {}, whose folder was already gone (it is carried by a squash merge)",
            carried.branch
        )),
        "{said}"
    );

    let (code, said) = removed(&ahead.piece);
    assert_eq!(code, 1, "{said}");
    assert!(has_branch(&f.clone, &ahead.branch), "{said}");
    assert!(
        said.contains(&format!(
            "The folder of api · {} is already gone, and its branch {} is kept: its branch \
             holds commits its base does not have",
            ahead.piece, ahead.branch
        )),
        "{said}"
    );
    assert!(!said.contains("without --delete-branch"), "{said}");
}

#[test]
fn a_chats_remove_delete_branch_on_a_gone_folder_deletes_its_branch_config_and_all() {
    purlis_core::unsteered!();
    // #5439: by the brokered route, so the branch's section of `.git/config` goes too, which a
    // sandboxed chat's own `git branch -D` could not write.
    use purlis_core::hookwire::{Answer, GitAsk, GitWork};
    let f = support::plane_with_clone("api");
    let carried = task_folder(&f, "a carried task", true);
    f.commit(&carried.path, "one");
    git(&f.clone, &["merge", "-q", "--squash", &carried.branch]);
    git(&f.clone, &["commit", "-q", "-m", "squashed the task"]);
    gone_by_hand(&f, &carried.path);

    let answer = purlis_core::gitbroker::answer(
        &f.plane,
        &purlis_core::gitbroker::Asker {
            chat: 4,
            cwd: Some(f.workspace()),
            persona: None,
            harnessed: true,
            unsandboxed: false,
            roots_git_held: false,
            unreadable: Vec::new(),
            config: None,
            id: None,
            live: Some(Vec::new()),
        },
        &GitAsk {
            chat: 4,
            workspace: f.ws.clone(),
            alive: false,
            work: GitWork::WorktreeRemove {
                repo: f.repo.clone(),
                piece: carried.piece.clone(),
                delete_branch: true,
            },
        },
        &Isolated::identity(Some("Tester"), Some("t@e.invalid")),
        chrono::Utc::now(),
    );

    let said = match &answer {
        Answer::Said { lines, .. } => lines.iter().map(spoken).collect::<Vec<_>>().join("\n"),
        other => format!("{other:?}"),
    };
    assert!(matches!(answer, Answer::Said { code: 0, .. }), "{said}");
    assert!(!has_branch(&f.clone, &carried.branch), "{said}");
    assert_eq!(branch_config(&f.clone, &carried.branch), "", "{said}");
}

#[test]
fn a_gone_folder_whose_task_has_not_ended_keeps_its_branch() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("api");
    let running = task_folder(&f, "still at work", false);
    gone_by_hand(&f, &running.path);
    let mut said = Vec::new();

    let code = purlis_core::piececmd::remove(
        &f.plane,
        &f.ws,
        &f.repo,
        &running.piece,
        false,
        true,
        Forge::none(),
        &mut |line| said.push(spoken(&line)),
    );

    let said = said.join("\n");
    assert_eq!(code, 1, "{said}");
    assert!(has_branch(&f.clone, &running.branch), "{said}");
    assert!(said.contains("its task has not ended"), "{said}");

    let (_, said) = pruned(&f, false, false);
    assert!(has_branch(&f.clone, &running.branch), "{said}");
}

#[test]
fn offline_a_plain_removal_says_the_forge_could_not_be_asked() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("api");
    let (lane, _) = a_lane_inside_a_merged_head(&f);
    let offline = |_: &str, _: &str| Request::Unasked("the forge did not answer".to_owned());
    let mut said = Vec::new();

    let code = purlis_core::piececmd::remove(
        &f.plane,
        &f.ws,
        &f.repo,
        "lane",
        false,
        false,
        Forge::asking(&offline),
        &mut |line| said.push(spoken(&line)),
    );

    let said = said.join("\n");
    assert_eq!(code, 1, "{said}");
    assert!(lane.is_dir(), "{said}");
    assert!(
        said.contains(
            "Whether a merged pull request holds its commits could not be told (the forge did \
             not answer)."
        ),
        "{said}"
    );
}

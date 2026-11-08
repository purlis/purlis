// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! #4500: `purlis worktree remove` lets a folder go without `--force` where its pull request
//! says nothing would be lost, and where it does not, says every reason at once.
//!
//! - a tip that is the head of a request merged through the queue, whose squash its base later
//!   changed again, goes;
//! - a tip that is the head of a request that closed without merging goes only where that
//!   head is on a branch of origin, and the refusal says the request closed;
//! - the folders a build or a dev server writes (`.astro/`, a `node_modules/` holding only
//!   Vite's cache) do not keep it;
//! - a folder kept for its commits and its ignored files is refused naming both.
//!
//! A real clone, folders cut as the terminal and a dispatch cut them, and the forge a stub:
//! the answer of `finished::merged::of_the_plane` for the branch, as a closure.

use crate::support;

use std::path::Path;

use purlis_core::chatpiece::Cut;
use purlis_core::finished::{Forge, Request};
use purlis_core::gitbroker::{self, Asker};
use purlis_core::hookwire::{Answer, GitAsk, GitWork};
use purlis_core::piececmd;
use purlis_core::pieces::Who;
use purlis_core::worktree::git::Isolated;

use support::folders::{has_branch, spoken, task_folder};
use support::git;

fn sha(dir: &Path, rev: &str) -> String {
    String::from_utf8(git(dir, &["rev-parse", rev]).stdout)
        .unwrap()
        .trim()
        .to_owned()
}

/// Commits `content` to `file` in `tree`, as `message`.
fn write_commit(tree: &Path, file: &str, content: &str, message: &str) {
    let at = tree.join(file);
    std::fs::create_dir_all(at.parent().unwrap()).unwrap();
    std::fs::write(at, content).unwrap();
    git(tree, &["add", "-A"]);
    git(tree, &["commit", "-q", "-m", message]);
}

/// The terminal's own removal of `piece`, not forced, the forge answering `forge`.
fn removed(
    f: &support::Fixture,
    piece: &str,
    delete_branch: bool,
    forge: &(dyn Fn(&str, &str) -> Request + Sync),
) -> (u8, String) {
    let mut said = Vec::new();
    let code = piececmd::remove(
        &f.plane,
        &f.ws,
        &f.repo,
        piece,
        false,
        delete_branch,
        Forge::asking(forge),
        &mut |line| said.push(spoken(&line)),
    );
    (code, said.join("\n"))
}

/// What the app answers a chat standing in the workspace that asks for `piece`'s removal.
fn from_a_chat(f: &support::Fixture, piece: &str) -> (Answer, String) {
    let answer = gitbroker::answer(
        &f.plane,
        &Asker {
            chat: 4,
            cwd: Some(f.plane.join("workspaces").join(&f.ws)),
            persona: None,
            harnessed: true,
            unsandboxed: false,
            roots_git_held: false,
            unreadable: Vec::new(),
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
                piece: piece.to_owned(),
                delete_branch: false,
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

/// A plain folder `name`, cut from the clone's HEAD as the terminal cuts one.
fn plain(f: &support::Fixture, name: &str) -> std::path::PathBuf {
    let code = piececmd::add(
        &f.plane,
        &f.ws,
        &f.repo,
        name,
        None,
        Some(piececmd::HEAD),
        None,
        None,
        &Who::here(None, None, None),
        chrono::Utc::now(),
        &mut |_| {},
    );
    assert_eq!(code, 0, "{name} was not cut");
    f.workspace().join(".worktrees").join(&f.repo).join(name)
}

/// Ignores `patterns` in every folder of the clone, as a `.gitignore` would, without a commit.
fn ignore(f: &support::Fixture, patterns: &str) {
    let info = f.clone.join(".git").join("info");
    std::fs::create_dir_all(&info).unwrap();
    std::fs::write(info.join("exclude"), patterns).unwrap();
}

/// A forge whose newest request from `branch` is `request`, and that knows no other branch.
fn answering(branch: String, request: Request) -> impl Fn(&str, &str) -> Request + Sync {
    move |_, asked| {
        if asked == branch {
            request.clone()
        } else {
            Request::NotMerged
        }
    }
}

/// The request `number` from a branch, closed without merging at `head`.
fn closed(number: u64, head: &str) -> Request {
    Request::Closed {
        number,
        head: head.to_owned(),
        head_ref: format!("refs/pull/{number}/head"),
    }
}

/// A bare repository beside the clone, made its `origin`, and its path.
fn an_origin(f: &support::Fixture) -> std::path::PathBuf {
    let origin = f.plane.join("origin.git");
    git(
        &f.plane,
        &["init", "-q", "--bare", "-b", "main", "origin.git"],
    );
    let at = origin.to_str().unwrap().to_owned();
    git(&f.clone, &["remote", "add", "origin", &at]);
    git(&f.clone, &["push", "-q", "origin", "main"]);
    origin
}

/// A task folder with two commits, the second a review's, whose request then landed through
/// the queue: squashed onto a base that had moved under it, so the squash is not the branch's
/// own change, and its base changed the branch's file again after. Its tip is the request's
/// head.
fn queue_merged(f: &support::Fixture) -> (Cut, String) {
    let cut = task_folder(f, "lands through the queue", true);
    write_commit(&cut.path, "one", "one\n", "one");
    write_commit(&cut.path, "two", "two\n", "review");
    let head = sha(&cut.path, "HEAD");
    write_commit(&f.clone, "two", "two, from the base\n", "the base moves");
    git(&f.clone, &["rm", "-q", "two"]);
    write_commit(&f.clone, "one", "one\n", "squash of request 9 (#9)");
    write_commit(
        &f.clone,
        "two",
        "two, as the queue resolved it\n",
        "resolve",
    );
    write_commit(&f.clone, "one", "one, changed since\n", "later");
    (cut, head)
}

#[test]
fn a_task_folder_whose_tip_is_the_head_of_a_request_merged_through_the_queue_goes_with_its_branch()
{
    purlis_core::unsteered!();
    let f = support::plane_with_clone("api");
    let (cut, head) = queue_merged(&f);
    let forge = answering(
        cut.branch.clone(),
        Request::Merged {
            number: 9,
            head,
            head_ref: "refs/pull/9/head".to_owned(),
        },
    );

    let (code, said) = removed(&f, &cut.piece, true, &forge);

    assert_eq!(code, 0, "{said}");
    assert!(!cut.path.exists(), "{said}");
    assert!(!has_branch(&f.clone, &cut.branch), "{said}");
    assert!(said.contains("pull request 9"), "{said}");
}

#[test]
fn a_task_folder_whose_request_closed_with_its_head_on_origin_goes_saying_the_request_closed() {
    purlis_core::unsteered!();
    // `4563-the-waiting-strip`: its request #4708 closed, superseded by another branch's, and
    // its head stayed on origin. The task guard said its branch held commits its base did not.
    let f = support::plane_with_clone("api");
    let cut = task_folder(&f, "superseded", true);
    write_commit(&cut.path, "one", "one\n", "one");
    let head = sha(&cut.path, "HEAD");
    let origin = an_origin(&f);
    git(&f.clone, &["push", "-q", "origin", &cut.branch]);
    git(&origin, &["update-ref", "refs/pull/12/head", &head]);
    let forge = answering(cut.branch.clone(), closed(12, &head));

    let (code, said) = removed(&f, &cut.piece, false, &forge);

    assert_eq!(code, 0, "{said}");
    assert!(!cut.path.exists(), "{said}");
    assert!(
        said.contains("pull request 12, which closed without merging, is on origin"),
        "{said}"
    );
}

#[test]
fn a_task_folder_whose_request_closed_with_its_head_on_no_branch_of_origin_is_kept_saying_so() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("api");
    let cut = task_folder(&f, "abandoned", true);
    write_commit(&cut.path, "one", "one\n", "one");
    let head = sha(&cut.path, "HEAD");
    an_origin(&f);
    let forge = answering(cut.branch.clone(), closed(12, &head));

    let (code, said) = removed(&f, &cut.piece, false, &forge);

    assert_eq!(code, 1, "{said}");
    assert!(cut.path.is_dir(), "{said}");
    assert!(
        said.contains(
            "its pull request 12 closed without merging, and its commits are on no branch of \
             origin"
        ),
        "{said}"
    );
}

#[test]
fn a_plain_folder_whose_request_closed_with_its_head_on_no_branch_of_origin_says_so_in_its_refusal()
{
    purlis_core::unsteered!();
    let f = support::plane_with_clone("api");
    let lane = plain(&f, "lane");
    write_commit(&lane, "one", "one\n", "one");
    let head = sha(&lane, "HEAD");
    an_origin(&f);
    let forge = answering("lane".to_owned(), closed(12, &head));

    let (code, said) = removed(&f, "lane", false, &forge);

    assert_eq!(code, 1, "{said}");
    assert!(lane.is_dir(), "{said}");
    assert!(said.contains("exists nowhere else"), "{said}");
    assert!(
        said.contains(
            "Its pull request 12 closed without merging, and its commits are on no branch of \
             origin."
        ),
        "{said}"
    );
}

#[test]
fn what_astro_and_vite_write_does_not_keep_a_finished_task_folder_from_a_chat() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("api");
    write_commit(&f.clone, "site/package.json", "{}\n", "the site");
    write_commit(&f.clone, "app/src/main.ts", "\n", "the app");
    ignore(&f, ".astro/\nnode_modules/\n.vite/\n");
    let cut = task_folder(&f, "built the site", true);
    for file in [
        "site/.astro/types.d.ts",
        "app/src/node_modules/.vite/deps/react.js",
        "app/.vite/manifest.json",
    ] {
        let at = cut.path.join(file);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(at, "built\n").unwrap();
    }

    let (answer, said) = from_a_chat(&f, &cut.piece);

    assert!(matches!(answer, Answer::Said { code: 0, .. }), "{said}");
    assert!(!cut.path.exists(), "{said}");
}

#[test]
fn a_node_modules_holding_more_than_vites_cache_away_from_a_manifest_still_keeps_a_folder() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("api");
    ignore(&f, "node_modules/\n");
    let cut = task_folder(&f, "vendored by hand", true);
    let at = cut.path.join("tools/node_modules/left-pad/index.js");
    std::fs::create_dir_all(at.parent().unwrap()).unwrap();
    std::fs::write(at, "kept\n").unwrap();

    let (answer, said) = from_a_chat(&f, &cut.piece);

    assert!(!matches!(answer, Answer::Said { code: 0, .. }), "{said}");
    assert!(said.contains("`tools/node_modules/`"), "{said}");
    assert!(cut.path.is_dir(), "{said}");
}

#[test]
fn a_chat_is_told_of_a_folders_commits_and_its_ignored_files_in_one_refusal() {
    purlis_core::unsteered!();
    // #4500: a refusal over ignored files came first, and only the next try named the commits.
    let f = support::plane_with_clone("api");
    ignore(&f, ".env\n");
    let lane = plain(&f, "lane");
    write_commit(&lane, "one", "one\n", "only here");
    std::fs::write(lane.join(".env"), "SECRET=1\n").unwrap();

    let (answer, said) = from_a_chat(&f, "lane");

    assert!(!matches!(answer, Answer::Said { code: 0, .. }), "{said}");
    assert!(lane.is_dir(), "{said}");
    assert!(said.contains("`.env`"), "{said}");
    assert!(said.contains("only here"), "{said}");
}

#[test]
fn a_task_folder_kept_for_its_commits_and_its_ignored_files_names_both() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("api");
    ignore(&f, ".env\n");
    let cut = task_folder(&f, "ahead and ignored", true);
    write_commit(&cut.path, "one", "one\n", "only here");
    std::fs::write(cut.path.join(".env"), "SECRET=1\n").unwrap();
    let forge = |_: &str, _: &str| Request::NotMerged;

    let (code, said) = removed(&f, &cut.piece, false, &forge);

    assert_eq!(code, 1, "{said}");
    assert!(
        said.contains("its branch holds commits its base does not have"),
        "{said}"
    );
    assert!(said.contains("`.env`"), "{said}");
}

#[test]
fn a_closed_requests_branch_deleted_on_origin_is_not_on_origin_however_the_clone_remembers_it() {
    purlis_core::unsteered!();
    // On review: the clone's `refs/remotes/origin/<branch>` outlives the branch on origin until
    // a fetch prunes it, and read alone it would let the commits go.
    let f = support::plane_with_clone("api");
    let cut = task_folder(&f, "deleted on origin", true);
    write_commit(&cut.path, "one", "one\n", "one");
    let head = sha(&cut.path, "HEAD");
    let origin = an_origin(&f);
    git(&f.clone, &["push", "-q", "origin", &cut.branch]);
    git(&origin, &["branch", "-q", "-D", &cut.branch]);
    let forge = answering(cut.branch.clone(), closed(12, &head));

    let (code, said) = removed(&f, &cut.piece, false, &forge);

    assert_eq!(code, 1, "{said}");
    assert!(cut.path.is_dir(), "{said}");
    assert!(
        said.contains(
            "its pull request 12 closed without merging, and its commits are on no branch of origin"
        ),
        "{said}"
    );
}

#[test]
fn a_closed_requests_head_the_clone_never_fetched_is_fetched_and_read() {
    purlis_core::unsteered!();
    // On review: a request closed after commits pushed from elsewhere has a head the clone
    // lacks; it is fetched by its own ref, not read as "no request".
    let f = support::plane_with_clone("api");
    let cut = task_folder(&f, "pushed from elsewhere", true);
    write_commit(&cut.path, "one", "one\n", "one");
    let origin = an_origin(&f);
    git(&f.clone, &["push", "-q", "origin", &cut.branch]);
    let elsewhere = f.plane.join("elsewhere");
    let from = origin.to_str().unwrap().to_owned();
    git(
        &f.plane,
        &["clone", "-q", "-b", &cut.branch, &from, "elsewhere"],
    );
    write_commit(&elsewhere, "two", "two\n", "review from elsewhere");
    git(&elsewhere, &["push", "-q", "origin", &cut.branch]);
    let head = sha(&elsewhere, "HEAD");
    git(&origin, &["update-ref", "refs/pull/12/head", &head]);
    let forge = answering(cut.branch.clone(), closed(12, &head));

    let (code, said) = removed(&f, &cut.piece, false, &forge);

    assert_eq!(code, 0, "{said}");
    assert!(!cut.path.exists(), "{said}");
    assert!(
        said.contains("pull request 12, which closed without merging, is on origin"),
        "{said}"
    );
}

#[test]
fn a_dirty_folder_is_refused_without_asking_the_forge() {
    purlis_core::unsteered!();
    // On review: a prune over many dirty folders asked the forge of each.
    let f = support::plane_with_clone("api");
    let cut = task_folder(&f, "dirty and ahead", true);
    write_commit(&cut.path, "one", "one\n", "only here");
    std::fs::write(cut.path.join("one"), "changed\n").unwrap();
    let asked = std::sync::atomic::AtomicBool::new(false);
    let forge = |_: &str, _: &str| {
        asked.store(true, std::sync::atomic::Ordering::Relaxed);
        Request::NotMerged
    };

    let (code, said) = removed(&f, &cut.piece, false, &forge);

    assert_eq!(code, 1, "{said}");
    assert!(said.contains("uncommitted changes"), "{said}");
    assert!(
        said.contains("its branch holds commits its base does not have"),
        "{said}"
    );
    assert!(!asked.load(std::sync::atomic::Ordering::Relaxed), "{said}");
}

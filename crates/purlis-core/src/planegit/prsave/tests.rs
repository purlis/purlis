// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! [`push_save_branch`] against local bare remotes: what it asks the remote, and what it says
//! when the remote refuses (#464). The PR modes end to end are
//! `tests/it/a_pr_mode_save_keeps_one_pr_open_from_the_save_branch.rs`, against a stand-in `gh`.

use std::path::{Path, PathBuf};

use super::*;

const SAVE: &str = "charter/save/test";

fn run(dir: &Path, args: &[&str]) -> String {
    let done = crate::testgit::run(dir, args);
    assert!(done.ok(), "git {args:?} failed: {done:?}");
    done.out.trim().to_string()
}

/// A repository with one commit, an identity of its own, and an empty bare remote beside it.
fn plane_and_remote() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let top = dir.path().canonicalize().unwrap();
    let root = top.join("plane");
    let bare = top.join("remote.git");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::create_dir_all(&bare).unwrap();
    run(&bare, &["init", "-q", "--bare", "-b", "main", "."]);
    run(&root, &["init", "-q", "-b", "main", "."]);
    run(&root, &["config", "user.name", "Fixture"]);
    run(&root, &["config", "user.email", "fixture@example.invalid"]);
    std::fs::write(root.join("f.md"), "one\n").unwrap();
    run(&root, &["add", "-A"]);
    run(&root, &["commit", "-q", "-m", "one"]);
    (dir, root, bare)
}

/// A hook in the bare remote `bare`.
fn hook(bare: &Path, name: &str, body: &str) {
    std::fs::create_dir_all(bare.join("hooks")).unwrap();
    stand_in::program(
        bare,
        &format!("hooks/{name}"),
        &format!("#!/bin/sh\n{body}\n"),
    );
}

/// How many git commands run in `root` named `verb`.
fn asked(root: &Path, verb: &str) -> usize {
    crate::worktree::git::tally::asked(root)
        .iter()
        .filter(|argv| argv.iter().any(|a| a == verb))
        .count()
}

/// [`push_save_branch`] of `root` to `url`, as a clone with nothing kept, and what it said.
fn push_first(root: &Path, url: &str) -> (Result<(), Box<PushResult>>, String) {
    let mut said = String::new();
    let mut say = |line: Say| {
        said.push_str(&line.to_string());
        said.push('\n');
    };
    let reach = Reach {
        https: url,
        helper: "",
        head: "HEAD",
    };
    let got = push_save_branch(reach, root, SAVE, "main", &Kept::default(), &mut say);
    (got, said)
}

#[test]
fn a_push_that_lands_asks_the_remote_nothing_more() {
    let (_dir, root, bare) = plane_and_remote();

    let (got, said) = push_first(&root, &format!("file://{}", bare.display()));

    assert!(got.is_ok(), "{said}");
    assert_eq!(asked(&root, "push"), 1);
    assert_eq!(asked(&root, "ls-remote"), 0);
}

#[test]
fn a_push_that_lands_has_landed_even_when_the_remote_goes_away_right_after() {
    let (_dir, root, bare) = plane_and_remote();
    let gone = bare.with_extension("gone");
    hook(
        &bare,
        "post-receive",
        &format!("mv '{}' '{}'", bare.display(), gone.display()),
    );

    let (got, said) = push_first(&root, &format!("file://{}", bare.display()));

    assert!(gone.exists(), "the hook ran");
    assert!(got.is_ok(), "{said}");
}

#[test]
fn a_push_the_remote_refuses_for_its_own_reason_asks_it_nothing_more() {
    let (_dir, root, bare) = plane_and_remote();
    hook(
        &bare,
        "pre-receive",
        "echo 'policy: no pushes today' >&2; exit 1",
    );

    let (got, said) = push_first(&root, &format!("file://{}", bare.display()));

    assert!(got.is_err(), "{said}");
    assert_eq!(asked(&root, "ls-remote"), 0);
}

#[test]
fn a_push_the_remote_refuses_keeps_the_remotes_reason_when_the_remote_then_goes_away() {
    let (_dir, root, bare) = plane_and_remote();
    let gone = bare.with_extension("gone");
    hook(
        &bare,
        "pre-receive",
        &format!(
            "echo 'policy: no pushes today' >&2; mv '{}' '{}'; exit 1",
            bare.display(),
            gone.display()
        ),
    );

    let (got, said) = push_first(&root, &format!("file://{}", bare.display()));

    let refused = got.expect_err("refused");
    assert!(gone.exists(), "the hook ran");
    assert!(
        refused.detail.contains("policy: no pushes today"),
        "{refused:?} {said}"
    );
}

#[test]
fn a_save_branch_deleted_on_the_remote_while_a_first_push_was_on_its_way_is_pushed_afresh() {
    // D-HY8g, amended after review: the first push finds somebody's branch there, so its absent
    // lease is refused; by the time charter lists the remote it is gone, as a merged request's
    // branch is deleted. Gone is leased again as absent, so the save lands. The moment is
    // made with the listing's own upload-pack (`remote.<url>.uploadpack`, which git reads for
    // a remote named by its URL), given on the command line: it deletes the branch, then lists.
    let (dir, root, bare) = plane_and_remote();
    let top = dir.path().canonicalize().unwrap();
    let other = top.join("other");
    std::fs::create_dir_all(&other).unwrap();
    run(&other, &["init", "-q", "-b", "main", "."]);
    run(&other, &["config", "user.name", "Other"]);
    run(&other, &["config", "user.email", "other@example.invalid"]);
    std::fs::write(other.join("o.md"), "theirs\n").unwrap();
    run(&other, &["add", "-A"]);
    run(&other, &["commit", "-q", "-m", "theirs"]);
    run(
        &other,
        &[
            "push",
            "-q",
            &bare.display().to_string(),
            &format!("HEAD:refs/heads/{SAVE}"),
        ],
    );
    let url = "https://forge.invalid/acme/plane.git";
    run(
        &root,
        &[
            "config",
            &format!("url.file://{}.insteadOf", bare.display()),
            url,
        ],
    );
    // On the command line: an upload-pack the repository's own config names is held (#1805).
    let racing = crate::worktree::git::Isolated::default().also(
        &format!("remote.{url}.uploadpack"),
        &format!(
            "git -C '{}' update-ref -d refs/heads/{SAVE} && git-upload-pack",
            bare.display()
        ),
    );

    let (got, said) = crate::worktree::git::isolated(&racing, || push_first(&root, url));

    assert!(got.is_ok(), "{got:?} {said}");
    assert_eq!(asked(&root, "ls-remote"), 1, "{said}");
    assert_eq!(asked(&root, "push"), 2, "{said}");
    assert_eq!(
        run(&bare, &["rev-parse", &format!("refs/heads/{SAVE}")]),
        run(&root, &["rev-parse", "HEAD"]),
        "this clone's save is the branch now"
    );
}

/// What a clone keeps when a request is open from `branch`, as its saves left it.
fn kept_open_on(root: &Path, branch: &str) {
    Kept {
        branch: Some(branch.to_string()),
        pushed: Some("a".repeat(40)),
        pr: Some(KeptPr {
            number: 12,
            url: "https://x.invalid/pull/12".into(),
            head: "a".repeat(40),
            target: "main".into(),
            merges_itself: false,
            pushed_at: None,
            merge: None,
        }),
        held: false,
    }
    .write(root);
}

#[test]
fn a_new_clone_saves_to_a_purlis_branch() {
    let (_dir, root, _bare) = plane_and_remote();
    let plane = crate::planesave::Settings::from_text(None, None).plane;
    let save = save_branch(&root, &plane);
    assert_eq!(
        save,
        format!("purlis/{}", crate::planesave::default_rest(&root))
    );
}

#[test]
fn a_request_still_open_from_the_charter_save_branch_carries_on_there_until_it_is_settled() {
    // V93j: the clone opened its request before the rename, from `charter/save/<host>-<clone>`.
    // While it is open, saves go on updating it there rather than opening a second one.
    let (_dir, root, _bare) = plane_and_remote();
    let plane = crate::planesave::Settings::from_text(None, None).plane;
    let old = format!("charter/{}", crate::planesave::default_rest(&root));
    kept_open_on(&root, &old);
    assert_eq!(save_branch(&root, &plane), old);

    // Once it is settled — merged, or closed — what is kept names no request, and the next
    // save goes to the purlis name.
    let mut kept = Kept::read(&root);
    forget_old_branch(&root, &plane, &mut kept);
    kept.write(&root);
    assert_eq!(
        save_branch(&root, &plane),
        format!("purlis/{}", crate::planesave::default_rest(&root))
    );

    // A request forgotten without being seen to merge or close — the plane was moved off its
    // commit by hand — is held, and may still be open: the old branch carries on, so none is
    // opened twice. The hold is kept on disk, and only a settle that saw it end lets it go.
    kept_open_on(&root, &old);
    let mut kept = Kept::read(&root);
    kept.pr = None;
    kept.held = true;
    kept.write(&root);
    assert!(Kept::read(&root).held);
    assert_eq!(save_branch(&root, &plane), old);
    let mut kept = Kept::read(&root);
    forget_old_branch(&root, &plane, &mut kept);
    kept.write(&root);
    assert!(!Kept::read(&root).held);
    assert_eq!(
        save_branch(&root, &plane),
        format!("purlis/{}", crate::planesave::default_rest(&root))
    );
}

#[test]
fn a_clone_upgraded_after_its_last_request_merged_saves_to_the_purlis_name() {
    // What a pre-rename clone keeps once its last request merged: the old branch, the commit it
    // pushed there, and no request. Nothing may still be open, so new work says purlis.
    let (_dir, root, _bare) = plane_and_remote();
    let plane = crate::planesave::Settings::from_text(None, None).plane;
    let rest = crate::planesave::default_rest(&root);
    std::fs::create_dir_all(kept_path(&root).parent().unwrap()).unwrap();
    std::fs::write(
        kept_path(&root),
        serde_json::json!({"branch": format!("charter/{rest}"), "pushed": "a".repeat(40), "pr": null})
            .to_string(),
    )
    .unwrap();
    assert_eq!(save_branch(&root, &plane), format!("purlis/{rest}"));
}

#[test]
fn a_block_about_the_charter_save_branch_still_holds_once_its_request_is_forgotten() {
    // A request closed without merging blocks the plane, and the block names the branch it was
    // on. Forgetting the request moves the next push to the purlis name; the block still holds.
    let (_dir, root, _bare) = plane_and_remote();
    let old = format!("charter/{}", crate::planesave::default_rest(&root));
    let head = run(&root, &["rev-parse", "HEAD"]);
    let rec =
        serde_json::json!({"outcome": "blocked", "branch": "main", "landed": old, "head": head});
    assert!(still_holds(&root, &rec, &head));
    let other = serde_json::json!({"outcome": "blocked", "branch": "main", "landed": "charter/save/elsewhere-000000", "head": head});
    assert!(!still_holds(&root, &other, &head));
}

#[test]
fn only_this_clones_own_default_branch_carries_on_and_a_save_branch_set_by_hand_wins() {
    let (_dir, root, _bare) = plane_and_remote();
    let plane = crate::planesave::Settings::from_text(None, None).plane;
    // Another clone's charter save branch, kept here somehow, is not this clone's.
    kept_open_on(&root, "charter/save/elsewhere-000000");
    assert_eq!(
        save_branch(&root, &plane),
        format!("purlis/{}", crate::planesave::default_rest(&root))
    );
    // A branch named by hand is the branch, whatever is kept.
    kept_open_on(
        &root,
        &format!("charter/{}", crate::planesave::default_rest(&root)),
    );
    let named = crate::planesave::Settings::from_text(
        Some("[plane]\nsave_branch = \"team/saves\"\n"),
        None,
    )
    .plane;
    assert_eq!(save_branch(&root, &named), "team/saves");
}

/// **The fetch after purlis merges its own request is held to the recorded remote** (#1864), as
/// every other fetch is (CI: the sandbox this was written in refuses `git init`): it signs in
/// to the recorded remote, and a remote changed since gets nothing fetched.
#[test]
fn the_fetch_after_purlis_merges_its_request_is_held_to_the_recorded_remote() {
    use crate::forge::gitcredential::{
        FETCH_REMOTE_CHANGED, HELPER_ARG, as_the_window, record_push_remote, tests::Machine,
    };
    use crate::forge::route::HostScope;

    let (_dir, root, _bare) = plane_and_remote();
    let recorded = "https://github.com/acme/plane.git";
    run(&root, &["remote", "add", "origin", recorded]);
    let machine = Machine::signed_in_on("github.com");
    let on = forge::Forge::default_of(forge::Kind::GitHub);
    let in_the_window = |then: &dyn Fn() -> Result<Option<(String, String)>, &'static str>| {
        as_the_window(
            &HostScope::for_a_test(),
            Path::new("/Applications/purlis.app/purlis"),
            machine.config(),
            then,
        )
    };
    in_the_window(&|| {
        record_push_remote(&root, recorded);
        Ok(None)
    })
    .unwrap();

    let (https, helper) = in_the_window(&|| fetch_after_merge(&root, &on))
        .unwrap()
        .expect("a fetch from the recorded remote");
    assert_eq!(https, recorded);
    assert!(helper.contains(HELPER_ARG), "{helper}");

    run(
        &root,
        &[
            "remote",
            "set-url",
            "origin",
            "https://github.com/someone/public.git",
        ],
    );
    assert_eq!(
        in_the_window(&|| fetch_after_merge(&root, &on)),
        Err(FETCH_REMOTE_CHANGED)
    );
}

/// **A commit just pushed is not settled, and one from a minute ago is** (ADR 0051, amended
/// 2026-10-13): no check at a head that new is "not yet", never "none ran".
#[test]
fn a_head_is_settled_a_minute_after_its_push_and_never_at_it() {
    let wait = crate::forge::pr::SETTLE.as_secs();
    let pushed = 1_800_000_000;
    assert!(!settled_at(Some(pushed), pushed));
    assert!(!settled_at(Some(pushed), pushed + wait - 1));
    assert!(settled_at(Some(pushed), pushed + wait));
    // No stamp is no reason to believe it has been there long enough: the forge is the truth,
    // and what is kept on disk is a hint.
    assert!(!settled_at(None, pushed));
    // A clock set back, by a little or by a day, never reads as "long ago" (review N4): the
    // stamp is in the future, so the head is not settled, and the look stamps it again.
    assert!(!settled_at(Some(pushed), pushed - 5));
    assert!(!settled_at(Some(pushed), pushed - wait - 1));
    assert!(!settled_at(Some(pushed), pushed - 86_400));
    assert!(stamp_is_unusable(Some(pushed), pushed - 5));
    assert!(stamp_is_unusable(None, pushed));
    assert!(!stamp_is_unusable(Some(pushed), pushed + 5));
}

/// What is kept about the request survives its file: when its head was pushed, that purlis
/// merges it, and where it stands, with the checks that failed.
#[test]
fn where_a_request_stands_is_kept_beside_it_and_read_back() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let red = Merging::of(&Ok(Ready::Red(vec![
        crate::forge::checks::FailedCheck::of(
            Some("lint"),
            Some("https://github.com/acme/plane/actions/runs/9/job/5"),
        ),
    ])));
    let kept = Kept {
        branch: Some(SAVE.to_string()),
        pushed: Some("a".repeat(40)),
        pr: Some(KeptPr {
            number: 12,
            url: "https://x.invalid/pull/12".into(),
            head: "a".repeat(40),
            target: "main".into(),
            merges_itself: true,
            pushed_at: Some(1_800_000_000),
            merge: Some(red.clone()),
        }),
        held: false,
    };
    kept.write(&root);
    assert_eq!(Kept::read(&root), kept);
    assert_eq!(known_merging(&root), Some(red.clone()));
    assert_eq!(
        open_request(&root),
        Some((Some(1_800_000_000), "a".repeat(40)))
    );

    // Kept again only when it changed, and a request put in a queue is no longer purlis's
    // to merge.
    assert!(!remember(&root, &red, false));
    let queued = Merging::new(MergeState::Queued, "in the merge queue");
    assert!(remember(&root, &queued, true));
    let after = Kept::read(&root).pr.expect("still open");
    assert!(!after.merges_itself);
    assert_eq!(after.merge, Some(queued));

    // A file from before any of this was kept reads as it always did.
    std::fs::write(
        kept_path(&root),
        serde_json::json!({"branch": SAVE, "pushed": "a".repeat(40),
            "pr": {"number": 12, "url": "https://x.invalid/pull/12", "head": "a".repeat(40),
                   "target": "main", "merges_itself": true}})
        .to_string(),
    )
    .unwrap();
    let old = Kept::read(&root).pr.expect("a request");
    assert_eq!(
        (old.pushed_at, old.merge, old.merges_itself),
        (None, None, true)
    );
}

/// A `pr-merge` plane whose saves keep #12 open at `head`, standing as `merge` says, its
/// page nowhere: a forge asked would not answer.
fn kept_for_the_person(merge: Merging, head: &str) -> (tempfile::TempDir, PathBuf, Plane) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    std::fs::write(
        root.join("charter.toml"),
        "[plane]\nmode = \"pr-merge\"\nbranch = \"main\"\n",
    )
    .unwrap();
    let plane = crate::planesave::Settings::read(&root).plane;
    Kept {
        branch: Some(SAVE.to_string()),
        pushed: Some(head.to_string()),
        pr: Some(KeptPr {
            number: 12,
            url: "https://x.invalid/pull/12".into(),
            head: head.to_string(),
            target: "main".into(),
            merges_itself: true,
            pushed_at: Some(1_800_000_000),
            merge: Some(merge),
        }),
        held: false,
    }
    .write(&root);
    (dir, root, plane)
}

/// The person's merge as it stands of `seen`, at the plane `root` keeps.
fn merged_as_shown(
    root: &Path,
    plane: &Plane,
    seen: &super::super::Seen,
) -> Result<MergedAsItStands, String> {
    merge_as_it_stands(
        root,
        plane,
        "main",
        false,
        seen,
        &Caller::window(),
        &mut |_| {},
    )
}

/// **The person's merge is of the commit they were shown, and of no other** (#1978): what
/// the confirmation showed is handed down to the merge, which reads no check, and is read
/// again under its claim. A save that pushed a new commit onto the request after the fresh
/// look is refused there, before the forge is asked anything, and never merged in its place.
#[test]
fn the_merge_as_it_stands_refuses_a_request_at_another_commit_than_the_one_shown() {
    let red = Merging::new(MergeState::Red, "1 check failed: test");
    let (_dir, root, plane) = kept_for_the_person(red.clone(), &"b".repeat(40));
    let seen = super::super::Seen {
        head: "a".repeat(40),
        shown: red.shown(),
    };

    assert_eq!(
        merged_as_shown(&root, &plane, &seen),
        Err(
            "The request changed since you looked: a new commit was pushed onto it, so purlis \
             did not merge it. Look at it again"
                .to_string()
        )
    );
}

/// **Nor past a failure that came after the look** (#1978): the confirmation showed a wait
/// at this commit, and a look in the meantime kept it red at the same commit. The merge's own
/// read under its claim refuses it, saying its checks failed, before the forge is asked.
#[test]
fn the_merge_as_it_stands_refuses_a_request_that_turned_red_after_the_look() {
    let head = "a".repeat(40);
    let red = Merging::new(MergeState::Red, "1 check failed: test");
    let (_dir, root, plane) = kept_for_the_person(red, &head);
    let waited = Merging::new(MergeState::Waiting, "no check has started on it");
    let seen = super::super::Seen {
        head,
        shown: waited.shown(),
    };

    assert_eq!(
        merged_as_shown(&root, &plane, &seen),
        Err(
            "The request changed since you looked (1 check failed: test), so purlis did not \
             merge it. Look at it again"
                .to_string()
        )
    );
}

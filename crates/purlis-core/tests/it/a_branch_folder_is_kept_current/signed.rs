// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! #5480: the commits `purlis worktree update` makes for a chat, by a rebase or a merge, are
//! signed with the app's key, and nothing it did not make is.

use super::*;
use purlis_core::commitsign::Signer;

/// [`update`], as the app runs it for a chat whose person set up `signer`.
fn signed_update(w: &World, piece: &str, step: &UpdateStep, signer: &Signer) -> (u8, String) {
    let mut lines = Vec::new();
    let isolation =
        git::Isolated::identity(Some("Tester"), Some("t@e.invalid")).signing(Some(signer.clone()));
    let code = git::isolated(&isolation, || {
        pieceupdate::update(&w.at(piece), step, &Reach::default(), &mut |l| {
            lines.push(l);
        })
    });
    (code, said(&lines))
}

/// Whether git's own check verifies `id` in `dir` with `signer` for the identity the app
/// commits as: `git verify-commit`, through `ssh-keygen -Y verify`.
fn verified(dir: &Path, id: &str, signer: &Signer) -> bool {
    let allowed = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(
        allowed.path(),
        format!("t@e.invalid namespaces=\"git\" {}\n", signer.public_line()),
    )
    .unwrap();
    purlis_core::forklock::output(
        std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["-c", "gpg.format=ssh", "-c"])
            .arg(format!(
                "gpg.ssh.allowedSignersFile={}",
                allowed.path().display()
            ))
            .args(["verify-commit", id])
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1"),
    )
    .expect("git runs")
    .status
    .success()
}

#[test]
fn a_rebase_signs_each_commit_it_replays_and_leaves_the_bases_alone() {
    purlis_core::unsteered!();
    let w = world();
    let (code, cut) = add(&w, "p1", None);
    assert_eq!(code, 0, "{cut}");
    let piece = w.piece("p1");
    commit_in(&piece, "one.txt", "one\n");
    commit_in(&piece, "two.txt", "two\n");
    w.they_commit("theirs.txt", "theirs\n");
    let main = out(&w.theirs, &["rev-parse", "HEAD"]);
    let signer = Signer::generate().unwrap();

    let (code, said) = signed_update(&w, "p1", &start(None), &signer);

    assert_eq!(code, 0, "{said}");
    assert!(!said.contains("not signed"), "{said}");
    let head = out(&piece, &["rev-parse", "HEAD"]);
    assert_eq!(
        out(&piece, &["branch", "--show-current"]),
        "p1",
        "the folder is on its branch"
    );
    assert_eq!(out(&w.clone, &["rev-parse", "refs/heads/p1"]), head);
    assert_eq!(
        out(&piece, &["rev-parse", "HEAD~2"]),
        main,
        "the base's commit is the one the forge has"
    );
    for id in out(&piece, &["rev-list", "HEAD", "--not", &main]).lines() {
        assert!(verified(&piece, id, &signer), "{id} is verified");
    }
    assert!(!verified(&piece, &main, &signer), "the base's is not ours");
    assert_eq!(out(&piece, &["status", "--porcelain"]), "");
    // And the push publishes the signed commits.
    let (code, said) = push(&w, "p1");
    assert_eq!(code, 0, "{said}");
    assert_eq!(forge_has(&w, "p1"), head);
}

#[test]
fn a_merge_signs_its_merge_commit_and_rewrites_no_commit_already_pushed() {
    purlis_core::unsteered!();
    let w = world();
    let piece = pushed_piece(&w, "p1");
    let mine = out(&piece, &["rev-parse", "HEAD"]);
    w.they_commit("theirs.txt", "theirs\n");
    let main = out(&w.theirs, &["rev-parse", "HEAD"]);
    let signer = Signer::generate().unwrap();

    let merge = UpdateStep::Merge {
        base: None,
        take_base_baselines: false,
    };
    let (code, said) = signed_update(&w, "p1", &merge, &signer);

    assert_eq!(code, 0, "{said}");
    let head = out(&piece, &["rev-parse", "HEAD"]);
    assert_eq!(
        out(&piece, &["log", "-1", "--format=%P"]),
        format!("{mine} {main}"),
        "its parents are the commit already pushed and the base, as they were"
    );
    assert!(verified(&piece, &head, &signer));
    assert_eq!(out(&piece, &["branch", "--show-current"]), "p1");
    assert_eq!(out(&piece, &["status", "--porcelain"]), "");
    let (code, said) = push(&w, "p1");
    assert_eq!(code, 0, "{said}");
}

#[test]
fn with_no_key_a_rebase_is_made_unsigned_and_says_how_to_set_one_up() {
    purlis_core::unsteered!();
    let w = world();
    let (code, cut) = add(&w, "p1", None);
    assert_eq!(code, 0, "{cut}");
    let piece = w.piece("p1");
    commit_in(&piece, "one.txt", "one\n");
    w.they_commit("theirs.txt", "theirs\n");

    let (code, said) = update(&w, "p1", &start(None), &Reach::default());

    assert_eq!(code, 0, "{said}");
    assert!(said.contains("not signed"), "{said}");
    assert!(said.contains("purlis signing setup"), "{said}");
    assert_eq!(out(&piece, &["log", "-1", "--format=%G?"]), "N");
}

#[test]
fn an_update_that_makes_no_commit_says_nothing_of_signing() {
    purlis_core::unsteered!();
    let w = world();
    let (code, cut) = add(&w, "p1", None);
    assert_eq!(code, 0, "{cut}");
    w.they_commit("theirs.txt", "theirs\n");

    // A fast-forward: the base's commits, made by someone else, are not the app's to sign.
    let (code, said) = update(&w, "p1", &start(None), &Reach::default());

    assert_eq!(code, 0, "{said}");
    assert!(!said.contains("signed"), "{said}");
}

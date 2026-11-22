// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat's own branch is one the app recorded it creating through the forge route**, and
//! nothing a chat can write: not the piece log (a hand-written `claimed` line, or taking a free
//! folder, is a claim on a folder and no more) and not a clone's refs (a remote ref can be deleted
//! or never fetched). The cut also refuses a name a remote has, as a courtesy.

use crate::support;

use std::path::Path;

use purlis_core::forge::createdbranches;
use purlis_core::piececlaim::{self, Act, Live, Writer};
use purlis_core::pieces::Who;
use purlis_core::worktree::{self, Located, Refusal};

const CHAT: &str = "01J9ZQ3V5N8X4T2K7M6P0R1S2A";
const OTHER: &str = "01J9ZQ3V5N8X4T2K7M6P0R1S2B";
const HOST: &str = "github.com";
const REPO: &str = "acme/app";

fn who(id: &str) -> Who {
    Who {
        session: Some("7".into()),
        persona: Some("builder".into()),
        chat: Some(id.to_owned()),
        host: "box".into(),
        log: "box".into(),
    }
}

/// The clone has seen `branch` on `origin`, as a fetch leaves it.
fn remote_has(f: &support::Fixture, branch: &str) {
    support::git(
        &f.clone,
        &["remote", "add", "origin", "https://example.invalid/x.git"],
    );
    support::git(
        &f.clone,
        &[
            "update-ref",
            &format!("refs/remotes/origin/{branch}"),
            "HEAD",
        ],
    );
}

#[test]
fn a_cut_may_not_take_the_name_of_a_branch_a_remote_has() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    remote_has(&f, "phase-1-proven");

    let refused = worktree::add(&f.plane, &f.ws, &f.repo, "mine", Some("phase-1-proven"));
    assert!(
        matches!(refused, Err(Refusal::BranchTaken { ref branch, .. }) if branch == "phase-1-proven"),
        "{refused:?}"
    );

    // The piece's own name is read the same way, and another name is cut.
    let piece = worktree::add(&f.plane, &f.ws, &f.repo, "phase-1-proven", None);
    assert!(
        matches!(piece, Err(Refusal::BranchTaken { .. })),
        "{piece:?}"
    );
    worktree::add(&f.plane, &f.ws, &f.repo, "mine", Some("task/mine")).unwrap();
}

fn own(config: &Path, chat: &str) -> Vec<String> {
    createdbranches::created_by(config, chat, HOST, REPO)
}

fn located(f: &support::Fixture, piece: &str) -> Located {
    Located {
        workspace: f.ws.clone(),
        repo: f.repo.clone(),
        piece: piece.to_owned(),
    }
}

#[test]
fn a_hand_written_claim_line_makes_a_chat_the_folders_holder_and_no_branch_its_own() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    let config = tempfile::tempdir().unwrap();
    worktree::add(&f.plane, &f.ws, &f.repo, "task-one", Some("phase-1-proven")).unwrap();
    // A chat writes a `claimed` line for itself with a ts far ahead, into the log it can reach.
    let log = purlis_core::pieces::log_path(&f.plane, &f.ws, "box");
    std::fs::create_dir_all(log.parent().unwrap()).unwrap();
    let line = serde_json::json!({
        "event": "claimed", "piece": "task-one", "repo": f.repo, "chat": CHAT,
        "session": "7", "persona": "builder", "ts": "2999-01-01T00:00:00Z",
    });
    std::fs::write(&log, format!("{line}\n")).unwrap();

    // The claim does what a claim does: the chat holds the folder...
    let writer = piececlaim::writer(
        &f.plane,
        &located(&f, "task-one"),
        Some(CHAT),
        None,
        Act::Commit,
    );
    assert_eq!(writer, Writer::Yours);
    // ...and that makes no branch its own.
    assert!(own(config.path(), CHAT).is_empty());
}

#[test]
fn taking_a_free_folder_on_an_existing_branch_makes_it_no_ones_own() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    let config = tempfile::tempdir().unwrap();
    worktree::add(&f.plane, &f.ws, &f.repo, "task-one", Some("task/one")).unwrap();
    let live = [Live {
        id: CHAT.to_owned(),
        chat: Some(7),
        back: true,
    }];
    piececlaim::take(
        &f.plane,
        &located(&f, "task-one"),
        &who(CHAT),
        Some(&live),
        Act::Commit,
        jiff::Timestamp::now(),
    )
    .unwrap();

    assert!(own(config.path(), CHAT).is_empty());
}

#[test]
fn a_remote_ref_that_is_deleted_or_never_fetched_lets_the_name_be_cut_and_still_not_be_owned() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    let config = tempfile::tempdir().unwrap();
    remote_has(&f, "m75-policy");
    assert!(worktree::add(&f.plane, &f.ws, &f.repo, "spoof", Some("m75-policy")).is_err());

    // The chat deletes the ref the cut check reads (or it was never fetched), and cuts the name.
    support::git(
        &f.clone,
        &["update-ref", "-d", "refs/remotes/origin/m75-policy"],
    );
    worktree::add(&f.plane, &f.ws, &f.repo, "spoof", Some("m75-policy")).unwrap();

    assert!(own(config.path(), CHAT).is_empty());
}

#[test]
fn a_branch_recorded_as_created_is_its_creating_chats_own_and_no_other_chats() {
    let config = tempfile::tempdir().unwrap();
    createdbranches::record(
        config.path(),
        CHAT,
        &[(HOST.to_owned(), REPO.to_owned(), "task/one".to_owned())],
    )
    .unwrap();

    assert_eq!(own(config.path(), CHAT), ["task/one"]);
    // A resumed chat has another id: it owns none, and may fast-forward the branch only.
    assert!(own(config.path(), OTHER).is_empty());
}

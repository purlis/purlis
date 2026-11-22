// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat's own branch is one purlis cut for it**, never a name it picked that the forge had
//! already: the forge route lets a chat push its own branches although they exist on the forge,
//! so a cut may not take the name of a branch a remote has, and what the route enrols is read
//! from the real piece log and the real clone.

use crate::support;

use purlis_core::piececlaim::{self, Act, Live};
use purlis_core::pieces::Who;
use purlis_core::worktree::{self, Located, Refusal};

const CHAT: &str = "01J9ZQ3V5N8X4T2K7M6P0R1S2A";
const OTHER: &str = "01J9ZQ3V5N8X4T2K7M6P0R1S2B";

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

fn claim(f: &support::Fixture, piece: &str, id: &str) {
    let at = Located {
        workspace: f.ws.clone(),
        repo: f.repo.clone(),
        piece: piece.to_owned(),
    };
    let live = [Live {
        id: id.to_owned(),
        chat: Some(7),
        back: true,
    }];
    piececlaim::take(
        &f.plane,
        &at,
        &who(id),
        Some(&live),
        Act::Commit,
        jiff::Timestamp::now(),
    )
    .unwrap();
}

fn own(f: &support::Fixture, chat: &str) -> Vec<String> {
    let clone = std::fs::canonicalize(&f.clone).unwrap();
    piececlaim::branches_held_in_clones(&f.plane, &[clone], chat)
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

#[test]
fn a_branch_a_remote_has_is_in_no_chats_own_branches() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    remote_has(&f, "m75-policy");
    // The chat gets what it asked for only under a name the remote lacks.
    assert!(worktree::add(&f.plane, &f.ws, &f.repo, "spoof", Some("m75-policy")).is_err());
    assert!(own(&f, CHAT).is_empty());
}

#[test]
fn the_branches_enrolled_are_those_of_the_folders_the_chats_claim_names() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    let cut = worktree::add(&f.plane, &f.ws, &f.repo, "task-one", Some("task/one")).unwrap();
    assert_eq!(cut.branch, "task/one");
    worktree::add(&f.plane, &f.ws, &f.repo, "task-two", Some("task/two")).unwrap();
    claim(&f, "task-one", CHAT);

    // The chat id the route is started with (`identity.id`) is the one the claim names.
    assert_eq!(own(&f, CHAT), vec!["task/one".to_owned()]);
    assert!(own(&f, OTHER).is_empty(), "another chat holds none of them");
}

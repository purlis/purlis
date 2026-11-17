// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A write to the default branch's ref through the API is held as a merge is (Q330).
//!
//! `gh api -X PATCH repos/o/r/git/refs/heads/main -f sha=…` moves `main` to any commit: it lands
//! code with no request and no press, which is what the merge hold is for. A `PATCH`, `POST` or
//! `DELETE` on the default branch's ref is held in every mode, and refused unattended by the
//! floor. The floor reads no forge, so the default branch is `main` or `master`, and a branch
//! it cannot read is treated as the default. A feature branch's ref is left alone.

use purlis_core::floorguard::release_floor_reason;
use purlis_core::mergehold::{self, Held};

/// Writes to the default branch's ref, or to a ref whose branch purlis cannot read.
const HELD: &[&str] = &[
    "gh api -X PATCH repos/o/r/git/refs/heads/main -f sha=abc123",
    "gh api -X PATCH repos/o/r/git/refs/heads/main -f sha=abc123 -F force=true",
    "gh api --method PATCH repos/o/r/git/refs/heads/master -f sha=abc123",
    "gh api -X DELETE repos/o/r/git/refs/heads/main",
    "gh api -X PATCH /repos/o/r/git/refs/heads/main -f sha=abc123",
    "gh api -X PATCH https://ghe.example.com/api/v3/repos/o/r/git/refs/heads/main -f sha=abc",
    "gh api -X PATCH repos/o/r/git/refs/heads%2Fmain -f sha=abc123",
    "gh api -X PATCH repositories/42/git/refs/heads/main -f sha=abc123",
    "gh api -X POST repos/o/r/git/refs -f ref=refs/heads/main -f sha=abc123",
    "gh api repos/o/r/git/refs -f ref=refs/heads/master -f sha=abc123",
    "gh api -X POST repos/o/r/git/refs -f ref=$REF -f sha=abc123",
    "gh api -X POST repos/o/r/git/refs -f sha=abc123",
    "gh api -X POST repos/o/r/git/refs --input body.json",
    "gh api -X PATCH repos/o/r/git/refs/heads/$B -f sha=abc123",
    "glab api -X PATCH projects/1/git/refs/heads/main -f sha=abc123",
    "x=$(gh api -X PATCH repos/o/r/git/refs/heads/main -f sha=abc123 | head -1)",
];

#[test]
fn a_write_to_the_default_branch_s_ref_is_held_as_a_merge() {
    purlis_core::unsteered!();
    for cmd in HELD {
        assert_eq!(mergehold::held(cmd), Some(Held::Unreadable), "{cmd}");
        assert!(
            release_floor_reason(cmd, true).is_some(),
            "not floored: {cmd}"
        );
        assert_eq!(release_floor_reason(cmd, false), None, "attended: {cmd}");
    }
}

#[test]
fn a_feature_branch_s_ref_and_a_read_of_the_default_branch_s_pass() {
    purlis_core::unsteered!();
    for cmd in [
        "gh api -X PATCH repos/o/r/git/refs/heads/feat/x -f sha=abc123",
        "gh api -X PATCH repos/o/r/git/refs/heads/main-old -f sha=abc123",
        "gh api -X DELETE repos/o/r/git/refs/heads/7124-fix",
        "gh api -X POST repos/o/r/git/refs -f ref=refs/heads/feat -f sha=abc123",
        "gh api repos/o/r/git/refs/heads/main",
        "gh api repos/o/r/git/refs/heads/main --jq .object.sha",
        "gh api -X GET repos/o/r/git/refs/heads/main",
    ] {
        assert_eq!(mergehold::held(cmd), None, "held: {cmd}");
        assert_eq!(release_floor_reason(cmd, true), None, "floored: {cmd}");
    }
}

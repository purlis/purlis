// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A `gh api` / `glab api` call whose arguments the shell fills in is held, read or not (#7416).
//! An unquoted expansion is word-split, and a word that begins with an expansion may be a flag,
//! so `gh api repos/o/r/pulls/5/merge $(cat n)` merges when `n` holds `-XPUT`, and a merge
//! request's title or a user's bio can hold it. Quoted after a literal start, an expansion is
//! one word that is no flag, and a variable the line binds to digits is digits.

use purlis_core::floorguard::{merge_reason, release_floor_reason};
use purlis_core::mergehold;

/// Each is held by the hold, and refused by the floor's merge half and the release floor.
fn held(cmds: &[&str]) {
    for cmd in cmds {
        assert!(mergehold::held(cmd).is_some(), "not held: {cmd}");
        assert!(
            merge_reason(cmd).is_some(),
            "not a merge to the floor: {cmd}"
        );
        assert!(
            release_floor_reason(cmd, true).is_some(),
            "not floored: {cmd}"
        );
    }
}

/// Each passes the hold and the floor's merge half.
fn passes(cmds: &[&str]) {
    for cmd in cmds {
        assert_eq!(mergehold::held(cmd), None, "held: {cmd}");
        assert_eq!(merge_reason(cmd), None, "a merge to the floor: {cmd}");
    }
}

#[test]
fn the_shapes_in_the_issue_are_held() {
    purlis_core::unsteered!();
    held(&[
        "gh api repos/o/r/pulls/5/merge $(gh api user --jq '\"-XPUT\"')",
        "gh api repos/o/r/pulls/5/merge $(cat n)",
        "gh api repos/o/r/pulls/5/merge $(git config x.y)",
        "gh api repos/o/r/pulls/5/merge $(tr a b < n)",
        "gh api repos/o/r/pulls/5/merge $F",
        "F=$(cat n); gh api repos/o/r/pulls/5/merge $F",
        "gh api repos/o/r/pulls/5/merge \"$(cat n)\"",
        "gh api repos/o/r/pulls/5/merge \"$F\"",
        "gh api repos/o/r/pulls/5/merge `cat n`",
        "gh api repos/o/r/pulls/5/merge \"`cat n`\"",
    ]);
}

#[test]
fn the_table_in_the_review_of_7414_is_held() {
    purlis_core::unsteered!();
    held(&[
        "gh api repos/o/r/pulls/5/merge $(gh api user | jq -r .bio)",
        "glab api projects/:id/merge_requests/5/merge $(glab api user | jq -r .bio)",
        "glab api projects/:id/merge_requests/$(glab api projects/:id/merge_requests | jq -r '.[0].title')/notes",
        "gh api $(gh api repos/o/r/pulls/5 | jq -r .title)",
        "gh api repos/o/r/git/refs/heads/$(gh api user | jq -r .bio)",
        "gh api repos/o/r/pulls/5/merge $(gh api user | tail -1)",
    ]);
}

#[test]
fn an_expansion_that_may_add_a_word_or_begin_one_is_held_in_any_position() {
    purlis_core::unsteered!();
    held(&[
        "gh api repos/o/r/pulls/$N",
        "gh api repos/o/r/pulls/5 -f title=$T",
        "gh api repos/o/r/pulls/5 \"$M\"",
        "gh api -X \"$M\" repos/o/r/pulls/5/merge",
        "gh api repos/o/r/pulls/5/merge \"-X$M\"",
        "gh api repos/o/r/pulls/5/merge -\"$M\"",
        "gh api repos/o/r/pulls/5/merge \"\"$F",
        "gh api repos/o/r/pulls/5/merge $@",
        "glab api -R g/p projects/$ID/jobs",
        "id=$(gh api repos/o/r/pulls/$N | jq -r .id)",
        "bash -c 'gh api repos/o/r/pulls/5/merge $F'",
    ]);
}

#[test]
fn the_refusal_says_the_way_through_and_no_way_round() {
    purlis_core::unsteered!();
    let said =
        release_floor_reason("gh api repos/o/r/pulls/5/merge $(cat n)", true).expect("a refusal");
    // The release floor's own opening names `bypassPermissions`; what is said of this call is
    // what follows it.
    let said = said
        .split("treated as one.")
        .nth(1)
        .expect("the call's own part");
    assert!(said.contains("`$(…)`"), "{said}");
    assert!(said.contains("double quotes"), "{said}");
    assert!(said.contains("literal"), "{said}");
    assert!(said.contains("Write the endpoint out"), "{said}");
    for round in ["bypass", "disable", "eval", "unset"] {
        assert!(!said.contains(round), "{round}: {said}");
    }
}

#[test]
fn a_quoted_expansion_after_a_literal_start_that_is_no_flag_passes() {
    purlis_core::unsteered!();
    passes(&[
        "gh api \"repos/o/r/pulls/$N\"",
        "glab api \"projects/:id/pipelines/$(glab api projects/:id/pipelines | jq -r '.[0].id')/jobs\"",
        "gh api repos/o/r/pulls/5 -f title=\"$T\"",
        "gh api repos/o/r/issues/5/comments -f body=\"$B\" --jq .id",
        "gh api 'repos/o/r/pulls/$N'",
        "gh api repos/o/r/pulls/5 -H 'Accept: application/json'",
    ]);
}

#[test]
fn plain_literal_reads_and_digit_bound_variables_pass() {
    purlis_core::unsteered!();
    passes(&[
        "gh api repos/o/r/pulls/5",
        "gh api repos/{owner}/{repo}/pulls --jq '.[].number'",
        "glab api projects/:id/merge_requests/5 | jq -r .title",
        "for id in 1 2; do gh api repos/o/r/pulls/$id; done",
        "N=5; gh api repos/o/r/pulls/$N",
        "gh api repos/o/r/pulls/5 > $TMPDIR/pr.json",
    ]);
}

#[test]
fn a_variable_the_line_does_not_bind_to_digits_is_held_even_in_a_loop() {
    purlis_core::unsteered!();
    held(&[
        "for id in 1 $X; do gh api repos/o/r/pulls/$id; done",
        "for id in $(cat ids); do gh api repos/o/r/pulls/$id; done",
        "N=$(cat n); gh api repos/o/r/pulls/$N",
    ]);
}

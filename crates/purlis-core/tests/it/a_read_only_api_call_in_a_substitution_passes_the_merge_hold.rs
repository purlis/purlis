// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A read-only `gh api` / `glab api` call passes the merge hold wherever it stands, the value of
//! a variable a substitution sets included, and a merge through the API is held there as
//! anywhere (#7092).
//!
//! A chat watching CI ran `id=$(gh api "repos/o/r/actions/runs?head_sha=$H" --jq … | head -1)`
//! and the hold refused it as a merge it could not read. The floor read every segment with each
//! substitution's words handed out to it, so the assignment was also `id=$ gh api … head -1`: a
//! `gh api` call with `head`'s `-1` among its flags, which could not be shown to be a `GET`,
//! to an endpoint the shell fills in. An assignment runs nothing but its substitution, whose
//! commands the floor reads on their own, so it is no longer read outward.

use purlis_core::floorguard::{merge_reason, release_floor_reason};
use purlis_core::mergehold;

/// The line the hold refused on 2026-11-17, as the chat ran it, its worktree's path shortened.
const THE_REFUSED_LINE: &str = r#"H=$(cd /tmp/wt && git rev-parse HEAD); echo $H; until [ "$(gh api "repos/purlis/purlis/actions/runs?head_sha=$H" --jq '[.workflow_runs[] | select(.name=="sandbox scenarios")] | length' 2>/dev/null)" != "0" ]; do sleep 30; done; id=$(gh api "repos/purlis/purlis/actions/runs?head_sha=$H" --jq '.workflow_runs[] | select(.name=="sandbox scenarios") | .id' | head -1); echo "scen=$id"; until [ "$(gh run view $id -R purlis/purlis --json jobs --jq '[.jobs[] | select(.name=="sandbox-scenarios-linux") | select(.status=="completed")] | length')" = "1" ]; do sleep 90; done; j=$(gh run view $id -R purlis/purlis --json jobs --jq '.jobs[] | select(.name=="sandbox-scenarios-linux") | .databaseId'); gh api "repos/purlis/purlis/actions/jobs/$j/logs" 2>/dev/null | cut -c30- | sed 's/\x1b\[[0-9;]*m//g' > $TMPDIR/m2.log; grep -E "^\s+FAIL \[|Summary \[" $TMPDIR/m2.log | awk '{print $1,$NF}' | sort -u; grep -E -A1 "panicked at app" $TMPDIR/m2.log | grep -vE "panicked|^--" | sort -u | cut -c1-200 | head -10"#;

/// Each passes the hold and the floor, unattended.
fn passes(cmds: &[&str]) {
    for cmd in cmds {
        assert_eq!(mergehold::held(cmd), None, "held: {cmd}");
        assert_eq!(merge_reason(cmd), None, "a merge to the floor: {cmd}");
        assert_eq!(release_floor_reason(cmd, true), None, "floored: {cmd}");
    }
}

/// Each is held, and refused unattended.
fn held(cmds: &[&str]) {
    for cmd in cmds {
        assert!(mergehold::held(cmd).is_some(), "not held: {cmd}");
        assert!(
            release_floor_reason(cmd, true).is_some(),
            "not floored: {cmd}"
        );
    }
}

#[test]
fn the_ci_watch_the_hold_refused_passes_it() {
    purlis_core::unsteered!();
    passes(&[THE_REFUSED_LINE]);
}

#[test]
fn a_read_only_api_path_with_a_query_string_passes_the_hold() {
    purlis_core::unsteered!();
    passes(&[
        "gh api 'repos/o/r/actions/runs?head_sha=abc&per_page=5'",
        "id=$(gh api \"repos/o/r/actions/runs?head_sha=$H\" | head -1)",
        "id=$(gh api repos/o/r/actions/runs?head_sha=abc | head -n 1)",
        "glab api \"projects/:id/pipelines?sha=$H\" | jq -r '.[0].id'",
        "id=$(glab api \"projects/:id/pipelines?sha=$H\" | jq -r '.[0].id')",
    ]);
}

#[test]
fn a_read_only_api_call_with_jq_passes_the_hold() {
    purlis_core::unsteered!();
    passes(&[
        "gh api 'repos/o/r/actions/runs?head_sha=abc' --jq '.workflow_runs[] | .name'",
        "id=$(gh api \"repos/o/r/actions/runs?head_sha=$H\" --jq '.workflow_runs[] | .id' | head -1)",
        "id=`gh api \"repos/o/r/actions/runs?head_sha=$H\" -q '.workflow_runs[0].id' | head -1`",
        "n=$(gh api \"repos/o/r/actions/runs?head_sha=$H\" --jq '.total_count' 2>/dev/null | tr -d ' ')",
    ]);
}

#[test]
fn a_read_only_api_call_with_x_get_passes_the_hold() {
    purlis_core::unsteered!();
    passes(&[
        "gh api -X GET 'repos/o/r/actions/runs?head_sha=abc'",
        "id=$(gh api -X GET repos/o/r/actions/runs -f head_sha=\"$H\" --jq '.workflow_runs[0].id' | head -1)",
        "id=$(gh api --method GET \"repos/o/r/pulls/$N/merge\" | head -1)",
    ]);
}

#[test]
fn a_read_only_api_call_with_paginate_passes_the_hold() {
    purlis_core::unsteered!();
    passes(&[
        "gh api --paginate 'repos/o/r/actions/runs?head_sha=abc' --jq '.workflow_runs[].name'",
        "names=$(gh api --paginate \"repos/o/r/actions/runs?head_sha=$H\" --jq '.workflow_runs[].name' | sort -u)",
    ]);
}

/// A merge through the API is held standing as the value of a variable as it is standing alone,
/// each route the issue names.
#[test]
fn a_merge_through_the_api_stays_held_in_a_substitution() {
    purlis_core::unsteered!();
    held(&[
        "gh api -X PUT repos/o/r/pulls/12/merge",
        "id=$(gh api -X PUT repos/o/r/pulls/12/merge | head -1)",
        "id=$(gh api -X PUT \"repos/o/r/pulls/$N/merge\" --jq .sha)",
        "gh api -X POST repos/o/r/merges -f base=main -f head=feature",
        "sha=$(gh api -X POST repos/o/r/merges -f base=main -f head=feature --jq .sha | head -1)",
        "glab api -X PUT projects/:id/merge_requests/5/merge",
        "id=$(glab api -X PUT projects/:id/merge_requests/5/merge | jq .iid)",
        "gh api graphql -f query='mutation { mergePullRequest(input: {pullRequestId: \"x\"}) { clientMutationId } }'",
        "r=$(gh api graphql -f query='mutation { mergePullRequest(input: {pullRequestId: \"x\"}) { clientMutationId } }' | head -1)",
        "gh api graphql -f query='mutation { enqueuePullRequest(input: {pullRequestId: \"x\"}) { clientMutationId } }'",
        "r=$(gh api graphql -f query='mutation { enqueuePullRequest(input: {pullRequestId: \"x\"}) { clientMutationId } }' | head -1)",
        // A write whose method or endpoint the shell fills in is still read as one that may merge.
        "id=$(gh api --method \"$M\" repos/o/r/pulls/12/merge | head -1)",
        "id=$(gh api \"repos/$R/pulls/12/merge\" -f sha=abc | head -1)",
    ]);
}

/// The floor's reading of a release through the API does not loosen, standing alone or as the
/// value of a variable.
#[test]
fn a_release_through_the_api_is_still_refused_unattended() {
    purlis_core::unsteered!();
    for cmd in [
        "gh api -X POST repos/o/r/releases",
        "gh api -X POST repos/o/r/releases -f tag_name=v1",
        "id=$(gh api -X POST repos/o/r/releases -f tag_name=v1 --jq .id | head -1)",
        "gh api -X POST repos/o/r/git/refs -f ref=refs/tags/v1 -f sha=abc",
        "r=$(gh api -X POST repos/o/r/git/refs -f ref=refs/tags/v1 -f sha=abc | head -1)",
    ] {
        assert!(
            release_floor_reason(cmd, true).is_some(),
            "not floored: {cmd}"
        );
        assert_eq!(release_floor_reason(cmd, false), None, "attended: {cmd}");
    }
}

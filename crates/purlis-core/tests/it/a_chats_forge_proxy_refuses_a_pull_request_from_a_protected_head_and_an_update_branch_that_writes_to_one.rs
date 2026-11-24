// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! #7494: a pull request whose head is the default branch (or a protected one) and whose base is
//! the chat's feature branch lets `updatePullRequestBranch` / `PUT …/update-branch` merge the
//! feature branch into the default branch on the forge's side, past the merge guard and the push
//! refusal. So a pull request is not opened from a protected head, by GraphQL or REST, and
//! update-branch is forwarded only when the forge says the pull request's head is an unprotected
//! branch of the chat's own repo: a head in a fork, and a head nobody can place, are refused too.

use purlis_core::forge::proxy::{Outcome, Refusal};

use crate::support::forge_proxy::{ObjectsWithHeads, Scene, graphql, is_refused};

const CREATE: &str = "mutation PullRequestCreate($input: CreatePullRequestInput!) { \
    createPullRequest(input: $input) { pullRequest { id url } } }";

const UPDATE_BRANCH: &str = "mutation PullRequestUpdateBranch($input: UpdatePullRequestBranchInput!) { \
    updatePullRequestBranch(input: $input) { pullRequest { id } } }";

/// chat-a's repo is `Acme/App` (default `main`, `release` protected). Pull requests of it, by node
/// id and number: 7 from a feature branch, 8 from `main`, 9 from a fork's branch, 10 from
/// `release`, 11 from a branch of the repo that has a name `Main` in another case. A pull request
/// 12 exists but the forge cannot say its head.
fn scene() -> Scene {
    let mut objects = ObjectsWithHeads::default()
        .pull("acme/app", "PR_feature", 7, "acme/app", "fix")
        .pull("acme/app", "PR_main", 8, "acme/app", "main")
        .pull("acme/app", "PR_fork", 9, "someone/app", "fix")
        .pull("acme/app", "PR_release", 10, "acme/app", "release")
        .pull("acme/app", "PR_cased", 11, "acme/app", "Main");
    objects
        .objects
        .0
        .insert("PR_nohead".into(), "acme/app".into());
    Scene::tuned(objects, |proxy| proxy)
}

fn gql(s: &Scene, query: &str, variables: serde_json::Value) -> Outcome {
    s.send(
        "POST",
        "api.github.com",
        "/graphql",
        &graphql(query, variables),
    )
}

fn create(s: &Scene, head: &str, base: &str) -> Outcome {
    gql(
        s,
        CREATE,
        serde_json::json!({"input": {
            "repositoryId": "R_acme_app", "baseRefName": base, "headRefName": head,
            "title": "t", "body": "b"}}),
    )
}

fn update(s: &Scene, id: &str) -> Outcome {
    gql(
        s,
        UPDATE_BRANCH,
        serde_json::json!({"input": {"pullRequestId": id, "expectedHeadOid": "a".repeat(40)}}),
    )
}

fn rest_create(s: &Scene, head: &str, base: &str) -> Outcome {
    let body = serde_json::json!({"title": "t", "head": head, "base": base});
    s.send(
        "POST",
        "api.github.com",
        "/repos/Acme/App/pulls",
        body.to_string().as_bytes(),
    )
}

fn rest_update(s: &Scene, number: u64) -> Outcome {
    s.send(
        "PUT",
        "api.github.com",
        &format!("/repos/Acme/App/pulls/{number}/update-branch"),
        b"",
    )
}

fn forwarded(out: &Outcome) -> bool {
    matches!(out, Outcome::Forward(_))
}

#[test]
fn a_graphql_pull_request_from_the_default_branch_into_a_feature_branch_is_refused() {
    purlis_core::unsteered!();
    let s = scene();
    for head in [
        "main",
        "Main",
        "refs/heads/main",
        "acme:main",
        "release",
        "release/1.2",
        "refs/tags/v1",
    ] {
        let out = create(&s, head, "fix");
        assert!(
            is_refused(&out, Refusal::ProtectedPullHead),
            "{head}: {out:?}"
        );
    }
}

#[test]
fn a_graphql_pull_request_from_a_feature_branch_into_the_default_branch_is_still_answered() {
    purlis_core::unsteered!();
    let s = scene();
    for head in ["fix", "acme:fix", "refs/heads/fix"] {
        let out = create(&s, head, "main");
        assert!(forwarded(&out), "{head}: {out:?}");
    }
}

#[test]
fn a_rest_pull_request_from_the_default_branch_into_a_feature_branch_is_refused() {
    purlis_core::unsteered!();
    let s = scene();
    for head in [
        "main",
        "Main",
        "refs/heads/main",
        "acme:main",
        "release",
        "refs/tags/v1",
    ] {
        let out = rest_create(&s, head, "fix");
        assert!(
            is_refused(&out, Refusal::ProtectedPullHead),
            "{head}: {out:?}"
        );
    }
}

#[test]
fn a_rest_pull_request_from_a_feature_branch_is_still_answered() {
    purlis_core::unsteered!();
    let s = scene();
    for head in ["fix", "acme:fix"] {
        let out = rest_create(&s, head, "main");
        assert!(forwarded(&out), "{head}: {out:?}");
    }
}

#[test]
fn a_graphql_update_branch_on_a_pull_request_whose_head_is_protected_is_refused() {
    purlis_core::unsteered!();
    let s = scene();
    for id in ["PR_main", "PR_release", "PR_cased"] {
        let out = update(&s, id);
        assert!(
            is_refused(&out, Refusal::ProtectedPullHead),
            "{id}: {out:?}"
        );
    }
    // An alias does not hide it.
    let aliased = "mutation($input: UpdatePullRequestBranchInput!) { \
        a: updatePullRequestBranch(input: $input) { pullRequest { id } } }";
    let out = gql(
        &s,
        aliased,
        serde_json::json!({"input": {"pullRequestId": "PR_main"}}),
    );
    assert!(is_refused(&out, Refusal::ProtectedPullHead), "{out:?}");
    // Nor does a second, harmless mutation in the same document.
    let two = "mutation($a: UpdatePullRequestBranchInput!, $b: UpdatePullRequestBranchInput!) { \
        ok: updatePullRequestBranch(input: $a) { pullRequest { id } } \
        bad: updatePullRequestBranch(input: $b) { pullRequest { id } } }";
    let out = gql(
        &s,
        two,
        serde_json::json!({"a": {"pullRequestId": "PR_feature"}, "b": {"pullRequestId": "PR_main"}}),
    );
    assert!(is_refused(&out, Refusal::ProtectedPullHead), "{out:?}");
}

#[test]
fn a_graphql_update_branch_on_a_pull_request_whose_head_is_in_a_fork_is_refused() {
    purlis_core::unsteered!();
    let out = update(&scene(), "PR_fork");
    assert!(is_refused(&out, Refusal::ProtectedPullHead), "{out:?}");
}

#[test]
fn a_graphql_update_branch_whose_head_cannot_be_determined_is_refused() {
    purlis_core::unsteered!();
    let s = scene();
    let out = update(&s, "PR_nohead");
    assert!(is_refused(&out, Refusal::ProtectedPullHead), "{out:?}");
    // No pull request named at all.
    let out = gql(
        &s,
        UPDATE_BRANCH,
        serde_json::json!({"input": {"repositoryId": "R_acme_app"}}),
    );
    assert!(is_refused(&out, Refusal::ProtectedPullHead), "{out:?}");
}

#[test]
fn a_graphql_update_branch_on_a_feature_branch_pull_request_is_still_answered() {
    purlis_core::unsteered!();
    let out = update(&scene(), "PR_feature");
    assert!(forwarded(&out), "{out:?}");
}

#[test]
fn a_rest_update_branch_on_a_pull_request_whose_head_is_protected_is_refused() {
    purlis_core::unsteered!();
    let s = scene();
    for number in [8, 10, 11] {
        let out = rest_update(&s, number);
        assert!(
            is_refused(&out, Refusal::ProtectedPullHead),
            "#{number}: {out:?}"
        );
    }
}

#[test]
fn a_rest_update_branch_on_a_pull_request_whose_head_is_in_a_fork_is_refused() {
    purlis_core::unsteered!();
    let out = rest_update(&scene(), 9);
    assert!(is_refused(&out, Refusal::ProtectedPullHead), "{out:?}");
}

#[test]
fn a_rest_update_branch_whose_head_cannot_be_determined_is_refused() {
    purlis_core::unsteered!();
    // 12 is a pull request the forge cannot place; a resolver that implements no lookup knows none.
    let out = rest_update(&scene(), 12);
    assert!(is_refused(&out, Refusal::ProtectedPullHead), "{out:?}");
    let blind = Scene::new();
    let out = rest_update(&blind, 7);
    assert!(is_refused(&out, Refusal::ProtectedPullHead), "{out:?}");
}

#[test]
fn a_rest_update_branch_on_a_feature_branch_pull_request_is_still_answered() {
    purlis_core::unsteered!();
    let out = rest_update(&scene(), 7);
    assert!(forwarded(&out), "{out:?}");
}

#[test]
fn the_refusal_says_what_to_do_instead() {
    purlis_core::unsteered!();
    let said = Refusal::ProtectedPullHead.advice().expect("advice");
    assert!(
        said.contains("from your feature branch into the default branch"),
        "{said}"
    );
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! One login, slice A: a chat never merges, whatever the person's token could do (ADR 0128 D2).
//!
//! The merge is the person's press in the window. Every route to it is refused by name, so the
//! refusal says why: REST and GraphQL, GitHub and GitLab, auto-merge and merge when the pipeline
//! succeeds, a branch merge through the API, and an approval that would stand for one.

use purlis_core::forge::proxy::{Outcome, Refusal};

use crate::support::forge_proxy::{Objects, Scene, graphql, is_refused};
use purlis_core::forge::proxy::TokenKind;

#[test]
fn a_rest_merge_is_refused_on_every_forge() {
    let s = Scene::new();
    for (method, host, target) in [
        ("PUT", "api.github.com", "/repos/acme/app/pulls/7/merge"),
        ("POST", "api.github.com", "/repos/acme/app/merges"),
        (
            "PUT",
            "ghe.corp.example",
            "/api/v3/repos/acme/app/pulls/7/merge",
        ),
        (
            "PUT",
            "gitlab.com",
            "/api/v4/projects/group%2Fproj/merge_requests/3/merge",
        ),
        (
            "PUT",
            "gitlab.com",
            "/api/v4/projects/42/merge_requests/3/merge?merge_when_pipeline_succeeds=true",
        ),
        (
            "POST",
            "gitlab.com",
            "/api/v4/projects/42/merge_requests/3/cancel_merge_when_pipeline_succeeds",
        ),
        (
            "PUT",
            "gitlab.com",
            "/api/v4/projects/42/merge_requests/3?merge_when_pipeline_succeeds=true",
        ),
        (
            "PUT",
            "api.github.com",
            "/repos/acme/app/pulls/7?auto_merge=true",
        ),
    ] {
        let out = s.send(method, host, target, b"{}");
        assert!(
            is_refused(&out, Refusal::Merge),
            "{method} {host}{target}: {out:?}"
        );
    }
}

#[test]
fn a_graphql_merge_and_auto_merge_are_refused_even_on_the_chats_own_pull_request() {
    let mut objects = Objects::default();
    objects.0.insert("PR_kwDO_1".into(), "acme/app".into());
    let s = Scene::with(TokenKind::Oauth, objects);
    for (mutation, input) in [
        ("mergePullRequest", "MergePullRequestInput"),
        (
            "enablePullRequestAutoMerge",
            "EnablePullRequestAutoMergeInput",
        ),
        (
            "disablePullRequestAutoMerge",
            "DisablePullRequestAutoMergeInput",
        ),
        ("mergeBranch", "MergeBranchInput"),
    ] {
        let q = format!("mutation($i:{input}!){{ {mutation}(input:$i){{ clientMutationId }} }}");
        let body = graphql(
            &q,
            serde_json::json!({"i": {"pullRequestId": "PR_kwDO_1", "repositoryId": "R_acme_app"}}),
        );
        let out = s.send("POST", "api.github.com", "/graphql", &body);
        assert!(is_refused(&out, Refusal::Merge), "{mutation}: {out:?}");
    }
    let q = "mutation{ mergeRequestAccept(input:{projectPath:\"group/proj\", iid:\"3\", sha:\"abc\"}){ errors } }";
    let out = s.send(
        "POST",
        "gitlab.com",
        "/api/graphql",
        &graphql(q, serde_json::json!({})),
    );
    assert!(is_refused(&out, Refusal::Merge), "{out:?}");
}

#[test]
fn an_approval_that_would_stand_for_a_merge_is_refused() {
    let s = Scene::new();
    for (method, host, target) in [
        ("POST", "api.github.com", "/repos/acme/app/pulls/7/reviews"),
        (
            "POST",
            "gitlab.com",
            "/api/v4/projects/42/merge_requests/3/approve",
        ),
        (
            "POST",
            "gitlab.com",
            "/api/v4/projects/42/merge_requests/3/unapprove",
        ),
    ] {
        let out = s.send(method, host, target, b"{}");
        assert!(
            is_refused(&out, Refusal::Approval),
            "{method} {host}{target}: {out:?}"
        );
    }
    let q = "mutation{ addPullRequestReview(input:{pullRequestId:\"x\", event:APPROVE}){ clientMutationId } }";
    let out = s.send(
        "POST",
        "api.github.com",
        "/graphql",
        &graphql(q, serde_json::json!({})),
    );
    assert!(is_refused(&out, Refusal::Approval), "{out:?}");
}

#[test]
fn reading_whether_a_pull_request_is_merged_is_not_a_merge() {
    let s = Scene::new();
    assert!(matches!(
        s.send(
            "GET",
            "api.github.com",
            "/repos/acme/app/pulls/7/merge",
            b""
        ),
        Outcome::Forward(_)
    ));
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! #7485: `gh pr create` and `gh pr update-branch` on the chat's own repo are answered through
//! GraphQL, as `gh pr ready` already was. The bodies are what `gh` 2.69 sent: the repository query
//! `gh pr create` opens with was captured off the chat's own route (it was refused as
//! `GraphqlOtherRepo`, because its `parent { ...repo }` selects fields the parent's table lacked);
//! the two mutations are gh's `PullRequestCreate` and `PullRequestUpdateBranch` as its source
//! builds them. A mutation on another repo's pull request or repository stays refused, and so
//! does the write of an id nobody can place.

use purlis_core::forge::proxy::{Outcome, Refusal, TokenKind};

use crate::support::forge_proxy::{Objects, Scene, graphql, is_refused};

/// What `gh pr create --repo OWNER/REPO` sends first, captured from gh 2.69.0 (the repository and
/// its fork parent, which are the same fragment).
const REPOSITORY_INFO: &str = "fragment repo on Repository {
    id
    name
    owner { login }
    hasIssuesEnabled
    description
    hasWikiEnabled
    viewerPermission
    defaultBranchRef {
      name
    }
  }

  query RepositoryInfo($owner: String!, $name: String!) {
    repository(owner: $owner, name: $name) {
      ...repo
      parent {
        ...repo
      }
      mergeCommitAllowed
      rebaseMergeAllowed
      squashMergeAllowed
    }
  }";

/// gh's `api.CreatePullRequest`.
const CREATE: &str = "mutation PullRequestCreate($input: CreatePullRequestInput!) { \
    createPullRequest(input: $input) { pullRequest { id url } } }";

/// gh's `UpdatePullRequestBranch`.
const UPDATE_BRANCH: &str = "mutation PullRequestUpdateBranch($input: UpdatePullRequestBranchInput!) { \
    updatePullRequestBranch(input: $input) { pullRequest { id } } }";

fn scene() -> Scene {
    let mut objects = Objects::default();
    objects.0.insert("PR_kwDO_app".into(), "acme/app".into());
    objects
        .0
        .insert("PR_kwDO_other".into(), "acme/other".into());
    Scene::with(TokenKind::Oauth, objects)
}

fn send(s: &Scene, query: &str, variables: serde_json::Value) -> Outcome {
    s.send(
        "POST",
        "api.github.com",
        "/graphql",
        &graphql(query, variables),
    )
}

#[test]
fn the_repository_query_gh_pr_create_opens_with_is_answered() {
    purlis_core::unsteered!();
    let s = scene();
    let out = send(
        &s,
        REPOSITORY_INFO,
        serde_json::json!({"owner": "acme", "name": "app"}),
    );
    assert!(matches!(out, Outcome::Forward(_)), "{out:?}");
}

#[test]
fn the_repository_query_for_another_repo_is_still_refused() {
    purlis_core::unsteered!();
    let s = scene();
    let out = send(
        &s,
        REPOSITORY_INFO,
        serde_json::json!({"owner": "acme", "name": "other"}),
    );
    assert!(is_refused(&out, Refusal::GraphqlOtherRepo), "{out:?}");
}

#[test]
fn the_parent_of_a_repo_may_not_be_walked_into() {
    purlis_core::unsteered!();
    let s = scene();
    let q = "query{ repository(owner:\"acme\", name:\"app\"){ parent{ description issues(first:1){ totalCount } } } }";
    let out = send(&s, q, serde_json::json!({}));
    assert!(is_refused(&out, Refusal::GraphqlOtherRepo), "{out:?}");
}

#[test]
fn gh_pr_create_on_the_chats_repo_is_answered_and_on_another_is_refused() {
    purlis_core::unsteered!();
    let s = scene();
    let input = |repo: &str| {
        serde_json::json!({"input": {
            "repositoryId": repo, "baseRefName": "main", "headRefName": "fix",
            "title": "t", "body": "b", "draft": false, "maintainerCanModify": true}})
    };
    let out = send(&s, CREATE, input("R_acme_app"));
    assert!(matches!(out, Outcome::Forward(_)), "{out:?}");
    let out = send(&s, CREATE, input("R_acme_other"));
    assert!(
        is_refused(&out, Refusal::GraphqlPullRequestOtherRepo),
        "{out:?}"
    );
}

#[test]
fn gh_pr_update_branch_on_the_chats_pull_request_is_answered_and_on_another_is_refused() {
    purlis_core::unsteered!();
    let s = scene();
    let input = |id: &str| {
        serde_json::json!({"input": {
            "pullRequestId": id, "expectedHeadOid": "a".repeat(40)}})
    };
    let out = send(&s, UPDATE_BRANCH, input("PR_kwDO_app"));
    assert!(matches!(out, Outcome::Forward(_)), "{out:?}");
    // `--rebase` adds the method; gh's input struct also carries a null `clientMutationId`.
    for method in ["MERGE", "REBASE"] {
        let with_method = serde_json::json!({"input": {
            "clientMutationId": null, "pullRequestId": "PR_kwDO_app",
            "expectedHeadOid": "a".repeat(40), "updateMethod": method}});
        let out = send(&s, UPDATE_BRANCH, with_method);
        assert!(matches!(out, Outcome::Forward(_)), "{method}: {out:?}");
    }
    let out = send(&s, UPDATE_BRANCH, input("PR_kwDO_other"));
    assert!(
        is_refused(&out, Refusal::GraphqlPullRequestOtherRepo),
        "{out:?}"
    );
    // An id the forge cannot place fails closed.
    let out = send(&s, UPDATE_BRANCH, input("PR_kwDO_nobody"));
    assert!(
        is_refused(&out, Refusal::GraphqlPullRequestOtherRepo),
        "{out:?}"
    );
}

#[test]
fn a_refusal_as_another_repo_names_the_rest_routes_that_work() {
    purlis_core::unsteered!();
    let said = Refusal::GraphqlPullRequestOtherRepo
        .advice()
        .expect("advice");
    assert!(said.contains("repos/OWNER/REPO/pulls -X POST"), "{said}");
    assert!(said.contains("pulls/N/update-branch"), "{said}");
}

#[test]
fn a_fragment_on_repository_does_not_widen_what_the_parent_may_be_asked() {
    purlis_core::unsteered!();
    let s = scene();
    let q = "fragment r on Repository { issues(first:1){ totalCount } }
        query{ repository(owner:\"acme\", name:\"app\"){ parent{ ...r } } }";
    let out = send(&s, q, serde_json::json!({}));
    assert!(is_refused(&out, Refusal::GraphqlOtherRepo), "{out:?}");
}

#[test]
fn an_update_method_that_is_not_one_of_the_two_enum_values_is_still_read_as_text() {
    purlis_core::unsteered!();
    let s = scene();
    let body = serde_json::json!({"input": {
        "pullRequestId": "PR_kwDO_app", "updateMethod": "/merge"}});
    let out = send(&s, UPDATE_BRANCH, body);
    assert!(is_refused(&out, Refusal::MergeCommand), "{out:?}");
}

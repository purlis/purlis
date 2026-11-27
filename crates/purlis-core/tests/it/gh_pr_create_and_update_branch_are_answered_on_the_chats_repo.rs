// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! #7485: `gh pr create` and `gh pr update-branch` on the chat's own repo are answered through
//! GraphQL, as `gh pr ready` already was. The bodies are what `gh` 2.69 sent: the repository query
//! `gh pr create` opens with was captured off the chat's own route (it was refused as
//! `GraphqlOtherRepo`, because its `parent { ...repo }` selects fields the parent's table lacked);
//! the two mutations are gh's `PullRequestCreate` and `PullRequestUpdateBranch` as its source
//! builds them. A mutation on another repo's pull request or repository stays refused, and so
//! does the write of an id nobody can place.

use purlis_core::forge::proxy::{Outcome, Refusal};

use crate::support::forge_proxy::{ObjectsWithHeads, Scene, graphql, is_refused};

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

/// The pull request `gh pr update-branch <number>` reads first, captured from gh 2.69.0 off the
/// chat's own route.
const FIND_BY_NUMBER: &str = "query PullRequestByNumber($owner: String!, $repo: String!, $pr_number: Int!) {
    repository(owner: $owner, name: $repo) {
      pullRequest(number: $pr_number) {id,number,headRefName,headRefOid,headRepositoryOwner{id,login,...on User{name}},mergeable}
    }
  }";

/// What gh 2.69.0 asks next, `api.ComparePullRequestBaseBranchWith`: how far the head is behind
/// its base (#7957). It was refused as `GraphqlOtherRepo`, because `Ref` had no `compare`.
const COMPARE: &str = "query ComparePullRequestBaseBranchWith($owner: String!, $repo: String!, $pullRequestNumber: Int!, $headRef: String!) {
\t\trepository(owner: $owner, name: $repo) {
\t\t\tpullRequest(number: $pullRequestNumber) {
\t\t\t\tbaseRef {
\t\t\t\t\tcompare (headRef: $headRef) {
\t\t\t\t\t\taheadBy, behindBy, status
\t\t\t\t\t}
\t\t\t\t}
\t\t\t}
\t\t}
\t}";

fn scene() -> Scene {
    // The pull request's head is what update-branch writes to: here a branch of the chat's repo
    // that the chat made (its tip is `1` * 40, as the pull request's head is).
    let mut objects =
        ObjectsWithHeads::default().pull("acme/app", "PR_kwDO_app", 7, "acme/app", "fix");
    objects
        .objects
        .0
        .insert("PR_kwDO_other".into(), "acme/other".into());
    Scene::tuned_owning(objects, &[("fix", &"1".repeat(40))], |proxy| proxy)
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

#[test]
fn every_document_gh_pr_update_branch_sends_on_the_chats_pull_request_is_answered() {
    purlis_core::unsteered!();
    let s = scene();
    let out = send(
        &s,
        FIND_BY_NUMBER,
        serde_json::json!({"owner": "acme", "repo": "app", "pr_number": 7}),
    );
    assert!(matches!(out, Outcome::Forward(_)), "{out:?}");
    let out = send(
        &s,
        COMPARE,
        serde_json::json!({"owner": "acme", "repo": "app", "pullRequestNumber": 7, "headRef": "fix"}),
    );
    assert!(matches!(out, Outcome::Forward(_)), "{out:?}");
    let out = send(
        &s,
        UPDATE_BRANCH,
        serde_json::json!({"input": {"pullRequestId": "PR_kwDO_app", "expectedHeadOid": "1".repeat(40)}}),
    );
    assert!(matches!(out, Outcome::Forward(_)), "{out:?}");
}

#[test]
fn the_compare_of_another_repos_pull_request_is_refused() {
    purlis_core::unsteered!();
    let s = scene();
    let out = send(
        &s,
        COMPARE,
        serde_json::json!({"owner": "acme", "repo": "other", "pullRequestNumber": 7, "headRef": "fix"}),
    );
    assert!(is_refused(&out, Refusal::GraphqlOtherRepo), "{out:?}");
}

#[test]
fn a_comparison_shows_its_counts_and_nothing_it_holds() {
    purlis_core::unsteered!();
    let s = scene();
    // The commits between the two sides, and either side's target, may reach a fork's content.
    for inner in [
        "commits(first: 5) { nodes { oid message } }",
        "baseTarget { oid }",
        "headTarget { oid }",
        "id",
    ] {
        let q = format!(
            "query{{ repository(owner:\"acme\", name:\"app\"){{ defaultBranchRef{{ compare(headRef:\"someone:fix\"){{ {inner} }} }} }} }}"
        );
        let out = send(&s, &q, serde_json::json!({}));
        assert!(
            is_refused(&out, Refusal::GraphqlOtherRepo),
            "{inner}: {out:?}"
        );
    }
}

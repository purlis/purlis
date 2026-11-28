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
fn a_compare_under_a_repository_that_is_not_the_chats_is_refused_at_that_repository() {
    purlis_core::unsteered!();
    let s = scene();
    // Refused at `repository(owner:, name:)`, before `compare` is read: what `compare` may be
    // given is pinned by the tests below.
    let out = send(
        &s,
        COMPARE,
        serde_json::json!({"owner": "acme", "repo": "other", "pullRequestNumber": 7, "headRef": "fix"}),
    );
    assert!(is_refused(&out, Refusal::GraphqlOtherRepo), "{out:?}");
}

/// A `compare` of the chat's repo's default branch given `args`, under the variable
/// declarations `decl` and the variables `vars`.
fn compare_of(s: &Scene, decl: &str, args: &str, vars: serde_json::Value) -> Outcome {
    // No argument at all is written without parentheses, which GraphQL does not allow empty.
    let args = if args.is_empty() {
        String::new()
    } else {
        format!("({args})")
    };
    let q = format!(
        "query Q{decl} {{ repository(owner:\"acme\", name:\"app\") {{ defaultBranchRef {{ compare{args} {{ aheadBy behindBy status }} }} }} }}"
    );
    send(s, &q, vars)
}

#[test]
fn a_compare_with_a_branch_of_the_chats_repo_is_answered() {
    purlis_core::unsteered!();
    let s = scene();
    for head in [
        "fix",
        "acme:fix",
        "Acme:fix",
        "acme/app:fix",
        "refs/heads/fix",
    ] {
        let literal = compare_of(&s, "", &format!("headRef: {head:?}"), serde_json::json!({}));
        assert!(
            matches!(literal, Outcome::Forward(_)),
            "{head}: {literal:?}"
        );
        let held = compare_of(
            &s,
            "($h: String!)",
            "headRef: $h",
            serde_json::json!({"h": head}),
        );
        assert!(matches!(held, Outcome::Forward(_)), "{head}: {held:?}");
    }
    // A default the request does not override is the value.
    let defaulted = compare_of(
        &s,
        "($h: String = \"fix\")",
        "headRef: $h",
        serde_json::json!({}),
    );
    assert!(matches!(defaulted, Outcome::Forward(_)), "{defaulted:?}");
}

#[test]
fn a_compare_with_a_head_in_another_owner_is_refused() {
    purlis_core::unsteered!();
    let s = scene();
    // ADR 0128 D3d: a compare that names a head in another owner reaches that fork's objects.
    for head in [
        "someone:fix",
        "someone/app:fix",
        "acme/other:fix",
        "acmes:fix",
        "acme:fix:more",
        "main...someone:fix",
    ] {
        let literal = compare_of(&s, "", &format!("headRef: {head:?}"), serde_json::json!({}));
        assert!(
            is_refused(&literal, Refusal::GraphqlOtherRepo),
            "{head}: {literal:?}"
        );
        let held = compare_of(
            &s,
            "($h: String!)",
            "headRef: $h",
            serde_json::json!({"h": head}),
        );
        assert!(
            is_refused(&held, Refusal::GraphqlOtherRepo),
            "{head}: {held:?}"
        );
        let defaulted = compare_of(
            &s,
            &format!("($h: String = {head:?})"),
            "headRef: $h",
            serde_json::json!({}),
        );
        assert!(
            is_refused(&defaulted, Refusal::GraphqlOtherRepo),
            "{head}: {defaulted:?}"
        );
    }
}

#[test]
fn a_compare_whose_head_cannot_be_read_as_one_string_is_refused() {
    purlis_core::unsteered!();
    let s = scene();
    for (decl, args, vars) in [
        // Given twice: the forge reads one of them by its own rule.
        (
            "",
            "headRef: \"fix\", headRef: \"someone:fix\"",
            serde_json::json!({}),
        ),
        (
            "",
            "headRef: \"someone:fix\", headRef: \"fix\"",
            serde_json::json!({}),
        ),
        // Not given at all, or not one string.
        ("", "", serde_json::json!({})),
        ("($h: String!)", "headRef: $h", serde_json::json!({})),
        ("($h: String!)", "headRef: $h", serde_json::json!({"h": 5})),
        (
            "($h: String!)",
            "headRef: $h",
            serde_json::json!({"h": ["someone:fix"]}),
        ),
        (
            "($h: String)",
            "headRef: $h",
            serde_json::json!({"h": null}),
        ),
        (
            "",
            "headRef: $undeclared",
            // The value is read wherever the document takes it from.
            serde_json::json!({"undeclared": "someone:fix"}),
        ),
        // An argument `compare` does not have.
        (
            "",
            "headRef: \"fix\", baseRef: \"someone:main\"",
            serde_json::json!({}),
        ),
    ] {
        let out = compare_of(&s, decl, args, vars.clone());
        assert!(
            is_refused(&out, Refusal::GraphqlOtherRepo),
            "{decl} {args} {vars}: {out:?}"
        );
    }
}

#[test]
fn a_compare_on_a_pull_requests_head_is_refused_whatever_it_is_given() {
    purlis_core::unsteered!();
    let s = scene();
    // A pull request's head may be a ref in a fork, whose `compare` reads that fork.
    let under_pull = |inner: &str| {
        format!(
            "query {{ repository(owner:\"acme\", name:\"app\") {{ pullRequest(number: 7) {{ {inner} }} }} }}"
        )
    };
    for q in [
        under_pull("headRef { compare(headRef: \"fix\") { aheadBy } }"),
        under_pull("headRef { ... on Ref { compare(headRef: \"fix\") { aheadBy } } }"),
        under_pull("headRef { ...R }")
            + " fragment R on Ref { compare(headRef: \"fix\") { aheadBy } }",
    ] {
        let out = send(&s, &q, serde_json::json!({}));
        assert!(is_refused(&out, Refusal::GraphqlOtherRepo), "{q}: {out:?}");
    }
    // What a pull request's head is read for still is, a fragment on `Ref` included.
    for q in [
        under_pull("headRef { name target { oid } }"),
        under_pull("headRef { ...N }") + " fragment N on Ref { name }",
        under_pull("baseRef { compare(headRef: \"fix\") { aheadBy } }"),
    ] {
        let out = send(&s, &q, serde_json::json!({}));
        assert!(matches!(out, Outcome::Forward(_)), "{q}: {out:?}");
    }
}

#[test]
fn a_comparison_shows_its_counts_and_nothing_it_holds() {
    purlis_core::unsteered!();
    let s = scene();
    // The comparison's counts and status are read, so the refusals below are its fields'.
    let counts = compare_of(&s, "", "headRef: \"fix\"", serde_json::json!({}));
    assert!(matches!(counts, Outcome::Forward(_)), "{counts:?}");
    // The commits between the two sides, and either side's target, may reach a fork's content.
    for inner in [
        "commits(first: 5) { nodes { oid message } }",
        "baseTarget { oid }",
        "headTarget { oid }",
        "id",
    ] {
        let q = format!(
            "query{{ repository(owner:\"acme\", name:\"app\"){{ defaultBranchRef{{ compare(headRef:\"fix\"){{ {inner} }} }} }} }}"
        );
        let out = send(&s, &q, serde_json::json!({}));
        assert!(
            is_refused(&out, Refusal::GraphqlOtherRepo),
            "{inner}: {out:?}"
        );
    }
}

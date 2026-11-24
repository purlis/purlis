// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! #7544: through the forge route `gh auth status` said the token was invalid, and issue search
//! (`gh issue list --search`, `gh api search/issues`) was refused. `gh auth status` reads the API
//! root (`GET /`, for the token's scopes) and then `viewer { login }`; the root is now on the list
//! of reads outside a repo, answered by the forge for the brokered token. `gh issue list --search`
//! sends `search(type: $type, ...)` with the type in a variable, which is read by its value. REST
//! `GET /search/issues?q=` is answered when `q` names exactly one `repo:`, the chat's own: the
//! same reading as GraphQL's `search`, so a search of another repo, of none, or of two is refused.
//! The advice about `gh pr create` and `gh pr update-branch` is said only for those mutations.
//!
//! The GraphQL body is gh 2.69's `IssueSearch`, as its source builds it for `--search`.

use purlis_core::forge::proxy::{Outcome, Refusal, TokenKind};

use crate::support::forge_proxy::{Objects, Scene, graphql, is_refused};

/// What `gh issue list --repo OWNER/REPO --search ...` sends (gh's `api.IssueSearch`).
const ISSUE_SEARCH: &str = "query IssueSearch($repo: String!, $owner: String!, $type: SearchType!, $limit: Int, $after: String, $query: String!) {
  repository(owner: $owner, name: $repo) { hasIssuesEnabled }
  search(type: $type, last: $limit, after: $after, query: $query) {
    issueCount
    nodes {
      ...on Issue { number title url state updatedAt labels(first: 100) { nodes { name } totalCount } }
    }
    pageInfo { hasNextPage endCursor }
  }
}";

/// What `gh auth status` asks for after the root: the login behind the token.
const USER_CURRENT: &str = "query UserCurrent{viewer{login}}";

fn scene() -> Scene {
    Scene::with(TokenKind::Oauth, Objects::default())
}

fn get(s: &Scene, target: &str) -> Outcome {
    s.send("GET", "api.github.com", target, b"")
}

fn issue_search(s: &Scene, query: &str, kind: &str) -> Outcome {
    s.send(
        "POST",
        "api.github.com",
        "/graphql",
        &graphql(
            ISSUE_SEARCH,
            serde_json::json!({
                "owner": "acme", "repo": "app", "type": kind, "limit": 30, "query": query}),
        ),
    )
}

#[test]
fn gh_auth_status_reads_of_the_api_root_and_the_login_are_answered() {
    purlis_core::unsteered!();
    let s = scene();
    for method in ["GET", "HEAD"] {
        let out = s.send(method, "api.github.com", "/", b"");
        let Outcome::Forward(f) = out else {
            panic!("{method}: {out:?}");
        };
        assert_eq!(f.upstream, "https://api.github.com/", "{method}");
    }
    let out = s.send(
        "POST",
        "api.github.com",
        "/graphql",
        &graphql(USER_CURRENT, serde_json::json!({})),
    );
    assert!(matches!(out, Outcome::Forward(_)), "{out:?}");
}

#[test]
fn the_api_root_is_read_only_and_the_web_root_is_not_the_api() {
    purlis_core::unsteered!();
    let s = scene();
    let out = s.send("POST", "api.github.com", "/", b"{}");
    assert!(!matches!(out, Outcome::Forward(_)), "{out:?}");
    // The web host's `/` is the site's home page, not the API root.
    let out = s.send("GET", "github.com", "/", b"");
    assert!(!matches!(out, Outcome::Forward(_)), "{out:?}");
}

#[test]
fn gh_issue_list_search_on_the_chats_repo_is_answered_with_its_type_in_a_variable() {
    purlis_core::unsteered!();
    let s = scene();
    for q in [
        "repo:acme/app is:issue state:open duplicate in:title",
        "repo:Acme/App is:issue state:open",
    ] {
        let out = issue_search(&s, q, "ISSUE");
        assert!(matches!(out, Outcome::Forward(_)), "{q}: {out:?}");
    }
}

#[test]
fn a_graphql_search_of_another_repo_or_none_or_two_or_another_type_is_refused() {
    purlis_core::unsteered!();
    let s = scene();
    for q in [
        "repo:acme/other is:issue duplicate",
        "is:issue duplicate in:title",
        "org:acme is:issue",
        "repo:acme/app repo:acme/other is:issue",
        "repo:acme/app OR repo:acme/other",
    ] {
        let out = issue_search(&s, q, "ISSUE");
        assert!(is_refused(&out, Refusal::GraphqlOtherRepo), "{q}: {out:?}");
    }
    // The type in a variable is read by its value: anything but ISSUE is another search.
    for kind in ["REPOSITORY", "USER", "DISCUSSION"] {
        let out = issue_search(&s, "repo:acme/app", kind);
        assert!(
            is_refused(&out, Refusal::GraphqlOtherRepo),
            "{kind}: {out:?}"
        );
    }
}

#[test]
fn a_rest_issue_search_on_the_chats_repo_is_answered() {
    purlis_core::unsteered!();
    let s = scene();
    for (target, upstream) in [
        (
            "/search/issues?q=repo:acme/app+is:issue+duplicate",
            "https://api.github.com/search/issues?q=repo%3Aacme%2Fapp+is%3Aissue+duplicate",
        ),
        (
            "/search/issues?q=repo%3Aacme%2Fapp%20in%3Atitle&per_page=5&page=2&sort=updated&order=desc",
            "https://api.github.com/search/issues?q=repo%3Aacme%2Fapp+in%3Atitle&per_page=5&page=2&sort=updated&order=desc",
        ),
    ] {
        let out = get(&s, target);
        let Outcome::Forward(f) = out else {
            panic!("{target}: {out:?}");
        };
        assert_eq!(f.upstream, upstream, "{target}");
    }
}

#[test]
fn a_rest_issue_search_of_another_repo_or_none_or_two_is_refused() {
    purlis_core::unsteered!();
    let s = scene();
    for target in [
        // another repo
        "/search/issues?q=repo:acme/other+is:issue",
        // no repo qualifier, an org, a user
        "/search/issues?q=duplicate+in:title",
        "/search/issues?q=org:acme+is:issue",
        "/search/issues?q=repo:acme/app+user:someone",
        // two repo qualifiers, and an OR across repos
        "/search/issues?q=repo:acme/app+repo:acme/other",
        "/search/issues?q=repo:acme/app+OR+repo:acme/other",
        "/search/issues?q=repo:acme/app+OR+is:issue",
        // no q, two q, a key the proxy does not know
        "/search/issues",
        "/search/issues?q=repo:acme/app&q=repo:acme/other",
        "/search/issues?q=repo:acme/app&advanced_search=true",
        // another search route
        "/search/code?q=repo:acme/app+token",
        "/search/repositories?q=repo:acme/app",
    ] {
        let out = get(&s, target);
        assert!(!matches!(out, Outcome::Forward(_)), "{target}: {out:?}");
    }
    // A search is a read: a write to the route is not one.
    let out = s.send(
        "POST",
        "api.github.com",
        "/search/issues?q=repo:acme/app",
        b"{}",
    );
    assert!(!matches!(out, Outcome::Forward(_)), "{out:?}");
}

#[test]
fn the_rest_advice_is_said_only_for_the_two_pull_request_mutations() {
    purlis_core::unsteered!();
    let said = Refusal::GraphqlPullRequestOtherRepo
        .advice()
        .expect("advice");
    assert!(said.contains("repos/OWNER/REPO/pulls -X POST"), "{said}");
    assert!(said.contains("pulls/N/update-branch"), "{said}");
    // A refusal of a search or a read has none.
    assert_eq!(Refusal::GraphqlOtherRepo.advice(), None);
    let s = scene();
    let out = issue_search(&s, "repo:acme/other is:issue", "ISSUE");
    assert!(is_refused(&out, Refusal::GraphqlOtherRepo), "{out:?}");
    let read = "query{ repository(owner:\"acme\", name:\"other\"){ name } }";
    let out = s.send(
        "POST",
        "api.github.com",
        "/graphql",
        &graphql(read, serde_json::json!({})),
    );
    assert!(is_refused(&out, Refusal::GraphqlOtherRepo), "{out:?}");
    // The mutation on another repo is the one that carries it.
    let mutation = "mutation PullRequestCreate($input: CreatePullRequestInput!) { \
        createPullRequest(input: $input) { pullRequest { id url } } }";
    let input = serde_json::json!({"input": {
        "repositoryId": "R_acme_other", "baseRefName": "main", "headRefName": "fix",
        "title": "t"}});
    let out = s.send(
        "POST",
        "api.github.com",
        "/graphql",
        &graphql(mutation, input),
    );
    assert!(
        is_refused(&out, Refusal::GraphqlPullRequestOtherRepo),
        "{out:?}"
    );
}

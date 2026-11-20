// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! One login, slice A2 (review item M2): GraphQL is read against an explicit allowlist of fields
//! **per type**. What `gh` and `glab` send for pull request, issue, repository and release view
//! and list is answered (the fixtures in `tests/fixtures/one-login-graphql/`), `node` and `nodes`
//! work for the types the tables name and ids the proxy can tie to the chat's repos, and a field
//! the tables do not name is refused wherever it is, a fragment or a connection included.

use purlis_core::forge::proxy::{Outcome, Refusal, TokenKind};

use crate::support::forge_proxy::{Objects, Scene, graphql, is_refused};

fn fixtures(name: &str) -> Vec<serde_json::Value> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/one-login-graphql")
        .join(name);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path:?}: {e}"));
    serde_json::from_str(&text).unwrap()
}

fn scene() -> Scene {
    let mut objects = Objects::default();
    objects.0.insert("PR_kwDO_1".into(), "acme/app".into());
    objects.0.insert("I_kwDO_2".into(), "acme/app".into());
    objects
        .0
        .insert("PR_kwDO_other".into(), "acme/other".into());
    Scene::with(TokenKind::Oauth, objects)
}

fn gh(s: &Scene, query: &str) -> Outcome {
    s.send(
        "POST",
        "api.github.com",
        "/graphql",
        &graphql(
            query,
            serde_json::json!({"o":"acme","n":"app","id":"PR_kwDO_1"}),
        ),
    )
}

#[test]
fn every_document_gh_sends_for_view_and_list_is_answered() {
    purlis_core::unsteered!();
    let s = scene();
    let mut documents = fixtures("gh.json");
    documents.extend(fixtures("gh-detect-status-create.json"));
    // What `gh` itself sent on the CI runners of the listener's workflow, Linux and macOS.
    documents.extend(fixtures("gh-captured.json"));
    assert!(documents.len() >= 30, "the fixtures are the evidence");
    for doc in documents {
        let body = graphql(doc["query"].as_str().unwrap(), doc["variables"].clone());
        let out = s.send("POST", "api.github.com", "/graphql", &body);
        assert!(
            matches!(out, Outcome::Forward(_)),
            "{}: {out:?}",
            doc["command"]
        );
    }
}

#[test]
fn every_document_glab_sends_is_answered() {
    purlis_core::unsteered!();
    let s = scene();
    for doc in fixtures("glab.json") {
        let body = graphql(doc["query"].as_str().unwrap(), doc["variables"].clone());
        let out = s.send("POST", "gitlab.com", "/api/graphql", &body);
        assert!(
            matches!(out, Outcome::Forward(_)),
            "{}: {out:?}",
            doc["command"]
        );
    }
}

#[test]
fn a_field_that_is_not_on_its_types_table_is_refused_wherever_it_is() {
    purlis_core::unsteered!();
    let s = scene();
    let repo = "repository(owner:\"acme\", name:\"app\")";
    for q in [
        // A field nobody told the proxy of, at each depth.
        format!("query{{ {repo}{{ somethingGitHubShipsTomorrow }} }}"),
        format!("query{{ {repo}{{ pullRequest(number:1){{ somethingNew }} }} }}"),
        format!(
            "query{{ {repo}{{ pullRequests(first:5){{ nodes{{ author{{ repositories(first:5){{ totalCount }} }} }} }} }} }}"
        ),
        // A way out of the repo, through a type that is allowed.
        format!(
            "query{{ {repo}{{ pullRequest(number:1){{ headRepository{{ pullRequests(first:1){{ totalCount }} }} }} }} }}"
        ),
        format!(
            "query{{ {repo}{{ pullRequest(number:1){{ headRepository{{ collaborators(first:1){{ totalCount }} }} }} }} }}"
        ),
        format!("query{{ {repo}{{ parent{{ issues(first:1){{ totalCount }} }} }} }}"),
        format!("query{{ {repo}{{ collaborators(first:1){{ totalCount }} }} }}"),
        format!("query{{ {repo}{{ branchProtectionRules(first:1){{ totalCount }} }} }}"),
        format!("query{{ {repo}{{ rulesets(first:1){{ totalCount }} }} }}"),
        format!("query{{ {repo}{{ owner{{ repositories(first:1){{ totalCount }} }} }} }}"),
        format!("query{{ {repo}{{ issue(number:1){{ author{{ ...on User{{ email }} }} }} }} }}"),
        // Not a connection's field.
        format!("query{{ {repo}{{ issues(first:1){{ secrets }} }} }}"),
        format!("query{{ {repo}{{ issues(first:1){{ edges{{ other }} }} }} }}"),
        // A leaf with something selected, an object with nothing.
        format!("query{{ {repo}{{ name{{ x }} }} }}"),
        format!("query{{ {repo}{{ owner }} }}"),
        // Roots that are not on the list.
        "query{ search(query:\"x\", type:ISSUE, first:1){ issueCount } }".to_string(),
        "query{ organization(login:\"acme\"){ login } }".to_string(),
        "query{ user(login:\"x\"){ login } }".to_string(),
    ] {
        let out = gh(&s, &q);
        assert!(is_refused(&out, Refusal::GraphqlOtherRepo), "{q}: {out:?}");
    }
}

#[test]
fn a_fragment_is_read_where_it_is_spread_as_the_type_it_is_spread_on() {
    purlis_core::unsteered!();
    let s = scene();
    let ok = "query{ repository(owner:\"acme\", name:\"app\"){ pullRequest(number:1){ ...P } } } fragment P on PullRequest{ number title author{ login } }";
    assert!(matches!(gh(&s, ok), Outcome::Forward(_)));
    for q in [
        // The fragment names a type the spread is not on.
        "query{ repository(owner:\"acme\", name:\"app\"){ pullRequest(number:1){ ...P } } } fragment P on Repository{ name }",
        // A field the fragment's type does not have.
        "query{ repository(owner:\"acme\", name:\"app\"){ pullRequest(number:1){ ...P } } } fragment P on PullRequest{ secrets }",
        // A spread of a fragment the document does not define.
        "query{ repository(owner:\"acme\", name:\"app\"){ ...Nope } }",
        // Fragments that spread each other for ever.
        "query{ repository(owner:\"acme\", name:\"app\"){ ...A } } fragment A on Repository{ ...B } fragment B on Repository{ ...A }",
        // Two fragments of a name: a reader may pick either.
        "query{ repository(owner:\"acme\", name:\"app\"){ ...A } } fragment A on Repository{ name } fragment A on Repository{ forks }",
        // An inline fragment on a type the parent cannot be.
        "query{ repository(owner:\"acme\", name:\"app\"){ pullRequest(number:1){ ...on Repository{ name } } } }",
        "query{ repository(owner:\"acme\", name:\"app\"){ owner{ ...on Team{ name } } } }",
    ] {
        let out = gh(&s, q);
        assert!(
            is_refused(&out, Refusal::GraphqlOtherRepo) || is_refused(&out, Refusal::GraphqlParse),
            "{q}: {out:?}"
        );
    }
}

#[test]
fn node_and_nodes_work_only_for_ids_tied_to_the_chats_repos_and_the_types_the_tables_name() {
    purlis_core::unsteered!();
    let s = scene();
    for q in [
        "query{ node(id:\"PR_kwDO_1\"){ id ...on PullRequest{ number title } } }",
        "query{ node(id:\"R_acme_app\"){ ...on Repository{ name } } }",
        "query{ nodes(ids:[\"PR_kwDO_1\",\"I_kwDO_2\"]){ __typename ...on PullRequest{ number } ...on Issue{ title } } }",
    ] {
        let out = gh(&s, q);
        assert!(matches!(out, Outcome::Forward(_)), "{q}: {out:?}");
    }
    for q in [
        // Another chat's repo, an id nobody can place, and one among good ones.
        "query{ node(id:\"PR_kwDO_other\"){ id } }",
        "query{ node(id:\"PR_unknown\"){ id } }",
        "query{ nodes(ids:[\"PR_kwDO_1\",\"PR_kwDO_other\"]){ id } }",
        "query{ nodes(ids:[]){ id } }",
        "query{ nodes(ids:\"PR_kwDO_1\"){ id } }",
        "query{ node{ id } }",
        // An id of the chat's own, read as a type that walks out of it.
        "query{ node(id:\"PR_kwDO_1\"){ ...on User{ login } } }",
        "query{ node(id:\"PR_kwDO_1\"){ ...on Organization{ login } } }",
        "query{ node(id:\"PR_kwDO_1\"){ ...on PullRequest{ somethingNew } } }",
        "query{ node(id:\"R_acme_app\"){ ...on Repository{ collaborators(first:1){ totalCount } } } }",
        "query{ node(id:\"PR_kwDO_1\"){ secrets } }",
    ] {
        let out = gh(&s, q);
        assert!(is_refused(&out, Refusal::GraphqlOtherRepo), "{q}: {out:?}");
    }
    // A variable id is read like a literal one.
    let q = "query($id:ID!){ node(id:$id){ id } }";
    let out = s.send(
        "POST",
        "api.github.com",
        "/graphql",
        &graphql(q, serde_json::json!({"id":"PR_kwDO_other"})),
    );
    assert!(is_refused(&out, Refusal::GraphqlOtherRepo), "{out:?}");
    // With no resolver an object is nobody's, so the document is refused.
    let none = Scene::single();
    let out = none.send(
        "POST",
        "api.github.com",
        "/graphql",
        &graphql(
            "query{ node(id:\"PR_kwDO_1\"){ id } }",
            serde_json::json!({}),
        ),
    );
    assert!(is_refused(&out, Refusal::GraphqlOtherRepo), "{out:?}");
}

#[test]
fn a_gitlab_document_is_read_against_gitlabs_tables() {
    purlis_core::unsteered!();
    let s = scene();
    let send = |q: &str| {
        s.send(
            "POST",
            "gitlab.com",
            "/api/graphql",
            &graphql(q, serde_json::json!({})),
        )
    };
    for q in [
        "query{ project(fullPath:\"group/proj\"){ group{ name } } }",
        "query{ project(fullPath:\"group/proj\"){ members{ nodes{ id } } } }",
        "query{ project(fullPath:\"group/proj\"){ ciVariables{ nodes{ key } } } }",
        "query{ project(fullPath:\"group/proj\"){ mergeRequests{ nodes{ somethingNew } } } }",
        "query{ project(fullPath:\"group/proj-x\"){ name } }",
        "query{ group(fullPath:\"group\"){ name } }",
        "query{ users{ nodes{ id } } }",
        "query{ currentUser{ personalAccessTokens{ nodes{ id } } } }",
    ] {
        let out = send(q);
        assert!(is_refused(&out, Refusal::GraphqlOtherRepo), "{q}: {out:?}");
    }
}

#[test]
fn what_a_mutation_asks_back_is_read_against_the_tables_too() {
    purlis_core::unsteered!();
    let s = Scene::single();
    let create = |back: &str| {
        let q = format!(
            "mutation{{ createIssue(input:{{repositoryId:\"R_acme_app\", title:\"t\"}}){{ {back} }} }}"
        );
        gh(&s, &q)
    };
    for back in [
        "clientMutationId",
        "issue{ number url title }",
        "issue{ author{ login } labels(first:5){ nodes{ name } } }",
    ] {
        assert!(matches!(create(back), Outcome::Forward(_)), "{back}");
    }
    for back in [
        "issue{ repository{ collaborators(first:1){ totalCount } } }",
        "issue{ somethingNew }",
        "somethingNew",
        "issue{ author{ repositories(first:5){ totalCount } } }",
    ] {
        let out = create(back);
        assert!(
            is_refused(&out, Refusal::GraphqlOtherRepo),
            "{back}: {out:?}"
        );
    }
}

#[test]
fn a_search_is_answered_only_when_it_is_of_one_of_the_chats_repos_and_of_nothing_else() {
    purlis_core::unsteered!();
    let s = scene();
    let search = |q: &str| {
        let doc = "query($q:String!){ search(query:$q, type:ISSUE, first:5){ issueCount nodes{ ...on PullRequest{ number } } } }";
        s.send(
            "POST",
            "api.github.com",
            "/graphql",
            &graphql(doc, serde_json::json!({ "q": q })),
        )
    };
    for q in [
        "repo:acme/app is:pr is:open",
        "is:pr repo:Acme/App author:octocat",
        "repo:acme/app author:a -label:wip",
    ] {
        assert!(matches!(search(q), Outcome::Forward(_)), "{q}");
    }
    for q in [
        "is:pr is:open",
        "repo:acme/other is:pr",
        "repo:acme/app repo:acme/other",
        "repo:acme/app OR org:acme",
        "repo:acme/app OR is:issue",
        "repo:acme/app or is:issue",
        "repo:acme/app Or is:issue",
        "is:issue OR repo:acme/app",
        "repo:acme/app AND is:issue",
        "repo:acme/app NOT is:closed",
        "repo:acme/app (author:a OR author:b)",
        "repo:acme/app (is:issue)",
        "(repo:acme/app)",
        "repo:acme/app user:someone",
        "repo:acme/app owner:someone",
        "repo:acme/app team:acme/x",
        "repo:acme/app enterprise:x",
        "-repo:acme/app",
        "repo:acme/app -org:acme",
        "repo:\"acme/app\"",
        "repo:acme/app \"org:acme\"",
        "repo:acme/app,acme/other",
        "repo:acme/*",
        "",
    ] {
        let out = search(q);
        assert!(
            is_refused(&out, Refusal::GraphqlOtherRepo),
            "{q:?}: {out:?}"
        );
    }
    // Not issues, not a literal type, and not what a search of a repo can return.
    for q in [
        "query{ search(query:\"repo:acme/app\", type:REPOSITORY, first:5){ repositoryCount } }",
        "query{ search(query:\"repo:acme/app\", type:USER, first:5){ userCount } }",
        "query($t:SearchType!){ search(query:\"repo:acme/app\", type:$t, first:5){ issueCount } }",
        "query{ search(query:\"repo:acme/app\", type:ISSUE, first:5){ nodes{ ...on User{ login } } } }",
        "query{ search(query:\"repo:acme/app\", type:ISSUE, first:5){ nodes{ ...on Repository{ name } } } }",
        "query{ search(type:ISSUE, first:5){ issueCount } }",
    ] {
        let out = gh(&s, q);
        assert!(is_refused(&out, Refusal::GraphqlOtherRepo), "{q}: {out:?}");
    }
}

#[test]
fn what_a_reference_reaches_is_named_and_never_read() {
    purlis_core::unsteered!();
    let s = scene();
    // "Fixes org/private#N" in the chat's own pull request makes `closingIssuesReferences` name
    // an issue of another repo: only what names it may be asked for, never its body.
    let reach = |root: &str, edge: &str, inner: &str| {
        gh(
            &s,
            &format!(
                "query{{ repository(owner:\"acme\", name:\"app\"){{ {root}(number:1){{ {edge}(first:5){{ nodes{{ {inner} }} }} }} }} }}"
            ),
        )
    };
    let named = "number title url state repository{ nameWithOwner }";
    for (root, edge) in [
        ("pullRequest", "closingIssuesReferences"),
        ("issue", "closedByPullRequestsReferences"),
    ] {
        assert!(
            matches!(reach(root, edge, named), Outcome::Forward(_)),
            "{edge}"
        );
        for secret in [
            "body",
            "bodyText",
            "bodyHTML",
            "comments(first:5){ nodes{ body } }",
            "author{ login }",
            "labels(first:5){ nodes{ name } }",
            "reactionGroups{ content }",
        ] {
            let out = reach(root, edge, &format!("{named} {secret}"));
            assert!(
                is_refused(&out, Refusal::GraphqlOtherRepo),
                "{edge} {secret}: {out:?}"
            );
        }
    }
    for (root, inner) in [
        (
            "ref(qualifiedName:\"refs/heads/main\"){ associatedPullRequests(first:5){ nodes{ body } } }",
            "repository",
        ),
        (
            "ref(qualifiedName:\"refs/heads/main\"){ associatedPullRequests(first:5){ nodes{ comments(first:1){ totalCount } } } }",
            "repository",
        ),
    ] {
        let q = format!("query{{ repository(owner:\"acme\", name:\"app\"){{ {root} }} }}");
        let out = gh(&s, &q);
        assert!(
            is_refused(&out, Refusal::GraphqlOtherRepo),
            "{root} {inner}: {out:?}"
        );
    }
    let ok = "query{ repository(owner:\"acme\", name:\"app\"){ ref(qualifiedName:\"refs/heads/main\"){ associatedPullRequests(first:5){ nodes{ number title url state repository{ nameWithOwner } } } } } }";
    assert!(matches!(gh(&s, ok), Outcome::Forward(_)));
}

#[test]
fn the_schemas_own_description_is_read_only_metadata_and_is_answered() {
    purlis_core::unsteered!();
    let s = scene();
    for q in [
        "query{ __type(name:\"PullRequest\"){ fields{ name } } }",
        "query{ A: __type(name:\"Issue\"){ fields{ name type{ name kind ofType{ name } } } } B: __type(name:\"Repository\"){ fields{ name } } }",
        "query{ __schema{ queryType{ name } types{ name kind } } }",
        "query{ __type(name:\"CreateIssueInput\"){ inputFields{ name } } }",
    ] {
        assert!(matches!(gh(&s, q), Outcome::Forward(_)), "{q}");
    }
    // It is metadata and no more: a field the specification does not have is not one.
    for q in [
        "query{ __type(name:\"Repository\"){ secrets } }",
        "query{ __schema{ repositories{ name } } }",
    ] {
        let out = gh(&s, q);
        assert!(is_refused(&out, Refusal::GraphqlOtherRepo), "{q}: {out:?}");
    }
}

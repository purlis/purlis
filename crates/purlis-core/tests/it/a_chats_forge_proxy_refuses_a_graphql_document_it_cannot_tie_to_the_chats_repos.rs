// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! One login, slice A: GraphQL is parsed, and what is not known to be the chat's is refused (ADR
//! 0128 D2; Open item 6: the list someone keeps, which fails closed).

use purlis_core::forge::proxy::{Outcome, Refusal};

use crate::support::forge_proxy::{Scene, graphql, is_refused};

fn gh(s: &Scene, query: &str, vars: serde_json::Value) -> Outcome {
    s.send("POST", "api.github.com", "/graphql", &graphql(query, vars))
}

#[test]
fn a_mutation_that_is_not_on_the_list_is_refused() {
    purlis_core::unsteered!();
    let s = Scene::single();
    for q in [
        "mutation{ deleteIssue(input:{issueId:\"x\"}){ clientMutationId } }",
        "mutation{ addStar(input:{starrableId:\"R_acme_app\"}){ clientMutationId } }",
        "mutation{ createRef(input:{repositoryId:\"R_acme_app\", name:\"refs/heads/x\", oid:\"abc\"}){ clientMutationId } }",
        "mutation{ deleteRef(input:{refId:\"x\"}){ clientMutationId } }",
        "mutation{ updateBranchProtectionRuleX(input:{repositoryId:\"R_acme_app\"}){ clientMutationId } }",
        "mutation{ somethingNewGitHubShipped(input:{repositoryId:\"R_acme_app\"}){ clientMutationId } }",
    ] {
        let out = gh(&s, q, serde_json::json!({}));
        assert!(
            is_refused(&out, Refusal::GraphqlUnknownMutation) || is_refused(&out, Refusal::Ruleset),
            "{q}: {out:?}"
        );
    }
    let out = gh(
        &s,
        "mutation{ somethingNewGitHubShipped(input:{repositoryId:\"R_acme_app\"}){ clientMutationId } }",
        serde_json::json!({}),
    );
    assert!(is_refused(&out, Refusal::GraphqlUnknownMutation), "{out:?}");
}

#[test]
fn one_bad_operation_among_good_ones_refuses_the_whole_document() {
    purlis_core::unsteered!();
    let s = Scene::single();
    let q = "query A{ viewer{login} } mutation B{ somethingNew(input:{repositoryId:\"R_acme_app\"}){ clientMutationId } }";
    let out = gh(&s, q, serde_json::json!({}));
    assert!(is_refused(&out, Refusal::GraphqlUnknownMutation), "{out:?}");
    // A mutation that is on the list, beside a merge, is refused by the merge.
    let q = "mutation{ createIssue(input:{repositoryId:\"R_acme_app\", title:\"t\"}){ clientMutationId } mergePullRequest(input:{pullRequestId:\"x\"}){ clientMutationId } }";
    let out = gh(&s, q, serde_json::json!({}));
    assert!(is_refused(&out, Refusal::Merge), "{out:?}");
}

#[test]
fn a_mutation_naming_another_repo_or_nothing_it_can_be_tied_to_is_refused() {
    purlis_core::unsteered!();
    let s = Scene::single();
    for q in [
        "mutation{ createIssue(input:{repositoryId:\"R_someone_elses\", title:\"t\"}){ clientMutationId } }",
        "mutation{ createIssue(input:{title:\"t\"}){ clientMutationId } }",
        "mutation{ addComment(input:{subjectId:\"PR_unknown\", body:\"x\"}){ clientMutationId } }",
        "mutation{ createIssue(input:{repositoryId:\"R_acme_app\", labelIds:[\"L_1\"], title:\"t\"}){ clientMutationId } }",
    ] {
        let out = gh(&s, q, serde_json::json!({}));
        assert!(is_refused(&out, Refusal::GraphqlOtherRepo), "{q}: {out:?}");
    }
    let out = s.send(
        "POST",
        "gitlab.com",
        "/api/graphql",
        &graphql("mutation{ mergeRequestUpdate(input:{projectPath:\"group/proj-x\", iid:\"1\", title:\"t\"}){ errors } }", serde_json::json!({})),
    );
    assert!(is_refused(&out, Refusal::GraphqlOtherRepo), "{out:?}");
    let out = s.send(
        "POST",
        "gitlab.com",
        "/api/graphql",
        &graphql("mutation{ mergeRequestUpdate(input:{projectPath:\"group/proj\", iid:\"1\", title:\"t\"}){ errors } }", serde_json::json!({})),
    );
    assert!(matches!(out, Outcome::Forward(_)), "{out:?}");
}

#[test]
fn a_query_that_reaches_another_repo_or_a_way_out_of_the_repo_is_refused() {
    purlis_core::unsteered!();
    let s = Scene::single();
    for q in [
        "query{ repository(owner:\"acme\", name:\"other\"){ name } }",
        "query{ repository(owner:\"acme\", name:\"app-secrets\"){ name } }",
        "query{ repository(owner:\"acme\"){ name } }",
        "query{ organization(login:\"acme\"){ repositories(first:100){ totalCount } } }",
        "query{ user(login:\"someone\"){ login } }",
        "query{ node(id:\"R_someone_elses\"){ id } }",
        "query{ nodes(ids:[\"a\"]){ id } }",
        "query{ search(query:\"x\", type:REPOSITORY, first:5){ repositoryCount } }",
        "query{ viewer{ repositories(first:100){ totalCount } } }",
        "query{ viewer{ login ...F } } fragment F on User{ email }",
        "query{ repository(owner:\"acme\", name:\"app\"){ parent{ pullRequests(first:1){ totalCount } } } }",
        "query{ repository(owner:\"acme\", name:\"app\"){ ...F } } fragment F on Repository{ forks(first:1){ totalCount } }",
        "query{ repository(owner:\"acme\", name:\"app\"){ pullRequest(number:1){ headRepository{ collaborators(first:5){ totalCount } } } } }",
        "query{ __schema{ types{ name } } }",
        "query{ ...F } fragment F on Query{ viewer{ login } }",
    ] {
        let out = gh(&s, q, serde_json::json!({}));
        assert!(is_refused(&out, Refusal::GraphqlOtherRepo), "{q}: {out:?}");
    }
}

#[test]
fn a_variable_is_read_from_the_request_and_one_with_no_value_is_refused() {
    purlis_core::unsteered!();
    let s = Scene::single();
    let q = "query($o:String!,$n:String!){ repository(owner:$o,name:$n){ name } }";
    assert!(matches!(
        gh(&s, q, serde_json::json!({"o":"acme","n":"app"})),
        Outcome::Forward(_)
    ));
    for vars in [
        serde_json::json!({"o":"acme","n":"other"}),
        serde_json::json!({"o":"acme"}),
        serde_json::json!({}),
        serde_json::json!({"o":["acme"],"n":"app"}),
    ] {
        assert!(
            is_refused(&gh(&s, q, vars.clone()), Refusal::GraphqlOtherRepo),
            "{vars}"
        );
    }
    let defaulted = "query($n:String=\"app\"){ repository(owner:\"acme\", name:$n){ name } }";
    assert!(matches!(
        gh(&s, defaulted, serde_json::json!({})),
        Outcome::Forward(_)
    ));
}

#[test]
fn a_document_that_is_not_one_request_is_refused() {
    purlis_core::unsteered!();
    let s = Scene::single();
    let post = |body: &[u8]| s.send("POST", "api.github.com", "/graphql", body);
    for (name, body) in [
        ("not json", b"query{viewer{login}}".to_vec()),
        ("a batch", br#"[{"query":"query{viewer{login}}"}]"#.to_vec()),
        ("no query", br#"{"operationName":"x"}"#.to_vec()),
        (
            "a persisted query",
            br#"{"extensions":{"persistedQuery":{"sha256Hash":"abc"}}}"#.to_vec(),
        ),
        (
            "variables that are not an object",
            br#"{"query":"query{viewer{login}}","variables":[1]}"#.to_vec(),
        ),
        ("empty", Vec::new()),
    ] {
        let out = post(&body);
        assert!(
            is_refused(&out, Refusal::GraphqlParse) || is_refused(&out, Refusal::BodyUnreadable),
            "{name}: {out:?}"
        );
    }
    assert!(is_refused(
        &gh(&s, "query{ viewer{ login }", serde_json::json!({})),
        Refusal::GraphqlParse
    ));
    assert!(is_refused(
        &gh(&s, "subscription{ x }", serde_json::json!({})),
        Refusal::NotOnList
    ));
    let big = vec![b' '; (1 << 20) + 1];
    assert!(is_refused(&post(&big), Refusal::BodyUnreadable));
}

#[test]
fn graphql_is_a_post_to_the_forges_one_address_only() {
    purlis_core::unsteered!();
    let s = Scene::single();
    let body = graphql("query{ viewer{ login } }", serde_json::json!({}));
    assert!(is_refused(
        &s.send("GET", "api.github.com", "/graphql", &body),
        Refusal::NotOnList
    ));
    assert!(is_refused(
        &s.send("POST", "api.github.com", "/graphql/extra", &body),
        Refusal::NotOnList
    ));
    assert!(
        is_refused(
            &s.send("POST", "github.com", "/graphql", &body),
            Refusal::NotOnList
        ) || is_refused(
            &s.send("POST", "github.com", "/graphql", &body),
            Refusal::OtherRepo
        )
    );
    // Two GitHub hosts recorded: a document that names no repo cannot say which is meant.
    let two = Scene::with_two_github_hosts(
        purlis_core::forge::proxy::TokenKind::Oauth,
        crate::support::forge_proxy::Objects::default(),
    );
    assert!(is_refused(
        &two.send("POST", "api.github.com", "/graphql", &body),
        Refusal::AmbiguousHost
    ));
    assert!(is_refused(
        &two.send("POST", "ghe.corp.example", "/api/graphql", &body),
        Refusal::AmbiguousHost
    ));
    assert!(matches!(
        s.send("POST", "api.github.com", "/graphql", &body),
        Outcome::Forward(_)
    ));
}

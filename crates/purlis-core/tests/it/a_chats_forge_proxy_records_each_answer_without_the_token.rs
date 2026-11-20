// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! One login, slice A: every answer is recorded for the chat that asked, as method, host, class
//! and verdict, and the record holds no token, sentinel, path or query (ADR 0128 D2: audit).

use purlis_core::forge::proxy::{AuditLog, Class, Refusal, Verdict};

use crate::support::forge_proxy::{REAL, Scene, push_body};

#[test]
fn an_allowed_and_a_refused_request_are_each_recorded_with_their_verdict() {
    let s = Scene::new();
    let _ = s.send(
        "GET",
        "api.github.com",
        "/repos/acme/app/pulls?access_token=SECRETQUERY",
        b"",
    );
    let _ = s.send(
        "PUT",
        "api.github.com",
        "/repos/acme/app/pulls/7/merge",
        b"",
    );
    let _ = s.send("GET", "api.github.com", "/repos/acme/other/pulls", b"");
    let log = s.proxy.audit(&s.a);
    assert_eq!(log.len(), 3);
    assert_eq!(log[0].method, "GET");
    assert_eq!(log[0].host, "api.github.com");
    assert_eq!(log[0].repo.as_deref(), Some("acme/app"));
    assert_eq!(log[0].class, Some(Class::ApiRead));
    assert_eq!(log[0].verdict, Verdict::Allowed);
    assert_eq!(log[1].verdict, Verdict::Refused(Refusal::Merge));
    assert_eq!(log[2].verdict, Verdict::Refused(Refusal::OtherRepo));
}

#[test]
fn a_refused_request_for_a_missing_sentinel_is_recorded_too() {
    let s = Scene::new();
    let out = s.send_as(
        &s.a,
        "GET",
        "api.github.com",
        443,
        "/repos/acme/app/pulls",
        crate::support::forge_proxy::Auth::None,
        b"",
    );
    assert!(matches!(
        out,
        purlis_core::forge::proxy::Outcome::Refused(Refusal::NoSentinel)
    ));
    assert_eq!(
        s.proxy.audit(&s.a)[0].verdict,
        Verdict::Refused(Refusal::NoSentinel)
    );
}

#[test]
fn a_record_holds_neither_a_token_a_path_nor_a_query() {
    let s = Scene::new();
    let _ = s.send(
        "GET",
        "api.github.com",
        "/repos/acme/app/pulls?access_token=SECRETQUERY",
        b"",
    );
    let _ = s.send(
        "POST",
        "github.com",
        "/acme/app.git/git-receive-pack",
        &push_body(&[("create", "refs/heads/feature-SECRETBRANCH")]),
    );
    let all = format!("{:?}", s.proxy.audit(&s.a));
    for needle in [
        REAL,
        s.a_sentinel.expose(),
        s.b_sentinel.expose(),
        "SECRETQUERY",
        "SECRETBRANCH",
        "/pulls",
        "access_token",
    ] {
        assert!(!all.contains(needle), "the audit holds {needle}: {all}");
    }
}

#[test]
fn a_graphql_mutation_is_recorded_by_its_name_alone() {
    let s = Scene::single();
    let body = crate::support::forge_proxy::graphql(
        "mutation{ createIssue(input:{repositoryId:\"R_acme_app\", title:\"PRIVATETITLE\"}){ clientMutationId } }",
        serde_json::json!({}),
    );
    let _ = s.send("POST", "api.github.com", "/graphql", &body);
    let log = s.proxy.audit(&s.a);
    assert_eq!(
        log[0].class,
        Some(Class::GraphqlMutation("createIssue".into()))
    );
    assert!(!format!("{log:?}").contains("PRIVATETITLE"));
}

#[test]
fn each_chats_record_is_its_own() {
    let s = Scene::new();
    let _ = s.send("GET", "api.github.com", "/repos/acme/app/pulls", b"");
    assert_eq!(s.proxy.audit(&s.a).len(), 1);
    assert!(s.proxy.audit(&s.b).is_empty());
}

#[test]
fn a_chats_record_outlives_the_chat() {
    let s = Scene::new();
    let _ = s.send("GET", "api.github.com", "/repos/acme/app/pulls", b"");
    s.proxy.retire(&s.a);
    assert_eq!(s.proxy.audit(&s.a).len(), 1);
}

#[test]
fn the_record_keeps_the_newest_and_drops_the_oldest_past_its_cap() {
    let s = Scene::new();
    for _ in 0..AuditLog::CAP + 5 {
        let _ = s.send("GET", "api.github.com", "/repos/acme/app/pulls", b"");
    }
    assert_eq!(s.proxy.audit(&s.a).len(), AuditLog::CAP);
}

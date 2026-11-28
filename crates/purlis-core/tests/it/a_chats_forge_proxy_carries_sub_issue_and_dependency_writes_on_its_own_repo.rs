// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! GitHub's issue relations on the chat's own repo (#8022; ADR 0128 D3d): a sub-issue added,
//! removed or reprioritised, and a blocking link added or removed, are carried as an issue
//! update is. Each names its other issues by GitHub's numeric id, which can be an issue of
//! another repo, so every id is placed by asking the forge which repo the issue is in; one the
//! route can't place in the chat's own repo refuses the write, as does the same write on another
//! repo.

use base64::Engine as _;
use purlis_core::forge::proxy::{Class, Outcome, Refusal};

use crate::support::forge_proxy::{Objects, Scene, is_refused};

/// The chat's own issues, by GitHub's numeric id.
const OWN: u64 = 1001;
const OWN_TOO: u64 = 1002;
/// An issue of another repo of the same owner.
const OTHERS: u64 = 2001;
/// An id the forge places nowhere.
const NOWHERE: u64 = 3001;

/// The node id the forge reads an issue's numeric id as: `05:Issue<id>`, base64.
fn node(id: u64) -> String {
    base64::engine::general_purpose::STANDARD.encode(format!("05:Issue{id}"))
}

fn scene() -> Scene {
    let mut objects = Objects::default();
    objects.0.insert(node(OWN), "acme/app".into());
    objects.0.insert(node(OWN_TOO), "acme/app".into());
    objects.0.insert(node(OTHERS), "acme/other".into());
    Scene::with(purlis_core::forge::proxy::TokenKind::Oauth, objects)
}

fn send(s: &Scene, method: &str, target: &str, body: serde_json::Value) -> Outcome {
    let body = if body.is_null() {
        Vec::new()
    } else {
        body.to_string().into_bytes()
    };
    s.send(method, "api.github.com", target, &body)
}

/// Each relation write, on `repo`, naming `id` wherever it names another issue.
fn writes(repo: &str, id: u64) -> Vec<(&'static str, String, serde_json::Value)> {
    let issue = format!("/repos/{repo}/issues/5");
    vec![
        (
            "POST",
            format!("{issue}/sub_issues"),
            serde_json::json!({ "sub_issue_id": id }),
        ),
        (
            "DELETE",
            format!("{issue}/sub_issue"),
            serde_json::json!({ "sub_issue_id": id }),
        ),
        (
            "PATCH",
            format!("{issue}/sub_issues/priority"),
            serde_json::json!({ "sub_issue_id": id, "after_id": id }),
        ),
        (
            "POST",
            format!("{issue}/dependencies/blocked_by"),
            serde_json::json!({ "issue_id": id }),
        ),
        (
            "DELETE",
            format!("{issue}/dependencies/blocked_by/{id}"),
            serde_json::Value::Null,
        ),
    ]
}

#[test]
fn each_sub_issue_and_dependency_write_on_the_chats_own_repo_is_carried_as_an_issue_update() {
    purlis_core::unsteered!();
    let s = scene();
    for (method, target, body) in writes("acme/app", OWN) {
        match send(&s, method, &target, body.clone()) {
            Outcome::Forward(allowed) => {
                assert_eq!(allowed.class, Class::IssueWrite, "{method} {target}")
            }
            other => panic!("{method} {target} {body}: not carried: {other:?}"),
        }
    }
    // Moved before another sub-issue rather than after one.
    let out = send(
        &s,
        "PATCH",
        "/repos/acme/app/issues/5/sub_issues/priority",
        serde_json::json!({ "sub_issue_id": OWN, "before_id": OWN_TOO }),
    );
    assert!(matches!(out, Outcome::Forward(_)), "{out:?}");
}

#[test]
fn each_sub_issue_and_dependency_write_on_another_repo_is_refused() {
    purlis_core::unsteered!();
    let s = scene();
    for (method, target, body) in writes("acme/other", OTHERS) {
        let out = send(&s, method, &target, body.clone());
        assert!(
            is_refused(&out, Refusal::OtherRepo),
            "{method} {target} {body}: {out:?}"
        );
    }
}

#[test]
fn a_relation_write_naming_an_issue_of_another_repo_or_of_none_is_refused() {
    purlis_core::unsteered!();
    let s = scene();
    for id in [OTHERS, NOWHERE] {
        for (method, target, body) in writes("acme/app", id) {
            let out = send(&s, method, &target, body.clone());
            assert!(
                is_refused(&out, Refusal::OtherRepo),
                "{method} {target} {body}: {out:?}"
            );
        }
    }
    // One id placed and the other not refuses the write.
    for body in [
        serde_json::json!({ "sub_issue_id": OWN, "after_id": OTHERS }),
        serde_json::json!({ "sub_issue_id": OWN, "before_id": NOWHERE }),
        serde_json::json!({ "sub_issue_id": OTHERS, "after_id": OWN }),
    ] {
        let out = send(
            &s,
            "PATCH",
            "/repos/acme/app/issues/5/sub_issues/priority",
            body.clone(),
        );
        assert!(is_refused(&out, Refusal::OtherRepo), "{body}: {out:?}");
    }
}

#[test]
fn a_relation_write_with_a_body_the_route_cannot_read_as_ids_is_refused() {
    purlis_core::unsteered!();
    let s = scene();
    let sub_issues = "/repos/acme/app/issues/5/sub_issues";
    let blocked_by = "/repos/acme/app/issues/5/dependencies/blocked_by";
    for (method, target, body) in [
        // No id at all.
        ("POST", sub_issues, serde_json::Value::Null),
        ("POST", sub_issues, serde_json::json!({})),
        ("POST", blocked_by, serde_json::json!({})),
        // An id that is no id.
        (
            "POST",
            sub_issues,
            serde_json::json!({ "sub_issue_id": -1 }),
        ),
        ("POST", sub_issues, serde_json::json!({ "sub_issue_id": 0 })),
        (
            "POST",
            sub_issues,
            serde_json::json!({ "sub_issue_id": 1.5 }),
        ),
        (
            "POST",
            sub_issues,
            serde_json::json!({ "sub_issue_id": "1001 " }),
        ),
        (
            "POST",
            sub_issues,
            serde_json::json!({ "sub_issue_id": [OWN] }),
        ),
        ("POST", blocked_by, serde_json::json!({ "issue_id": null })),
        // A key the route does not read.
        (
            "POST",
            sub_issues,
            serde_json::json!({ "sub_issue_id": OWN, "issue_id": OTHERS }),
        ),
        (
            "POST",
            blocked_by,
            serde_json::json!({ "issue_id": OWN, "repository_id": 7 }),
        ),
        // Moving the sub-issue from its parent, which can be another repo's issue, whatever it
        // says.
        (
            "POST",
            sub_issues,
            serde_json::json!({ "sub_issue_id": OWN, "replace_parent": true }),
        ),
        (
            "POST",
            sub_issues,
            serde_json::json!({ "sub_issue_id": OWN, "replace_parent": false }),
        ),
        // An id as a string (`gh api -f`), however it is spelled: GitHub's schema has an integer.
        (
            "POST",
            blocked_by,
            serde_json::json!({ "issue_id": "1001" }),
        ),
        (
            "POST",
            blocked_by,
            serde_json::json!({ "issue_id": "+1001" }),
        ),
        (
            "POST",
            blocked_by,
            serde_json::json!({ "issue_id": "01001" }),
        ),
        // A removal by path whose id has a leading zero.
        (
            "DELETE",
            "/repos/acme/app/issues/5/dependencies/blocked_by/01001",
            serde_json::Value::Null,
        ),
        // A removal by path carries no body.
        (
            "DELETE",
            "/repos/acme/app/issues/5/dependencies/blocked_by/1001",
            serde_json::json!({ "issue_id": OWN }),
        ),
    ] {
        let out = send(&s, method, target, body.clone());
        assert!(
            is_refused(&out, Refusal::MalformedRequest),
            "{method} {target} {body}: {out:?}"
        );
    }
    // A form, which GitHub's JSON API does not read the way the route would.
    let out = s.send_typed(
        "api.github.com",
        sub_issues,
        "application/x-www-form-urlencoded",
        format!("sub_issue_id={OWN}").as_bytes(),
    );
    assert!(is_refused(&out, Refusal::MalformedRequest), "{out:?}");
}

#[test]
fn a_relation_read_and_a_relation_write_of_a_shape_github_has_not_stay_refused() {
    purlis_core::unsteered!();
    let s = scene();
    for (method, target) in [
        ("GET", "/repos/acme/app/issues/5/sub_issues"),
        ("GET", "/repos/acme/app/issues/5/dependencies/blocked_by"),
        ("POST", "/repos/acme/app/issues/5/dependencies/blocking"),
        ("DELETE", "/repos/acme/app/issues/5/sub_issues"),
        ("PUT", "/repos/acme/app/issues/5/sub_issues"),
        ("POST", "/repos/acme/app/issues/5/parent"),
    ] {
        let body = if method == "GET" {
            serde_json::Value::Null
        } else {
            serde_json::json!({ "sub_issue_id": OWN, "issue_id": OWN })
        };
        let out = send(&s, method, target, body);
        assert!(
            matches!(out, Outcome::Refused(_)),
            "{method} {target}: {out:?}"
        );
    }
}

#[test]
fn a_relation_write_on_gitlab_is_not_carried() {
    purlis_core::unsteered!();
    let s = scene();
    let out = s.send(
        "POST",
        "gitlab.com",
        "/api/v4/projects/42/issues/5/sub_issues",
        br#"{"sub_issue_id": 1001}"#,
    );
    assert!(is_refused(&out, Refusal::NotOnList), "{out:?}");
}

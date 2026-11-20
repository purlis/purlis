// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! One login, slice A2 (instruction 200, re-review of slice A): the spellings a second reader
//! would read differently. A header whose name holds an underscore, a second `Content-Type`, a
//! body on a read, a credential as a body key, a label list a bot reads, a quick action and a line
//! break the proxy did not split on are each refused; what is sent is one `Content-Type`.

use purlis_core::forge::proxy::{Inbound, Outcome, Refusal, RequestHead};

use crate::support::forge_proxy::{Scene, is_refused, push_body};

fn json(v: serde_json::Value) -> Vec<u8> {
    serde_json::to_vec(&v).unwrap()
}

/// A request from chat-a with its own sentinel and these headers besides.
fn send_with(
    s: &Scene,
    method: &str,
    host: &str,
    target: &str,
    extra: &[(&str, &str)],
    body: &[u8],
) -> Outcome {
    let head = RequestHead::new(method, host, 443, target);
    let mut headers = vec![(
        "Authorization".to_string(),
        format!("Bearer {}", s.a_sentinel.expose()),
    )];
    headers.extend(extra.iter().map(|(n, v)| (n.to_string(), v.to_string())));
    s.proxy.handle(
        &s.a,
        &Inbound {
            head: &head,
            headers: &headers,
            body,
        },
    )
}

const COMMENT: &str = "/repos/acme/app/issues/7/comments";

#[test]
fn a_header_whose_name_holds_an_underscore_is_refused() {
    purlis_core::unsteered!();
    let s = Scene::new();
    let body = json(serde_json::json!({"body": "ok"}));
    for name in [
        "Content_Type",
        "content_type",
        "X_Anything",
        "Private_Token",
    ] {
        let out = send_with(
            &s,
            "POST",
            "api.github.com",
            COMMENT,
            &[(name, "application/json")],
            &body,
        );
        assert!(
            is_refused(&out, Refusal::MalformedRequest),
            "{name}: {out:?}"
        );
    }
}

#[test]
fn a_second_content_type_of_any_spelling_is_refused() {
    purlis_core::unsteered!();
    let s = Scene::new();
    let body = json(serde_json::json!({"body": "ok"}));
    for pair in [
        [
            ("Content-Type", "application/json"),
            ("content-type", "application/json"),
        ],
        [
            ("Content-Type", "application/json"),
            ("CONTENT-TYPE", "text/plain"),
        ],
    ] {
        let out = send_with(&s, "POST", "api.github.com", COMMENT, &pair, &body);
        assert!(
            is_refused(&out, Refusal::MalformedRequest),
            "{pair:?}: {out:?}"
        );
    }
}

#[test]
fn a_body_that_was_read_is_sent_with_exactly_one_content_type_of_the_type_it_was_read_by() {
    purlis_core::unsteered!();
    let s = Scene::new();
    let body = json(serde_json::json!({"body": "ok"}));
    let Outcome::Forward(f) = send_with(
        &s,
        "POST",
        "api.github.com",
        COMMENT,
        &[("Content-Type", "Application/JSON; charset=utf-8; x=y")],
        &body,
    ) else {
        panic!("a plain comment is answered");
    };
    let types: Vec<&(String, String)> = f
        .headers
        .iter()
        .filter(|(n, _)| n.to_ascii_lowercase().replace('_', "-") == "content-type")
        .collect();
    assert_eq!(types.len(), 1, "{:?}", f.headers);
    assert_eq!(types[0].1, "application/json");

    let Outcome::Forward(f) = send_with(
        &s,
        "POST",
        "gitlab.com",
        "/api/v4/projects/42/issues/1/notes",
        &[(
            "Content-Type",
            "application/x-www-form-urlencoded; charset=x",
        )],
        b"body=ok",
    ) else {
        panic!("a plain note is answered");
    };
    let types: Vec<&str> = f
        .headers
        .iter()
        .filter(|(n, _)| n.eq_ignore_ascii_case("content-type"))
        .map(|(_, v)| v.as_str())
        .collect();
    assert_eq!(types, ["application/x-www-form-urlencoded"]);
}

#[test]
fn a_read_goes_upstream_with_no_body_whatever_the_chat_sent_and_said() {
    purlis_core::unsteered!();
    let s = Scene::new();
    for extra in [
        vec![
            ("Content-Type", "application/json"),
            ("Content-Length", "7"),
        ],
        vec![("Transfer-Encoding", "chunked")],
        vec![("Content-Length", "0")],
        vec![],
    ] {
        let Outcome::Forward(f) = send_with(
            &s,
            "GET",
            "api.github.com",
            "/repos/acme/app/issues/7",
            &extra,
            b"{\"a\":1}",
        ) else {
            panic!("a read is answered: {extra:?}");
        };
        assert_eq!(f.body.as_deref(), Some(&b""[..]), "{extra:?}");
        let lengths: Vec<&str> = f
            .headers
            .iter()
            .filter(|(n, _)| n.eq_ignore_ascii_case("content-length"))
            .map(|(_, v)| v.as_str())
            .collect();
        assert_eq!(lengths, ["0"], "{extra:?}");
        assert!(
            f.headers.iter().all(|(n, _)| {
                !n.eq_ignore_ascii_case("transfer-encoding")
                    && !n.eq_ignore_ascii_case("content-type")
            }),
            "{extra:?}: {:?}",
            f.headers
        );
    }
}

#[test]
fn a_credential_as_a_key_of_a_json_or_form_body_is_refused() {
    purlis_core::unsteered!();
    let s = Scene::new();
    for key in [
        "private_token",
        "access_token",
        "job_token",
        "token",
        "Token",
    ] {
        for body in [
            json(serde_json::json!({"body": "ok", key: "x"})),
            json(serde_json::json!({"body": "ok", "nested": [{key: "x"}]})),
        ] {
            let out = s.send("POST", "api.github.com", COMMENT, &body);
            assert!(
                is_refused(&out, Refusal::CredentialInBody),
                "{key}: {out:?}"
            );
        }
        let form = format!("body=ok&{key}=x");
        let out = s.send(
            "POST",
            "gitlab.com",
            "/api/v4/projects/42/issues/1/notes",
            form.as_bytes(),
        );
        assert!(
            is_refused(&out, Refusal::CredentialInBody),
            "form {key}: {out:?}"
        );
    }
    // The word as a value, in a sentence, is text.
    let out = s.send(
        "POST",
        "api.github.com",
        COMMENT,
        &json(serde_json::json!({"body": "the token is rotated"})),
    );
    assert!(matches!(out, Outcome::Forward(_)), "{out:?}");
}

#[test]
fn gitlab_label_lists_are_split_and_each_label_is_checked() {
    purlis_core::unsteered!();
    let s = Scene::new();
    let mr = "/api/v4/projects/42/merge_requests/3";
    for body in [
        json(serde_json::json!({"labels": "bug,automerge"})),
        json(serde_json::json!({"add_labels": "bug, Auto_Merge"})),
        json(serde_json::json!({"labels": ["bug", "ready to merge"]})),
        json(serde_json::json!({"add_labels": "ship-it"})),
    ] {
        let out = s.send("PUT", "gitlab.com", mr, &body);
        assert!(is_refused(&out, Refusal::MergeCommand), "{out:?}");
    }
    for form in [
        "labels=bug%2Cautomerge",
        "add_labels=lgtm",
        "labels[]=bug&labels[]=bors",
    ] {
        let out = s.send("PUT", "gitlab.com", mr, form.as_bytes());
        assert!(is_refused(&out, Refusal::MergeCommand), "{form}: {out:?}");
    }
    let out = s.send(
        "PUT",
        "gitlab.com",
        mr,
        &json(serde_json::json!({"labels": "bug,docs", "remove_labels": "wip"})),
    );
    assert!(matches!(out, Outcome::Forward(_)), "{out:?}");
}

#[test]
fn a_label_quick_action_that_names_a_merge_label_is_refused() {
    purlis_core::unsteered!();
    let s = Scene::new();
    for text in [
        "/label ~automerge",
        "ok\n/label ~bug ~\"ready to merge\"",
        "/relabel ~auto-merge",
        "/label ~bug, ~merge-queue",
    ] {
        let out = s.send(
            "POST",
            "gitlab.com",
            "/api/v4/projects/42/merge_requests/3/notes",
            &json(serde_json::json!({"body": text})),
        );
        assert!(is_refused(&out, Refusal::MergeCommand), "{text:?}: {out:?}");
    }
    let out = s.send(
        "POST",
        "gitlab.com",
        "/api/v4/projects/42/merge_requests/3/notes",
        &json(serde_json::json!({"body": "/label ~bug ~docs"})),
    );
    assert!(matches!(out, Outcome::Forward(_)), "{out:?}");
}

#[test]
fn every_line_break_a_reader_splits_on_ends_the_line_a_command_starts() {
    purlis_core::unsteered!();
    let s = Scene::new();
    for brk in ["\u{2028}", "\u{2029}", "\u{b}", "\u{c}", "\u{85}"] {
        let text = format!("looks good{brk}/merge");
        let out = s.send(
            "POST",
            "gitlab.com",
            "/api/v4/projects/42/merge_requests/3/notes",
            &json(serde_json::json!({"body": text})),
        );
        assert!(is_refused(&out, Refusal::MergeCommand), "{brk:?}: {out:?}");
    }
}

#[test]
fn serde_jsons_own_number_key_does_not_read_a_string_as_json() {
    purlis_core::unsteered!();
    let s = Scene::new();
    // The string is JSON of another shape, which would hide a command from the strings read.
    let body = br#"{"$serde_json::private::Number":"{\"body\":\"/merge\"}"}"#;
    let out = s.send("POST", "api.github.com", COMMENT, body);
    assert!(
        is_refused(&out, Refusal::BodyUnreadable) || is_refused(&out, Refusal::MergeCommand),
        "{out:?}"
    );
    // A number, however long, is still a number.
    let big = br#"{"body":"ok","n":123456789012345678901234567890}"#;
    let out = s.send("POST", "api.github.com", COMMENT, big);
    assert!(matches!(out, Outcome::Forward(_)), "{out:?}");
}

#[test]
fn a_branch_named_release_or_deploy_is_not_one_a_chat_may_push_to() {
    purlis_core::unsteered!();
    let s = Scene::new();
    for name in [
        "release",
        "Release",
        "deploy",
        "release-1.2",
        "release-candidate",
        "release/1.2",
        "deploy/prod",
    ] {
        let body = push_body(&[("create", &format!("refs/heads/{name}"))]);
        let out = s.send(
            "POST",
            "github.com",
            "/acme/app.git/git-receive-pack",
            &body,
        );
        assert!(is_refused(&out, Refusal::ProtectedRef), "{name}: {out:?}");
    }
    let body = push_body(&[("create", "refs/heads/fix-release-notes")]);
    let out = s.send(
        "POST",
        "github.com",
        "/acme/app.git/git-receive-pack",
        &body,
    );
    assert!(matches!(out, Outcome::Forward(_)), "{out:?}");
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! One login, slice A (review 194): what the proxy checks is what the forge reads. A query is read
//! like a body, a body is read by its type and sent on as what was read, a header the two could
//! read differently is refused, and a path is decoded before it is judged.

use std::time::Duration;

use purlis_core::forge::proxy::{ForgeProxy, Inbound, Outcome, Refusal, RequestHead};

use crate::support::forge_proxy::{
    Auth, Creds, Objects, REAL, Scene, graphql, headers, is_refused,
};

fn post(s: &Scene, target: &str, content_type: Option<&str>, body: &[u8]) -> Outcome {
    let head =
        RequestHead::new("POST", "api.github.com", 443, target).with_content_type(content_type);
    let mut h = headers(Auth::Bearer(s.a_sentinel.expose()));
    if let Some(ct) = content_type {
        h.push(("Content-Type".into(), ct.into()));
    }
    s.proxy.handle(
        &s.a,
        &Inbound {
            head: &head,
            headers: &h,
            body,
        },
    )
}

const ISSUES: &str = "/repos/acme/app/issues";

#[test]
fn a_query_on_the_graphql_address_is_refused_and_on_a_write_too() {
    purlis_core::unsteered!();
    let s = Scene::single();
    let doc = graphql("query{ viewer{ login } }", serde_json::json!({}));
    let out = s.send("POST", "api.github.com", "/graphql?query=x", &doc);
    assert!(is_refused(&out, Refusal::MalformedRequest), "{out:?}");
    for target in [
        "/repos/acme/app/issues?title=x",
        "/repos/acme/app/issues?labels=automerge",
        "/repos/acme/app/pulls?base=main&draft=false",
    ] {
        let out = post(&s, target, Some("application/json"), b"{}");
        assert!(
            is_refused(&out, Refusal::MalformedRequest),
            "{target}: {out:?}"
        );
    }
    // A git request carries `service` and nothing else.
    let out = s.send(
        "GET",
        "github.com",
        "/acme/app.git/info/refs?service=git-upload-pack&x=1",
        b"",
    );
    assert!(is_refused(&out, Refusal::NotOnList), "{out:?}");
}

#[test]
fn a_credential_in_the_query_is_refused_on_every_route() {
    purlis_core::unsteered!();
    let s = Scene::single();
    for name in [
        "access_token",
        "private_token",
        "job_token",
        "deploy_token",
        "token",
        "oauth_token",
        "API_KEY",
        "client_secret",
        "private-token",
    ] {
        for (host, target) in [
            ("api.github.com", format!("/repos/acme/app/pulls?{name}=x")),
            (
                "gitlab.com",
                format!("/api/v4/projects/42/pipelines?{name}=x"),
            ),
            (
                "github.com",
                format!("/acme/app.git/info/refs?service=git-upload-pack&{name}=x"),
            ),
            // The anonymous route too: a public read does not carry a login either.
            (
                "github.com",
                format!("/rust-lang/rust.git/info/refs?service=git-upload-pack&{name}=x"),
            ),
            ("api.github.com", format!("/user?{name}=x")),
        ] {
            let out = s.send("GET", host, &target, b"");
            assert!(
                is_refused(&out, Refusal::CredentialInQuery),
                "{target}: {out:?}"
            );
        }
    }
}

#[test]
fn a_write_body_is_read_by_its_content_type_and_nothing_else_is_read() {
    purlis_core::unsteered!();
    let s = Scene::single();
    assert!(matches!(
        post(
            &s,
            ISSUES,
            Some("application/json; charset=utf-8"),
            br#"{"title":"t"}"#
        ),
        Outcome::Forward(_)
    ));
    assert!(matches!(
        post(
            &s,
            ISSUES,
            Some("application/x-www-form-urlencoded"),
            b"title=t"
        ),
        Outcome::Forward(_)
    ));
    for (ct, body) in [
        (None, &br#"{"title":"t"}"#[..]),
        (Some("text/plain"), &br#"{"title":"t"}"#[..]),
        (Some("application/octet-stream"), &b"x"[..]),
        (Some("application/json"), &b"{not json"[..]),
    ] {
        let out = post(&s, ISSUES, ct, body);
        assert!(is_refused(&out, Refusal::BodyUnreadable), "{ct:?}: {out:?}");
    }
    // A multipart body is named for what it is, and is carried only as one file to an upload.
    let out = post(
        &s,
        ISSUES,
        Some("multipart/form-data; boundary=x"),
        b"--x\r\n\r\n--x--",
    );
    assert!(is_refused(&out, Refusal::Multipart), "{out:?}");
}

#[test]
fn a_json_body_that_repeats_a_key_is_refused_because_two_readers_pick_two_values() {
    purlis_core::unsteered!();
    let s = Scene::single();
    for body in [
        &br#"{"body":"fine","body":"/merge"}"#[..],
        &br#"{"a":{"title":"x","title":"y"}}"#[..],
        &br#"{"labels":[{"name":"a","name":"automerge"}]}"#[..],
    ] {
        let out = post(&s, ISSUES, Some("application/json"), body);
        assert!(is_refused(&out, Refusal::BodyUnreadable), "{out:?}");
    }
}

#[test]
fn what_the_forge_is_sent_is_the_body_as_it_was_read_and_not_the_chats_bytes() {
    purlis_core::unsteered!();
    let s = Scene::single();
    let sent = b"{ \"title\" : \"t\" ,\n \"n\": 42,  \"big\": 12345678901234567890 }";
    let head = RequestHead::new("POST", "api.github.com", 443, ISSUES)
        .with_content_type(Some("application/json"));
    let mut h = headers(Auth::Bearer(s.a_sentinel.expose()));
    h.push(("Content-Type".into(), "application/json".into()));
    h.push(("Content-Length".into(), sent.len().to_string()));
    let Outcome::Forward(f) = s.proxy.handle(
        &s.a,
        &Inbound {
            head: &head,
            headers: &h,
            body: sent,
        },
    ) else {
        panic!("refused")
    };
    let body = f.body.expect("a body that was read is sent as read");
    assert_eq!(body, br#"{"title":"t","n":42,"big":12345678901234567890}"#);
    let lengths: Vec<_> = f
        .headers
        .iter()
        .filter(|(n, _)| n.eq_ignore_ascii_case("content-length"))
        .collect();
    assert_eq!(lengths.len(), 1);
    assert_eq!(lengths[0].1, body.len().to_string());
}

#[test]
fn a_graphql_document_is_sent_as_read_too() {
    purlis_core::unsteered!();
    let s = Scene::single();
    let doc = graphql("query{ viewer{ login } }", serde_json::json!({}));
    let Outcome::Forward(f) = s.send("POST", "api.github.com", "/graphql", &doc) else {
        panic!("refused")
    };
    let sent: serde_json::Value = serde_json::from_slice(&f.body.unwrap()).unwrap();
    assert_eq!(sent["query"], "query{ viewer{ login } }");
}

#[test]
fn a_write_body_over_the_limit_is_refused_and_never_scanned_in_part() {
    purlis_core::unsteered!();
    let s = Scene::single();
    // The command is past the first mebibyte: a scan of a prefix would not see it.
    let mut text = "x".repeat(1 << 20);
    text.push_str("\\n/merge");
    let body = format!(r#"{{"body":"{text}"}}"#);
    let out = post(
        &s,
        "/repos/acme/app/issues/7/comments",
        Some("application/json"),
        body.as_bytes(),
    );
    assert!(is_refused(&out, Refusal::BodyUnreadable), "{out:?}");
}

#[test]
fn a_mutation_may_not_ask_back_for_a_way_out_of_the_repo() {
    purlis_core::unsteered!();
    let s = Scene::single();
    for back in [
        "issue{ author{ repositories(first:1){ totalCount } } }",
        "issue{ repository{ parent{ name } } }",
        "issue{ repository{ rulesets(first:1){ totalCount } } }",
        "issue{ repository{ environments(first:1){ totalCount } } }",
        "issue{ repository{ branchProtectionRules(first:1){ totalCount } } }",
    ] {
        let q = format!(
            "mutation{{ createIssue(input:{{repositoryId:\"R_acme_app\", title:\"t\"}}){{ {back} }} }}"
        );
        let out = s.send(
            "POST",
            "api.github.com",
            "/graphql",
            &graphql(&q, serde_json::json!({})),
        );
        assert!(
            is_refused(&out, Refusal::GraphqlOtherRepo),
            "{back}: {out:?}"
        );
    }
    let ok = "mutation{ createIssue(input:{repositoryId:\"R_acme_app\", title:\"t\"}){ issue{ number url } } }";
    assert!(matches!(
        s.send(
            "POST",
            "api.github.com",
            "/graphql",
            &graphql(ok, serde_json::json!({}))
        ),
        Outcome::Forward(_)
    ));
}

#[test]
fn a_read_is_answered_only_where_a_list_names_it() {
    purlis_core::unsteered!();
    let s = Scene::single();
    for (host, target) in [
        ("api.github.com", "/repos/acme/app/secret-scanning/alerts"),
        ("api.github.com", "/repos/acme/app/dependabot/alerts"),
        ("api.github.com", "/repos/acme/app/traffic/views"),
        ("api.github.com", "/repos/acme/app/code-scanning/alerts"),
        (
            "api.github.com",
            "/repos/acme/app/actions/oidc/customization/sub",
        ),
        ("api.github.com", "/repos/acme/app/tarball/main"),
        ("api.github.com", "/repos/acme/app/zipball/main"),
        ("api.github.com", "/repos/acme/app/events"),
        ("gitlab.com", "/api/v4/projects/42/export"),
        ("gitlab.com", "/api/v4/projects/42/export/download"),
        ("gitlab.com", "/api/v4/projects/42/access_requests"),
        ("gitlab.com", "/api/v4/projects/42/snippets"),
        ("gitlab.com", "/api/v4/projects/42/triggers"),
        ("gitlab.com", "/api/v4/projects/42/pipeline_schedules"),
        ("gitlab.com", "/api/v4/projects/42/repository/archive"),
    ] {
        let out = s.send("GET", host, target, b"");
        assert!(
            matches!(out, Outcome::Refused(_)),
            "{host}{target}: {out:?}"
        );
    }
    for (host, target) in [
        ("api.github.com", "/repos/acme/app"),
        ("api.github.com", "/repos/acme/app/pulls/7/comments"),
        ("api.github.com", "/repos/acme/app/actions/runs/9/jobs"),
        ("api.github.com", "/repos/acme/app/git/trees/abc"),
        ("gitlab.com", "/api/v4/projects/42/merge_requests/3/notes"),
        ("gitlab.com", "/api/v4/projects/42/repository/tree"),
    ] {
        let out = s.send("GET", host, target, b"");
        assert!(
            matches!(out, Outcome::Forward(_)),
            "{host}{target}: {out:?}"
        );
    }
}

fn with_header(s: &Scene, extra: &[(&str, &str)]) -> Outcome {
    let head = RequestHead::new("GET", "api.github.com", 443, "/repos/acme/app/pulls");
    let mut h = headers(Auth::Bearer(s.a_sentinel.expose()));
    h.extend(extra.iter().map(|(n, v)| (n.to_string(), v.to_string())));
    s.proxy.handle(
        &s.a,
        &Inbound {
            head: &head,
            headers: &h,
            body: b"",
        },
    )
}

#[test]
fn a_host_header_that_disagrees_with_the_request_is_refused() {
    purlis_core::unsteered!();
    let s = Scene::single();
    for host in [
        "evil.example",
        "api.github.com.evil.example",
        "github.com",
        "api.github.com:8443",
        "",
    ] {
        let out = with_header(&s, &[("Host", host)]);
        assert!(
            is_refused(&out, Refusal::MalformedRequest),
            "{host:?}: {out:?}"
        );
    }
    let out = with_header(&s, &[("Host", "api.github.com"), ("host", "evil.example")]);
    assert!(is_refused(&out, Refusal::MalformedRequest), "{out:?}");
    for host in ["api.github.com", "API.GitHub.com:443", "api.github.com."] {
        assert!(
            matches!(with_header(&s, &[("Host", host)]), Outcome::Forward(_)),
            "{host}"
        );
    }
}

#[test]
fn only_a_plain_chunked_transfer_coding_is_carried_and_a_request_body_is_never_encoded() {
    purlis_core::unsteered!();
    let s = Scene::single();
    for coding in [
        "gzip",
        "gzip, chunked",
        "chunked, gzip",
        "identity",
        "CHUNKED x",
        "",
    ] {
        let out = with_header(&s, &[("Transfer-Encoding", coding)]);
        assert!(
            is_refused(&out, Refusal::MalformedRequest),
            "{coding:?}: {out:?}"
        );
    }
    assert!(matches!(
        with_header(&s, &[("Transfer-Encoding", "chunked")]),
        Outcome::Forward(_)
    ));
    for enc in ["gzip", "br", "deflate", "identity"] {
        let out = with_header(&s, &[("Content-Encoding", enc)]);
        assert!(
            is_refused(&out, Refusal::MalformedRequest),
            "{enc}: {out:?}"
        );
    }
    // Git compresses its own fetch request, which is never read.
    let head = RequestHead::new("POST", "github.com", 443, "/acme/app.git/git-upload-pack");
    let mut h = headers(Auth::Bearer(s.a_sentinel.expose()));
    h.push(("Content-Encoding".into(), "gzip".into()));
    let out = s.proxy.handle(
        &s.a,
        &Inbound {
            head: &head,
            headers: &h,
            body: b"",
        },
    );
    assert!(matches!(out, Outcome::Forward(_)), "{out:?}");
}

#[test]
fn a_credential_header_is_one_whichever_way_its_name_is_spelled() {
    purlis_core::unsteered!();
    let s = Scene::single();
    // A name with an underscore is refused whole (slice A2, F1): a forge's front end that reads
    // `_` as `-` and this reading it as another header are two readers.
    for name in [
        "Private_Token",
        "private_token",
        "Job_Token",
        "X_GitHub_Token",
    ] {
        for value in [REAL, s.a_sentinel.expose()] {
            let out = with_header(&s, &[(name, value)]);
            assert!(
                is_refused(&out, Refusal::MalformedRequest),
                "{name}: {out:?}"
            );
        }
    }
    for name in [
        "PRIVATE-TOKEN",
        "private-token",
        "Job-Token",
        "X-GitHub-Token",
    ] {
        let out = with_header(&s, &[(name, REAL)]);
        assert!(is_refused(&out, Refusal::WrongSentinel), "{name}: {out:?}");
        let Outcome::Forward(f) = with_header(&s, &[(name, s.a_sentinel.expose())]) else {
            panic!("{name} with the sentinel was refused");
        };
        assert!(
            f.headers
                .iter()
                .all(|(n, _)| !n.to_ascii_lowercase().replace('_', "-").contains("token")),
            "{name} was forwarded"
        );
    }
}

#[test]
fn a_decoded_slash_is_judged_as_a_path_before_anything_is_matched() {
    purlis_core::unsteered!();
    let s = Scene::single();
    for target in [
        "/api/v4/projects/group%2Fproj%2F..%2Fother/merge_requests",
        "/api/v4/projects/group%2F..%2F..%2Fother/merge_requests",
        "/api/v4/projects/42/repository/files/..%2F..%2Fsecret?ref=main",
        "/api/v4/projects/42/repository/files/a%2F%2E%2E%2Fb?ref=main",
        "/api/v4/projects/42/repository/files/%2Fetc%2Fpasswd?ref=main",
        "/api/v4/projects/42/repository/files/a%2F%2Fb?ref=main",
    ] {
        let out = s.send("GET", "gitlab.com", target, b"");
        assert!(
            is_refused(&out, Refusal::MalformedRequest),
            "{target}: {out:?}"
        );
    }
    for target in [
        "/gitlab-org/gitlab/-/archive/..%2F..%2Fx/a.zip",
        "/gitlab-org/gitlab/-/archive/a%2F%2E%2E%2Fb/a.zip",
    ] {
        let out = s.send("GET", "gitlab.com", target, b"");
        assert!(
            is_refused(&out, Refusal::MalformedRequest),
            "{target}: {out:?}"
        );
    }
    // A file with a slash in its name is still a file.
    let ok = s.send(
        "GET",
        "gitlab.com",
        "/api/v4/projects/42/repository/files/docs%2Fguide.md?ref=main",
        b"",
    );
    assert!(matches!(ok, Outcome::Forward(_)), "{ok:?}");
}

#[test]
fn every_merge_command_and_every_label_that_merges_is_refused_however_its_lines_are_split() {
    purlis_core::unsteered!();
    let s = Scene::single();
    for text in [
        "/rebase",
        "/lock",
        "/ship",
        "@mergify queue",
        "@kodiakhq merge",
        "fine\r/merge",
        "fine\r\n/approve",
        "fine\n\r/lgtm",
        "one\rbors r+\rthree",
    ] {
        let body = serde_json::to_vec(&serde_json::json!({ "body": text })).unwrap();
        let out = post(
            &s,
            "/repos/acme/app/issues/7/comments",
            Some("application/json"),
            &body,
        );
        assert!(is_refused(&out, Refusal::MergeCommand), "{text:?}: {out:?}");
    }
    for label in [
        "automerge",
        "auto-merge",
        "Ready To Merge",
        "ready_to_merge",
        "merge-when-ready",
        "ship it",
        "bors",
        "queue",
    ] {
        for body in [
            serde_json::json!({ "labels": [label] }),
            serde_json::json!({ "labels": ["bug", label], "title": "t" }),
        ] {
            let out = post(
                &s,
                "/repos/acme/app/issues/7/labels",
                Some("application/json"),
                &serde_json::to_vec(&body).unwrap(),
            );
            assert!(is_refused(&out, Refusal::MergeCommand), "{label}: {out:?}");
        }
    }
    let ok = serde_json::to_vec(&serde_json::json!({ "labels": ["bug", "needs review"] })).unwrap();
    assert!(matches!(
        post(
            &s,
            "/repos/acme/app/issues/7/labels",
            Some("application/json"),
            &ok
        ),
        Outcome::Forward(_)
    ));
}

#[test]
fn a_chat_locked_out_by_guessing_is_let_back_in_when_the_lockout_ends() {
    purlis_core::unsteered!();
    let proxy = ForgeProxy::new(
        Box::new(Creds {
            asked: Default::default(),
            token_kind: purlis_core::forge::proxy::TokenKind::Oauth,
        }),
        Box::new(Objects::default()),
    )
    .with_lockout(Duration::from_millis(150));
    let chat = purlis_core::forge::proxy::ChatId::new("c");
    let repo = purlis_core::forge::proxy::AllowedRepo::new(
        purlis_core::forge::proxy::Kind::GitHub,
        "github.com",
        443,
        "acme/app",
        "main",
    )
    .unwrap();
    let sentinel = proxy.enrol(&chat, vec![repo]);
    let head = RequestHead::new("GET", "api.github.com", 443, "/repos/acme/app/pulls");
    let send = |token: &str| {
        let h = headers(Auth::Bearer(token));
        proxy.handle(
            &chat,
            &Inbound {
                head: &head,
                headers: &h,
                body: b"",
            },
        )
    };
    for i in 0..10 {
        let _ = send(&format!("guess-{i}"));
    }
    assert!(is_refused(&send(sentinel.expose()), Refusal::Locked));
    std::thread::sleep(Duration::from_millis(250));
    assert!(matches!(send(sentinel.expose()), Outcome::Forward(_)));
}

#[test]
fn a_read_that_follows_a_cross_reference_into_another_repo_is_refused_on_github() {
    purlis_core::unsteered!();
    let s = Scene::single();
    for target in [
        "/repos/acme/app/issues/5/timeline",
        "/repos/acme/app/issues/5/sub_issues",
        "/repos/acme/app/issues/5/parent",
        "/repos/acme/app/issues/5/dependencies/blocked_by",
        "/repos/acme/app/issues/5/dependencies/blocking",
        "/repos/acme/app/issues/5/events",
        "/repos/acme/app/commits/abc123/pulls",
        "/repos/acme/app/commits/abc123/branches-where-head",
        "/repos/acme/app/compare/main...other:topic",
        "/repos/acme/app/compare/other/app:main...topic",
    ] {
        let out = s.send("GET", "api.github.com", target, b"");
        assert!(matches!(out, Outcome::Refused(_)), "{target}: {out:?}");
    }
    for target in [
        "/repos/acme/app/issues/5",
        "/repos/acme/app/issues/5/comments",
        "/repos/acme/app/pulls/7/files",
        "/repos/acme/app/pulls/7/reviews",
        "/repos/acme/app/commits/abc123",
        "/repos/acme/app/commits/abc123/check-runs",
        "/repos/acme/app/compare/main...acme:topic",
        "/repos/acme/app/compare/main...topic",
    ] {
        let out = s.send("GET", "api.github.com", target, b"");
        assert!(matches!(out, Outcome::Forward(_)), "{target}: {out:?}");
    }
}

#[test]
fn a_read_that_follows_a_cross_reference_into_another_project_is_refused_on_gitlab() {
    purlis_core::unsteered!();
    let s = Scene::single();
    for target in [
        "/api/v4/projects/42/issues/5/links",
        "/api/v4/projects/42/issues/5/related_merge_requests",
        "/api/v4/projects/42/issues/5/closed_by",
        "/api/v4/projects/42/merge_requests/3/closes_issues",
        "/api/v4/projects/42/merge_requests/3/related_issues",
        "/api/v4/projects/42/merge_requests/3/related_merge_requests",
        "/api/v4/projects/42/repository/compare?from=a&to=b&from_project_id=99",
    ] {
        let out = s.send("GET", "gitlab.com", target, b"");
        assert!(matches!(out, Outcome::Refused(_)), "{target}: {out:?}");
    }
    for target in [
        "/api/v4/projects/42/issues/5",
        "/api/v4/projects/42/issues/5/notes",
        "/api/v4/projects/42/merge_requests/3/changes",
        "/api/v4/projects/42/merge_requests/3/discussions",
        "/api/v4/projects/42/repository/compare?from=a&to=b",
        "/api/v4/projects/42/repository/compare?from=a&to=b&from_project_id=42",
    ] {
        let out = s.send("GET", "gitlab.com", target, b"");
        assert!(matches!(out, Outcome::Forward(_)), "{target}: {out:?}");
    }
}

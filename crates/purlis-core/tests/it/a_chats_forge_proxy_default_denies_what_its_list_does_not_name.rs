// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! One login, slice A (instruction 178, items 1 and 5): a route is allowed only if a list names
//! it, and what is sent upstream is the proxy's own address for it, never the chat's.

use purlis_core::forge::proxy::{Forwarded, Inbound, Outcome, Refusal, RequestHead};

use crate::support::forge_proxy::{Auth, Scene, headers, is_refused};

fn fwd(o: Outcome) -> Forwarded {
    match o {
        Outcome::Forward(f) => f,
        Outcome::Refused(r) => panic!("refused: {r:?}"),
    }
}

#[test]
fn the_few_reads_outside_a_repo_that_a_forge_client_needs_are_named_and_answered() {
    let s = Scene::new();
    for (host, target) in [
        ("api.github.com", "/user"),
        ("api.github.com", "/meta"),
        ("api.github.com", "/rate_limit"),
        ("ghe.corp.example", "/api/v3/user"),
        ("gitlab.com", "/api/v4/user"),
        ("gitlab.com", "/api/v4/version"),
    ] {
        let f = fwd(s.send("GET", host, target, b""));
        assert!(f.credentialed, "{host}{target}");
    }
}

#[test]
fn every_other_route_outside_a_repo_is_refused() {
    let s = Scene::new();
    for (method, host, target) in [
        ("POST", "api.github.com", "/user/repos"),
        ("GET", "api.github.com", "/search/code?q=x"),
        ("GET", "api.github.com", "/search/repositories?q=x"),
        ("GET", "api.github.com", "/repositories/123"),
        ("GET", "api.github.com", "/orgs/acme/repos"),
        ("POST", "api.github.com", "/orgs/acme/repos"),
        ("GET", "api.github.com", "/user/repos"),
        ("PATCH", "api.github.com", "/user"),
        ("POST", "gitlab.com", "/api/v4/user/keys"),
        ("POST", "gitlab.com", "/api/v4/projects"),
        ("GET", "gitlab.com", "/api/v4/projects"),
        ("GET", "gitlab.com", "/api/v4/projects/43"),
        (
            "GET",
            "gitlab.com",
            "/api/v4/search?scope=projects&search=x",
        ),
        (
            "POST",
            "api.github.com",
            "/repos/acme/app/forks?organization=acme",
        ),
    ] {
        let out = s.send(method, host, target, b"{}");
        assert!(
            matches!(out, Outcome::Refused(_)),
            "{method} {host}{target}: {out:?}"
        );
    }
}

#[test]
fn a_contents_write_and_a_ref_write_are_refused_even_for_a_branch_the_chat_owns() {
    let s = Scene::new();
    for (method, target) in [
        ("PUT", "/repos/acme/app/contents/README.md"),
        ("PATCH", "/repos/acme/app/git/refs/heads/main"),
        ("POST", "/repos/acme/app/git/refs"),
        ("POST", "/repos/acme/app/merges"),
    ] {
        let out = s.send(method, "api.github.com", target, b"{}");
        assert!(
            matches!(out, Outcome::Refused(_)),
            "{method} {target}: {out:?}"
        );
    }
}

#[test]
fn a_double_encoded_slash_is_not_a_slash() {
    let s = Scene::new();
    for target in [
        "/api/v4/projects/group%252Fproj/merge_requests",
        "/api/v4/projects/group%252Fproj%252Fsub/issues",
    ] {
        let out = s.send("GET", "gitlab.com", target, b"");
        assert!(is_refused(&out, Refusal::OtherRepo), "{target}: {out:?}");
    }
}

#[test]
fn a_gitlab_merge_request_aimed_at_another_project_is_refused() {
    let s = Scene::new();
    let post = |body: &[u8], q: &str| {
        s.send(
            "POST",
            "gitlab.com",
            &format!("/api/v4/projects/42/merge_requests{q}"),
            body,
        )
    };
    assert!(matches!(
        post(
            br#"{"source_branch":"work","target_branch":"main","title":"t"}"#,
            ""
        ),
        Outcome::Forward(_)
    ));
    assert!(matches!(
        post(br#"{"target_project_id":42,"title":"t"}"#, ""),
        Outcome::Forward(_)
    ));
    for (body, q) in [
        (&br#"{"target_project_id":43,"title":"t"}"#[..], ""),
        (&br#"{"target_project_id":"43"}"#[..], ""),
        (&b"target_project_id=43&title=t"[..], ""),
        (&b"{}"[..], "?target_project_id=43"),
    ] {
        let out = post(body, q);
        assert!(
            is_refused(&out, Refusal::OtherRepo),
            "{q} {}: {out:?}",
            String::from_utf8_lossy(body)
        );
    }
}

#[test]
fn what_is_sent_upstream_is_the_proxys_own_address_never_the_chats() {
    let s = Scene::new();
    let up =
        |host: &str, port: u16, target: &str| fwd(s.send_port("GET", host, port, target)).upstream;
    assert_eq!(
        up("API.GitHub.com.", 443, "/repos/ACME/App/pulls?b=1&a=2%203"),
        "https://api.github.com/repos/ACME/App/pulls?b=1&a=2+3"
    );
    assert_eq!(
        up("gitlab.com", 443, "/api/v4/projects/group%2Fproj/pipelines"),
        "https://gitlab.com/api/v4/projects/group%2Fproj/pipelines"
    );
    assert_eq!(
        up(
            "git.corp.example",
            8443,
            "/grp/sub/proj.git/info/refs?service=git-upload-pack"
        ),
        "https://git.corp.example:8443/grp/sub/proj.git/info/refs?service=git-upload-pack"
    );
    assert_eq!(
        up(
            "github.com",
            443,
            "/Acme/App.git/info/refs?service=git-upload-pack"
        ),
        "https://github.com/Acme/App.git/info/refs?service=git-upload-pack"
    );
}

#[test]
fn an_absolute_form_target_and_a_method_override_query_are_refused() {
    let s = Scene::new();
    for target in [
        "https://evil.example/repos/acme/app/pulls",
        "//evil.example/repos/acme/app/pulls",
        "/repos/acme/app/pulls?_method=PUT",
        "/repos/acme/app/pulls?_METHOD=DELETE",
    ] {
        let out = s.send("GET", "api.github.com", target, b"");
        assert!(
            is_refused(&out, Refusal::MalformedRequest),
            "{target}: {out:?}"
        );
    }
}

fn run(s: &Scene, extra: &[(&str, &str)]) -> Outcome {
    let head = RequestHead::new("POST", "api.github.com", 443, "/repos/acme/app/issues");
    let mut h = headers(Auth::Bearer(s.a_sentinel.expose()));
    h.extend(extra.iter().map(|(n, v)| (n.to_string(), v.to_string())));
    s.proxy.handle(
        &s.a,
        &Inbound {
            head: &head,
            headers: &h,
            body: b"{}",
        },
    )
}

#[test]
fn a_request_the_two_ends_could_frame_differently_is_refused() {
    let s = Scene::new();
    for extra in [
        vec![("Content-Length", "2"), ("Transfer-Encoding", "chunked")],
        vec![("content-length", "2"), ("Content-Length", "3")],
        vec![
            ("Transfer-Encoding", "chunked"),
            ("transfer-encoding", "identity"),
        ],
    ] {
        let out = run(&s, &extra);
        assert!(
            is_refused(&out, Refusal::MalformedRequest),
            "{extra:?}: {out:?}"
        );
    }
    assert!(matches!(
        run(&s, &[("Content-Length", "2")]),
        Outcome::Forward(_)
    ));
}

#[test]
fn headers_that_choose_the_host_the_method_or_a_login_are_not_forwarded() {
    let s = Scene::new();
    let f = fwd(run(
        &s,
        &[
            ("Host", "evil.example"),
            ("X-HTTP-Method-Override", "PUT"),
            ("X-Method-Override", "DELETE"),
            ("PRIVATE-TOKEN", s.a_sentinel.expose()),
            ("Cookie", "a=b"),
            ("Content-Length", "2"),
        ],
    ));
    let names: Vec<String> = f
        .headers
        .iter()
        .map(|(n, _)| n.to_ascii_lowercase())
        .collect();
    for gone in [
        "host",
        "x-http-method-override",
        "x-method-override",
        "private-token",
        "cookie",
    ] {
        assert!(
            !names.contains(&gone.to_string()),
            "{gone} was forwarded: {names:?}"
        );
    }
    assert!(names.contains(&"content-length".to_string()));
}

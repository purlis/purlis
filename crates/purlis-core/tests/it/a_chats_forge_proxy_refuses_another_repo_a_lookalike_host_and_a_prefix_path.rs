// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! One login, slice A: a chat reaches its own repos and nothing else, matched exactly on host,
//! port and normalised path, never by prefix (ADR 0128 D2).

use purlis_core::forge::proxy::{AllowedRepo, Kind, Refusal};

use crate::support::forge_proxy::{Scene, is_refused};

#[test]
fn another_repo_is_refused_by_git_and_by_the_api() {
    purlis_core::unsteered!();
    let s = Scene::new();
    for (host, target) in [
        (
            "github.com",
            "/acme/other.git/info/refs?service=git-receive-pack",
        ),
        (
            "github.com",
            "/acme/other.git/info/refs?service=git-receive-pack",
        ),
        ("api.github.com", "/repos/acme/other/pulls"),
        ("api.github.com", "/repos/other/app/pulls"),
        (
            "gitlab.com",
            "/group/other.git/info/refs?service=git-receive-pack",
        ),
        (
            "gitlab.com",
            "/api/v4/projects/group%2Fother/merge_requests",
        ),
        ("gitlab.com", "/api/v4/projects/43/merge_requests"),
    ] {
        let out = s.send("GET", host, target, b"");
        assert!(
            is_refused(&out, Refusal::OtherRepo),
            "{host}{target}: {out:?}"
        );
    }
}

#[test]
fn a_repo_whose_path_only_starts_like_the_chats_is_another_repo() {
    purlis_core::unsteered!();
    let s = Scene::new();
    for (host, target) in [
        (
            "github.com",
            "/acme/app-secrets.git/info/refs?service=git-receive-pack",
        ),
        (
            "github.com",
            "/acme/app2.git/info/refs?service=git-receive-pack",
        ),
        (
            "github.com",
            "/acme/app.git.evil/info/refs?service=git-receive-pack",
        ),
        (
            "github.com",
            "/acme/ap.git/info/refs?service=git-receive-pack",
        ),
        ("api.github.com", "/repos/acme/app-secrets/pulls"),
        ("api.github.com", "/repos/acme/app.git/pulls"),
        (
            "gitlab.com",
            "/group/proj-x.git/info/refs?service=git-receive-pack",
        ),
        (
            "gitlab.com",
            "/group/proj/sub.git/info/refs?service=git-receive-pack",
        ),
        (
            "gitlab.com",
            "/api/v4/projects/group%2Fproj-x/merge_requests",
        ),
        (
            "gitlab.com",
            "/api/v4/projects/group%2Fproj%2Fsub/merge_requests",
        ),
        (
            "gitlab.com",
            "/api/v4/projects/group%2Fproj.git/merge_requests",
        ),
        (
            "git.corp.example",
            "/grp/sub/proj-x.git/info/refs?service=git-receive-pack",
        ),
        (
            "git.corp.example",
            "/grp/sub.git/info/refs?service=git-receive-pack",
        ),
    ] {
        let port = if host == "git.corp.example" {
            8443
        } else {
            443
        };
        let out = s.send_port("GET", host, port, target);
        assert!(
            is_refused(&out, Refusal::OtherRepo),
            "{host}{target}: {out:?}"
        );
    }
}

#[test]
fn a_path_that_could_be_read_two_ways_is_refused() {
    purlis_core::unsteered!();
    let s = Scene::new();
    for target in [
        "/acme/app.git/../other.git/info/refs?service=git-upload-pack",
        "/acme/other/../app.git/info/refs?service=git-upload-pack",
        "/acme/app.git/%2e%2e/other.git/info/refs?service=git-upload-pack",
        "/acme/%2E%2e/other.git/info/refs?service=git-upload-pack",
        "/acme//app.git/info/refs?service=git-upload-pack",
        "/./acme/app.git/info/refs?service=git-upload-pack",
        "/acme/app.git/info/refs/?service=git-upload-pack",
        "/acme\\app.git/info/refs?service=git-upload-pack",
        "/acme/app.git;x/info/refs?service=git-upload-pack",
        "/acme/app.git/info/refs?service=git-upload-pack#x",
        "/acme/app.git/info%00/refs?service=git-upload-pack",
        "acme/app.git/info/refs?service=git-upload-pack",
    ] {
        let out = s.send("GET", "github.com", target, b"");
        assert!(
            is_refused(&out, Refusal::MalformedRequest),
            "{target}: {out:?}"
        );
    }
    // Decoded once: `%252e%252e` is the text `%2e%2e`, a name, and not a repo of the chat's.
    let out = s.send(
        "GET",
        "github.com",
        "/acme/%252e%252e.git/info/refs?service=git-upload-pack",
        b"",
    );
    assert!(is_refused(&out, Refusal::OtherRepo), "{out:?}");
    // An encoded slash is not a segment break outside GitLab's project id.
    let out = s.send("GET", "api.github.com", "/repos/acme%2Fapp/pulls", b"");
    assert!(is_refused(&out, Refusal::OtherRepo), "{out:?}");
    let out = s.send(
        "GET",
        "api.github.com",
        "/repos/acme/app/contents%2F..%2Fx",
        b"",
    );
    assert!(is_refused(&out, Refusal::MalformedRequest), "{out:?}");
    // On GitLab an encoded slash is where a file path is named, and names the routes it hides.
    let out = s.send("GET", "gitlab.com", "/api/v4/projects/42/hooks%2Fx", b"");
    assert!(is_refused(&out, Refusal::Admin), "{out:?}");
    let out = s.send(
        "PUT",
        "gitlab.com",
        "/api/v4/projects/42/merge_requests%2F3%2Fmerge",
        b"",
    );
    assert!(is_refused(&out, Refusal::Merge), "{out:?}");
}

#[test]
fn a_lookalike_host_is_refused() {
    purlis_core::unsteered!();
    let s = Scene::new();
    let userinfo_host = format!("github.com{}evil.example", '\u{40}');
    for host in [
        "github.com.evil.example",
        "evil-github.com",
        "githab.com",
        "githubx.com",
        "api.github.com.evil.example",
        "xapi.github.com",
        &userinfo_host,
        "github.com:443",
        "github.com/",
        "gitlab.co",
        "ghe.corp.example.evil.example",
        "",
        "[::1]",
        "127.0.0.1",
    ] {
        let out = s.send(
            "GET",
            host,
            "/acme/app.git/info/refs?service=git-upload-pack",
            b"",
        );
        assert!(
            is_refused(&out, Refusal::HostNotAllowed)
                || is_refused(&out, Refusal::MalformedRequest),
            "{host}: {out:?}"
        );
    }
    // The trailing dot of an absolute name and a capital are the same host, and are answered.
    assert!(matches!(
        s.send(
            "GET",
            "GitHub.com.",
            "/acme/app.git/info/refs?service=git-upload-pack",
            b""
        ),
        purlis_core::forge::proxy::Outcome::Forward(_)
    ));
}

#[test]
fn the_port_is_part_of_the_address() {
    purlis_core::unsteered!();
    let s = Scene::new();
    let target = "/acme/app.git/info/refs?service=git-upload-pack";
    assert!(is_refused(
        &s.send_port("GET", "github.com", 8443, target),
        Refusal::HostNotAllowed
    ));
    assert!(is_refused(
        &s.send_port("GET", "github.com", 80, target),
        Refusal::HostNotAllowed
    ));
    let target = "/grp/sub/proj.git/info/refs?service=git-upload-pack";
    assert!(is_refused(
        &s.send_port("GET", "git.corp.example", 443, target),
        Refusal::HostNotAllowed
    ));
    assert!(matches!(
        s.send_port("GET", "git.corp.example", 8443, target),
        purlis_core::forge::proxy::Outcome::Forward(_)
    ));
}

#[test]
fn a_repo_is_the_one_a_chat_was_given_not_the_one_another_chat_has() {
    purlis_core::unsteered!();
    let s = Scene::new();
    // `acme/other` is chat-b's. On chat-a's endpoint it is another repo.
    let out = s.send(
        "GET",
        "github.com",
        "/acme/other.git/info/refs?service=git-receive-pack",
        b"",
    );
    assert!(is_refused(&out, Refusal::OtherRepo), "{out:?}");
}

#[test]
fn an_address_that_cannot_be_a_repo_cannot_be_given_to_a_chat() {
    purlis_core::unsteered!();
    let userinfo_host = format!("user{}github.com", '\u{40}');
    for (kind, host, path) in [
        (Kind::GitHub, "github.com", "acme"),
        (Kind::GitHub, "github.com", "acme/app/extra"),
        (Kind::GitHub, "github.com", "acme/.."),
        (Kind::GitHub, "github.com", "acme/a pp"),
        (Kind::GitHub, "github.com", ""),
        (Kind::GitLab, "gitlab.com", "proj"),
        (Kind::GitLab, "gitlab.com", "g//proj"),
        (Kind::GitLab, "gitlab.com", "g/../proj"),
        (Kind::GitHub, "github.com/x", "acme/app"),
        (Kind::GitHub, &userinfo_host, "acme/app"),
        (Kind::GitHub, "", "acme/app"),
    ] {
        assert!(
            AllowedRepo::new(kind, host, 443, path, "main").is_err(),
            "{host} {path}"
        );
    }
    assert!(AllowedRepo::new(Kind::GitHub, "github.com", 443, "acme/app", "").is_err());
}

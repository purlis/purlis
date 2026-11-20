// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! One login, slice A (instruction 182 items 2 and 4): a chat reads a public repo it did not
//! record without the person's credential ever being added, and cannot start a login at the forge.

use std::sync::atomic::Ordering;

use purlis_core::forge::proxy::{Class, Forwarded, Outcome, Refusal};

use crate::support::forge_proxy::{Scene, is_refused};

fn anon(o: Outcome) -> Forwarded {
    match o {
        Outcome::Forward(f) => f,
        Outcome::Refused(r) => panic!("refused: {r:?}"),
    }
}

#[test]
fn a_fetch_of_a_repo_the_chat_did_not_record_goes_through_with_no_credential() {
    purlis_core::unsteered!();
    let s = Scene::new();
    for (host, repo) in [
        ("github.com", "/rust-lang/rust"),
        ("gitlab.com", "/gitlab-org/gitlab"),
        ("github.com", "/acme/app-secrets"),
    ] {
        let f = anon(s.send(
            "GET",
            host,
            &format!("{repo}.git/info/refs?service=git-upload-pack"),
            b"",
        ));
        assert!(!f.credentialed, "{repo}");
        assert!(
            f.headers
                .iter()
                .all(|(n, _)| !n.eq_ignore_ascii_case("authorization")),
            "{repo}"
        );
        let f = anon(s.send("POST", host, &format!("{repo}.git/git-upload-pack"), b""));
        assert!(!f.credentialed && f.class == Class::GitFetch);
    }
    assert_eq!(
        s.asked.load(Ordering::SeqCst),
        0,
        "the credential was asked for"
    );
}

#[test]
fn no_valid_sentinel_means_the_anonymous_route_with_all_authorization_stripped() {
    purlis_core::unsteered!();
    use crate::support::forge_proxy::Auth;
    let s = Scene::new();
    let target = "/rust-lang/rust.git/info/refs?service=git-upload-pack";
    let own = s.a_sentinel.expose().to_string();
    for auth in [
        Auth::None,
        Auth::Bearer("not-it"),
        Auth::Basic("u", "not-it"),
        Auth::Bearer(&own),
    ] {
        let f = anon(s.send_as(&s.a, "GET", "github.com", 443, target, auth, b""));
        assert!(!f.credentialed);
        assert!(
            f.headers
                .iter()
                .all(|(n, _)| !n.eq_ignore_ascii_case("authorization"))
        );
    }
    // The credentialed route is not open to it.
    let own_repo = "/acme/app.git/info/refs?service=git-upload-pack";
    let out = s.send_as(&s.a, "GET", "github.com", 443, own_repo, Auth::None, b"");
    assert!(is_refused(&out, Refusal::NoSentinel), "{out:?}");
    let out = s.send_as(
        &s.a,
        "GET",
        "github.com",
        443,
        own_repo,
        Auth::Bearer("not-it"),
        b"",
    );
    assert!(is_refused(&out, Refusal::WrongSentinel), "{out:?}");
    assert_eq!(s.asked.load(Ordering::SeqCst), 0);
}

#[test]
fn a_chat_locked_out_by_guessing_gets_no_anonymous_read_either() {
    purlis_core::unsteered!();
    use crate::support::forge_proxy::Auth;
    let s = Scene::new();
    let target = "/rust-lang/rust.git/info/refs?service=git-upload-pack";
    for i in 0..10 {
        let guess = format!("g{i}");
        let _ = s.send_as(
            &s.a,
            "GET",
            "github.com",
            443,
            target,
            Auth::Bearer(&guess),
            b"",
        );
    }
    let out = s.send_as(&s.a, "GET", "github.com", 443, target, Auth::None, b"");
    assert!(is_refused(&out, Refusal::Locked), "{out:?}");
}

#[test]
fn a_download_host_is_tunnelled_and_never_terminated_and_a_recorded_host_is_terminated() {
    purlis_core::unsteered!();
    use purlis_core::forge::proxy::{AllowedRepo, ChatPolicy, ConnectVerdict, Kind};
    let gh = AllowedRepo::new(Kind::GitHub, "github.com", 443, "acme/app", "main").unwrap();
    let policy = ChatPolicy::new(vec![gh]);
    for host in [
        "codeload.github.com",
        "release-assets.githubusercontent.com",
        "objects.githubusercontent.com",
    ] {
        assert!(
            matches!(policy.connect(host, 443), ConnectVerdict::Tunnel { .. }),
            "{host}"
        );
        assert!(
            matches!(policy.connect(host, 8443), ConnectVerdict::Refuse(_)),
            "{host}:8443"
        );
        assert!(
            !policy.terminable_hosts().contains(&host.to_string()),
            "{host} would get a leaf"
        );
    }
    assert_eq!(
        policy.terminable_hosts(),
        vec!["api.github.com".to_string(), "github.com".to_string()]
    );
    for host in [
        "raw.githubusercontent.com",
        "uploads.github.com",
        "evil.example",
    ] {
        assert!(
            matches!(policy.connect(host, 443), ConnectVerdict::Refuse(_)),
            "{host}"
        );
    }
    // A chat with no repo on github.com may not tunnel to GitHub's download hosts.
    let gl = AllowedRepo::new(Kind::GitLab, "gitlab.com", 443, "g/p", "main").unwrap();
    let only_gitlab = ChatPolicy::new(vec![gl]);
    assert!(matches!(
        only_gitlab.connect("codeload.github.com", 443),
        ConnectVerdict::Refuse(_)
    ));
}

#[test]
fn archives_and_release_downloads_of_an_unrecorded_repo_are_anonymous_reads() {
    purlis_core::unsteered!();
    let s = Scene::new();
    for (host, target) in [
        (
            "github.com",
            "/rust-lang/rust/archive/refs/heads/master.tar.gz",
        ),
        (
            "github.com",
            "/rust-lang/rust/releases/download/1.0/rustc.tar.xz",
        ),
        (
            "gitlab.com",
            "/gitlab-org/gitlab/-/archive/master/gitlab-master.zip",
        ),
        (
            "gitlab.com",
            "/gitlab-org/gitlab/-/releases/v1/downloads/asset",
        ),
    ] {
        let f = anon(s.send("GET", host, target, b""));
        assert!(!f.credentialed, "{target}");
        assert_eq!(f.class, Class::PublicRead);
    }
    assert_eq!(s.asked.load(Ordering::SeqCst), 0);
}

#[test]
fn nothing_else_is_anonymous_not_a_push_not_a_write_not_the_api() {
    purlis_core::unsteered!();
    let s = Scene::new();
    for (method, host, target) in [
        (
            "GET",
            "github.com",
            "/rust-lang/rust.git/info/refs?service=git-receive-pack",
        ),
        ("POST", "github.com", "/rust-lang/rust.git/git-receive-pack"),
        ("GET", "api.github.com", "/repos/rust-lang/rust/pulls"),
        ("POST", "api.github.com", "/repos/rust-lang/rust/issues"),
        ("GET", "gitlab.com", "/api/v4/projects/gitlab-org%2Fgitlab"),
        ("POST", "github.com", "/rust-lang/rust/archive/x.tar.gz"),
    ] {
        let out = s.send(method, host, target, b"");
        assert!(
            matches!(out, Outcome::Refused(_)),
            "{method} {host}{target}: {out:?}"
        );
    }
}

#[test]
fn a_host_the_chat_has_no_repo_on_is_not_reachable_even_anonymously() {
    purlis_core::unsteered!();
    let s = Scene::new();
    let out = s.send(
        "GET",
        "bitbucket.org",
        "/a/b.git/info/refs?service=git-upload-pack",
        b"",
    );
    assert!(is_refused(&out, Refusal::HostNotAllowed), "{out:?}");
}

#[test]
fn a_login_flow_at_the_forge_is_refused() {
    purlis_core::unsteered!();
    let s = Scene::new();
    for (method, host, target) in [
        ("POST", "github.com", "/login/device/code"),
        ("POST", "github.com", "/login/oauth/access_token"),
        ("GET", "github.com", "/login/oauth/authorize?client_id=x"),
        ("POST", "ghe.corp.example", "/login/device/code"),
        ("POST", "gitlab.com", "/oauth/token"),
        ("POST", "gitlab.com", "/oauth/authorize_device"),
        ("GET", "git.corp.example", "/oauth/authorize"),
    ] {
        let port = if host == "git.corp.example" {
            8443
        } else {
            443
        };
        let out = s.send_port(method, host, port, target);
        assert!(
            is_refused(&out, Refusal::OAuthFlow),
            "{method} {host}{target}: {out:?}"
        );
    }
}

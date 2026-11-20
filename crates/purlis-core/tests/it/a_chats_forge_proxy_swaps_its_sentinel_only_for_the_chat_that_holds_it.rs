// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! One login, slice A: a chat holds a sentinel, never the token; the real credential is put in
//! only after the policy allows, and only for the chat the sentinel was made for (ADR 0128 D2).

use std::sync::atomic::Ordering;

use purlis_core::forge::proxy::{ChatId, ForgeProxy, NoCredentials, NoObjects, Outcome, Refusal};

use crate::support::forge_proxy::{Auth, REAL, Scene, headers, is_refused};

const TARGET: &str = "/repos/acme/app/pulls";

#[test]
fn a_request_with_no_credential_is_refused_and_the_credential_is_not_even_asked_for() {
    let s = Scene::new();
    let out = s.send_as(&s.a, "GET", "api.github.com", 443, TARGET, Auth::None, b"");
    assert!(is_refused(&out, Refusal::NoSentinel), "{out:?}");
    assert_eq!(s.asked.load(Ordering::SeqCst), 0);
}

#[test]
fn a_request_with_a_wrong_sentinel_is_refused() {
    let s = Scene::new();
    let near = format!("{}0", s.a_sentinel.expose());
    for auth in [
        Auth::Bearer("not-the-sentinel"),
        Auth::Bearer(&near),
        Auth::Bearer(""),
        Auth::Token("purlis-sentinel-"),
        Auth::Basic("x-access-token", "wrong"),
        Auth::PrivateToken("wrong"),
    ] {
        let out = s.send_as(&s.a, "GET", "api.github.com", 443, TARGET, auth, b"");
        assert!(
            is_refused(&out, Refusal::WrongSentinel) || is_refused(&out, Refusal::NoSentinel),
            "{out:?}"
        );
    }
    assert_eq!(s.asked.load(Ordering::SeqCst), 0);
}

#[test]
fn another_chats_sentinel_is_refused_on_this_chats_endpoint() {
    let s = Scene::new();
    let theirs = s.b_sentinel.expose().to_string();
    let out = s.send_as(
        &s.a,
        "GET",
        "api.github.com",
        443,
        TARGET,
        Auth::Bearer(&theirs),
        b"",
    );
    assert!(is_refused(&out, Refusal::WrongSentinel), "{out:?}");
    // And the reverse: this chat's sentinel does not open chat-b's endpoint.
    let mine = s.a_sentinel.expose().to_string();
    let out = s.send_as(
        &s.b,
        "GET",
        "api.github.com",
        443,
        "/repos/acme/other/pulls",
        Auth::Bearer(&mine),
        b"",
    );
    assert!(is_refused(&out, Refusal::WrongSentinel), "{out:?}");
    assert_eq!(s.asked.load(Ordering::SeqCst), 0);
}

#[test]
fn a_real_token_a_chat_got_from_somewhere_else_is_refused_not_passed_on() {
    let s = Scene::new();
    let out = s.send_as(
        &s.a,
        "GET",
        "api.github.com",
        443,
        TARGET,
        Auth::Bearer(REAL),
        b"",
    );
    assert!(is_refused(&out, Refusal::WrongSentinel), "{out:?}");
}

#[test]
fn a_sentinel_beside_another_credential_is_refused_it_is_not_guessed_which_was_meant() {
    let s = Scene::new();
    let mut sent = headers(Auth::Bearer(s.a_sentinel.expose()));
    sent.push(("PRIVATE-TOKEN".into(), "glpat-another".into()));
    let head = purlis_core::forge::proxy::RequestHead::new("GET", "api.github.com", 443, TARGET);
    let out = s.proxy.handle(
        &s.a,
        &purlis_core::forge::proxy::Inbound {
            head: &head,
            headers: &sent,
            body: b"",
        },
    );
    assert!(is_refused(&out, Refusal::WrongSentinel), "{out:?}");
}

#[test]
fn the_sentinel_is_accepted_in_the_forms_git_gh_and_glab_send_it() {
    let s = Scene::new();
    let sent = s.a_sentinel.expose().to_string();
    for (auth, host, target) in [
        (Auth::Bearer(&sent), "api.github.com", TARGET),
        (Auth::Token(&sent), "api.github.com", TARGET),
        (
            Auth::Basic("x-access-token", &sent),
            "api.github.com",
            TARGET,
        ),
        (
            Auth::PrivateToken(&sent),
            "gitlab.com",
            "/api/v4/projects/42/pipelines",
        ),
    ] {
        let out = s.send_as(&s.a, "GET", host, 443, target, auth, b"");
        assert!(matches!(out, Outcome::Forward(_)), "{host}: {out:?}");
    }
}

#[test]
fn the_credential_is_asked_for_only_after_the_policy_allows() {
    let s = Scene::new();
    let _ = s.send(
        "PUT",
        "api.github.com",
        "/repos/acme/app/pulls/7/merge",
        b"",
    );
    let _ = s.send("GET", "api.github.com", "/repos/acme/other/pulls", b"");
    assert_eq!(s.asked.load(Ordering::SeqCst), 0);
    let _ = s.send("GET", "api.github.com", TARGET, b"");
    assert_eq!(s.asked.load(Ordering::SeqCst), 1);
}

#[test]
fn a_chat_that_ended_is_refused_and_its_sentinel_is_worth_nothing() {
    let s = Scene::new();
    let sent = s.a_sentinel.expose().to_string();
    s.proxy.retire(&s.a);
    let out = s.send_as(
        &s.a,
        "GET",
        "api.github.com",
        443,
        TARGET,
        Auth::Bearer(&sent),
        b"",
    );
    assert!(is_refused(&out, Refusal::UnknownChat), "{out:?}");
    let stranger = ChatId::new("never-started");
    let out = s.send_as(
        &stranger,
        "GET",
        "api.github.com",
        443,
        TARGET,
        Auth::Bearer(&sent),
        b"",
    );
    assert!(is_refused(&out, Refusal::UnknownChat), "{out:?}");
}

#[test]
fn a_chat_that_starts_again_gets_a_new_sentinel_and_the_old_one_stops() {
    let s = Scene::new();
    let old = s.a_sentinel.expose().to_string();
    let new = s.proxy.enrol(&s.a, vec![]);
    assert_ne!(old, new.expose());
    let out = s.send_as(
        &s.a,
        "GET",
        "api.github.com",
        443,
        TARGET,
        Auth::Bearer(&old),
        b"",
    );
    assert!(is_refused(&out, Refusal::WrongSentinel), "{out:?}");
}

#[test]
fn a_sentinel_is_long_random_and_never_printed() {
    let s = Scene::new();
    let a = s.a_sentinel.expose();
    assert!(a.len() >= 64 + "purlis-sentinel-".len());
    assert_ne!(a, s.b_sentinel.expose());
    assert!(!format!("{:?}", s.a_sentinel).contains(a));
}

#[test]
fn with_no_credential_source_an_allowed_request_is_refused_and_nothing_is_sent() {
    let proxy = ForgeProxy::new(Box::new(NoCredentials), Box::new(NoObjects));
    let chat = ChatId::new("c");
    let repo = purlis_core::forge::proxy::AllowedRepo::new(
        purlis_core::forge::proxy::Kind::GitHub,
        "github.com",
        443,
        "acme/app",
        "main",
    )
    .unwrap();
    let sentinel = proxy.enrol(&chat, vec![repo]);
    let head = purlis_core::forge::proxy::RequestHead::new("GET", "api.github.com", 443, TARGET);
    let h = headers(Auth::Bearer(sentinel.expose()));
    let out = proxy.handle(
        &chat,
        &purlis_core::forge::proxy::Inbound {
            head: &head,
            headers: &h,
            body: b"",
        },
    );
    assert!(is_refused(&out, Refusal::NoCredential), "{out:?}");
}

#[test]
fn what_is_forwarded_never_prints_the_credential() {
    let s = Scene::new();
    let out = s.send("GET", "api.github.com", TARGET, b"");
    assert!(matches!(out, Outcome::Forward(_)));
    assert!(!format!("{out:?}").contains(REAL));
}

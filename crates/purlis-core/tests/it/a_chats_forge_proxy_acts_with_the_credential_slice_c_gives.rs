// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! One login, slice A2 (instruction 211): the proxy asks slice C's credential source for the
//! person's token, once for each request, and acts with what it is given: a bearer on GitHub, a
//! private-token header for a GitLab access token. A host with no credential, a job's GitLab token and
//! another forge's token are refusals.

use std::sync::Arc;

use purlis_core::forge::credential::{TokenConnection, variable_for};
use purlis_core::forge::proxy::{
    AllowedRepo, BrokeredCredentials, ChatId, ForgeProxy, Inbound, Kind, NoObjects, Outcome,
    Refusal, RequestHead,
};

use crate::support::forge_proxy::is_refused;

// Tokens of the shape each forge issues, put together here so a scan of the source finds none.
const PAT_FINE: &str = concat!(
    "github",
    "_pat_",
    "fine____________________________________"
);
const GLPAT: &str = concat!("gl", "pat-", "AAAAAAAAAAAAAAAAAAAA");
const GLCBT: &str = concat!("gl", "cbt-", "0123456789abcdef");

fn proxy_with(tokens: Vec<(&'static str, &'static str)>) -> (ForgeProxy, ChatId, String) {
    let held: Vec<(String, String)> = tokens
        .into_iter()
        .map(|(host, token)| (variable_for(host), token.to_string()))
        .collect();
    let source = TokenConnection::reading(move |name| {
        held.iter()
            .find(|(variable, _)| variable == name)
            .map(|(_, token)| token.clone())
    });
    let proxy = ForgeProxy::new(
        Box::new(BrokeredCredentials::new(Arc::new(source))),
        Box::new(NoObjects),
    );
    let chat = ChatId::new("chat-c");
    let sentinel = proxy.enrol(
        &chat,
        vec![
            AllowedRepo::new(Kind::GitHub, "github.com", 443, "acme/app", "main").unwrap(),
            AllowedRepo::new(Kind::GitLab, "gitlab.com", 443, "group/proj", "main")
                .unwrap()
                .with_project_id(42),
            AllowedRepo::new(Kind::GitLab, "git.corp.example", 443, "grp/proj", "main").unwrap(),
        ],
    );
    (proxy, chat, sentinel.expose().to_string())
}

fn get(proxy: &ForgeProxy, chat: &ChatId, sentinel: &str, host: &str, target: &str) -> Outcome {
    let head = RequestHead::new("GET", host, 443, target);
    let headers = vec![("Authorization".to_string(), format!("Bearer {sentinel}"))];
    proxy.handle(
        chat,
        &Inbound {
            head: &head,
            headers: &headers,
            body: b"",
        },
    )
}

fn header<'a>(f: &'a purlis_core::forge::proxy::Forwarded, name: &str) -> Option<&'a str> {
    f.headers
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

#[test]
fn the_token_slice_c_holds_is_the_one_the_forge_is_sent() {
    purlis_core::unsteered!();
    let (proxy, chat, sentinel) = proxy_with(vec![("github.com", PAT_FINE), ("gitlab.com", GLPAT)]);
    let out = get(&proxy, &chat, &sentinel, "api.github.com", "/user");
    let Outcome::Forward(f) = out else {
        panic!("a GitHub call is answered: {out:?}");
    };
    assert_eq!(
        header(&f, "authorization"),
        Some(format!("Bearer {PAT_FINE}").as_str())
    );
    assert!(f.leak_needles().any(|n| n == PAT_FINE.as_bytes()));

    // A GitLab access token is a PRIVATE-TOKEN, not a bearer.
    let out = get(&proxy, &chat, &sentinel, "gitlab.com", "/api/v4/user");
    let Outcome::Forward(f) = out else {
        panic!("a GitLab call is answered: {out:?}");
    };
    assert_eq!(header(&f, "private-token"), Some(GLPAT));
    assert!(header(&f, "authorization").is_none());
}

#[test]
fn a_host_with_no_credential_and_a_jobs_token_are_refused() {
    purlis_core::unsteered!();
    let (proxy, chat, sentinel) = proxy_with(vec![("gitlab.com", GLCBT)]);
    for (host, target) in [
        ("api.github.com", "/user"),
        ("git.corp.example", "/api/v4/user"),
        ("gitlab.com", "/api/v4/user"),
    ] {
        let out = get(&proxy, &chat, &sentinel, host, target);
        assert!(
            is_refused(&out, Refusal::NoCredential),
            "{host}{target}: {out:?}"
        );
    }
}

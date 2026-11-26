// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! One login (#7301): a GraphQL document that names several of a chat's repos is sent only when
//! one account serves them all. The accounts are compared by id, never by the token's bytes: a
//! renewal between two lookups gives one account a new token, and two accounts may not be told
//! apart by a token that happens to be equal.

use std::sync::atomic::{AtomicUsize, Ordering};

use purlis_core::forge::proxy::{
    AllowedRepo, ChatId, Credential, CredentialError, CredentialRequest, CredentialSource,
    ForgeProxy, Inbound, Kind, Outcome, RequestHead, TokenKind,
};
use purlis_core::secrets::keyring::Secret;

use crate::support::forge_proxy::{Auth, Objects, graphql, headers};

/// A source whose token and account come from the lookup's number and the repo asked for.
struct Source {
    asked: AtomicUsize,
    token: fn(usize, &str) -> String,
    account: fn(&str) -> Option<String>,
}

impl CredentialSource for Source {
    fn credential_for(&self, request: &CredentialRequest) -> Result<Credential, CredentialError> {
        let n = self.asked.fetch_add(1, Ordering::SeqCst);
        Ok(Credential {
            token_kind: TokenKind::Oauth,
            secret: Secret::new((self.token)(n, &request.repo)),
            account: (self.account)(&request.repo),
        })
    }
}

const BOTH: &str = r#"{ a: repository(owner: "acme", name: "app") { id }
  b: repository(owner: "acme", name: "lib") { id } }"#;

fn send_both(source: Source) -> Outcome {
    send(source, &graphql(BOTH, serde_json::json!({})))
}

/// A search of both repos, as `gh pr list --search` sends one.
fn search_both(source: Source) -> Outcome {
    let query = "query($q: String!){ search(query: $q, type: ISSUE, first: 30) { issueCount } }";
    send(
        source,
        &graphql(
            query,
            serde_json::json!({"q": "repo:acme/app repo:acme/lib is:pr"}),
        ),
    )
}

fn send(source: Source, body: &[u8]) -> Outcome {
    let proxy = ForgeProxy::new(Box::new(source), Box::new(Objects::default()));
    let chat = ChatId::new("chat");
    let sentinel = proxy.enrol(
        &chat,
        vec![
            AllowedRepo::new(Kind::GitHub, "github.com", 443, "acme/app", "main").unwrap(),
            AllowedRepo::new(Kind::GitHub, "github.com", 443, "acme/lib", "main").unwrap(),
        ],
    );
    let head = RequestHead::new("POST", "api.github.com", 443, "/graphql");
    let mut h = headers(Auth::Bearer(sentinel.expose()));
    h.push(("Content-Type".into(), "application/json".into()));
    proxy.handle(
        &chat,
        &Inbound {
            head: &head,
            headers: &h,
            body,
        },
    )
}

#[test]
fn one_account_renewed_between_two_lookups_still_serves_a_document_naming_both_repos() {
    purlis_core::unsteered!();
    let out = send_both(Source {
        asked: AtomicUsize::new(0),
        // Every lookup answers a new token: a renewal landed between them.
        token: |n, _| format!("renewed-token-{n}"),
        account: |_| Some("account-1".to_owned()),
    });
    assert!(matches!(out, Outcome::Forward(_)), "{out:?}");
}

#[test]
fn two_accounts_holding_equal_token_bytes_do_not_serve_one_document() {
    purlis_core::unsteered!();
    let out = send_both(Source {
        asked: AtomicUsize::new(0),
        token: |_, _| "the-same-bytes".to_owned(),
        account: |repo| Some(format!("account-of-{repo}")),
    });
    assert!(!matches!(out, Outcome::Forward(_)), "{out:?}");
}

#[test]
fn a_search_of_two_recorded_repos_is_sent_when_one_account_serves_both() {
    purlis_core::unsteered!();
    let out = search_both(Source {
        asked: AtomicUsize::new(0),
        token: |_, _| "one-token".to_owned(),
        account: |_| Some("account-1".to_owned()),
    });
    assert!(matches!(out, Outcome::Forward(_)), "{out:?}");
}

#[test]
fn a_search_of_two_recorded_repos_on_two_accounts_is_not_sent() {
    purlis_core::unsteered!();
    let out = search_both(Source {
        asked: AtomicUsize::new(0),
        token: |_, _| "one-token".to_owned(),
        account: |repo| Some(format!("account-of-{repo}")),
    });
    assert!(!matches!(out, Outcome::Forward(_)), "{out:?}");
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **One sign-in per forge host keeps the right credential for that host** (ADR 0128 D1, D2).
//!
//! github.com and gitlab.com sign in through purlis's own registrations; a GitHub Enterprise
//! Server host and a self-hosted GitLab through the application an owner made and the person
//! pinned. For each, a grant is signed in as the window does and the test asks what was kept:
//! the account, under the host it signed in on, and the token in its own keyring item and in no
//! file; and what was not: another host's registration, an application nobody pinned, or one
//! pinned with no owner.

use std::path::Path;

use purlis_core::forge::Kind;
use purlis_core::forge::accounts::{self, ForgeAccount, Method, State};
use purlis_core::forge::registered;
use purlis_core::forge::registration::{self, Flow};
use purlis_core::forge::route::HostScope;
use secrecy::ExposeSecret;

use crate::support::one_login::{Tokens, grant, grant_that_does_not_expire};

fn scope() -> HostScope {
    HostScope::for_a_test()
}

/// Every file under `root` but the test build's keyring stub, which stands in for the keychain.
fn files(root: &Path) -> Vec<std::path::PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.file_name().is_some_and(|n| n != "keyring-stub.json") {
                found.push(path);
            }
        }
    }
    found
}

fn in_no_file(root: &Path, values: &[&str]) {
    for path in files(root) {
        let text = String::from_utf8_lossy(&std::fs::read(&path).unwrap()).into_owned();
        for value in values {
            assert!(!text.contains(value), "{} holds a token", path.display());
        }
    }
}

/// What a browser sign-in's keyring item holds, as JSON: the pair it kept.
fn item(
    keyring: &dyn purlis_core::secrets::keyring::Store,
    account: &ForgeAccount,
) -> serde_json::Value {
    let text = keyring
        .get(&accounts::service(account), "token")
        .unwrap()
        .unwrap()
        .into_inner();
    serde_json::from_str(&text).unwrap()
}

/// Signs in `kind` on `host` with `granted`, as the window's flow ends.
fn sign_in(
    home: &Path,
    tokens: &Tokens,
    kind: Kind,
    host: &str,
    granted: purlis_core::forge::oauth::Granted,
    method: Method,
) -> ForgeAccount {
    let keyring = accounts::keyring_at(home);
    let reach = tokens.reach();
    // The application the sign-in is made through, as the window resolves it for the host.
    let client = registration::resolve(home, kind, host)
        .unwrap()
        .expect("a registration for the host")
        .client_id;
    accounts::sign_in_with_grant(
        &scope(),
        home,
        keyring.as_ref(),
        &reach,
        kind,
        host,
        granted,
        method,
        client,
    )
    .unwrap()
}

#[test]
fn github_com_signs_in_through_purlis_own_app_and_keeps_a_token_that_does_not_expire() {
    let home = tempfile::tempdir().unwrap();
    let tokens = Tokens::default();
    tokens.know("canaryGITHUBaccess1", "octocat", "repo, read:org");
    let github = registration::resolve(home.path(), Kind::GitHub, "github.com")
        .unwrap()
        .unwrap();
    assert_eq!(github.flow, Flow::Device);
    assert_eq!(
        github.scopes(),
        ["repo", "read:org"],
        "no workflow at sign-in"
    );
    let account = sign_in(
        home.path(),
        &tokens,
        Kind::GitHub,
        "github.com",
        grant_that_does_not_expire("canaryGITHUBaccess1", "repo,read:org"),
        Method::Device,
    );
    assert_eq!(
        (account.host.as_str(), account.login.as_str(), account.state),
        ("github.com", "octocat", State::SignedIn)
    );
    let keyring = accounts::keyring_at(home.path());
    let kept = item(keyring.as_ref(), &account);
    assert_eq!(kept["access"], "canaryGITHUBaccess1");
    assert!(
        kept.get("refresh").is_none(),
        "an OAuth app's token has no refresh token"
    );
    assert!(kept.get("expires_at").is_none(), "and does not expire");
    assert_eq!(
        accounts::service(&account),
        format!("purlis/@forge/github.com/{}", account.id)
    );
    in_no_file(home.path(), &["canaryGITHUBaccess1"]);
}

#[test]
fn gitlab_com_signs_in_by_pkce_and_keeps_the_pair_that_renews() {
    let home = tempfile::tempdir().unwrap();
    let tokens = Tokens::default();
    tokens.know("canaryGITLABaccess1", "octocat", "api");
    let gitlab = registration::resolve(home.path(), Kind::GitLab, "gitlab.com")
        .unwrap()
        .unwrap();
    assert_eq!(gitlab.flow, Flow::Pkce);
    assert_eq!(gitlab.scopes(), ["api"]);
    let account = sign_in(
        home.path(),
        &tokens,
        Kind::GitLab,
        "gitlab.com",
        grant("canaryGITLABaccess1", "canaryGITLABrefresh1", "api"),
        Method::Pkce,
    );
    assert_eq!(account.host, "gitlab.com");
    let keyring = accounts::keyring_at(home.path());
    let kept = item(keyring.as_ref(), &account);
    assert_eq!(kept["access"], "canaryGITLABaccess1");
    assert_eq!(kept["refresh"], "canaryGITLABrefresh1");
    assert!(
        kept.get("expires_at").is_some(),
        "GitLab's token expires, so it is renewed"
    );
    in_no_file(
        home.path(),
        &["canaryGITLABaccess1", "canaryGITLABrefresh1"],
    );
}

#[test]
fn a_ghes_host_signs_in_through_the_application_an_owner_made_and_never_githubs() {
    let home = tempfile::tempdir().unwrap();
    let tokens = Tokens::default();
    tokens.know("canaryGHESaccess1", "octocat", "repo, read:org");
    // Nobody pinned an application: purlis has none for this host, and github.com's id is not used.
    assert!(
        registration::resolve(home.path(), Kind::GitHub, "ghe.example.com")
            .unwrap()
            .is_none()
    );
    registered::pin_with_owner(
        &scope(),
        home.path(),
        Kind::GitHub,
        "ghe.example.com",
        "Iv1.ghesAppOfAcme",
        "acme-platform",
    )
    .unwrap();
    let ghes = registration::resolve(home.path(), Kind::GitHub, "ghe.example.com")
        .unwrap()
        .unwrap();
    assert_eq!(ghes.flow, Flow::Device);
    assert_eq!(ghes.client_id, "Iv1.ghesAppOfAcme");
    let own = registration::on(Kind::GitHub, "github.com").unwrap();
    assert_ne!(
        ghes.client_id, own.client_id,
        "github.com's id is never sent to a GHES host"
    );
    let account = sign_in(
        home.path(),
        &tokens,
        Kind::GitHub,
        "ghe.example.com",
        grant_that_does_not_expire("canaryGHESaccess1", "repo,read:org"),
        Method::Device,
    );
    assert_eq!(account.host, "ghe.example.com");
    assert_eq!(
        accounts::service(&account),
        format!("purlis/@forge/ghe.example.com/{}", account.id)
    );
    in_no_file(home.path(), &["canaryGHESaccess1"]);
}

#[test]
fn a_self_hosted_gitlab_signs_in_by_pkce_through_the_group_application() {
    let home = tempfile::tempdir().unwrap();
    let tokens = Tokens::default();
    tokens.know("canarySELFaccess1", "octocat", "api");
    registered::pin_with_owner(
        &scope(),
        home.path(),
        Kind::GitLab,
        "git.example.com",
        "f00dbabe5e1f",
        "platform/tools",
    )
    .unwrap();
    let pinned = registration::resolve(home.path(), Kind::GitLab, "git.example.com")
        .unwrap()
        .unwrap();
    assert_eq!(pinned.flow, Flow::Pkce);
    assert_eq!(pinned.client_id, "f00dbabe5e1f");
    let account = sign_in(
        home.path(),
        &tokens,
        Kind::GitLab,
        "git.example.com",
        grant("canarySELFaccess1", "canarySELFrefresh1", "api"),
        Method::Pkce,
    );
    assert_eq!(account.host, "git.example.com");
    // gitlab.com's application is not this host's.
    assert_ne!(
        pinned.client_id,
        registration::on(Kind::GitLab, "gitlab.com")
            .unwrap()
            .client_id
    );
}

#[test]
fn the_press_refuses_an_application_with_no_owner_and_a_pin_cannot_replace_githubs_own() {
    let home = tempfile::tempdir().unwrap();
    for owner in ["", "  ", "a b"] {
        assert!(
            registered::pin_with_owner(
                &scope(),
                home.path(),
                Kind::GitHub,
                "ghe.example.com",
                "Iv1.old",
                owner
            )
            .is_err(),
            "{owner:?}"
        );
    }
    assert!(
        !registered::path(home.path()).exists(),
        "nothing was pinned"
    );
    assert!(
        registration::resolve(home.path(), Kind::GitHub, "ghe.example.com")
            .unwrap()
            .is_none()
    );
    // And github.com's own cannot be replaced.
    assert!(
        registered::pin_with_owner(
            &scope(),
            home.path(),
            Kind::GitHub,
            "github.com",
            "Iv1.mine",
            "acme"
        )
        .is_err()
    );
}

#[test]
fn each_host_keeps_its_own_accounts_token() {
    let home = tempfile::tempdir().unwrap();
    let tokens = Tokens::default();
    tokens.know("canaryONEaccess", "octocat", "repo, read:org");
    tokens.know("canaryTWOaccess", "octocat", "repo, read:org");
    registered::pin_with_owner(
        &scope(),
        home.path(),
        Kind::GitHub,
        "ghe.example.com",
        "Iv1.ghes",
        "acme",
    )
    .unwrap();
    let one = sign_in(
        home.path(),
        &tokens,
        Kind::GitHub,
        "github.com",
        grant_that_does_not_expire("canaryONEaccess", "repo,read:org"),
        Method::Device,
    );
    let two = sign_in(
        home.path(),
        &tokens,
        Kind::GitHub,
        "ghe.example.com",
        grant_that_does_not_expire("canaryTWOaccess", "repo,read:org"),
        Method::Device,
    );
    assert_ne!(
        one.id, two.id,
        "the same login on two hosts is two accounts"
    );
    let keyring = accounts::keyring_at(home.path());
    assert_eq!(
        accounts::token(keyring.as_ref(), &one)
            .unwrap()
            .unwrap()
            .expose_secret(),
        "canaryONEaccess"
    );
    assert_eq!(
        accounts::token(keyring.as_ref(), &two)
            .unwrap()
            .unwrap()
            .expose_secret(),
        "canaryTWOaccess"
    );
    let on = |host: &str| {
        accounts::every_signed_in_on(home.path(), Kind::GitHub, host)
            .unwrap()
            .len()
    };
    assert_eq!((on("github.com"), on("ghe.example.com")), (1, 1));
}

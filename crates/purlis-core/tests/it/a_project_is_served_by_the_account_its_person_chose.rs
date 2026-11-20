// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A person with a personal and an enterprise account on one host chooses which serves a
//! project** (ADR 0128 D2, D4).
//!
//! Both accounts are signed in on github.com. Two repos are confirmed, each with the account the
//! person chose. The credential a chat's route gets for a repo is the chosen account's token,
//! never the first account on the host's. An account that is gone is not replaced by the other: a
//! chat acting as an account the person did not choose is the one thing this must not allow.

use std::sync::Arc;

use purlis_core::forge::Kind;
use purlis_core::forge::accounts::{self, ForgeAccount, Method};
use purlis_core::forge::confirmed::{self, Serving};
use purlis_core::forge::credential::{CredentialError, SignedIn};
use purlis_core::forge::oauth::{Answer, Endpoint, Endpoints};
use purlis_core::forge::refresh::Refresher;
use purlis_core::forge::registration;
use purlis_core::forge::route::HostScope;

use crate::support::one_login::{Tokens, grant_that_does_not_expire};

const PERSONAL: &str = "canaryPERSONALtoken3e8";
const EMU: &str = "canaryEMUtoken6b2";

/// Now, in seconds since the epoch.
fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn scope() -> HostScope {
    HostScope::for_a_test()
}

struct Never;

impl Endpoint for Never {
    fn post(&self, url: &str, _form: &[(&str, &str)]) -> Result<Answer, String> {
        panic!("the forge was asked at {url}")
    }
}

struct Scene {
    home: tempfile::TempDir,
    personal: ForgeAccount,
    emu: ForgeAccount,
}

fn scene() -> Scene {
    let home = tempfile::tempdir().unwrap();
    let tokens = Tokens::default();
    tokens.know(PERSONAL, "octocat", "repo, read:org");
    tokens.know(EMU, "octocat_acme", "repo, read:org");
    let keyring = accounts::keyring_at(home.path());
    let reach = tokens.reach();
    let sign_in = |access: &str| {
        accounts::sign_in_with_grant(
            &scope(),
            home.path(),
            keyring.as_ref(),
            &reach,
            Kind::GitHub,
            "github.com",
            grant_that_does_not_expire(access, "repo,read:org"),
            Method::Device,
            "Ov23litEn94I5HXrpdsX",
        )
        .unwrap()
    };
    let personal = sign_in(PERSONAL);
    let emu = sign_in(EMU);
    Scene {
        home,
        personal,
        emu,
    }
}

fn source(scene: &Scene) -> SignedIn {
    let github = registration::on(Kind::GitHub, "github.com").unwrap();
    SignedIn::through(
        Refresher::through(
            &scope(),
            scene.home.path(),
            Arc::new(Never),
            Endpoints::of(github),
        ),
        scene.home.path(),
    )
}

#[test]
fn two_accounts_on_one_host_are_two_accounts_and_the_unchosen_route_picks_neither() {
    purlis_core::unsteered!();
    let scene = scene();
    assert_ne!(scene.personal.id, scene.emu.id);
    let source = source(&scene);
    // With two accounts on the host and none chosen for the repo, nothing is picked.
    let why = source.credential_at("github.com", now()).unwrap_err();
    assert!(why.to_string().contains("More than one account"), "{why}");
    assert!(!why.to_string().contains(PERSONAL) && !why.to_string().contains(EMU));
}

#[test]
fn a_chats_route_for_a_repo_is_the_account_the_person_chose_for_it() {
    purlis_core::unsteered!();
    let scene = scene();
    let source = source(&scene);
    let both = accounts::listed(scene.home.path()).unwrap();
    let hello = tempfile::tempdir().unwrap();
    let platform = tempfile::tempdir().unwrap();
    for (clone, id, path, account) in [
        (&hello, 1001u64, "octocat/hello", &scene.personal),
        (&platform, 2002, "acme/platform", &scene.emu),
    ] {
        confirmed::confirm(
            &scope(),
            scene.home.path(),
            clone.path(),
            &confirmed::Candidate {
                kind: Kind::GitHub,
                host: "github.com".into(),
                path: path.into(),
                repo_id: id,
            },
            account,
            "Ov23litEn94I5HXrpdsX",
        )
        .unwrap();
    }
    let credential_for = |clone: &std::path::Path| {
        let repo = confirmed::repos_of_clone(scene.home.path(), clone)
            .unwrap()
            .remove(0);
        match confirmed::account_for(&repo, &both) {
            Serving::Account(account) => source
                .credential_of("github.com", &account.id, now())
                .unwrap(),
            other => panic!("{other:?}"),
        }
    };
    assert_eq!(credential_for(hello.path()).expose(), PERSONAL);
    assert_eq!(credential_for(platform.path()).expose(), EMU);
    assert_eq!(
        credential_for(platform.path()).login(),
        Some("octocat_acme")
    );
    // The person changes which serves the enterprise repo, on a press, and only that repo.
    confirmed::choose_account(
        &scope(),
        scene.home.path(),
        &confirmed::key_of_clone(platform.path()),
        2002,
        &scene.personal,
    )
    .unwrap();
    assert_eq!(credential_for(platform.path()).expose(), PERSONAL);
    assert_eq!(credential_for(hello.path()).expose(), PERSONAL);
}

#[test]
fn a_chosen_account_that_is_gone_is_never_answered_by_the_other_one() {
    purlis_core::unsteered!();
    let scene = scene();
    let source = source(&scene);
    let platform = tempfile::tempdir().unwrap();
    let repo = confirmed::confirm(
        &scope(),
        scene.home.path(),
        platform.path(),
        &confirmed::Candidate {
            kind: Kind::GitHub,
            host: "github.com".into(),
            path: "acme/platform".into(),
            repo_id: 2002,
        },
        &scene.emu,
        "Ov23litEn94I5HXrpdsX",
    )
    .unwrap();
    let keyring = accounts::keyring_at(scene.home.path());
    accounts::sign_out(&scope(), scene.home.path(), keyring.as_ref(), &scene.emu.id).unwrap();
    let remaining = accounts::listed(scene.home.path()).unwrap();
    assert_eq!(
        remaining.len(),
        1,
        "the personal account is still signed in"
    );
    assert!(matches!(
        confirmed::account_for(&repo, &remaining),
        Serving::ChosenIsSignedOut { .. }
    ));
    let why = source
        .credential_of("github.com", &scene.emu.id, now())
        .unwrap_err();
    assert!(matches!(why, CredentialError::Unusable { .. }), "{why:?}");
    assert!(why.to_string().contains("Choose an account"), "{why}");
    assert!(!why.to_string().contains(PERSONAL) && !why.to_string().contains(EMU));
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The second token, for a held CI-file push, is kept apart and never on a chat's route**
//! (ADR 0128 D1.1, D8.3).
//!
//! GitHub refuses a push that changes a workflow file unless the token has `workflow`. The
//! everyday sign-in never asks for it. On the person's press the first time a held CI push needs
//! it, a second sign-in asks for it and keeps the result as its own keyring item. This holds, for
//! a canary of each token:
//!
//! - the everyday token is unchanged and still lacks `workflow`, and the second is read only by
//!   the call for a held CI push;
//! - what a chat's route is handed (the credential source the proxy asks, and every file of the
//!   config home) never holds the second;
//! - if GitHub **folds** `workflow` into the everyday token, nothing is kept and the push is not
//!   offered, rather than the everyday token silently gaining it;
//! - a grant for another login, or without `workflow`, keeps nothing;
//! - GitLab has no second token: its one scope covers CI files, and the app alone uses it;
//! - signing out deletes both.

use std::path::Path;
use std::sync::Arc;

use purlis_core::forge::Kind;
use purlis_core::forge::accounts::workflow::{self, Availability, CiPushCredential, Outcome};
use purlis_core::forge::accounts::{self, ForgeAccount, Method};
use purlis_core::forge::credential::SignedIn;
use purlis_core::forge::oauth::{Answer, Endpoint, Endpoints};
use purlis_core::forge::refresh::Refresher;
use purlis_core::forge::registration;
use purlis_core::forge::route::HostScope;
use secrecy::ExposeSecret;

use crate::support::one_login::{Tokens, grant, grant_that_does_not_expire};

const EVERYDAY: &str = "canaryEVERYDAYtoken7a1";
const SECOND: &str = "canarySECONDtoken9c4";

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

/// An endpoint that must never be asked: a token that does not expire is never renewed.
struct Never;

impl Endpoint for Never {
    fn post(&self, url: &str, _form: &[(&str, &str)]) -> Result<Answer, String> {
        panic!("the forge was asked at {url}")
    }
}

struct Scene {
    home: tempfile::TempDir,
    tokens: Tokens,
    account: ForgeAccount,
}

fn scene() -> Scene {
    let home = tempfile::tempdir().unwrap();
    let tokens = Tokens::default();
    tokens.know(EVERYDAY, "octocat", "repo, read:org");
    let keyring = accounts::keyring_at(home.path());
    let reach = tokens.reach();
    let account = accounts::sign_in_with_grant(
        &scope(),
        home.path(),
        keyring.as_ref(),
        &reach,
        Kind::GitHub,
        "github.com",
        grant_that_does_not_expire(EVERYDAY, "repo,read:org"),
        Method::Device,
        "Ov23litEn94I5HXrpdsX",
    )
    .unwrap();
    Scene {
        home,
        tokens,
        account,
    }
}

impl Scene {
    fn keyring(&self) -> Box<dyn purlis_core::secrets::keyring::Store> {
        accounts::keyring_at(self.home.path())
    }

    fn second_sign_in(&self, access: &str, scope_said: &str) -> Result<Outcome, String> {
        let keyring = self.keyring();
        let reach = self.tokens.reach();
        workflow::keep(
            &scope(),
            self.home.path(),
            keyring.as_ref(),
            &reach,
            &self.account,
            grant_that_does_not_expire(access, scope_said),
        )
    }

    fn listed(&self) -> ForgeAccount {
        accounts::listed(self.home.path()).unwrap().remove(0)
    }
}

fn token_of(credential: CiPushCredential) -> String {
    match credential {
        CiPushCredential::Token(token) => token.expose_secret().to_string(),
        other => panic!("wanted a token, got {other:?}"),
    }
}

#[test]
fn the_everyday_sign_in_never_asks_for_workflow_and_the_second_asks_for_what_is_held_and_it() {
    purlis_core::unsteered!();
    let scene = scene();
    let github = registration::on(Kind::GitHub, "github.com").unwrap();
    assert!(!github.scopes().contains(&workflow::SCOPE));
    assert!(
        !scene
            .account
            .scopes
            .clone()
            .unwrap_or_default()
            .iter()
            .any(|s| s == "workflow")
    );
    assert_eq!(
        workflow::scopes_to_ask(&scene.account),
        ["repo", "read:org", "workflow"]
    );
    assert_eq!(
        workflow::availability(scene.keyring().as_ref(), &scene.account),
        Availability::NeedsPress,
        "until the person's press, a CI push cannot be performed"
    );
}

#[test]
fn a_press_keeps_the_second_token_as_its_own_item_beside_the_unchanged_everyday_one() {
    purlis_core::unsteered!();
    let scene = scene();
    scene
        .tokens
        .know(SECOND, "octocat", "repo, read:org, workflow");
    assert_eq!(
        scene.second_sign_in(SECOND, "repo,read:org,workflow"),
        Ok(Outcome::Kept)
    );
    let keyring = scene.keyring();
    // The everyday token is what it was, and the account still records no workflow scope.
    assert_eq!(
        accounts::token(keyring.as_ref(), &scene.account)
            .unwrap()
            .unwrap()
            .expose_secret(),
        EVERYDAY
    );
    assert!(
        !scene
            .listed()
            .scopes
            .unwrap_or_default()
            .iter()
            .any(|s| s == "workflow")
    );
    // The second is another item of the same service.
    let service = accounts::service(&scene.account);
    let item = keyring
        .get(&service, "workflow")
        .unwrap()
        .unwrap()
        .into_inner();
    assert_eq!(item, SECOND);
    let everyday_item = keyring
        .get(&service, "token")
        .unwrap()
        .unwrap()
        .into_inner();
    assert!(!everyday_item.contains(SECOND));
    assert_eq!(
        workflow::availability(keyring.as_ref(), &scene.account),
        Availability::Ready
    );
    // A held CI push is performed with the second, and never with the everyday one.
    let credential = workflow::for_ci_push(&scope(), keyring.as_ref(), &scene.account).unwrap();
    assert_eq!(token_of(credential), SECOND);
}

#[test]
fn the_second_token_is_in_no_route_a_chat_has_and_in_no_file() {
    purlis_core::unsteered!();
    let scene = scene();
    scene
        .tokens
        .know(SECOND, "octocat", "repo, read:org, workflow");
    scene
        .second_sign_in(SECOND, "repo,read:org,workflow")
        .unwrap();
    // What the proxy asks for a chat's repos: the credential source. It answers the everyday token.
    let github = registration::on(Kind::GitHub, "github.com").unwrap();
    let refresher = Refresher::through(
        &scope(),
        scene.home.path(),
        Arc::new(Never),
        Endpoints::of(github),
    );
    let source = SignedIn::through(refresher, scene.home.path());
    let credential = source.credential_at("github.com", now()).unwrap();
    assert_eq!(credential.expose(), EVERYDAY);
    assert_ne!(credential.expose(), SECOND);
    let shown = format!("{credential:?}");
    assert!(
        !shown.contains(SECOND) && !shown.contains(EVERYDAY),
        "{shown}"
    );
    // And the account store, as the window lists it, holds neither.
    let listed = format!("{:?}", scene.listed());
    assert!(!listed.contains(SECOND), "{listed}");
    let mut stack = vec![scene.home.path().to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.file_name().is_some_and(|n| n != "keyring-stub.json") {
                let text = String::from_utf8_lossy(&std::fs::read(&path).unwrap()).into_owned();
                assert!(
                    !text.contains(SECOND),
                    "{} holds the second token",
                    path.display()
                );
            }
        }
    }
}

#[test]
fn when_github_folds_workflow_into_the_everyday_token_nothing_is_kept_and_a_push_is_not_offered() {
    purlis_core::unsteered!();
    let scene = scene();
    scene
        .tokens
        .know(SECOND, "octocat", "repo, read:org, workflow");
    // The forge now says the everyday token holds workflow too.
    scene
        .tokens
        .know(EVERYDAY, "octocat", "repo, read:org, workflow");
    let outcome = scene
        .second_sign_in(SECOND, "repo,read:org,workflow")
        .unwrap();
    assert!(
        matches!(outcome, Outcome::Folded(ref a) if a.scopes.as_ref().is_some_and(|s| s.iter().any(|w| w == "workflow")))
    );
    let keyring = scene.keyring();
    assert!(
        keyring
            .get(&accounts::service(&scene.account), "workflow")
            .unwrap()
            .is_none(),
        "the second token was not kept"
    );
    let listed = scene.listed();
    assert_eq!(
        workflow::availability(keyring.as_ref(), &listed),
        Availability::NotOffered
    );
    assert!(matches!(
        workflow::for_ci_push(&scope(), keyring.as_ref(), &listed).unwrap(),
        CiPushCredential::NotOffered
    ));
}

#[test]
fn an_everyday_read_back_that_says_nothing_of_scopes_is_a_refusal_and_keeps_nothing() {
    purlis_core::unsteered!();
    let scene = scene();
    scene
        .tokens
        .know(SECOND, "octocat", "repo, read:org, workflow");
    // The forge no longer says what the everyday token may do (no `X-OAuth-Scopes`): purlis cannot
    // tell whether the scope was folded in, so it must not call that "not folded".
    scene.tokens.know_without_scopes(EVERYDAY, "octocat");
    let why = scene
        .second_sign_in(SECOND, "repo,read:org,workflow")
        .unwrap_err();
    assert!(why.contains("did not say"), "{why}");
    assert!(why.contains("Nothing was kept"), "{why}");
    assert!(!why.contains("canary"), "{why}");
    let keyring = scene.keyring();
    assert!(
        keyring
            .get(&accounts::service(&scene.account), "workflow")
            .unwrap()
            .is_none(),
        "the second token was dropped"
    );
    assert_eq!(
        workflow::availability(keyring.as_ref(), &scene.listed()),
        Availability::NeedsPress,
        "a push that changes a CI file is still held for a press"
    );
    // GitHub may have folded `workflow` in all the same: the everyday token is served to no chat
    // until a read-back says what it holds.
    assert_eq!(
        accounts::workflow_doubt(&scene.listed()),
        accounts::WorkflowDoubt::Unknown
    );
    let source = source_of(&scene);
    assert!(
        source
            .credential_of("github.com", &scene.account.id, now())
            .unwrap_err()
            .is_sign_in_again()
    );
}

#[test]
fn an_everyday_read_back_that_fails_leaves_the_account_served_to_no_chat() {
    purlis_core::unsteered!();
    let scene = scene();
    scene
        .tokens
        .know(SECOND, "octocat", "repo, read:org, workflow");
    // The read-back of the everyday token fails (the forge answers nothing for it).
    scene.tokens.forget(EVERYDAY);
    assert!(
        scene
            .second_sign_in(SECOND, "repo,read:org,workflow")
            .is_err()
    );
    assert_eq!(
        accounts::workflow_doubt(&scene.listed()),
        accounts::WorkflowDoubt::Unknown
    );
    let source = source_of(&scene);
    assert!(
        source
            .credential_of("github.com", &scene.account.id, now())
            .unwrap_err()
            .is_sign_in_again()
    );
}

fn source_of(scene: &Scene) -> SignedIn {
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
fn after_a_fold_the_everyday_token_is_served_to_no_chat_until_a_sign_in_without_workflow() {
    purlis_core::unsteered!();
    let scene = scene();
    let github = registration::on(Kind::GitHub, "github.com").unwrap();
    let source = || {
        SignedIn::through(
            Refresher::through(
                &scope(),
                scene.home.path(),
                Arc::new(Never),
                Endpoints::of(github),
            ),
            scene.home.path(),
        )
    };
    // Before: the everyday token is served.
    assert_eq!(
        source()
            .credential_at("github.com", now())
            .unwrap()
            .expose(),
        EVERYDAY
    );
    // GitHub folds `workflow` into it.
    scene
        .tokens
        .know(SECOND, "octocat", "repo, read:org, workflow");
    scene
        .tokens
        .know(EVERYDAY, "octocat", "repo, read:org, workflow");
    let outcome = scene
        .second_sign_in(SECOND, "repo,read:org,workflow")
        .unwrap();
    assert!(matches!(outcome, Outcome::Folded(_)));
    // The account is marked by what it holds, and no route of a chat is given its token.
    let listed = scene.listed();
    assert!(accounts::everyday_holds_workflow(&listed));
    let why = source().credential_at("github.com", now()).unwrap_err();
    assert!(why.is_sign_in_again(), "{why:?}");
    let said = why.to_string();
    assert!(
        said.contains("Revoke") && said.contains("sign in again"),
        "{said}"
    );
    assert!(!said.contains(EVERYDAY) && !said.contains(SECOND), "{said}");
    let by_id = source()
        .credential_of("github.com", &listed.id, now())
        .unwrap_err();
    assert!(by_id.is_sign_in_again(), "{by_id:?}");
    // The person revokes purlis on GitHub and signs in again, without allowing CI files: served.
    const NARROW: &str = "canaryNARROWtoken5d2";
    scene.tokens.know(NARROW, "octocat", "repo, read:org");
    let keyring = scene.keyring();
    let reach = scene.tokens.reach();
    accounts::sign_in_with_grant(
        &scope(),
        scene.home.path(),
        keyring.as_ref(),
        &reach,
        Kind::GitHub,
        "github.com",
        grant_that_does_not_expire(NARROW, "repo,read:org"),
        Method::Device,
        "Ov23litEn94I5HXrpdsX",
    )
    .unwrap();
    assert!(!accounts::everyday_holds_workflow(&scene.listed()));
    assert_eq!(
        source()
            .credential_at("github.com", now())
            .unwrap()
            .expose(),
        NARROW
    );
}

#[test]
fn a_grant_for_another_login_or_without_workflow_keeps_nothing() {
    purlis_core::unsteered!();
    let scene = scene();
    scene
        .tokens
        .know("canaryOTHERlogin", "mallory", "repo, read:org, workflow");
    let why = scene
        .second_sign_in("canaryOTHERlogin", "repo,read:org,workflow")
        .unwrap_err();
    assert!(why.contains("not as octocat"), "{why}");
    scene
        .tokens
        .know("canaryNOworkflow", "octocat", "repo, read:org");
    let why = scene
        .second_sign_in("canaryNOworkflow", "repo,read:org")
        .unwrap_err();
    assert!(why.contains("did not allow workflow"), "{why}");
    assert!(!why.contains("canary"), "{why}");
    assert!(
        scene
            .keyring()
            .get(&accounts::service(&scene.account), "workflow")
            .unwrap()
            .is_none()
    );
    // A token the forge does not know is refused, and says nothing of it.
    assert!(
        scene
            .second_sign_in("canaryUNKNOWN", "repo,workflow")
            .is_err()
    );
}

#[test]
fn a_gitlab_account_has_no_second_token_and_its_one_serves_the_app_for_a_held_push() {
    purlis_core::unsteered!();
    let home = tempfile::tempdir().unwrap();
    let tokens = Tokens::default();
    tokens.know("canaryGITLABapi", "octocat", "api");
    let keyring = accounts::keyring_at(home.path());
    let reach = tokens.reach();
    let account = accounts::sign_in_with_grant(
        &scope(),
        home.path(),
        keyring.as_ref(),
        &reach,
        Kind::GitLab,
        "gitlab.com",
        grant("canaryGITLABapi", "canaryGITLABrefresh", "api"),
        Method::Pkce,
        "Ov23litEn94I5HXrpdsX",
    )
    .unwrap();
    assert_eq!(
        workflow::availability(keyring.as_ref(), &account),
        Availability::Ready
    );
    let why = workflow::keep(
        &scope(),
        home.path(),
        keyring.as_ref(),
        &reach,
        &account,
        grant("canaryOTHER", "r", "api"),
    )
    .unwrap_err();
    assert!(why.contains("no separate permission"), "{why}");
    assert_eq!(
        token_of(workflow::for_ci_push(&scope(), keyring.as_ref(), &account).unwrap()),
        "canaryGITLABapi"
    );
}

#[test]
fn signing_out_deletes_the_second_token_with_the_first() {
    purlis_core::unsteered!();
    let scene = scene();
    scene
        .tokens
        .know(SECOND, "octocat", "repo, read:org, workflow");
    scene
        .second_sign_in(SECOND, "repo,read:org,workflow")
        .unwrap();
    let keyring = scene.keyring();
    accounts::sign_out(
        &scope(),
        scene.home.path(),
        keyring.as_ref(),
        &scene.account.id,
    )
    .unwrap();
    let service = accounts::service(&scene.account);
    assert!(keyring.get(&service, "token").unwrap().is_none());
    assert!(keyring.get(&service, "workflow").unwrap().is_none());
}

#[test]
fn the_second_token_is_read_by_one_call_alone() {
    purlis_core::unsteered!();
    // A source scan: only `for_ci_push` reads the item, and nothing calls it but the window.
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut readers = Vec::new();
    let mut stack = vec![src];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs")
                && std::fs::read_to_string(&path)
                    .unwrap()
                    .lines()
                    .filter(|l| !l.trim_start().starts_with("//"))
                    .any(|l| {
                        l.contains("for_ci_push")
                            && !path.ends_with("workflow.rs")
                            && !path.ends_with("provision.rs")
                    })
            {
                readers.push(path);
            }
        }
    }
    assert!(readers.is_empty(), "{readers:?}");
}

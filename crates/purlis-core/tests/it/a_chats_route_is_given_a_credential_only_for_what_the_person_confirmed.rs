// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat's forge route is given a credential only for what the person confirmed** (ADR 0128
//! D1.1, D1.3 rule 2, D2, D4), through the real path and no glue.
//!
//! What is real here: the accounts file and the keyring item a sign-in writes, the list of repos a
//! press confirmed, the trust list a clone from the window writes, [`SignedIn`], the credential
//! source the proxy asks ([`ChatRoute`]) and the proxy ([`ForgeProxy`]) that decides the request
//! first. A request for the chat's own repo is forwarded **with the chosen account's token**; every
//! other case below is refused with [`Refusal::NoCredential`] and no token leaves the keyring:
//!
//! - a clone the person has not trusted, and a repo nobody confirmed;
//! - a repo recorded with no account chosen (two accounts on the host), until the person chooses;
//! - a chosen account that is signed out, or whose token holds `workflow`;
//! - a chosen account whose application is no longer the host's pin (a Notice for it is raised);
//! - a chat that was not given the clone.

use std::path::Path;
use std::sync::Arc;

use purlis_core::forge::Kind;
use purlis_core::forge::accounts::{self, ForgeAccount, Method};
use purlis_core::forge::confirmed::{self, Candidate, Serving};
use purlis_core::forge::credential::SignedIn;
use purlis_core::forge::credential::chatroute::ChatRoute;
use purlis_core::forge::gitcredential::remote::record_for_a_test;
use purlis_core::forge::oauth::{Answer, Endpoint, Endpoints};
use purlis_core::forge::proxy::{
    AllowedRepo, ChatId, Credential, CredentialError, CredentialRequest, CredentialSource,
    ForgeProxy, Inbound, NoObjects, Outcome, Refusal, RequestHead,
};
use purlis_core::forge::refresh::Refresher;
use purlis_core::forge::registered;
use purlis_core::forge::registration;
use purlis_core::forge::route::HostScope;

use crate::support::one_login::{Tokens, grant_that_does_not_expire};

const PERSONAL: &str = "canaryPERSONALroute3e8";
const EMU: &str = "canaryEMUroute6b2";
const OWN_CLIENT: &str = "Ov23litEn94I5HXrpdsX";

fn scope() -> HostScope {
    HostScope::for_a_test()
}

/// An endpoint that must never be asked: these tokens do not expire.
struct Never;

impl Endpoint for Never {
    fn post(&self, url: &str, _form: &[(&str, &str)]) -> Result<Answer, String> {
        panic!("the forge was asked at {url}")
    }
}

/// The route, shared between the proxy that asks it and the test that enrols chats with it.
struct Shared(Arc<ChatRoute>);

impl CredentialSource for Shared {
    fn credential_for(&self, request: &CredentialRequest) -> Result<Credential, CredentialError> {
        self.0.credential_for(request)
    }
}

struct Scene {
    home: tempfile::TempDir,
    personal: ForgeAccount,
    emu: ForgeAccount,
}

fn sign_in(
    scene_home: &Path,
    tokens: &Tokens,
    host: &str,
    access: &str,
    scopes: &str,
    client_id: &str,
) -> ForgeAccount {
    let keyring = accounts::keyring_at(scene_home);
    let reach = tokens.reach();
    accounts::sign_in_with_grant(
        &scope(),
        scene_home,
        keyring.as_ref(),
        &reach,
        Kind::GitHub,
        host,
        grant_that_does_not_expire(access, scopes),
        Method::Device,
        client_id,
    )
    .unwrap()
}

/// Two accounts on github.com, as a person with a personal and a managed one has.
fn scene() -> Scene {
    let home = tempfile::tempdir().unwrap();
    let tokens = Tokens::default();
    tokens.know(PERSONAL, "octocat", "repo, read:org");
    tokens.know(EMU, "octocat_acme", "repo, read:org");
    let personal = sign_in(
        home.path(),
        &tokens,
        "github.com",
        PERSONAL,
        "repo,read:org",
        OWN_CLIENT,
    );
    let emu = sign_in(
        home.path(),
        &tokens,
        "github.com",
        EMU,
        "repo,read:org",
        OWN_CLIENT,
    );
    Scene {
        home,
        personal,
        emu,
    }
}

fn route(home: &Path, registration: &'static registration::Registration) -> Arc<ChatRoute> {
    let source = SignedIn::through(
        Refresher::through(&scope(), home, Arc::new(Never), Endpoints::of(registration)),
        home,
    );
    Arc::new(ChatRoute::through(source, home))
}

fn github(home: &Path) -> Arc<ChatRoute> {
    route(home, registration::on(Kind::GitHub, "github.com").unwrap())
}

/// Trust `clone` at `place` and confirm its repo for `account`, as the window does after a clone.
fn confirm(
    home: &Path,
    clone: &Path,
    host: &str,
    path: &str,
    repo_id: u64,
    account: &ForgeAccount,
) {
    record_for_a_test(home, clone, &format!("{host}/{path}")).unwrap();
    let client_id = confirmed::application_of(home, account).unwrap();
    confirmed::confirm(
        &scope(),
        home,
        clone,
        &Candidate {
            kind: Kind::GitHub,
            host: host.into(),
            path: path.into(),
            repo_id,
        },
        account,
        &client_id,
    )
    .unwrap();
}

struct Chat {
    proxy: ForgeProxy,
    chat: ChatId,
    sentinel: String,
}

/// A chat whose one repo is `host`'s `path`, with `clones` in its workspace.
fn chat(route: &Arc<ChatRoute>, host: &str, path: &str, clones: Vec<&Path>) -> Chat {
    chat_at(route, host, 443, path, 0, clones)
}

/// [`chat`] on `host:port`, where the forge says the repo's id is `repo_id` (`0`: the id is
/// looked up from the entry's own, as the helper's callers of a repo they confirmed as 1001 or 77).
fn chat_at(
    route: &Arc<ChatRoute>,
    host: &str,
    port: u16,
    path: &str,
    repo_id: u64,
    clones: Vec<&Path>,
) -> Chat {
    let repo_id = match (repo_id, path) {
        (0, "octocat/hello") => 1001,
        (0, "acme/app") => 77,
        (0, _) => 1,
        (id, _) => id,
    };
    let proxy = ForgeProxy::new(Box::new(Shared(route.clone())), Box::new(NoObjects));
    let chat = ChatId::new("chat-a");
    let sentinel = proxy.enrol(
        &chat,
        vec![
            AllowedRepo::new(
                purlis_core::forge::proxy::Kind::GitHub,
                host,
                port,
                path,
                "main",
            )
            .unwrap()
            .with_project_id(repo_id),
        ],
    );
    route.enrol(&chat, clones.into_iter().map(Path::to_path_buf).collect());
    Chat {
        proxy,
        chat,
        sentinel: sentinel.expose().to_string(),
    }
}

impl Chat {
    /// The chat reads its repo, as `gh` does, on the host's API.
    fn reads(&self, api_host: &str, target: &str) -> Outcome {
        let head = RequestHead::new("GET", api_host, 443, target);
        let headers = vec![(
            "Authorization".to_string(),
            format!("Bearer {}", self.sentinel),
        )];
        self.proxy.handle(
            &self.chat,
            &Inbound {
                head: &head,
                headers: &headers,
                body: b"",
            },
        )
    }

    fn token_sent(&self, api_host: &str, target: &str) -> Result<String, Refusal> {
        match self.reads(api_host, target) {
            Outcome::Forward(forwarded) => Ok(forwarded
                .headers
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case("authorization"))
                .map(|(_, value)| value.clone())
                .expect("a credentialed request carries one")),
            Outcome::Refused(refusal) => Err(refusal),
        }
    }
}

const HELLO: &str = "/repos/octocat/hello";

#[test]
fn a_trusted_confirmed_repo_is_served_with_the_token_of_the_account_the_person_chose() {
    purlis_core::unsteered!();
    let scene = scene();
    let route = github(scene.home.path());
    let hello = tempfile::tempdir().unwrap();
    confirm(
        scene.home.path(),
        hello.path(),
        "github.com",
        "octocat/hello",
        1001,
        &scene.emu,
    );
    let chat = chat(&route, "github.com", "octocat/hello", vec![hello.path()]);
    let sent = chat.token_sent("api.github.com", HELLO).unwrap();
    assert_eq!(
        sent,
        format!("Bearer {EMU}"),
        "the chosen one, not the first"
    );
    assert_ne!(sent, format!("Bearer {PERSONAL}"));
    // The person chooses the other account, on a press, and the very next request follows.
    let listed = accounts::listed(scene.home.path()).unwrap();
    let personal = listed.iter().find(|a| a.id == scene.personal.id).unwrap();
    confirmed::choose_account(
        &scope(),
        scene.home.path(),
        &confirmed::key_of_clone(hello.path()),
        Kind::GitHub,
        "github.com",
        1001,
        personal,
    )
    .unwrap();
    assert_eq!(
        chat.token_sent("api.github.com", HELLO).unwrap(),
        format!("Bearer {PERSONAL}")
    );
}

#[test]
fn a_clone_the_person_does_not_trust_and_a_repo_nobody_confirmed_are_given_no_credential() {
    purlis_core::unsteered!();
    let scene = scene();
    let route = github(scene.home.path());
    // Confirmed, then the trust is taken away: a confirmed id on a clone the person no longer
    // trusts is not theirs to serve.
    let hello = tempfile::tempdir().unwrap();
    confirm(
        scene.home.path(),
        hello.path(),
        "github.com",
        "octocat/hello",
        1001,
        &scene.personal,
    );
    std::fs::remove_file(
        purlis_core::machine::dir(scene.home.path())
            .join(purlis_core::forge::gitcredential::PUSH_REMOTES),
    )
    .unwrap();
    let chat_one = chat(&route, "github.com", "octocat/hello", vec![hello.path()]);
    assert_eq!(
        chat_one.token_sent("api.github.com", HELLO).unwrap_err(),
        Refusal::NoCredential
    );
    // Trusted from the CLI's own clone, but never confirmed: no entry, no credential.
    let cli = tempfile::tempdir().unwrap();
    record_for_a_test(scene.home.path(), cli.path(), "github.com/octocat/hello").unwrap();
    let chat_two = chat(&route, "github.com", "octocat/hello", vec![cli.path()]);
    assert_eq!(
        chat_two.token_sent("api.github.com", HELLO).unwrap_err(),
        Refusal::NoCredential
    );
}

#[test]
fn a_repo_recorded_with_no_account_chosen_is_served_by_none_until_the_person_chooses() {
    purlis_core::unsteered!();
    let scene = scene();
    let route = github(scene.home.path());
    let hello = tempfile::tempdir().unwrap();
    record_for_a_test(scene.home.path(), hello.path(), "github.com/octocat/hello").unwrap();
    let repo = confirmed::record_unchosen(
        &scope(),
        scene.home.path(),
        hello.path(),
        &Candidate {
            kind: Kind::GitHub,
            host: "github.com".into(),
            path: "octocat/hello".into(),
            repo_id: 1001,
        },
    )
    .unwrap();
    assert_eq!(repo.account, "", "recorded, with no account");
    let listed = accounts::listed(scene.home.path()).unwrap();
    assert_eq!(
        confirmed::account_for(scene.home.path(), &repo, &listed),
        Serving::Unchosen
    );
    let chat = chat(&route, "github.com", "octocat/hello", vec![hello.path()]);
    assert_eq!(
        chat.token_sent("api.github.com", HELLO).unwrap_err(),
        Refusal::NoCredential,
        "neither of the two accounts is picked"
    );
    // A second look at the clone never undoes what the person then chooses.
    let personal = listed.iter().find(|a| a.id == scene.personal.id).unwrap();
    confirmed::choose_account(
        &scope(),
        scene.home.path(),
        &confirmed::key_of_clone(hello.path()),
        Kind::GitHub,
        "github.com",
        1001,
        personal,
    )
    .unwrap();
    let again = confirmed::record_unchosen(
        &scope(),
        scene.home.path(),
        hello.path(),
        &Candidate {
            kind: Kind::GitHub,
            host: "github.com".into(),
            path: "octocat/hello".into(),
            repo_id: 1001,
        },
    )
    .unwrap();
    assert_eq!(again.account, scene.personal.id);
    assert_eq!(
        chat.token_sent("api.github.com", HELLO).unwrap(),
        format!("Bearer {PERSONAL}")
    );
}

#[test]
fn a_chosen_account_that_is_signed_out_is_not_replaced_by_the_other_one() {
    purlis_core::unsteered!();
    let scene = scene();
    let route = github(scene.home.path());
    let hello = tempfile::tempdir().unwrap();
    confirm(
        scene.home.path(),
        hello.path(),
        "github.com",
        "octocat/hello",
        1001,
        &scene.emu,
    );
    let chat = chat(&route, "github.com", "octocat/hello", vec![hello.path()]);
    assert!(chat.token_sent("api.github.com", HELLO).is_ok());
    let keyring = accounts::keyring_at(scene.home.path());
    accounts::sign_out(&scope(), scene.home.path(), keyring.as_ref(), &scene.emu.id).unwrap();
    assert_eq!(
        chat.token_sent("api.github.com", HELLO).unwrap_err(),
        Refusal::NoCredential,
        "the personal account is signed in, and does not stand in"
    );
}

#[test]
fn an_account_whose_token_holds_workflow_is_given_to_no_chat_even_when_it_was_chosen() {
    purlis_core::unsteered!();
    let home = tempfile::tempdir().unwrap();
    let tokens = Tokens::default();
    const FOLDED: &str = "canaryFOLDEDroute4a9";
    tokens.know(FOLDED, "octocat", "repo, read:org, workflow");
    let account = sign_in(
        home.path(),
        &tokens,
        "github.com",
        FOLDED,
        "repo,read:org,workflow",
        OWN_CLIENT,
    );
    assert!(accounts::everyday_holds_workflow(&account));
    let route = github(home.path());
    let hello = tempfile::tempdir().unwrap();
    confirm(
        home.path(),
        hello.path(),
        "github.com",
        "octocat/hello",
        1001,
        &account,
    );
    let chat = chat(&route, "github.com", "octocat/hello", vec![hello.path()]);
    assert_eq!(
        chat.token_sent("api.github.com", HELLO).unwrap_err(),
        Refusal::NoCredential
    );
    let listed = accounts::listed(home.path()).unwrap();
    let repos = confirmed::repos_of_clone(home.path(), hello.path()).unwrap();
    assert!(matches!(
        confirmed::account_for(home.path(), &repos[0], &listed),
        Serving::ChosenIsSignedOut { .. }
    ));
}

#[test]
fn an_account_signed_in_through_an_application_the_host_no_longer_pins_serves_no_chat() {
    purlis_core::unsteered!();
    let home = tempfile::tempdir().unwrap();
    let tokens = Tokens::default();
    const GHES: &str = "ghe.example.com";
    const ORG_APP: &str = "Iv1.orgapp0123456789";
    const OTHER_APP: &str = "Iv1.lookalike99999999";
    const TOKEN: &str = "canaryGHESroute7c1";
    tokens.know(TOKEN, "octocat", "repo, read:org");
    registered::pin_with_owner(
        &scope(),
        home.path(),
        Kind::GitHub,
        GHES,
        ORG_APP,
        "acme-platform",
    )
    .unwrap();
    let account = sign_in(home.path(), &tokens, GHES, TOKEN, "repo,read:org", ORG_APP);
    let hello = tempfile::tempdir().unwrap();
    confirm(home.path(), hello.path(), GHES, "acme/app", 77, &account);
    let pinned = registered::pinned(home.path(), Kind::GitHub, GHES)
        .unwrap()
        .unwrap();
    let route = route(home.path(), pinned);
    let chat = chat(&route, GHES, "acme/app", vec![hello.path()]);
    let target = "/api/v3/repos/acme/app";
    assert_eq!(
        chat.token_sent(GHES, target).unwrap(),
        format!("Bearer {TOKEN}"),
        "the pinned application's sign-in serves"
    );
    // The person (or a sync) pins another application: the account is another application's.
    registered::pin_with_owner(
        &scope(),
        home.path(),
        Kind::GitHub,
        GHES,
        OTHER_APP,
        "acme-platform",
    )
    .unwrap();
    assert_eq!(
        chat.token_sent(GHES, target).unwrap_err(),
        Refusal::NoCredential
    );
    let listed = accounts::listed(home.path()).unwrap();
    let repos = confirmed::repos_of_clone(home.path(), hello.path()).unwrap();
    assert_eq!(
        confirmed::account_for(home.path(), &repos[0], &listed),
        Serving::ApplicationChanged {
            account_id: account.id.clone()
        }
    );
    // `serves` refuses on its own, too: the repo was recorded under the application that is gone.
    let served = confirmed::serves(
        home.path(),
        hello.path(),
        Kind::GitHub,
        GHES,
        "acme/app",
        77,
    )
    .unwrap();
    assert_eq!(served, confirmed::Admission::NotConfirmed);
}

#[test]
fn another_chat_is_given_nothing_for_a_clone_it_was_not_given() {
    purlis_core::unsteered!();
    let scene = scene();
    let route = github(scene.home.path());
    let hello = tempfile::tempdir().unwrap();
    confirm(
        scene.home.path(),
        hello.path(),
        "github.com",
        "octocat/hello",
        1001,
        &scene.personal,
    );
    let owner = chat(&route, "github.com", "octocat/hello", vec![hello.path()]);
    assert!(owner.token_sent("api.github.com", HELLO).is_ok());
    // A chat that names the same repo but holds no such clone, and one the route never heard of.
    let request = |chat: &ChatId| CredentialRequest {
        kind: purlis_core::forge::proxy::Kind::GitHub,
        host: "github.com".into(),
        port: 443,
        repo_id: Some(1001),
        chat: chat.clone(),
        repo: "octocat/hello".into(),
    };
    route.enrol(&ChatId::new("chat-b"), Vec::new());
    for stranger in ["chat-b", "chat-c"] {
        assert!(matches!(
            route.credential_for(&request(&ChatId::new(stranger))),
            Err(CredentialError::Unavailable)
        ));
    }
    // And a chat that ended is served nothing more.
    route.leave(&owner.chat);
    assert!(matches!(
        route.credential_for(&request(&owner.chat)),
        Err(CredentialError::Unavailable)
    ));
}

#[test]
fn a_repo_replaced_under_the_same_name_by_another_id_is_given_no_credential() {
    purlis_core::unsteered!();
    let scene = scene();
    let route = github(scene.home.path());
    let hello = tempfile::tempdir().unwrap();
    confirm(
        scene.home.path(),
        hello.path(),
        "github.com",
        "octocat/hello",
        1001,
        &scene.personal,
    );
    let same = chat_at(
        &route,
        "github.com",
        443,
        "octocat/hello",
        1001,
        vec![hello.path()],
    );
    assert!(same.token_sent("api.github.com", HELLO).is_ok());
    // The repo was renamed away and a new one took the name: the forge now says id 2002.
    let replaced = chat_at(
        &route,
        "github.com",
        443,
        "octocat/hello",
        2002,
        vec![hello.path()],
    );
    assert_eq!(
        replaced.token_sent("api.github.com", HELLO).unwrap_err(),
        Refusal::NoCredential
    );
}

#[test]
fn a_request_that_names_a_repo_without_the_forges_id_is_given_no_credential() {
    purlis_core::unsteered!();
    let scene = scene();
    let route = github(scene.home.path());
    let hello = tempfile::tempdir().unwrap();
    confirm(
        scene.home.path(),
        hello.path(),
        "github.com",
        "octocat/hello",
        1001,
        &scene.personal,
    );
    let owner = chat(&route, "github.com", "octocat/hello", vec![hello.path()]);
    assert!(owner.token_sent("api.github.com", HELLO).is_ok());
    let request = CredentialRequest {
        kind: purlis_core::forge::proxy::Kind::GitHub,
        host: "github.com".into(),
        port: 443,
        repo_id: None,
        chat: owner.chat.clone(),
        repo: "octocat/hello".into(),
    };
    assert!(matches!(
        route.credential_for(&request),
        Err(CredentialError::Unavailable)
    ));
}

#[test]
fn a_token_confirmed_for_one_port_is_not_sent_to_another_in_either_direction() {
    purlis_core::unsteered!();
    let home = tempfile::tempdir().unwrap();
    let tokens = Tokens::default();
    const GHES: &str = "git.corp";
    const ODD: &str = "git.corp:8443";
    const ORG_APP: &str = "Iv1.orgapp0123456789";
    const TOKEN: &str = "canaryPORTroute5d2";
    const TOKEN_ODD: &str = "canaryPORTodd5d3";
    tokens.know(TOKEN, "octocat", "repo, read:org");
    tokens.know(TOKEN_ODD, "octocat_odd", "repo, read:org");
    for host in [GHES, ODD] {
        registered::pin_with_owner(&scope(), home.path(), Kind::GitHub, host, ORG_APP, "acme")
            .unwrap();
    }
    let account = sign_in(home.path(), &tokens, GHES, TOKEN, "repo,read:org", ORG_APP);
    let account_odd = sign_in(
        home.path(),
        &tokens,
        ODD,
        TOKEN_ODD,
        "repo,read:org",
        ORG_APP,
    );
    let pinned = registered::pinned(home.path(), Kind::GitHub, GHES)
        .unwrap()
        .unwrap();
    let route = route(home.path(), pinned);
    // One clone confirmed at the default port, another at 8443.
    let plain = tempfile::tempdir().unwrap();
    let odd = tempfile::tempdir().unwrap();
    confirm(home.path(), plain.path(), GHES, "acme/app", 77, &account);
    confirm(home.path(), odd.path(), ODD, "acme/app", 77, &account_odd);
    let target = "/api/v3/repos/acme/app";
    // Confirmed for `git.corp` (443): served there, refused on `git.corp:8443`.
    let at_443 = chat_at(&route, GHES, 443, "acme/app", 77, vec![plain.path()]);
    assert_eq!(
        at_443.token_sent(GHES, target).unwrap(),
        format!("Bearer {TOKEN}")
    );
    // The proxy's own policy reads the port too; the route is asked directly, as the second wall.
    let ask = |chat: &Chat, host: &str, port: u16| {
        route.credential_for(&CredentialRequest {
            kind: purlis_core::forge::proxy::Kind::GitHub,
            host: host.into(),
            port,
            repo_id: Some(77),
            chat: chat.chat.clone(),
            repo: "acme/app".into(),
        })
    };
    assert!(
        matches!(ask(&at_443, GHES, 8443), Err(CredentialError::Unavailable)),
        "a token confirmed for git.corp:443 is not sent to git.corp:8443"
    );
    // Confirmed for `git.corp:8443`: served there, refused on 443.
    let at_8443 = chat_at(&route, GHES, 8443, "acme/app", 77, vec![odd.path()]);
    assert!(ask(&at_8443, GHES, 8443).is_ok());
    assert!(
        matches!(ask(&at_8443, GHES, 443), Err(CredentialError::Unavailable)),
        "a token confirmed for git.corp:8443 is not sent to git.corp:443"
    );
}

#[test]
fn an_account_whose_scopes_the_forge_never_said_is_given_to_no_chat() {
    purlis_core::unsteered!();
    let home = tempfile::tempdir().unwrap();
    let tokens = Tokens::default();
    const FINE: &str = "canaryFINEgrained8e3";
    tokens.know(FINE, "octocat", "repo, read:org");
    let account = sign_in(
        home.path(),
        &tokens,
        "github.com",
        FINE,
        "repo,read:org",
        OWN_CLIENT,
    );
    let route = github(home.path());
    let hello = tempfile::tempdir().unwrap();
    confirm(
        home.path(),
        hello.path(),
        "github.com",
        "octocat/hello",
        1001,
        &account,
    );
    let chat = chat(&route, "github.com", "octocat/hello", vec![hello.path()]);
    assert!(chat.token_sent("api.github.com", HELLO).is_ok());
    // The forge's read-back no longer says what the token holds: it may hold `workflow`.
    purlis_core::forge::accounts::forget_scopes_for_a_test(home.path(), &account).unwrap();
    assert_eq!(
        chat.token_sent("api.github.com", HELLO).unwrap_err(),
        Refusal::NoCredential
    );
}

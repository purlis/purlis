// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Signing in to GitHub by the device flow, end to end, against a stand-in** (#729; ADR 0077
//! §3 as amended 2026-10-12). No case here reaches github.com.
//!
//! Each case drives what the person does: start a sign-in, get a code and GitHub's page, confirm
//! it, and come back signed in. It asserts what they then have: the account listed as a browser
//! sign-in on github.com, its token pair in the account's own keyring item and nowhere else, the
//! window's next request sent with a token that was renewed before it expired, and a sign-in the
//! forge ended listed as signed out with a sentence that says to sign in again.
//!
//! Two stand-ins. [`Scripted`] answers each posted form in this process, so the flow's every
//! request can be read back: it runs anywhere. The last case sends the same flow over HTTP to a
//! stand-in server, so the forms are shown to go out as the forge reads them, with no secret.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use purlis_core::forge::accounts::{self, ForgeAccount, Method, Pasted, State};
use purlis_core::forge::device::{self, Ended, Pace};
use purlis_core::forge::http::{ApiRoot, TokenSource};
use purlis_core::forge::oauth::{Answer, Endpoint, Endpoints, Https};
use purlis_core::forge::recorded::Recorded;
use purlis_core::forge::refresh::Refresher;
use purlis_core::forge::registration::{self, Registration};
use purlis_core::forge::route::HostScope;
use purlis_core::forge::transport::Transport;
use purlis_core::forge::{Failure, Forge, ForgeError, Kind};
use purlis_core::secrets::keyring::Store;
use secrecy::ExposeSecret;

/// What the stand-in grants. Not shaped like any forge's token: the commit hook scans for those.
const ACCESS: &str = "stand-in-device-access-one";
const REFRESH: &str = "stand-in-device-refresh-one";
const ACCESS_TWO: &str = "stand-in-device-access-two";
const REFRESH_TWO: &str = "stand-in-device-refresh-two";
const DEVICE_CODE: &str = "stand-in-device-code-in-memory";

/// One posted form: the URL it went to, and its fields in order.
type Posted = (String, Vec<(String, String)>);

/// A stand-in forge's OAuth endpoints, in this process: each posted form answered from a
/// script, in order, and kept.
#[derive(Default)]
struct Scripted {
    answers: Mutex<Vec<(u16, String)>>,
    posted: Mutex<Vec<Posted>>,
}

impl Scripted {
    fn answering(answers: &[(u16, String)]) -> Arc<Scripted> {
        Arc::new(Scripted {
            answers: Mutex::new(answers.iter().rev().cloned().collect()),
            posted: Mutex::new(Vec::new()),
        })
    }

    fn posted(&self) -> Vec<Posted> {
        self.posted.lock().unwrap().clone()
    }
}

impl Endpoint for Scripted {
    fn post(&self, url: &str, form: &[(&str, &str)]) -> Result<Answer, String> {
        self.posted.lock().unwrap().push((
            url.to_string(),
            form.iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        ));
        match self.answers.lock().unwrap().pop() {
            Some((status, body)) => Ok(Answer::of(status, &body)),
            None => Err("the stand-in was asked more than its script answers".to_string()),
        }
    }
}

/// Never sleeps: the stand-in answers at once.
struct AtOnce;

impl Pace for AtOnce {
    fn wait(&self, _for: Duration) -> bool {
        true
    }
}

fn codes(page: &str) -> (u16, String) {
    (
        200,
        format!(
            r#"{{"device_code": "{DEVICE_CODE}", "user_code": "WDJB-MJHT",
                "verification_uri": "{page}", "expires_in": 900, "interval": 5}}"#
        ),
    )
}

fn pending() -> (u16, String) {
    (200, r#"{"error": "authorization_pending"}"#.to_string())
}

fn grant(access: &str, refresh: &str) -> (u16, String) {
    (
        200,
        format!(
            r#"{{"access_token": "{access}", "token_type": "bearer", "scope": "repo,read:org",
                "expires_in": 28800, "refresh_token": "{refresh}",
                "refresh_token_expires_in": 15897600}}"#
        ),
    )
}

fn github() -> (&'static Registration, Endpoints) {
    let registration = registration::on(Kind::GitHub, "github.com").unwrap();
    (registration, Endpoints::of(registration))
}

/// A config home and its keyring, and the forge every "whose token is this" is asked of: the
/// account store's own recording of `GET /user`, which answers `octocat`.
struct Machine {
    home: tempfile::TempDir,
    keyring: Box<dyn Store>,
    /// The token each check was asked with, and the forge it was sent to.
    asked: Mutex<Vec<(Forge, String)>>,
}

impl Machine {
    fn new() -> Machine {
        let home = tempfile::tempdir().unwrap();
        let keyring = accounts::keyring_at(home.path());
        Machine {
            home,
            keyring,
            asked: Mutex::new(Vec::new()),
        }
    }

    fn config(&self) -> &Path {
        self.home.path()
    }

    fn recorded(kind: Kind, case: &str) -> Arc<Recorded> {
        let file = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/forge_contract/accounts")
            .join(kind.word())
            .join(format!("{case}.json"));
        Arc::new(Recorded::parse(&std::fs::read_to_string(file).unwrap()).unwrap())
    }

    /// The whole sign-in, as the window runs it: start, wait, and the account kept.
    fn sign_in(&self, forge: &Arc<Scripted>) -> Result<ForgeAccount, Ended> {
        let (registration, at) = github();
        let started = device::start(forge.as_ref(), &at, registration).map_err(Ended::Failed)?;
        assert_eq!(started.user_code, "WDJB-MJHT");
        assert_eq!(started.verification_uri, "https://github.com/login/device");
        let recorded = Machine::recorded(Kind::GitHub, "token_check");
        let reach = |forge: &Forge,
                     tokens: Arc<dyn TokenSource>|
         -> Result<Arc<dyn Transport>, ForgeError> {
            let token = tokens.token().unwrap().expose_secret().to_string();
            self.asked.lock().unwrap().push((forge.clone(), token));
            Ok(recorded.clone() as Arc<dyn Transport>)
        };
        device::finish(
            &HostScope::for_a_test(),
            self.config(),
            self.keyring.as_ref(),
            &reach,
            forge.as_ref(),
            &at,
            registration,
            &started,
            &AtOnce,
        )
    }

    /// A token pasted for `kind` on `host`, the forge answering `octocat` or `jdoe`.
    fn paste(&self, kind: Kind, host: &str, token: &str) -> ForgeAccount {
        let recorded = Machine::recorded(kind, "token_check");
        let reach =
            |_: &Forge, _: Arc<dyn TokenSource>| -> Result<Arc<dyn Transport>, ForgeError> {
                Ok(recorded.clone() as Arc<dyn Transport>)
            };
        accounts::sign_in_with_token(
            &HostScope::for_a_test(),
            self.config(),
            self.keyring.as_ref(),
            &reach,
            kind,
            host,
            Pasted::new(token.to_string()),
        )
        .unwrap()
    }

    fn list(&self) -> Vec<ForgeAccount> {
        accounts::list(self.config(), self.keyring.as_ref()).unwrap()
    }

    fn token(&self, account: &ForgeAccount) -> Option<String> {
        accounts::token(self.keyring.as_ref(), account)
            .unwrap()
            .map(|t| t.expose_secret().to_string())
    }

    /// The account's keyring item, as it is kept.
    fn item(&self, account: &ForgeAccount) -> Option<serde_json::Value> {
        self.keyring
            .get(&accounts::service(account), "token")
            .unwrap()
            .map(|s| serde_json::from_str(&s.into_inner()).unwrap())
    }

    /// Every file under the config home but the test keyring, as text.
    fn files_but_the_keyring(&self) -> Vec<(PathBuf, String)> {
        let mut found = Vec::new();
        let mut stack = vec![self.config().to_path_buf()];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.file_name().is_some_and(|n| n != "keyring-stub.json") {
                    found.push((
                        path.clone(),
                        String::from_utf8_lossy(&std::fs::read(&path).unwrap()).into_owned(),
                    ));
                }
            }
        }
        found
    }

    fn refresher(&self, forge: &Arc<Scripted>) -> Refresher {
        let (_, at) = github();
        Refresher::through(&HostScope::for_a_test(), self.config(), forge.clone(), at)
    }

    fn expires_at(&self, account: &ForgeAccount) -> u64 {
        self.item(account).unwrap()["expires_at"].as_u64().unwrap()
    }
}

#[test]
fn a_person_confirms_the_code_and_comes_back_signed_in_on_github_com() {
    purlis_core::unsteered!();
    let machine = Machine::new();
    let forge = Scripted::answering(&[
        codes("https://github.com/login/device"),
        pending(),
        (200, r#"{"error": "slow_down", "interval": 10}"#.to_string()),
        grant(ACCESS, REFRESH),
    ]);
    let account = machine.sign_in(&forge).unwrap();

    // The account, as the Accounts page lists it.
    assert_eq!(
        (account.kind, account.host.as_str(), account.login.as_str()),
        (Kind::GitHub, "github.com", "octocat")
    );
    assert_eq!(account.method, Method::Device);
    assert_eq!(account.state, State::SignedIn);
    assert_eq!(
        account.scopes,
        Some(vec!["repo".to_string(), "read:org".to_string()])
    );
    assert_eq!(machine.list(), vec![account.clone()]);

    // The token the forge granted is what it was asked "whose is this" with, on github.com.
    assert_eq!(
        *machine.asked.lock().unwrap(),
        [(Forge::default_of(Kind::GitHub), ACCESS.to_string())]
    );

    // One keyring item, the same one a token sign-in uses, holding the pair and its expiry.
    assert_eq!(
        accounts::service(&account),
        format!("purlis/@forge/github.com/{}", account.id)
    );
    let item = machine.item(&account).unwrap();
    assert_eq!(item["access"], ACCESS);
    assert_eq!(item["refresh"], REFRESH);
    assert!(item["expires_at"].as_u64().unwrap() > 0);
    assert_eq!(machine.token(&account).as_deref(), Some(ACCESS));

    // And it is in no other file: the accounts file names the account and holds no token.
    for (path, text) in machine.files_but_the_keyring() {
        for secret in [ACCESS, REFRESH, DEVICE_CODE] {
            assert!(!text.contains(secret), "{} holds {secret}", path.display());
        }
    }
    let listed: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(accounts::path(machine.config())).unwrap())
            .unwrap();
    assert_eq!(listed["accounts"][0]["method"], "device");
    assert_eq!(listed["accounts"][0]["state"], "signed-in");

    // What was posted: the public client id and the scopes, then the device code, and never a
    // secret.
    let posted = forge.posted();
    assert_eq!(posted.len(), 4);
    assert_eq!(posted[0].0, "https://github.com/login/device/code");
    assert_eq!(
        posted[0].1,
        [
            ("client_id".to_string(), "Ov23litEn94I5HXrpdsX".to_string()),
            ("scope".to_string(), "repo read:org".to_string()),
        ]
    );
    for (url, form) in &posted[1..] {
        assert_eq!(url, "https://github.com/login/oauth/access_token");
        let names: Vec<&str> = form.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(names, ["client_id", "device_code", "grant_type"]);
    }

    // No log line or error can carry any of it from here on.
    for secret in [ACCESS, REFRESH, DEVICE_CODE] {
        assert!(purlis_core::applog::holds_a_value_kept_out(secret));
    }
}

#[test]
fn a_code_that_lapses_or_is_refused_signs_nobody_in_and_keeps_nothing() {
    purlis_core::unsteered!();
    for (answer, ended, says) in [
        (
            "expired_token",
            Ended::Expired,
            "Sign in again to get a new",
        ),
        ("access_denied", Ended::Denied, "was not signed in"),
    ] {
        let machine = Machine::new();
        let forge = Scripted::answering(&[
            codes("https://github.com/login/device"),
            pending(),
            (200, format!(r#"{{"error": "{answer}"}}"#)),
        ]);
        let why = machine.sign_in(&forge).unwrap_err();
        assert_eq!(why, ended);
        assert!(why.said("GitHub").contains(says), "{}", why.said("GitHub"));
        assert!(machine.list().is_empty());
        assert!(machine.files_but_the_keyring().is_empty());
    }
}

#[test]
fn a_grant_without_a_scope_purlis_needs_is_refused_naming_it() {
    purlis_core::unsteered!();
    let machine = Machine::new();
    let forge = Scripted::answering(&[
        codes("https://github.com/login/device"),
        (
            200,
            format!(r#"{{"access_token": "{ACCESS}", "scope": "repo", "expires_in": 28800}}"#),
        ),
    ]);
    let Err(Ended::Failed(why)) = machine.sign_in(&forge) else {
        panic!("a grant without read:org was taken");
    };
    assert!(why.contains("without the read:org scope"), "{why}");
    assert!(why.contains("Sign in again"), "{why}");
    assert!(!why.contains(ACCESS), "{why}");
    assert!(machine.list().is_empty());
}

#[test]
fn a_sign_in_is_one_account_by_forge_host_and_login_and_is_never_used_on_another_host() {
    purlis_core::unsteered!();
    let machine = Machine::new();
    // The same login, octocat, on a GitHub Enterprise server, signed in with a token.
    let enterprise = machine.paste(Kind::GitHub, "ghe.example.com", "pasted-for-the-enterprise");
    // And jdoe on gitlab.com.
    let gitlab = machine.paste(Kind::GitLab, "gitlab.com", "pasted-for-gitlab-com");
    // Then the browser sign-in on github.com, as octocat.
    let forge = Scripted::answering(&[
        codes("https://github.com/login/device"),
        grant(ACCESS, REFRESH),
    ]);
    let github = machine.sign_in(&forge).unwrap();

    // Three accounts, each its own, each with its own keyring item.
    assert_eq!(enterprise.login, github.login);
    assert_ne!(enterprise.id, github.id);
    assert_eq!(
        machine.list(),
        vec![enterprise.clone(), gitlab.clone(), github.clone()]
    );
    assert_ne!(accounts::service(&enterprise), accounts::service(&github));

    // Each host answers with its own account, and its own token.
    let on = |kind, host: &str| {
        accounts::signed_in_on(machine.config(), kind, host)
            .unwrap()
            .map(|account| account.id)
    };
    assert_eq!(on(Kind::GitHub, "github.com"), Some(github.id.clone()));
    assert_eq!(
        on(Kind::GitHub, "ghe.example.com"),
        Some(enterprise.id.clone())
    );
    assert_eq!(on(Kind::GitLab, "gitlab.com"), Some(gitlab.id.clone()));
    assert_eq!(
        on(Kind::GitLab, "github.com"),
        None,
        "a host is one forge's"
    );
    assert_eq!(on(Kind::GitHub, "api.github.com"), None);
    assert_eq!(
        machine.token(&enterprise).as_deref(),
        Some("pasted-for-the-enterprise")
    );
    assert_eq!(machine.token(&github).as_deref(), Some(ACCESS));

    // The browser sign-in's token was checked with github.com alone.
    let asked = machine.asked.lock().unwrap().clone();
    assert!(
        asked
            .iter()
            .all(|(forge, _)| forge.host == "github.com" && forge.kind == Kind::GitHub),
        "{asked:?}"
    );

    // purlis has a registration on github.com and gitlab.com, and on no other host.
    assert!(registration::on(Kind::GitHub, "ghe.example.com").is_none());

    // A refresh is for the github.com account alone: the enterprise account's token, which is
    // a pasted one, is handed out as it is and nothing is posted for it, however late it is.
    let renewing = Scripted::answering(&[]);
    let refresher = machine.refresher(&renewing);
    let late = machine.expires_at(&github) + 3600;
    let token = refresher.token_at(&enterprise, late).unwrap();
    assert_eq!(token.expose_secret(), "pasted-for-the-enterprise");
    assert!(renewing.posted().is_empty());

    // Signing in again on github.com as octocat is the same account, and touches no other.
    let again = Scripted::answering(&[
        codes("https://github.com/login/device"),
        grant(ACCESS_TWO, REFRESH_TWO),
    ]);
    let second = machine.sign_in(&again).unwrap();
    assert_eq!(second.id, github.id);
    assert_eq!(machine.list().len(), 3);
    assert_eq!(machine.token(&second).as_deref(), Some(ACCESS_TWO));
    assert_eq!(
        machine.token(&enterprise).as_deref(),
        Some("pasted-for-the-enterprise")
    );
}

#[test]
fn a_browser_sign_in_takes_over_a_token_account_of_the_same_login_on_the_same_host() {
    purlis_core::unsteered!();
    let machine = Machine::new();
    let pasted = machine.paste(Kind::GitHub, "github.com", "pasted-for-github-com");
    assert_eq!(pasted.method, Method::Pat);
    let forge = Scripted::answering(&[
        codes("https://github.com/login/device"),
        grant(ACCESS, REFRESH),
    ]);
    let signed_in = machine.sign_in(&forge).unwrap();
    assert_eq!(
        signed_in.id, pasted.id,
        "one account per forge, host and login"
    );
    assert_eq!(signed_in.method, Method::Device);
    assert_eq!(machine.list(), vec![signed_in.clone()]);
    assert_eq!(machine.token(&signed_in).as_deref(), Some(ACCESS));
}

#[test]
fn the_token_is_renewed_before_it_expires_and_the_new_pair_replaces_the_old() {
    purlis_core::unsteered!();
    let machine = Machine::new();
    let forge = Scripted::answering(&[
        codes("https://github.com/login/device"),
        grant(ACCESS, REFRESH),
    ]);
    let account = machine.sign_in(&forge).unwrap();
    let expires_at = machine.expires_at(&account);

    let renewing = Scripted::answering(&[grant(ACCESS_TWO, REFRESH_TWO)]);
    let refresher = machine.refresher(&renewing);
    // With more than five minutes left, the token is used as it is.
    let token = refresher.token_at(&account, expires_at - 301).unwrap();
    assert_eq!(token.expose_secret(), ACCESS);
    assert!(renewing.posted().is_empty());
    // Inside the last five minutes, it is renewed first: client id, refresh token, no secret.
    let token = refresher.token_at(&account, expires_at - 299).unwrap();
    assert_eq!(token.expose_secret(), ACCESS_TWO);
    assert_eq!(
        renewing.posted(),
        [(
            "https://github.com/login/oauth/access_token".to_string(),
            vec![
                ("client_id".to_string(), "Ov23litEn94I5HXrpdsX".to_string()),
                ("grant_type".to_string(), "refresh_token".to_string()),
                ("refresh_token".to_string(), REFRESH.to_string()),
            ]
        )]
    );
    let item = machine.item(&account).unwrap();
    assert_eq!(item["access"], ACCESS_TWO);
    assert_eq!(item["refresh"], REFRESH_TWO);
    // What reads the item afterwards (the window's next request, its git) gets the new token.
    assert_eq!(machine.token(&account).as_deref(), Some(ACCESS_TWO));
    assert_eq!(machine.list()[0].state, State::SignedIn);
    for (path, text) in machine.files_but_the_keyring() {
        for secret in [ACCESS, REFRESH, ACCESS_TWO, REFRESH_TWO] {
            assert!(!text.contains(secret), "{} holds {secret}", path.display());
        }
    }
}

#[test]
fn a_refresh_the_forge_refuses_leaves_the_account_signed_out_and_says_to_sign_in_again() {
    purlis_core::unsteered!();
    let machine = Machine::new();
    let forge = Scripted::answering(&[
        codes("https://github.com/login/device"),
        grant(ACCESS, REFRESH),
    ]);
    let account = machine.sign_in(&forge).unwrap();
    let refused = Scripted::answering(&[(
        200,
        format!(r#"{{"error": "bad_refresh_token", "error_description": "{REFRESH} is spent"}}"#),
    )]);
    let why = machine
        .refresher(&refused)
        .token_at(&account, machine.expires_at(&account))
        .unwrap_err();

    // The sentence the window shows: which sign-in, and where to sign in again.
    assert_eq!(why.failure(), &Failure::Auth);
    assert_eq!(
        why.said(),
        "purlis's sign-in to github.com as octocat has ended, and github.com would not renew \
         it. Sign in again in Settings › You › Accounts."
    );
    // Still listed, signed out; nothing left in the keyring; no window call is sent as it.
    let listed = machine.list();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, account.id);
    assert_eq!(listed[0].state, State::SignedOut);
    assert_eq!(machine.item(&account), None);
    assert_eq!(
        accounts::signed_in_on(machine.config(), Kind::GitHub, "github.com").unwrap(),
        None
    );

    // Signing in again signs that same account in.
    let again = Scripted::answering(&[
        codes("https://github.com/login/device"),
        grant(ACCESS_TWO, REFRESH_TWO),
    ]);
    let back = machine.sign_in(&again).unwrap();
    assert_eq!(back.id, account.id);
    assert_eq!(machine.list()[0].state, State::SignedIn);
}

/// The import of an existing `gh` login (#1376) stands beside the browser sign-in: it is offered
/// while github.com has no account, whichever way one is then made, and the browser sign-in
/// takes nothing from `gh` and asks it nothing.
#[test]
fn an_existing_gh_login_is_still_offered_until_github_com_is_signed_in_either_way() {
    purlis_core::unsteered!();
    use purlis_core::forge::cliimport;
    let machine = Machine::new();
    let offered = |machine: &Machine| -> Vec<&'static str> {
        cliimport::offered(machine.config(), &|_| true, &|_| true)
            .iter()
            .map(|offer| offer.cli)
            .collect()
    };
    assert_eq!(offered(&machine), ["gh", "glab"]);

    let forge = Scripted::answering(&[
        codes("https://github.com/login/device"),
        grant(ACCESS, REFRESH),
    ]);
    let account = machine.sign_in(&forge).unwrap();
    // github.com is signed in, so its import is no longer offered; gitlab.com's still is.
    assert_eq!(offered(&machine), ["glab"]);
    // A browser sign-in is not an import: chats on the host keep the CLI's own login (V22c).
    assert!(!accounts::imported_on(
        machine.config(),
        Kind::GitHub,
        "github.com"
    ));

    // Signed out again, the offer is back.
    accounts::sign_out(
        &HostScope::for_a_test(),
        machine.config(),
        machine.keyring.as_ref(),
        &account.id,
    )
    .unwrap();
    assert_eq!(machine.item(&account), None);
    assert_eq!(offered(&machine), ["gh", "glab"]);
}

/// The same sign-in over HTTP, to a stand-in server on this machine's loopback: the forms go
/// out form-encoded with no secret, the token goes in the `Authorization` header of the one
/// check, and a refresh is sent to the same server.
#[test]
fn the_device_flow_and_a_refresh_go_over_http_to_a_stand_in_server() {
    purlis_core::unsteered!();
    use wiremock::matchers::{body_string_contains, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let server = rt.block_on(MockServer::start());
    let page = format!("{}/login/device", server.uri());
    let json = |(status, body): (u16, String)| {
        ResponseTemplate::new(status).set_body_raw(body, "application/json")
    };
    rt.block_on(async {
        Mock::given(method("POST"))
            .and(path("/login/device/code"))
            .and(header("accept", "application/json"))
            .and(body_string_contains("client%5Fid=Ov23litEn94I5HXrpdsX"))
            .and(body_string_contains("scope=repo%20read%3Aorg"))
            .respond_with(json(codes(&page)))
            .expect(1)
            .mount(&server)
            .await;
        // The first poll is too early; the second is granted.
        Mock::given(method("POST"))
            .and(path("/login/oauth/access_token"))
            .and(body_string_contains("device%5Fcode"))
            .respond_with(json(pending()))
            .up_to_n_times(1)
            .with_priority(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/login/oauth/access_token"))
            .and(body_string_contains("device%5Fcode"))
            .respond_with(json(grant(ACCESS, REFRESH)))
            .with_priority(2)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/login/oauth/access_token"))
            .and(body_string_contains("grant%5Ftype=refresh%5Ftoken"))
            .respond_with(json(grant(ACCESS_TWO, REFRESH_TWO)))
            .expect(1)
            .with_priority(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/user"))
            .and(header("authorization", format!("Bearer {ACCESS}").as_str()))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("x-oauth-scopes", "repo, read:org")
                    .set_body_string(r#"{"login": "octocat"}"#),
            )
            .expect(1)
            .mount(&server)
            .await;
    });

    let machine = Machine::new();
    let registration = registration::on(Kind::GitHub, "github.com").unwrap();
    let at = Endpoints::at(Kind::GitHub, &server.uri());
    let https = Https::new();
    let started = device::start(&https, &at, registration).unwrap();
    assert_eq!(started.user_code, "WDJB-MJHT");
    assert_eq!(started.verification_uri, page);
    let reach = accounts::over_root(ApiRoot::at(&server.uri()));
    let account = device::finish(
        &HostScope::for_a_test(),
        machine.config(),
        machine.keyring.as_ref(),
        &reach,
        &https,
        &at,
        registration,
        &started,
        &AtOnce,
    )
    .unwrap();
    assert_eq!(account.login, "octocat");
    assert_eq!(account.method, Method::Device);
    assert_eq!(account.host, "github.com");
    assert_eq!(machine.token(&account).as_deref(), Some(ACCESS));

    let refresher = Refresher::through(
        &HostScope::for_a_test(),
        machine.config(),
        Arc::new(Https::new()),
        at,
    );
    let token = refresher
        .token_at(&account, machine.expires_at(&account))
        .unwrap();
    assert_eq!(token.expose_secret(), ACCESS_TWO);

    // Nothing the stand-in was sent holds a client secret.
    let received = rt.block_on(server.received_requests()).unwrap();
    assert!(received.len() >= 5, "{}", received.len());
    for request in &received {
        let body = String::from_utf8_lossy(&request.body);
        assert!(!body.contains("client%5Fsecret"), "{body}");
        assert!(!body.contains("client_secret"), "{body}");
    }
    rt.block_on(server.verify());
}

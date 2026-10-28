// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The forge account store and a token sign-in on any host** (#1814, ADR 0077 §4, §6–§8),
//! against recorded forges.
//!
//! Each case signs in through the account store with a token, the forge answering from a
//! recording under `tests/forge_contract/accounts/<forge>/`, and asserts what the person sees:
//! the account listed with its login and scopes, the token in its own keyring item and nowhere
//! else, a refusal that names the missing scope or says what is wrong with the host, and
//! sign-out removing the item. The native case at the end sends the same check over HTTPS to a
//! recorded server, so the token is shown to go in the `Authorization` header and the scopes to
//! come back from GitHub's header.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use purlis_core::forge::accounts::{self, ForgeAccount, Method, Pasted, State};
use purlis_core::forge::http::{ApiRoot, TokenSource};
use purlis_core::forge::recorded::Recorded;
use purlis_core::forge::route::HostScope;
use purlis_core::forge::transport::Transport;
use purlis_core::forge::{Forge, ForgeError, Kind};
use purlis_core::secrets::keyring::{Presence, Store};
use secrecy::ExposeSecret;

/// The token every case pastes. Not shaped like any forge's token: the commit hook scans for
/// those.
const TOKEN: &str = "pasted-for-the-recorded-forge";

fn recording(kind: Kind, case: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/forge_contract/accounts")
        .join(kind.word())
        .join(format!("{case}.json"))
}

/// A config home, its keyring, and the recorded forge every check is sent to.
struct Machine {
    home: tempfile::TempDir,
    keyring: Box<dyn Store>,
    /// Every forge a check was sent to, and the token it was given.
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

    /// Sign in to `kind` on `host`, the forge answering from `case`; every exchange recorded
    /// must have been asked for.
    fn sign_in(&self, kind: Kind, host: &str, case: &str) -> Result<ForgeAccount, String> {
        let text = std::fs::read_to_string(recording(kind, case)).unwrap();
        let recorded = Arc::new(Recorded::parse(&text).unwrap());
        let served = recorded.clone();
        let reach = |forge: &Forge,
                     tokens: Arc<dyn TokenSource>|
         -> Result<Arc<dyn Transport>, ForgeError> {
            let token = tokens.token().unwrap().expose_secret().to_string();
            self.asked.lock().unwrap().push((forge.clone(), token));
            Ok(served.clone() as Arc<dyn Transport>)
        };
        let got = accounts::sign_in_with_token(
            &HostScope::for_a_test(),
            self.config(),
            self.keyring.as_ref(),
            &reach,
            kind,
            host,
            Pasted::new(TOKEN.to_string()),
        );
        if got.is_ok() {
            assert!(recorded.unspent().is_empty(), "{:?}", recorded.unspent());
        }
        got
    }

    fn list(&self) -> Vec<ForgeAccount> {
        accounts::list(self.config(), self.keyring.as_ref()).unwrap()
    }

    fn item(&self, account: &ForgeAccount) -> Option<String> {
        self.keyring
            .get(&accounts::service(account), "token")
            .unwrap()
            .map(|s| s.into_inner())
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
}

fn scopes(all: &[&str]) -> Option<Vec<String>> {
    Some(all.iter().map(|s| s.to_string()).collect())
}

#[test]
fn a_token_on_github_com_is_checked_listed_and_kept_in_its_own_keyring_item() {
    purlis_core::unsteered!();
    let machine = Machine::new();
    let account = machine
        .sign_in(Kind::GitHub, "github.com", "token_check")
        .unwrap();
    assert_eq!(
        (account.kind, account.host.as_str(), account.login.as_str()),
        (Kind::GitHub, "github.com", "octocat")
    );
    assert_eq!(account.method, Method::Pat);
    assert_eq!(account.state, State::SignedIn);
    assert_eq!(account.scopes, scopes(&["repo", "read:org"]));
    assert_eq!(machine.list(), vec![account.clone()]);
    assert_eq!(
        accounts::service(&account),
        format!("purlis/@forge/github.com/{}", account.id)
    );
    assert_eq!(machine.item(&account).as_deref(), Some(TOKEN));
    let asked = machine.asked.lock().unwrap().clone();
    assert_eq!(
        asked,
        [(Forge::default_of(Kind::GitHub), TOKEN.to_string())],
        "the pasted token is what the forge was asked with"
    );
}

#[test]
fn a_token_on_gitlab_com_is_checked_by_its_own_record_and_listed() {
    purlis_core::unsteered!();
    let machine = Machine::new();
    let account = machine
        .sign_in(Kind::GitLab, "gitlab.com", "token_check")
        .unwrap();
    assert_eq!(account.login, "jdoe");
    assert_eq!(account.scopes, scopes(&["api", "write_repository"]));
    assert_eq!(machine.list(), vec![account]);
}

#[test]
fn a_self_managed_gitlab_and_a_github_enterprise_host_each_get_an_account_of_their_own() {
    purlis_core::unsteered!();
    let machine = Machine::new();
    let gitlab = machine
        .sign_in(Kind::GitLab, "https://Git.Example.com/", "token_check")
        .unwrap();
    let github = machine
        .sign_in(Kind::GitHub, "ghe.example.com:8443", "token_check")
        .unwrap();
    assert_eq!(gitlab.host, "git.example.com");
    assert_eq!(github.host, "ghe.example.com:8443");
    assert_ne!(gitlab.id, github.id);
    assert_eq!(machine.list(), vec![gitlab.clone(), github.clone()]);
    let hosts: Vec<String> = machine
        .asked
        .lock()
        .unwrap()
        .iter()
        .map(|(forge, _)| forge.host.clone())
        .collect();
    assert_eq!(hosts, ["git.example.com", "ghe.example.com:8443"]);
    assert!(accounts::service(&gitlab).starts_with("purlis/@forge/git.example.com/"));
}

#[test]
fn signing_out_deletes_the_keyring_item_and_the_account() {
    purlis_core::unsteered!();
    let machine = Machine::new();
    let kept = machine
        .sign_in(Kind::GitLab, "gitlab.com", "token_check")
        .unwrap();
    let gone = machine
        .sign_in(Kind::GitHub, "github.com", "token_check")
        .unwrap();
    let out = accounts::sign_out(
        &HostScope::for_a_test(),
        machine.config(),
        machine.keyring.as_ref(),
        &gone.id,
    )
    .unwrap();
    assert_eq!(out, gone);
    assert_eq!(
        machine.item(&gone),
        None,
        "the token is still in the keyring"
    );
    assert_eq!(
        machine.keyring.exists(&accounts::service(&gone), "token"),
        Presence::Absent
    );
    assert_eq!(machine.list(), vec![kept.clone()]);
    assert_eq!(machine.item(&kept).as_deref(), Some(TOKEN));
    let again = accounts::sign_out(
        &HostScope::for_a_test(),
        machine.config(),
        machine.keyring.as_ref(),
        &gone.id,
    );
    assert!(again.is_err(), "{again:?}");
}

#[test]
fn signing_in_again_as_the_same_login_replaces_the_token_and_keeps_one_account() {
    purlis_core::unsteered!();
    let machine = Machine::new();
    let first = machine
        .sign_in(Kind::GitHub, "github.com", "token_check")
        .unwrap();
    let second = machine
        .sign_in(Kind::GitHub, "github.com", "token_check.wider")
        .unwrap();
    assert_eq!(first.id, second.id);
    assert_eq!(machine.list(), vec![second.clone()]);
    assert_eq!(
        accounts::wider(Kind::GitHub, second.scopes.as_deref().unwrap()),
        ["admin:org", "gist", "workflow"],
        "a scope beyond what purlis needs is marked as wider"
    );
}

#[test]
fn a_token_missing_a_scope_is_refused_naming_it_and_nothing_is_kept() {
    purlis_core::unsteered!();
    for (kind, host, scope) in [
        (Kind::GitHub, "github.com", "read:org"),
        (Kind::GitLab, "gitlab.com", "api"),
    ] {
        let machine = Machine::new();
        let why = machine
            .sign_in(kind, host, "token_check.missing_scope")
            .unwrap_err();
        assert!(why.contains(&format!("missing the {scope} scope")), "{why}");
        assert!(
            why.contains(&format!("https://{host}/")),
            "names where: {why}"
        );
        assert!(machine.list().is_empty());
        assert!(machine.files_but_the_keyring().is_empty());
    }
}

#[test]
fn a_github_fine_grained_token_is_taken_with_its_scopes_unknown() {
    purlis_core::unsteered!();
    let machine = Machine::new();
    let account = machine
        .sign_in(Kind::GitHub, "github.com", "token_check.fine_grained")
        .unwrap();
    assert_eq!(account.scopes, None);
}

#[test]
fn a_refused_token_or_a_host_that_is_not_the_forge_says_what_is_wrong() {
    purlis_core::unsteered!();
    for (kind, host, case, says) in [
        (
            Kind::GitHub,
            "github.com",
            "token_check.refused",
            "mistyped, expired or revoked",
        ),
        (
            Kind::GitLab,
            "git.example.com",
            "token_check.refused",
            "mistyped, expired or revoked",
        ),
        (
            Kind::GitLab,
            "gitlab.com",
            "token_check.revoked",
            "expired or was revoked",
        ),
        (
            Kind::GitHub,
            "ghe.example.com",
            "token_check.not_github",
            "did not answer as a GitHub Enterprise server",
        ),
    ] {
        let machine = Machine::new();
        let why = machine.sign_in(kind, host, case).unwrap_err();
        assert!(why.contains(says), "{case}: {why}");
        assert!(why.contains(host), "{case}: {why}");
        assert!(machine.list().is_empty());
    }
}

#[test]
fn a_bad_host_is_refused_before_the_forge_is_asked() {
    purlis_core::unsteered!();
    let in_host = format!("{TOKEN}.example.com");
    for (kind, typed, says) in [
        (Kind::GitHub, "", "Enter the GitHub host"),
        (Kind::GitLab, "http://git.example.com", "HTTPS only"),
        (Kind::GitLab, "git.example.com/group", "not a host name"),
        (Kind::GitLab, TOKEN, "not a host name"),
        (Kind::GitLab, in_host.as_str(), "holds the token"),
        (Kind::GitLab, "github.com", "Choose GitHub"),
    ] {
        let machine = Machine::new();
        let why = machine.sign_in(kind, typed, "token_check").unwrap_err();
        assert!(why.contains(says), "{typed:?}: {why}");
        assert!(
            !why.contains(TOKEN),
            "a token pasted as the host is repeated: {why}"
        );
        assert!(
            machine.asked.lock().unwrap().is_empty(),
            "{typed:?} was asked"
        );
    }
}

#[test]
fn no_refusal_ever_repeats_the_token() {
    purlis_core::unsteered!();
    // A forge that quoted the token back in its refusal.
    let echoing = format!(
        r#"{{"source": "a forge that quotes a refused token back",
            "exchanges": [{{"call": {{"endpoint": {{"rest": {{"method": null, "path": "user"}}}}, "fields": []}},
                            "reply": {{"code": 1, "out": "", "err": "token {TOKEN} is unknown (HTTP 400)", "status": 400}}}}]}}"#
    );
    let dir = tempfile::tempdir().unwrap();
    let machine = Machine::new();
    let recorded = Arc::new(Recorded::parse(&echoing).unwrap());
    let reach = |_: &Forge, _: Arc<dyn TokenSource>| -> Result<Arc<dyn Transport>, ForgeError> {
        Ok(recorded.clone() as Arc<dyn Transport>)
    };
    let why = accounts::sign_in_with_token(
        &HostScope::for_a_test(),
        dir.path(),
        machine.keyring.as_ref(),
        &reach,
        Kind::GitHub,
        "github.com",
        Pasted::new(TOKEN.to_string()),
    )
    .unwrap_err();
    assert!(!why.contains(TOKEN), "{why}");
    assert!(
        why.contains("[the token]") || why.contains("[a token purlis holds]"),
        "{why}"
    );

    // A transport that could not be built, in words that hold the token.
    let failing = |_: &Forge, _: Arc<dyn TokenSource>| -> Result<Arc<dyn Transport>, ForgeError> {
        Err(ForgeError::transport(format!("no route for {TOKEN}")))
    };
    let why = accounts::sign_in_with_token(
        &HostScope::for_a_test(),
        dir.path(),
        machine.keyring.as_ref(),
        &failing,
        Kind::GitLab,
        "gitlab.com",
        Pasted::new(TOKEN.to_string()),
    )
    .unwrap_err();
    assert!(!why.contains(TOKEN), "{why}");

    for (kind, case) in [
        (Kind::GitHub, "token_check.refused"),
        (Kind::GitHub, "token_check.missing_scope"),
        (Kind::GitLab, "token_check.revoked"),
        (Kind::GitLab, "token_check.missing_scope"),
    ] {
        let why = Machine::new()
            .sign_in(kind, kind.default_host(), case)
            .unwrap_err();
        assert!(!why.contains(TOKEN), "{case}: {why}");
    }
}

#[test]
fn the_accounts_file_names_each_account_and_holds_no_token() {
    purlis_core::unsteered!();
    let machine = Machine::new();
    let account = machine
        .sign_in(Kind::GitLab, "git.example.com", "token_check")
        .unwrap();
    let file = accounts::path(machine.config());
    let text = std::fs::read_to_string(&file).unwrap();
    let doc: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(doc["version"], 1);
    assert_eq!(
        doc["accounts"][0],
        serde_json::json!({
            "id": account.id,
            "kind": "gitlab",
            "host": "git.example.com",
            "login": "jdoe",
            "method": "pat",
            "state": "signed-in",
            "scopes": ["api", "write_repository"],
        })
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&file).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
    for (path, text) in machine.files_but_the_keyring() {
        assert!(!text.contains(TOKEN), "{} holds the token", path.display());
    }
}

#[test]
fn what_a_later_purlis_wrote_survives_a_sign_in_and_a_sign_out() {
    purlis_core::unsteered!();
    let machine = Machine::new();
    let file = accounts::path(machine.config());
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(
        &file,
        r#"{"version": 1, "bindings": [{"owner": "acme"}],
            "accounts": [{"id": "01J9LATER", "kind": "bitbucket", "host": "b.example.com",
                          "login": "x", "method": "pat", "state": "signed-in"}]}"#,
    )
    .unwrap();
    let account = machine
        .sign_in(Kind::GitHub, "github.com", "token_check")
        .unwrap();
    accounts::sign_out(
        &HostScope::for_a_test(),
        machine.config(),
        machine.keyring.as_ref(),
        &account.id,
    )
    .unwrap();
    let doc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    assert_eq!(doc["bindings"][0]["owner"], "acme");
    assert_eq!(doc["accounts"][0]["id"], "01J9LATER");
    assert!(
        machine.list().is_empty(),
        "an account of an unknown forge is not listed"
    );
}

#[test]
fn an_accounts_file_that_cannot_be_read_is_an_error_and_is_never_written_over() {
    purlis_core::unsteered!();
    let machine = Machine::new();
    let file = accounts::path(machine.config());
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, "not json").unwrap();
    assert!(accounts::list(machine.config(), machine.keyring.as_ref()).is_err());
    assert!(
        machine
            .sign_in(Kind::GitHub, "github.com", "token_check")
            .is_err()
    );
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "not json");
}

/// The check over the native transport, to a recorded server: the pasted token goes in the
/// `Authorization` header of the one request, and the scopes come back from GitHub's header.
#[test]
fn the_check_sends_the_token_over_the_native_transport_and_reads_githubs_scopes_header() {
    purlis_core::unsteered!();
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let server = rt.block_on(MockServer::start());
    rt.block_on(
        Mock::given(method("GET"))
            .and(path("/user"))
            .and(header("authorization", format!("Bearer {TOKEN}").as_str()))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("x-oauth-scopes", "repo, read:org")
                    .set_body_string(r#"{"login": "octocat"}"#),
            )
            .expect(1)
            .mount(&server),
    );
    let machine = Machine::new();
    let reach = accounts::over_root(ApiRoot::at(&server.uri()));
    let account = accounts::sign_in_with_token(
        &HostScope::for_a_test(),
        machine.config(),
        machine.keyring.as_ref(),
        &reach,
        Kind::GitHub,
        "github.com",
        Pasted::new(TOKEN.to_string()),
    )
    .unwrap();
    assert_eq!(account.login, "octocat");
    assert_eq!(account.scopes, scopes(&["repo", "read:org"]));
    rt.block_on(server.verify());
}

/// A token with no prefix any shape rule knows (a GitLab or self-managed one) is kept out of the
/// log and every error by its value, from the moment it is signed in with.
#[test]
fn a_signed_in_token_with_no_known_shape_is_kept_out_of_the_log_by_its_value() {
    purlis_core::unsteered!();
    let event = format!("purlis: the forge answered with {TOKEN}");
    let machine = Machine::new();
    machine
        .sign_in(Kind::GitLab, "git.example.com", "token_check")
        .unwrap();
    assert!(purlis_core::applog::holds_a_value_kept_out(&event));
    assert!(!purlis_core::applog::without_values_kept_out(&event).contains(TOKEN));
}

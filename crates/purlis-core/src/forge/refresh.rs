// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A browser sign-in's token, renewed before it expires** (ADR 0077 §3).
//!
//! A token a browser sign-in was granted expires: after eight hours on github.com, after two on
//! gitlab.com. Its keyring item also holds a refresh token ([`super::accounts`]), and a
//! [`Refresher`] trades that for a new pair when a request is about to be sent with a token
//! that has less than [`AHEAD`] left. No client secret is sent: a public client refreshes with
//! its client id alone.
//!
//! **One writer.** A refresh rotates the refresh token, so two refreshes of one account at once
//! would end its sign-in. Only the host's process refreshes (a [`Refresher`] is made with the
//! [`HostScope`]), one refresh at a time in it, and the new pair replaces the old in one write
//! to the account's keyring item. The git credential helper, which is another process, never
//! refreshes: the window renews the token before it starts the git that will ask for it.
//!
//! **A refresh the forge refuses ends the sign-in, and says so.** The account is marked signed
//! out, its keyring item is deleted, and the request that needed the token fails with a
//! sentence that names the account and where to sign in again. It is never retried under
//! another account or through the forge's CLI (ADR 0070 §5).
//!
//! **A forge that cannot be reached ends nothing**, and neither does an answer that is not the
//! forge's refusal of the grant (a rate limit, a proxy's refusal, OAuth's own "not now"). While the token still has time left it is
//! used as it is; once it has none the request fails saying the forge could not be reached, and
//! the sign-in is kept for the next try.
//!
//! **Only purlis's own grants are refreshed.** A pasted token does not expire through purlis,
//! and an imported CLI login is never refreshed with the CLI's registration (ADR 0077 §5).

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use secrecy::{ExposeSecret, SecretString};

use super::accounts::{self, ForgeAccount, Kept, Method};
use super::oauth::{Endpoint, Endpoints, Https};
use super::registration::{self, Registration};
use super::route::HostScope;
use super::signin::SIGN_IN_AT;
use super::{Failure, ForgeError};

/// How long before its expiry a token is renewed (ADR 0077 §3).
pub const AHEAD: Duration = Duration::from_secs(5 * 60);

/// One refresh at a time in this process, whatever the account.
pub(super) static REFRESHING: Mutex<()> = Mutex::new(());

/// What the host is told with when a forge ended a sign-in.
type Told = Box<dyn Fn(&ForgeAccount) + Send + Sync>;

/// Whom to tell when a forge ended a sign-in: the window, so it can say so at once.
static TOLD: OnceLock<Told> = OnceLock::new();

/// From now on `told` is called, once per sign-in a forge ended, with the account as it was:
/// the app tells its window, which draws the Notice with Sign in again. Set once, by the host,
/// as it starts; a later call changes nothing.
pub fn tell_when_ended(_host: &HostScope, told: impl Fn(&ForgeAccount) + Send + Sync + 'static) {
    let _ = TOLD.set(Box::new(told));
}

/// Hands out an account's access token, renewed first when it is about to expire.
pub struct Refresher {
    config_root: PathBuf,
    /// Made when the first refresh is due: most runs of the window never refresh.
    endpoint: OnceLock<Arc<dyn Endpoint>>,
    /// A stand-in's endpoints, in a test build; `None` for the registration's own.
    at: Option<Endpoints>,
    scope: HostScope,
}

impl Refresher {
    /// A refresher for the accounts under `config_root`, in the host's process: only the host
    /// can claim the scope this takes.
    pub fn new(host: &HostScope, config_root: &Path) -> Refresher {
        Refresher {
            config_root: config_root.to_path_buf(),
            endpoint: OnceLock::new(),
            at: None,
            scope: host.again(),
        }
    }

    /// [`Refresher::new`], refreshing through `endpoint` at `at`: a stand-in forge. Only a test
    /// build has it.
    #[cfg(any(test, feature = "fenced"))]
    pub fn through(
        host: &HostScope,
        config_root: &Path,
        endpoint: Arc<dyn Endpoint>,
        at: Endpoints,
    ) -> Refresher {
        let refresher = Refresher::new(host, config_root);
        let _ = refresher.endpoint.set(endpoint);
        Refresher {
            at: Some(at),
            ..refresher
        }
    }

    /// `account`'s access token for one request, renewed first when it has less than [`AHEAD`]
    /// left.
    pub fn token(&self, account: &ForgeAccount) -> Result<SecretString, ForgeError> {
        self.token_at(account, accounts::now())
    }

    /// [`Refresher::token`], with the time given.
    pub fn token_at(&self, account: &ForgeAccount, now: u64) -> Result<SecretString, ForgeError> {
        let keyring = accounts::keyring_at(&self.config_root);
        let kept = self.read(keyring.as_ref(), account)?;
        if !due(account, &kept, now) {
            return Ok(kept.access);
        }
        let _one = REFRESHING.lock().unwrap_or_else(|e| e.into_inner());
        // Another request may have renewed it while this one waited its turn.
        let kept = self.read(keyring.as_ref(), account)?;
        if !due(account, &kept, now) {
            return Ok(kept.access);
        }
        let expired = kept.expires_at.is_some_and(|at| at <= now);
        let (Some(registration), Some(spent)) = (
            registration::on(account.kind, &account.host),
            kept.refresh.as_ref(),
        ) else {
            // Nothing to renew it with. It is used while it lasts.
            if expired {
                return Err(self.ended(keyring.as_ref(), account));
            }
            return Ok(kept.access);
        };
        let at = self
            .at
            .clone()
            .unwrap_or_else(|| Endpoints::of(registration));
        let endpoint = self.endpoint.get_or_init(Https::shared);
        match renewed(endpoint.as_ref(), &at, registration, spent) {
            Renewed::Granted(granted) => {
                accounts::keep_refreshed(
                    &self.scope,
                    keyring.as_ref(),
                    account,
                    &granted,
                    Some(spent),
                    now,
                )
                .map_err(ForgeError::transport)?;
                Ok(granted.access)
            }
            Renewed::Refused => Err(self.ended(keyring.as_ref(), account)),
            Renewed::Unreachable(_) if !expired => Ok(kept.access),
            Renewed::Unreachable(why) => Err(ForgeError::transport(format!(
                "purlis could not reach {} to renew its sign-in as {}: {why}. Check your \
                 connection and try again.",
                account.host, account.login
            ))),
        }
    }

    fn read(
        &self,
        keyring: &dyn crate::secrets::keyring::Store,
        account: &ForgeAccount,
    ) -> Result<Kept, ForgeError> {
        match accounts::kept(keyring, account) {
            Ok(Some(kept)) => Ok(kept),
            Ok(None) => Err(ForgeError::of(
                Failure::Auth,
                format!(
                    "purlis's sign-in to {} as {} has no token in the keychain any more. Sign in \
                     again in {SIGN_IN_AT}.",
                    account.host, account.login
                ),
            )),
            Err(why) => Err(ForgeError::transport(why)),
        }
    }

    /// The forge ended `account`'s sign-in: it is marked signed out, and this is the refusal the
    /// request that needed it gets.
    fn ended(
        &self,
        keyring: &dyn crate::secrets::keyring::Store,
        account: &ForgeAccount,
    ) -> ForgeError {
        if let Err(why) =
            accounts::signed_out_by_the_forge(&self.scope, &self.config_root, keyring, &account.id)
        {
            tracing::warn!("purlis: an ended forge sign-in could not be marked signed out: {why}");
        }
        if let Some(told) = TOLD.get() {
            told(account);
        }
        ForgeError::of(Failure::Auth, ended_said(account))
    }
}

/// What the window says when the forge ended `account`'s sign-in.
pub fn ended_said(account: &ForgeAccount) -> String {
    format!(
        "purlis's sign-in to {} as {} has ended, and {} would not renew it. Sign in again in \
         {SIGN_IN_AT}.",
        account.host, account.login, account.host
    )
}

/// Whether `account`'s token is to be renewed before it is used at `now`.
fn due(account: &ForgeAccount, kept: &Kept, now: u64) -> bool {
    matches!(account.method, Method::Device | Method::Pkce)
        && kept
            .expires_at
            .is_some_and(|at| at <= now.saturating_add(AHEAD.as_secs()))
}

/// OAuth's `error` words that say "not now" and nothing about the grant (RFC 6749 §4.1.2.1,
/// RFC 8628 §3.5).
const NOT_NOW: [&str; 3] = ["temporarily_unavailable", "server_error", "slow_down"];

/// How a refresh went.
enum Renewed {
    Granted(super::oauth::Granted),
    /// The forge answered and would not: the sign-in has ended.
    Refused,
    /// Nothing answered, or not as a forge would: nothing is known about the sign-in.
    Unreachable(String),
}

/// `spent` traded for a new pair at `at`, with the registration's public client id and no
/// secret.
fn renewed(
    endpoint: &dyn Endpoint,
    at: &Endpoints,
    registration: &Registration,
    spent: &SecretString,
) -> Renewed {
    let answer = match endpoint.post(
        &at.token,
        &[
            ("client_id", registration.client_id),
            ("grant_type", "refresh_token"),
            ("refresh_token", spent.expose_secret()),
        ],
    ) {
        Ok(answer) => answer,
        Err(why) => return Renewed::Unreachable(why),
    };
    let status = answer.status;
    match answer.error() {
        // The forge's own word that it is busy: nothing is known about the sign-in.
        Some(word) if NOT_NOW.contains(&word) => {
            return Renewed::Unreachable(format!("it answered HTTP {status}"));
        }
        // `bad_refresh_token`, `invalid_grant`, an app the person revoked: the forge's answer,
        // whatever its word.
        Some(_) => return Renewed::Refused,
        // A bare 400 or 401 is how a token endpoint refuses a grant.
        None if matches!(status, 400 | 401) => return Renewed::Refused,
        // Any other refusal without OAuth's word (a rate limit, a proxy between here and the
        // forge) says nothing about the sign-in, and ending it cannot be undone.
        None if (400..500).contains(&status) => {
            return Renewed::Unreachable(format!("it answered HTTP {status}"));
        }
        None => {}
    }
    match answer.granted() {
        Some(granted) => Renewed::Granted(granted),
        None => Renewed::Unreachable(format!("it answered HTTP {status}")),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::forge::Forge;
    use crate::forge::Kind;
    use crate::forge::accounts::State;
    use crate::forge::device::tests::StandIn;
    use crate::forge::http::TokenSource;
    use crate::forge::oauth::Answer;
    use crate::forge::transport::{Call, NoAnswer, Reply, Transport};

    /// A forge that says whose token it is: `octocat`'s.
    pub(crate) struct WhoAmI;

    impl Transport for WhoAmI {
        fn send(&self, _forge: &Forge, _call: &Call) -> Result<Reply, NoAnswer> {
            Ok(Reply {
                code: 0,
                out: r#"{"login": "octocat", "username": "octocat"}"#.to_string(),
                err: String::new(),
                status: Some(200),
                headers: Vec::new(),
            })
        }

        fn check_auth(&self, _forge: &Forge) -> Result<(), ForgeError> {
            Ok(())
        }
    }

    const LASTS: u64 = 8 * 60 * 60;

    /// A config home with one account signed in by a browser sign-in, and when its token
    /// expires, read back from the item.
    pub(crate) struct Machine {
        pub(crate) home: tempfile::TempDir,
        pub(crate) account: ForgeAccount,
        pub(crate) expires_at: u64,
    }

    pub(crate) fn machine(kind: Kind, host: &str, method: Method) -> Machine {
        let home = tempfile::tempdir().unwrap();
        let keyring = accounts::keyring_at(home.path());
        let granted = Answer::of(
            200,
            &format!(
                r#"{{"access_token": "first-access-value", "scope": "{}", "expires_in": {LASTS},
                     "refresh_token": "first-refresh-value"}}"#,
                accounts::needed(kind).join(",")
            ),
        )
        .granted()
        .unwrap();
        let reach = |_: &Forge,
                     _: Arc<dyn TokenSource>|
         -> Result<Arc<dyn Transport>, ForgeError> { Ok(Arc::new(WhoAmI)) };
        let account = accounts::sign_in_with_grant(
            &HostScope::for_a_test(),
            home.path(),
            keyring.as_ref(),
            &reach,
            kind,
            host,
            granted,
            method,
        )
        .unwrap();
        let expires_at = accounts::kept(keyring.as_ref(), &account)
            .unwrap()
            .unwrap()
            .expires_at
            .unwrap();
        Machine {
            home,
            account,
            expires_at,
        }
    }

    impl Machine {
        fn refresher(&self, forge: &Arc<StandIn>) -> Refresher {
            let registration = registration::on(self.account.kind, &self.account.host).unwrap();
            Refresher::through(
                &HostScope::for_a_test(),
                self.home.path(),
                forge.clone(),
                Endpoints::of(registration),
            )
        }

        pub(crate) fn kept(&self) -> Option<Kept> {
            accounts::kept(
                accounts::keyring_at(self.home.path()).as_ref(),
                &self.account,
            )
            .unwrap()
        }

        fn listed(&self) -> ForgeAccount {
            accounts::list(
                self.home.path(),
                accounts::keyring_at(self.home.path()).as_ref(),
            )
            .unwrap()
            .remove(0)
        }
    }

    const RENEWED: &str = r#"{"access_token": "second-access-value", "scope": "repo,read:org",
        "expires_in": 28800, "refresh_token": "second-refresh-value"}"#;

    #[test]
    fn a_token_with_time_left_is_handed_out_and_nothing_is_asked() {
        let machine = machine(Kind::GitHub, "github.com", Method::Device);
        let forge = Arc::new(StandIn::answering(&[]));
        let token = machine
            .refresher(&forge)
            .token_at(&machine.account, machine.expires_at - AHEAD.as_secs() - 1)
            .unwrap();
        assert_eq!(token.expose_secret(), "first-access-value");
        assert!(forge.posted.lock().unwrap().is_empty());
    }

    #[test]
    fn a_token_about_to_expire_is_renewed_first_with_the_client_id_and_no_secret() {
        let machine = machine(Kind::GitHub, "github.com", Method::Device);
        let forge = Arc::new(StandIn::answering(&[Ok((200, RENEWED))]));
        let refresher = machine.refresher(&forge);
        let now = machine.expires_at - AHEAD.as_secs();
        let token = refresher.token_at(&machine.account, now).unwrap();
        assert_eq!(token.expose_secret(), "second-access-value");
        assert_eq!(
            *forge.posted.lock().unwrap(),
            [(
                "https://github.com/login/oauth/access_token".to_string(),
                vec![
                    ("client_id".to_string(), "Ov23litEn94I5HXrpdsX".to_string()),
                    ("grant_type".to_string(), "refresh_token".to_string()),
                    (
                        "refresh_token".to_string(),
                        "first-refresh-value".to_string()
                    ),
                ]
            )]
        );
        // The new pair replaced the old, in the one item.
        let kept = machine.kept().unwrap();
        assert_eq!(kept.access.expose_secret(), "second-access-value");
        assert_eq!(
            kept.refresh.as_ref().map(|r| r.expose_secret()),
            Some("second-refresh-value")
        );
        assert_eq!(kept.expires_at, Some(now + 28800));
        assert_eq!(machine.listed().state, State::SignedIn);
        // And the next request uses it, asking nothing.
        let again = refresher.token_at(&machine.account, now + 60).unwrap();
        assert_eq!(again.expose_secret(), "second-access-value");
        assert_eq!(forge.posted.lock().unwrap().len(), 1);
        // Both new values are kept out of the log from the moment they are read.
        assert!(crate::applog::holds_a_value_kept_out(
            "second-refresh-value"
        ));
    }

    #[test]
    fn gitlab_com_is_renewed_at_its_own_token_endpoint() {
        let machine = machine(Kind::GitLab, "gitlab.com", Method::Pkce);
        let forge = Arc::new(StandIn::answering(&[Ok((
            200,
            r#"{"access_token": "gitlab-second-access", "scope": "api", "expires_in": 7200,
                "refresh_token": "gitlab-second-refresh"}"#,
        ))]));
        let token = machine
            .refresher(&forge)
            .token_at(&machine.account, machine.expires_at)
            .unwrap();
        assert_eq!(token.expose_secret(), "gitlab-second-access");
        let posted = forge.posted.lock().unwrap().clone();
        assert_eq!(posted[0].0, "https://gitlab.com/oauth/token");
        assert!(
            posted[0].1.iter().all(|(name, _)| name != "client_secret"),
            "{posted:?}"
        );
    }

    #[test]
    fn a_refresh_the_forge_refuses_signs_the_account_out_and_says_to_sign_in_again() {
        for answer in [
            (200, r#"{"error": "bad_refresh_token"}"#),
            (400, r#"{"error": "invalid_grant"}"#),
            (401, ""),
        ] {
            let machine = machine(Kind::GitHub, "github.com", Method::Device);
            let forge = Arc::new(StandIn::answering(&[Ok(answer)]));
            let refresher = machine.refresher(&forge);
            let why = refresher
                .token_at(&machine.account, machine.expires_at - 10)
                .unwrap_err();
            assert_eq!(why.failure(), &Failure::Auth);
            assert!(
                why.said()
                    .contains("purlis's sign-in to github.com as octocat has ended"),
                "{why}"
            );
            assert!(why.said().contains("Sign in again in Settings"), "{why}");
            assert!(!why.said().contains("first-"), "{why}");
            // Still listed, signed out, with nothing left in the keyring.
            let listed = machine.listed();
            assert_eq!(listed.id, machine.account.id);
            assert_eq!(listed.state, State::SignedOut);
            assert!(machine.kept().is_none());
            assert_eq!(
                accounts::signed_in_on(machine.home.path(), Kind::GitHub, "github.com").unwrap(),
                None
            );
            // And it is not tried again: the next request is refused without asking the forge.
            assert!(
                refresher
                    .token_at(&machine.account, machine.expires_at)
                    .is_err()
            );
            assert_eq!(forge.posted.lock().unwrap().len(), 1);
        }
    }

    #[test]
    fn a_forge_that_cannot_be_reached_ends_nothing() {
        let machine = machine(Kind::GitHub, "github.com", Method::Device);
        let forge = Arc::new(StandIn::answering(&[
            Err("timed out"),
            Ok((503, "<html>unavailable</html>")),
            Err("timed out"),
        ]));
        let refresher = machine.refresher(&forge);
        // With time left, the token it has is used.
        for _ in 0..2 {
            let token = refresher
                .token_at(&machine.account, machine.expires_at - 60)
                .unwrap();
            assert_eq!(token.expose_secret(), "first-access-value");
        }
        // With none left, the request fails, and the sign-in is kept for the next try.
        let why = refresher
            .token_at(&machine.account, machine.expires_at + 1)
            .unwrap_err();
        assert_eq!(why.failure(), &Failure::Transport);
        assert!(why.said().contains("could not reach github.com"), "{why}");
        assert_eq!(machine.listed().state, State::SignedIn);
        assert_eq!(
            machine.kept().unwrap().refresh.unwrap().expose_secret(),
            "first-refresh-value"
        );
    }

    #[test]
    fn a_browser_sign_ins_item_is_never_handed_out_whole_as_a_token() {
        // The keyring was written and the list was not: the account still reads as a token's.
        let machine = machine(Kind::GitHub, "github.com", Method::Device);
        let keyring = accounts::keyring_at(machine.home.path());
        let listed_as_a_tokens = ForgeAccount {
            method: Method::Pat,
            ..machine.account.clone()
        };
        let why = accounts::token(keyring.as_ref(), &listed_as_a_tokens).unwrap_err();
        assert!(why.contains("cannot be read from the keychain"), "{why}");
        assert!(!why.contains("first-"), "{why}");
    }

    #[test]
    fn an_answer_that_is_no_refusal_of_the_grant_ends_nothing() {
        // A rate limit, a proxy in between, a forge that says it is busy: none of them is the
        // forge ending the sign-in, and a sign-in that was ended cannot be put back.
        for answer in [
            (429, "<html>slow down</html>"),
            (403, "<html>blocked by the gateway</html>"),
            (407, ""),
            (503, r#"{"error": "temporarily_unavailable"}"#),
            (400, r#"{"error": "server_error"}"#),
        ] {
            let machine = machine(Kind::GitHub, "github.com", Method::Device);
            let forge = Arc::new(StandIn::answering(&[Ok(answer), Ok(answer)]));
            let refresher = machine.refresher(&forge);
            let token = refresher
                .token_at(&machine.account, machine.expires_at - 60)
                .unwrap();
            assert_eq!(token.expose_secret(), "first-access-value");
            let why = refresher
                .token_at(&machine.account, machine.expires_at + 1)
                .unwrap_err();
            assert_eq!(why.failure(), &Failure::Transport, "{answer:?}");
            assert_eq!(machine.listed().state, State::SignedIn, "{answer:?}");
            assert!(machine.kept().unwrap().refresh.is_some(), "{answer:?}");
        }
    }

    #[test]
    fn a_sign_in_on_a_host_purlis_has_no_registration_on_is_never_sent_to_another_hosts_endpoint() {
        // An account a later purlis signed in on a host of its own: this one has no
        // registration there, so it never posts that account's refresh token anywhere.
        let machine = machine(Kind::GitHub, "ghe.example.com", Method::Device);
        let forge = Arc::new(StandIn::answering(&[]));
        let refresher = Refresher::through(
            &HostScope::for_a_test(),
            machine.home.path(),
            forge.clone(),
            Endpoints::of(registration::on(Kind::GitHub, "github.com").unwrap()),
        );
        let token = refresher
            .token_at(&machine.account, machine.expires_at - 60)
            .unwrap();
        assert_eq!(token.expose_secret(), "first-access-value");
        assert!(
            refresher
                .token_at(&machine.account, machine.expires_at + 1)
                .is_err()
        );
        assert!(forge.posted.lock().unwrap().is_empty());
    }
}

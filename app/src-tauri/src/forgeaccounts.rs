// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Settings › You › Accounts** and the token sign-in dialog (#1814, ADR 0077 §4, §6, §7): this
//! machine's forge accounts, listed, signed in to with a token on any host, and signed out of.
//!
//! The window is the host's human scope (`local-ui`, ADR 0077 §3): each act here claims
//! [`HostScope`], which a process a chat started is refused. All of it is the core's
//! ([`purlis_core::forge::accounts`]); this file only carries it to the window.
//!
//! **The token crosses once, inward.** [`forge_sign_in_with_token`] takes it from the window's
//! password box, held as a [`ForgeToken`] that is wiped as it is handed to the core, which keeps
//! it as a `SecretString` and then in its keyring item. No answer here carries it, and no error:
//! the core's sentences never hold it. The command is in `ipc_commands.rs`'s `vault_values`
//! class, which holds the commands that take a credential from the window (ADR 0052 amended
//! 2026-10-09).
//!
//! **An existing `gh` or `glab` login is offered, never taken** (#1376, ADR 0077 §5):
//! [`forge_cli_logins`] lists the CLI logins purlis could import, asking each CLI only whether it
//! is signed in, and [`forge_import_cli_login`] is the person's yes, the one call that asks a CLI
//! for its token. Both are the window's alone (`WINDOW_ONLY`).
//!
//! **Signing in to github.com through the browser** (#729, ADR 0077 §3 as amended 2026-10-12) is
//! the OAuth device flow, in three calls: [`forge_device_sign_in_start`] asks the forge for a
//! code and answers what the window shows, [`forge_device_sign_in_wait`] polls until the person
//! confirmed it and signs the account in, and [`forge_device_sign_in_cancel`] stops the
//! polling. The flow is held in this process's memory for its length and nowhere else; the
//! device code never crosses to the window, and the token it is traded for never does either.
//! All three are the window's alone (`WINDOW_ONLY`).
//!
//! **The git the window runs signs in with these accounts** (#752, ADR 0070 amended 2026-10-12):
//! [`window_git`] runs a save, a fetch, a push or a clone with purlis's own credential helper for
//! a host the person has an account on.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use purlis_core::forge::Kind;
use purlis_core::forge::accounts::{self, ForgeAccount, Method, Pasted, State};
use purlis_core::forge::cliimport::{self, CliLogin};
use purlis_core::forge::registration::{self, Registration};
use purlis_core::forge::route::HostScope;
use purlis_core::forge::{device, oauth};

/// One forge account, as Settings lists it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct ForgeAccountRow {
    /// What sign-out names it by.
    pub id: String,
    /// `github` or `gitlab`.
    pub kind: String,
    pub host: String,
    pub login: String,
    /// How it was signed in: `token`, `browser` or `imported`.
    pub method: String,
    pub signed_in: bool,
    /// The token's scopes when it was checked, or null when the forge does not say.
    pub scopes: Option<Vec<String>>,
    /// The scopes beyond what purlis needs (ADR 0077 §2).
    pub wider: Vec<String>,
}

impl From<ForgeAccount> for ForgeAccountRow {
    fn from(account: ForgeAccount) -> Self {
        let wider = account
            .scopes
            .as_deref()
            .map(|scopes| accounts::wider(account.kind, scopes))
            .unwrap_or_default();
        ForgeAccountRow {
            kind: account.kind.word().to_string(),
            method: match account.method {
                Method::Pat => "token",
                Method::Device | Method::Pkce => "browser",
                Method::Import => "imported",
            }
            .to_string(),
            signed_in: account.state == State::SignedIn,
            id: account.id,
            host: account.host,
            login: account.login,
            scopes: account.scopes,
            wider,
        }
    }
}

/// What the sign-in dialog shows for a host: the scopes to tick and where to make a token.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct ForgeTokenHelp {
    pub host: String,
    pub scopes: Vec<String>,
    pub create_at: String,
}

/// A token the window hands over: never printed, and wiped as it is handed to the core.
#[derive(serde::Deserialize, specta::Type)]
#[serde(transparent)]
pub struct ForgeToken(String);

impl std::fmt::Debug for ForgeToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ForgeToken(***)")
    }
}

impl ForgeToken {
    /// The token, as the core holds it: the `String` it came in is wiped there.
    fn pasted(mut self) -> Pasted {
        Pasted::new(std::mem::take(&mut self.0))
    }
}

pub(crate) fn kind_of(word: &str) -> Result<Kind, String> {
    Kind::parse(word).ok_or_else(|| "Choose GitHub or GitLab.".to_string())
}

/// Runs `then` with the git it starts on this thread being the window's (#752): a network call to
/// a host the person has a signed-in account on gets purlis's own credential helper, this binary
/// started again, in place of the forge CLI's. Elsewhere, and in a chat's process tree, `then`
/// runs as it is, and the CLI's helper stays.
///
/// A test build (every one is fenced) runs `then` as it is too: the config home a test's window
/// would read is outside its fence. The core's tests cover the helper itself. The exception is a
/// scenario run with a fake forge named: its config home is the run's own, so its window's git
/// is the shipped one, helper and all (`e2eforge::browser_forge`).
pub(crate) fn window_git<T>(then: impl FnOnce() -> T) -> T {
    #[cfg(feature = "e2e")]
    let on_a_fake_forge = crate::e2eforge::browser_forge().is_some();
    #[cfg(not(feature = "e2e"))]
    let on_a_fake_forge = false;
    if purlis_core::fence::FENCED && !on_a_fake_forge {
        return then();
    }
    let (Some(scope), Ok(exe), Some(root)) = (
        HostScope::claim(),
        std::env::current_exe(),
        purlis_core::machine::config_root(),
    ) else {
        return then();
    };
    purlis_core::forge::gitcredential::as_the_window(&scope, &exe, &root, then)
}

pub(crate) fn config_root() -> Result<std::path::PathBuf, String> {
    purlis_core::machine::config_root()
        .ok_or_else(|| "purlis found no home directory to keep forge accounts in".to_string())
}

pub(crate) fn host_scope() -> Result<HostScope, String> {
    HostScope::claim().ok_or_else(|| "Forge sign-in is the window's, never a chat's.".to_string())
}

/// Hand this machine's forge sign-ins to the window's own forge calls (#1816): from here on a
/// call the person makes in the window, on a host they signed in to, goes over HTTPS with that
/// sign-in, and needs no `gh` or `glab`. Called once, as the app starts. A process a chat
/// started is refused the host's scope, so it hands nothing over.
pub fn hand_over_sign_ins() {
    let Some(root) = purlis_core::machine::config_root() else {
        tracing::warn!("purlis: no home directory, so no forge sign-in is used");
        return;
    };
    // A scenario run's window answers from the run's recorded forge (#1818); the CLI route stays.
    #[cfg(feature = "e2e")]
    if let Some(forge) = crate::e2eforge::stand_in() {
        purlis_core::forge::signin::hand_over_for_a_test(
            HostScope::for_a_test(),
            &root,
            Arc::new(move |_tokens: Arc<dyn purlis_core::forge::http::TokenSource>| forge.clone()),
            Arc::new(purlis_core::forge::cli::Cli::default()),
        );
        return;
    }
    if !purlis_core::forge::signin::hand_over_unless_in_a_chat(&root) {
        tracing::warn!("purlis: forge sign-ins are not handed over inside a chat");
    }
}

/// The event the window is sent when a forge ended one of its sign-ins (#729): it reads the
/// accounts again and draws the Notice with Sign in again. `ForgeSignInEnded.tsx` holds the
/// window's spelling to this one. It carries nothing: no account, no login.
pub const SIGN_IN_ENDED: &str = "forge-sign-in-ended";

/// From now on the window is told when a forge refuses to renew a sign-in (ADR 0077 §3), so
/// the person reads which sign-in ended, and how to sign in again, when it happens rather than
/// at the next thing that fails. Called once, as the app starts.
pub fn tell_the_window_when_a_sign_in_ends(app: &tauri::AppHandle) {
    use tauri::Emitter;
    let Some(scope) = HostScope::claim() else {
        return;
    };
    let app = app.clone();
    purlis_core::forge::refresh::tell_when_ended(&scope, move |_account| {
        let _ = app.emit(SIGN_IN_ENDED, ());
    });
}

/// Run `work` on a blocking thread: each act may wait on the keychain or the forge.
pub(crate) async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|err| format!("the forge accounts did not answer: {err}"))?
}

/// This machine's forge accounts.
#[tauri::command]
#[specta::specta]
pub async fn forge_accounts() -> Result<Vec<ForgeAccountRow>, String> {
    blocking(|| {
        let root = config_root()?;
        let keyring = accounts::keyring_at(&root);
        Ok(accounts::list(&root, keyring.as_ref())?
            .into_iter()
            .map(ForgeAccountRow::from)
            .collect())
    })
    .await
}

/// The scopes a token needs on `kind` and the page that makes one on `host`, or why `host` is not
/// a host.
#[tauri::command]
#[specta::specta]
pub fn forge_token_help(kind: String, host: String) -> Result<ForgeTokenHelp, String> {
    let help = accounts::token_help(kind_of(&kind)?, &host)?;
    Ok(ForgeTokenHelp {
        host: help.host,
        scopes: help.scopes.iter().map(|s| s.to_string()).collect(),
        create_at: help.create_at,
    })
}

/// Sign in to `kind` on `host` with a pasted `token`: checked with the forge, then kept in its
/// own keyring item. The account as Settings lists it, or the core's sentence saying why not.
#[tauri::command]
#[specta::specta]
pub async fn forge_sign_in_with_token(
    kind: String,
    host: String,
    token: ForgeToken,
) -> Result<ForgeAccountRow, String> {
    // Taken first, so a refusal below still leaves only the core's wiped copy.
    let token = token.pasted();
    let kind = kind_of(&kind)?;
    blocking(move || {
        let scope = host_scope()?;
        let root = config_root()?;
        let keyring = accounts::keyring_at(&root);
        accounts::sign_in_with_token(
            &scope,
            &root,
            keyring.as_ref(),
            &token_check,
            kind,
            &host,
            token,
        )
        .map(ForgeAccountRow::from)
    })
    .await
}

/// Where a pasted token is checked: the forge, over HTTPS; in a scenario run, its recorded forge
/// (#1818).
pub(crate) fn token_check(
    forge: &purlis_core::forge::Forge,
    tokens: Arc<dyn purlis_core::forge::http::TokenSource>,
) -> Result<Arc<dyn purlis_core::forge::transport::Transport>, purlis_core::forge::ForgeError> {
    #[cfg(feature = "e2e")]
    if let Some(stand_in) = crate::e2eforge::stand_in() {
        return Ok(stand_in);
    }
    accounts::over_https(forge, tokens)
}

/// A forge CLI's login purlis could import (#1376, ADR 0077 §5): `cli` is signed in to `host`,
/// where purlis has no account yet.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct ForgeCliLogin {
    /// `github` or `gitlab`.
    pub kind: String,
    pub host: String,
    /// `gh` or `glab`.
    pub cli: String,
    /// The forge, as a sentence names it (`GitHub`, `GitLab`).
    pub title: String,
}

impl From<CliLogin> for ForgeCliLogin {
    fn from(login: CliLogin) -> Self {
        ForgeCliLogin {
            kind: login.kind.word().to_string(),
            title: login.kind.display().to_string(),
            cli: login.cli.to_string(),
            host: login.host,
        }
    }
}

/// The `gh` and `glab` logins on this machine that purlis could import: offered, never taken.
/// Each CLI is asked whether it is signed in, never for its token.
#[tauri::command]
#[specta::specta]
pub async fn forge_cli_logins() -> Result<Vec<ForgeCliLogin>, String> {
    blocking(|| {
        let root = config_root()?;
        Ok(cliimport::offered_here(&root)
            .into_iter()
            .map(ForgeCliLogin::from)
            .collect())
    })
    .await
}

/// The person's yes to an offer: the token `kind`'s CLI keeps for `host` copied into purlis's own
/// keyring item, checked with the forge, and listed as an imported account. From then on chats
/// on that host have no forge credential from purlis (V22c). The account, or the core's sentence
/// saying why not; neither ever holds the token.
#[tauri::command]
#[specta::specta]
pub async fn forge_import_cli_login(kind: String, host: String) -> Result<ForgeAccountRow, String> {
    let kind = kind_of(&kind)?;
    blocking(move || {
        let scope = host_scope()?;
        let root = config_root()?;
        let keyring = accounts::keyring_at(&root);
        cliimport::import(&scope, &root, keyring.as_ref(), &token_check, kind, &host)
            .map(ForgeAccountRow::from)
    })
    .await
}

/// What the window shows while a device flow waits: the code to type, and the forge's own page
/// to type it on. Nothing here is secret: the code is of no use without the person's own
/// confirmation on that page.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct ForgeDeviceCode {
    /// Which flow this is: what waiting and cancelling name it by.
    pub flow: u32,
    pub host: String,
    /// The forge, as a sentence names it (`GitHub`).
    pub title: String,
    /// The code the person types on the forge's page.
    pub user_code: String,
    /// The forge's own page, opened in the system browser.
    pub verification_uri: String,
    /// How many seconds the code lasts.
    pub expires_in: u32,
    /// The scopes the sign-in asks for, each explained on the page.
    pub scopes: Vec<String>,
}

/// A device flow this process is running: held in memory for its length, and nowhere else.
struct DeviceFlow {
    number: u32,
    registration: &'static Registration,
    started: Arc<device::Started>,
    cancelled: Arc<AtomicBool>,
}

/// The one device flow in progress. Starting another ends this one.
static DEVICE_FLOW: Mutex<Option<DeviceFlow>> = Mutex::new(None);

/// The number the next flow gets.
static NEXT_FLOW: AtomicU32 = AtomicU32::new(1);

/// How often a waiting flow looks for a cancel.
const CANCEL_CHECK: Duration = Duration::from_millis(200);

/// Waits between a flow's polls, a slice at a time, so a cancel ends it within a moment.
struct UntilCancelled(Arc<AtomicBool>);

impl device::Pace for UntilCancelled {
    fn wait(&self, for_: Duration) -> bool {
        let mut left = for_;
        while !left.is_zero() {
            if self.0.load(Ordering::SeqCst) {
                return false;
            }
            let slice = left.min(CANCEL_CHECK);
            std::thread::sleep(slice);
            left -= slice;
        }
        !self.0.load(Ordering::SeqCst)
    }
}

/// Why `kind` on `host` has no browser sign-in, when it has none.
fn no_device_flow(kind: Kind, host: &str) -> String {
    format!(
        "purlis signs in through the browser on github.com only. On {host}, sign in to {} with \
         a token.",
        kind.display()
    )
}

/// Start signing in to `kind` on `host` by the device flow: the forge is asked for a code with
/// purlis's public client id, and the code and the forge's page are answered for the window to
/// show. A flow already waiting is ended. Refused, with the token named as the way, on a host
/// purlis has no registration on.
#[tauri::command]
#[specta::specta]
pub async fn forge_device_sign_in_start(
    kind: String,
    host: String,
) -> Result<ForgeDeviceCode, String> {
    let kind = kind_of(&kind)?;
    blocking(move || {
        host_scope()?;
        let host = accounts::host_named(kind, &host)?;
        let registration = registration::on(kind, &host)
            .filter(|r| r.flow == registration::Flow::Device)
            .ok_or_else(|| no_device_flow(kind, &host))?;
        // A test build (the e2e build among them) has no person at a browser, and must never
        // reach the real forge: the same refusal as the other browser sign-in's.
        if purlis_core::fence::FENCED {
            return Err(
                "A test build of purlis does not sign in through the browser. Sign in with a \
                 token instead."
                    .to_string(),
            );
        }
        let started = device::start(
            &oauth::Https::new(),
            &oauth::Endpoints::of(registration),
            registration,
        )?;
        let number = NEXT_FLOW.fetch_add(1, Ordering::SeqCst);
        let code = ForgeDeviceCode {
            flow: number,
            host: registration.host.to_string(),
            title: kind.display().to_string(),
            user_code: started.user_code.clone(),
            verification_uri: started.verification_uri.clone(),
            expires_in: u32::try_from(started.expires_in.as_secs()).unwrap_or(u32::MAX),
            scopes: registration
                .scopes()
                .iter()
                .map(|s| s.to_string())
                .collect(),
        };
        let mut held = DEVICE_FLOW.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(before) = held.replace(DeviceFlow {
            number,
            registration,
            started: Arc::new(started),
            cancelled: Arc::new(AtomicBool::new(false)),
        }) {
            before.cancelled.store(true, Ordering::SeqCst);
        }
        Ok(code)
    })
    .await
}

/// Wait for the person to confirm the code of `flow` on the forge's page, then sign the account
/// in: the token is checked with the forge and kept, with its refresh token, in the account's
/// own keyring item. The account as Settings lists it, or a sentence saying how the flow ended
/// and what to do next.
#[tauri::command]
#[specta::specta]
pub async fn forge_device_sign_in_wait(flow: u32) -> Result<ForgeAccountRow, String> {
    blocking(move || {
        let scope = host_scope()?;
        let (registration, started, cancelled) = {
            let held = DEVICE_FLOW.lock().unwrap_or_else(|e| e.into_inner());
            match held.as_ref().filter(|held| held.number == flow) {
                Some(held) => (
                    held.registration,
                    held.started.clone(),
                    held.cancelled.clone(),
                ),
                None => return Err(device::Ended::Cancelled.said("the forge")),
            }
        };
        let root = config_root()?;
        let keyring = accounts::keyring_at(&root);
        let ended = device::finish(
            &scope,
            &root,
            keyring.as_ref(),
            &token_check,
            &oauth::Https::new(),
            &oauth::Endpoints::of(registration),
            registration,
            &started,
            &UntilCancelled(cancelled),
        );
        // The flow is over either way: nothing of it is kept.
        let mut held = DEVICE_FLOW.lock().unwrap_or_else(|e| e.into_inner());
        if held.as_ref().is_some_and(|held| held.number == flow) {
            *held = None;
        }
        ended
            .map(ForgeAccountRow::from)
            .map_err(|why| why.said(registration.kind.display()))
    })
    .await
}

/// Stop waiting on `flow`: the polling ends within a moment, and nothing of the flow is kept.
/// The code may still be confirmed on the forge's page; nothing is listening for it then.
#[tauri::command]
#[specta::specta]
pub fn forge_device_sign_in_cancel(flow: u32) {
    let mut held = DEVICE_FLOW.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(flow) = held.take_if(|held| held.number == flow) {
        flow.cancelled.store(true, Ordering::SeqCst);
    }
}

/// Sign out of the account `id`: its token deleted from the keychain and the account taken off
/// this machine. Then, for a browser sign-in on a forge that takes a revocation with no secret
/// (gitlab.com), the forge is asked to end the token (ADR 0077 §3).
///
/// Answers what became of the token at the forge: `true` when the forge ended it, `false` when
/// it was asked and did not (the person is signed out here all the same, and the row says
/// where to end it), and nothing when no forge was asked: a token the person made, or GitHub,
/// whose revocation needs a secret purlis does not have.
#[tauri::command]
#[specta::specta]
pub async fn forge_sign_out(id: String) -> Result<Option<bool>, String> {
    use purlis_core::forge::revoke::{self, AtTheForge};
    blocking(move || {
        let scope = host_scope()?;
        let root = config_root()?;
        let keyring = accounts::keyring_at(&root);
        // A test build (the e2e build among them) never reaches the real forge.
        let endpoint = (!purlis_core::fence::FENCED).then(oauth::Https::new);
        let (_, at) = revoke::sign_out(
            &scope,
            &root,
            keyring.as_ref(),
            endpoint.as_ref().map(|https| https as &dyn oauth::Endpoint),
            None,
            &id,
        )?;
        Ok(match at {
            AtTheForge::Ended => Some(true),
            AtTheForge::NotEnded => Some(false),
            AtTheForge::NotAsked => None,
        })
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_handed_token_never_prints() {
        let token: ForgeToken = serde_json::from_str("\"handed-over-once\"").unwrap();
        assert_eq!(format!("{token:?}"), "ForgeToken(***)");
    }

    #[test]
    fn a_row_says_how_it_was_signed_in_and_which_scopes_are_wider_than_needed() {
        let row = ForgeAccountRow::from(ForgeAccount {
            id: "01J9ACCOUNT".into(),
            kind: Kind::GitHub,
            host: "github.com".into(),
            login: "octocat".into(),
            method: Method::Pat,
            state: State::SignedIn,
            scopes: Some(vec!["repo".into(), "read:org".into(), "gist".into()]),
        });
        assert_eq!(row.method, "token");
        assert_eq!(row.kind, "github");
        assert!(row.signed_in);
        assert_eq!(row.wider, ["gist"]);
    }

    #[test]
    fn a_cancel_ends_a_wait_within_a_moment() {
        use device::Pace;
        let cancelled = Arc::new(AtomicBool::new(false));
        let pace = UntilCancelled(cancelled.clone());
        assert!(pace.wait(Duration::from_millis(1)));
        let flag = cancelled.clone();
        let canceller = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            flag.store(true, Ordering::SeqCst);
        });
        let began = std::time::Instant::now();
        assert!(!pace.wait(Duration::from_secs(600)));
        assert!(began.elapsed() < Duration::from_secs(30));
        canceller.join().unwrap();
        assert!(!pace.wait(Duration::from_millis(1)), "a cancel is final");
    }

    #[test]
    fn a_cancel_or_a_wait_for_a_flow_that_is_not_running_changes_nothing() {
        forge_device_sign_in_cancel(u32::MAX);
        assert!(no_device_flow(Kind::GitLab, "gitlab.com").contains("with a token"));
        assert!(no_device_flow(Kind::GitHub, "ghe.example.com").contains("ghe.example.com"));
    }

    #[test]
    fn an_offer_names_its_cli_its_host_and_its_forge() {
        let row = ForgeCliLogin::from(CliLogin {
            kind: Kind::GitLab,
            host: "gitlab.com".into(),
            cli: "glab",
        });
        assert_eq!(
            row,
            ForgeCliLogin {
                kind: "gitlab".into(),
                host: "gitlab.com".into(),
                cli: "glab".into(),
                title: "GitLab".into(),
            }
        );
    }

    #[test]
    fn the_help_for_a_host_names_the_scopes_and_the_token_page() {
        let help = forge_token_help("gitlab".into(), "git.example.com".into()).unwrap();
        assert_eq!(help.scopes, ["api"]);
        assert!(help.create_at.starts_with("https://git.example.com/"));
        assert!(forge_token_help("svn".into(), "x".into()).is_err());
    }
}

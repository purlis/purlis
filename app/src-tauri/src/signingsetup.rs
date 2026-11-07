// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Signed commits, set up from the window** (#5628, spec #5607, ADR 0116): the commands the
//! Signed commits card in Settings › You › Accounts and its setup dialog call. Each is the
//! window's alone (`session_protocol::ui::WINDOW_ONLY`): no chat or link reaches the signing key
//! or adds it to an account.
//!
//! The steps are `purlis_core::signingsetup`'s and `purlis_core::forge::signingkeys`'s; the
//! forge is asked with the window's own sign-in on github.com, never `gh`. The permission
//! step-up is `forgeaccounts::forge_signing_permission_start`'s.

use std::sync::Mutex;

use purlis_core::commitsign;
use purlis_core::forge::signingkeys::{self, Refused};
use purlis_core::forge::{Caller, Kind, accounts};
use purlis_core::signingsetup;

use crate::forgeaccounts::{blocking, config_root};

/// What the card shows: whether this device has a signing key, and the GitHub account it goes to.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct SigningState {
    /// "On": this device has a signing key (ADR 0116 §1). Nothing else turns it on.
    pub on: bool,
    /// What the key is titled on the forge: `purlis · <device>`.
    pub title: String,
    /// The GitHub account on github.com setup adds the key to, where one is signed in.
    pub account: Option<SigningAccount>,
}

/// The forge account setup adds the key to.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct SigningAccount {
    pub login: String,
    pub host: String,
    /// The forge, as a sentence names it: `GitHub`.
    pub forge: String,
    /// Whether its sign-in may add a signing key already, so setup asks for no permission.
    pub may_add: bool,
    /// The email the last Check in this window found verified, for "Verified on GitHub ✓".
    /// Only the forge's own answer sets it, and it lasts the window's life.
    pub verified_as: Option<String>,
}

/// What Check found: GitHub's own verdict on a signed test commit.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct SigningCheck {
    pub verified: bool,
    /// GitHub's reason, such as `valid`, `unverified_email` or `unknown_key`: the window words
    /// it.
    pub reason: String,
    /// The email the test commit was made as: chat commits use it too.
    pub email: String,
    /// The repository the test commit was made in.
    pub repo: String,
}

/// How adding the key went.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum SigningAdded {
    /// GitHub has it now, or had it already.
    Added,
    /// The sign-in may not add it: setup asks for the permission, or the person adds it by hand.
    NeedsPermission,
}

/// The account and email the last Check in this window found verified.
static VERIFIED: Mutex<Option<(String, String)>> = Mutex::new(None);

fn github() -> purlis_core::forge::Forge {
    signingsetup::github()
}

/// The GitHub account on github.com, where one is signed in.
fn the_account(
    root: &std::path::Path,
) -> Result<Option<purlis_core::forge::accounts::ForgeAccount>, String> {
    accounts::signed_in_on(root, Kind::GitHub, "github.com")
}

/// The window's transport to GitHub, with its own sign-in.
fn transport() -> Result<std::sync::Arc<dyn purlis_core::forge::transport::Transport>, String> {
    signingkeys::window(&github(), &Caller::window())
        .map_err(|_| "Sign in to GitHub first, in Settings › You › Accounts.".to_string())
}

/// This device's signing key, or a sentence where it has none.
fn the_key(root: &std::path::Path) -> Result<commitsign::Signer, String> {
    let keyring = commitsign::this_builds_keyring(root);
    signingsetup::key_kept(root, keyring.as_ref())?
        .ok_or_else(|| "This Mac has no signing key yet. Make it first.".to_string())
}

/// **What the Signed commits card shows.** Asks the keychain only whether the key is there, never
/// for the key, so opening Settings never makes macOS ask ([`signing_key_shown`] reads it).
#[tauri::command]
#[specta::specta]
pub async fn signing_state() -> Result<SigningState, String> {
    blocking(|| {
        let root = config_root()?;
        let keyring = commitsign::this_builds_keyring(&root);
        let on = signingsetup::key_is_there(&root, keyring.as_ref())?;
        let verified = VERIFIED.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let account = the_account(&root)?.map(|account| SigningAccount {
            may_add: accounts::holds(&account, &signingkeys::GITHUB_SCOPES),
            verified_as: verified
                .filter(|(login, _)| *login == account.login)
                .map(|(_, email)| email),
            forge: account.kind.display().to_string(),
            login: account.login,
            host: account.host,
        });
        Ok(SigningState {
            on,
            title: signingkeys::title(),
            account,
        })
    })
    .await
}

/// This device's key as Details and the by-hand step show it: never its private half.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct SigningKeyShown {
    /// `SHA256:…`, as GitHub shows it.
    pub fingerprint: String,
    /// The public line to paste on GitHub's page.
    pub public_key: String,
}

/// **This device's key, shown**: read from the Keychain only when the person opens Details or
/// adds the key by hand, so the card itself never makes macOS ask.
#[tauri::command]
#[specta::specta]
pub async fn signing_key_shown() -> Result<SigningKeyShown, String> {
    blocking(|| {
        let key = the_key(&config_root()?)?;
        Ok(SigningKeyShown {
            fingerprint: key.fingerprint(),
            public_key: key.public_line(),
        })
    })
    .await
}

/// **Step 1: this device's key, made and kept in the Keychain**, where it has none. macOS asks the
/// person here; where they say no, the sentence says to try again and choose Always Allow.
#[tauri::command]
#[specta::specta]
pub async fn signing_make_key() -> Result<String, String> {
    blocking(|| {
        let root = config_root()?;
        let keyring = commitsign::this_builds_keyring(&root);
        signingsetup::key_made(&root, keyring.as_ref())
            .map(|(key, _)| key.fingerprint())
            .map_err(|_| {
                "macOS said no, so purlis couldn't keep the key. Try again and choose Always \
                 Allow."
                    .to_string()
            })
    })
    .await
}

/// **Step 2: the key added to the GitHub account** with the window's sign-in, under
/// `purlis · <device>`. One GitHub has already is added again as nothing.
#[tauri::command]
#[specta::specta]
pub async fn signing_add_key() -> Result<SigningAdded, String> {
    blocking(|| {
        let root = config_root()?;
        let key = the_key(&root)?;
        let transport = transport()?;
        match signingkeys::listed_over(transport.as_ref(), &github()) {
            Ok(listed) if listed.this_device(&key).is_some() => return Ok(SigningAdded::Added),
            Ok(_) => {}
            Err(Refused::NeedsPermission) => return Ok(SigningAdded::NeedsPermission),
            Err(why) => return Err(why.said(Kind::GitHub)),
        }
        match signingkeys::add_over(transport.as_ref(), &github(), &key, &signingkeys::title()) {
            Ok(_) => Ok(SigningAdded::Added),
            Err(Refused::NeedsPermission) => Ok(SigningAdded::NeedsPermission),
            Err(why) => Err(why.said(Kind::GitHub)),
        }
    })
    .await
}

/// **Whether GitHub lists this device's key on the account yet**, from its public list, which
/// needs no permission: how the by-hand path notices the person added it.
#[tauri::command]
#[specta::specta]
pub async fn signing_key_listed() -> Result<bool, String> {
    blocking(|| {
        let root = config_root()?;
        let key = the_key(&root)?;
        let account = the_account(&root)?
            .ok_or_else(|| "Sign in to GitHub first, in Settings › You › Accounts.".to_string())?;
        let transport = transport()?;
        signingkeys::listed_publicly_over(transport.as_ref(), &github(), &account.login)
            .map(|listed| listed.this_device(&key).is_some())
            .map_err(|why| why.said(Kind::GitHub))
    })
    .await
}

/// **Step 3, Check**: a test commit signed with this device's key, made in `repo` (`owner/name`)
/// or, with none, in the account's own most recently pushed repository, and GitHub's own verdict
/// on it. Never on a branch. A verdict of Verified is kept for the card while the window runs.
#[tauri::command]
#[specta::specta]
pub async fn signing_check(repo: Option<String>) -> Result<SigningCheck, String> {
    blocking(move || {
        let root = config_root()?;
        let key = the_key(&root)?;
        let account = the_account(&root)?
            .ok_or_else(|| "Sign in to GitHub first, in Settings › You › Accounts.".to_string())?;
        let who = signingsetup::who_commits()?;
        let transport = transport()?;
        let repo = match repo {
            Some(repo) => repo,
            None => signingsetup::a_repo_to_check_in(transport.as_ref(), &github())?.ok_or_else(
                || {
                    "Your GitHub account has no repository of its own to check in. Make one, \
                     then check again."
                        .to_string()
                },
            )?,
        };
        let when = chrono::Utc::now().timestamp();
        let checked = signingsetup::check_with_a_test_commit(
            transport.as_ref(),
            &github(),
            &key,
            &repo,
            &who,
            when,
        )?;
        let mut verified = VERIFIED.lock().unwrap_or_else(|e| e.into_inner());
        *verified = checked
            .verified
            .then(|| (account.login.clone(), who.email.clone()));
        Ok(SigningCheck {
            verified: checked.verified,
            reason: checked.reason,
            email: who.email,
            repo,
        })
    })
    .await
}

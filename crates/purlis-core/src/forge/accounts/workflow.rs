// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The second token** (ADR 0128 D1.1 and D8.3): the one credential that may push a CI file, and
//! the one no chat's route ever carries.
//!
//! GitHub refuses a push that adds or changes a workflow file unless the token has the
//! **`workflow`** scope. The everyday sign-in never asks for it, so the CI-file protection holds
//! on GitHub even if the proxy had a bug. A held CI push (a person approving it in Asks) needs
//! it, so it is made **on that press, the first time it is needed**, by a second device flow that
//! asks for what the account holds and `workflow`, and it is kept as **its own keyring item**
//! beside the everyday one: the account field [`ITEM`] of the same service, where the everyday
//! token is the field `token`. [`super::token`] and everything that reads it for a chat's route
//! ([`crate::forge::credential`]) read the field `token` and never this one.
//!
//! **Who may read it.** [`for_ci_push`] takes a [`HostScope`], refused inside a chat's process
//! tree, and answers only the app's own code that performs an approved, held CI push. Nothing
//! else in this crate reads the item, and `tests/it/the_second_token_is_kept_apart_and_never_on_a_chat_route.rs` pins that.
//!
//! **GitHub may fold the scope into the first token** (a re-consent can widen the grant the
//! everyday token already belongs to; ADR 0128 Open item 3 is the live probe that settles it).
//! After the second sign-in the everyday token is read back, and **if it now says `workflow`
//! ([`Outcome::Folded`]) nothing is kept, the account's recorded scopes are corrected, and a CI-file
//! push is not offered** ([`Availability::NotOffered`]): the ask says to push it from the person's own
//! terminal, rather than the everyday token silently gaining `workflow`. **That token is then
//! never served to a chat's route** ([`super::everyday_holds_workflow`]: the credential source
//! refuses it, the window raises a Notice, and the way out is a press: revoke purlis on GitHub and
//! sign in again, [`needs_narrower_sign_in`]). **A read-back that says nothing about scopes** (no
//! `X-OAuth-Scopes`) **is a refusal, never "not folded"**: purlis cannot tell, so nothing is kept.
//!
//! **GitLab has no second token.** Its one scope, `api`, covers the API and git over HTTP, so the
//! protection there is the hold and its Ask (D8.3), not a scope. [`for_ci_push`] answers the
//! everyday token for a GitLab account, to the app alone.

use super::*;
use crate::forge::refresh::Trouble;

/// The scope GitHub requires to push a workflow file.
pub const SCOPE: &str = "workflow";

/// The keyring item's account field: the second token, beside the everyday one's `token`.
const ITEM: &str = "workflow";

/// Whether a CI-file push can be performed for an account, and what it needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Availability {
    /// The second token is kept (GitHub), or the account is GitLab's, whose one token serves.
    Ready,
    /// GitHub, and the person has not allowed `workflow` yet: the Ask for the push offers the
    /// press that makes the second token.
    NeedsPress,
    /// GitHub folded `workflow` into the everyday token, so the two cannot be kept apart: a
    /// CI-file push is not offered, and the person pushes it from their own terminal.
    NotOffered,
}

/// What a held CI push is to be performed with.
pub enum CiPushCredential {
    /// The token to push with, for the app's one call.
    Token(SecretString),
    /// GitHub, with no second token yet: ask the person's press first.
    NeedsPress,
    /// Not offered: [`Availability::NotOffered`].
    NotOffered,
}

impl std::fmt::Debug for CiPushCredential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            CiPushCredential::Token(_) => "Token(***)",
            CiPushCredential::NeedsPress => "NeedsPress",
            CiPushCredential::NotOffered => "NotOffered",
        })
    }
}

/// How asking for the second token ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The second token is kept, and the everyday one still lacks `workflow`.
    Kept,
    /// GitHub folded the scope into the everyday token. Nothing was kept; the account, with the
    /// scopes the forge now says it holds.
    Folded(ForgeAccount),
}

/// The scopes a second sign-in for `account` asks for: those it holds, then `workflow`.
pub fn scopes_to_ask(account: &ForgeAccount) -> Vec<String> {
    widened_scopes(account, &[SCOPE])
}

/// Whether a CI-file push can be performed for `account`.
pub fn availability(keyring: &dyn Store, account: &ForgeAccount) -> Availability {
    if account.kind != Kind::GitHub {
        return Availability::Ready;
    }
    if holds(account, &[SCOPE]) {
        return Availability::NotOffered;
    }
    let present = match keyring.exists(&service(account), ITEM) {
        Presence::Present => true,
        Presence::Absent => false,
        Presence::Unknown => matches!(keyring.get(&service(account), ITEM), Ok(Some(_))),
    };
    if present {
        Availability::Ready
    } else {
        Availability::NeedsPress
    }
}

/// **The credential for a held CI-file push** the person approved: the second token on GitHub
/// (a `workflow`-scoped one, never the everyday token), GitLab's one token on GitLab. For the
/// app's own call, in the host's process: a [`HostScope`] is refused inside a chat's tree.
pub fn for_ci_push(
    _host: &HostScope,
    keyring: &dyn Store,
    account: &ForgeAccount,
) -> Result<CiPushCredential, String> {
    if account.kind != Kind::GitHub {
        return Ok(match token(keyring, account)? {
            Some(token) => CiPushCredential::Token(token),
            None => CiPushCredential::NeedsPress,
        });
    }
    match availability(keyring, account) {
        Availability::NotOffered => return Ok(CiPushCredential::NotOffered),
        Availability::NeedsPress => return Ok(CiPushCredential::NeedsPress),
        Availability::Ready => {}
    }
    let read = keyring.get(&service(account), ITEM).map_err(|e| {
        format!(
            "purlis could not read the CI permission for {} on {} from the keychain: {}",
            account.login,
            account.host,
            unechoed(e.message.to_string(), &SecretString::from(""))
        )
    })?;
    let Some(secret) = read else {
        return Ok(CiPushCredential::NeedsPress);
    };
    let mut text = secret.into_inner();
    let held = SecretString::from(text.trim());
    text.zeroize();
    crate::applog::keep_out(&held);
    Ok(CiPushCredential::Token(held))
}

/// **Starts the second sign-in's account out as not known to be free of `workflow`**, before any
/// step that can fail or be abandoned. The second grant may widen the one the everyday token
/// belongs to, and a flow can end in ways that never reach [`keep`] (cancelled, expired, a poll
/// that failed, the app quit): from here on the everyday token is served to no chat until a
/// successful read-back in [`keep`] records what it holds. If the record cannot be written this
/// fails and the flow must not start.
pub fn begin(_host: &HostScope, config_root: &Path, account: &ForgeAccount) -> Result<(), String> {
    if account.kind != Kind::GitHub {
        return Ok(());
    }
    if !scopes_recorded(config_root, account)? {
        return Ok(());
    }
    mark_unknown(config_root, account)
        .map_err(|why| format!("{why} Nothing was started, so nothing was changed."))
}

/// Whether the accounts file records scopes for `account` now (what a chat is served by).
fn scopes_recorded(config_root: &Path, account: &ForgeAccount) -> Result<bool, String> {
    let file = read(config_root)?;
    Ok(file
        .accounts
        .iter()
        .filter_map(account_of)
        .find(|a| a.id == account.id)
        .ok_or_else(|| "That account is not on this machine any more.".to_string())?
        .scopes
        .is_some())
}

/// **Keeps the second token** a second sign-in `granted` for `account`, on the person's press.
///
/// Refused, with nothing kept, unless: the account is on GitHub, the grant is for the same login
/// on the same host (the forge says so), and the grant holds `workflow`. Then the everyday token
/// is read back: if it now holds `workflow` too, the forge folded the scopes together and
/// [`Outcome::Folded`] answers with nothing kept. Otherwise the token is written as its own
/// keyring item and [`Outcome::Kept`] answers.
pub fn keep(
    host: &HostScope,
    config_root: &Path,
    keyring: &dyn Store,
    reach: Reach<'_>,
    account: &ForgeAccount,
    granted: Granted,
) -> Result<Outcome, String> {
    if account.kind != Kind::GitHub {
        return Err(format!(
            "{} has no separate permission for CI files: its one sign-in covers them, and purlis \
             asks you before it pushes one. Nothing was changed.",
            account.host
        ));
    }
    // Whatever the caller did, nothing below can leave the everyday token served on a path that
    // ends early: the account is not known to be clean until the read-back at the end says so.
    begin(host, config_root, account)?;
    let second = granted.access.clone();
    crate::applog::keep_out(&second);
    if second.expose_secret().is_empty() {
        return Err(format!(
            "{} gave no token, so nothing was changed.",
            account.host
        ));
    }
    let forge = Forge {
        kind: account.kind,
        host: account.host.clone(),
    };
    let checked = reach(&forge, Arc::new(Checking(second.clone())))
        .map_err(|e| e.to_string())
        .and_then(|transport| check(transport.as_ref(), &forge, true))
        .map_err(|why| unechoed(why, &second))?;
    if checked.login != account.login {
        return Err(format!(
            "The code was confirmed as {} on {}, not as {}, so nothing was changed. Confirm the \
             code while signed in to GitHub as {}.",
            checked.login, forge.host, account.login, account.login
        ));
    }
    let granted_scopes = granted
        .scopes
        .clone()
        .or(checked.scopes.clone())
        .unwrap_or_default();
    if !grants(Kind::GitHub, &granted_scopes, SCOPE) {
        return Err(format!(
            "{} did not allow {SCOPE}, so nothing was changed. Try again, and allow it on the page \
             GitHub shows.",
            forge.host
        ));
    }

    // Did GitHub keep the two tokens' scopes apart? Read the everyday token back.
    let everyday = token(keyring, account)?.ok_or_else(|| {
        format!(
            "The sign-in to {} as {} ended while GitHub's page was open, so nothing was changed. \
             Sign in again.",
            account.host, account.login
        )
    })?;
    let everyday_now = match reach(&forge, Arc::new(Checking(everyday.clone())))
        .map_err(|e| e.to_string())
        .and_then(|transport| check(transport.as_ref(), &forge, true))
    {
        Ok(now) => now,
        Err(why) => {
            // GitHub may have folded `workflow` into the everyday token, and it cannot be read:
            // the account is already recorded as not known ([`begin`]), so it is served to no
            // chat until a read-back succeeds.
            return Err(format!(
                "{} Purlis cannot tell whether the everyday sign-in as {} now allows CI files, so \
                 it gives that sign-in to no chat until it can. Nothing was kept. Try again later.",
                unechoed(why, &everyday),
                account.login
            ));
        }
    };
    // The second token is dropped here, in memory, on every path that does not write it below.
    let Some(scopes) = &everyday_now.scopes else {
        return Err(format!(
            "{} did not say what purlis's everyday sign-in as {} may do, so purlis cannot tell \
             whether it now allows CI files too. Nothing was kept, and a push that changes a CI \
             file stays held. Try again later.",
            forge.host, account.login
        ));
    };
    if grants(Kind::GitHub, scopes, SCOPE) {
        let account = set_scopes(config_root, account, scopes.clone())?;
        crate::forge::refresh::raise_once(&account, &Trouble::EverydayHoldsWorkflow);
        return Ok(Outcome::Folded(account));
    }

    // A clean read-back is the only thing that records what the everyday token holds again, so a
    // retry after an unknown outcome serves it once more.
    set_scopes(config_root, account, scopes.clone())?;
    keyring
        .set(&service(account), ITEM, second.expose_secret())
        .map_err(|e| unechoed(e.message.to_string(), &second))?;
    Ok(Outcome::Kept)
}

/// What a sign-in's `account` is told when its scopes are not known.
pub fn scopes_unknown_said(account: &ForgeAccount) -> String {
    if matches!(account.method, Method::Pat | Method::Import) {
        // A pasted token (a fine-grained one never says) has no sign-in to repeat.
        return format!(
            "purlis cannot check what the token for {} on {} may do, so it may allow CI files \
             ({SCOPE}) and purlis does not give it to your chats. Use a token or a sign-in that \
             says what it may do.",
            account.login, account.host
        );
    }
    format!(
        "{} did not say what purlis's sign-in as {} may do, so it may allow CI files ({SCOPE}) and \
         purlis does not give it to your chats. Sign in again here to check it.",
        account.host, account.login
    )
}

/// Marks `account`'s scopes as not known, so it is served to no chat until a read-back says what
/// it holds, and says so in a Notice.
fn mark_unknown(config_root: &Path, account: &ForgeAccount) -> Result<(), String> {
    let account = clear_scopes(config_root, account)?;
    crate::forge::refresh::raise_once(&account, &Trouble::ScopesUnknown);
    Ok(())
}

/// What the person does when `account`'s everyday token holds `workflow` and purlis therefore
/// serves it to no chat ([`super::everyday_holds_workflow`]).
pub fn needs_narrower_sign_in(account: &ForgeAccount) -> String {
    format!(
        "GitHub added permission to push CI files ({SCOPE}) to purlis's sign-in to {} as {}, so \
         purlis does not give it to your chats. Revoke purlis under Settings › Applications › \
         Authorized OAuth Apps on {}, then sign in again here without allowing CI files.",
        account.host, account.login, account.host
    )
}

/// Deletes the second token of `account`, if there is one. Called wherever the everyday token's
/// item is deleted, so a signed-out account leaves nothing behind.
pub(super) fn remove(keyring: &dyn Store, account: &ForgeAccount) -> Result<(), String> {
    keyring
        .delete(&service(account), ITEM)
        .map(|_| ())
        .map_err(|e| {
            format!(
                "purlis could not delete the CI permission from the keychain: {}",
                e.message
            )
        })
}

/// `account` with `scopes` recorded as what the forge says it holds.
fn set_scopes(
    config_root: &Path,
    account: &ForgeAccount,
    scopes: Vec<String>,
) -> Result<ForgeAccount, String> {
    write_scopes(config_root, account, Some(scopes))
}

/// `account` with its scopes recorded as not known.
pub(super) fn clear_scopes(
    config_root: &Path,
    account: &ForgeAccount,
) -> Result<ForgeAccount, String> {
    write_scopes(config_root, account, None)
}

fn write_scopes(
    config_root: &Path,
    account: &ForgeAccount,
    scopes: Option<Vec<String>>,
) -> Result<ForgeAccount, String> {
    let _one = WRITING.lock().unwrap_or_else(|e| e.into_inner());
    let mut file = read(config_root)?;
    let at = file
        .accounts
        .iter()
        .position(|value| account_of(value).is_some_and(|a| a.id == account.id))
        .ok_or_else(|| "That account is not on this machine any more.".to_string())?;
    let Some(Value::Object(fields)) = file.accounts.get_mut(at) else {
        return Err("That account is not on this machine any more.".to_string());
    };
    match scopes {
        Some(scopes) => {
            fields.insert(
                "scopes".to_string(),
                serde_json::to_value(&scopes).unwrap_or(Value::Null),
            );
        }
        None => {
            fields.remove("scopes");
        }
    }
    let updated = account_of(&file.accounts[at])
        .ok_or_else(|| "That account is not on this machine any more.".to_string())?;
    write(config_root, file)?;
    // What it was told of this account was about the scopes before: told afresh if it holds again.
    crate::forge::refresh::forget_raised(&account.id);
    Ok(updated)
}

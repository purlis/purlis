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

/// What a person is told after a second sign-in has begun and ended without a kept token: [`begin`]
/// already recorded the account's scopes as not known, so "nothing changed" would be untrue.
pub const HELD_BACK: &str =
    "Your chats cannot use this account until you try again or sign in again.";

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
    /// The second token is kept, but an earlier code is still approvable, so the everyday token's
    /// scopes stay unknown (and it is served to no chat) until that code has lapsed.
    KeptAccountHeld,
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
    mark_unknown(config_root, account).map_err(|why| format!("{why} Nothing was started."))
}

/// The account field that records each second sign-in's code still approvable: an object of the
/// code the person typed (`user_code`, which names the flow and is no secret) to the time, in
/// seconds since the epoch, after which GitHub has surely let it lapse. **One entry per issued
/// code**: pressing again while an earlier code is live adds a second entry, and each ends only
/// its own.
const FLOW_FIELD: &str = "ci_flows";

/// A little past the code's own lifetime, for the time the forge and this machine disagree about.
const FLOW_SLACK: std::time::Duration = std::time::Duration::from_secs(60);

/// What the record says of codes that may still be approved.
enum Flows {
    /// None: every recorded code has lapsed or ended.
    None,
    /// At least one, the latest of which lapses at this time.
    Until(u64),
    /// The field is there and cannot be read (an entry that is no time, or no object at all). A
    /// code may be live and nothing says when it ends: **fails closed**, as pending.
    Unreadable,
}

fn flows_of(rest: &Map<String, Value>) -> Flows {
    let now = now();
    match rest.get(FLOW_FIELD) {
        None => Flows::None,
        Some(Value::Object(codes)) => {
            let mut latest = 0;
            for until in codes.values() {
                match until.as_u64() {
                    Some(until) => latest = latest.max(until),
                    None => return Flows::Unreadable,
                }
            }
            if latest > now {
                Flows::Until(latest)
            } else {
                Flows::None
            }
        }
        Some(_) => Flows::Unreadable,
    }
}

/// Whether an account's other fields (`rest` of the entry) say a CI-permission code may still be
/// approved on the forge: **any** code unexpired, or a record that cannot be read.
pub(super) fn flow_pending(rest: &Map<String, Value>) -> bool {
    !matches!(flows_of(rest), Flows::None)
}

/// What a person is told when `account`'s sign-in cannot be believed because a CI-permission code
/// may still be approved, or `None` when none can. It says how long to wait, from the record.
pub fn waiting_said(config_root: &Path, account: &ForgeAccount) -> Option<String> {
    let file = read(config_root).ok()?;
    let fields = file
        .accounts
        .iter()
        .find(|value| account_of(value).is_some_and(|a| a.id == account.id))
        .and_then(Value::as_object)?;
    let how_long = match flows_of(fields) {
        Flows::None => return None,
        Flows::Until(until) => {
            let minutes = until.saturating_sub(now()).div_ceil(60).max(1);
            if minutes == 1 {
                "about a minute".to_string()
            } else {
                format!("about {minutes} minutes")
            }
        }
        Flows::Unreadable => "up to about 15 minutes".to_string(),
    };
    Some(format!(
        "A code you started for CI files on {} can still be allowed for {how_long}, so purlis \
         cannot tell what a sign-in as {} may do and gives the account to no chat. Wait until \
         it expires, then sign in again here.",
        account.host, account.login
    ))
}

/// What a flow that ended without a kept token tells the person to do: the wait if a code is
/// still live ([`waiting_said`]), else [`HELD_BACK`].
pub fn held_back(config_root: &Path, account: &ForgeAccount) -> String {
    waiting_said(config_root, account).unwrap_or_else(|| HELD_BACK.to_string())
}

/// **Records that `account`'s CI-permission code `code` stays approvable for `lasts`** (it was
/// started by [`begin`]'s caller). Cancelling the wait or quitting purlis does not end the code at
/// GitHub: approving it later can still widen the everyday grant, and a sign-in made meanwhile
/// would read clean scopes. Until every code has lapsed ([`dead`] says earlier that one has) a
/// sign-in records the account's scopes as not known. A second code never shortens the first's
/// record. If this cannot be written the flow must not be shown.
pub fn pending(
    _host: &HostScope,
    config_root: &Path,
    account: &ForgeAccount,
    code: &str,
    lasts: std::time::Duration,
) -> Result<(), String> {
    if account.kind != Kind::GitHub {
        return Ok(());
    }
    let until = now().saturating_add((lasts + FLOW_SLACK).as_secs());
    edit_flows(config_root, account, code, Some(until))
}

/// **Records that `account`'s CI-permission code `code` is dead** (GitHub said it expired or that
/// the person refused it, or it was used): it alone can be approved no more. Another code may
/// still be live, and then a sign-in is still not believed. The account's scopes stay unknown
/// until a retry reads them back or a new sign-in records them.
pub fn dead(
    _host: &HostScope,
    config_root: &Path,
    account: &ForgeAccount,
    code: &str,
) -> Result<(), String> {
    if account.kind != Kind::GitHub {
        return Ok(());
    }
    edit_flows(config_root, account, code, None)
}

/// Adds `code` lapsing at `until` to the record of codes, or (`None`) retires it, under the one
/// writing lock: lapsed codes are dropped and an empty record is removed. A record that is not an
/// object is **replaced only by a code just issued**; a retirement leaves it, so it keeps
/// counting as pending.
fn edit_flows(
    config_root: &Path,
    account: &ForgeAccount,
    code: &str,
    until: Option<u64>,
) -> Result<(), String> {
    let _one = WRITING.lock().unwrap_or_else(|e| e.into_inner());
    let mut file = read(config_root)?;
    let fields = file
        .accounts
        .iter_mut()
        .find(|value| account_of(value).is_some_and(|a| a.id == account.id))
        .and_then(Value::as_object_mut)
        .ok_or_else(|| "That account is not on this machine any more.".to_string())?;
    let mut codes = match fields.get(FLOW_FIELD) {
        None => Map::new(),
        Some(Value::Object(codes)) => codes.clone(),
        Some(_) if until.is_none() => return Ok(()),
        Some(_) => Map::new(),
    };
    match until {
        Some(until) => {
            let kept = codes.get(code).and_then(Value::as_u64).unwrap_or(0);
            codes.insert(code.to_string(), Value::from(until.max(kept)));
        }
        None => {
            codes.remove(code);
        }
    }
    let now = now();
    // An entry that is no time is kept: it cannot be shown to have lapsed.
    codes.retain(|_, until| until.as_u64().is_none_or(|until| until > now));
    if codes.is_empty() {
        fields.remove(FLOW_FIELD);
    } else {
        fields.insert(FLOW_FIELD.to_string(), Value::Object(codes));
    }
    write(config_root, file)
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
    code: &str,
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
    // The code was approved, so it can fold nothing more in: it alone is retired. Another code the
    // person started earlier may still be live ([`Outcome::KeptAccountHeld`]).
    dead(host, config_root, account, code)?;
    let held_back = held_back(config_root, account);
    let second = granted.access.clone();
    crate::applog::keep_out(&second);
    if second.expose_secret().is_empty() {
        return Err(format!(
            "{} gave no token, so purlis kept nothing. {held_back}",
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
            "The code was confirmed as {} on {}, not as {}, so purlis kept nothing. {held_back} \
             Confirm the code while signed in to GitHub as {}.",
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
            "{} did not allow {SCOPE}, so purlis kept nothing. {held_back} Allow it on the page GitHub shows.",
            forge.host
        ));
    }

    // Did GitHub keep the two tokens' scopes apart? Read the everyday token back.
    let everyday = token(keyring, account)?.ok_or_else(|| {
        format!(
            "The sign-in to {} as {} ended while GitHub's page was open, so purlis kept nothing. \
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

    // A clean read-back records what the everyday token holds again, so a retry after an unknown
    // outcome serves it once more, **unless another code is still live**: that one can fold
    // `workflow` in unseen, so the scopes stay unknown until it has lapsed.
    let recorded = write_scopes(config_root, account, Some(scopes.clone()), true)?;
    keyring
        .set(&service(account), ITEM, second.expose_secret())
        .map_err(|e| unechoed(e.message.to_string(), &second))?;
    Ok(if recorded.is_some() {
        Outcome::Kept
    } else {
        Outcome::KeptAccountHeld
    })
}

/// What a sign-in's `account` is told when its scopes are not known.
pub fn scopes_unknown_said(config_root: &Path, account: &ForgeAccount) -> String {
    // A code still approvable is the cause, and signing in again records unknown again: say wait.
    if let Some(wait) = waiting_said(config_root, account) {
        return wait;
    }
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
    write_scopes(config_root, account, Some(scopes), false)?
        .ok_or_else(|| "That account is not on this machine any more.".to_string())
}

/// `account` with its scopes recorded as not known.
pub(super) fn clear_scopes(
    config_root: &Path,
    account: &ForgeAccount,
) -> Result<ForgeAccount, String> {
    write_scopes(config_root, account, None, false)?
        .ok_or_else(|| "That account is not on this machine any more.".to_string())
}

fn write_scopes(
    config_root: &Path,
    account: &ForgeAccount,
    scopes: Option<Vec<String>>,
    unless_pending: bool,
) -> Result<Option<ForgeAccount>, String> {
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
    if unless_pending && flow_pending(fields) {
        return Ok(None);
    }
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
    Ok(Some(updated))
}

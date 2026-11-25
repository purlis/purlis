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
//! **One writer, across processes.** A refresh rotates the refresh token, so two refreshes of
//! one account at once would end its sign-in. Only the host's processes refresh (a [`Refresher`]
//! is made with the [`HostScope`]), one refresh at a time in a process ([`REFRESHING`]) **and one
//! at a time on the machine** (a lock file in the session host's folder, [`REFRESH_LOCK`]), so the
//! window and a session host never spend the same refresh token. The one that waits reads the item
//! afresh when it gets the lock, and finds the new pair. The git credential helper, which is another
//! process, never refreshes: the window renews the token before it starts the git that will ask
//! for it.
//!
//! **The new pair is written before the new token is used.** The forge has already spent the old
//! refresh token when it answers, so a pair that was not kept would end the sign-in at the next
//! renewal. The write to the keychain is tried again; if it still fails, both pairs stay in this
//! process's memory ([`PENDING`]), the write is tried first the next time the account is asked
//! for, **the spent refresh token is never sent**, and the new access token is not used until it
//! is kept. The old access token serves while it lasts.
//!
//! **Only the forge's word that the grant is over ends the sign-in**: `invalid_grant`, and GitHub's
//! `bad_refresh_token`, the refresh token revoked or expired ([`ENDS_IT`]). The account is marked
//! signed out, its keyring item is deleted, and the request that needed the token fails with a
//! sentence that names the account and where to sign in again. It is never retried under another
//! account or through the forge's CLI (ADR 0070 section 5).
//!
//! **Nothing else ends it**: not `invalid_client` or `unauthorized_client` (the application, not
//! the grant), not a 5xx, a rate limit, a proxy's refusal, OAuth's own "not now", and not a forge
//! that cannot be reached. The keyring item is kept. While the token still has time left it is
//! used as it is; once it has none the request fails saying the forge could not be reached. The
//! next try waits longer each time ([`backoff`]), and after [`NOTICE_AFTER`] failures in a row the
//! window is told once ([`Trouble::RenewalFailing`]).
//!
//! **A renewal uses the client id the account signed in through** ([`ForgeAccount::client_id`]),
//! never what the registrations file says now (ADR 0128 D1.3): a refresh token belongs to one
//! application. If the pin on the host is another application than the one the account was made
//! with, or is gone, the renewal **stops**, the item is kept and the window is told once
//! ([`Trouble::PinChanged`]); the token in hand is used while it lasts, and signing in again is
//! the person's press.
//!
//! **Only purlis's own grants are refreshed.** A pasted token does not expire through purlis,
//! and an imported CLI login is never refreshed with the CLI's registration (ADR 0077 §5).

use std::collections::HashMap;
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

/// The file name of the lock that makes a refresh one act across processes, in the session
/// host's folder of the config home (`machine::Lock`).
pub const REFRESH_LOCK: &str = "forge-refresh.lock";

/// The words in an OAuth error reply that say the **grant** is over: the refresh token was
/// revoked, expired or already spent (RFC 6749 section 5.2 `invalid_grant`; GitHub's
/// `bad_refresh_token`). No other reply ends a sign-in.
const ENDS_IT: [&str; 2] = ["invalid_grant", "bad_refresh_token"];

/// Failures in a row after which the window is told that renewal is not working.
pub const NOTICE_AFTER: u32 = 3;

/// The wait before the next try after `failures` in a row: 30 seconds, doubling, at most 15
/// minutes.
pub fn backoff(failures: u32) -> Duration {
    let doubled = 30u64.saturating_mul(1u64 << failures.saturating_sub(1).min(10));
    Duration::from_secs(doubled.min(15 * 60))
}

/// What the host is told with when a forge ended a sign-in.
type Told = Box<dyn Fn(&ForgeAccount) + Send + Sync>;

/// What the host is told of a sign-in that is not renewing, though the forge has not ended it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trouble {
    /// The application pinned for the host is not the one the account signed in through (or is
    /// no longer pinned): renewal has stopped, and the item is kept. The fix is the person's
    /// press, in the window.
    PinChanged,
    /// [`NOTICE_AFTER`] renewals in a row did not get an answer the sign-in could use.
    RenewalFailing { failures: u32 },
    /// GitHub added `workflow`, the permission to push CI files, to the account's everyday token
    /// (ADR 0128 D1.1): purlis stopped serving that token to any chat until the person signs in
    /// again without it.
    EverydayHoldsWorkflow,
    /// The forge never said what the account's everyday token may do (or a read-back after a second
    /// grant could not): it may hold `workflow`, so purlis serves it to no chat until a check
    /// succeeds. Signing in again makes one.
    ScopesUnknown,
    /// GitHub answered 401 to the second token, the one that allows CI files: it is dead (GitHub
    /// revokes the oldest of more than 10 tokens a user holds for one app and scope set). purlis
    /// dropped it, so no CI-file push is offered until the person allows CI files again.
    SecondSignInRefused,
}

type TroubleTold = Box<dyn Fn(&ForgeAccount, &Trouble) + Send + Sync>;

static TROUBLE: OnceLock<TroubleTold> = OnceLock::new();

/// From now on `told` is called, once per kind of trouble per account (until it clears), with the
/// account: the app draws a Notice with the fix. Set once, by the host, as it starts.
pub fn tell_of_trouble(
    _host: &HostScope,
    told: impl Fn(&ForgeAccount, &Trouble) + Send + Sync + 'static,
) {
    let _ = TROUBLE.set(Box::new(told));
}

/// How one account's renewals are going, in this process.
#[derive(Default)]
struct Going {
    failures: u32,
    /// Not before this (seconds since the epoch).
    retry_at: u64,
    told_failing: bool,
    told_pin: bool,
}

static GOING: Mutex<Option<HashMap<String, Going>>> = Mutex::new(None);

fn going<R>(id: &str, with: impl FnOnce(&mut Going) -> R) -> R {
    let mut all = GOING.lock().unwrap_or_else(|e| e.into_inner());
    with(
        all.get_or_insert_with(HashMap::new)
            .entry(id.to_string())
            .or_default(),
    )
}

/// A renewed pair the keychain would not take: what the forge granted, and the refresh token it
/// spent. Held until it is written.
struct Pending {
    granted: super::oauth::Granted,
    spent: Option<SecretString>,
    at: u64,
}

static PENDING: Mutex<Option<HashMap<String, Pending>>> = Mutex::new(None);

fn pending<R>(with: impl FnOnce(&mut HashMap<String, Pending>) -> R) -> R {
    let mut all = PENDING.lock().unwrap_or_else(|e| e.into_inner());
    with(all.get_or_insert_with(HashMap::new))
}

fn copy_of(granted: &super::oauth::Granted) -> super::oauth::Granted {
    super::oauth::Granted {
        access: granted.access.clone(),
        refresh: granted.refresh.clone(),
        expires_in: granted.expires_in,
        scopes: granted.scopes.clone(),
    }
}

fn raise(account: &ForgeAccount, trouble: &Trouble) {
    if let Some(told) = TROUBLE.get() {
        told(account, trouble);
    }
}

/// [`raise`] for the troubles that a request, not a renewal, finds: told once per account and kind
/// of trouble in this process, so a chat that asks again and again does not tell the window
/// again. The Notice itself is derived from the account list, so it survives a restart.
pub(crate) fn raise_once(account: &ForgeAccount, trouble: &Trouble) {
    let key = once_key(&account.id, trouble);
    let first = {
        let mut told = ONCE.lock().unwrap_or_else(|e| e.into_inner());
        told.get_or_insert_with(Default::default).insert(key)
    };
    if first {
        raise(account, trouble);
    }
}

static ONCE: Mutex<Option<std::collections::HashSet<String>>> = Mutex::new(None);

/// What [`raise_once`] remembers a trouble of an account by.
fn once_key(account_id: &str, trouble: &Trouble) -> String {
    format!("{account_id}:{trouble:?}")
}

/// That `trouble` was told of `account_id` is forgotten, and nothing else: the next one of that
/// kind is told again, and every other kind already told stays told.
pub(crate) fn forget_raised_of(account_id: &str, trouble: &Trouble) {
    if let Some(told) = ONCE.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
        told.remove(&once_key(account_id, trouble));
    }
}

/// What was told of `account_id` is forgotten, so the next trouble of any kind is told again: the
/// account's scopes were just read afresh, and what it was told before is no longer so. Without
/// this a second fold in one run of the app would never raise a Notice.
pub(crate) fn forget_raised(account_id: &str) {
    let prefix = format!("{account_id}:");
    if let Some(told) = ONCE.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
        told.retain(|key| !key.starts_with(&prefix));
    }
}

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
        self.token_ahead(account, now, AHEAD)
    }

    /// [`Refresher::token_at`], renewing a token with less than `ahead` left: the renewal sweep
    /// ([`super::credential::renewal`]) asks for a longer lead than a request does, so a token
    /// is renewed while nobody is waiting on it.
    pub(super) fn token_ahead(
        &self,
        account: &ForgeAccount,
        now: u64,
        ahead: Duration,
    ) -> Result<SecretString, ForgeError> {
        let keyring = accounts::keyring_at(&self.config_root);
        let kept = self.read(keyring.as_ref(), account)?;
        if !due(account, &kept, now, ahead) && !pending(|all| all.contains_key(&account.id)) {
            return Ok(kept.access);
        }
        let _one = REFRESHING.lock().unwrap_or_else(|e| e.into_inner());
        // And one at a time on the machine: the window and a session host are two processes.
        let _across = crate::machine::Lock::named(&self.config_root, REFRESH_LOCK);
        // Fail closed: a refresh rotates the refresh token, so a renewal that cannot be made
        // one at a time on the machine is not made. The token in hand serves while it lasts. Off
        // unix there is no machine lock to take (`machine::supported` refuses the store, ADR
        // 0031), so a renewal there is one at a time in its process only.
        if cfg!(unix) && !_across.held() {
            let kept = self.read(keyring.as_ref(), account)?;
            return match kept.expires_at {
                Some(at) if at <= now => Err(ForgeError::transport(format!(
                    "purlis could not take the lock that makes a renewal one act on this machine, \
                     so it did not renew its sign-in to {} as {}. Try again.",
                    account.host, account.login
                ))),
                _ => Ok(kept.access),
            };
        }
        // A renewed pair another process could not keep is not to be replayed over: the refresh
        // token in the keychain is spent.
        if let Some(why) = self.held_elsewhere(account) {
            let kept = self.read(keyring.as_ref(), account)?;
            return match kept.expires_at {
                Some(at) if at <= now => Err(ForgeError::transport(why)),
                _ => Ok(kept.access),
            };
        }
        // A renewal an earlier call could not keep is kept first, before anything is sent.
        if let Err(why) = self.settle(keyring.as_ref(), account) {
            tracing::warn!("purlis: a renewed sign-in is still not in the keychain: {why}");
            let kept = self.read(keyring.as_ref(), account)?;
            return match kept.expires_at {
                Some(at) if at <= now => Err(ForgeError::transport(why)),
                _ => Ok(kept.access),
            };
        }
        // Another request may have renewed it while this one waited its turn, or another
        // process (the session host, the window) may have: read afresh, past this process's
        // memory, so a spent refresh token is never sent (#2656).
        let kept = self.read_afresh(keyring.as_ref(), account)?;
        if !due(account, &kept, now, ahead) {
            return Ok(kept.access);
        }
        let expired = kept.expires_at.is_some_and(|at| at <= now);
        let Some(spent) = kept.refresh.as_ref() else {
            // Nothing to renew it with. It is used while it lasts.
            if expired {
                return Err(self.ended(keyring.as_ref(), account));
            }
            return Ok(kept.access);
        };
        // The client id the account signed in through, which the pin on the host must still say.
        let (client_id, registration) = match self.client_id_of(account) {
            Ok(found) => found,
            Err(Pin::Unreadable(why)) if !expired => {
                tracing::warn!("purlis: a sign-in could not be renewed yet: {why}");
                return Ok(kept.access);
            }
            Err(Pin::Unreadable(why)) => return Err(ForgeError::transport(why)),
            Err(Pin::Changed) => {
                if !going(&account.id, |g| std::mem::replace(&mut g.told_pin, true)) {
                    raise(account, &Trouble::PinChanged);
                }
                return if expired {
                    Err(ForgeError::of(Failure::Auth, pin_changed_said(account)))
                } else {
                    Ok(kept.access)
                };
            }
        };
        // The pin says the application the account signed in through again: told afresh if it
        // moves again.
        going(&account.id, |g| g.told_pin = false);
        // A host that answered with a failure waits its turn: not before the backoff says.
        let wait = going(&account.id, |g| g.retry_at);
        if now < wait {
            return if expired {
                Err(self.could_not_reach(account, "it is not being asked again yet"))
            } else {
                Ok(kept.access)
            };
        }
        let at = self
            .at
            .clone()
            .unwrap_or_else(|| Endpoints::of(registration));
        let endpoint = self.endpoint.get_or_init(Https::shared);
        // Marked before the refresh token is sent, not after: a renewal that cannot say it is
        // under way is not made (fail closed), so no process is left free to send a token the
        // forge has spent. The token in hand serves while it lasts.
        if let Err(why) = self.mark_held(account) {
            tracing::warn!("purlis: {why}");
            return match kept.expires_at {
                Some(at) if at <= now => Err(ForgeError::transport(format!(
                    "purlis could not record that it is renewing its sign-in to {} as {}, so it \
                     did not renew it. Try again.",
                    account.host, account.login
                ))),
                _ => Ok(kept.access),
            };
        }
        match renewed(endpoint.as_ref(), &at, &client_id, spent) {
            Renewed::Granted(granted) => {
                going(&account.id, |g| *g = Going::default());
                // Kept before it is used: the forge has spent the old refresh token.
                match self.keep_durably(keyring.as_ref(), account, &granted, Some(spent), now) {
                    Ok(()) => {
                        self.unmark_held(account);
                        Ok(granted.access)
                    }
                    Err(why) => {
                        // The marker written before the send stays: the pair is held here.
                        pending(|all| {
                            all.insert(
                                account.id.clone(),
                                Pending {
                                    granted: copy_of(&granted),
                                    spent: Some(spent.clone()),
                                    at: now,
                                },
                            )
                        });
                        tracing::warn!("purlis: {why}");
                        match kept.expires_at {
                            Some(at) if at <= now => Err(ForgeError::transport(why)),
                            _ => Ok(kept.access),
                        }
                    }
                }
            }
            Renewed::Refused => {
                pending(|all| all.remove(&account.id));
                self.unmark_held(account);
                Err(self.ended(keyring.as_ref(), account))
            }
            Renewed::Unreachable(why) => {
                self.unmark_held(account);
                let failures = going(&account.id, |g| {
                    g.failures += 1;
                    g.retry_at = now.saturating_add(backoff(g.failures).as_secs());
                    g.failures
                });
                if failures >= NOTICE_AFTER
                    && !going(&account.id, |g| {
                        std::mem::replace(&mut g.told_failing, true)
                    })
                {
                    raise(account, &Trouble::RenewalFailing { failures });
                }
                if expired {
                    Err(self.could_not_reach(account, &why))
                } else {
                    Ok(kept.access)
                }
            }
        }
    }

    /// The client id `account` is renewed with: the one it signed in through, which the pin on
    /// its host must still say. A compiled-in host's account made before the id was kept is
    /// renewed with purlis's own registration.
    fn client_id_of(&self, account: &ForgeAccount) -> Result<(String, &'static Registration), Pin> {
        let current = registration::resolve(&self.config_root, account.kind, &account.host)
            .map_err(Pin::Unreadable)?;
        match (current, account.client_id.as_deref()) {
            (Some(now), Some(pinned)) if now.client_id == pinned => Ok((pinned.to_string(), now)),
            (Some(now), None) if registration::on(account.kind, &account.host).is_some() => {
                Ok((now.client_id.to_string(), now))
            }
            _ => Err(Pin::Changed),
        }
    }

    /// Writes a renewed pair to the account's keyring item, trying a few times.
    fn keep_durably(
        &self,
        keyring: &dyn crate::secrets::keyring::Store,
        account: &ForgeAccount,
        granted: &super::oauth::Granted,
        spent: Option<&SecretString>,
        now: u64,
    ) -> Result<(), String> {
        let mut last = String::new();
        for _ in 0..WRITE_TRIES {
            match accounts::keep_refreshed(&self.scope, keyring, account, granted, spent, now) {
                // Superseded is settled too: a newer item stands, and this pair is dropped.
                Ok(_) => return Ok(()),
                Err(why) => last = why,
            }
        }
        Err(last)
    }

    /// Writes the pair an earlier renewal could not keep, if there is one.
    fn settle(
        &self,
        keyring: &dyn crate::secrets::keyring::Store,
        account: &ForgeAccount,
    ) -> Result<(), String> {
        let Some(held) = pending(|all| {
            all.get(&account.id)
                .map(|p| (copy_of(&p.granted), p.spent.clone(), p.at))
        }) else {
            return Ok(());
        };
        self.keep_durably(keyring, account, &held.0, held.1.as_ref(), held.2)?;
        pending(|all| all.remove(&account.id));
        self.unmark_held(account);
        Ok(())
    }

    /// The marker file of `account`'s unkept pair, in the lock folder (hex of the id: any id is a
    /// safe file name).
    fn marker(&self, account: &ForgeAccount) -> Option<PathBuf> {
        marker_of(&self.config_root, &account.id)
    }

    /// Says, under the machine lock, that this process is renewing `account` and so may hold a
    /// renewed pair it could not write: the keychain still has the refresh token the forge spends.
    /// The file holds the account id and this process's id; its age is the file's. An error means
    /// no marker stands, and the renewal is not made.
    fn mark_held(&self, account: &ForgeAccount) -> Result<(), String> {
        let Some(path) = self.marker(account) else {
            // Off unix purlis keeps no machine store, so there is no lock folder and no second
            // process to mark it for ([`crate::machine::supported`]).
            return match crate::machine::supported() {
                Ok(()) => Err("the folder for the renewal marker could not be made".to_string()),
                Err(_) => Ok(()),
            };
        };
        let text = format!("{}\n{}\n", account.id, std::process::id());
        std::fs::write(&path, text)
            .map_err(|why| format!("could not mark a renewal as under way: {why}"))
    }

    fn unmark_held(&self, account: &ForgeAccount) {
        if let Some(path) = self.marker(account) {
            let _ = std::fs::remove_file(path);
        }
    }

    /// Why this process must not renew `account`: another live process holds a renewed pair it
    /// has not yet kept. A marker of a process that is gone, or older than [`HELD_FOR`], is
    /// removed: its pair is lost, and the forge's own answer to the spent token is what ends the
    /// sign-in. **A marker that cannot be read, or says nothing sensible, holds** until it ages
    /// out: the doubt closes the door.
    fn held_elsewhere(&self, account: &ForgeAccount) -> Option<String> {
        let path = self.marker(account)?;
        let held = || {
            Some(format!(
                "another purlis process holds a renewed sign-in to {} as {} that it has not yet \
                 kept in the keychain, so this one did not renew it. Try again shortly.",
                account.host, account.login
            ))
        };
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
            Err(_) => return self.hold_or_age_out(&path, held),
        };
        let mut lines = text.lines();
        let said = lines
            .next()
            .zip(lines.next().and_then(|l| l.trim().parse::<u32>().ok()));
        let Some((_, pid)) = said.filter(|(id, _)| *id == account.id) else {
            return self.hold_or_age_out(&path, held);
        };
        if pid == std::process::id() {
            return None;
        }
        if aged(&path) || !crate::process::alive(pid) {
            let _ = std::fs::remove_file(path);
            return None;
        }
        held()
    }

    /// A marker nobody can read the writer of: it holds until it is older than [`HELD_FOR`].
    fn hold_or_age_out(
        &self,
        path: &Path,
        held: impl FnOnce() -> Option<String>,
    ) -> Option<String> {
        if aged(path) {
            let _ = std::fs::remove_file(path);
            return None;
        }
        held()
    }

    fn could_not_reach(&self, account: &ForgeAccount, why: &str) -> ForgeError {
        ForgeError::transport(format!(
            "purlis could not reach {} to renew its sign-in as {}: {why}. Check your \
             connection and try again.",
            account.host, account.login
        ))
    }

    fn read(
        &self,
        keyring: &dyn crate::secrets::keyring::Store,
        account: &ForgeAccount,
    ) -> Result<Kept, ForgeError> {
        Self::found(account, accounts::kept(keyring, account))
    }

    fn read_afresh(
        &self,
        keyring: &dyn crate::secrets::keyring::Store,
        account: &ForgeAccount,
    ) -> Result<Kept, ForgeError> {
        Self::found(account, accounts::kept_afresh(keyring, account))
    }

    /// The forge refused `account`'s token: this process forgets it, so the next request reads
    /// the item again rather than sending what was refused (#2656). The window may have
    /// replaced it, or signed in again.
    pub fn refused(&self, account: &ForgeAccount) {
        accounts::forget(accounts::keyring_at(&self.config_root).as_ref(), account);
    }

    fn found(
        account: &ForgeAccount,
        read: Result<Option<Kept>, String>,
    ) -> Result<Kept, ForgeError> {
        match read {
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

/// Why a renewal cannot say which application to use.
enum Pin {
    /// The pins could not be read.
    Unreadable(String),
    /// The host's pin is not the application the account signed in through, or there is none.
    Changed,
}

/// The marker file of the account `id` under `config_root`.
fn marker_of(config_root: &Path, id: &str) -> Option<PathBuf> {
    let hex: String = id.bytes().map(|b| format!("{b:02x}")).collect();
    let dir = crate::machine::locks_dir(config_root).ok()?;
    Some(dir.join(format!("forge-refresh-held-{hex}")))
}

/// Whether the marker at `path` is older than [`HELD_FOR`], by the file's own modified time. One
/// whose age cannot be read is not aged.
fn aged(path: &Path) -> bool {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|at| at.elapsed().ok())
        .is_some_and(|age| age > HELD_FOR)
}

/// Clears the marker of the account `id`: a fresh sign-in (or a sign-out) makes whatever pair an
/// earlier renewal held beside the point. The caller holds [`REFRESH_LOCK`].
pub(super) fn clear_held(config_root: &Path, id: &str) {
    if let Some(path) = marker_of(config_root, id) {
        let _ = std::fs::remove_file(path);
    }
}

/// The tries of the keychain write for one renewed pair, before the pair is held for the next
/// call.
const WRITE_TRIES: u32 = 3;

/// How long a held-pair marker is believed: a marker older than this is cleared whoever wrote it.
/// A live process that is not the writer (a reused pid) or an idle session host would otherwise
/// block renewal for good. The forge's own answer to the spent token then ends the sign-in, which
/// is the same end a lost pair comes to.
const HELD_FOR: Duration = Duration::from_secs(30 * 60);

/// What the window says when renewal stopped because the host's application is not the one
/// `account` signed in through.
pub fn pin_changed_said(account: &ForgeAccount) -> String {
    format!(
        "purlis stopped renewing its sign-in to {} as {}: the application pinned for {} is not the \
         one it signed in through. Check the application in {SIGN_IN_AT}, then sign in again.",
        account.host, account.login, account.host
    )
}

/// What the window says when the forge ended `account`'s sign-in.
pub fn ended_said(account: &ForgeAccount) -> String {
    format!(
        "purlis's sign-in to {} as {} has ended, and {} would not renew it. Sign in again in \
         {SIGN_IN_AT}.",
        account.host, account.login, account.host
    )
}

/// Whether `account`'s token is to be renewed before it is used at `now`, with `ahead` to spare.
fn due(account: &ForgeAccount, kept: &Kept, now: u64, ahead: Duration) -> bool {
    matches!(account.method, Method::Device | Method::Pkce)
        && kept
            .expires_at
            .is_some_and(|at| at <= now.saturating_add(ahead.as_secs()))
}

/// How a refresh went.
enum Renewed {
    Granted(super::oauth::Granted),
    /// The forge said the grant is over ([`ENDS_IT`]): the sign-in has ended.
    Refused,
    /// Anything else that was not a grant: nothing is known about the sign-in, and ending it
    /// cannot be undone. The sentence says what was seen.
    Unreachable(String),
}

/// `spent` traded for a new pair at `at`, with the public client id the account signed in
/// through and no secret.
fn renewed(
    endpoint: &dyn Endpoint,
    at: &Endpoints,
    client_id: &str,
    spent: &SecretString,
) -> Renewed {
    let answer = match endpoint.post(
        &at.token,
        &[
            ("client_id", client_id),
            ("grant_type", "refresh_token"),
            ("refresh_token", spent.expose_secret()),
        ],
    ) {
        Ok(answer) => answer,
        Err(why) => return Renewed::Unreachable(why),
    };
    let status = answer.status;
    match answer.error() {
        Some(word) if ENDS_IT.contains(&word) => return Renewed::Refused,
        // `invalid_client`, `unauthorized_client`, `temporarily_unavailable`, a rate limit, a
        // proxy, a 5xx: the forge's word is not about the grant, so the sign-in stands.
        Some(word) => return Renewed::Unreachable(format!("it answered {word} (HTTP {status})")),
        None if status >= 400 => return Renewed::Unreachable(format!("it answered HTTP {status}")),
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
    use crate::forge::registered;
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

    /// The client id a sign-in on `host` is made through: purlis's own where it has a registration
    /// there, else the one a test pins for a host of its own ([`registered::pin`]).
    pub(crate) fn client_id_of_the_application_on(kind: Kind, host: &str) -> &'static str {
        match (registration::on(kind, host), kind) {
            (Some(own), _) => own.client_id,
            (None, Kind::GitHub) => "Iv1.ghes-client",
            (None, Kind::GitLab) => "gl-group-app",
        }
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
            client_id_of_the_application_on(kind, host),
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
    fn a_renewal_another_process_made_is_read_afresh_and_its_spent_refresh_token_never_sent() {
        // This process remembers the first pair (#2656). Another process of the app (the
        // session host, or the window) renews it first and writes the second pair to the item.
        use crate::secrets::keyring::Store as _;
        let machine = machine(Kind::GitHub, "github.com", Method::Device);
        let other_process = crate::secrets::keyring::FileStore::at(
            crate::machine::dir(machine.home.path()).join(crate::secrets::keyring::STUB_FILE),
        );
        let renewed = format!(
            r#"{{"access":"second-access-value","refresh":"second-refresh-value","expires_at":{}}}"#,
            machine.expires_at + LASTS
        );
        other_process
            .set(&accounts::service(&machine.account), "token", &renewed)
            .unwrap();
        let forge = Arc::new(StandIn::answering(&[]));
        let token = machine
            .refresher(&forge)
            .token_at(&machine.account, machine.expires_at - AHEAD.as_secs())
            .unwrap();
        assert_eq!(token.expose_secret(), "second-access-value");
        assert!(
            forge.posted.lock().unwrap().is_empty(),
            "the first refresh token, spent by the other process, is never sent"
        );
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
            (
                401,
                r#"{"error": "invalid_grant", "error_description": "revoked"}"#,
            ),
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
        for later in [0, backoff(1).as_secs()] {
            let token = refresher
                .token_at(&machine.account, machine.expires_at - 60 + later)
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
            // The application, not the grant: a wrong or retired client id, or one that may not
            // use the flow. Ending the sign-in would punish the person for a setting of the owner's.
            (401, r#"{"error": "invalid_client"}"#),
            (400, r#"{"error": "unauthorized_client"}"#),
            (200, r#"{"error": "incorrect_client_credentials"}"#),
            // A bare 400 or 401 carries no word about the grant.
            (400, ""),
            (401, ""),
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

    /// The keyring stub's file, which a test makes unwritable to make the keychain refuse.
    fn the_keychain_refuses(machine: &Machine, refuses: bool) {
        use std::os::unix::fs::PermissionsExt;
        let stub =
            crate::machine::dir(machine.home.path()).join(crate::secrets::keyring::STUB_FILE);
        let dir = stub.parent().unwrap();
        let (file_mode, dir_mode) = if refuses {
            (0o400, 0o500)
        } else {
            (0o600, 0o700)
        };
        if !refuses {
            std::fs::set_permissions(dir, std::fs::Permissions::from_mode(dir_mode)).unwrap();
        }
        std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(file_mode)).unwrap();
        if refuses {
            std::fs::set_permissions(dir, std::fs::Permissions::from_mode(dir_mode)).unwrap();
        }
    }

    #[test]
    fn a_renewed_pair_the_keychain_would_not_take_is_kept_in_memory_and_the_spent_token_never_sent()
    {
        let machine = machine(Kind::GitHub, "github.com", Method::Device);
        let forge = Arc::new(StandIn::answering(&[Ok((200, RENEWED))]));
        let refresher = machine.refresher(&forge);
        let now = machine.expires_at - AHEAD.as_secs();
        the_keychain_refuses(&machine, true);
        // The forge renewed, and the keychain refused the write: the new access token is not
        // used before it is kept. The old one has time left, so it serves.
        let token = refresher.token_at(&machine.account, now).unwrap();
        assert_eq!(token.expose_secret(), "first-access-value");
        assert_eq!(forge.posted.lock().unwrap().len(), 1);
        // The next request does not post the refresh token the forge has spent.
        let token = refresher.token_at(&machine.account, now + 10).unwrap();
        assert_eq!(token.expose_secret(), "first-access-value");
        assert_eq!(
            forge.posted.lock().unwrap().len(),
            1,
            "the spent token was sent"
        );
        // With the old token spent too, the request fails and says why; nothing is signed out.
        let why = refresher
            .token_at(&machine.account, machine.expires_at + 1)
            .unwrap_err();
        assert_eq!(why.failure(), &Failure::Transport);
        assert_eq!(forge.posted.lock().unwrap().len(), 1);
        // The keychain takes it again: the pair held is written first, and then used.
        the_keychain_refuses(&machine, false);
        let token = refresher
            .token_at(&machine.account, machine.expires_at + 2)
            .unwrap();
        assert_eq!(token.expose_secret(), "second-access-value");
        assert_eq!(forge.posted.lock().unwrap().len(), 1, "it renewed again");
        let kept = machine.kept().unwrap();
        assert_eq!(kept.access.expose_secret(), "second-access-value");
        assert_eq!(
            kept.refresh.as_ref().map(|r| r.expose_secret()),
            Some("second-refresh-value")
        );
        assert_eq!(machine.listed().state, State::SignedIn);
    }

    #[test]
    fn requests_at_once_renew_once_and_all_get_the_new_token() {
        let machine = machine(Kind::GitHub, "github.com", Method::Device);
        let forge = Arc::new(StandIn::answering(&[Ok((200, RENEWED))]));
        let now = machine.expires_at - AHEAD.as_secs();
        let tokens: Vec<String> = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..6)
                .map(|_| {
                    scope.spawn(|| {
                        // Each is a refresher of its own, as the window's and the session host's are.
                        let refresher = machine.refresher(&forge);
                        refresher
                            .token_at(&machine.account, now)
                            .unwrap()
                            .expose_secret()
                            .to_string()
                    })
                })
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        });
        assert!(
            tokens.iter().all(|t| t == "second-access-value"),
            "{tokens:?}"
        );
        assert_eq!(forge.posted.lock().unwrap().len(), 1);
    }

    #[test]
    fn a_renewal_waits_for_the_machine_wide_lock_another_process_holds() {
        let machine = machine(Kind::GitHub, "github.com", Method::Device);
        let forge = Arc::new(StandIn::answering(&[Ok((200, RENEWED))]));
        let now = machine.expires_at - AHEAD.as_secs();
        // Another process of the app is in the middle of a renewal: it holds the lock file.
        let theirs = crate::machine::Lock::named(machine.home.path(), REFRESH_LOCK);
        assert!(theirs.held());
        std::thread::scope(|scope| {
            let renewing = scope.spawn(|| {
                machine
                    .refresher(&forge)
                    .token_at(&machine.account, now)
                    .unwrap()
            });
            std::thread::sleep(Duration::from_millis(300));
            assert!(
                forge.posted.lock().unwrap().is_empty(),
                "a renewal went ahead while another process held the lock"
            );
            drop(theirs);
            assert_eq!(
                renewing.join().unwrap().expose_secret(),
                "second-access-value"
            );
        });
        assert_eq!(forge.posted.lock().unwrap().len(), 1);
    }

    #[test]
    fn a_renewal_that_fails_waits_longer_each_time_and_the_window_is_told_after_three() {
        assert_eq!(backoff(1), Duration::from_secs(30));
        assert_eq!(backoff(2), Duration::from_secs(60));
        assert_eq!(backoff(3), Duration::from_secs(120));
        assert_eq!(backoff(40), Duration::from_secs(15 * 60));
        let machine = machine(Kind::GitHub, "github.com", Method::Device);
        let forge = Arc::new(StandIn::answering(&[
            Ok((503, "")),
            Ok((503, "")),
            Ok((503, "")),
            Ok((503, "")),
        ]));
        let refresher = machine.refresher(&forge);
        let id = machine.account.id.clone();
        let mut at = machine.expires_at - 290;
        for round in 1..=3u32 {
            refresher.token_at(&machine.account, at).unwrap();
            assert_eq!(forge.posted.lock().unwrap().len(), round as usize);
            assert_eq!(going(&id, |g| g.told_failing), round >= NOTICE_AFTER);
            // Inside the wait it is not asked again.
            refresher.token_at(&machine.account, at + 1).unwrap();
            assert_eq!(forge.posted.lock().unwrap().len(), round as usize);
            at += backoff(round).as_secs();
        }
        // The item is kept through all of it.
        assert_eq!(
            machine.kept().unwrap().refresh.unwrap().expose_secret(),
            "first-refresh-value"
        );
        assert_eq!(machine.listed().state, State::SignedIn);
    }

    #[test]
    fn a_renewal_uses_the_client_id_the_account_signed_in_through_and_stops_when_the_pin_moves() {
        let machine = machine(Kind::GitHub, "ghe.example.com", Method::Device);
        assert_eq!(
            machine.account.client_id.as_deref(),
            Some("Iv1.ghes-client")
        );
        let pin = |id: &str| {
            registered::pin(
                &HostScope::for_a_test(),
                machine.home.path(),
                Kind::GitHub,
                "ghe.example.com",
                id,
            )
            .unwrap()
        };
        pin("Iv1.ghes-client");
        let forge = Arc::new(StandIn::answering(&[Ok((200, RENEWED))]));
        let refresher = Refresher::through(
            &HostScope::for_a_test(),
            machine.home.path(),
            forge.clone(),
            Endpoints::of(
                registered::pinned(machine.home.path(), Kind::GitHub, "ghe.example.com")
                    .unwrap()
                    .unwrap(),
            ),
        );
        // Another application is pinned for the host (someone pressed Use this application, or
        // the file was replaced): the renewal sends the refresh token to nobody.
        pin("Iv1.another-application");
        let token = refresher
            .token_at(&machine.account, machine.expires_at - 60)
            .unwrap();
        assert_eq!(token.expose_secret(), "first-access-value");
        let why = refresher
            .token_at(&machine.account, machine.expires_at + 1)
            .unwrap_err();
        assert_eq!(why.failure(), &Failure::Auth);
        assert!(why.said().contains("stopped renewing"), "{why}");
        assert!(forge.posted.lock().unwrap().is_empty());
        assert!(going(&machine.account.id, |g| g.told_pin));
        // The item is kept, and the account is still listed as signed in.
        assert!(machine.kept().unwrap().refresh.is_some());
        assert_eq!(machine.listed().state, State::SignedIn);
        // Nothing pinned at all is the same.
        registered::unpin(
            &HostScope::for_a_test(),
            machine.home.path(),
            Kind::GitHub,
            "ghe.example.com",
        )
        .unwrap();
        assert!(
            refresher
                .token_at(&machine.account, machine.expires_at + 1)
                .is_err()
        );
        assert!(forge.posted.lock().unwrap().is_empty());
        // Pinned back to the application the account was made with, it renews with that id.
        pin("Iv1.ghes-client");
        let token = refresher
            .token_at(&machine.account, machine.expires_at + 1)
            .unwrap();
        assert_eq!(token.expose_secret(), "second-access-value");
        assert_eq!(
            forge.posted.lock().unwrap()[0].1[0],
            ("client_id".to_string(), "Iv1.ghes-client".to_string())
        );
    }

    #[test]
    fn a_pin_that_is_settled_again_is_told_afresh_when_it_moves_again() {
        let machine = machine(Kind::GitHub, "ghe.example.com", Method::Device);
        let pin = |id: &str| {
            registered::pin(
                &HostScope::for_a_test(),
                machine.home.path(),
                Kind::GitHub,
                "ghe.example.com",
                id,
            )
            .unwrap()
        };
        pin("Iv1.ghes-client");
        let forge = Arc::new(StandIn::answering(&[Ok((200, RENEWED))]));
        let refresher = Refresher::through(
            &HostScope::for_a_test(),
            machine.home.path(),
            forge.clone(),
            Endpoints::of(
                registered::pinned(machine.home.path(), Kind::GitHub, "ghe.example.com")
                    .unwrap()
                    .unwrap(),
            ),
        );
        let id = machine.account.id.clone();
        pin("Iv1.another-application");
        refresher
            .token_at(&machine.account, machine.expires_at - 60)
            .unwrap();
        assert!(going(&id, |g| g.told_pin));
        // Pinned back: the pin is settled, and the flag is cleared.
        pin("Iv1.ghes-client");
        refresher
            .token_at(&machine.account, machine.expires_at - 60)
            .unwrap();
        assert!(!going(&id, |g| g.told_pin), "told_pin outlived the pin");
        // Moved again: told again. (The renewal above made a new token, so the time is its.)
        pin("Iv1.another-application");
        refresher
            .token_at(&machine.account, machine.expires_at + 28_700)
            .unwrap();
        assert!(going(&id, |g| g.told_pin));
    }

    #[cfg(unix)]
    #[test]
    fn a_renewal_that_cannot_take_the_machine_lock_is_not_made() {
        let machine = machine(Kind::GitHub, "github.com", Method::Device);
        let forge = Arc::new(StandIn::answering(&[Ok((200, RENEWED))]));
        let refresher = machine.refresher(&forge);
        // A directory where the lock file goes: it cannot be opened as a file.
        let dir = crate::machine::locks_dir(machine.home.path()).unwrap();
        // (The sign-in that made the account has already made the file.)
        let _ = std::fs::remove_file(dir.join(REFRESH_LOCK));
        std::fs::create_dir(dir.join(REFRESH_LOCK)).unwrap();
        let now = machine.expires_at - AHEAD.as_secs();
        // The token in hand has time left: it serves, and nothing is sent.
        let token = refresher.token_at(&machine.account, now).unwrap();
        assert_eq!(token.expose_secret(), "first-access-value");
        // With none left the request fails and says so, and still nothing is sent.
        let why = refresher
            .token_at(&machine.account, machine.expires_at + 1)
            .unwrap_err();
        assert_eq!(why.failure(), &Failure::Transport);
        assert!(why.said().contains("lock"), "{why}");
        assert!(forge.posted.lock().unwrap().is_empty());
        assert_eq!(
            machine.kept().unwrap().refresh.unwrap().expose_secret(),
            "first-refresh-value"
        );
    }

    fn keep(machine: &Machine, was: Option<&str>) -> Result<accounts::KeepOutcome, String> {
        let granted = Answer::of(200, RENEWED).granted().unwrap();
        let was = was.map(SecretString::from);
        accounts::keep_refreshed(
            &HostScope::for_a_test(),
            accounts::keyring_at(machine.home.path()).as_ref(),
            &machine.account,
            &granted,
            was.as_ref(),
            accounts::now(),
        )
    }

    #[test]
    fn a_pair_is_written_only_over_the_refresh_token_it_spent() {
        let machine = machine(Kind::GitHub, "github.com", Method::Device);
        // A stale pair (it spent a token the item no longer holds) overwrites nothing.
        assert_eq!(
            keep(&machine, Some("an-older-refresh-value")),
            Ok(accounts::KeepOutcome::Superseded)
        );
        assert_eq!(
            machine.kept().unwrap().access.expose_secret(),
            "first-access-value"
        );
        // The one it was made from is written.
        assert_eq!(
            keep(&machine, Some("first-refresh-value")),
            Ok(accounts::KeepOutcome::Written)
        );
        assert_eq!(
            machine.kept().unwrap().access.expose_secret(),
            "second-access-value"
        );
    }

    #[test]
    fn a_pair_is_never_written_to_an_item_another_process_ended() {
        let machine = machine(Kind::GitHub, "github.com", Method::Device);
        let keyring = accounts::keyring_at(machine.home.path());
        accounts::signed_out_by_the_forge(
            &HostScope::for_a_test(),
            machine.home.path(),
            keyring.as_ref(),
            &machine.account.id,
        )
        .unwrap();
        assert_eq!(
            keep(&machine, Some("first-refresh-value")),
            Ok(accounts::KeepOutcome::Superseded)
        );
        assert!(machine.kept().is_none(), "the ended item was made again");
    }

    /// The marker a process that holds an unkept pair leaves, written by `pid`.
    fn held_by(machine: &Machine, refresher: &Refresher, pid: u32) {
        let path = refresher.marker(&machine.account).unwrap();
        std::fs::write(path, format!("{}\n{pid}\n", machine.account.id)).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn a_renewal_waits_for_a_live_process_that_holds_an_unkept_pair() {
        let machine = machine(Kind::GitHub, "github.com", Method::Device);
        let forge = Arc::new(StandIn::answering(&[Ok((200, RENEWED))]));
        let refresher = machine.refresher(&forge);
        // This test's parent is another live process.
        held_by(&machine, &refresher, std::os::unix::process::parent_id());
        let now = machine.expires_at - AHEAD.as_secs();
        let token = refresher.token_at(&machine.account, now).unwrap();
        assert_eq!(token.expose_secret(), "first-access-value");
        let why = refresher
            .token_at(&machine.account, machine.expires_at + 1)
            .unwrap_err();
        assert!(why.said().contains("another purlis process"), "{why}");
        assert!(
            forge.posted.lock().unwrap().is_empty(),
            "the spent refresh token was replayed"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_marker_of_a_process_that_is_gone_is_cleared_and_the_renewal_goes_on() {
        let machine = machine(Kind::GitHub, "github.com", Method::Device);
        let forge = Arc::new(StandIn::answering(&[Ok((200, RENEWED))]));
        let refresher = machine.refresher(&forge);
        let mut command = std::process::Command::new("true");
        let mut child = crate::forklock::spawn(&mut command).unwrap();
        let gone = child.id();
        child.wait().unwrap();
        held_by(&machine, &refresher, gone);
        let token = refresher
            .token_at(&machine.account, machine.expires_at - AHEAD.as_secs())
            .unwrap();
        assert_eq!(token.expose_secret(), "second-access-value");
        assert!(!refresher.marker(&machine.account).unwrap().exists());
    }

    #[test]
    fn a_pair_the_keychain_would_not_take_leaves_a_marker_until_it_is_kept() {
        let machine = machine(Kind::GitHub, "github.com", Method::Device);
        let forge = Arc::new(StandIn::answering(&[Ok((200, RENEWED))]));
        let refresher = machine.refresher(&forge);
        let marker = refresher.marker(&machine.account).unwrap();
        let now = machine.expires_at - AHEAD.as_secs();
        the_keychain_refuses(&machine, true);
        refresher.token_at(&machine.account, now).unwrap();
        let text = std::fs::read_to_string(&marker).unwrap();
        assert_eq!(
            text,
            format!("{}\n{}\n", machine.account.id, std::process::id())
        );
        the_keychain_refuses(&machine, false);
        refresher.token_at(&machine.account, now + 10).unwrap();
        assert!(!marker.exists(), "the marker outlived the unkept pair");
    }

    /// Backdates `path`'s modified time past [`HELD_FOR`].
    fn aged_past_the_bound(path: &Path) {
        let long_ago = std::time::SystemTime::now() - HELD_FOR - Duration::from_secs(60);
        std::fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(long_ago)
            .unwrap();
    }

    /// The account signed in again through a browser sign-in, as the person's press does.
    fn sign_in_again(machine: &Machine) -> Result<ForgeAccount, String> {
        let granted = Answer::of(200, RENEWED).granted().unwrap();
        let reach = |_: &Forge,
                     _: Arc<dyn TokenSource>|
         -> Result<Arc<dyn Transport>, ForgeError> { Ok(Arc::new(WhoAmI)) };
        accounts::sign_in_with_grant(
            &HostScope::for_a_test(),
            machine.home.path(),
            accounts::keyring_at(machine.home.path()).as_ref(),
            &reach,
            machine.account.kind,
            &machine.account.host,
            granted,
            machine.account.method,
            client_id_of_the_application_on(machine.account.kind, &machine.account.host),
        )
    }

    #[cfg(unix)]
    #[test]
    fn a_marker_older_than_the_bound_is_cleared_though_its_process_is_alive() {
        let machine = machine(Kind::GitHub, "github.com", Method::Device);
        let forge = Arc::new(StandIn::answering(&[Ok((200, RENEWED))]));
        let refresher = machine.refresher(&forge);
        // This test's parent is live, and its marker is two minutes past the bound.
        held_by(&machine, &refresher, std::os::unix::process::parent_id());
        let marker = refresher.marker(&machine.account).unwrap();
        aged_past_the_bound(&marker);
        let token = refresher
            .token_at(&machine.account, machine.expires_at - AHEAD.as_secs())
            .unwrap();
        assert_eq!(token.expose_secret(), "second-access-value");
        assert!(!marker.exists());
    }

    #[cfg(unix)]
    #[test]
    fn a_marker_inside_the_bound_still_holds_a_live_process_to_its_word() {
        let machine = machine(Kind::GitHub, "github.com", Method::Device);
        let forge = Arc::new(StandIn::answering(&[Ok((200, RENEWED))]));
        let refresher = machine.refresher(&forge);
        held_by(&machine, &refresher, std::os::unix::process::parent_id());
        let marker = refresher.marker(&machine.account).unwrap();
        // Just inside the bound.
        let recent = std::time::SystemTime::now() - HELD_FOR + Duration::from_secs(120);
        std::fs::File::options()
            .write(true)
            .open(&marker)
            .unwrap()
            .set_modified(recent)
            .unwrap();
        assert!(
            refresher
                .token_at(&machine.account, machine.expires_at + 1)
                .is_err()
        );
        assert!(marker.exists());
        assert!(forge.posted.lock().unwrap().is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn a_fresh_sign_in_and_a_sign_out_clear_the_marker() {
        let machine = machine(Kind::GitHub, "github.com", Method::Device);
        let forge = Arc::new(StandIn::answering(&[]));
        let refresher = machine.refresher(&forge);
        let marker = refresher.marker(&machine.account).unwrap();
        held_by(&machine, &refresher, std::os::unix::process::parent_id());
        // The same account id comes back from a sign-in, and the marker of the old pair does not.
        let again = sign_in_again(&machine).unwrap();
        assert_eq!(again.id, machine.account.id);
        assert!(!marker.exists(), "a fresh sign-in left the marker");
        // And one signed out is not left holding anything either.
        held_by(&machine, &refresher, std::os::unix::process::parent_id());
        accounts::sign_out(
            &HostScope::for_a_test(),
            machine.home.path(),
            accounts::keyring_at(machine.home.path()).as_ref(),
            &machine.account.id,
        )
        .unwrap();
        assert!(!marker.exists(), "a sign-out left the marker");
    }

    #[test]
    fn a_marker_nobody_can_read_holds_until_it_ages_out() {
        let machine = machine(Kind::GitHub, "github.com", Method::Device);
        let forge = Arc::new(StandIn::answering(&[Ok((200, RENEWED))]));
        let refresher = machine.refresher(&forge);
        let marker = refresher.marker(&machine.account).unwrap();
        let expired = machine.expires_at + 1;
        let id = &machine.account.id;
        // Garbled: not an id and a process id, a pid that is not a number, another account's id,
        // and nothing at all.
        for text in [
            "garbage".to_string(),
            format!("{id}\nnot-a-pid\n"),
            "01J9SOMEONEELSE\n4242\n".to_string(),
            String::new(),
        ] {
            std::fs::write(&marker, &text).unwrap();
            let why = refresher.token_at(&machine.account, expired).unwrap_err();
            assert!(
                why.said().contains("another purlis process"),
                "{text:?}: {why}"
            );
            assert!(marker.exists(), "{text:?}");
        }
        // Unreadable: a directory where the file goes.
        std::fs::remove_file(&marker).unwrap();
        std::fs::create_dir(&marker).unwrap();
        let why = refresher.token_at(&machine.account, expired).unwrap_err();
        assert!(why.said().contains("another purlis process"), "{why}");
        std::fs::remove_dir(&marker).unwrap();
        assert!(forge.posted.lock().unwrap().is_empty());
        // The token in hand still serves while it lasts.
        std::fs::write(&marker, "garbage").unwrap();
        let token = refresher
            .token_at(&machine.account, machine.expires_at - 60)
            .unwrap();
        assert_eq!(token.expose_secret(), "first-access-value");
        // And a garbled marker past the bound is cleared, and the renewal goes on.
        aged_past_the_bound(&marker);
        let token = refresher.token_at(&machine.account, expired).unwrap();
        assert_eq!(token.expose_secret(), "second-access-value");
        assert!(!marker.exists());
    }

    #[cfg(unix)]
    #[test]
    fn a_renewal_that_cannot_write_its_marker_is_not_made() {
        let machine = machine(Kind::GitHub, "github.com", Method::Device);
        let forge = Arc::new(StandIn::answering(&[Ok((200, RENEWED))]));
        let refresher = machine.refresher(&forge);
        let marker = refresher.marker(&machine.account).unwrap();
        // This process's own marker, which holds nobody off, and which cannot be written over.
        std::fs::write(
            &marker,
            format!("{}\n{}\n", machine.account.id, std::process::id()),
        )
        .unwrap();
        let mut mode = std::fs::metadata(&marker).unwrap().permissions();
        mode.set_readonly(true);
        std::fs::set_permissions(&marker, mode).unwrap();
        if std::fs::OpenOptions::new()
            .write(true)
            .open(&marker)
            .is_ok()
        {
            return; // Running as root: a read-only file stops nothing.
        }
        let token = refresher
            .token_at(&machine.account, machine.expires_at - AHEAD.as_secs())
            .unwrap();
        assert_eq!(token.expose_secret(), "first-access-value");
        let why = refresher
            .token_at(&machine.account, machine.expires_at + 1)
            .unwrap_err();
        assert_eq!(why.failure(), &Failure::Transport);
        assert!(why.said().contains("record"), "{why}");
        assert!(
            forge.posted.lock().unwrap().is_empty(),
            "a refresh token was sent with no marker to say so"
        );
        assert_eq!(
            machine.kept().unwrap().refresh.unwrap().expose_secret(),
            "first-refresh-value"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_sign_in_and_a_sign_out_wait_for_the_machine_wide_lock_a_renewal_holds() {
        let machine = machine(Kind::GitHub, "github.com", Method::Device);
        let keyring = accounts::keyring_at(machine.home.path());
        // Another process is in the middle of a renewal: it holds the lock file.
        let theirs = crate::machine::Lock::named(machine.home.path(), REFRESH_LOCK);
        assert!(theirs.held());
        let access_now = || machine.kept().unwrap().access.expose_secret().to_string();
        std::thread::scope(|scope| {
            let signing_in = scope.spawn(|| sign_in_again(&machine).unwrap());
            std::thread::sleep(Duration::from_millis(300));
            assert_eq!(
                access_now(),
                "first-access-value",
                "a sign-in wrote while another process held the lock"
            );
            drop(theirs);
            signing_in.join().unwrap();
            assert_eq!(access_now(), "second-access-value");
        });
        let theirs = crate::machine::Lock::named(machine.home.path(), REFRESH_LOCK);
        std::thread::scope(|scope| {
            let signing_out = scope.spawn(|| {
                accounts::sign_out(
                    &HostScope::for_a_test(),
                    machine.home.path(),
                    keyring.as_ref(),
                    &machine.account.id,
                )
                .unwrap()
            });
            std::thread::sleep(Duration::from_millis(300));
            assert!(
                machine.kept().is_some(),
                "a sign-out deleted while another process held the lock"
            );
            drop(theirs);
            signing_out.join().unwrap();
            assert!(machine.kept().is_none());
        });
    }

    #[test]
    fn forgetting_one_trouble_told_of_an_account_keeps_every_other_kind_told() {
        let mut account = machine(Kind::GitHub, "github.com", Method::Device).account;
        // An id of its own, so no other test's troubles share its keys.
        account.id = "01J9FORGETS-ONE-KIND".to_owned();
        let told = |trouble: &Trouble| {
            ONCE.lock()
                .unwrap_or_else(|e| e.into_inner())
                .as_ref()
                .is_some_and(|told| told.contains(&once_key(&account.id, trouble)))
        };
        raise_once(&account, &Trouble::ScopesUnknown);
        raise_once(&account, &Trouble::SecondSignInRefused);

        forget_raised_of(&account.id, &Trouble::SecondSignInRefused);

        assert!(!told(&Trouble::SecondSignInRefused));
        assert!(told(&Trouble::ScopesUnknown), "another kind must stay told");
    }
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The interactive sign-in as a credential source** (ADR 0077, ADR 0128 D1).
//!
//! The account the person signed in to on a host, its token read from the keyring and renewed by
//! the [`Refresher`] when it is about to expire. No account for the host answers
//! [`CredentialError::None`]; an account the forge ended, or whose token left the keyring,
//! answers [`CredentialError::SignInAgain`]. **It never answers with another account's token**,
//! another host's, or a forge CLI's login (ADR 0070 §5): where a host has more than one account
//! and none is chosen for the project it refuses, and an account the forge ended is not replaced by
//! another that is signed in. **An account whose token holds `workflow`** (GitHub folded the CI-file
//! permission into it) **is refused the same way** until the person signs in again without it: that
//! token is never on a chat's route.

use std::path::{Path, PathBuf};

use super::{Credential, CredentialError, CredentialSource, Source, TokenKind};
use crate::forge::accounts::{self, ForgeAccount, State};
use crate::forge::refresh::{Refresher, Trouble, ended_said};
use crate::forge::route::HostScope;
use crate::forge::signin::SIGN_IN_AT;
use crate::forge::{Failure, ForgeError};

/// The signed-in account's token for a host.
pub struct SignedIn {
    config_root: PathBuf,
    refresher: Refresher,
}

impl SignedIn {
    /// The accounts under `config_root`, in the host's process: only the host can claim the
    /// scope this takes, because only it may renew a sign-in (one writer, ADR 0077 §3).
    pub fn new(host: &HostScope, config_root: &Path) -> SignedIn {
        SignedIn {
            refresher: Refresher::new(host, config_root),
            config_root: config_root.to_path_buf(),
        }
    }

    /// [`SignedIn::new`], renewing through `refresher`: a test's stand-in forge.
    #[cfg(any(test, feature = "fenced"))]
    pub fn through(refresher: Refresher, config_root: &Path) -> SignedIn {
        SignedIn {
            refresher,
            config_root: config_root.to_path_buf(),
        }
    }

    /// [`CredentialSource::credential_for`], with the time given. **Only a host with one account
    /// answers without being told which**: with several, and none chosen for the project (the
    /// choice is the person's press, ADR 0128 D2 and D4), nothing is picked, and an account whose
    /// sign-in ended is never replaced by another that happens to be signed in. Use
    /// [`SignedIn::credential_of`] with the account the project is bound to.
    pub fn credential_at(&self, host: &str, now: u64) -> Result<Credential, CredentialError> {
        let host = host.trim().to_ascii_lowercase();
        let on_host = self.on(&host)?;
        match on_host.as_slice() {
            [] => Err(CredentialError::None {
                how: super::no_account_how(super::Mode::Interactive, &host),
                host,
            }),
            [only] => self.credential_for_account(&host, only, now, false),
            several => {
                let logins: Vec<&str> = several.iter().map(|a| a.login.as_str()).collect();
                Err(unusable(
                    &host,
                    format!(
                        "More than one account is signed in to {host} ({}), and none is chosen \
                         for this project. Choose an account in {SIGN_IN_AT}.",
                        logins.join(", ")
                    ),
                ))
            }
        }
    }

    /// The token of the account `account_id` (the one a project is bound to) on `host`, at `now`.
    /// An account that is not on this host, or not listed, is an error and no other is tried.
    pub fn credential_of(
        &self,
        host: &str,
        account_id: &str,
        now: u64,
    ) -> Result<Credential, CredentialError> {
        let host = host.trim().to_ascii_lowercase();
        let on_host = self.on(&host)?;
        match on_host.iter().find(|a| a.id == account_id) {
            Some(account) => self.credential_for_account(&host, account, now, true),
            None => Err(unusable(
                &host,
                format!(
                    "The account chosen for this project is not signed in to {host} on this \
                     machine. Choose an account in {SIGN_IN_AT}."
                ),
            )),
        }
    }

    /// Every account listed for `host`, signed in or not.
    fn on(&self, host: &str) -> Result<Vec<ForgeAccount>, CredentialError> {
        Ok(accounts::listed(&self.config_root)
            .map_err(|why| unusable(host, why))?
            .into_iter()
            .filter(|a| a.host.eq_ignore_ascii_case(host))
            .collect())
    }

    /// `for_chat`: the token goes to a chat's route, which is given an account only when the forge
    /// has said it does not hold `workflow` (an account whose scopes are unknown may). The
    /// person's own use of an account does not wait on that.
    fn credential_for_account(
        &self,
        host: &str,
        account: &ForgeAccount,
        now: u64,
        for_chat: bool,
    ) -> Result<Credential, CredentialError> {
        if account.state != State::SignedIn {
            // Listed and signed out: the forge ended it, or the person signed out.
            return Err(CredentialError::SignInAgain {
                host: host.to_string(),
                said: ended_said(account),
            });
        }
        // A token that holds `workflow` is never a chat route's (ADR 0128 D1.1): checked before
        // the keyring is read, so nothing of it is even fetched.
        match accounts::workflow_doubt(account) {
            accounts::WorkflowDoubt::No => {}
            accounts::WorkflowDoubt::Holds => {
                crate::forge::refresh::raise_once(account, &Trouble::EverydayHoldsWorkflow);
                return Err(CredentialError::SignInAgain {
                    host: host.to_string(),
                    said: accounts::workflow::needs_narrower_sign_in(account),
                });
            }
            accounts::WorkflowDoubt::Unknown if for_chat => {
                crate::forge::refresh::raise_once(account, &Trouble::ScopesUnknown);
                return Err(CredentialError::SignInAgain {
                    host: host.to_string(),
                    said: accounts::workflow::scopes_unknown_said(account),
                });
            }
            accounts::WorkflowDoubt::Unknown => {}
        }
        let token = self
            .refresher
            .token_at(account, now)
            .map_err(|e| failed(host, account, &e))?;
        let expires_at = accounts::kept(accounts::keyring_at(&self.config_root).as_ref(), account)
            .ok()
            .flatten()
            .and_then(|kept| kept.expires_at);
        Ok(Credential::new(
            account.kind,
            host,
            token,
            Source::SignedIn,
            TokenKind::of_method(account.method),
            format!("the sign-in to {host}"),
        )
        .with_login(&account.login)
        .expiring_at(expires_at))
    }
}

impl CredentialSource for SignedIn {
    fn credential_for(&self, host: &str) -> Result<Credential, CredentialError> {
        self.credential_at(host, accounts::now())
    }
}

fn unusable(host: &str, said: String) -> CredentialError {
    CredentialError::Unusable {
        host: host.to_string(),
        said,
    }
}

/// A renewal or a read that failed, as a credential error. The refresher has already raised the
/// Notice when the forge ended the sign-in.
fn failed(host: &str, account: &ForgeAccount, why: &ForgeError) -> CredentialError {
    match why.failure() {
        Failure::Auth => CredentialError::SignInAgain {
            host: host.to_string(),
            said: why.said().to_string(),
        },
        Failure::Transport => CredentialError::Unreachable {
            host: host.to_string(),
            said: why.said().to_string(),
        },
        _ => unusable(
            host,
            format!(
                "purlis could not use its sign-in to {host} as {}: {}",
                account.login,
                why.said()
            ),
        ),
    }
}

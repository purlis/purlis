// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The interactive sign-in as a credential source** (ADR 0077, ADR 0128 D1).
//!
//! The account the person signed in to on a host, its token read from the keyring and renewed by
//! the [`Refresher`] when it is about to expire. No account for the host answers
//! [`CredentialError::None`]; an account the forge ended, or whose token left the keyring,
//! answers [`CredentialError::SignInAgain`]. **It never answers with another account's token**,
//! another host's, or a forge CLI's login (ADR 0070 §5).

use std::path::{Path, PathBuf};

use super::{Credential, CredentialError, CredentialSource, Source, TokenKind};
use crate::forge::accounts::{self, ForgeAccount, State};
use crate::forge::refresh::{Refresher, ended_said};
use crate::forge::route::HostScope;
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

    /// [`CredentialSource::credential_for`], with the time given.
    pub fn credential_at(&self, host: &str, now: u64) -> Result<Credential, CredentialError> {
        let host = host.trim().to_ascii_lowercase();
        let on_host: Vec<ForgeAccount> = accounts::listed(&self.config_root)
            .map_err(|why| unusable(&host, why))?
            .into_iter()
            .filter(|a| a.host.eq_ignore_ascii_case(&host))
            .collect();
        // The first signed-in account on the host, as the window's own requests choose it.
        let Some(account) = on_host.iter().find(|a| a.state == State::SignedIn) else {
            return Err(match on_host.first() {
                // Listed and signed out: the forge ended it, or the person signed out.
                Some(ended) => CredentialError::SignInAgain {
                    host,
                    said: ended_said(ended),
                },
                None => CredentialError::None {
                    how: super::no_account_how(super::Mode::Interactive, &host),
                    host,
                },
            });
        };
        let token = self
            .refresher
            .token_at(account, now)
            .map_err(|e| failed(&host, account, &e))?;
        let expires_at = accounts::kept(accounts::keyring_at(&self.config_root).as_ref(), account)
            .ok()
            .flatten()
            .and_then(|kept| kept.expires_at);
        Ok(Credential::new(
            account.kind,
            &host,
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

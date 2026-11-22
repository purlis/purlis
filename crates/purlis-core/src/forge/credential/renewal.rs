// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Renewing a sign-in before it expires, without the person** (ADR 0077 §3, ADR 0128 D1).
//!
//! A browser sign-in's token lasts eight hours on GitHub and two on GitLab. A [`Refresher`]
//! already renews one when a request is about to use it with less than five minutes left; this
//! renews them **ahead of any request**, [`LEAD`] before they end, so that a request is never the
//! one that waits on the forge, and a forge that has ended the sign-in is found while the person
//! is still at their desk.
//!
//! - [`Renewal::sweep`] looks at every signed-in account whose token a grant made and renews the
//!   ones with less than [`LEAD`] left. It is the one writer: renewal goes through the same
//!   [`Refresher`] and the same process-wide lock, so it cannot race a request's own renewal.
//! - **A refusal ends the sign-in and raises one Notice.** The refresher marks the account
//!   signed out, deletes its keyring item and tells the window ([`super::super::refresh`]); the
//!   next sweep finds the account signed out and skips it, so the Notice is not raised twice.
//!   **Nothing else stands in**: no other account, no token connection and no forge CLI's login.
//! - **A forge that cannot be reached ends nothing.** The token is kept while it has time, and
//!   the next sweep tries again after [`RETRY`].
//! - [`Renewal::keep`] runs the sweep on a thread of its own until the [`Keeper`] is dropped.

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use crate::forge::Failure;
use crate::forge::accounts::{self, ForgeAccount, Method, State};
use crate::forge::refresh::Refresher;
use crate::forge::route::HostScope;

/// How long before a token ends a sweep renews it: well ahead of [`super::super::refresh::AHEAD`],
/// the five minutes a request renews at.
pub const LEAD: Duration = Duration::from_secs(15 * 60);

/// The longest the thread sleeps: an account signed in meanwhile is found within this.
pub const LONGEST_SLEEP: Duration = Duration::from_secs(10 * 60);

/// The shortest it sleeps, and how long it waits to try again after a forge could not be reached.
pub const RETRY: Duration = Duration::from_secs(60);

/// What a sweep did about one account.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Swept {
    pub host: String,
    pub login: String,
    pub outcome: Outcome,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// More than [`LEAD`] left: nothing was asked.
    NotDue,
    /// Renewed: a new token, with a later expiry.
    Renewed,
    /// The forge refused: the sign-in has ended, and the Notice is raised.
    Ended,
    /// The forge could not be reached or answered something else; the token is kept as it is.
    Unreachable,
}

/// Renews the signed-in accounts under a config root.
pub struct Renewal {
    config_root: PathBuf,
    refresher: Refresher,
}

impl Renewal {
    /// In the host's process: only the host can claim the scope, as for any renewal.
    pub fn new(host: &HostScope, config_root: &Path) -> Renewal {
        Renewal {
            refresher: Refresher::new(host, config_root),
            config_root: config_root.to_path_buf(),
        }
    }

    /// [`Renewal::new`], renewing through `refresher`: a test's stand-in forge.
    #[cfg(any(test, feature = "fenced"))]
    pub fn through(refresher: Refresher, config_root: &Path) -> Renewal {
        Renewal {
            refresher,
            config_root: config_root.to_path_buf(),
        }
    }

    /// The accounts a grant made, signed in, with when each token ends.
    fn renewable(&self) -> Vec<(ForgeAccount, Option<u64>)> {
        let Ok(listed) = accounts::listed(&self.config_root) else {
            return Vec::new();
        };
        let keyring = accounts::keyring_at(&self.config_root);
        listed
            .into_iter()
            .filter(|a| {
                a.state == State::SignedIn && matches!(a.method, Method::Device | Method::Pkce)
            })
            .map(|account| {
                let ends = accounts::kept(keyring.as_ref(), &account)
                    .ok()
                    .flatten()
                    .and_then(|kept| kept.expires_at);
                (account, ends)
            })
            .collect()
    }

    /// Renews every account with less than [`LEAD`] left, at `now` (seconds since the epoch).
    pub fn sweep(&self, now: u64) -> Vec<Swept> {
        self.renewable()
            .into_iter()
            .map(|(account, ends)| {
                let outcome = match ends {
                    Some(at) if at <= now.saturating_add(LEAD.as_secs()) => {
                        match self.refresher.token_ahead(&account, now, LEAD) {
                            Ok(_) => self.after_ok(&account, ends),
                            Err(e) if *e.failure() == Failure::Auth => Outcome::Ended,
                            Err(_) => Outcome::Unreachable,
                        }
                    }
                    _ => Outcome::NotDue,
                };
                Swept {
                    host: account.host,
                    login: account.login,
                    outcome,
                }
            })
            .collect()
    }

    /// `Ok` from the refresher is a renewal when the expiry moved, and a token kept while the
    /// forge could not be reached when it did not.
    fn after_ok(&self, account: &ForgeAccount, was: Option<u64>) -> Outcome {
        let now_ends = accounts::kept(accounts::keyring_at(&self.config_root).as_ref(), account)
            .ok()
            .flatten()
            .and_then(|kept| kept.expires_at);
        if now_ends > was {
            Outcome::Renewed
        } else {
            Outcome::Unreachable
        }
    }

    /// How long until the next sweep has anything to do, at `now`: until the earliest token is
    /// [`LEAD`] from its end, never less than [`RETRY`] nor more than [`LONGEST_SLEEP`].
    pub fn next_wake(&self, now: u64) -> Duration {
        self.renewable()
            .into_iter()
            .filter_map(|(_, ends)| ends)
            .map(|at| Duration::from_secs(at.saturating_sub(LEAD.as_secs()).saturating_sub(now)))
            .min()
            .unwrap_or(LONGEST_SLEEP)
            .clamp(RETRY, LONGEST_SLEEP)
    }

    /// Sweeps now, and then each time [`Renewal::next_wake`] says, on a thread of its own, until
    /// the returned [`Keeper`] is dropped. The thread ends after the sweep it is in.
    pub fn keep(self) -> Keeper {
        let (stop, stopped) = mpsc::channel::<()>();
        let spawned = std::thread::Builder::new()
            .name("forge-sign-in-renewal".to_string())
            .spawn(move || {
                loop {
                    let now = accounts::now();
                    for swept in self.sweep(now) {
                        if swept.outcome == Outcome::Renewed {
                            tracing::info!(
                                "purlis: renewed its sign-in to {} before it ended",
                                swept.host
                            );
                        }
                    }
                    // A stop, or the Keeper dropped, ends the loop; a timeout is the next sweep.
                    if !matches!(
                        stopped.recv_timeout(self.next_wake(accounts::now())),
                        Err(mpsc::RecvTimeoutError::Timeout)
                    ) {
                        break;
                    }
                }
            });
        if let Err(why) = &spawned {
            tracing::warn!("purlis: sign-ins are renewed only when used: {why}");
        }
        Keeper { _stop: stop }
    }
}

/// Keeps the renewal thread going. Dropping it stops the thread after its current sweep.
pub struct Keeper {
    _stop: mpsc::Sender<()>,
}

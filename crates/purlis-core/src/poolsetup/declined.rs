// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A failed add keeps the pool** (#7191, ruled 2026-11-18), on Docker and on a cluster: the pool
//! stays, in the state *Refused*, with its reason, so the person sees what was refused and why
//! instead of a setup that silently forgot it. Kept with Pool setup's choices
//! (`declined` in `pool-setup.json`); a later add that succeeds forgets it.
//!
//! [`PoolState`] is this lane's own state, one name for each of the five states ruled for the
//! status bar. Remote trains A adds the same five as `RemoteState`; each variant here maps 1:1,
//! so switching is a `From` impl and the wire's names do not change.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{Refused, read_choices, runners, write_choices};

/// **A pool the add refused, and why**, in the words the person is shown.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Declined {
    pub pool: String,
    pub reason: String,
}

/// **A pool's state, as the status bar says it** (#7191): five, named as ruled on #7191. Only
/// `Refused` is set from what Pool setup keeps; the others are Remote trains A's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum PoolState {
    Unreachable,
    Refused { reason: String },
    HostKeyChanged,
    RunnerGone,
    PoolForgotten,
}

impl PoolState {
    /// Its name on the wire and in the status bar.
    #[must_use]
    pub fn word(&self) -> &'static str {
        match self {
            PoolState::Unreachable => "unreachable",
            PoolState::Refused { .. } => "refused",
            PoolState::HostKeyChanged => "host-key-changed",
            PoolState::RunnerGone => "runner-gone",
            PoolState::PoolForgotten => "pool-forgotten",
        }
    }

    /// Why, where the state has a reason of its own.
    #[must_use]
    pub fn reason(&self) -> Option<&str> {
        match self {
            PoolState::Refused { reason } => Some(reason),
            _ => None,
        }
    }
}

/// **The pool a state is of**, and the state, as the window reads them: `{pool, state, reason?}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PoolStatus {
    pub pool: String,
    #[serde(flatten)]
    pub state: PoolState,
}

/// The longest a reason is kept: what the person reads, never a log.
const REASON_MOST: usize = 600;

/// **Keeps that the add of pool `pool` was refused**, for `why`, in place of any earlier refusal.
pub fn keep(config: &Path, pool: &str, why: &Refused) -> Result<(), Refused> {
    let _locked = runners::lock(config);
    let mut choices = read_choices(config)?;
    let mut reason = crate::shown::short(&why.to_string());
    reason.truncate(
        reason
            .char_indices()
            .nth(REASON_MOST)
            .map_or(reason.len(), |(at, _)| at),
    );
    choices.declined = Some(Declined {
        pool: pool.to_owned(),
        reason,
    });
    write_choices(config, choices)
}

/// **Forgets the refusal of pool `pool`**, where one is kept for it: it was made after all.
pub fn forget(config: &Path, pool: &str) -> Result<(), Refused> {
    let _locked = runners::lock(config);
    let mut choices = read_choices(config)?;
    if choices
        .declined
        .as_ref()
        .is_none_or(|kept| kept.pool != pool)
    {
        return Ok(());
    }
    choices.declined = None;
    write_choices(config, choices)
}

/// **What an add of pool `pool` answered, with the pool kept where it was refused**: an `Err` is
/// kept as the pool's reason and handed back as it was; an add that worked forgets any refusal.
/// A refusal that can't be kept is not allowed to hide the first.
pub fn kept_after<T>(config: &Path, pool: &str, added: Result<T, Refused>) -> Result<T, Refused> {
    match &added {
        Err(why) => {
            let _ = keep(config, pool, why);
        }
        Ok(_) => {
            let _ = forget(config, pool);
        }
    }
    added
}

/// **The state of the pool Pool setup keeps**, where it keeps one refused.
#[must_use]
pub fn status_of(declined: Option<&Declined>) -> Option<PoolStatus> {
    declined.map(|kept| PoolStatus {
        pool: kept.pool.clone(),
        state: PoolState::Refused {
            reason: kept.reason.clone(),
        },
    })
}

#[cfg(test)]
mod tests;

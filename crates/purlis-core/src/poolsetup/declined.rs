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
    /// **Where it was asked for**: the machine's route, a cluster's `cluster:<context>`, or empty
    /// for this machine. A pool is a name *there*: a pool of the same name made elsewhere is
    /// another pool, and forgets nothing of this refusal. Empty where a file older than this
    /// field kept it, which only [`forget_kept`] clears.
    #[serde(default)]
    pub on: String,
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

/// **Where an add was asked for, as one spelling** (#7191): a route spelled with its socket
/// (`ssh://me@lab/var/run/docker.sock`) and without (`ssh://me@lab`) is one machine, so a pool
/// made by `purlis pool add` on one forgets the refusal kept on the other. Anything that is no
/// route (empty for this machine, `cluster:<context>`) is kept as typed.
#[must_use]
pub fn place(on: &str) -> String {
    let on = on.trim();
    if let Ok(route) = crate::sshroute::Route::parse(on) {
        return format!(
            "ssh://{}{}{}",
            route
                .user()
                .map(|user| format!("{user}@"))
                .unwrap_or_default(),
            route.host(),
            route
                .port()
                .map(|port| format!(":{port}"))
                .unwrap_or_default()
        );
    }
    if let Ok(route) = crate::tlsroute::Route::parse(on) {
        return format!("tcp://{}:{}", route.host(), route.port());
    }
    on.to_owned()
}

/// The longest a reason is kept: what the person reads, never a log.
const REASON_MOST: usize = 600;

/// **Keeps that the add of pool `pool`, asked for `on`, was refused**, for `why`, in place of any
/// earlier refusal. The caller holds [`runners::lock`].
fn keep_locked(config: &Path, pool: &str, on: &str, why: &Refused) -> Result<(), Refused> {
    let on = place(on);
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
        on,
        reason,
    });
    write_choices(config, choices)
}

/// **Keeps that the add of pool `pool`, asked for `on`, was refused**, for `why`.
pub fn keep(config: &Path, pool: &str, on: &str, why: &Refused) -> Result<(), Refused> {
    let _locked = runners::lock(config);
    keep_locked(config, pool, on, why)
}

/// **Keeps the refusal of pool `pool`, unless this machine already keeps a pool of that name**: a
/// name turned away for being taken is no refused pool, and the pool that stands is not Refused.
/// Whether the name is kept is read under the same lock the refusal is written under.
pub fn keep_unless_kept(config: &Path, pool: &str, on: &str, why: &Refused) {
    let _locked = runners::lock(config);
    let kept = super::kept_pools(config).is_ok_and(|kept| kept.iter().any(|one| one == pool));
    if !kept {
        let _ = keep_locked(config, pool, on, why);
    }
}

/// **Forgets the refusal of pool `pool` asked for `on`**, where one is kept for that very pool:
/// it was made after all. A pool of another name, or of the same name elsewhere, clears nothing.
pub fn forget(config: &Path, pool: &str, on: &str) -> Result<(), Refused> {
    let on = place(on);
    let _locked = runners::lock(config);
    let mut choices = read_choices(config)?;
    if choices
        .declined
        .as_ref()
        .is_none_or(|kept| kept.pool != pool || kept.on != on)
    {
        return Ok(());
    }
    choices.declined = None;
    write_choices(config, choices)
}

/// **What an add of pool `pool` on `on` answered, with the pool kept where it was refused**: an
/// `Err` is kept as the pool's reason and handed back as it was; an add that made the pool
/// (`was_made`) forgets that pool's refusal. A refusal that can't be kept is not allowed to hide
/// the first.
pub fn kept_after<T>(
    config: &Path,
    pool: &str,
    on: &str,
    added: Result<T, Refused>,
    was_made: impl Fn(&T) -> bool,
) -> Result<T, Refused> {
    match &added {
        Err(why) => keep_unless_kept(config, pool, on, why),
        // Only an add that made the pool forgets a refusal: one that made nothing (a team's image
        // breaking the contract) leaves what was kept as it was.
        Ok(made) if was_made(made) => {
            let _ = forget(config, pool, on);
        }
        Ok(_) => {}
    }
    added
}

/// **Forgets whatever refusal is kept**, whichever pool it is of: the person forgets the refused
/// pool from the status bar, where no pool was made to forget by any other way.
pub fn forget_kept(config: &Path) -> Result<(), Refused> {
    let _locked = runners::lock(config);
    let mut choices = read_choices(config)?;
    if choices.declined.is_none() {
        return Ok(());
    }
    choices.declined = None;
    write_choices(config, choices)
}

/// **The state of the pool Pool setup keeps**, where it keeps one refused, and only while the
/// machine or cluster chosen now (`now`, as [`place`] spells it) is the one it was refused on:
/// pointing Pool setup at another says nothing of the first. A refusal an older build kept has no
/// place, and shows wherever it is.
#[must_use]
pub fn status_of(declined: Option<&Declined>, now: &str) -> Option<PoolStatus> {
    let now = place(now);
    declined
        .filter(|kept| kept.on.is_empty() || kept.on == now)
        .map(|kept| PoolStatus {
            pool: kept.pool.clone(),
            state: PoolState::Refused {
                reason: kept.reason.clone(),
            },
        })
}

#[cfg(test)]
mod tests;

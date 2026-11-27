// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A profile a runner kept is trusted kept there until it restarts** (#7863, ADR 0078 as
//! amended 2026-11-27).
//!
//! A chat's start on a pool has the pool's runner keep its profile first, over a link of its own
//! (ADR 0078's profile link), before the chat's own link starts it there. That link is a
//! `docker exec`, a handshake and a keep: the larger part of a start where the runner already
//! holds the profile as it is. So this machine remembers, per runner (its device id), pool and
//! profile, the digest of what was kept there and how its harness stood with it, and a start
//! makes the link again only where:
//!
//! - **what would be sent changed**: its name, harness, pin, sandbox mark, settings files, MCP
//!   servers or the pool's limits ([`digest_of`]);
//! - **the runner is new**: another device id answers for the pool, a runner made again;
//! - **the runner restarted**: Docker says it is not running, so the start wakes it, or it
//!   started at another time than when it kept the profile (`Resumed.started` among them); or
//! - **its harness did not say signed in** (or that it cannot tell): a signed-out profile is
//!   never remembered, so a sign-in on the runner is seen at the next start.
//!
//! **A skipped keep is never trusted on its own** (the person's ruling of 2026-11-27 on #7863):
//! a keep is what puts the profile's settings on the runner back as this machine has them, since a
//! chat may be able to write there (ADR 0095 as amended 2026-10-31). So only a keep that left the
//! runner holding a digest it can be held to is remembered (session protocol 1.97,
//! [`KeptThere::held`]), and the chat's start names that digest ([`held_for_a_start`]): the runner
//! reads what it holds off its disk then and starts the chat only where it comes to it. Any drift,
//! a settings file planted, changed or gone, refuses the start, and the profile is kept again
//! before the chat starts. A runner before 1.97, which could not be held to it, is never
//! remembered, so every start there keeps the profile as before.
//!
//! **What is remembered is this run's, in this process only**: never the plane, never a file
//! (the way [`super::TIDIED`] is kept), so no store has a tier to name (ADR 0069), and the first
//! start on each runner after the app starts makes the link as before. A profile forgotten on a
//! runner, signed out there, or refused there as signed out is forgotten here too ([`forget`]).
//!
//! **Only a Docker pool's runner is remembered** ([`RunnerNow`]): its start time is Docker's to
//! say. A cluster pool's runner, which its operator vouches for at each link, and a chat's own
//! fresh runner, which holds nothing yet, make the link at every start.

use std::sync::{Mutex, PoisonError};

use purlis_core::profilestore::onrunner;
use purlis_core::profilestore::sign_in::Standing;

/// **A pool's runner as Docker says it is now**, where it runs: what a remembered keep is held
/// to. `None` for one that does not run (the start wakes it), one Docker cannot be asked about,
/// and any runner that is not a Docker pool's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RunnerNow {
    /// The device id its pairing answered, compared without case (ADR 0078 as amended
    /// 2026-10-20).
    pub device: String,
    /// When its container last started, in seconds since the epoch, as Docker prints it.
    pub started: Option<u64>,
}

/// **What a keep on a runner came to**: how its harness stood with the profile, and the digest of
/// the settings the runner holds since, where it can be held to it at a start (1.97).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct KeptThere {
    pub standing: Standing,
    pub held: Option<String>,
}

/// One keep remembered.
#[derive(Debug, Clone)]
struct Remembered {
    device: String,
    pool: String,
    profile: String,
    digest: Vec<u8>,
    started: u64,
    standing: Standing,
    /// What the runner holds of the profile's settings since the keep, which a start names.
    held: String,
}

static REMEMBERED: Mutex<Vec<Remembered>> = Mutex::new(Vec::new());

/// **Has pool `pool`'s runner keep `kept` through `link`, unless it is known to hold it as it is**:
/// the standing remembered, with no link made, where `now` (asked before) says the same runner
/// runs since the same start as when it kept the same digest. Otherwise `link` is made, and what
/// its harness said is remembered against the runner as `now` says it is after the link, or
/// forgotten.
pub(super) fn kept_or_linked(
    now: &dyn Fn() -> Option<RunnerNow>,
    pool: &str,
    kept: &onrunner::Kept,
    link: impl FnOnce(&onrunner::Kept) -> Result<Option<KeptThere>, String>,
) -> Result<Option<Standing>, String> {
    let digest = digest_of(kept);
    if let (Some(digest), Some(before)) = (&digest, now())
        && let Some(standing) = remembered(&before, pool, &kept.name, digest)
    {
        return Ok(Some(standing));
    }
    let said = link(kept);
    forget(pool, &kept.name);
    let kept_as = match &said {
        Ok(Some(KeptThere {
            standing: standing @ (Standing::SignedIn | Standing::CannotTell),
            held: Some(held),
        })) => Some((*standing, held.clone())),
        _ => None,
    };
    if let (Some((standing, held)), Some(digest)) = (kept_as, digest)
        && let Some(RunnerNow {
            device,
            started: Some(started),
        }) = now()
    {
        memo().push(Remembered {
            device,
            pool: pool.to_owned(),
            profile: kept.name.clone(),
            digest,
            started,
            standing,
            held,
        });
    }
    said.map(|there| there.map(|there| there.standing))
}

/// **What a chat's start on pool `pool` on profile `profile` names its runner must still hold**
/// (1.97): the digest of the settings the last keep there left, where it is remembered. `None`
/// where nothing is, which is where that start's own readying made the keep.
pub(super) fn held_for_a_start(pool: &str, profile: &str) -> Option<String> {
    memo()
        .iter()
        .find(|one| one.pool == pool && one.profile == profile)
        .map(|one| one.held.clone())
}

/// What is remembered, whoever asks: a poisoned lock still holds a list that is whole, since
/// each change to it is one push or one retain.
fn memo() -> std::sync::MutexGuard<'static, Vec<Remembered>> {
    REMEMBERED.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The standing remembered of profile `profile` on pool `pool`, where `now` is the runner that
/// kept `digest` there, since the same start.
fn remembered(now: &RunnerNow, pool: &str, profile: &str, digest: &[u8]) -> Option<Standing> {
    let started = now.started?;
    memo()
        .iter()
        .find(|one| {
            one.pool == pool
                && one.profile == profile
                && one.device.eq_ignore_ascii_case(&now.device)
                && one.started == started
                && one.digest == digest
        })
        .map(|one| one.standing)
}

/// **Forgets what profile `name` was kept as on pool `pool`'s runner**: its next start there
/// makes the link again.
pub(super) fn forget(pool: &str, name: &str) {
    memo().retain(|one| one.pool != pool || one.profile != name);
}

/// The digest of `kept` as a runner of 1.56 or later is sent it ([`onrunner::in_batches`]),
/// with its pool's limits: `None` where it could not be read, which always makes the link, so
/// the start is refused there as it would have been.
fn digest_of(kept: &onrunner::Kept) -> Option<Vec<u8>> {
    use sha2::Digest as _;
    let to_keep = onrunner::in_batches(kept).ok()?;
    let mut hash = sha2::Sha256::new();
    hash.update(serde_json::to_vec(&to_keep.profile).ok()?);
    // By path, whatever order the folder was walked in; a file's pieces stay in their order.
    let mut files = std::collections::BTreeMap::<&str, Vec<u8>>::new();
    for piece in to_keep.batches.iter().flat_map(|batch| &batch.pieces) {
        files
            .entry(&piece.path)
            .or_default()
            .extend_from_slice(&piece.bytes);
    }
    for (path, bytes) in files {
        for part in [path.as_bytes(), &bytes] {
            hash.update((part.len() as u64).to_le_bytes());
            hash.update(part);
        }
    }
    for limit in purlis_core::poollimits::all() {
        hash.update(kept.limits.in_force(limit.id).to_le_bytes());
        hash.update(
            kept.highest
                .get(limit.id)
                .map_or([0xff; 8], u64::to_le_bytes),
        );
    }
    Some(hash.finalize().to_vec())
}

#[cfg(test)]
mod tests;

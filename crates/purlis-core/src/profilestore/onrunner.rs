// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A desktop's profiles on its runners** (#2049, spec #2041's *Profiles on runners*).
//!
//! A chat on a pool's runner is composed there by its names (#2171), so the runner needs a
//! profile of the name the chat runs on. The desktop asks its runner to keep one ([`Kept`],
//! the session protocol's `keep_profile`), and the runner makes it in its own store: a folder of
//! its own, in its own data home, which on a pool is the runner's home volume.
//!
//! **Nothing of the desktop's profile folder goes to the runner.** What is kept is the profile's
//! name, its harness, whether it is for sandboxed chats only, and the harness version it is
//! pinned to: no file, no setting, no sign-in, no vault reference. A subscription is signed in
//! on the runner through its harness's own flow, run there in the runner's folder
//! ([`super::sign_in`]), once per device, so no sign-in is ever copied between devices and a
//! single-use refresh token never has two holders (ADR 0095 §3).
//!
//! **Only a profile that signs in by a subscription goes to a runner in this version**
//! ([`Kept::of`]). A key or a cloud's credentials are handed to no chat yet, here or there
//! (#2151); a local model is a model on the desktop, which a runner does not reach.
//!
//! **A runner keeps a built-in's name too.** The desktop's own harness folder, adopted in place
//! as `claude`, is a profile of that name on the runner, made in a folder of the runner's own
//! (no runner folder is ever adopted), and it stands for the runner's built-in of that name
//! ([`crate::profiles::derive_with`]).

use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use super::sign_in::{self, Look, Standing};
use super::{Homes, MachineProfile};

/// **A desktop's profile, as a runner keeps it**: what the desktop sends, and nothing else.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kept {
    /// The name the desktop's chats, personas and dispatch lists use.
    pub name: String,
    /// The harness, as a profile's kind names it: `claude`, `codex` or `opencode`.
    pub harness: String,
    /// Only a sandboxed chat starts on it (#2037).
    pub sandbox_only: bool,
    /// The harness version the desktop's profile is pinned to (#2038), where it is pinned.
    pub pinned: Option<String>,
}

impl Kept {
    /// **The desktop's profile `record`, as its runners keep it**, or why it is not kept on a
    /// runner: one signed in by a key, a cloud's credentials or a local model, and one of a
    /// harness purlis does not ship.
    pub fn of(record: &MachineProfile) -> Result<Kept, String> {
        use crate::harness_declaration::SignInKind;
        let name = crate::shown::short(&record.name);
        match record
            .sign_in
            .as_ref()
            .map(crate::profilesignin::SignIn::kind)
        {
            None | Some(Some(SignInKind::Subscription)) => {}
            Some(Some(SignInKind::ApiKey | SignInKind::Cloud)) => {
                return Err(format!(
                    "the profile '{name}' signs in with credentials from your vault, which \
                     purlis hands to no chat on a pool's runner in this version"
                ));
            }
            Some(Some(SignInKind::Local)) => {
                return Err(format!(
                    "the profile '{name}' runs a model on this machine, which a pool's runner \
                     does not reach"
                ));
            }
            Some(None) => {
                return Err(format!(
                    "the profile '{name}' signs in a way this purlis does not know"
                ));
            }
        }
        if crate::harness_declaration::builtin(&record.harness).is_none() {
            return Err(format!(
                "the profile '{name}' runs no harness purlis ships, so no runner keeps it"
            ));
        }
        Ok(Kept {
            name: record.name.clone(),
            harness: record.harness.clone(),
            sandbox_only: record.sandbox_only,
            pinned: record.pinned.as_ref().map(|p| p.version.clone()),
        })
    }
}

/// **Keeps the desktop's profile `kept` in this runner's store under `homes`**, at `now`: made,
/// with nothing imported and no sign-in, where it has none of that name; brought to `kept`'s pin
/// and sandbox-only mark where it has. A profile of that name of another harness, or one adopted
/// in place, is refused and left as it is. The record as kept.
pub fn keep(homes: &Homes, kept: &Kept, now: u64) -> Result<MachineProfile, String> {
    let name = crate::shown::short(&kept.name);
    let pinned = match &kept.pinned {
        Some(version) => Some(
            super::pin::Version::parse(version)
                .map_err(|_| {
                    format!("the profile '{name}' is pinned to a version that is not one")
                })?
                .to_string(),
        ),
        None => None,
    };
    let brought = |record: &mut MachineProfile| {
        record.sandbox_only = kept.sandbox_only;
        if let Some(version) = &pinned
            && record.pinned.as_ref().map(|p| &p.version) != Some(version)
        {
            record.pinned = Some(super::pin::Pinned {
                version: version.clone(),
                at: now,
            });
        }
    };
    let found = super::update(homes.config_root(), |store| {
        let record = store.profiles.iter_mut().find(|p| p.name == kept.name)?;
        if record.harness != kept.harness || record.adopted.is_some() {
            return Some(Err(format!(
                "this runner has a profile named '{name}' of its own, so it keeps no other of \
                 that name"
            )));
        }
        brought(record);
        Some(Ok(record.clone()))
    })?;
    if let Some(found) = found {
        return found;
    }
    super::made::make(
        homes,
        &kept.name,
        &kept.harness,
        super::made::Making {
            machine: None,
            sandbox_only: kept.sandbox_only,
            builtin_name: true,
        },
        now,
    )?;
    super::update(homes.config_root(), |store| {
        let record = store
            .profiles
            .iter_mut()
            .find(|p| p.name == kept.name)
            .ok_or_else(|| format!("the profile '{name}' was not kept"))?;
        brought(record);
        Ok(record.clone())
    })?
}

/// **Whether the profile `record`, kept on this runner, is signed in here**, as its harness says
/// through `look` now: never a remembered answer, since this is what the desktop asks after a
/// sign-in.
pub fn standing(record: &MachineProfile, look: &dyn Look) -> Standing {
    match crate::profiles::of_record(record) {
        Some(profile) => {
            sign_in::forget(&profile);
            sign_in::standing_remembered(&profile, look)
        }
        None => Standing::CannotTell,
    }
}

impl From<&purlis_session_protocol::session::ProfileToKeep> for Kept {
    fn from(sent: &purlis_session_protocol::session::ProfileToKeep) -> Self {
        Kept {
            name: sent.name.clone(),
            harness: sent.harness.clone(),
            sandbox_only: sent.sandbox_only,
            pinned: sent.pinned.clone(),
        }
    }
}

impl From<Kept> for purlis_session_protocol::session::ProfileToKeep {
    fn from(kept: Kept) -> Self {
        purlis_session_protocol::session::ProfileToKeep {
            name: kept.name,
            harness: kept.harness,
            sandbox_only: kept.sandbox_only,
            pinned: kept.pinned,
        }
    }
}

/// The standing a runner's `profile_kept` names by `word`; a word this build does not know is
/// one it cannot tell.
pub fn standing_of_word(word: &str) -> Standing {
    [Standing::SignedIn, Standing::SignedOut]
        .into_iter()
        .find(|standing| standing.word() == word)
        .unwrap_or(Standing::CannotTell)
}

/// **What a runner's host answers its desktop's `keep_profile` with** (1.43): the profile
/// `sent` kept in this runner's store under `homes` at `now`, and the word for whether its
/// harness says, through `look`, that it is signed in here.
pub fn kept_for_the_desktop(
    homes: &Homes,
    sent: &purlis_session_protocol::session::ProfileToKeep,
    look: &dyn Look,
    now: u64,
) -> Result<String, String> {
    let record = keep(homes, &Kept::from(sent), now)?;
    Ok(standing(&record, look).word().to_owned())
}

/// **Whether a chat on the desktop's profile `name` may start on pool `pool`'s runner**, for the
/// project at `root`, with this desktop's profiles in the store under `config_root`, the runner
/// asked to keep it through `keep`. A profile this desktop keeps no record of (a built-in, or one
/// a clone declares) is the runner's own to say. One that goes to no runner is refused in words;
/// one its harness says is signed out on the runner is refused with a wait the window shows as
/// *Sign in <profile> on pool <pool>* ([`waiting_on_pools`]). Signed in, or where its harness
/// cannot tell, the chat starts, and nobody is asked anything.
pub fn ready_on(
    config_root: &Path,
    root: &Path,
    pool: &str,
    name: &str,
    keep: &dyn Fn(&Kept) -> Result<Standing, String>,
) -> Result<(), String> {
    let Some(record) = super::list(config_root)?
        .into_iter()
        .find(|p| p.name == name)
    else {
        return Ok(());
    };
    let shown = crate::shown::short(name);
    let kept = Kept::of(&record).map_err(|why| format!("{why}. Nothing was started."))?;
    match keep(&kept).map_err(|why| {
        format!(
            "pool {pool}'s runner did not keep the profile '{shown}': {why}. Nothing was started."
        )
    })? {
        Standing::SignedOut => {
            waits_on_pool(root, pool, &record);
            Err(format!(
                "the profile '{shown}' is not signed in on pool {pool}. Nothing was started. It \
                 waits in the Inbox as Sign in {shown} on pool {pool}, which runs its harness's \
                 own sign-in on the runner; then start the chat again."
            ))
        }
        Standing::SignedIn | Standing::CannotTell => Ok(()),
    }
}

/// **A profile a start on a pool was refused on**, waiting to be signed in on that pool's
/// runner: one per project, pool and profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolWait {
    /// The project the start was refused in.
    pub root: PathBuf,
    pub pool: String,
    /// The profile's name.
    pub profile: String,
    /// Its harness, as a profile's kind names it.
    pub kind: String,
    /// Its harness as people call it: `Claude Code`.
    pub harness: String,
    /// When the first start was refused, in seconds since 1970.
    pub since: u64,
}

static POOL_WAITS: Mutex<Vec<PoolWait>> = Mutex::new(Vec::new());

/// Records that a start in the project at `root` on pool `pool` was refused because `record` is
/// not signed in there.
fn waits_on_pool(root: &Path, pool: &str, record: &MachineProfile) {
    let mut all = POOL_WAITS.lock().unwrap_or_else(PoisonError::into_inner);
    if all
        .iter()
        .any(|w| w.root == root && w.pool == pool && w.profile == record.name)
    {
        return;
    }
    all.push(PoolWait {
        root: root.to_path_buf(),
        pool: pool.to_owned(),
        profile: record.name.clone(),
        kind: record.harness.clone(),
        harness: crate::harness_declaration::builtin(&record.harness)
            .map_or_else(|| record.harness.clone(), |d| d.title.clone()),
        since: crate::halt::now(),
    });
}

/// The profiles the project at `root` waits on to be signed in on a pool, oldest first.
pub fn waiting_on_pools(root: &Path) -> Vec<PoolWait> {
    POOL_WAITS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .iter()
        .filter(|w| w.root == root)
        .cloned()
        .collect()
}

/// The profile `name` is signed in on pool `pool` now, or the person put its wait away in the
/// project at `root` (`None`: in every project): its waits go.
pub fn let_go(root: Option<&Path>, pool: &str, name: &str) {
    POOL_WAITS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .retain(|w| !(w.pool == pool && w.profile == name && root.is_none_or(|r| w.root == r)));
}

#[cfg(test)]
mod tests;

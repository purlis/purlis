// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The *Your keys* page, as core reads it** (R-171, R-578; T1a-11).
//!
//! The window draws what [`rows`] answers and nothing of its own: each entry's product, the
//! origins its mark binds it to, its approval setting and what the check at save found, the
//! state its row is drawn in, the profiles and chats that use it, and the pools it is sent to.
//! The counts its questions say (W5's for *Remove*, W123's for *Stop sending*) are core's too.
//! Nothing here writes: the writers are [`super`]'s, called from the window's commands alone.

use std::path::Path;

use serde::Serialize;
use ulid::Ulid;

use super::pools::Allowances;
use super::{Approval, Checked, Product};
use crate::seam::Credentials;

/// One chat open on this machine, as the window knows it: its name, its profile, and the pool
/// it runs on (`None` here).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatUse {
    pub name: String,
    pub profile: String,
    pub pool: Option<String>,
}

/// The state a row is drawn in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum State {
    /// Its value reads, and its mark verifies.
    Marked,
    /// Its value reads, and it isn't marked: no mark, or changed outside purlis (W60).
    Unmarked,
    /// The keychain holds no item for its line: an orphaned index line (W55).
    Missing,
    /// The keychain couldn't be read, as when it is locked (W6).
    Locked { why: String },
}

/// One row of Your keys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub id: Ulid,
    pub product: Product,
    /// The origins its mark binds it to; empty where it isn't marked.
    pub origins: Vec<String>,
    pub approval: Approval,
    pub checked: Checked,
    pub state: State,
    /// The profiles naming it, by name.
    pub profiles: Vec<String>,
    /// The chats on those profiles, by name.
    pub chats: Vec<String>,
    /// The pools it is sent to, by name.
    pub pools: Vec<String>,
}

/// **Your keys as the page lists it**: every entry the index holds, in its order, each read
/// once for its state (its value read and its mark verified, never shown), with the profiles in
/// `profiles.json` naming it, the chats among `chats` on those profiles, and the pools
/// `allowances` sends it to. An index or a `profiles.json` that can't be read is the page's
/// trouble state (W115), so no row says no profile uses it because the file couldn't be read.
pub fn rows(
    config_root: &Path,
    credentials: &dyn Credentials,
    chats: &[ChatUse],
    allowances: &dyn Allowances,
) -> Result<Vec<Row>, String> {
    let profiles = profiles_of(config_root)?;
    Ok(super::list(config_root, credentials)?
        .into_iter()
        .map(|listed| {
            let entry = listed.entry;
            let (state, origins) = if listed.readable {
                state_of(config_root, credentials, entry.id)
            } else {
                (State::Missing, Vec::new())
            };
            let named = naming(&profiles, entry.id);
            Row {
                id: entry.id,
                product: entry.product,
                origins,
                approval: entry.approval,
                checked: entry.checked,
                state,
                chats: on(&named, chats, |_| true),
                profiles: named,
                pools: allowances.pools(entry.id),
            }
        })
        .collect())
}

/// Entry `id`'s state and its marked origins, from one read of its value and its mark.
fn state_of(config_root: &Path, credentials: &dyn Credentials, id: Ulid) -> (State, Vec<String>) {
    use super::ReadError;
    use super::mark::{ModelKeyError, read_model_key};
    match read_model_key(config_root, &crate::seam::RealFiles, credentials, id) {
        Ok((_, binding)) => (State::Marked, binding.origins.into_iter().collect()),
        Err(ModelKeyError::Unmarked(_)) => (State::Unmarked, Vec::new()),
        Err(ModelKeyError::Read(ReadError::Unreadable)) => (State::Missing, Vec::new()),
        Err(ModelKeyError::Read(why)) => (
            State::Locked {
                why: why.to_string(),
            },
            Vec::new(),
        ),
    }
}

/// The profiles of `profiles` whose sign-in names entry `id`, by name.
fn naming(profiles: &[crate::profilestore::MachineProfile], id: Ulid) -> Vec<String> {
    let id = id.to_string();
    profiles
        .iter()
        .filter(|p| p.sign_in.as_ref().and_then(|s| s.key()) == Some(id.as_str()))
        .map(|p| p.name.clone())
        .collect()
}

/// The profiles `profiles.json` holds. One that can't be read is an error, never no profiles:
/// the page and its questions never say no chats use a key because they couldn't look.
fn profiles_of(config_root: &Path) -> Result<Vec<crate::profilestore::MachineProfile>, String> {
    crate::profilestore::list(config_root)
        .map_err(|why| format!("purlis couldn't tell which profiles use your keys: {why}"))
}

/// The chats of `chats` on one of `profiles` that `keep` keeps, by name.
fn on(profiles: &[String], chats: &[ChatUse], keep: impl Fn(&ChatUse) -> bool) -> Vec<String> {
    chats
        .iter()
        .filter(|c| profiles.contains(&c.profile) && keep(c))
        .map(|c| c.name.clone())
        .collect()
}

/// **The chats *Remove* stops** (W5's count): the chats on a profile naming entry `id`.
pub fn chats_using(config_root: &Path, id: Ulid, chats: &[ChatUse]) -> Result<Vec<String>, String> {
    let profiles = profiles_of(config_root)?;
    Ok(on(&naming(&profiles, id), chats, |_| true))
}

/// **The chats *Stop sending* stops** (W123's count): those of [`chats_using`] on `pool`.
pub fn chats_on_pool(
    config_root: &Path,
    id: Ulid,
    pool: &str,
    chats: &[ChatUse],
) -> Result<Vec<String>, String> {
    let profiles = profiles_of(config_root)?;
    Ok(on(&naming(&profiles, id), chats, |c| {
        c.pool.as_deref() == Some(pool)
    }))
}

/// **Unlock** (W6): reads entry `id`'s item, which on a locked keychain asks the system to
/// unlock it. `Ok(true)` where it now reads, `Ok(false)` where the keychain holds no item for
/// the entry (a missing item, W55), and the error where the keychain failed (W84), which is
/// also what a cancelled unlock answers: telling the two apart is T1a-61's.
pub fn unlock(config_root: &Path, credentials: &dyn Credentials, id: Ulid) -> Result<bool, String> {
    use super::ReadError;
    match super::read_key(config_root, &crate::seam::RealFiles, credentials, id) {
        Ok(_) => Ok(true),
        Err(ReadError::Unreadable) => Ok(false),
        Err(why) => Err(why.to_string()),
    }
}

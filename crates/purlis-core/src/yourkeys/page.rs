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

/// **Your keys as the page lists it**: every entry the index holds, in its order.
pub fn rows(
    config_root: &Path,
    credentials: &dyn Credentials,
    chats: &[ChatUse],
    allowances: &dyn Allowances,
) -> Result<Vec<Row>, String> {
    let _ = (config_root, credentials, chats, allowances);
    Ok(Vec::new())
}

/// **The chats *Remove* stops** (W5's count): the chats on a profile naming entry `id`.
pub fn chats_using(config_root: &Path, id: Ulid, chats: &[ChatUse]) -> Vec<String> {
    let _ = (config_root, id, chats);
    Vec::new()
}

/// **The chats *Stop sending* stops** (W123's count): those of [`chats_using`] on `pool`.
pub fn chats_on_pool(config_root: &Path, id: Ulid, pool: &str, chats: &[ChatUse]) -> Vec<String> {
    let _ = (config_root, id, pool, chats);
    Vec::new()
}

/// **Unlock** (W6): reads entry `id`'s item, which on a locked keychain asks the system to
/// unlock it. `Ok(true)` where it now reads, `Ok(false)` where it still doesn't (the person
/// cancelled), and the error where the keychain failed (W84).
pub fn unlock(config_root: &Path, credentials: &dyn Credentials, id: Ulid) -> Result<bool, String> {
    let _ = (config_root, credentials, id);
    Ok(false)
}

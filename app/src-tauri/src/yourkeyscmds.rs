// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The window's commands for Your keys** (T1a-11, #6756; R-171, R-578): the one file of the
//! app's that writes your keys or its marks (T1a-09's and T1a-10's guards allow it alone), and
//! only from the window: no CLI verb, session-protocol method or chat tool reaches it. On macOS
//! a write from here lands held to the app, which `yourkeys::add` requires.
//!
//! - **The page** reads [`your_keys`]: core's rows (`purlis_core::yourkeys::page`), with the
//!   chats open in every project this window holds, so the profiles, chats and pools using a key
//!   are core's answer, never the window's own state.
//! - **The key form** saves through [`your_key_save`], aimed at a new entry, this entry or this
//!   profile (V7). Where a key for the product is held, it answers [`KeySaved::Held`]: W9's
//!   *Replace the key…* is [`your_key_replace_held`], which replaces in place with the value in
//!   hand and finishes the save (X2); *Use the key you already have* is [`your_key_use_held`].
//! - **Every pasted value is checked at save** (V10): [`check_at_save`], T1a-38's check, which
//!   answers *not checked yet* until it lands. *Replace* records what it found (T1a-10's review).
//! - **Remove** deletes the item, the mark, the pool key allowances, then the line (C-23); the
//!   allowances and the push of *Stop sending*'s removal are T1a-53's store and T1a-74's link
//!   message, reached through core's seams, which answer nothing until they land.

use std::path::{Path, PathBuf};

use purlis_core::seam::{Credentials, RealCredentials};
use purlis_core::secrets::keyring::Secret;
use purlis_core::yourkeys::mark::{self, Binding};
use purlis_core::yourkeys::page::{self, ChatUse, State};
use purlis_core::yourkeys::pools::{self, NoneYet};
use purlis_core::yourkeys::{self, Checked, Product, Replaced, Saved};
use serde::{Deserialize, Serialize};
use ulid::Ulid;

use crate::planes::Planes;

/// One row of Your keys, as the window draws it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct YourKeyRow {
    /// The entry's id: what every command for it names.
    pub id: String,
    /// The product's name, as its declaration gives it (*Anthropic*); no label in 1a.
    pub product: String,
    /// The origins its mark binds it to; empty where it isn't marked.
    pub origins: Vec<String>,
    /// Its approval setting: `off`, the only one in 1a.
    pub approval: String,
    /// What the check at save found: `passed`, `not checked yet` or `refused at save`.
    pub checked: String,
    /// The state its row is drawn in: `marked`, `unmarked` (W60), `missing` (W55) or
    /// `locked` (W6).
    pub state: String,
    /// The profiles naming it.
    pub profiles: Vec<String>,
    /// The chats on those profiles.
    pub chats: Vec<String>,
    /// The pools it is sent to.
    pub pools: Vec<String>,
}

/// What the key form is aimed at (V7).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum KeyTarget {
    /// A new entry: J0's form and the page's *Add a key…*.
    NewEntry,
    /// This entry: *Replace the key…*, and *Add a key…* for a missing entry. The new value is
    /// written under the same id and marked.
    ThisEntry { id: String },
    /// This keyless profile: the saved entry's id is written into it.
    ThisProfile { name: String },
}

/// What a save of the key form did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum KeySaved {
    /// The key is in your keys, under `id`, marked.
    Saved { id: String },
    /// A key for the product is held, under `held`, and nothing was written: the form offers
    /// *Use the key you already have* and, where `may_replace`, W9's *Replace the key…* (U9);
    /// aimed at a missing entry, only the first (F-8).
    Held { held: String, may_replace: bool },
}

/// **The check at save** (V10): T1a-38's. Until it lands nothing reaches Anthropic from here,
/// so every pasted value is saved *not checked yet* (W7), which is what is true of it.
fn check_at_save(_value: &Secret) -> Checked {
    Checked::NotCheckedYet
}

/// The config home your keys are kept in.
fn config_root() -> Result<PathBuf, String> {
    purlis_core::machine::config_root().ok_or_else(|| {
        "purlis cannot tell where its config home is, so it keeps no keys".to_owned()
    })
}

/// The keychain this build keeps your keys in: the system's, or a fenced build's stub.
fn credentials(config_root: &Path) -> RealCredentials {
    RealCredentials::new(purlis_core::linkkey::this_builds_keyring(config_root))
}

/// The pool key allowances and the runner link (T1a-53, T1a-74): none yet.
const ALLOWANCES: NoneYet = NoneYet;

fn id_of(id: &str) -> Result<Ulid, String> {
    id.parse()
        .map_err(|_| format!("'{id}' isn't the id of a key in your keys"))
}

/// The built-in declaration `product` names, for its name and the binding a mark takes.
fn declaration_of(product: &Product) -> Option<&'static purlis_core::model_provider::Declaration> {
    (product.from == "built-in")
        .then(|| {
            purlis_core::model_provider::built_ins()
                .iter()
                .find(|d| d.id == product.id)
        })
        .flatten()
}

fn binding_of(product: &Product) -> Result<Binding, String> {
    declaration_of(product).map(Binding::of).ok_or_else(|| {
        format!(
            "purlis has no declaration {} {} to mark this key for",
            product.from, product.id
        )
    })
}

fn checked_word(checked: Checked) -> &'static str {
    match checked {
        Checked::Passed => "passed",
        Checked::NotCheckedYet => "not checked yet",
        Checked::RefusedAtSave => "refused at save",
    }
}

fn row_of(row: page::Row) -> YourKeyRow {
    let state = match row.state {
        State::Marked => "marked",
        State::Unmarked => "unmarked",
        State::Missing => "missing",
        State::Locked { .. } => "locked",
    };
    YourKeyRow {
        id: row.id.to_string(),
        product: declaration_of(&row.product)
            .map_or_else(|| row.product.id.clone(), |d| d.name.clone()),
        origins: row.origins,
        approval: "off".to_owned(),
        checked: checked_word(row.checked).to_owned(),
        state: state.to_owned(),
        profiles: row.profiles,
        chats: row.chats,
        pools: row.pools,
    }
}

/// The chats open in every project this window holds, each with its profile and pool.
fn chats_here(planes: &Planes) -> Vec<ChatUse> {
    planes
        .open_now()
        .into_iter()
        .filter_map(|plane| planes.held(&plane).ok())
        .flat_map(|held| held.chats().on_profiles())
        .collect()
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|err| format!("your keys did not answer: {err}"))?
}

/// **Your keys, as the page lists them** (R-171).
#[tauri::command]
#[specta::specta]
pub async fn your_keys(planes: tauri::State<'_, Planes>) -> Result<Vec<YourKeyRow>, String> {
    let chats = chats_here(&planes);
    blocking(move || {
        let root = config_root()?;
        Ok(page::rows(&root, &credentials(&root), &chats, &ALLOWANCES)?
            .into_iter()
            .map(row_of)
            .collect())
    })
    .await
}

/// **How many chats use entry `id`** (W5's count, before *Remove* and W9's *Replace the key…*).
#[tauri::command]
#[specta::specta]
pub async fn your_key_chats(planes: tauri::State<'_, Planes>, id: String) -> Result<u32, String> {
    let chats = chats_here(&planes);
    blocking(move || {
        let count = page::chats_using(&config_root()?, id_of(&id)?, &chats).len();
        Ok(u32::try_from(count).unwrap_or(u32::MAX))
    })
    .await
}

/// **How many chats on `pool` use entry `id`** (W123's count, before *Stop sending*).
#[tauri::command]
#[specta::specta]
pub async fn your_key_pool_chats(
    planes: tauri::State<'_, Planes>,
    id: String,
    pool: String,
) -> Result<u32, String> {
    let chats = chats_here(&planes);
    blocking(move || {
        let count = page::chats_on_pool(&config_root()?, id_of(&id)?, &pool, &chats).len();
        Ok(u32::try_from(count).unwrap_or(u32::MAX))
    })
    .await
}

/// **Remove** (C-23): the item, the mark, the allowances, then the line.
#[tauri::command]
#[specta::specta]
pub async fn your_key_remove(id: String) -> Result<(), String> {
    blocking(move || {
        let root = config_root()?;
        yourkeys::remove(&root, &credentials(&root), &ALLOWANCES, id_of(&id)?)
    })
    .await
}

/// **Mark again** (W126, U8): T1a-10's one writer of a mark, for the entry's own declaration.
#[tauri::command]
#[specta::specta]
pub async fn your_key_mark_again(id: String) -> Result<(), String> {
    blocking(move || {
        let root = config_root()?;
        mark_again(&root, &credentials(&root), id_of(&id)?)
    })
    .await
}

/// **Unlock** (W6): `true` where the key now reads, `false` where the person cancelled.
#[tauri::command]
#[specta::specta]
pub async fn your_key_unlock(id: String) -> Result<bool, String> {
    blocking(move || {
        let root = config_root()?;
        page::unlock(&root, &credentials(&root), id_of(&id)?)
    })
    .await
}

/// **The row's *Try again*** for a key *not checked yet* (W7, F-6): the check again, and what
/// it found kept. T1a-71's one request takes its place once it lands.
#[tauri::command]
#[specta::specta]
pub async fn your_key_check_again(id: String) -> Result<String, String> {
    blocking(move || {
        let root = config_root()?;
        let id = id_of(&id)?;
        let value = yourkeys::read_key(
            &root,
            &purlis_core::seam::RealFiles,
            &credentials(&root),
            id,
        )
        .map_err(|why| why.to_string())?;
        let checked = check_at_save(&value);
        yourkeys::set_checked(&root, id, checked)?;
        Ok(checked_word(checked).to_owned())
    })
    .await
}

/// **Stop sending** (W125, X1): the entry's row for `pool`, and its removal pushed to the pool.
#[tauri::command]
#[specta::specta]
pub async fn your_key_stop_sending(id: String, pool: String) -> Result<(), String> {
    blocking(move || pools::stop_sending(id_of(&id)?, &pool, &ALLOWANCES, &ALLOWANCES)).await
}

/// **The key form's Save** (V7): aimed at `target`, with the pasted `value`.
#[tauri::command]
#[specta::specta]
pub async fn your_key_save(target: KeyTarget, value: String) -> Result<KeySaved, String> {
    blocking(move || {
        let root = config_root()?;
        save(&root, &credentials(&root), &target, &value)
    })
    .await
}

/// **W9's *Replace the key…*** (X2): replaces the held entry's value in place with the value in
/// hand, then finishes the save: a keyless `profile` is pointed at it.
#[tauri::command]
#[specta::specta]
pub async fn your_key_replace_held(
    held: String,
    value: String,
    profile: Option<String>,
) -> Result<KeySaved, String> {
    blocking(move || {
        let root = config_root()?;
        replace_held(
            &root,
            &credentials(&root),
            id_of(&held)?,
            &value,
            profile.as_deref(),
        )
    })
    .await
}

/// **Use the key you already have** (W2): a keyless `profile` is pointed at the held entry.
#[tauri::command]
#[specta::specta]
pub async fn your_key_use_held(held: String, profile: Option<String>) -> Result<KeySaved, String> {
    blocking(move || {
        let held = id_of(&held)?;
        if let Some(name) = profile {
            purlis_core::profilestore::key_into(&config_root()?, &name, held)?;
        }
        Ok(KeySaved::Saved {
            id: held.to_string(),
        })
    })
    .await
}

// ---------------------------------------------------------------------------------------------
// What the commands do, over a config home and a keychain a test can give

/// Marks entry `id` for its own product's declaration.
fn mark_again(root: &Path, credentials: &dyn Credentials, id: Ulid) -> Result<(), String> {
    let listed = yourkeys::list(root, credentials)?;
    let entry = listed
        .iter()
        .find(|l| l.entry.id == id)
        .ok_or_else(|| yourkeys::ReadError::NotInYourKeys.to_string())?;
    mark::mark(root, credentials, id, &binding_of(&entry.entry.product)?)
}

/// The key form's save, aimed at `target` (V7). Save order (C-23): the index line, the item, the
/// mark, then the profile; a failed step after the item takes out what this save wrote.
fn save(
    root: &Path,
    credentials: &dyn Credentials,
    target: &KeyTarget,
    value: &str,
) -> Result<KeySaved, String> {
    let product = Product::anthropic();
    let checked = check_at_save(&Secret::new(value.to_owned()));
    let listed = yourkeys::list(root, credentials)?;
    let held = listed
        .iter()
        .find(|l| l.entry.product == product)
        .map(|l| l.entry.id);
    if let KeyTarget::ThisEntry { id } = target {
        let id = id_of(id)?;
        if listed.iter().any(|l| l.entry.id == id) {
            // The same id, its new value, marked: a rotated key keeps its pools (X1).
            let replaced = yourkeys::replace(root, credentials, &ALLOWANCES, id, value, checked)?;
            if replaced == Replaced::Unmarked {
                mark_again(root, credentials, id)?;
            }
            return Ok(KeySaved::Saved { id: id.to_string() });
        }
        // Aimed at a missing entry while another key is held: only *Use the key you already
        // have* (F-8).
        if let Some(held) = held {
            return Ok(KeySaved::Held {
                held: held.to_string(),
                may_replace: false,
            });
        }
    }
    let id = match yourkeys::add(root, credentials, product.clone(), checked, value)? {
        Saved::Added(id) => id,
        Saved::ReplaceOffered { held } => {
            return Ok(KeySaved::Held {
                held: held.to_string(),
                may_replace: true,
            });
        }
    };
    let finished =
        mark::mark(root, credentials, id, &binding_of(&product)?).and_then(|()| match target {
            KeyTarget::ThisProfile { name } => purlis_core::profilestore::key_into(root, name, id),
            _ => Ok(()),
        });
    if let Err(why) = finished {
        let mut said = why;
        if let Err(left) = yourkeys::remove(root, credentials, &ALLOWANCES, id) {
            said += &format!("; and the key is left in your keys: {left}");
        }
        return Err(said);
    }
    Ok(KeySaved::Saved { id: id.to_string() })
}

/// W9's *Replace the key…* (X2): the value in hand under the held id, marked, and the save
/// finished.
fn replace_held(
    root: &Path,
    credentials: &dyn Credentials,
    held: Ulid,
    value: &str,
    profile: Option<&str>,
) -> Result<KeySaved, String> {
    let checked = check_at_save(&Secret::new(value.to_owned()));
    if yourkeys::replace(root, credentials, &ALLOWANCES, held, value, checked)?
        == Replaced::Unmarked
    {
        mark_again(root, credentials, held)?;
    }
    if let Some(name) = profile {
        purlis_core::profilestore::key_into(root, name, held)?;
    }
    Ok(KeySaved::Saved {
        id: held.to_string(),
    })
}

#[cfg(test)]
mod tests;

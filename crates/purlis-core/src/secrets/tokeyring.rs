// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Moving a plain-file vault into the system keychain** (#2071): what a person is offered
//! before they turn their sandbox off for a project, since with no sandbox a chat can read any
//! file on disk, a plain-file vault's included.
//!
//! **Whole or not at all.** Every secret is written to the keychain and read back before the
//! registration changes and the file and its rotation record go, all under the vault's keys'
//! lock. A secret that cannot be written or read back stops the move: what was written is
//! deleted, and the answer says what could not be taken back rather than calling it gone.
//!
//! **Only this machine's own registration.** A vault the team's file registers is the team's
//! to move: its registration reaches every machine, and this one moving it would leave the
//! others with a vault they cannot read. It is refused with a sentence.

use serde_json::Map;

use super::registry::{self, Vault};
use super::{Ctx, VaultError, keyring, plain_file};
use crate::tui::Markup;

/// The provider a plain-file vault is registered with.
pub const PLAIN_FILE: &str = "plain-file";

/// A plain-file vault this machine has, and whether this machine may move it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct PlainFile {
    pub name: String,
    /// `false` for a vault the team's file registers, which is the team's to move.
    pub movable: bool,
}

/// Every vault registered with the plain-file provider, sorted by name.
pub fn plain_file_vaults(ctx: &Ctx) -> Result<Vec<PlainFile>, VaultError> {
    let shared = registry::load_shared(ctx)?;
    let local = registry::load_local(ctx)?;
    let doc = registry::merged(&shared, &local);
    let mut out: Vec<PlainFile> = registry::vaults(&doc)
        .iter()
        .filter(|(_, entry)| {
            entry.get("provider").and_then(serde_json::Value::as_str) == Some(PLAIN_FILE)
        })
        .map(|(name, _)| PlainFile {
            name: name.clone(),
            movable: registry::scope_in(&shared, &local, name) == "local",
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// What a move did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Moved {
    /// How many secrets are now in the keychain.
    pub secrets: usize,
    /// Where the old file is, where it could not be removed: the person removes it.
    pub file_kept: Option<String>,
}

/// Moves plain-file vault `name` into the keychain: each secret written and read back, the
/// registration changed to the keychain, and the file and its rotation record removed.
pub fn move_to_keyring(ctx: &Ctx, name: &str) -> Result<Moved, VaultError> {
    move_to_keyring_with(&*keyring::store(ctx), ctx, name)
}

/// [`move_to_keyring`] against a store the caller chose.
///
/// **In this order, under the vault's keys' lock** ([`keyring::lock_keys_named`]), which every
/// keyring and identity writer of the project takes: the keychain vault is built beside the
/// plain file, every secret written and read back, and only then is the registration changed
/// and the file removed. Until the registration changes, the plain file is the vault; a move
/// that stops deletes what it wrote and its index, and says whatever it could not take back.
pub fn move_to_keyring_with(
    store: &dyn keyring::Store,
    ctx: &Ctx,
    name: &str,
) -> Result<Moved, VaultError> {
    // Every check below is made under the lock, so no writer changes what it judged before
    // the move is made.
    let _keys = keyring::lock_keys_named(ctx, name, keyring::A_WRITE_WAITS)?;
    let shared = registry::load_shared(ctx)?;
    let local = registry::load_local(ctx)?;
    if registry::scope_in(&shared, &local, name) != "local" {
        return Err(VaultError::new(format!(
            "vault '{name}' is registered in the team's file, so moving it is the team's: it \
             would change it on every machine. Nothing was moved"
        )));
    }
    let plain = registry::vault(ctx, name)?;
    if plain.provider != PLAIN_FILE {
        return Err(VaultError::new(format!(
            "vault '{name}' is not a plain file, so there is nothing to move"
        )));
    }
    let held = Vault {
        name: plain.name.clone(),
        provider: KEYRING.to_owned(),
        persona: plain.persona.clone(),
        config: Map::new(),
    };
    clear_an_old_index(ctx, &held)?;
    let file = plain_file::file_path(ctx, &plain)?;
    let secrets: Vec<(String, String)> = plain_file::load(ctx, &plain, "secret")?
        .iter()
        .map(|(key, value)| (key.clone(), super::py_str(value)))
        .collect();
    let mut written: Vec<String> = Vec::new();
    let mut service = None;
    let made = write_all(store, ctx, &held, &secrets, &mut service, &mut written).and_then(|()| {
        registry::add_vault(
            ctx,
            name,
            KEYRING,
            Map::new(),
            plain.persona.as_deref(),
            true,
            false,
        )
    });
    if let Err(why) = made {
        return Err(stopped(
            store,
            ctx,
            &plain,
            &held,
            service.as_deref(),
            &written,
            &why,
        ));
    }
    let file_kept = [file.clone(), plain_file::meta_path(&file)]
        .iter()
        .filter_map(|one| match std::fs::remove_file(one) {
            Ok(()) => None,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(_) => Some(one.display().to_string()),
        })
        .reduce(|a, b| format!("{a} and {b}"));
    Ok(Moved {
        secrets: secrets.len(),
        file_kept,
    })
}

/// The keyring provider's name in the registry.
const KEYRING: &str = "keyring";

/// An index a keychain vault of this name left before (`<name>.keys.json`): one that lists no
/// item is removed, and one that lists items is refused, since its items are a vault of their
/// own that this move would otherwise write over or strand.
fn clear_an_old_index(ctx: &Ctx, held: &Vault) -> Result<(), VaultError> {
    let at = keyring::index_path(ctx, held);
    if std::fs::symlink_metadata(&at).is_err() {
        return Ok(());
    }
    let old = keyring::load_index(ctx, held)?;
    if !old.keys.is_empty() {
        return Err(VaultError::new(format!(
            "a keychain vault called '{}' was here before, and its index {} still lists {}. \
             Nothing was moved: remove that index if those items are not wanted, or \
             rename this vault first",
            held.name,
            super::short_path(&ctx.root, &at),
            crate::counts::counted(old.keys.len(), "item", "items")
        )));
    }
    std::fs::remove_file(&at).map_err(|e| {
        VaultError::new(format!(
            "the old keychain index {} could not be removed: {e}. Nothing was moved",
            super::short_path(&ctx.root, &at)
        ))
    })
}

/// Every secret written under a new service, each read back, with the index naming them.
/// `service` and `written` say what was made, for [`stopped`] to take back.
fn write_all(
    store: &dyn keyring::Store,
    ctx: &Ctx,
    held: &Vault,
    secrets: &[(String, String)],
    service: &mut Option<String>,
    written: &mut Vec<String>,
) -> Result<(), VaultError> {
    for (key, _) in secrets {
        keyring::check_key(key)?;
    }
    let made = keyring::new_service(held)?;
    let mut index = keyring::Index {
        service: Some(made.clone()),
        ..keyring::Index::default()
    };
    // Kept before the first item is written, as `keyring::set_with` does: an item under a
    // service no index records is one nothing would find again.
    keyring::save_index(ctx, held, &index)?;
    *service = Some(made.clone());
    let at = keyring::now();
    for (key, value) in secrets {
        // Listed before it is written: a move that dies after the write leaves the item named
        // in the index, never one in the keychain that nothing finds again.
        let entry = |kept: bool| keyring::Entry {
            size: super::fingerprint::size_band(value),
            updated: at.clone(),
            held: kept,
        };
        index.keys.insert(key.clone(), entry(false));
        keyring::save_index(ctx, held, &index)?;
        written.push(key.clone());
        let kept = store.set(&made, key, value)?;
        let back = store.get(&made, key)?.map(keyring::Secret::into_inner);
        if back.as_deref() != Some(value.as_str()) {
            return Err(VaultError::new(format!(
                "the keychain gave back something else for '{key}'"
            )));
        }
        if kept == keyring::Held::ToTheApp {
            index.keys.insert(key.clone(), entry(true));
            keyring::save_index(ctx, held, &index)?;
        }
    }
    Ok(())
}

/// A move that stopped, undone as far as it can be, and said as it stands: the items written
/// deleted, the new index removed, and the registration the plain file's. What could not be
/// taken back is named, never called gone.
fn stopped(
    store: &dyn keyring::Store,
    ctx: &Ctx,
    plain: &Vault,
    held: &Vault,
    service: Option<&str>,
    written: &[String],
    why: &VaultError,
) -> VaultError {
    let name = &plain.name;
    let mut left: Vec<String> = Vec::new();
    if let Some(service) = service {
        for key in written {
            if store.delete(service, key).is_err() {
                left.push(format!("'{key}'"));
            }
        }
    }
    let index_left = left.is_empty()
        && std::fs::remove_file(keyring::index_path(ctx, held))
            .is_err_and(|e| e.kind() != std::io::ErrorKind::NotFound);
    let registered = registry::vault(ctx, name).is_ok_and(|now| now.provider == PLAIN_FILE)
        || registry::add_vault(
            ctx,
            name,
            PLAIN_FILE,
            plain.config.clone(),
            plain.persona.as_deref(),
            true,
            false,
        )
        .is_ok();
    let mut said = Markup::from(format!("vault '{name}' was not moved: "))
        .then(why.message.clone())
        .then(Markup::from("."));
    if registered {
        said = said.then(Markup::from(
            " It is still a plain file, and its file still holds its values.",
        ));
    } else {
        said = said.then(Markup::from(format!(
            " purlis could not register it as a plain file again: its file is still there, \
             and `purlis vault add {name} --provider plain-file --force` with its file points \
             at it again."
        )));
    }
    if !left.is_empty() {
        said = said.then(Markup::from(format!(
            " purlis could not take back {} it wrote to the keychain under {}: they stay \
             there, listed in {}.",
            left.join(", "),
            service.unwrap_or("?"),
            super::short_path(&ctx.root, &keyring::index_path(ctx, held))
        )));
    } else if index_left {
        said = said.then(Markup::from(format!(
            " Its unused keychain index {} could not be removed: remove it before moving again.",
            super::short_path(&ctx.root, &keyring::index_path(ctx, held))
        )));
    }
    VaultError::new(said)
}

#[cfg(test)]
mod tests;

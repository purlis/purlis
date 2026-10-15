// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Moving a plain-file vault into the system keychain** (#2071): what a person is offered
//! before they turn their sandbox off for a project, since with no sandbox a chat can read any
//! file on disk, a plain-file vault's included.
//!
//! **Whole or not at all.** Every secret is written to the keychain and read back before the
//! registration changes for good and the file goes. A secret that cannot be written or read back
//! takes back what was written, and the vault stays a plain file, as it was.
//!
//! **Only this machine's own registration.** A vault the team's file registers is the team's
//! to move: its registration reaches every machine, and this one moving it would leave the
//! others with a vault they cannot read. It is refused with a sentence.

use serde_json::Map;

use super::registry::{self, Vault};
use super::{Ctx, VaultError, keyring, plain_file};

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
/// registration changed to the keychain, and the file removed.
pub fn move_to_keyring(ctx: &Ctx, name: &str) -> Result<Moved, VaultError> {
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
    let file = plain_file::file_path(ctx, &plain)?;
    let secrets: Vec<(String, String)> = plain_file::load(ctx, &plain, "secret")?
        .iter()
        .map(|(key, value)| (key.clone(), super::py_str(value)))
        .collect();
    let persona = plain.persona.clone();
    registry::add_vault(
        ctx,
        name,
        "keyring",
        Map::new(),
        persona.as_deref(),
        true,
        false,
    )?;
    let held = registry::vault(ctx, name)?;
    let mut written: Vec<&str> = Vec::new();
    for (key, value) in &secrets {
        let kept = keyring::set(ctx, &held, key, value)
            .and_then(|()| keyring::get(ctx, &held, key))
            .and_then(|back| {
                if back == *value {
                    Ok(())
                } else {
                    Err(VaultError::new(format!(
                        "the keychain gave back something else for '{key}'"
                    )))
                }
            });
        if let Err(why) = kept {
            written.push(key);
            take_back(ctx, &held, &plain, &written);
            return Err(VaultError::new(format!(
                "vault '{name}' was not moved: {}. It is a plain file, as it was",
                why.message
            )));
        }
        written.push(key);
    }
    let file_kept = match std::fs::remove_file(&file) {
        Ok(()) => None,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(_) => Some(file.display().to_string()),
    };
    Ok(Moved {
        secrets: secrets.len(),
        file_kept,
    })
}

/// Undoes a move that stopped: the keychain's items written for it deleted, and the vault
/// registered as the plain file it was.
fn take_back(ctx: &Ctx, held: &Vault, plain: &Vault, written: &[&str]) {
    for key in written {
        let _ = keyring::delete(ctx, held, key);
    }
    let _ = registry::add_vault(
        ctx,
        &plain.name,
        PLAIN_FILE,
        plain.config.clone(),
        plain.persona.as_deref(),
        true,
        false,
    );
}

#[cfg(test)]
mod tests;

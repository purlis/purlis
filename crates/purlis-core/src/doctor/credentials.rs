// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The `vaults` and `credential paths` rows (OB-8, #994): whether each vault's credentials are
//! where it says they are, and whether a file that holds them in plain text is kept the way a
//! credential has to be.
//!
//! **Never a value, never a provider asked.** Both rows read the registry the way every vault
//! command does and ask only what can be answered without a secret in hand:
//!
//! - **`keyring`**: its keys index, and for each key the index lists whether the system keyring
//!   holds its item, asked of the item's attributes ([`keyring::Store::exists`], #1660), which
//!   reads no value and never makes the keyring ask the person. A keyring that cannot say
//!   without reading the value (the Secret Service, the Credential Manager) is said as not
//!   asked. The preflight a chat's start runs asks no keyring at all.
//! - **`plain-file`**: whether its file is there, or gone though values were written to it. The
//!   file is never opened: it holds the values.
//! - **`1password` and `reference`**: whether each program the vault is read through is found,
//!   the lookup `secret exec` makes ([`Ctx::program`]). The items themselves are not asked,
//!   because asking runs that program: `purlis vault verify` does, when a person runs it.
//!
//! `credential paths` is about the files that hold values in plain text, the `plain-file`
//! vaults': each one is not reached through a link (every read refuses one) and no other
//! account may read or write it. Its contents are never read. Where the file was put is the
//! `vault files` row's question, and whether the registry can be read is `vault registry`'s.

use super::{Doctor, Row};
use crate::counts::{counted, plural};
use crate::secrets::keyring::{self, Presence};
use crate::secrets::registry::{self, Vault};
use crate::secrets::{Ctx, Env, plain_file};

/// The rows' names, as the Python charter printed them.
pub(super) const VAULTS: &str = "vaults";
pub(super) const PATHS: &str = "credential paths";

/// The `vaults` row for the project `d` answers for.
pub(super) fn vaults(d: &Doctor) -> Row {
    if !d.has_plane {
        return Row::ok(VAULTS, "no control plane found");
    }
    vaults_row(&Ctx::new(&d.root, Env::from_process()), !d.preflight)
}

/// The `credential paths` row for the project `d` answers for.
pub(super) fn credential_paths(d: &Doctor) -> Row {
    if !d.has_plane {
        return Row::ok(PATHS, "no control plane found");
    }
    paths_row(&Ctx::new(&d.root, Env::from_process()))
}

/// The usable vaults of the project `ctx` names, by name, or `None` when the registry cannot be
/// read. An entry no command can use is the `vault registry` row's to name.
fn usable(ctx: &Ctx) -> Option<Vec<Vault>> {
    let doc = registry::load_registry(ctx).ok()?;
    let mut names: Vec<String> = registry::vaults(&doc).keys().cloned().collect();
    names.sort();
    Some(
        names
            .iter()
            .filter_map(|name| registry::vault_in(&doc, name).ok())
            .collect(),
    )
}

/// A vault's name as a row says it: the committed half arrives by `git pull`.
fn shown(vault: &Vault) -> String {
    format!("'{}'", crate::personas::one_line(&vault.name))
}

/// What one vault came to: what is said of it, and whether that is something to act on.
struct Said {
    text: String,
    to_act_on: bool,
}

impl Said {
    fn fine(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            to_act_on: false,
        }
    }

    fn act(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            to_act_on: true,
        }
    }
}

/// [`vaults`] for the project `ctx` names. `ask_keyring` is false for the preflight, which asks
/// the system keyring nothing.
pub(super) fn vaults_row(ctx: &Ctx, ask_keyring: bool) -> Row {
    let Some(vaults) = usable(ctx) else {
        return Row::not_checked(
            VAULTS,
            "the vault registry cannot be read; the `vault registry` row says why",
        );
    };
    if vaults.is_empty() {
        return Row::ok(VAULTS, "no vaults registered");
    }
    let said: Vec<Said> = vaults
        .iter()
        .map(|vault| {
            let s = one_vault(ctx, vault, ask_keyring);
            Said {
                text: format!("{} ({}): {}", shown(vault), vault.provider, s.text),
                ..s
            }
        })
        .collect();
    let detail = said
        .iter()
        .map(|s| s.text.as_str())
        .collect::<Vec<_>>()
        .join("; ");
    if said.iter().any(|s| s.to_act_on) {
        return Row::warn(
            VAULTS,
            detail,
            "A vault named above cannot hand out every secret it lists. `purlis vault list` \
             shows each vault's state and `purlis vault verify <name>` asks its provider; a \
             program that is not found is the `<program> for vaults` row's to say where to put.",
        );
    }
    Row::ok(VAULTS, detail)
}

/// What [`vaults_row`] says of `vault`.
fn one_vault(ctx: &Ctx, vault: &Vault, ask_keyring: bool) -> Said {
    match vault.provider.as_str() {
        "keyring" => keyring_items(ctx, vault, ask_keyring),
        "plain-file" => file_there(ctx, vault),
        _ => programs_found(ctx, vault),
    }
}

/// A keyring vault: its index, and each item it lists, asked of its attributes.
fn keyring_items(ctx: &Ctx, vault: &Vault, ask_keyring: bool) -> Said {
    let index = match keyring::load_index(ctx, vault) {
        Ok(index) => index,
        Err(e) => return Said::act(super::one_line(&e.message, super::PATH_DISPLAY_LIMIT)),
    };
    let Some(service) = index.service else {
        return Said::fine("nothing stored yet");
    };
    let count = index.keys.len();
    if !ask_keyring {
        return Said::fine(format!(
            "{} in its index; the preflight asks the keyring nothing",
            counted(count, "key", "keys")
        ));
    }
    let store = keyring::store(ctx);
    let (mut present, mut absent, mut unknown) = (0usize, Vec::new(), 0usize);
    for key in index.keys.keys() {
        match store.exists(&service, key) {
            Presence::Present => present += 1,
            Presence::Absent => absent.push(crate::personas::one_line(key)),
            Presence::Unknown => unknown += 1,
        }
    }
    if !absent.is_empty() {
        return Said::act(format!(
            "{} of {} its index lists {} no item in {}: {}",
            absent.len(),
            counted(count, "key", "keys"),
            plural(absent.len(), "has", "have"),
            keyring::STORE_NAME,
            absent.join(", ")
        ));
    }
    if unknown > 0 {
        return Said::fine(format!(
            "{} in its index, {} found; {unknown} not asked, because \
             this keyring cannot say whether it holds an item without reading its value",
            counted(count, "key", "keys"),
            counted(present, "item", "items")
        ));
    }
    Said::fine(format!(
        "{} in its index, each with its item in {}",
        counted(count, "key", "keys"),
        keyring::STORE_NAME
    ))
}

/// A plain-file vault: whether its file is there. Never opened: it holds the values.
fn file_there(ctx: &Ctx, vault: &Vault) -> Said {
    let path = match plain_file::file_path(ctx, vault) {
        Ok(path) => path,
        Err(e) => return Said::act(e.message.to_string()),
    };
    if let Some(refused) = plain_file::linked_on_the_way(ctx, &path) {
        return Said::act(super::one_line(&refused.message, super::PATH_DISPLAY_LIMIT));
    }
    if std::fs::symlink_metadata(&path).is_ok() {
        return Said::fine("its file is there");
    }
    match plain_file::gone(ctx, vault, &path) {
        Some(_) => Said::act("its file is gone, though values were written to it"),
        None => Said::fine("no file yet, as nothing was written to it"),
    }
}

/// A vault read through a program: whether each one is found. Its items are not asked.
fn programs_found(ctx: &Ctx, vault: &Vault) -> Said {
    let programs = crate::secrets::program::needed_by(ctx, vault);
    if programs.is_empty() {
        return Said::fine("reads through no program");
    }
    let mut missing: Vec<String> = Vec::new();
    for program in &programs {
        if let Err(why) = ctx.program(program) {
            // Found only where a chat can write is never run, so it counts as missing here.
            missing.push(format!(
                "{program} {}",
                crate::secrets::program::status_of(&why)
            ));
        }
    }
    if !missing.is_empty() {
        return Said::act(missing.join(", "));
    }
    Said::fine(format!(
        "{} found; its items are not asked here, because asking runs {}",
        programs.join(", "),
        if programs.len() == 1 { "it" } else { "them" }
    ))
}

/// [`credential_paths`] for the project `ctx` names.
pub(super) fn paths_row(ctx: &Ctx) -> Row {
    let Some(vaults) = usable(ctx) else {
        return Row::not_checked(
            PATHS,
            "the vault registry cannot be read; the `vault registry` row says why",
        );
    };
    let files: Vec<&Vault> = vaults
        .iter()
        .filter(|vault| vault.provider == "plain-file")
        .collect();
    if files.is_empty() {
        return Row::ok(PATHS, "no vault keeps its credentials in a file");
    }
    let said: Vec<Said> = files.iter().map(|vault| one_path(ctx, vault)).collect();
    let detail = said
        .iter()
        .map(|s| s.text.as_str())
        .collect::<Vec<_>>()
        .join("; ");
    if said.iter().any(|s| s.to_act_on) {
        return Row::warn(
            PATHS,
            detail,
            "A vault file holds its values in plain text, so only your account may read or \
             write it: `chmod 600 <file>` for each one named above. A file reached through a \
             link is refused on every read: put the file itself where the vault names it.",
        );
    }
    Row::ok(PATHS, detail)
}

/// What [`paths_row`] says of the plain-file vault `vault`'s file. Its contents are not read.
fn one_path(ctx: &Ctx, vault: &Vault) -> Said {
    let path = match plain_file::file_path(ctx, vault) {
        Ok(path) => path,
        Err(e) => return Said::act(format!("{}: {}", shown(vault), e.message)),
    };
    let at = format!(
        "{} ({})",
        shown(vault),
        crate::personas::one_line(&crate::secrets::short_path(&ctx.root, &path))
    );
    if plain_file::linked_on_the_way(ctx, &path).is_some() {
        return Said::act(format!("{at} is reached through a link"));
    }
    let meta = match std::fs::symlink_metadata(&path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Said::fine(format!("{at} not created yet"));
        }
        Err(e) => return Said::act(format!("{at} cannot be looked at: {e}")),
    };
    if !meta.is_file() {
        return Said::act(format!("{at} is not a regular file"));
    }
    if open_to_others(&meta) {
        return Said::act(format!(
            "{at} can be read or written by other accounts (want 0600)"
        ));
    }
    Said::fine(format!("{at} is private to your account"))
}

/// Whether a group or other bit is set on the file: the test `secret get` tightens a vault file
/// by before it reads one ([`plain_file::tighten`]).
#[cfg(unix)]
fn open_to_others(meta: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    meta.permissions().mode() & crate::secrets::OTHERS != 0
}

#[cfg(not(unix))]
fn open_to_others(_meta: &std::fs::Metadata) -> bool {
    false
}

#[cfg(test)]
mod tests;

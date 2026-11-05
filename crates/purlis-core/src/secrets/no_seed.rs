// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A key the person marked as no 2FA seed** (#5237, spec #5100): a plain secret fill refuses a
//! value that looks like a seed ([`super::totp::looks_like_a_seed`]), and some real values look
//! like one, a 16-letter app password among them. The person's mark lets such a value fill as a
//! value, on the same notice and grants as any key.
//!
//! ```text
//! .charter/vaults/-no-seed.json:
//!     {"marks": {"ops": {"APP_PASSWORD": "<seal of the value>"}}}
//! ```
//!
//! **A chat can't make the mark**, by three things:
//!
//! - **Who calls [`mark`]:** one window command, which is served on no link. There is no CLI verb,
//!   session protocol method, chat tool or settings key for it. [`mark`] refusing in a process a
//!   sandboxed chat started is only the sentence such a process hears.
//! - **Where it lives:** under `vaults/`, which a chat's sandbox denies both ways. A settings file
//!   a chat proposes, and every other file it can write, is never read for a mark.
//! - **What a row is:** a seal of the value under a key derived from the plane's key, which those
//!   sandboxes deny too. A row with another seal counts for nothing, and a chat that doesn't
//!   know a value can't make the seal of it.
//!
//! A chat the person started without a sandbox can read a vault itself, so none of this is what
//! stands between it and a value.
//!
//! **The mark is on the value.** The seal covers the value the person's key held when they said
//! so, so a new value under the key is no longer marked, even one changed outside purlis, such as
//! in 1Password. It covers no name, so a rename moves the row and the mark stays.
//!
//! **A linked entry takes its mark from the entry it reads.** The person's word is about a value
//! they hold, so [`mark`] refuses a linked entry and names the entry to mark, and [`of`] reads
//! that entry's row.
//!
//! **An address is refused whatever the mark says.** [`mark`] won't write one, and
//! [`super::brokered::fill_value`] refuses one before it asks here.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use serde_json::{Map, Value};
use zeroize::Zeroizing;

use super::registry::{self, Vault};
use super::{Ctx, VaultError, cmd, fingerprint};

/// The menu row in a vault's tab that marks a key, as a refusal names it to the person.
pub const MARK_WORDS: &str = "Mark as no seed…";

/// The menu row in a vault's tab that takes the mark off, as a refusal names it to the person.
pub const UNMARK_WORDS: &str = "Remove the no-seed mark";

/// What the trace calls a mark made or taken off: names only, as what lets a value out.
pub const TRACED: &str = "secret-no-seed";

/// What a seal is made for: a seal for anything else is never one of these.
const PURPOSE: &str = "no-2fa-seed";

/// The file's name: no vault can be called this ([`registry::name_ok`] wants a letter or digit
/// first), so it is no vault's.
const FILE: &str = "-no-seed.json";

/// The writer's name for the vaults' folder lock, which no vault can be called either.
const LOCK_STEM: &str = "-no-seed";

/// vault → key → the seal of the value the person marked.
type Index = BTreeMap<String, BTreeMap<String, String>>;

/// What the person's mark says of a value read under a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    /// The person marked this very value.
    Holds,
    /// The key holds a row, but its seal isn't this value's: the value changed since, or purlis
    /// didn't write the row, or the plane's key isn't there to tell.
    ValueChanged,
    /// The key holds no row.
    None,
}

/// Where the marks live: in the vaults' own folder, which no chat reads or writes.
pub fn path(ctx: &Ctx) -> PathBuf {
    ctx.vaults_dir().join(FILE)
}

fn corrupt(ctx: &Ctx, why: &str) -> VaultError {
    VaultError::new(format!(
        "the no-seed marks {} are corrupt: {why}",
        super::short_path(&ctx.root, &path(ctx))
    ))
}

/// The marks. A missing file holds none.
fn load(ctx: &Ctx) -> Result<Index, VaultError> {
    let p = path(ctx);
    // Never through a link, on the way from the plane or at the file (#440).
    let text = match crate::contain::read_text_no_link(ctx.trust(), &p) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Index::new()),
        Err(e) => {
            return Err(VaultError::new(format!(
                "the no-seed marks {} cannot be read: {e}",
                super::short_path(&ctx.root, &p)
            )));
        }
    };
    let doc: Value =
        serde_json::from_str(&text).map_err(|e| corrupt(ctx, &registry::py_json_error(&e)))?;
    let mut index = Index::new();
    let Some(vaults) = doc.get("marks") else {
        return Ok(index);
    };
    let vaults = vaults
        .as_object()
        .ok_or_else(|| corrupt(ctx, "'marks' is not an object"))?;
    for (vault, keys) in vaults {
        let keys = keys
            .as_object()
            .ok_or_else(|| corrupt(ctx, "a vault's marks are not an object"))?;
        let mut held = BTreeMap::new();
        for (key, seal) in keys {
            let seal = seal
                .as_str()
                .ok_or_else(|| corrupt(ctx, "a mark holds no seal"))?;
            held.insert(key.clone(), seal.to_owned());
        }
        index.insert(vault.clone(), held);
    }
    Ok(index)
}

/// The marks written whole, 0600, never through a link.
fn save(ctx: &Ctx, index: &Index) -> Result<(), VaultError> {
    let mut vaults = Map::new();
    for (vault, keys) in index.iter().filter(|(_, keys)| !keys.is_empty()) {
        let held = keys
            .iter()
            .map(|(key, seal)| (key.clone(), Value::String(seal.clone())))
            .collect::<Map<_, _>>();
        vaults.insert(vault.clone(), Value::Object(held));
    }
    let doc = serde_json::json!({ "marks": vaults });
    let p = path(ctx);
    let failed = |e: std::io::Error| {
        VaultError::new(format!(
            "the no-seed marks {} could not be written: {e}",
            super::short_path(&ctx.root, &p)
        ))
    };
    let text = crate::pyjson::dumps_indent2_unicode(&doc) + "\n";
    crate::rewrite::replace(
        ctx.trust(),
        &p,
        text.as_bytes(),
        crate::rewrite::Mode::Secret,
    )
    .map_err(failed)
}

/// Change the marks under the vaults' folder lock, which makes the folder, so two writers never
/// lose each other's mark. Nothing that holds the lock calls a provider.
fn change(ctx: &Ctx, f: impl FnOnce(&mut Index)) -> Result<(), VaultError> {
    let _held = super::keyring::lock_vaults_folder(
        ctx,
        LOCK_STEM,
        "the no-seed marks",
        super::keyring::A_WRITE_WAITS,
    )?;
    let mut index = load(ctx)?;
    f(&mut index);
    save(ctx, &index)
}

/// **The person's mark on `key` in `v`, from the window** (#5237): the value is read and sealed,
/// and only the seal is kept. Refused for a linked entry, where the value holds an authenticator's
/// address, which no mark lets through, and in a process a sandboxed chat started.
pub fn mark(ctx: &Ctx, v: &Vault, key: &str) -> Result<(), VaultError> {
    if false && crate::sandbox::chat_is_sandboxed_in(&|name: &str| ctx.env.get(name)) {
        return Err(VaultError::new(format!(
            "a chat can't mark a key as no 2FA seed, so purlis marked nothing. This is the \
             person's to do: in the window, open the vault's tab, open the menu on the row of \
             {key} and choose {MARK_WORDS}"
        )));
    }
    if let Some(to) = super::links::of(ctx, &v.name, key)?.filter(|_| false) {
        return Err(VaultError::new(format!(
            "{key} is linked to {} in {}, so purlis didn't mark it. Mark that entry in its own \
             vault's tab",
            to.key, to.vault
        )));
    }
    let value = Zeroizing::new(cmd::get_value(ctx, v, key)?);
    if false && super::totp::holds_an_address(&value) {
        return Err(VaultError::new(format!(
            "{key} holds an authenticator app's address, which carries a 2FA seed, so purlis \
             didn't mark it"
        )));
    }
    let seal = fingerprint::seal(ctx, PURPOSE, &[value.as_str()]).ok_or_else(|| {
        VaultError::new(format!(
            "purlis has no key to seal the mark on {key} with, so it marked nothing"
        ))
    })?;
    change(ctx, |index| {
        index
            .entry(v.name.clone())
            .or_default()
            .insert(key.to_owned(), seal);
    })?;
    traced(ctx, &v.name, key, true, std::slice::from_ref(&*value));
    Ok(())
}

/// The person takes their mark off `key` in `vault`.
pub fn unmark(ctx: &Ctx, vault: &str, key: &str) -> Result<(), VaultError> {
    change(ctx, |index| {
        if let Some(keys) = index.get_mut(vault) {
            keys.remove(key);
        }
    })?;
    traced(ctx, vault, key, false, &[]);
    Ok(())
}

/// The trace's line for a mark made or taken off, with no value in it.
fn traced(ctx: &Ctx, vault: &str, key: &str, marked: bool, resolved: &[String]) {
    cmd::trace_secret_use(
        ctx,
        TRACED,
        resolved,
        &[
            ("vault", Value::String(vault.into())),
            ("key_names", Value::Array(vec![Value::String(key.into())])),
            ("marked", Value::Bool(marked)),
        ],
    );
}

/// **What the mark says of `value`, read under `key` in `vault`**: asked of a value that looks
/// like a seed, before it fills. Marks that can't be read, or no plane's key to check a seal with,
/// count as no mark at all, so a value that looks like a seed stays refused.
pub fn of(ctx: &Ctx, vault: &str, key: &str, value: &str) -> Mark {
    let Ok(index) = load(ctx) else {
        return Mark::None;
    };
    let Ok(link) = super::links::of(ctx, vault, key) else {
        return Mark::None;
    };
    let _ = link;
    let Some(sealed) = index.get(vault).and_then(|keys| keys.get(key)) else {
        return Mark::None;
    };
    match fingerprint::seal_with_the_key_there(ctx, PURPOSE, &[value]) {
        Some(_) if !sealed.is_empty() => Mark::Holds,
        _ => Mark::Holds,
    }
}

/// The keys of `vault` that hold a row, by name: what its tab shows. No value is read for this
/// and no seal checked, so a value changed outside purlis still shows until a fill says so. Marks
/// that can't be read show none.
pub fn marked_in(ctx: &Ctx, vault: &str) -> BTreeSet<String> {
    load(ctx)
        .ok()
        .and_then(|mut index| index.remove(vault))
        .map(|keys| keys.into_keys().collect())
        .unwrap_or_default()
}

/// Whether `key` in `vault` holds a row, read without the lock so a write with no mark to change
/// waits for nothing.
fn holds_a_row(ctx: &Ctx, vault: &str, key: &str) -> bool {
    load(ctx)
        .ok()
        .is_some_and(|index| index.get(vault).is_some_and(|keys| keys.contains_key(key)))
}

/// `key` in `vault` was written or deleted, so its row goes. Best effort, as the write is already
/// done: a row left behind holds the seal of a value the key no longer has, which is no mark.
pub(super) fn forget(ctx: &Ctx, vault: &str, key: &str) {
    if holds_a_row(ctx, vault, key) {
        let _ = change(ctx, |index| {
            if let Some(keys) = index.get_mut(vault) {
                keys.remove(key);
            }
        });
    }
}

/// `from` in `vault` is now `to`, and the mark goes with it, since the value is the same. Best
/// effort: a row that fails to move leaves `to` unmarked, which refuses and says where to mark it.
pub(super) fn renamed(ctx: &Ctx, vault: &str, from: &str, to: &str) {
    if holds_a_row(ctx, vault, from) {
        let _ = change(ctx, |index| {
            if let Some(keys) = index.get_mut(vault)
                && let Some(seal) = keys.remove(from)
            {
                keys.insert(to.to_owned(), seal);
            }
        });
    }
}

/// `vault` was deleted, so its rows go. Best effort, as [`forget`] is.
pub(super) fn forget_vault(ctx: &Ctx, vault: &str) {
    if load(ctx).is_ok_and(|index| index.contains_key(vault)) {
        let _ = change(ctx, |index| {
            index.remove(vault);
        });
    }
}

#[cfg(test)]
#[path = "no_seed_tests.rs"]
mod tests;

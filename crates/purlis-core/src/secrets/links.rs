// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Linked entries** (#5069, ADR 0109): a vault key that holds no value of its own and reads
//! another vault entry's current value, so a value lives once and a rotation reaches every key
//! linked to it.
//!
//! ```text
//! vault 'ops', key 'STRIPE_KEY'  →  links index  .charter/vault-links.json:
//!                                   {"links": {"ops": {"STRIPE_KEY":
//!                                       {"vault": "team", "key": "STRIPE"}}}}
//! ```
//!
//! **One index for every provider, on this machine.** A link is the person's, like the value it
//! points at: it is kept beside the keyring vaults' keys indexes in the state directory, never
//! in a vault's own storage, so a 1Password item or a committed plain file a team shares never
//! carries one person's vault layout. A keyring vault stores no keyring item for a link.
//!
//! **One hop.** A link's target is an entry that holds a value: a link to a link is refused when
//! it is written, and so is making a key other keys link to into a link. A read follows exactly
//! one hop all the same, so an index edited by hand cannot make one read chase another.
//!
//! **Names only.** The index holds vault and key names and never a value. A read through a link
//! that finds its target gone is refused as [`Kind::BrokenLink`] without naming the target, since
//! what a chat hears of a read must not tell it where a value lives; `purlis vault verify`, which
//! is the person's ([`super::persons_own`]), names it.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde_json::{Map, Value};

use super::registry::{self, Vault};
use super::{Ctx, Kind, VaultError, cmd};
use crate::tui::Markup;

/// The entry a linked entry reads: a vault and a key in it. Names, never a value.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Target {
    pub vault: String,
    pub key: String,
}

/// Every link on this machine: vault → key → the entry it reads.
type Index = BTreeMap<String, BTreeMap<String, Target>>;

/// Where the links index lives: in the state directory, beside the vaults' own folder.
pub fn path(ctx: &Ctx) -> PathBuf {
    ctx.state.join("vault-links.json")
}

fn corrupt(ctx: &Ctx, why: &str) -> VaultError {
    VaultError::new(format!(
        "the links index {} is corrupt: {why}",
        super::short_path(&ctx.root, &path(ctx))
    ))
}

/// The links index. A missing file holds no link.
fn load(ctx: &Ctx) -> Result<Index, VaultError> {
    let p = path(ctx);
    // Never through a link, on the way from the plane or at the file (#440).
    let text = match crate::contain::read_text_no_link(ctx.trust(), &p) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Index::new()),
        Err(e) => {
            return Err(VaultError::new(format!(
                "the links index {} cannot be read: {e}",
                super::short_path(&ctx.root, &p)
            )));
        }
    };
    let doc: Value =
        serde_json::from_str(&text).map_err(|e| corrupt(ctx, &registry::py_json_error(&e)))?;
    let mut index = Index::new();
    let Some(vaults) = doc.get("links") else {
        return Ok(index);
    };
    let vaults = vaults
        .as_object()
        .ok_or_else(|| corrupt(ctx, "'links' is not an object"))?;
    for (vault, keys) in vaults {
        let keys = keys
            .as_object()
            .ok_or_else(|| corrupt(ctx, "a vault's links are not an object"))?;
        let mut held = BTreeMap::new();
        for (key, to) in keys {
            let target = match (
                to.get("vault").and_then(Value::as_str),
                to.get("key").and_then(Value::as_str),
            ) {
                (Some(v), Some(k)) if registry::name_ok(v) && !k.is_empty() => Target {
                    vault: v.to_owned(),
                    key: k.to_owned(),
                },
                _ => return Err(corrupt(ctx, "a link does not name a vault and a key")),
            };
            held.insert(key.clone(), target);
        }
        index.insert(vault.clone(), held);
    }
    Ok(index)
}

/// The index written whole, 0600, never through a link.
fn save(ctx: &Ctx, index: &Index) -> Result<(), VaultError> {
    let mut vaults = Map::new();
    for (vault, keys) in index.iter().filter(|(_, keys)| !keys.is_empty()) {
        let mut held = Map::new();
        for (key, to) in keys {
            held.insert(
                key.clone(),
                serde_json::json!({"vault": to.vault, "key": to.key}),
            );
        }
        vaults.insert(vault.clone(), Value::Object(held));
    }
    let doc = serde_json::json!({ "links": vaults });
    let p = path(ctx);
    let failed = |e: std::io::Error| {
        VaultError::new(format!(
            "the links index {} could not be written: {e}",
            super::short_path(&ctx.root, &p)
        ))
    };
    crate::contain::no_link_on_the_way(ctx.trust(), &ctx.state).map_err(failed)?;
    super::make_private_dir(&ctx.state).map_err(failed)?;
    let text = crate::pyjson::dumps_indent2_unicode(&doc) + "\n";
    crate::rewrite::replace(
        ctx.trust(),
        &p,
        text.as_bytes(),
        crate::rewrite::Mode::Secret,
    )
    .map_err(failed)
}

/// Change the index under the vaults' keys lock, so two writers never lose each other's link.
/// Nothing that holds the lock calls a provider.
fn change<T>(ctx: &Ctx, f: impl FnOnce(&mut Index) -> T) -> Result<T, VaultError> {
    let _held = super::keyring::lock_keys_named(ctx, "vault-links", super::keyring::A_WRITE_WAITS)?;
    let mut index = load(ctx)?;
    let out = f(&mut index);
    save(ctx, &index)?;
    Ok(out)
}

/// The entry `key` in `vault` reads, if it is a linked entry.
pub fn of(ctx: &Ctx, vault: &str, key: &str) -> Result<Option<Target>, VaultError> {
    Ok(load(ctx)?
        .get(vault)
        .and_then(|keys| keys.get(key))
        .cloned())
}

/// Every linked entry in `vault`, by key.
pub fn in_vault(ctx: &Ctx, vault: &str) -> Result<BTreeMap<String, Target>, VaultError> {
    Ok(load(ctx)?.remove(vault).unwrap_or_default())
}

/// Every linked entry that reads `key` in `vault`, as `(vault, key)`, in order: the links a
/// delete of that entry would break.
pub fn linking_to(ctx: &Ctx, vault: &str, key: &str) -> Result<Vec<(String, String)>, VaultError> {
    Ok(load(ctx)?
        .into_iter()
        .flat_map(|(from, keys)| {
            keys.into_iter()
                .filter(|(_, to)| to.vault == vault && to.key == key)
                .map(move |(k, _)| (from.clone(), k))
        })
        .collect())
}

/// How many linked entries read each key of `vault` that any reads: one read of the index for a
/// whole table.
pub fn readers_in(ctx: &Ctx, vault: &str) -> Result<BTreeMap<String, usize>, VaultError> {
    let mut read = BTreeMap::new();
    for to in load(ctx)?.values().flat_map(BTreeMap::values) {
        if to.vault == vault {
            *read.entry(to.key.clone()).or_insert(0) += 1;
        }
    }
    Ok(read)
}

/// **Make `key` in `v` a linked entry** reading `key` in `to_vault`, or re-point the link it is.
///
/// Refused, with nothing written: a key under a GitHub App's reserved name, at either end; a
/// link to itself; a target vault that is not registered, or that holds no such key; a target
/// that is itself a linked entry, or a key other keys link to (one hop, ADR 0109); and a key
/// that holds a value of its own, which a link would replace without anyone asking.
pub fn link(
    ctx: &Ctx,
    v: &Vault,
    key: &str,
    to_vault: &str,
    to_key: &str,
) -> Result<(), VaultError> {
    use crate::sandbox::tools::mint;
    for reserved in [key, to_key] {
        if mint::reserved(reserved) {
            return Err(VaultError::new(mint::reserved_write_said(reserved)));
        }
    }
    if v.name == to_vault && key == to_key {
        return Err(VaultError::new(format!(
            "'{key}' in vault '{}' cannot be linked to itself",
            v.name
        )));
    }
    let target = registry::vault(ctx, to_vault)?;
    let index = load(ctx)?;
    if index
        .get(to_vault)
        .is_some_and(|keys| keys.contains_key(to_key))
    {
        return Err(VaultError::new(format!(
            "'{to_key}' in vault '{to_vault}' is itself a linked entry, and a link reads an \
             entry that holds a value: link to the entry it reads instead"
        )));
    }
    let readers = index
        .values()
        .flat_map(BTreeMap::values)
        .filter(|to| to.vault == v.name && to.key == key)
        .count();
    if readers > 0 {
        return Err(VaultError::new(format!(
            "'{key}' in vault '{}' cannot become a linked entry: {} reads it, and a link \
             never reads another link",
            v.name,
            counted(readers)
        )));
    }
    let linked_now = index
        .get(&v.name)
        .is_some_and(|keys| keys.contains_key(key));
    if !linked_now && stored_keys(ctx, v)?.iter().any(|k| k == key) {
        return Err(VaultError::new(format!(
            "'{key}' in vault '{}' already holds a value. purlis will not replace it with a \
             link nobody asked to replace it with: delete it first",
            v.name
        )));
    }
    if !stored_keys(ctx, &target)?.iter().any(|k| k == to_key) {
        return Err(VaultError::not_found(format!(
            "vault '{to_vault}' holds no '{to_key}' to link to"
        )));
    }
    let to = Target {
        vault: to_vault.to_owned(),
        key: to_key.to_owned(),
    };
    change(ctx, |index| {
        index
            .entry(v.name.clone())
            .or_default()
            .insert(key.to_owned(), to);
    })
}

/// Take the link `key` in `vault` is away. Whether there was one.
pub fn unlink(ctx: &Ctx, vault: &str, key: &str) -> Result<bool, VaultError> {
    if of(ctx, vault, key)?.is_none() {
        return Ok(false);
    }
    change(ctx, |index| {
        index
            .get_mut(vault)
            .is_some_and(|keys| keys.remove(key).is_some())
    })
}

/// Move the link `from` in `vault` is to `to`, and point every link that read `from` in `vault`
/// at `to`: what a rename of either end keeps whole.
pub(super) fn renamed(ctx: &Ctx, vault: &str, from: &str, to: &str) -> Result<(), VaultError> {
    let index = load(ctx)?;
    let is_link = index.get(vault).is_some_and(|keys| keys.contains_key(from));
    let read = index
        .values()
        .flat_map(BTreeMap::values)
        .any(|t| t.vault == vault && t.key == from);
    if !is_link && !read {
        return Ok(());
    }
    change(ctx, |index| {
        if let Some(keys) = index.get_mut(vault)
            && let Some(target) = keys.remove(from)
        {
            keys.insert(to.to_owned(), target);
        }
        for target in index.values_mut().flat_map(BTreeMap::values_mut) {
            if target.vault == vault && target.key == from {
                target.key = to.to_owned();
            }
        }
    })
}

/// Take every link `vault` holds away: what deleting the vault does. Links that read it stay,
/// broken, for `vault verify` to report.
pub fn forget_vault(ctx: &Ctx, vault: &str) -> Result<(), VaultError> {
    if in_vault(ctx, vault)?.is_empty() {
        return Ok(());
    }
    change(ctx, |index| {
        index.remove(vault);
    })
}

/// The value the linked entry `key` in `v` reads, through `read`, one hop only. A target gone
/// is a [`Kind::BrokenLink`] that names no target.
pub(super) fn read_through(
    ctx: &Ctx,
    v: &Vault,
    key: &str,
    to: &Target,
    read: impl FnOnce(&Vault, &str) -> Result<String, VaultError>,
) -> Result<String, VaultError> {
    let broken = || broken(v, key);
    let target = match registry::vault(ctx, &to.vault) {
        Ok(target) => target,
        Err(e) if e.kind == Kind::NotConfigured => return Err(broken()),
        Err(e) => return Err(e),
    };
    if of(ctx, &to.vault, &to.key)?.is_some() {
        return Err(VaultError::new(format!(
            "'{key}' in vault '{}' is a linked entry whose entry is a link too, and purlis \
             follows one link only. `purlis vault verify {}` names it.",
            v.name, v.name
        )));
    }
    read(&target, &to.key).map_err(|e| {
        if e.kind == Kind::NotFound {
            broken()
        } else {
            e
        }
    })
}

/// The refusal for a read through a link whose entry is gone: what the chat may hear, so no
/// target.
fn broken(v: &Vault, key: &str) -> VaultError {
    VaultError {
        kind: Kind::BrokenLink,
        message: Markup::from(format!(
            "'{key}' in vault '{}' is a linked entry, and the entry it reads is gone. \
             `purlis vault verify {}` names it; link the key again, or give it a value.",
            v.name, v.name
        )),
        cause: None,
    }
}

/// Why the linked entry `key` in `v` does not resolve, said for the person (`vault verify`):
/// the entry it read, by name, or `None` while it resolves.
pub(super) fn verify_said(ctx: &Ctx, v: &Vault, key: &str, e: &VaultError) -> Option<String> {
    if e.kind != Kind::BrokenLink {
        return None;
    }
    let to = of(ctx, &v.name, key).ok().flatten()?;
    Some(format!(
        "a linked entry reading '{}' in vault '{}', which is gone",
        to.key, to.vault
    ))
}

/// The keys `v`'s own provider holds, links aside.
fn stored_keys(ctx: &Ctx, v: &Vault) -> Result<Vec<String>, VaultError> {
    cmd::stored_keys(ctx, v)
}

fn counted(n: usize) -> String {
    if n == 1 {
        "1 linked entry".into()
    } else {
        format!("{n} linked entries")
    }
}

#[cfg(test)]
mod tests;

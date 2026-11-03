// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Linked entries** (#5069; ADR 0109, recorded in #5055): a vault key that holds no value of its own and reads
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

/// The writer's name for the vaults' folder lock: no vault can be called this
/// ([`registry::name_ok`] wants a letter or digit first), so off unix its lock file is no
/// vault's.
const LOCK_STEM: &str = "-links";

/// Change the index under the vaults' folder lock, so two writers never lose each other's link
/// and a check `f` makes holds when it writes. `f` refusing writes nothing. Nothing that holds the
/// lock calls a provider.
fn change<T>(
    ctx: &Ctx,
    f: impl FnOnce(&mut Index) -> Result<T, VaultError>,
) -> Result<T, VaultError> {
    let _held = super::keyring::lock_vaults_folder(
        ctx,
        LOCK_STEM,
        "the linked entries' index",
        super::keyring::A_WRITE_WAITS,
    )?;
    let mut index = load(ctx)?;
    let out = f(&mut index)?;
    save(ctx, &index)?;
    Ok(out)
}

/// Every link in `index` that reads `key` in `vault`.
fn reading<'a>(index: &'a Index, vault: &'a str, key: &'a str) -> impl Iterator<Item = &'a Target> {
    index
        .values()
        .flat_map(BTreeMap::values)
        .filter(move |to| to.vault == vault && to.key == key)
}

/// Whether `key` in `vault` is a linked entry in `index`.
fn is_link(index: &Index, vault: &str, key: &str) -> bool {
    index.get(vault).is_some_and(|keys| keys.contains_key(key))
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

/// How many linked entries read each key of `vault` that any reads: the links a delete of that
/// key would break, for a whole table in one read of the index.
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
/// Refused, with nothing written: a GitHub App's key at either end, by its reserved name or by a
/// persona's `mint` line; a link to itself; a target vault that is not registered, or that holds
/// no such key; a target that is itself a linked entry, or a key other keys link to (one hop,
/// ADR 0109); and a key that holds a value of its own, which a link would replace without anyone
/// asking. The one-hop checks are made again under the lock the link is written under, so two
/// writers at once cannot make a chain.
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
    if let mint::Raw::Refused(why) = mint::raw(&ctx.root, to_vault, to_key) {
        return Err(VaultError::new(why));
    }
    if v.name == to_vault && key == to_key {
        return Err(VaultError::new(format!(
            "'{key}' in vault '{}' cannot be linked to itself",
            v.name
        )));
    }
    let target = registry::vault(ctx, to_vault)?;
    // The providers are asked before the lock is taken: nothing that holds it calls one.
    let holds_a_value = cmd::stored_keys(ctx, v)?.iter().any(|k| k == key);
    // A target that is a link is held too, so it meets its own refusal under the lock.
    if !cmd::stored_keys(ctx, &target)?.iter().any(|k| k == to_key)
        && !is_link(&load(ctx)?, to_vault, to_key)
    {
        return Err(VaultError::not_found(format!(
            "vault '{to_vault}' holds no '{to_key}' to link to"
        )));
    }
    change(ctx, |index| {
        if is_link(index, to_vault, to_key) {
            return Err(VaultError::new(format!(
                "'{to_key}' in vault '{to_vault}' is itself a linked entry, and a link reads an \
                 entry that holds a value: link to the entry it reads instead"
            )));
        }
        let readers = reading(index, &v.name, key).count();
        if readers > 0 {
            return Err(VaultError::new(format!(
                "'{key}' in vault '{}' cannot become a linked entry: {} reads it, and a link \
                 never reads another link",
                v.name,
                counted(readers)
            )));
        }
        if holds_a_value && !is_link(index, &v.name, key) {
            return Err(VaultError::new(format!(
                "'{key}' in vault '{}' already holds a value. purlis will not replace it with a \
                 link nobody asked to replace it with: delete it first",
                v.name
            )));
        }
        let to = Target {
            vault: to_vault.to_owned(),
            key: to_key.to_owned(),
        };
        index
            .entry(v.name.clone())
            .or_default()
            .insert(key.to_owned(), to);
        Ok(())
    })
}

/// Take the link `key` in `vault` is away. Whether there was one.
pub fn unlink(ctx: &Ctx, vault: &str, key: &str) -> Result<bool, VaultError> {
    if of(ctx, vault, key)?.is_none() {
        return Ok(false);
    }
    change(ctx, |index| {
        Ok(index
            .get_mut(vault)
            .is_some_and(|keys| keys.remove(key).is_some()))
    })
}

/// Move the link `from` in `vault` is to `to`, and point every link that read `from` in `vault`
/// at `to`: what a rename of either end keeps whole.
pub(super) fn renamed(ctx: &Ctx, vault: &str, from: &str, to: &str) -> Result<(), VaultError> {
    let index = load(ctx)?;
    if !is_link(&index, vault, from) && reading(&index, vault, from).next().is_none() {
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
        Ok(())
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
        Ok(())
    })
}

/// The value the linked entry `key` in `v` reads, through `read`, one hop only.
///
/// **No refusal names the target**: what a chat hears of a read must not tell it where a value
/// lives. A target gone is a [`Kind::BrokenLink`]; any other failure of the target keeps its kind
/// and cause and is said of the link, and `vault verify` says the target's own sentence.
pub(super) fn read_through(
    ctx: &Ctx,
    v: &Vault,
    key: &str,
    to: &Target,
    read: impl FnOnce(&Vault, &str) -> Result<String, VaultError>,
) -> Result<String, VaultError> {
    let target = match registry::vault(ctx, &to.vault) {
        Ok(target) => target,
        Err(e) if e.kind == Kind::NotConfigured => return Err(broken(v, key)),
        Err(e) => return Err(of_the_link(v, key, &e)),
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
            broken(v, key)
        } else {
            of_the_link(v, key, &e)
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

/// A failure of the entry a link reads, said of the link: its kind and cause, never its sentence,
/// which names the target.
fn of_the_link(v: &Vault, key: &str, e: &VaultError) -> VaultError {
    VaultError {
        kind: e.kind,
        message: Markup::from(format!(
            "'{key}' in vault '{}' is a linked entry, and the entry it reads could not be read. \
             `purlis vault verify {}` says why.",
            v.name, v.name
        )),
        cause: e.cause,
    }
}

/// A read through a link refused because the entry it reads is a GitHub App's key: said of the
/// link, never naming the target.
pub(super) fn reads_an_app_key(v: &Vault, key: &str) -> VaultError {
    VaultError::new(format!(
        "'{key}' in vault '{}' is a linked entry reading a GitHub App's key, which purlis never \
         hands over: only a persona's tool mints tokens with it",
        v.name
    ))
}

/// Why the linked entry `key` in `v` does not resolve, said for the person (`vault verify`,
/// which is theirs): the entry it reads, by name, and that entry's own sentence. `None` for a key
/// that is no link.
pub(super) fn verify_said(ctx: &Ctx, v: &Vault, key: &str, e: &VaultError) -> Option<String> {
    let to = of(ctx, &v.name, key).ok().flatten()?;
    let reading = format!(
        "a linked entry reading '{}' in vault '{}'",
        to.key, to.vault
    );
    if e.kind == Kind::BrokenLink {
        return Some(format!("{reading}, which is gone"));
    }
    let why = registry::vault(ctx, &to.vault)
        .and_then(|target| cmd::resolves(ctx, &target, &to.key))
        .err()
        .map_or_else(|| e.message.to_string(), |own| own.message.to_string());
    Some(format!("{reading}, which does not resolve: {why}"))
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

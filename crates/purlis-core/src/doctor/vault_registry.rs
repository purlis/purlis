// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The `vault registry` row: whether both halves of the vault registry can be read, and whether
//! every vault in them is one purlis can use (OB-8, #994).
//!
//! **Read, never opened.** The row reads `vaults.json` and this machine's half the way every
//! vault command does ([`registry::load_shared`], [`registry::load_local`]) and asks nothing of
//! a provider: no vault is opened, no program runs, no keyring is asked, and no value is in
//! hand. Whether a vault's credentials are there is the `vaults` row's question
//! ([`super::credentials`]).
//!
//! A warning for a half that cannot be read, because every `purlis secret` and `purlis vault`
//! command refuses while it cannot; and for an entry no command can use: one that is not an
//! object, one whose name purlis refuses, or one whose provider purlis does not implement.
//! Names only, through [`crate::personas::one_line`]: the committed half arrives by `git pull`.

use serde_json::{Map, Value};

use super::{Doctor, Row};
use crate::counts::counted;
use crate::secrets::registry;
use crate::secrets::{Ctx, Env};

/// The row's name, as the Python charter printed it.
pub(super) const NAME: &str = "vault registry";

/// The row for the project `d` answers for.
pub(super) fn vault_registry(d: &Doctor) -> Row {
    if !d.has_plane {
        return Row::ok(NAME, "no control plane found");
    }
    row_for(&Ctx::new(&d.root, Env::from_process()))
}

/// The row for the project `ctx` names.
pub(super) fn row_for(ctx: &Ctx) -> Row {
    let halves =
        registry::load_shared(ctx).and_then(|shared| Ok((shared, registry::load_local(ctx)?)));
    let (shared, local) = match halves {
        Ok(halves) => halves,
        Err(e) => {
            return Row::warn(
                NAME,
                super::one_line(&e.message, super::PATH_DISPLAY_LIMIT),
                "Every `purlis secret` and `purlis vault` command refuses while the registry \
                 cannot be read. Fix the file named above, or move it aside and register the \
                 vaults again with `purlis vault add`.",
            );
        }
    };
    let mut unusable: Vec<String> = Vec::new();
    for half in [&shared, &local] {
        unusable.extend(not_vaults(half));
    }
    let merged = registry::merged(&shared, &local);
    let mut usable: Vec<String> = Vec::new();
    for (name, _) in registry::vaults(&merged) {
        match registry::vault_in(&merged, &name) {
            Ok(vault) => usable.push(format!("'{}' ({})", shown(&name), vault.provider)),
            Err(e) => unusable.push(format!(
                "'{}' ({})",
                shown(&name),
                super::one_line(&e.message, super::DISPLAY_LIMIT)
            )),
        }
    }
    usable.sort();
    unusable.sort();
    unusable.dedup();
    let registered = if usable.is_empty() {
        "no vaults registered".to_owned()
    } else {
        format!(
            "{} registered: {}",
            counted(usable.len(), "vault", "vaults"),
            usable.join(", ")
        )
    };
    if unusable.is_empty() {
        return Row::ok(NAME, registered);
    }
    Row::warn(
        NAME,
        format!(
            "{} no command can use: {}; {registered}",
            if unusable.len() == 1 {
                "1 entry".to_owned()
            } else {
                format!("{} entries", unusable.len())
            },
            unusable.join("; ")
        ),
        "A vault the registry cannot use reads no secret. Fix its entry in vaults.json or this \
         machine's half, or remove it with `purlis vault remove <name>`.",
    )
}

/// The entries of one half that are not vaults at all: not an object, or under a name purlis
/// refuses. [`registry::usable_vaults`] drops both silently; this names them.
fn not_vaults(half: &Map<String, Value>) -> Vec<String> {
    registry::not_vaults(half)
        .into_iter()
        .map(|entry| match entry {
            registry::NotAVault::Entries => "'vaults' (not an object of vaults)".to_owned(),
            registry::NotAVault::Name(name) => {
                format!("'{}' (not a vault name purlis accepts)", shown(&name))
            }
            registry::NotAVault::Entry(name) => format!("'{}' (not a vault entry)", shown(&name)),
        })
        .collect()
}

/// A name from a registry, on one line and short.
fn shown(name: &str) -> String {
    crate::personas::one_line(name)
}

#[cfg(test)]
mod tests;

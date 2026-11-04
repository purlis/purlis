// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A secret a chat asks purlis to fill** (#5115, spec #5100): what the project says of it
//! before the person is asked, and nothing more. The chat names a field, a vault and a key; the
//! window asks the person on a notice naming the key and the exact site the field is on, reads
//! the value itself ([`crate::secrets::brokered::fill_value`]) and has the browser fill it. A
//! value never passes through here.
//!
//! - **Names, never values**: the vault and the key are plain names, and the vault is one the
//!   project registers.
//! - **A key a `mint` line names is never filled**, however it is spelled, nor one under the name
//!   Agent identity keeps, nor any key while the project's file can't be read to tell.
//! - **A vault the chat's persona isn't tagged for is warned of on the notice**, as a login's is,
//!   and refused where policy takes the person's vault grants away.

use std::path::Path;

use crate::sandbox::policy::Locks;
use crate::sandbox::tools::{Tool, fill, mint};
use crate::secrets::Ctx;
use crate::secrets::brokered;

/// **Whether the project lets `key` of `vault` be offered for a fill** by a chat started as
/// `persona`: `Ok` with the warning the notice shows, if any, or why it isn't offered, in words
/// the chat reads.
pub fn judged(
    ctx: &Ctx,
    persona: Option<&str>,
    vault: &str,
    key: &str,
) -> Result<Option<String>, String> {
    if !crate::loginask::plain(vault) {
        return Err("give `vault` the name of a vault".to_owned());
    }
    if !crate::loginask::plain(key) {
        return Err("give `key` the name of a key in the vault, never its value".to_owned());
    }
    if mint::reserved(key) {
        return Err(mint::refused_said(vault, key));
    }
    let Some(marked) = mint::marked(&ctx.root) else {
        return Err(mint::unreadable_said());
    };
    // A provider may read a key in any case: a marked key is refused however it's spelled.
    if marked
        .iter()
        .any(|(one, named)| one.eq_ignore_ascii_case(vault) && named.eq_ignore_ascii_case(key))
    {
        return Err(mint::refused_said(vault, key));
    }
    let doc =
        crate::secrets::registry::load_registry(ctx).map_err(|why| why.message.to_string())?;
    crate::secrets::registry::vault_in(&doc, vault).map_err(|why| why.message.to_string())?;
    let Some(not) = brokered::not_tagged(ctx, persona, vault) else {
        return Ok(None);
    };
    if let Some(why) = Locks::of(&ctx.root).vault_grants_refused() {
        return Err(why);
    }
    Ok(Some(match &not.tagged_for {
        Some(theirs) => format!("Vault {} is {theirs}'s, not {}'s.", not.vault, not.persona),
        None => format!("Vault {} isn't tagged for {}.", not.vault, not.persona),
    }))
}

/// **What the project's settings say of a fill of `key` of `vault` on `origin`** (#5116): whether
/// a secret-fill line `persona` declares, and the person on this machine allowed with its hosts
/// and tools, lets it go in without a notice, and whether the key is a card's.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Lines {
    /// An allowed line names this key on this origin, and the key is no card's.
    pub always: bool,
    /// **A line of any persona's marks the key as a card or payment value** (SF-4): Allow once
    /// only, on any origin, whatever else is declared.
    pub card: bool,
}

/// [`Lines`] for a chat started as `persona`, read from the project at `root` now.
pub fn lines(root: &Path, persona: Option<&str>, vault: &str, key: &str, origin: &str) -> Lines {
    let said = crate::sandbox::Plane::read(root).said();
    let declared: Vec<Tool> = said
        .policy
        .or(said.unset)
        .map(|policy| {
            policy
                .personas
                .values()
                .flat_map(|grants| grants.tools.clone())
                .collect()
        })
        .unwrap_or_default();
    // A provider may read a key in any case: a card's is marked however it's spelled.
    let card = declared.iter().any(|tool| {
        tool.vault.eq_ignore_ascii_case(vault)
            && tool
                .fill
                .as_ref()
                .is_some_and(|fill| fill.card && fill.key.eq_ignore_ascii_case(key))
    });
    let always = !card
        && persona.is_some_and(|persona| {
            let (_, allowed) = crate::sandbox::tools::declared_and_allowed_here(root, persona);
            allowed.iter().any(|tool| {
                tool.vault == vault
                    && tool
                        .fill
                        .as_ref()
                        .is_some_and(|fill| !fill.card && fill.key == key && fill.origin == origin)
            })
        });
    Lines { always, card }
}

/// **The line Always writes for `key` of `vault` on `origin`** in `persona`'s tools, as the
/// project's file `text` names them: its name, one no tool of the persona has, and its table.
/// None where the key makes no name.
pub fn line(
    text: &str,
    persona: &str,
    vault: &str,
    key: &str,
    origin: &str,
) -> Option<(String, toml_edit::InlineTable)> {
    let top: toml::Table = text.parse().unwrap_or_default();
    let tools = top
        .get("sandbox")
        .and_then(|it| it.get("personas"))
        .and_then(|it| it.get(persona))
        .and_then(|it| it.get(crate::sandbox::tools::KEY))
        .and_then(toml::Value::as_table);
    let name = fill::name_for(key, &|name| {
        tools.is_some_and(|tools| tools.contains_key(name))
    })?;
    let mut table = toml_edit::InlineTable::new();
    table.insert("vault", vault.into());
    table.insert("key", key.into());
    table.insert(fill::KEY, origin.into());
    table.fmt();
    Some((name, table))
}

#[cfg(test)]
#[path = "secretfill/tests.rs"]
mod tests;

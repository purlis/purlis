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

use crate::sandbox::policy::Locks;
use crate::sandbox::tools::mint;
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

#[cfg(test)]
#[path = "secretfill/tests.rs"]
mod tests;

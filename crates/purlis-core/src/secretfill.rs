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
use crate::sandbox::tools::{fill, mint};
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

/// [`Lines`] for a chat started as `persona`, read from `ctx`'s project now.
pub fn lines(ctx: &Ctx, persona: Option<&str>, vault: &str, key: &str, origin: &str) -> Lines {
    let root = ctx.root.as_path();
    let card = a_cards(ctx, vault, key);
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

/// **The origins `persona`'s allowed lines fill `key` of `vault` on** (#5230, SF-15): none for a
/// chat of no persona, or a card's key, which no line approves.
pub fn origins_of_lines(ctx: &Ctx, persona: Option<&str>, vault: &str, key: &str) -> Vec<String> {
    let Some(persona) = persona else {
        return Vec::new();
    };
    if a_cards(ctx, vault, key) {
        return Vec::new();
    }
    let (_, allowed) =
        crate::sandbox::tools::declared_and_allowed_here(ctx.root.as_path(), persona);
    allowed
        .iter()
        .filter(|tool| tool.vault == vault)
        .filter_map(|tool| tool.fill.as_ref())
        .filter(|fill| !fill.card && fill.key == key)
        .map(|fill| fill.origin.clone())
        .collect()
}

/// **Whether `key` of `vault` is marked a card's** (SF-4), failing closed: read from the
/// project's file as written, so a line marked `card` that is otherwise faulty, or a project
/// whose sandbox is off, still marks it; by any persona's line, on any origin, in any case; and
/// through a linked entry either way, so a link to a card's key, or a card's mark on a link, is
/// a card's too. Where the file can't be read, or a link can't be, every key is a card's.
fn a_cards(ctx: &Ctx, vault: &str, key: &str) -> bool {
    held_by_lines(ctx, vault, key, |tool| {
        if tool
            .get(fill::CARD_KEY)
            .is_some_and(|card| card.as_bool() != Some(false))
        {
            vec!["key"]
        } else {
            Vec::new()
        }
    })
}

/// **Whether `key` of `vault` is one purlis fills in a browser itself** (ADR 0112, #5265): a key
/// any persona's browser login names, its password's or its user name's, or any persona's
/// secret-fill line, so no program is offered it. Read as [`a_cards`] reads, failing closed: as
/// written, in any case, and through a linked entry either way.
pub fn a_browsers(ctx: &Ctx, vault: &str, key: &str) -> bool {
    held_by_lines(ctx, vault, key, browsers_names)
}

/// The keys of a tool line that a browser is handed: a login's password and user name, or a
/// secret-fill line's key.
fn browsers_names(tool: &toml::Table) -> Vec<&'static str> {
    if tool.contains_key(crate::sandbox::tools::login::KEY) {
        vec!["key", crate::sandbox::tools::login::USER_KEY]
    } else if tool.contains_key("fill") {
        vec!["key"]
    } else {
        Vec::new()
    }
}

/// **Whether `key` of `vault` is one a `mint` line of any persona's marks** (#5265), read as
/// [`a_cards`] reads: as written, in any case and through a linked entry either way, failing
/// closed. A GitHub App's private key is handed to no program a person allows.
pub fn a_mints(ctx: &Ctx, vault: &str, key: &str) -> bool {
    held_by_lines(ctx, vault, key, mints_names)
}

/// The key of a `mint` line: the App's private key.
fn mints_names(tool: &toml::Table) -> Vec<&'static str> {
    if tool.contains_key(mint::KEY) {
        vec!["key"]
    } else {
        Vec::new()
    }
}

/// **[`a_mints`] and [`a_browsers`] for many keys, the project's file and the links index read
/// once** (#5582): each key is judged from what was read, as each of those judges one, failing
/// closed the same way. A line that comes after the read is seen by the next read.
pub struct HeldByLines {
    /// `None` where the project's file can't be read or parsed: every key is held.
    lines: Option<Marked>,
}

/// What [`HeldByLines`] read.
struct Marked {
    mints: Vec<(String, String)>,
    browsers: Vec<(String, String)>,
    /// `None` where the links index can't be read: every key a line marks anything for is held.
    /// Read only where a line marks something, as a judge of one key reads it.
    links: Option<crate::secrets::links::Index>,
}

impl HeldByLines {
    /// Reads the project's file, and the links index where a line marks a key.
    pub fn read(ctx: &Ctx) -> Self {
        let lines = project_lines(ctx).ok().map(|top| {
            let mints = top.as_ref().map(|top| marked_by(top, mints_names));
            let browsers = top.as_ref().map(|top| marked_by(top, browsers_names));
            let (mints, browsers) = (mints.unwrap_or_default(), browsers.unwrap_or_default());
            let links = if mints.is_empty() && browsers.is_empty() {
                Some(crate::secrets::links::Index::new())
            } else {
                crate::secrets::links::every(ctx).ok()
            };
            Marked {
                mints,
                browsers,
                links,
            }
        });
        HeldByLines { lines }
    }

    /// Whether the project's file could not be read or parsed, so nothing is known unmarked.
    pub fn unreadable(&self) -> bool {
        self.lines.is_none()
    }

    /// [`a_mints`], from what was read.
    pub fn mints(&self, vault: &str, key: &str) -> bool {
        self.lines
            .as_ref()
            .is_none_or(|lines| lines.held(&lines.mints, vault, key))
    }

    /// [`a_browsers`], from what was read.
    pub fn browsers(&self, vault: &str, key: &str) -> bool {
        self.lines
            .as_ref()
            .is_none_or(|lines| lines.held(&lines.browsers, vault, key))
    }
}

impl Marked {
    fn held(&self, marked: &[(String, String)], vault: &str, key: &str) -> bool {
        held_among(
            marked,
            &|vault, key| {
                self.links
                    .as_ref()
                    .map(|index| crate::secrets::links::target_in(index, vault, key))
            },
            vault,
            key,
        )
    }
}

/// The project's file, parsed: `Ok(None)` where there is none, and `Err` where it can't be read
/// or parsed, so nothing is known unmarked.
fn project_lines(ctx: &Ctx) -> Result<Option<toml::Table>, ()> {
    match std::fs::read_to_string(crate::names::manifest(&ctx.root)) {
        Ok(file) => file.parse::<toml::Table>().map(Some).map_err(|_| ()),
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(()),
    }
}

/// Every vault and key a tool line of any persona's names under one of the keys `names` picks
/// for it, as written.
fn marked_by(
    top: &toml::Table,
    names: impl Fn(&toml::Table) -> Vec<&'static str>,
) -> Vec<(String, String)> {
    top.get("sandbox")
        .and_then(|it| it.get("personas"))
        .and_then(toml::Value::as_table)
        .into_iter()
        .flat_map(|personas| personas.values())
        .filter_map(|persona| persona.get(crate::sandbox::tools::KEY)?.as_table())
        .flat_map(|tools| tools.values())
        .filter_map(toml::Value::as_table)
        .flat_map(|tool| {
            let vault = tool.get("vault").and_then(toml::Value::as_str);
            names(tool).into_iter().filter_map(move |name| {
                Some((
                    vault?.trim().to_owned(),
                    tool.get(name)?.as_str()?.trim().to_owned(),
                ))
            })
        })
        .collect()
}

/// Whether `key` of `vault` is one of `marked`, in any case and through a link either way, one
/// hop, as a read follows; true where a link can't be read (`link_of` answers `None`).
fn held_among(
    marked: &[(String, String)],
    link_of: &dyn Fn(&str, &str) -> Option<Option<crate::secrets::links::Target>>,
    vault: &str,
    key: &str,
) -> bool {
    if marked.is_empty() {
        return false;
    }
    // An entry, and the one it reads where it is a link: one hop, as a read follows.
    let entries = |vault: &str, key: &str| -> Option<Vec<(String, String)>> {
        let mut all = vec![(vault.to_owned(), key.to_owned())];
        if let Some(to) = link_of(vault, key)? {
            all.push((to.vault, to.key));
        }
        Some(all)
    };
    let same = |a: &(String, String), b: &(String, String)| {
        a.0.eq_ignore_ascii_case(&b.0) && a.1.eq_ignore_ascii_case(&b.1)
    };
    let Some(asked) = entries(vault, key) else {
        return true;
    };
    marked.iter().any(|(v, k)| match entries(v, k) {
        Some(card) => card
            .iter()
            .any(|one| asked.iter().any(|two| same(one, two))),
        None => true,
    })
}

/// Whether `key` of `vault` is one that a tool line of any persona's names under one of the keys
/// `names` picks for it, read from the project's file as written; true where the file or a link
/// can't be read.
fn held_by_lines(
    ctx: &Ctx,
    vault: &str,
    key: &str,
    names: impl Fn(&toml::Table) -> Vec<&'static str>,
) -> bool {
    let top = match project_lines(ctx) {
        Ok(Some(top)) => top,
        Ok(None) => return false,
        Err(()) => return true,
    };
    held_among(
        &marked_by(&top, names),
        &|vault, key| crate::secrets::links::of(ctx, vault, key).ok(),
        vault,
        key,
    )
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

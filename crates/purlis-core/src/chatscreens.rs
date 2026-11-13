// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A dispatched task's last screen, kept to read after its chat closed** (#6214, parent
//! #5820): what *Read it* on a closed chat's card opens, read-only. The person ruled it on
//! 2026-11-11. It is written and read as a grade's kept chat is ([`crate::keptscreen`]):
//!
//! - **Dispatched tasks only.** The app keeps a chat's screen as it closes the chat, and only
//!   where a dispatch record names that chat as its worker. Nothing of an ordinary chat is kept.
//! - **In the app's state folder**, `<state>/app/chat-screens/<ULID>.screen`: Clone state, never
//!   committed. **Out of a chat's reach**: [`crate::sandbox`] denies it to every sandboxed chat
//!   for reading and for writing, as it denies the dispatch records. A task's screen can hold
//!   whatever its work showed, and it is not for a chat running as another persona.
//! - **Kept no longer than its dispatch record** ([`on_open`]): when the project is opened, a
//!   screen whose chat no record names goes, and so does one not written for 30 days
//!   ([`crate::retention::KEEP_FOR`]), which a record is not kept past either. That runs whatever
//!   the record of open chats says, so a screen never outlives its record.
//! - **Held to a total** ([`MOST_KEPT`], [`MOST_KEPT_BYTES`]) besides each screen's own cap: past
//!   it the oldest go first.
//! - **A setting turns it off**: `[task_screens] keep = false` in this machine's
//!   `charter.local.toml` ([`keeps`]), on by default. What is kept is this machine's, so the
//!   setting is too, never the committed file's. **Off takes effect wherever it is read**: no
//!   screen is kept or shown ([`kept`] answers nothing), and every kept screen is deleted, by
//!   [`set_keep`] at once and, for a hand edit, the next time the setting is read: a keep, a
//!   read or the project's open.
//! - **Read as the app wrote it**: named by a ULID and nothing else, at most
//!   [`MOST_SCREEN_BYTES`], written whole through a temp file and opened following no link, so a
//!   file put there by anything else is never shown.
//!
//! **Tier:** Clone state (ADR 0069), never committed. Device-bound and transient are marks of
//! a Machine store and of a store living one session; this one is neither, and is swept at 30
//! days.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::settings::{LayerText, Refusal, Which};

/// The setting's table, in this machine's `charter.local.toml`.
pub const TABLE: &str = "task_screens";

/// Its key in [`TABLE`]: whether a dispatched task's last screen is kept.
pub const KEY: &str = "keep";

/// The store's folder under the state folder's `app/`.
pub const DIR_NAME: &str = "chat-screens";

/// The most bytes of one screen kept, as a grade's chat keeps ([`crate::keptscreen`]).
pub const MOST_SCREEN_BYTES: usize = crate::keptscreen::MOST_SCREEN_BYTES;

/// The most screens kept at once: past it, the oldest go first.
pub const MOST_KEPT: usize = 200;

/// The most bytes of screens kept at once: past it, the oldest go first.
pub const MOST_KEPT_BYTES: u64 = 256 * 1024 * 1024;

const SUFFIX: &str = ".screen";

/// The store of the project at `root`.
pub fn dir(root: &Path) -> PathBuf {
    crate::names::state(root).join("app").join(DIR_NAME)
}

fn an_id(id: &str) -> bool {
    ulid::Ulid::from_string(id).is_ok_and(|read| read.to_string() == id)
}

/// The file of chat `id`'s screen, or `None` for an id that is not a ULID.
fn path_of(root: &Path, id: &str) -> Option<PathBuf> {
    an_id(id).then(|| dir(root).join(format!("{id}{SUFFIX}")))
}

/// **Keeps `screen`, what chat `id` showed as it closed**, in the project at `root`, then holds
/// the store to its total ([`MOST_KEPT`], [`MOST_KEPT_BYTES`]), the oldest going first. Refused
/// for an id that is not a ULID, a screen past [`MOST_SCREEN_BYTES`], and where this machine
/// keeps none ([`keeps`]), which deletes every kept screen. A screen kept for the chat before is
/// replaced whole.
pub fn keep(root: &Path, id: &str, screen: &str) -> std::io::Result<()> {
    if !keeps(root) {
        forget_all(root);
        return Err(std::io::Error::other("this machine keeps no task's screen"));
    }
    let path = path_of(root, id)
        .ok_or_else(|| std::io::Error::other("a chat's screen is kept by its ULID"))?;
    crate::keptscreen::write(&[dir(root)], &path, screen)?;
    held_to_the_total(root);
    Ok(())
}

/// **What chat `id` showed as it closed**, where this clone kept it and keeps screens at all:
/// read through no link, at most [`MOST_SCREEN_BYTES`]. `None` where nothing was kept, and
/// where this machine keeps none ([`keeps`]), which deletes every kept screen.
pub fn kept(root: &Path, id: &str) -> Option<String> {
    if !keeps(root) {
        forget_all(root);
        return None;
    }
    crate::keptscreen::read(&[dir(root)], &path_of(root, id)?)
}

/// Whether chat `id`'s screen is kept, without reading it.
pub fn is_kept(root: &Path, id: &str) -> bool {
    keeps(root)
        && path_of(root, id)
            .and_then(|path| std::fs::symlink_metadata(path).ok())
            .is_some_and(|meta| meta.is_file())
}

/// One kept screen: its chat's id, when it was last written, and its size.
struct Kept {
    id: String,
    written: SystemTime,
    bytes: u64,
}

/// Every screen the store holds, as its names say them, oldest first. Every file is looked at,
/// however many there are.
fn every_kept(root: &Path) -> Vec<Kept> {
    let dir = dir(root);
    if !std::fs::symlink_metadata(&dir).is_ok_and(|meta| meta.is_dir()) {
        return Vec::new();
    }
    let Ok(reader) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut kept: Vec<Kept> = reader
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let meta = entry.metadata().ok().filter(std::fs::Metadata::is_file)?;
            let id = entry.file_name().into_string().ok()?;
            let id = id.strip_suffix(SUFFIX).filter(|id| an_id(id))?.to_owned();
            Some(Kept {
                id,
                written: meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                bytes: meta.len(),
            })
        })
        .collect();
    kept.sort_by_key(|one| one.written);
    kept
}

/// Deletes chat `id`'s kept screen; whether it went.
fn forget(root: &Path, id: &str) -> bool {
    path_of(root, id).is_some_and(|path| std::fs::remove_file(path).is_ok())
}

/// The oldest screens deleted until the store is within [`MOST_KEPT`] and [`MOST_KEPT_BYTES`].
fn held_to_the_total(root: &Path) -> usize {
    let kept = every_kept(root);
    let mut count = kept.len();
    let mut bytes: u64 = kept.iter().map(|one| one.bytes).sum();
    let mut gone = 0;
    for one in &kept {
        if count <= MOST_KEPT && bytes <= MOST_KEPT_BYTES {
            break;
        }
        if forget(root, &one.id) {
            gone += 1;
        }
        count -= 1;
        bytes = bytes.saturating_sub(one.bytes);
    }
    gone
}

/// **Takes away every kept screen whose chat `named` does not name, or not written since
/// [`crate::retention::KEEP_FOR`] before `now`**, in the project at `root`, then holds it to its
/// total. With the setting off ([`keeps`]), every kept screen goes. Answers how many went.
pub fn sweep(root: &Path, named: impl Fn(&str) -> bool, now: SystemTime) -> usize {
    if !keeps(root) {
        return forget_all(root);
    }
    let gone = every_kept(root)
        .into_iter()
        .filter(|one| {
            !named(&one.id)
                || now
                    .duration_since(one.written)
                    .is_ok_and(|age| age > crate::retention::KEEP_FOR)
        })
        .filter(|one| forget(root, &one.id))
        .count();
    gone + held_to_the_total(root)
}

/// **The sweep at a project's open**: [`sweep`], with the chats the project's dispatch records
/// name as workers, running or ended ([`crate::dispatchrecord::Ledger::workers`]). Run on its
/// own, whatever the record of open chats says, so a screen never outlives its record.
pub fn on_open(root: &Path, now: SystemTime) -> usize {
    let ledger = crate::dispatchrecord::ledger(root);
    let workers: std::collections::HashSet<&str> = ledger.workers().collect();
    sweep(root, |id| workers.contains(id), now)
}

/// **Takes away every kept screen** of the project at `root`: what turning the setting off does.
/// Answers how many went.
pub fn forget_all(root: &Path) -> usize {
    every_kept(root)
        .into_iter()
        .filter(|one| forget(root, &one.id))
        .count()
}

/// **Whether this machine keeps a dispatched task's last screen** in the project at `root`:
/// `[task_screens] keep` in its `charter.local.toml`, `true` where it is not set or does not
/// read as a bool. The Settings tab says why of one that does not read ([`keyed_at`]).
pub fn keeps(root: &Path) -> bool {
    let LayerText::Text(text) = crate::settings::local_layer_holding(root, TABLE) else {
        return true;
    };
    text.parse::<toml::Table>()
        .ok()
        .and_then(|top| {
            top.get(TABLE)
                .and_then(toml::Value::as_table)
                .and_then(|table| table.get(KEY))
                .and_then(toml::Value::as_bool)
        })
        .unwrap_or(true)
}

/// **What `text`, the settings file `which` of the project at `root`, says wrong of [`TABLE`]**:
/// a table that is not one, a key it does not know, a value that is no bool, or the table in the
/// committed file, where a teammate's push would decide what this machine keeps.
pub fn keyed_at(root: &Path, text: &str, which: Which) -> Vec<Refusal> {
    let file = which.file_at(root);
    let Ok(top) = text.parse::<toml::Table>() else {
        return Vec::new();
    };
    let Some(value) = top.get(TABLE) else {
        return Vec::new();
    };
    if which != Which::Local {
        return vec![Refusal::at(
            format!(
                "[{TABLE}] in {file} is not read: what a task's screen keeps is this machine's, \
                 so it is set in {} alone",
                Which::Local.file_at(root)
            ),
            &[TABLE],
        )];
    }
    let Some(table) = value.as_table() else {
        return vec![Refusal::at(
            format!("[{TABLE}] in {file} is not a table"),
            &[TABLE],
        )];
    };
    let mut out = Vec::new();
    for (key, value) in table {
        if key != KEY {
            out.push(Refusal::at(
                format!(
                    "[{TABLE}] {} in {file} is not read: the table holds {KEY} alone",
                    crate::shown::short(key)
                ),
                &[TABLE, key],
            ));
        } else if !value.is_bool() {
            out.push(Refusal::at(
                format!("[{TABLE}] {KEY} in {file} is not true or false, so screens are kept"),
                &[TABLE, KEY],
            ));
        }
    }
    out
}

/// **Sets whether this machine keeps a task's last screen** in the project at `root`, in
/// `charter.local.toml`, every other line kept; refused for what the file's own save refuses.
/// Turned off, every kept screen goes at once.
pub fn set_keep(root: &Path, keep: bool) -> Result<(), String> {
    let file = Which::Local.file_at(root);
    let path = root.join(file);
    let base = match std::fs::read_to_string(&path) {
        Ok(text) => Some(text),
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => None,
        Err(why) => return Err(format!("{file} could not be read ({why}).")),
    };
    let mut doc: toml_edit::DocumentMut =
        base.as_deref()
            .unwrap_or_default()
            .parse()
            .map_err(|why: toml_edit::TomlError| {
                format!(
                    "{file} is not valid TOML ({}), so nothing was changed.",
                    crate::shown::short(why.message())
                )
            })?;
    let table = doc
        .entry(TABLE)
        .or_insert_with(|| toml_edit::Item::Table(toml_edit::Table::new()));
    let Some(table) = table.as_table_mut() else {
        return Err(format!(
            "[{TABLE}] in {file} is not a table, so nothing was changed."
        ));
    };
    table[KEY] = toml_edit::value(keep);
    crate::settings::save(root, Which::Local, base.as_deref(), &doc.to_string())
        .map_err(|why| why.join(" "))?;
    if !keep {
        forget_all(root);
    }
    Ok(())
}

#[cfg(test)]
mod tests;

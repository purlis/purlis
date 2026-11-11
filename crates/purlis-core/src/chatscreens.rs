// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A dispatched task's last screen, kept to read after its chat closed** (#6214, parent
//! #5820): what *Read it* on a closed chat's card opens, read-only. The person ruled it on
//! 2026-11-11. It follows the precedent of the grades' kept chats
//! ([`crate::skills::grades::kept`]):
//!
//! - **Dispatched tasks only.** The app keeps a chat's screen as it closes the chat, and only
//!   where a dispatch record names that chat as its worker. Nothing of an ordinary chat is kept.
//! - **In the app's state folder**, `<state>/app/chat-screens/<ULID>.screen`: Clone state, never
//!   committed. **Out of a chat's reach**: [`crate::sandbox`] denies it to every sandboxed chat
//!   for reading and for writing, as it denies the dispatch records. A task's screen can hold
//!   whatever its work showed, and it is not for a chat running as another persona.
//! - **Kept as long as its dispatch record** ([`sweep`]): a screen whose chat no record names
//!   any more goes when the project is opened, the same sweep that collects records after 30
//!   days ([`crate::retention`]).
//! - **A setting turns it off**: `[task_screens] keep = false` in this machine's
//!   `charter.local.toml` ([`keeps`]), on by default. What is kept is this machine's, so the
//!   setting is too, never the committed file's. Turning it off takes away every kept screen:
//!   [`set_keep`] does at once, and a hand edit is caught by the next keep or sweep ([`sweep`]).
//! - **Read as the app wrote it**: named by a ULID and nothing else, at most
//!   [`MOST_SCREEN_BYTES`], written private and opened following no link, so a file put there by
//!   anything else is never shown.
//!
//! **Tier:** Clone state, device-bound, transient, never committed.

use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};

use crate::settings::{LayerText, Refusal, Which};

/// The setting's table, in this machine's `charter.local.toml`.
pub const TABLE: &str = "task_screens";

/// Its key in [`TABLE`]: whether a dispatched task's last screen is kept.
pub const KEY: &str = "keep";

/// The store's folder under the state folder's `app/`.
pub const DIR_NAME: &str = "chat-screens";

/// The most bytes of one screen kept, as a grade's chat keeps: a screen past it is not kept,
/// because cutting it would cut the sequences that draw it.
pub const MOST_SCREEN_BYTES: usize = crate::skills::grades::kept::MOST_SCREEN_BYTES;

/// The most files of the store looked at by one sweep: a store with more is swept no further.
const MOST_FILES: usize = 20_000;

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

/// **Keeps `screen`, what chat `id` showed as it closed**, in the project at `root`. Refused
/// for an id that is not a ULID and for a screen past [`MOST_SCREEN_BYTES`]; written whole,
/// private, following no link. A screen kept for the chat before is written over.
pub fn keep(root: &Path, id: &str, screen: &str) -> std::io::Result<()> {
    if !keeps(root) {
        forget_all(root);
        return Err(std::io::Error::other("this machine keeps no task's screen"));
    }
    let path = path_of(root, id)
        .ok_or_else(|| std::io::Error::other("a chat's screen is kept by its ULID"))?;
    if screen.len() > MOST_SCREEN_BYTES {
        return Err(std::io::Error::other("the screen is too big to keep"));
    }
    let dir = dir(root);
    crate::rewrite::create_dir_all(&dir)?;
    if !std::fs::symlink_metadata(&dir)?.is_dir() {
        return Err(std::io::Error::other("the store is not a folder"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))?;
    }
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut file = crate::contain::nofollow(&mut options).open(&path)?;
    file.write_all(screen.as_bytes())
}

/// **What chat `id` showed as it closed**, where this clone kept it: read through no link, at
/// most [`MOST_SCREEN_BYTES`]. `None` where nothing was kept.
pub fn kept(root: &Path, id: &str) -> Option<String> {
    let path = path_of(root, id)?;
    if !std::fs::symlink_metadata(dir(root)).is_ok_and(|meta| meta.is_dir()) {
        return None;
    }
    let meta = std::fs::symlink_metadata(&path).ok()?;
    if !meta.is_file() || meta.len() > MOST_SCREEN_BYTES as u64 {
        return None;
    }
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    let file = crate::contain::nofollow(&mut options).open(&path).ok()?;
    let mut text = String::new();
    file.take(MOST_SCREEN_BYTES as u64)
        .read_to_string(&mut text)
        .ok()?;
    Some(text)
}

/// Whether chat `id`'s screen is kept, without reading it.
pub fn is_kept(root: &Path, id: &str) -> bool {
    path_of(root, id)
        .and_then(|path| std::fs::symlink_metadata(path).ok())
        .is_some_and(|meta| meta.is_file())
}

/// The ids of the chats whose screens are kept, as the store's names say them.
fn kept_ids(root: &Path) -> Vec<String> {
    let dir = dir(root);
    if !std::fs::symlink_metadata(&dir).is_ok_and(|meta| meta.is_dir()) {
        return Vec::new();
    }
    let Ok(reader) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    reader
        .take(MOST_FILES)
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter_map(|name| name.strip_suffix(SUFFIX).map(str::to_owned))
        .filter(|id| an_id(id))
        .collect()
}

/// **Takes away every kept screen whose chat `named` does not name**, in the project at `root`:
/// what the sweep at a project's open does, with the ULIDs of the chats its dispatch records
/// name as workers. Answers how many went.
///
/// With the setting off ([`keeps`]), every kept screen goes.
pub fn sweep(root: &Path, named: impl Fn(&str) -> bool) -> usize {
    let keeps = keeps(root);
    kept_ids(root)
        .into_iter()
        .filter(|id| !keeps || !named(id))
        .filter(|id| path_of(root, id).is_some_and(|path| std::fs::remove_file(path).is_ok()))
        .count()
}

/// **Takes away every kept screen** of the project at `root`: what turning the setting off does.
/// Answers how many went.
pub fn forget_all(root: &Path) -> usize {
    sweep(root, |_| false)
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

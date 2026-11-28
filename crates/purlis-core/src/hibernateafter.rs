// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **`[chats] hibernate_after`** (SC-4h, #7637; spec #1391, *The setting*; the person's ruling
//! of 2026-11-25): how long a chat of a project that nobody looks at rests before the app may
//! hibernate it for the person, while the device is over its hot-chat target. Each project's
//! own, in its `charter.local.toml`: **Clone state**, this machine's alone.
//!
//! Spelled as `autosave_after` is ([`crate::spelledtime`]), **[`DEFAULT`] where it is not set**,
//! `off` allowed ([`read`], the small wrapper over [`crate::spelledtime::read`], which has no
//! `off`), and **a value under [`FLOOR`] refused** with a sentence. `off` hibernates nothing of
//! the project automatically; the operator's Hibernate does not read it.

use std::path::Path;
use std::time::Duration;

use crate::settings::{LayerText, Refusal, Which};
use crate::spelledtime::{UP_TO_HOURS, spelled};

/// The setting's table, in this machine's `charter.local.toml`.
pub const TABLE: &str = "chats";

/// Its key in [`TABLE`].
pub const KEY: &str = "hibernate_after";

/// What turns it off.
pub const OFF: &str = "off";

/// The spec's default, where nothing sets it: 30 minutes.
pub const DEFAULT: Duration = Duration::from_secs(30 * 60);

/// The least it may be: 10 minutes.
pub const FLOOR: Duration = Duration::from_secs(10 * 60);

/// **`text` read as the setting**: `Ok(None)` for `off`, `Ok(Some(rest))` for a spelled time of
/// at least [`FLOOR`], and otherwise why not, in a sentence that says what to write instead.
pub fn read(text: &str) -> Result<Option<Duration>, String> {
    if text == OFF {
        return Ok(None);
    }
    let shown = crate::shown::short(text);
    match crate::spelledtime::read(text, UP_TO_HOURS) {
        Some(after) if after >= FLOOR => Ok(Some(after)),
        Some(_) => Err(format!(
            "{shown} is under 10 minutes, the least it may be. Set 10m or more, or off."
        )),
        None => Err(format!(
            "{shown} is not a time purlis reads. Write a whole number and m or h, such as 45m \
             or 2h, or off."
        )),
    }
}

/// **The setting as the files spell it**: `off`, or the time in its largest whole unit.
pub fn spelled_as(after: Option<Duration>) -> String {
    after
        .and_then(|after| spelled(after, UP_TO_HOURS))
        .unwrap_or_else(|| OFF.to_owned())
}

/// **How long a chat of the project at `root` rests before the app may hibernate it**: `None`
/// where it is `off`, [`DEFAULT`] where it is not set or does not read ([`keyed_at`] says why).
pub fn of(root: &Path) -> Option<Duration> {
    let LayerText::Text(text) = crate::settings::local_layer_holding(root, TABLE) else {
        return Some(DEFAULT);
    };
    of_text(&text).unwrap_or(Some(DEFAULT))
}

/// The value `text` sets, where it sets one that reads.
fn of_text(text: &str) -> Option<Option<Duration>> {
    let top = text.parse::<toml::Table>().ok()?;
    let value = top.get(TABLE)?.as_table()?.get(KEY)?.as_str()?;
    read(value).ok()
}

/// **What `text`, the settings file `which` of the project at `root`, says wrong of [`TABLE`]**:
/// the table in the committed file, where a teammate's push would decide when this machine's
/// chats are ended; a table that is not one; a key it does not know; a value that does not read.
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
                "[{TABLE}] in {file} is not read: when this machine's chats hibernate is this \
                 machine's, so it is set in {} alone",
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
    let default = spelled_as(Some(DEFAULT));
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
            continue;
        }
        let why = match value.as_str() {
            Some(text) => read(text).err(),
            None => Some("It is not text. Write a time such as 45m or 2h, or off.".to_owned()),
        };
        if let Some(why) = why {
            out.push(Refusal::at(
                format!("[{TABLE}] {KEY} in {file} is not read, so chats hibernate after {default}. {why}"),
                &[TABLE, KEY],
            ));
        }
    }
    out
}

/// **Sets the setting** of the project at `root` to `text`, in `charter.local.toml`, every other
/// line kept, and answers what is in force then. Refused, with nothing written, for a value
/// [`read`] refuses and for what the file's own save refuses.
pub fn set(root: &Path, text: &str) -> Result<Option<Duration>, String> {
    let after = read(text.trim())?;
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
    table[KEY] = toml_edit::value(spelled_as(after));
    crate::settings::save(root, Which::Local, base.as_deref(), &doc.to_string())
        .map_err(|why| why.join(" "))?;
    Ok(after)
}

/// **One project's setting, as a reader that asks often keeps it**: read from the project's
/// local file again only once that file's modified time is not what it was, so a hand edit is
/// read at the next ask and an ask costs a `stat`, never a parse or a git check. The app's gate
/// of automatic hibernation asks at every read, which a keystroke can cause.
#[derive(Debug, Default)]
pub struct Kept {
    /// The file's modified time when it was last read (`None`: no file), and what it said.
    read: std::sync::Mutex<Option<(Option<std::time::SystemTime>, Option<Duration>)>>,
}

impl Kept {
    /// What the project at `root` says now ([`of`]).
    pub fn at(&self, root: &Path) -> Option<Duration> {
        let stamp = std::fs::metadata(Which::Local.path(root))
            .and_then(|meta| meta.modified())
            .ok();
        let mut read = self
            .read
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some((was, after)) = *read
            && was == stamp
        {
            return after;
        }
        let after = of(root);
        *read = Some((stamp, after));
        after
    }

    /// Forgets what was read, so the next ask reads the file: it was just written, and two
    /// writes in one tick of the file's clock leave its modified time as it was.
    pub fn forget(&self) {
        *self
            .read
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
    }
}

#[cfg(test)]
mod tests;

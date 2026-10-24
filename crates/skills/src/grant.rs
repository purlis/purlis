// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Where a project skill reaches, and what a workspace turned off (ADR 0099 §2; SK-4, #2767).
//!
//! A project skill's **grant** is kept beside its folder, in `<folder>.toml`, as `grant`: never
//! inside `SKILL.md`, which stays the open format. A workspace's **turn-off list** is
//! purlis-core's to read (its Workspace settings layer) and comes in as [`crate::Chat`]'s
//! `turned_off`: it can only take away what a grant gave, so there is no list that turns a
//! skill on.

use std::path::Path;

/// The key, in a skill's `<folder>.toml`, that holds its grant.
pub const KEY: &str = "grant";

/// Where a project skill may be used (ADR 0099 §2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Grant {
    /// Chats at the plane root only: the default, and what a missing file says.
    Root,
    /// Every workspace's chats too.
    All,
    /// The named workspaces' chats too.
    Only(Vec<String>),
}

impl Grant {
    /// The grant the words of `purlis skill grant <name> …` say: `root`, `all`, or workspace
    /// names, read by the same rule as a grant file's `grant` (one word is a string, more are a
    /// list), or why they say none.
    pub fn from_words(words: &[String]) -> Result<Grant, String> {
        let value = match words {
            [] => return Err("no grant was named".to_owned()),
            [one] if one == "root" || one == "all" => toml::Value::String(one.clone()),
            names if names.iter().any(|name| name == "root" || name == "all") => {
                return Err("root and all each stand alone, never beside a workspace".to_owned());
            }
            names => toml::Value::Array(names.iter().cloned().map(toml::Value::String).collect()),
        };
        grant_in(&value)
    }

    /// Whether a chat in `workspace` gets the skill. A chat at the plane root always does.
    pub fn reaches(&self, workspace: &str) -> bool {
        match self {
            Self::Root => false,
            Self::All => true,
            Self::Only(names) => names.iter().any(|name| name == workspace),
        }
    }
}

/// `root`, `all`, or the workspaces' names, comma-separated: as a person is shown it.
impl std::fmt::Display for Grant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Root => f.write_str("root"),
            Self::All => f.write_str("all"),
            Self::Only(names) => f.write_str(&names.join(", ")),
        }
    }
}

/// **The grant of the project skill in `folder`**: `root` where `<folder>.toml` is not there
/// or holds no `grant`, or why it cannot be read. A reader treats an error as `root`, the
/// narrowest grant, and says why.
pub fn grant_of(folder: &Path) -> Result<Grant, String> {
    let Some(table) = crate::beside::table(folder)? else {
        return Ok(Grant::Root);
    };
    match table.get(KEY) {
        None => Ok(Grant::Root),
        Some(value) => grant_in(value),
    }
}

/// The grant `value` says, or why it says none.
fn grant_in(value: &toml::Value) -> Result<Grant, String> {
    let shape = "which is none of \"root\", \"all\" or a list of workspace names";
    match value {
        toml::Value::String(word) if word == "root" => Ok(Grant::Root),
        toml::Value::String(word) if word == "all" => Ok(Grant::All),
        toml::Value::Array(items) => items
            .iter()
            .map(|item| {
                item.as_str()
                    .filter(|name| crate::plain(name))
                    .map(str::to_owned)
                    .ok_or_else(|| format!("{KEY} holds {item}, {shape}"))
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Grant::Only),
        other => Err(format!("{KEY} is {other}, {shape}")),
    }
}

/// **Writes `grant` as the grant of the project skill in `folder`**, keeping every other key of
/// `<folder>.toml` as it is. `root`, the default, removes the key, and the file when nothing
/// else is left in it. A file that is not TOML is refused, and left as it was. The file is
/// written beside itself and renamed over, so a reader never sees half of it.
pub fn set_grant(folder: &Path, grant: &Grant) -> Result<(), String> {
    crate::beside::edit(folder, |doc| put(doc, grant))
}

/// `grant` written into `doc` as its `grant` key: `root`, the default, as no key at all.
fn put(doc: &mut toml_edit::DocumentMut, grant: &Grant) {
    match grant {
        Grant::Root => {
            doc.remove(KEY);
        }
        Grant::All => doc[KEY] = toml_edit::value("all"),
        Grant::Only(names) => {
            let list: toml_edit::Array = names.iter().map(String::as_str).collect();
            doc[KEY] = toml_edit::value(list);
        }
    }
}

/// **The text of a new `<folder>.toml` that holds `grant` and nothing else**, as [`set_grant`]
/// writes it: for a caller that makes the file itself, new, through its own folder's handle.
pub fn grant_file(grant: &Grant) -> String {
    let mut doc = toml_edit::DocumentMut::new();
    put(&mut doc, grant);
    doc.to_string()
}

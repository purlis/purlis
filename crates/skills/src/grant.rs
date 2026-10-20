// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Where a project skill reaches, and what a workspace turned off (ADR 0099 §2; SK-4, #2767).
//!
//! A project skill's **grant** is kept beside its folder, in `<folder>.toml`, as `grant`: never
//! inside `SKILL.md`, which stays the open format. A workspace's **turn-off list** is
//! `settings.skills.off` in its `workspace.json`: it can only take away what a grant gave, so
//! there is no list that turns a skill on.

use std::path::{Path, PathBuf};

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

/// The file beside `folder` that holds what purlis alone knows about its skill.
fn file_of(folder: &Path) -> PathBuf {
    let mut name = folder.file_name().unwrap_or_default().to_os_string();
    name.push(".toml");
    folder.with_file_name(name)
}

/// That file's name, as a person is told it: `deploy.toml`.
pub(crate) fn file_name(folder: &Path) -> String {
    file_of(folder)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// **The grant of the project skill in `folder`**: `root` where `<folder>.toml` is not there
/// or holds no `grant`, or why it cannot be read. A reader treats an error as `root`, the
/// narrowest grant, and says why.
pub fn grant_of(folder: &Path) -> Result<Grant, String> {
    let Ok(text) = std::fs::read_to_string(file_of(folder)) else {
        return Ok(Grant::Root);
    };
    let table = text
        .parse::<toml::Table>()
        .map_err(|why| format!("it is not TOML: {}", first_line(&why.to_string())))?;
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
    let file = file_of(folder);
    let name = file_name(folder);
    let text = match std::fs::read_to_string(&file) {
        Ok(text) => text,
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(why) => {
            return Err(format!(
                "{name} could not be read ({why}), so nothing changed"
            ));
        }
    };
    let mut doc = text.parse::<toml_edit::DocumentMut>().map_err(|why| {
        format!(
            "{name} is not TOML ({}), so nothing changed: mend it by hand first",
            first_line(&why.to_string())
        )
    })?;
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
    let written = doc.to_string();
    if written.trim().is_empty() {
        return match std::fs::remove_file(&file) {
            Ok(()) => Ok(()),
            Err(why) if why.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(why) => Err(format!("{name} could not be removed ({why})")),
        };
    }
    let dir = file.parent().unwrap_or(Path::new("."));
    let mut temp = tempfile::NamedTempFile::new_in(dir)
        .map_err(|why| format!("{name} could not be written ({why}), so nothing changed"))?;
    std::io::Write::write_all(&mut temp, written.as_bytes())
        .map_err(|why| format!("{name} could not be written ({why}), so nothing changed"))?;
    temp.persist(&file)
        .map(|_| ())
        .map_err(|why| format!("{name} could not be written ({why}), so nothing changed"))
}

/// **The names the workspace `workspace` turned off**: `settings.skills.off` in its
/// `workspace.json`. Empty where the file, the key or a list of names is not there: a list that
/// cannot be read turns nothing off, and the Workspace level's save refuses to write one.
pub fn turned_off(plane: &Path, workspace: &str) -> Vec<String> {
    if !crate::plain(workspace) {
        return Vec::new();
    }
    let path = plane
        .join("workspaces")
        .join(workspace)
        .join("workspace.json");
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(doc) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Vec::new();
    };
    let Some(items) = doc
        .pointer("/settings/skills/off")
        .and_then(serde_json::Value::as_array)
    else {
        return Vec::new();
    };
    items
        .iter()
        .map(|item| item.as_str().map(str::to_owned))
        .collect::<Option<Vec<_>>>()
        .unwrap_or_default()
}

fn first_line(text: &str) -> &str {
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("")
}

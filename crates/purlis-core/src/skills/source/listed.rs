// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The sources a project browses** (ADR 0100 §1): `[[skill_sources]]` in the shared settings
//! file and in this machine's, read with a real parser into an allow-listed shape, failing closed:
//! a file whose sources cannot all be read lists none, and says so in one sentence.
//!
//! **What only this machine says.** A sign-in is read from this machine's file alone ([`sources`]):
//! a committed file could otherwise name the person's vault beside a URL of its own choosing, and
//! the token would be handed to that host. And this machine's file is where it **confirms** a
//! source a committed lockfile names ([`confirm`]), before purlis contacts it at all
//! ([`super::update::updates`]). A shared source names an `https://` URL alone, never a folder on
//! one machine.

use std::path::Path;

use serde::Deserialize;

use super::{Remote, Source, VaultEntry};
use crate::settings::Which;
use crate::skills::verbs::Ctx;

/// The table of the settings files the project's sources are in.
pub const TABLE: &str = "skill_sources";

/// One source the project browses, and which file lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listed {
    pub source: Source,
    pub which: Which,
}

/// A source as a settings file writes it, every key it may hold and no other.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Written {
    url: String,
    #[serde(default)]
    branch: Option<String>,
    #[serde(default)]
    folder: Option<String>,
    #[serde(default)]
    vault: Option<String>,
    #[serde(default)]
    key: Option<String>,
    #[serde(default)]
    user: Option<String>,
}

/// The sentence a settings file whose sources are not all readable is refused with.
const NOT_READ: &str = "the project's skill sources could not be read: one of them is not \
                        written the way purlis reads a source, so none is listed until it is \
                        fixed in Settings";

/// The sources `text` lists, each checked for the file it is in, or the sentence that refuses
/// them all.
fn listed_in(text: &str, which: Which) -> Result<Vec<Source>, String> {
    let doc: toml::Table = toml::from_str(text).map_err(|_| NOT_READ.to_owned())?;
    let Some(value) = doc.get(TABLE) else {
        return Ok(Vec::new());
    };
    let toml::Value::Array(rows) = value else {
        return Err(NOT_READ.to_owned());
    };
    let mut out = Vec::new();
    for row in rows {
        let written: Written = row.clone().try_into().map_err(|_| NOT_READ.to_owned())?;
        let sign_in = match (written.vault, written.key) {
            (Some(vault), Some(key)) => Some(VaultEntry {
                vault,
                key,
                user: written.user,
            }),
            (None, None) => None,
            _ => return Err(NOT_READ.to_owned()),
        };
        let source = Source {
            remote: Remote {
                url: written.url,
                branch: written.branch,
            },
            folder: written.folder,
            sign_in,
        };
        source
            .remote
            .check(which == Which::Shared)
            .and_then(|()| source.check())
            .map_err(|_| NOT_READ.to_owned())?;
        out.push(source);
    }
    Ok(out)
}

/// The sources the file `which` lists. A local file git would carry is refused.
fn read(plane: &Path, which: Which) -> Result<Vec<Source>, String> {
    match crate::settings::layer_text(plane, which) {
        crate::settings::LayerText::Text(text) => listed_in(&text, which),
        crate::settings::LayerText::Nothing => Ok(Vec::new()),
        crate::settings::LayerText::LeftOut { .. } => Err(
            "this machine's own settings would reach the team as they are, so its skill sources \
             are not read: Settings says why"
                .to_owned(),
        ),
    }
}

/// **The sources the project at `plane` browses**: the shared ones, then this machine's alone.
///
/// A shared source's own vault entry is dropped, and this machine's entry for the same repository
/// gives it its sign-in. Refused in one sentence where either file's sources cannot all be read.
pub fn sources(plane: &Path) -> Result<Vec<Listed>, String> {
    let mut locals = read(plane, Which::Local)?;
    let mut out: Vec<Listed> = Vec::new();
    for mut source in read(plane, Which::Shared)? {
        source.sign_in = None;
        if let Some(at) = locals.iter().position(|one| one.remote == source.remote) {
            source.sign_in = locals.remove(at).sign_in;
        }
        out.push(Listed {
            source,
            which: Which::Shared,
        });
    }
    out.extend(locals.into_iter().map(|source| Listed {
        source,
        which: Which::Local,
    }));
    Ok(out)
}

/// The source the project lists as `remote`, for its sign-in.
pub(super) fn listed_as(plane: &Path, remote: &Remote) -> Result<Option<Source>, String> {
    Ok(sources(plane)?
        .into_iter()
        .map(|one| one.source)
        .find(|one| one.remote == *remote))
}

/// Whether this machine confirmed `remote`: its own settings name it.
pub(super) fn confirmed(plane: &Path, remote: &Remote) -> Result<bool, String> {
    Ok(read(plane, Which::Local)?
        .iter()
        .any(|one| one.remote == *remote))
}

/// `written`'s keys as a settings table.
fn table_of(
    remote: &Remote,
    folder: Option<&str>,
    sign_in: Option<&VaultEntry>,
) -> toml_edit::Table {
    let mut table = toml_edit::Table::new();
    let mut put = |key: &str, value: Option<&str>| {
        if let Some(value) = value {
            table.insert(key, toml_edit::value(value));
        }
    };
    put("url", Some(&remote.url));
    put("branch", remote.branch.as_deref());
    put("folder", folder);
    put("vault", sign_in.map(|s| s.vault.as_str()));
    put("key", sign_in.map(|s| s.key.as_str()));
    put("user", sign_in.and_then(|s| s.user.as_deref()));
    table
}

/// Changes `which` file's sources by `how`, in the window's words where it cannot.
fn edit(
    plane: &Path,
    which: Which,
    how: impl FnOnce(&mut toml_edit::ArrayOfTables) -> bool,
) -> Result<bool, String> {
    crate::settings::skill_sources::edit(plane, which, how).map_err(|why| {
        tracing::info!(target: "purlis::skills", "a skill source was not written: {why}");
        "the project's settings could not be changed, so nothing was written: Settings says why"
            .to_owned()
    })
}

/// Whether `table` names `remote`.
fn names(table: &toml_edit::Table, remote: &Remote) -> bool {
    table.get("url").and_then(|v| v.as_str()) == Some(remote.url.as_str())
        && table.get("branch").and_then(|v| v.as_str()) == remote.branch.as_deref()
}

/// **Adds `source` to the project's sources** in `which` file, committed where shared. Its
/// sign-in goes in this machine's file alone, beside it. One already listed is said and changes
/// nothing. A shared source is an `https://` URL.
pub fn add(cx: &mut Ctx<'_>, source: &Source, which: Which) -> Result<bool, String> {
    source.check()?;
    source.remote.check(which == Which::Shared)?;
    let plane = cx.plane;
    crate::settings::writable(plane)?;
    if listed_as(plane, &source.remote)?.is_some() {
        return Ok(false);
    }
    let folder = source.folder.as_deref();
    match which {
        Which::Local => {
            edit(plane, Which::Local, |list| {
                list.push(table_of(&source.remote, folder, source.sign_in.as_ref()));
                true
            })?;
        }
        Which::Shared => {
            if source.sign_in.is_some() || !confirmed(plane, &source.remote)? {
                // This machine's own line: the sign-in, and the source confirmed here.
                edit(plane, Which::Local, |list| {
                    list.retain(|table| !names(table, &source.remote));
                    list.push(table_of(&source.remote, None, source.sign_in.as_ref()));
                    true
                })?;
            }
            edit(plane, Which::Shared, |list| {
                list.push(table_of(&source.remote, folder, None));
                true
            })?;
        }
    }
    cx.done(format!("{source} is a skill source of this project"));
    if which == Which::Shared {
        cx.commit(
            &[Which::Shared.file_at(plane).to_owned()],
            "skill source add",
        );
    }
    Ok(true)
}

/// **Confirms `remote`, a source the committed lockfile names, on this machine**: purlis may ask
/// it for a newer commit. An `https://` URL alone: a folder on another machine is never contacted
/// from a committed file. Written in this machine's settings; `Ok(false)` where it was confirmed
/// already.
pub fn confirm(cx: &mut Ctx<'_>, remote: &Remote) -> Result<bool, String> {
    remote.check(true)?;
    confirm_here(cx, remote)
}

/// [`confirm`] for a source this machine installed from itself, which may be a folder here.
pub(super) fn confirm_here(cx: &mut Ctx<'_>, remote: &Remote) -> Result<bool, String> {
    remote.check(false)?;
    let plane = cx.plane;
    crate::settings::writable(plane)?;
    if confirmed(plane, remote)? {
        return Ok(false);
    }
    edit(plane, Which::Local, |list| {
        list.push(table_of(remote, None, None));
        true
    })?;
    cx.done(format!("{remote} is confirmed on this machine"));
    Ok(true)
}

/// **Takes `remote` out of the project's sources**, and this machine's line for it. Skills
/// installed from it stay, with their pins.
pub fn remove(cx: &mut Ctx<'_>, remote: &Remote) -> Result<bool, String> {
    let plane = cx.plane;
    crate::settings::writable(plane)?;
    let Some(listed) = sources(plane)?
        .into_iter()
        .find(|one| one.source.remote == *remote)
    else {
        return Ok(false);
    };
    let take_out = |list: &mut toml_edit::ArrayOfTables| {
        let before = list.len();
        list.retain(|table| !names(table, remote));
        list.len() != before
    };
    edit(plane, listed.which, take_out)?;
    if listed.which == Which::Shared {
        edit(plane, Which::Local, take_out)?;
    }
    cx.done(format!(
        "{} is no longer a skill source here",
        listed.source
    ));
    if listed.which == Which::Shared {
        cx.commit(
            &[Which::Shared.file_at(plane).to_owned()],
            "skill source remove",
        );
    }
    Ok(true)
}

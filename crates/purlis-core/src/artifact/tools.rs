// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The read-only `purlis artifact` verbs, as MCP tools** (spec #5948: "each with an MCP twin
//! where read-only"; #5978's review): `artifact_list`, `artifact_check` and
//! `artifact_approve_status`, about the asking chat's own workspace.
//!
//! Each tool and its verb say one thing in the same words, because both are made here: the
//! lines of `list` ([`list_line`]), of `approve-status` ([`status_line`], [`status_lines`]) and
//! of `check` ([`check_lines`], [`check_summary`]). The command line prints them; a tool answers
//! with them. Nothing here writes: approving a page's script stays the person's, in purlis's bar.

use std::path::Path;

use serde_json::{Map, Value, json};

use super::approval::{self, Standing};
use super::{Listed, check};
use crate::chattools::Tool;
use crate::counts::counted;
use crate::personas::one_line;

/// The twin of `purlis artifact list`.
pub const LIST: &str = "artifact_list";
/// The twin of `purlis artifact check`, on a folder of the chat's own artifacts.
pub const CHECK: &str = "artifact_check";
/// The twin of `purlis artifact approve-status`.
pub const APPROVE_STATUS: &str = "artifact_approve_status";

/// The tools, for the one catalogue ([`crate::catalogue::every_tool`]).
pub static TOOLS: [Tool; 3] = [
    Tool {
        name: LIST,
        description: "List this chat's workspace's pinned artifacts in their order, as `purlis \
                      artifact list` does: one line each of order, id, folder, icon and name, \
                      and why one cannot be opened here (missing, or over the size cap).",
        schema: crate::chattools::no_arguments,
        read_only: true,
    },
    Tool {
        name: CHECK,
        description: "Check an artifact's page before anyone is asked to approve it, as `purlis \
                      artifact check` does: that its declaration (<script \
                      type=\"application/json\" id=\"purlis\">) is one purlis reads, names only \
                      queries the catalogue has, and that every query its pages and scripts call \
                      is one it declared. An error lists what would be refused.",
        schema: || {
            crate::chattools::one_string(
                "folder",
                "The artifact's folder inside this chat's workspace's artifacts/.",
            )
        },
        read_only: true,
    },
    Tool {
        name: APPROVE_STATUS,
        description: "Where each of this chat's workspace's pinned artifacts stands on this \
                      machine, as `purlis artifact approve-status` does: script-free, approved, \
                      or asking (new, its template changed, or its pin changed), with its \
                      template's fingerprint. Approving is the person's, in the artifact's bar.",
        schema: || {
            json!({
                "type": "object",
                "properties": {
                    "pin": {
                        "type": "string",
                        "description": "Only this pin, by id or folder.",
                    },
                },
                "additionalProperties": false,
            })
        },
        read_only: true,
    },
];

/// Whether `name` is one of these tools.
pub fn is_one(name: &str) -> bool {
    TOOLS.iter().any(|tool| tool.name == name)
}

/// **What tool `name` answers** for a chat in the workspace `workspace` at `dir`, on the machine
/// whose config home is `config`: the text, or the sentence saying why nothing was read.
pub fn call(
    workspace: &str,
    dir: &Path,
    config: Option<&Path>,
    name: &str,
    args: &Map<String, Value>,
) -> Result<String, String> {
    let text = |key: &str| match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(said)) => Ok(Some(said.as_str())),
        Some(_) => Err(format!("`{key}` must be a string")),
    };
    match name {
        LIST => {
            let listed = super::list(dir)?;
            if listed.is_empty() {
                return Ok(none_pinned(workspace));
            }
            Ok(listed.iter().map(list_line).collect::<Vec<_>>().join("\n"))
        }
        APPROVE_STATUS => Ok(status_lines(config, dir, text("pin")?)?.join("\n")),
        CHECK => {
            let folder = text("folder")?.ok_or("`folder` is required")?;
            let real = super::resolve(dir, folder)?;
            let checked = check::check(&real, folder)?;
            let mut said = check_lines(&checked);
            said.extend(checked.warnings.iter().map(|why| format!("warning: {why}")));
            said.extend(checked.refused.iter().map(|why| format!("refused: {why}")));
            if checked.refused.is_empty() {
                said.push(check_summary(folder, &checked));
                Ok(said.join("\n"))
            } else {
                Err(said.join("\n"))
            }
        }
        _ => Err(format!("purlis has no tool {}", crate::shown::short(name))),
    }
}

/// What `list` says of a workspace with no pin.
pub fn none_pinned(workspace: &str) -> String {
    format!("No artifact is pinned to '{workspace}' (pin one: purlis artifact pin <folder>).")
}

/// One pin's line of `list`: order, id, folder, icon, name, and why it cannot be opened here.
pub fn list_line(item: &Listed) -> String {
    let pin = &item.pin;
    let mut line = format!(
        "{}  {}  {}  {}  {}",
        pin.order,
        pin.id,
        one_line(&pin.folder),
        pin.icon,
        one_line(&pin.name)
    );
    if let Some(why) = &item.missing {
        line.push_str(&format!("  (missing: {why})"));
    }
    if let Some(why) = &item.too_large {
        line.push_str(&format!("  (too large: {why})"));
    }
    line
}

/// **`approve-status`'s lines** for the workspace at `dir`: every pin's, or only the one `pin`
/// names (by id or folder), refused when it names none.
pub fn status_lines(
    config: Option<&Path>,
    dir: &Path,
    pin: Option<&str>,
) -> Result<Vec<String>, String> {
    let lines: Vec<String> = super::list(dir)?
        .iter()
        .filter(|item| pin.is_none_or(|key| key == item.pin.id || key == item.pin.folder))
        .map(|item| status_line(config, dir, item))
        .collect();
    match pin {
        Some(key) if lines.is_empty() => Err(format!(
            "no pin '{}' in this workspace (`purlis artifact list` shows them)",
            one_line(key)
        )),
        _ => Ok(lines),
    }
}

/// One pin's line of `approve-status`: folder, standing, fingerprint, and what it means.
pub fn status_line(config: Option<&Path>, workspace: &Path, item: &Listed) -> String {
    let pin = &item.pin;
    let folder = one_line(&pin.folder);
    if let Some(why) = &item.too_large {
        return format!("{folder}  too-large  -  {why}");
    }
    let read = match &item.missing {
        Some(why) => Err(why.clone()),
        None => super::resolve(workspace, &pin.folder)
            .and_then(|real| super::serve::Snapshot::read(&real, &pin.folder)),
    };
    let snapshot = match read {
        Ok(snapshot) => snapshot,
        Err(why) => return format!("{folder}  missing  -  {why}"),
    };
    let standing = approval::standing(config, workspace, &pin.folder, &pin.id, &snapshot);
    let meaning = match standing {
        Standing::ScriptFree => "opens without asking: no script".to_owned(),
        Standing::Approved => "approved on this machine: its script runs".to_owned(),
        Standing::New => "asks before its script runs: never approved on this machine".to_owned(),
        Standing::Changed => "asks again: its template changed since you approved it".to_owned(),
        Standing::PinChanged => match super::pinned_by(workspace, &pin.id) {
            Some(who) => format!(
                "asks again: pin changed by {who} (its folder or command) since you approved it"
            ),
            None => "asks again: pin changed (its folder or command) since you approved it, \
                     not saved yet"
                .to_owned(),
        },
        Standing::NotHere => {
            "renders without its script: scripted artifacts do not run on this platform yet"
                .to_owned()
        }
    };
    format!(
        "{folder}  {}  {}  {meaning}",
        standing.as_str(),
        snapshot.fingerprint()
    )
}

/// `check`'s line for each query a page declares: its id, its scope, and what widens it.
pub fn check_lines(checked: &check::Checked) -> Vec<String> {
    let Some(declared) = &checked.declared else {
        return Vec::new();
    };
    super::calls::listed(&declared.queries)
        .into_iter()
        .map(|listed| {
            let mut line = format!("  {} · {}", listed.id, listed.scope.as_str());
            for widens in &listed.widens {
                line.push_str(&format!(" · widens {widens}"));
            }
            line
        })
        .collect()
}

/// What `check` says of a page named `name` that nothing would refuse.
pub fn check_summary(name: &str, checked: &check::Checked) -> String {
    match &checked.declared {
        Some(declared) => format!(
            "{name} declares api {}: {} and {}, and calls only what it declares.",
            declared.api,
            counted(declared.data.len(), "data file", "data files"),
            counted(declared.queries.len(), "query", "queries")
        ),
        None => format!("{name} declares nothing: it reads no data and calls no query."),
    }
}

#[cfg(test)]
mod tests;

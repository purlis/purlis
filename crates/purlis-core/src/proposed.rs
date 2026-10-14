// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A proposed change** (#1975, the operator's rulings PC-1..PC-14 of 2026-10-14): an edit to
//! one of purlis's own configuration files that a chat may not write, which only the person
//! applies, in the window.
//!
//! A chat's sandbox keeps it from writing the files a later program loads code or settings from
//! (ADR 0067 §5). purlis's own configuration is among them, and a chat used to write such a
//! change to its scratch folder and ask the person to run commands. Now:
//!
//! - **In Claude Code** the chat's own `Write`, `Edit` or `MultiEdit` of such a file is read by
//!   its `pretooluse-edit` hook ([`from_edit`]): the change is worked out against the file as it
//!   is now, handed to the app as a proposal ([`crate::hookwire::ProposeAsk`]), and the tool is
//!   refused with [`PROPOSED_SAID`], so the harness never writes the file (PC-2).
//! - **In another harness** the chat runs `purlis propose <file> --from <draft>`, the draft in
//!   its own folder (PC-2).
//! - **The app shows it whole and the person applies it** (PC-3..PC-10, PC-13): never a chat,
//!   never the harness, never "always". What is shown as the file now is the app's own read of
//!   it; the base a proposal carries is only checked against the file at Apply, so one that is
//!   wrong can only make the proposal stale.
//!
//! **Which files** ([`proposable`], PC-1, PC-11): purlis's own configuration, in the plane only:
//! the manifest at the project root and in each workspace, a persona's `mcp.json` and
//! `persona.md`, and a `.claude/skills` folder's files at the project root, in a workspace or in
//! a persona. Never a file that runs as code the moment it is there (git hooks, a harness's
//! settings, an editor's tasks, package scripts, `.envrc`), never a repo clone's own file, never
//! purlis's state, and never a path with a link in it.
//!
//! **Policy** (PC-14): `sandbox.proposed-changes = "off"` turns it off on this machine
//! ([`crate::sandbox::policy::Locks::proposed_changes_refused`]); the chat then gets the refusal
//! it got before.

use std::path::{Component, Path, PathBuf};

use serde_json::{Map, Value};

/// What the chat's refused tool says (PC-10): the person has it, and the chat waits.
pub const PROPOSED_SAID: &str = "purlis showed this change to the person as a proposed change: \
     only they apply it, in the window. Wait for their answer, which comes back to this chat as \
     applied, dismissed or stale. Do not write the file another way.";

/// What an app that does not hold proposed changes answers one ([`crate::hookwire::Ask::Propose`]):
/// the asking chat is then refused its edit as before.
pub const NOT_HELD: &str = "this app does not show proposed changes to the person";

/// The most bytes a proposed file may hold: far past any configuration file, and small enough
/// to send whole over the hook channel and show.
pub const MOST_BYTES: usize = 512 * 1024;

/// **Whether `path` is a file a chat may propose a change to** in the project at `root`, judged
/// on its lexical path and on the disk: the path relative to `root` where it is one, or why not.
/// Every folder from `root` down to the file, and the file itself where it is there, must be no
/// link: a link would make the window write somewhere the person did not see named.
pub fn proposable(root: &Path, path: &Path) -> Result<PathBuf, String> {
    let rel = relative_to(root, path).ok_or_else(|| not_proposable(path))?;
    let parts: Vec<&str> = rel
        .components()
        .map(|part| match part {
            Component::Normal(name) => name.to_str().unwrap_or("\u{0}"),
            _ => "\u{0}",
        })
        .collect();
    if parts.iter().any(|part| *part == "\u{0}" || part.is_empty()) {
        return Err(not_proposable(path));
    }
    let manifest = |name: &str| {
        crate::names::PLANE_MANIFEST
            .spellings()
            .any(|one| one == name)
    };
    let a_name = |name: &str| !name.starts_with('.');
    let skills = |rest: &[&str]| {
        rest.len() >= 3
            && rest[0] == ".claude"
            && rest[1] == "skills"
            && rest[2..].iter().all(|p| *p != "..")
    };
    let ours = match parts.as_slice() {
        [name] => manifest(name),
        ["workspaces", workspace, name] => a_name(workspace) && manifest(name),
        ["personas", persona, "mcp.json" | "persona.md"] => a_name(persona),
        rest if skills(rest) => true,
        ["workspaces" | "personas", owner, rest @ ..] => a_name(owner) && skills(rest),
        _ => false,
    };
    if !ours {
        return Err(not_proposable(path));
    }
    // No link anywhere from the root down: judged on the disk, each folder that is there.
    let mut at = root.to_path_buf();
    for part in &parts {
        at.push(part);
        match std::fs::symlink_metadata(&at) {
            Ok(meta) if meta.file_type().is_symlink() => return Err(not_proposable(path)),
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => break,
            Err(_) => return Err(not_proposable(path)),
        }
    }
    Ok(rel)
}

/// Why `path` is not a file a chat may propose a change to: the way that works instead.
pub fn not_proposable(path: &Path) -> String {
    format!(
        "{} is not a file purlis lets a chat propose a change to: only purlis's own \
         configuration in the project (its manifest, a persona's mcp.json and persona.md, and a \
         .claude/skills folder's files) is. Make the change yourself, or ask the person to.",
        path.display()
    )
}

/// `path` relative to `root`, by its words alone: `.` and `..` taken out, and none left that
/// climbs out of `root`.
/// Under either spelling of `root`: as given, or as the kernel names it.
fn relative_to(root: &Path, path: &Path) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    let real = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    [root, real.as_path()]
        .into_iter()
        .find_map(|root| out.strip_prefix(root).ok().map(Path::to_path_buf))
        .filter(|rel| !rel.as_os_str().is_empty())
}

/// **The file's content once a Claude Code edit tool's `input` is applied to `now`** (the file
/// as it is, or none where it is not there), exactly as the tool would write it: `Write` the
/// whole content, `Edit` one replacement, `MultiEdit` each in turn. Or why it cannot be, in the
/// tool's own terms: a text that is not there, or there more than once without `replace_all`.
pub fn apply_edit(tool: &str, input: &Value, now: Option<&str>) -> Result<String, String> {
    let input = input
        .as_object()
        .ok_or_else(|| "the edit's input is not an object".to_owned())?;
    let out = match tool {
        "Write" => text(input, "content")?.to_owned(),
        "Edit" => replace(now.unwrap_or(""), input)?,
        "MultiEdit" => {
            let edits = input
                .get("edits")
                .and_then(Value::as_array)
                .ok_or_else(|| "the edit's input has no edits".to_owned())?;
            let mut text = now.unwrap_or("").to_owned();
            for edit in edits {
                let edit = edit
                    .as_object()
                    .ok_or_else(|| "one of the edits is not an object".to_owned())?;
                text = replace(&text, edit)?;
            }
            text
        }
        other => return Err(format!("{other} is not an edit purlis proposes")),
    };
    if out.len() > MOST_BYTES {
        return Err(format!(
            "the file would be longer than the {MOST_BYTES} bytes a proposed change may be"
        ));
    }
    Ok(out)
}

fn text<'a>(input: &'a Map<String, Value>, key: &str) -> Result<&'a str, String> {
    input
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("the edit's input has no {key}"))
}

/// One `old_string` → `new_string` replacement of `now`, as the tool makes it.
fn replace(now: &str, input: &Map<String, Value>) -> Result<String, String> {
    let old = text(input, "old_string")?;
    let new = text(input, "new_string")?;
    let every = input.get("replace_all").and_then(Value::as_bool) == Some(true);
    if old.is_empty() {
        return Err("the text to replace is empty".to_owned());
    }
    match now.matches(old).count() {
        0 => Err("the text to replace is not in the file".to_owned()),
        1 => Ok(now.replacen(old, new, 1)),
        _ if every => Ok(now.replace(old, new)),
        many => Err(format!(
            "the text to replace is in the file {many} times: name more of it, or replace every \
             one"
        )),
    }
}

/// The base a proposal carries: the SHA-256 of the file as it was read, in hex, or none where
/// it was not there.
pub fn base_of(now: Option<&[u8]>) -> Option<String> {
    use sha2::Digest as _;
    now.map(|bytes| {
        sha2::Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    })
}

/// **What a Claude Code edit tool's hook `payload` proposes**, in the project at `root`, with
/// `cwd` the folder a relative path is read from: none where the tool's target is not a
/// proposable file (it is not this module's), else the proposal or why there is none.
pub fn from_edit(root: &Path, payload: &Value, cwd: &Path) -> Option<Result<Proposed, String>> {
    let tool = payload["tool_name"].as_str()?;
    if !matches!(tool, "Write" | "Edit" | "MultiEdit") {
        return None;
    }
    let named = payload["tool_input"]["file_path"].as_str()?;
    let path = cwd.join(named);
    let rel = proposable(root, &path).ok()?;
    Some(read_and_apply(root, &rel, |now| {
        apply_edit(tool, &payload["tool_input"], now)
    }))
}

/// A proposal, as a hook or `purlis propose` hands it over: the file relative to the project,
/// the base the change was worked out against, and the file's whole new content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proposed {
    pub path: PathBuf,
    pub base: Option<String>,
    pub content: String,
}

/// The file at `rel` as it is now, and `change` made to it: the proposal, or why there is none.
pub fn read_and_apply(
    root: &Path,
    rel: &Path,
    change: impl FnOnce(Option<&str>) -> Result<String, String>,
) -> Result<Proposed, String> {
    let now = match std::fs::read(root.join(rel)) {
        Ok(bytes) => Some(bytes),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(format!("purlis could not read {}: {e}", rel.display())),
    };
    if now.as_ref().is_some_and(|bytes| bytes.len() > MOST_BYTES) {
        return Err(format!(
            "{} is longer than the {MOST_BYTES} bytes a proposed change may be",
            rel.display()
        ));
    }
    let text = match &now {
        None => None,
        Some(bytes) => Some(
            std::str::from_utf8(bytes)
                .map_err(|_| format!("{} is not text purlis can show", rel.display()))?,
        ),
    };
    let content = change(text)?;
    Ok(Proposed {
        path: rel.to_path_buf(),
        base: base_of(now.as_deref()),
        content,
    })
}

#[cfg(test)]
#[path = "proposed_tests.rs"]
mod tests;

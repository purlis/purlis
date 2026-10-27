// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The `[harness_plugins]` keys purlis no longer reads** (#3467, the operator's rulings of
//! 2026-10-26/27 on spec #2762): purlis stopped choosing another harness's plugins per project,
//! and the harness decides. A project that still holds the old keys starts as any other, with
//! the keys ignored and never rewritten behind the person's back. This finds them, for the one
//! notice that says so, and makes the text without them, for the change the person applies
//! (`Remove`, held as a proposed change: [`crate::proposed`]).
//!
//! The project's two files are found here and made without the keys. A workspace's
//! `settings.harness_plugins` is only named: its manifest has its own writer, and the
//! Workspace settings tab's Edit as JSON takes the key out.

use std::path::Path;

use crate::settings::{Edit, Step, Which};

/// The table purlis read a project's choice of another harness's plugins from, until #3467.
pub const TABLE: &str = "harness_plugins";

/// One file that still holds [`TABLE`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    /// The file, relative to the project: `charter.toml`, `charter.local.toml` (as the project
    /// spells them), or `workspaces/<ws>/workspace.json`.
    pub file: String,
    /// Which of the project's two files it is, or `None` for a workspace's manifest.
    pub which: Option<Which>,
    /// Its text as read, which the change is worked out against.
    pub text: String,
}

/// Whether `top`, a TOML file's top or a workspace's settings, holds [`TABLE`].
fn holds(top: &toml::Table) -> bool {
    top.contains_key(TABLE)
}

/// **Every file of the project at `root` that still holds [`TABLE`]**: its two files, then each
/// workspace's manifest, in name order. A file that cannot be read, or is not TOML (JSON for a
/// manifest), is not one this can say holds anything.
pub fn found(root: &Path) -> Vec<Found> {
    let mut out = Vec::new();
    for which in [Which::Shared, Which::Local] {
        let Ok(read) = crate::settings::read(root, which) else {
            continue;
        };
        if read.exists && read.text.parse::<toml::Table>().is_ok_and(|top| holds(&top)) {
            out.push(Found {
                file: which.file_at(root).to_owned(),
                which: Some(which),
                text: read.text,
            });
        }
    }
    let mut names = crate::workspaces::Plane::open(root)
        .workspaces()
        .unwrap_or_default();
    names.sort();
    for name in names {
        if crate::settings::workspace::read(root, &name).is_some_and(|top| holds(&top)) {
            out.push(Found {
                file: crate::settings::workspace::named(&name),
                which: None,
                text: String::new(),
            });
        }
    }
    out
}

/// **`text`, a TOML settings file, without [`TABLE`]**, everything else kept as written: the
/// change Remove proposes. Refused where `text` is not TOML.
pub fn removed(text: &str) -> Result<String, String> {
    crate::settings::edited(
        text,
        &[Edit {
            path: vec![Step::Key(TABLE.to_owned())],
            value: None,
        }],
    )
}

/// What the notice says, for the files in `found`: the keys are ignored, the harness decides,
/// and where Remove reaches.
pub fn said(found: &[Found]) -> String {
    let files: Vec<&str> = found.iter().map(|one| one.file.as_str()).collect();
    let mut said = format!(
        "purlis no longer turns other harnesses' plugins on or off per project, so [{TABLE}] in \
         {} is ignored: each harness decides from its own settings. purlis's own plugin is \
         loaded as before.",
        list(&files)
    );
    let manifests: Vec<&str> = found
        .iter()
        .filter(|one| one.which.is_none())
        .map(|one| one.file.as_str())
        .collect();
    if !manifests.is_empty() {
        said.push_str(&format!(
            " Remove takes it out of the project's own files; take settings.{TABLE} out of {} \
             with that workspace's Edit as JSON.",
            list(&manifests)
        ));
    }
    said
}

/// `a`, `a and b`, `a, b and c`.
fn list(items: &[&str]) -> String {
    match items {
        [] => String::new(),
        [one] => (*one).to_owned(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

#[cfg(test)]
mod tests;

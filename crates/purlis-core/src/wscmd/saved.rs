// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Saved with the project** (ADR 0038, amended 2026-11-05; #5377): the workspace files a
//! LIVE workspace shares beside the paths the LIVE block publishes for every LIVE workspace.
//!
//! **The line.** Saving an entry at the top of a LIVE workspace's folder adds one line to the
//! LIVE block, `!/workspaces/<ws>/<entry>`, after that workspace's own lines. It un-ignores that
//! file or folder and nothing else: `/workspaces/*/*` ignores only the entries at a workspace's
//! top, so a folder's contents follow it. A line of that shape inside the block that the block
//! does not write for every LIVE workspace is a saved entry ([`saved_in`]), so every rewrite of
//! the block keeps it: `refresh_live_block` (and so `reinit`), `rename`, and making another
//! workspace LIVE or LOCAL. Making this workspace LOCAL drops its lines with the rest.
//!
//! **Nothing else adds one.** Only the person's press of Save with the project calls [`add`]:
//! a script in a workspace may hold a token, so nothing is shared that they did not pick.
//!
//! **Where each entry is kept** ([`Marks`]) is read from the plane's `.gitignore` as git reads
//! it, through the `ignore` crate's matcher: an entry is saved with the project when neither it
//! nor a folder above it is ignored. Asked only of a LIVE workspace: a LOCAL one's files are all
//! this machine's, and the window marks none of them.

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::Path;

use ignore::gitignore::{Gitignore, GitignoreBuilder};

use super::{in_blocks, live_in, pattern_name, workspace_lines, workspace_of};

/// Where a LIVE workspace's file is kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kept {
    /// Ignored by the plane's git: it stays in this clone.
    ThisMachineOnly,
    /// Un-ignored by the LIVE block: a save commits it.
    WithTheProject,
}

/// The saved entries of each LIVE workspace the managed block in `text` holds, by workspace.
pub(super) fn saved_in(text: &str) -> BTreeMap<String, BTreeSet<String>> {
    let live = live_in(text);
    let mut saved: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for line in in_blocks(text) {
        if !line.read.starts_with('!') {
            continue;
        }
        let Some((name, entry)) = workspace_of(line.read) else {
            continue;
        };
        if !live.contains(name) || !one_entry(entry) {
            continue;
        }
        if workspace_lines(name).contains(&line_for(name, entry)) {
            continue;
        }
        saved
            .entry(name.to_string())
            .or_default()
            .insert(entry.to_string());
    }
    saved
}

/// Whether `entry` is one name at the top of a workspace's folder that one line can spell: no
/// `/`, nothing git reads as a pattern, and no whitespace git would trim from its end.
pub(crate) fn one_entry(entry: &str) -> bool {
    !entry.is_empty()
        && !entry.contains(['/', '\n', '\r'])
        && !pattern_name(entry)
        && entry.trim_end() == entry
        && entry != "."
        && entry != ".."
}

/// The one line that saves `entry` of workspace `ws`.
pub(super) fn line_for(ws: &str, entry: &str) -> String {
    format!("!/workspaces/{ws}/{entry}")
}

/// The entries workspace `ws` has saved with the project, as its plane's `.gitignore` stands.
pub fn saved(root: &Path, ws: &str) -> BTreeSet<String> {
    std::fs::read_to_string(root.join(".gitignore"))
        .map(|text| saved_in(&text).remove(ws).unwrap_or_default())
        .unwrap_or_default()
}

/// Why an entry was not saved with the project.
#[derive(Debug, thiserror::Error)]
pub enum NotSaved {
    #[error(
        "'{0}' is not LIVE, so nothing of it is saved with the project. Make the workspace LIVE \
         first."
    )]
    NotLive(String),
    #[error(
        "purlis saves only a file or folder at the top of the workspace's folder, and '{0}' is \
         not one it can name in a single line. Save the folder it is in, or rename it."
    )]
    NotOneEntry(String),
    #[error(
        "purlis could not write the project's .gitignore, so '{entry}' is still kept on this \
         machine only ({why}). Check the file can be written, then try again."
    )]
    Unwritten { entry: String, why: io::Error },
}

/// **Save `entry` of LIVE workspace `ws` with the project**: one line in the LIVE block, after
/// the workspace's own. `true` when the line was added; `false` when it was there already.
/// Read and written under the plane's lock, as [`super::set_live`] is. The caller has checked
/// that `entry` is the person's file and not one purlis keeps.
pub fn add(root: &Path, ws: &str, entry: &str) -> Result<bool, NotSaved> {
    if !one_entry(entry) {
        return Err(NotSaved::NotOneEntry(entry.to_string()));
    }
    let mut not_live = false;
    let changed = super::rewrite_gitignore(root, |text| {
        let live = live_in(text);
        if !live.contains(ws) {
            not_live = true;
            return None;
        }
        let mut saved = saved_in(text);
        if workspace_lines(ws).contains(&line_for(ws, entry))
            || !saved
                .entry(ws.to_string())
                .or_default()
                .insert(entry.to_string())
        {
            return None;
        }
        Some(super::with_block(
            text,
            &super::block_for(root, &live, &saved),
        ))
    })
    .map_err(|why| NotSaved::Unwritten {
        entry: entry.to_string(),
        why,
    })?;
    if not_live {
        return Err(NotSaved::NotLive(ws.to_string()));
    }
    Ok(changed)
}

/// **Where each file of one LIVE workspace is kept**, read from its plane's `.gitignore` once.
#[derive(Debug)]
pub struct Marks {
    ignored: Gitignore,
    ws: String,
}

impl Marks {
    /// The marks of workspace `ws`'s files, or `None` when it is not LIVE: a LOCAL workspace's
    /// files are all this machine's, and none is marked. A `.gitignore` that cannot be read is
    /// no LIVE block, so none either.
    pub fn of(root: &Path, ws: &str) -> Option<Self> {
        let text = std::fs::read_to_string(root.join(".gitignore")).ok()?;
        if !live_in(&text).contains(ws) {
            return None;
        }
        let mut builder = GitignoreBuilder::new(root);
        for line in text.lines() {
            // A line the matcher cannot read is one git skips too.
            let _ = builder.add_line(None, line);
        }
        let ignored = builder.build().ok()?;
        Some(Self {
            ignored,
            ws: ws.to_string(),
        })
    }

    /// Where `path`, a path of the workspace's files, is kept. As git reads it: ignored when a
    /// folder above it is, each folder judged from the top down, and otherwise by its own match.
    pub fn of_path(&self, path: &str, is_folder: bool) -> Kept {
        let mut at = format!("workspaces/{}", self.ws);
        let steps: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        for (i, step) in steps.iter().enumerate() {
            at.push('/');
            at.push_str(step);
            let folder = i + 1 < steps.len() || is_folder;
            if self.ignored.matched(&at, folder).is_ignore() {
                return Kept::ThisMachineOnly;
            }
        }
        Kept::WithTheProject
    }
}

#[cfg(test)]
mod tests;

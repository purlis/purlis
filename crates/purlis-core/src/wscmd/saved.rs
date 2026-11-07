// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Saved with the project** (ADR 0038, amended 2026-11-05; #5377): the workspace files a
//! LIVE workspace shares beside the paths the LIVE block publishes for every LIVE workspace.
//!
//! **The line.** Saving an entry at the top of a LIVE workspace's folder adds one line to the
//! LIVE block, `!/workspaces/<ws>/<entry>`, after that workspace's own lines. It un-ignores that
//! file or folder and nothing else: `/workspaces/*/*` ignores only the entries at a workspace's
//! top, so a folder's contents follow it. A name whose composed form (NFC) is not how it is
//! spelled on disk gets a second line in that form ([`lines_for`], #5552): git on a Mac that
//! precomposes names reads `café` composed whatever the disk holds, and one that does not reads
//! the disk's bytes, so one of the two always matches. A line of that shape inside the block that the block
//! does not write for every LIVE workspace is a saved entry ([`saved_in`]), so every rewrite of
//! the block keeps it: `refresh_live_block` (and so `reinit`), `rename`, and making another
//! workspace LIVE or LOCAL. Making this workspace LOCAL drops its lines with the rest, and
//! removing it drops them too ([`forget`], #5552): a new workspace of the same name shares
//! nothing its person did not pick.
//!
//! **Nothing else adds one.** Only the person's press of Save with the project calls [`add`]:
//! a script in a workspace may hold a token, so nothing is shared that they did not pick.
//!
//! **Where each entry is kept** ([`Marks`]) is git's own answer (#5552), never a reading of the
//! block: `git check-ignore`, which reads every `.gitignore` on the way, `info/exclude` and the
//! person's global excludes, and which never calls a path ignored that the index tracks. So a
//! tracked file whose line is gone is marked saved, because a save commits it. Asked only of a
//! LIVE workspace: a LOCAL one's files are all this machine's, and the window marks none. Each
//! path is asked in the one form git reads it in, and one git would read as a pattern is never
//! asked ([`Marks::of_paths`]).

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::{Path, PathBuf};

use unicode_normalization::UnicodeNormalization as _;

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
    // An entry spelled decomposed has a second line in its composed form; that line is the same
    // entry, not another one.
    for entries in saved.values_mut() {
        let spelled: Vec<String> = entries
            .iter()
            .filter(|entry| composed(entry) != **entry)
            .cloned()
            .collect();
        for entry in spelled {
            entries.remove(&composed(&entry));
        }
    }
    saved
}

/// `entry` in Unicode's composed form (NFC), the form git reads a name in on a Mac that
/// precomposes names.
fn composed(entry: &str) -> String {
    entry.nfc().collect()
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

/// The lines that save `entry` of workspace `ws`: [`line_for`], and the same in the composed
/// form when the entry is not spelled in it.
pub(super) fn lines_for(ws: &str, entry: &str) -> Vec<String> {
    let mut lines = vec![line_for(ws, entry)];
    let nfc = composed(entry);
    if nfc != entry {
        lines.push(line_for(ws, &nfc));
    }
    lines
}

/// The entries workspace `ws` has saved with the project, as its plane's `.gitignore` stands.
pub fn saved(root: &Path, ws: &str) -> BTreeSet<String> {
    std::fs::read_to_string(root.join(".gitignore"))
        .map(|text| saved_in(&text).remove(ws).unwrap_or_default())
        .unwrap_or_default()
}

/// Whether `entry` of workspace `ws` holds a `.git` of any kind. git would commit such a folder
/// as a gitlink, never its files, so no line saves it, and a saved entry that has since become
/// one loses its line at the block's next rewrite.
pub(crate) fn holds_a_repo(root: &Path, ws: &str, entry: &str) -> bool {
    let dir = root.join("workspaces").join(ws);
    dir.join(entry).join(".git").symlink_metadata().is_ok()
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
        "'{0}' holds a repo, which has its own history, so purlis does not save it with the \
         project. Save the files beside it instead."
    )]
    IsARepo(String),
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
    if holds_a_repo(root, ws, entry) {
        return Err(NotSaved::IsARepo(entry.to_string()));
    }
    let mut not_live = false;
    let changed = super::rewrite_gitignore(root, |text| {
        let live = live_in(text);
        if !live.contains(ws) {
            not_live = true;
            return None;
        }
        let mut saved = saved_in(text);
        let entries = saved.entry(ws.to_string()).or_default();
        if workspace_lines(ws).contains(&line_for(ws, entry))
            || entries.iter().any(|one| composed(one) == composed(entry))
        {
            return None;
        }
        entries.insert(entry.to_string());
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

/// **Forget what workspace `ws` saved with the project** (#5552): its lines leave the LIVE
/// block, and the rest of the block is as it was. What removing a workspace does, so a new one
/// of the same name starts with nothing saved. `true` when a line went.
pub fn forget(root: &Path, ws: &str) -> io::Result<bool> {
    super::rewrite_gitignore(root, |text| {
        let mut saved = saved_in(text);
        saved.remove(ws)?;
        Some(super::with_block(
            text,
            &super::block_for(root, &live_in(text), &saved),
        ))
    })
}

/// **Where each file of one LIVE workspace is kept**, as the plane's git decides it.
#[derive(Debug)]
pub struct Marks {
    root: PathBuf,
    ws: String,
    /// Whether git reads names composed (NFC), as a Mac's git does with
    /// `core.precomposeunicode` on; otherwise it reads the bytes on disk.
    precomposes: bool,
}

impl Marks {
    /// The marks of workspace `ws`'s files, or `None` when it is not LIVE: a LOCAL workspace's
    /// files are all this machine's, and none is marked. A `.gitignore` that cannot be read is
    /// no LIVE block, so none either.
    pub fn of(root: &Path, ws: &str) -> Option<Self> {
        let text = std::fs::read_to_string(root.join(".gitignore")).ok()?;
        live_in(&text).contains(ws).then(|| Self {
            root: root.to_path_buf(),
            ws: ws.to_string(),
            precomposes: precomposes(root),
        })
    }

    /// Where each of `paths`, paths of the workspace's files, is kept, asked of git in one
    /// `check-ignore`: kept on this machine only when git ignores it, which it never says of a
    /// path the index tracks or of a folder holding one. Each is asked in the form git reads it
    /// in, composed when git precomposes names and as on disk otherwise, since `check-ignore`
    /// echoes what it is sent without precomposing it. A path git would read as a pattern
    /// (`check-ignore` matches its input as a pathspec and refuses `--literal-pathspecs`), or
    /// whose top entry no line can save, is this machine's without asking: it could match a
    /// tracked file it is not. `None` when git cannot answer, as in a project that is no git
    /// repo: then nothing is committed by anybody, and nothing marked.
    pub fn of_paths(&self, paths: &[&str]) -> Option<Vec<Kept>> {
        let asked: Vec<Option<String>> = paths
            .iter()
            .map(|path| {
                let path = path.trim_matches('/');
                let top = path.split('/').next().unwrap_or_default();
                if path.contains(['*', '?', '[', '\\']) || !one_entry(top) {
                    return None;
                }
                let path = format!("workspaces/{}/{path}", self.ws);
                Some(if self.precomposes {
                    composed(&path)
                } else {
                    path
                })
            })
            .collect();
        let mut input = Vec::new();
        for path in asked.iter().flatten() {
            input.extend_from_slice(path.as_bytes());
            input.push(0);
        }
        // Paths go to git as data on its standard input, never as arguments.
        let run = crate::worktree::git::run_with_input(
            &self.root,
            &["--no-optional-locks", "check-ignore", "--stdin", "-z"],
            input,
            crate::worktree::git::READ,
        )
        .ok()?;
        // 0: some are ignored; 1: none is. Anything else is git failing to answer.
        if !matches!(run.code, Some(0 | 1)) {
            return None;
        }
        let ignored: BTreeSet<&[u8]> = run
            .out
            .split(|byte| *byte == 0)
            .filter(|path| !path.is_empty())
            .collect();
        Some(
            asked
                .iter()
                .map(|path| match path {
                    Some(path) if !ignored.contains(path.as_bytes()) => Kept::WithTheProject,
                    _ => Kept::ThisMachineOnly,
                })
                .collect(),
        )
    }
}

/// Whether the plane's git reads names composed: only a Mac's git precomposes, and only with
/// `core.precomposeunicode` on.
fn precomposes(root: &Path) -> bool {
    cfg!(target_os = "macos")
        && crate::worktree::git::run(
            root,
            &["config", "--bool", "--get", "core.precomposeunicode"],
            crate::worktree::git::READ,
        )
        .is_ok_and(|run| run.ok() && run.out.trim() == "true")
}

#[cfg(test)]
mod tests;

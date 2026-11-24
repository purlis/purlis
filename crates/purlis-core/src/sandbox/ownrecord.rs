// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The one git record a chat in a branch folder purlis cut may write** (#7550).
//!
//! A linked worktree's git state (`HEAD`, `index`, `FETCH_HEAD`, `ORIG_HEAD`, `MERGE_HEAD`, the
//! `*.lock` files git makes beside them, `logs/`, the per-worktree `refs/`) lives in its clone's
//! `.git/worktrees/<name>/`, and the clone's `.git` is held whole from a chat
//! (`clones_git_held`, #1055, ruling V99h). Held whole, `git add`, `reset`, `merge` and
//! `fetch` fail in the chat's own folder on `index.lock` and `FETCH_HEAD`. This gives back that
//! one folder and nothing else of the clone's git:
//!
//! - **Which folder is derived, never named by the chat.** It is the record the folder's `.git`
//!   file points at, which the chat's sandbox holds unwritten ([`super::PLANTED`]: `.git`), and
//!   only where all of this agrees: the record is a real folder, never a link, directly in
//!   `worktrees` of a git folder the app holds from this chat already
//!   (`git_the_app_writes`); its `gitdir` names the folder's `.git` back; and its
//!   `commondir` names that git folder. Anything else grants nothing.
//! - **What stays held inside it:** `commondir` and `gitdir` (what points git elsewhere),
//!   `config.worktree` (config git reads when the repository has the extension on), `hooks`,
//!   `info` (attributes) and `glab-cli`: the per-worktree counterparts of what
//!   [`super::PLANTED`] holds in the clone.
//! - **What it does not give:** the refs a branch is (`refs/heads` of the clone), its config, its
//!   hooks, other worktrees' records and the shared objects. A commit still goes through the app
//!   (`purlis worktree commit`), which writes the branch ref and the objects; a fetch likewise
//!   (`purlis worktree update --fetch-only`). An object a chat's own git would write (`git add`
//!   of changed content, a merge commit) is refused by the shared store staying read-only: the
//!   shared store can be emptied by an `rm`, so it is not opened for a chat's git (#7550).

use std::path::{Path, PathBuf};

use super::{Access, Class, Denial, Reach};

/// The most a `.git` pointer, `gitdir` or `commondir` file holds.
const LARGEST: u64 = 8 * 1024;

/// What inside a record is never the chat's: each a path under the record, held with what is
/// below it.
pub const HELD_INSIDE: [&str; 6] = [
    "commondir",
    "gitdir",
    "config.worktree",
    "hooks",
    "info",
    "glab-cli",
];

/// A small file's text, read as it is once open: no link followed and no wait.
fn text(path: &Path) -> Option<String> {
    let bytes = super::arrived::read_regular(path, LARGEST + 1)?;
    (bytes.len() as u64 <= LARGEST)
        .then(|| String::from_utf8(bytes).ok())
        .flatten()
}

/// The record of the branch folder `folder`, where it is one a chat may write; `held` is what
/// the chat is denied, which must hold the git folder the record is in.
pub fn of(folder: &Path, held: &[Denial]) -> Option<PathBuf> {
    let folder = std::fs::canonicalize(folder).ok()?;
    let pointer = text(&folder.join(".git"))?;
    let named = pointer
        .trim_end_matches(['\n', '\r'])
        .strip_prefix("gitdir: ")?;
    let record = folder.join(named);
    // A real folder under its own name: a link there is never followed.
    std::fs::symlink_metadata(&record)
        .ok()?
        .file_type()
        .is_dir()
        .then_some(())?;
    let record = std::fs::canonicalize(&record).ok()?;
    let worktrees = record.parent()?;
    (worktrees.file_name()? == "worktrees").then_some(())?;
    let clone_git = worktrees.parent()?;
    // The git folder the app holds from this chat, whole: the clone's own.
    held.iter()
        .any(|denial| {
            denial.class == Class::LaterCode
                && denial.access == Access::Write
                && denial.reach == Reach::AndBelow
                && std::fs::canonicalize(&denial.path).is_ok_and(|path| path == clone_git)
        })
        .then_some(())?;
    // It names this folder back, and the clone's git folder as its common one.
    let back = text(&record.join("gitdir"))?;
    (std::fs::canonicalize(back.trim_end_matches(['\n', '\r'])).ok()? == folder.join(".git"))
        .then_some(())?;
    let common = text(&record.join("commondir"))?;
    (std::fs::canonicalize(record.join(common.trim_end_matches(['\n', '\r']))).ok()? == clone_git)
        .then_some(())?;
    Some(record)
}

/// The rules that give `record` back after the clone's git folder is denied, and hold what is
/// inside it ([`HELD_INSIDE`]) again: for the end of the profile's denials, where Seatbelt (and
/// the Linux sandbox that reads the same profile) takes the last rule that matches.
pub fn rules(record: &Path) -> Result<Vec<String>, &'static str> {
    let mut out = Vec::new();
    for name in super::seatbelt::kernel_strings(record)? {
        out.push(format!("(allow file-write* (subpath {name}))"));
    }
    for held in HELD_INSIDE {
        for name in super::seatbelt::kernel_strings(&record.join(held))? {
            out.push(format!("(deny file-write* (subpath {name}))"));
        }
    }
    Ok(out)
}

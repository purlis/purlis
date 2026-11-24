// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The worktree's git record, looked at before the app's git runs in it** (#7553).
//!
//! A chat that may write `.git/worktrees/<piece>/` (sandbox level off, or any later grant)
//! can plant a link where git writes a file in place, and git then writes through it into a
//! file the person owns. So before each call the app's git makes into a piece, the record and
//! every entry git reads or writes in it must be a regular file or a folder, never a link and
//! never a pipe, a socket or a device. Looked at with `symlink_metadata`, so no link is
//! followed. A name that is not there is fine: git makes it.

use std::path::{Path, PathBuf};

/// The files of a record that git reads, or writes in place.
const FILES: [&str; 4] = ["HEAD", "index", "COMMIT_EDITMSG", "MERGE_MSG"];

/// The folders of a record whose every entry git reads or writes: the ref log, and the state of a
/// rebase, an apply and a sequencer.
const FOLDERS: [&str; 4] = ["logs", "rebase-merge", "rebase-apply", "sequencer"];

/// How deep a folder is looked into: far past any state git keeps.
const DEEPEST: u8 = 16;

/// Why the app's git does not run in the piece whose git record is `git_dir`, or nothing: the
/// record, or one of its entries ([`FILES`], any `*_HEAD`, [`FOLDERS`] and everything in them),
/// is a link, or is not a regular file or a folder. What cannot be looked at is refused.
pub fn refused(git_dir: &Path) -> Result<(), String> {
    plain(git_dir, true)?;
    for name in FILES {
        plain(&git_dir.join(name), false)?;
    }
    let entries = std::fs::read_dir(git_dir).map_err(|e| unreadable(git_dir, &e))?;
    for entry in entries {
        let entry = entry.map_err(|e| unreadable(git_dir, &e))?;
        if entry.file_name().to_string_lossy().ends_with("_HEAD") {
            plain(&entry.path(), false)?;
        }
    }
    for name in FOLDERS {
        below(&git_dir.join(name), 0)?;
    }
    Ok(())
}

/// [`refused`], with the notes of the app's own that the record held before they moved out of
/// it (`purlis-brokered-commit`): one still there is a link or nothing.
pub fn refused_with(git_dir: &Path, also: &[&str]) -> Result<(), String> {
    refused(git_dir)?;
    for name in also {
        plain(&git_dir.join(name), false)?;
    }
    Ok(())
}

/// Why `path` is not a regular file or a folder, or nothing where it is not there. A folder is
/// asked for where `folder` is set.
fn plain(path: &Path, folder: bool) -> Result<(), String> {
    let meta = match std::fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound && !folder => return Ok(()),
        Err(e) => return Err(unreadable(path, &e)),
    };
    let kind = meta.file_type();
    if kind.is_symlink() {
        return Err(refusal(path, "a link"));
    }
    if !(kind.is_file() || kind.is_dir()) || (folder && !kind.is_dir()) {
        return Err(refusal(path, "neither a file nor a folder"));
    }
    Ok(())
}

/// `dir` and everything below it, each a regular file or a folder.
fn below(dir: &Path, depth: u8) -> Result<(), String> {
    plain(dir, false)?;
    let Ok(meta) = std::fs::symlink_metadata(dir) else {
        return Ok(());
    };
    if !meta.is_dir() {
        return Ok(());
    }
    if depth >= DEEPEST {
        return Err(refusal(dir, "nested deeper than git ever nests it"));
    }
    for entry in std::fs::read_dir(dir).map_err(|e| unreadable(dir, &e))? {
        let entry = entry.map_err(|e| unreadable(dir, &e))?;
        below(&entry.path(), depth + 1)?;
    }
    Ok(())
}

fn refusal(path: &Path, what: &str) -> String {
    format!(
        "{} is {what}, where git keeps a file of this branch folder's own record, so purlis \
         runs no git in it for a chat. Nothing was run. Remove it from your own terminal, then \
         ask again",
        shown(path)
    )
}

fn unreadable(path: &Path, why: &std::io::Error) -> String {
    format!(
        "{} could not be looked at ({why}), so purlis runs no git in this branch folder for a \
         chat. Nothing was run",
        shown(path)
    )
}

/// The record's path, as the sentence names it: its last two parts below `.git/worktrees`.
fn shown(path: &Path) -> String {
    let parts: Vec<_> = path.components().rev().take(3).collect();
    let tail: PathBuf = parts.iter().rev().collect();
    format!("`{}`", tail.display())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn a_record_of_plain_files_and_folders_is_fine_and_a_missing_one_is_made_by_git() {
        let dir = tempfile::tempdir().unwrap();
        let git_dir = dir.path().join("piece");
        std::fs::create_dir_all(git_dir.join("logs/refs")).unwrap();
        std::fs::write(git_dir.join("HEAD"), "ref: refs/heads/piece\n").unwrap();
        std::fs::write(git_dir.join("logs/HEAD"), "").unwrap();
        assert_eq!(refused(&git_dir), Ok(()));
    }

    #[test]
    fn a_link_anywhere_in_it_is_refused_and_a_link_as_the_record_itself_too() {
        let dir = tempfile::tempdir().unwrap();
        let git_dir = dir.path().join("piece");
        std::fs::create_dir_all(git_dir.join("rebase-merge/deeper")).unwrap();
        let aside = dir.path().join("aside");
        std::fs::write(&aside, "x").unwrap();
        std::os::unix::fs::symlink(&aside, git_dir.join("rebase-merge/deeper/done")).unwrap();
        assert!(refused(&git_dir).unwrap_err().contains("a link"));

        let linked = dir.path().join("linked-record");
        std::os::unix::fs::symlink(&git_dir, &linked).unwrap();
        assert!(refused(&linked).unwrap_err().contains("a link"));
    }
}

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
        let name = entry.file_name().to_string_lossy().into_owned();
        if name != "HEAD" && (name.ends_with("_HEAD") || name == "AUTO_MERGE") {
            plain(&entry.path(), false)?;
            not_a_symbolic_ref(&entry.path())?;
        }
    }
    for name in FOLDERS {
        below(&git_dir.join(name), 0)?;
    }
    Ok(())
}

/// Why the pseudo-ref at `path` (`ORIG_HEAD`, `REBASE_HEAD`, `AUTO_MERGE`, any `*_HEAD`) is a
/// symbolic one, or nothing (#7594): git writes an id in each, never `ref: <name>`, and one that
/// holds `ref:` makes a rebase, a merge or a reset move the ref it names (a regular file with
/// `ref: refs/heads/<another branch>` in `ORIG_HEAD` moved that branch). Where it is not there,
/// or not a regular file, [`plain`] has said so.
fn not_a_symbolic_ref(path: &Path) -> Result<(), String> {
    let Ok(meta) = std::fs::symlink_metadata(path) else {
        return Ok(());
    };
    if !meta.is_file() {
        return Ok(());
    }
    let mut head = Vec::new();
    let read = std::fs::File::open(path).and_then(|file| {
        use std::io::Read as _;
        file.take(64).read_to_end(&mut head)
    });
    read.map_err(|e| unreadable(path, &e))?;
    if head.trim_ascii_start().starts_with(b"ref:") {
        return Err(refusal(
            path,
            "a symbolic ref (`ref: <name>`), which makes git move that ref",
        ));
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

/// The note of the branch a piece was cut for, in the app's notes for the folder
/// (`app/worktree-updates/<id>/`): written when the piece is cut, outside every git directory
/// and written through no link, so no chat writes it.
pub const BRANCH_NOTE: &str = "purlis-branch";

/// The branch a branch folder was cut for, and whether the app only noted it now.
#[derive(Debug, PartialEq, Eq)]
pub struct Own {
    pub branch: String,
    /// The piece was cut before the app kept this note: see [`own_branch`].
    pub noted_now: bool,
}

/// Records `branch` as the one the piece at `folder` was cut for (#7594). Called by the cut.
pub fn note_branch(root: &Path, folder: &Path, branch: &str) -> Result<(), String> {
    let git_dir = git_dir_of(folder)?;
    let notes = crate::pieceupdate::notes_of(root, &git_dir);
    crate::pieceupdate::note(root, &notes, BRANCH_NOTE, &format!("{branch}\n"))
        .map_err(|e| e.to_string())
}

fn git_dir_of(folder: &Path) -> Result<PathBuf, String> {
    let said = super::git::run(
        folder,
        &["rev-parse", "--absolute-git-dir"],
        super::git::READ,
    )
    .map_err(|e| e.to_string())?;
    if !said.ok() || said.line().is_empty() {
        return Err("git did not name this folder's git directory".to_owned());
    }
    Ok(PathBuf::from(said.line()))
}

/// The branch the piece whose git record is `git_dir` was cut for: what the cut noted
/// ([`BRANCH_NOTE`]), and never the folder's name, which `--branch` makes a different string
/// (`plane-root-gh` is on `fix/6494-plane-root-gh-token`) and which a chat can choose.
///
/// A piece cut before the app kept the note has none. It is **noted now, from the branch `HEAD`
/// names**, where that branch is the folder's own name or one the clone recorded a base for
/// (only a cut writes that, `branch.<name>.charterBase`): a branch a chat repointed `HEAD` to
/// has neither, so it is refused. That is trust on first use, and cheaper than refusing every
/// piece cut so far; `noted_now` lets the caller say so.
pub fn own_branch(root: &Path, git_dir: &Path, folder: &Path) -> Result<Own, String> {
    let notes = crate::pieceupdate::notes_of(root, git_dir);
    if let Ok(text) = std::fs::read_to_string(notes.join(BRANCH_NOTE)) {
        let branch = text.trim();
        if super::name::branch_name_ok(branch).is_ok() {
            return Ok(Own {
                branch: branch.to_owned(),
                noted_now: false,
            });
        }
    }
    let named = std::fs::read(git_dir.join("HEAD"))
        .ok()
        .and_then(|bytes| {
            let text = String::from_utf8(bytes).ok()?;
            Some(text.trim().strip_prefix("ref: refs/heads/")?.to_owned())
        })
        .ok_or_else(|| {
            "this folder's HEAD names no branch, and purlis kept no note of the one it was cut              for, so it does not act on it for a chat"
                .to_owned()
        })?;
    let piece = folder.file_name().and_then(|name| name.to_str());
    let clone = git_dir
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent);
    let cut_here = piece == Some(named.as_str())
        || clone.is_some_and(|clone| super::recorded_base(clone, &named).is_some());
    if !cut_here || super::name::branch_name_ok(&named).is_err() {
        return Err(format!(
            "this folder's HEAD names {}, which purlis did not cut it for (it has no note of \
             the branch, and {} is neither this folder's name nor a branch with a recorded \
             base), so it does not act on it for a chat. Nothing was run. Put the folder back \
             on its own branch from your own terminal",
            shown_name(&named),
            shown_name(&named)
        ));
    }
    // Best effort: where it cannot be written, the next call decides the same way.
    let _ = crate::pieceupdate::note(root, &notes, BRANCH_NOTE, &format!("{named}\n"));
    Ok(Own {
        branch: named,
        noted_now: true,
    })
}

fn shown_name(branch: &str) -> String {
    format!("`{}`", branch.chars().take(80).collect::<String>())
}

/// Why `git_dir`'s `HEAD` is not what the app's git may run on, or the piece's own branch where
/// it names one (#7594). On a branch (`on` is the branch git reports, never empty): `HEAD` is
/// exactly `ref: refs/heads/<the branch the cut noted>`, and `on` is that branch. Off one (a
/// rebase the app stopped): `HEAD` is a bare commit id, never a `ref:` that could move another
/// branch. Also refused: a link on the shared reflog of the branch (`logs/refs/heads/<branch>`
/// in the clone's own `.git`), which a commit, an update or a push appends to.
pub fn held(root: &Path, git_dir: &Path, folder: &Path, on: &str) -> Result<Option<Own>, String> {
    let head =
        std::fs::read(git_dir.join("HEAD")).map_err(|e| unreadable(&git_dir.join("HEAD"), &e))?;
    let head = String::from_utf8_lossy(&head);
    let head = head.trim_end_matches(['\n', '\r']);
    if on.is_empty() {
        let bare = matches!(head.len(), 40 | 64) && head.bytes().all(|b| b.is_ascii_hexdigit());
        return if bare {
            Ok(None)
        } else {
            Err(
                "this folder is mid-rebase and its HEAD is not a bare commit id, so purlis does \
                 not run git in it for a chat. Nothing was run. Put it right from your own \
                 terminal"
                    .to_owned(),
            )
        };
    }
    let own = own_branch(root, git_dir, folder)?;
    if head != format!("ref: refs/heads/{}", own.branch) || on != own.branch {
        return Err(format!(
            "this folder (the piece) was cut for branch {} and its HEAD names {}, so purlis \
             will not run git on it for a chat. Nothing was run. Put the folder back on its own \
             branch from your own terminal",
            shown_name(&own.branch),
            shown_name(on)
        ));
    }
    if let Some(common) = git_dir.parent().and_then(Path::parent) {
        let mut at = common.join("logs");
        plain(&at, false)?;
        for part in ["refs", "heads"].into_iter().chain(own.branch.split('/')) {
            at.push(part);
            plain(&at, false)?;
        }
    }
    Ok(Some(own))
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

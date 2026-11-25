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

/// What a worktree's record may hold at its top, by name, case-folded (git opens `orig_head` as
/// `ORIG_HEAD` on a case-insensitive disk, so every name is compared folded): the files git and
/// the app's own updates leave there. Anything else is refused, and not looked at further, because
/// git reads more names than any list of bad ones could hold (`MERGE_MODE`, `config.worktree`,
/// `info/`, `modules/` …). Sources: `path.c` (`common_list`'s per-worktree `HEAD`, `index`,
/// `logs`, `ORIG_HEAD`, `*_HEAD`, `MERGE_*`, `COMMIT_EDITMSG`, `SQUASH_MSG`, `AUTO_MERGE`),
/// `builtin/worktree.c` (`commondir`, `gitdir`, `locked`) and `fetch` (`FETCH_HEAD`).
const FILES: [&str; 17] = [
    "head",
    "commondir",
    "gitdir",
    "locked",
    "index",
    "orig_head",
    "fetch_head",
    "commit_editmsg",
    "merge_msg",
    "merge_head",
    "merge_mode",
    "auto_merge",
    "rebase_head",
    "cherry_pick_head",
    "revert_head",
    "squash_msg",
    "merge_rr",
];

/// The folders of a record that git writes in: the ref log, and the state of a rebase. A
/// `rebase-apply` and a `sequencer` are not here: the app's update never leaves either (it
/// rebases with the merge backend), and what is in them (`sequencer/opts`) names programs.
const FOLDERS: [&str; 2] = ["logs", "rebase-merge"];

/// What `rebase-merge/` may hold, case-folded: the state the app's own `git rebase` leaves
/// (the merge backend, `sequencer.c`'s `write_basic_state`, `rebase_path_*` and `todo_list` files,
/// and `rebase.c`). Left out on purpose, each of them a file git acts on: `refs-to-delete` (deletes
/// any branch it names on `--continue` or `--abort`), `autostash`, `strategy`, `strategy_opts`,
/// `gpg_sign_opt`, `allow_rerere_autoupdate`, `squash-onto`, `rewritten/` and `dropped/`.
const REBASE_FILES: [&str; 22] = [
    "author-script",
    "done",
    "drop_redundant_commits",
    "end",
    "git-rebase-todo",
    "git-rebase-todo.backup",
    "git-rebase-todo.tmp",
    "head-name",
    "interactive",
    "message",
    "msgnum",
    "no-reschedule-failed-exec",
    "onto",
    "orig-head",
    "patch",
    "stopped-sha",
    "amend",
    "current-fixups",
    "message-fixup",
    "message-squash",
    "rewritten-list",
    "rewritten-pending",
];

/// How deep a folder is looked into: far past any state git keeps.
const DEEPEST: u8 = 16;

/// Why the app's git does not run in the piece whose git record is `git_dir`, or nothing: the
/// record holds an entry that is not on the allowlist ([`FILES`], `*.lock`, [`FOLDERS`] and, in
/// `rebase-merge/`, [`REBASE_FILES`]), by name case-folded and by type, or one is a link, a pipe, a
/// socket or a device, or a pseudo-ref holds a symbolic one. What cannot be looked at is refused.
pub fn refused(git_dir: &Path) -> Result<(), String> {
    // The record of a linked worktree (`<clone>/.git/worktrees/<id>`) holds only what git and the
    // app leave there. A clone's own `.git` also holds the config, the hooks, the objects and
    // the refs, so there the names that are not listed are not looked at, and the folders that
    // are listed are looked into as the record's are.
    let linked = git_dir
        .parent()
        .and_then(Path::file_name)
        .is_some_and(|name| name == "worktrees");
    plain(git_dir, true)?;
    let entries = std::fs::read_dir(git_dir).map_err(|e| unreadable(git_dir, &e))?;
    for entry in entries {
        let entry = entry.map_err(|e| unreadable(git_dir, &e))?;
        let path = entry.path();
        let name = folded(&entry);
        let a_file = FILES.contains(&name.as_str()) || is_lock(&name);
        let a_folder = FOLDERS.contains(&name.as_str())
            || (!linked && matches!(name.as_str(), "rebase-apply" | "sequencer"));
        if !a_file && !a_folder {
            if linked {
                plain(&path, false)?;
                return Err(not_listed(&path));
            }
            continue;
        }
        plain(&path, false)?;
        let is_dir = std::fs::symlink_metadata(&path).is_ok_and(|meta| meta.is_dir());
        if is_dir && a_folder {
            if name == "rebase-merge" {
                rebase_state(&path)?;
            } else {
                below(&path, 0)?;
            }
        } else if !is_dir && a_file {
            if name != "head" && (name.ends_with("_head") || name == "auto_merge") {
                not_a_symbolic_ref(&path)?;
            }
        } else {
            return Err(not_listed(&path));
        }
    }
    Ok(())
}

/// An entry's name, lower-cased: the case-insensitive disk opens it as the listed name.
fn folded(entry: &std::fs::DirEntry) -> String {
    entry.file_name().to_string_lossy().to_lowercase()
}

/// A lock git holds while it writes (`index.lock`): made with `O_EXCL`, so never written through.
fn is_lock(name: &str) -> bool {
    name.ends_with(".lock")
}

/// `rebase-merge/`: only the files of [`REBASE_FILES`] (and locks), each a regular file.
fn rebase_state(dir: &Path) -> Result<(), String> {
    for entry in std::fs::read_dir(dir).map_err(|e| unreadable(dir, &e))? {
        let entry = entry.map_err(|e| unreadable(dir, &e))?;
        let path = entry.path();
        plain(&path, false)?;
        let name = folded(&entry);
        let is_file = std::fs::symlink_metadata(&path).is_ok_and(|meta| meta.is_file());
        if !is_file || !(REBASE_FILES.contains(&name.as_str()) || is_lock(&name)) {
            return Err(not_listed(&path));
        }
    }
    Ok(())
}

fn not_listed(path: &Path) -> String {
    refusal(
        path,
        "not something git or purlis's own update leaves in it (it may make git act on a file \
         or a branch the chat chose)",
    )
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
/// A piece cut before the app kept the note has none. It is **noted now only where `HEAD` names a
/// branch called exactly as the folder is**, and no other worktree has that branch checked out
/// (#7594): `HEAD` is a file a chat can write, so a `HEAD` naming anything else (another piece's
/// branch, a `--branch` name) proves nothing about what the piece was cut for, and is refused.
/// That is trust on first use for the one shape a cut without `--branch` makes; `noted_now` lets
/// the caller say so.
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
    if piece != Some(named.as_str())
        || super::name::branch_name_ok(&named).is_err()
        || clone.is_none_or(|clone| held_elsewhere(clone, folder, &named))
    {
        return Err(format!(
            "this folder has no note of the branch purlis cut it for (it was cut before purlis \
             kept one), and its HEAD names {}, which is not the folder's own name or is checked \
             out in another folder, so purlis does not act on it for a chat. Nothing was run. \
             Cut the piece again, or put the folder back on the branch of its own name from \
             your own terminal",
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

/// Whether a worktree of `clone` other than `folder` has `branch` checked out, or whether that
/// cannot be told (then it is taken as held).
fn held_elsewhere(clone: &Path, folder: &Path, branch: &str) -> bool {
    let Ok(said) = super::git::run(
        clone,
        &["worktree", "list", "--porcelain"],
        super::git::READ,
    ) else {
        return true;
    };
    if !said.ok() {
        return true;
    }
    let canon = |path: &str| std::fs::canonicalize(path).unwrap_or_else(|_| PathBuf::from(path));
    let mine = std::fs::canonicalize(folder).unwrap_or_else(|_| folder.to_path_buf());
    let wanted = format!("branch refs/heads/{branch}");
    let mut at: Option<PathBuf> = None;
    for line in said.out.lines() {
        if let Some(path) = line.strip_prefix("worktree ") {
            at = Some(canon(path));
        } else if line == wanted && at.as_ref().is_some_and(|at| *at != mine) {
            return true;
        }
    }
    false
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
        std::fs::create_dir_all(git_dir.join("rebase-merge")).unwrap();
        let aside = dir.path().join("aside");
        std::fs::write(&aside, "x").unwrap();
        std::os::unix::fs::symlink(&aside, git_dir.join("rebase-merge/done")).unwrap();
        assert!(refused(&git_dir).unwrap_err().contains("a link"));

        let linked = dir.path().join("linked-record");
        std::os::unix::fs::symlink(&git_dir, &linked).unwrap();
        assert!(refused(&linked).unwrap_err().contains("a link"));
    }

    #[test]
    fn a_linked_record_holds_only_listed_names_whatever_their_case() {
        let dir = tempfile::tempdir().unwrap();
        let git_dir = dir.path().join("worktrees/piece");
        std::fs::create_dir_all(git_dir.join("rebase-merge")).unwrap();
        std::fs::write(git_dir.join("HEAD"), "ref: refs/heads/piece\n").unwrap();
        std::fs::write(git_dir.join("MERGE_MODE"), "no-ff").unwrap();
        std::fs::write(git_dir.join("rebase-merge/head-name"), "refs/heads/piece\n").unwrap();
        assert_eq!(refused(&git_dir), Ok(()));

        std::fs::write(
            git_dir.join("rebase-merge/Refs-To-Delete"),
            "refs/heads/x\n",
        )
        .unwrap();
        assert!(refused(&git_dir).unwrap_err().contains("not something git"));
        std::fs::remove_file(git_dir.join("rebase-merge/Refs-To-Delete")).unwrap();

        std::fs::write(git_dir.join("Orig_Head"), "ref: refs/heads/x\n").unwrap();
        assert!(refused(&git_dir).unwrap_err().contains("symbolic ref"));
        std::fs::remove_file(git_dir.join("Orig_Head")).unwrap();

        std::fs::write(git_dir.join("config.worktree"), "[core]\n").unwrap();
        assert!(refused(&git_dir).unwrap_err().contains("not something git"));
    }
}

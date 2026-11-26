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

/// The names git reads or writes as **state**, case-folded (git opens `orig_head` as `ORIG_HEAD`
/// on a case-insensitive disk, so every name is compared folded): each must be a regular file,
/// never a link, a folder or a device. Any `*_head` and `auto_merge` also must not hold `ref:`
/// ([`not_a_symbolic_ref`]), because a rebase, a merge or a reset then moves the ref it names.
/// `MERGE_*` is the merge's own (`MERGE_MSG`, `MERGE_MODE`, `MERGE_RR` …). Sources: `path.c`
/// (`common_list`'s per-worktree `HEAD`, `index`, `logs`, `ORIG_HEAD`, `*_HEAD`, `MERGE_*`,
/// `COMMIT_EDITMSG`, `SQUASH_MSG`, `AUTO_MERGE`), `builtin/worktree.c` (`commondir`, `gitdir`,
/// `locked`) and `fetch` (`FETCH_HEAD`).
const STATE_FILES: [&str; 8] = [
    "head",
    "commondir",
    "gitdir",
    "locked",
    "index",
    "auto_merge",
    "commit_editmsg",
    "squash_msg",
];

/// Whether `name` (folded) is a file git reads or writes as state ([`STATE_FILES`], `*_head`, `merge_*`).
fn is_state_file(name: &str) -> bool {
    STATE_FILES.contains(&name) || name.ends_with("_head") || name.starts_with("merge_")
}

/// What `rebase-merge/` may hold, case-folded: the state the app's own `git rebase` leaves
/// (the merge backend, `sequencer.c`'s `write_basic_state`, `rebase_path_*` and `todo_list` files,
/// and `rebase.c`). Left out on purpose, each of them a file git acts on: `refs-to-delete` (deletes
/// any branch it names on `--continue` or `--abort`), `update-refs`, `autostash`, `strategy`,
/// `strategy_opts`, `gpg_sign_opt`, `allow_rerere_autoupdate`, `squash-onto`, `rewritten/` and
/// `dropped/`.
const REBASE_FILES: [&str; 23] = [
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
    "reschedule-failed-exec",
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

/// Why the app's git does not run in the piece whose git record is `git_dir`, or nothing.
///
/// **A hybrid rule (#7594, round 4): refuse what git acts on, ignore what it does not.** Every
/// entry at the top of a linked record must be no link, whatever its name. The names git reads or
/// writes as state are then held to their kind and content ([`is_state_file`], `logs/`,
/// `rebase-merge/` and its allowlist, a non-empty `refs/`), and a few are refused outright
/// (`config.worktree`, `sequencer/`, `rebase-apply/`). Every other name is **not looked inside**:
/// a forge CLI's empty `glab-cli/`, `gc.log`, the app's legacy `purlis-updated`, `modules/` (git
/// uses it only for a submodule operation, and the app's git runs none: every call carries
/// `submodule.recurse=false`, a fetch `--no-recurse-submodules`, a push `push.recurseSubmodules=no`),
/// `fsmonitor--daemon*`, `sharedindex.*`, `BISECT_*`. Refusing those would refuse most records
/// git and the person's own tools leave. What cannot be looked at is refused.
pub fn refused(git_dir: &Path) -> Result<(), String> {
    // The record of a linked worktree (`<clone>/.git/worktrees/<id>`) holds only what git and the
    // app leave there. A clone's own `.git` also holds the config, the hooks, the objects and
    // the refs, so there only the names git acts on are looked at, as the record's are.
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
        let acted_on = is_state_file(&name)
            || matches!(
                name.as_str(),
                "logs" | "rebase-merge" | "rebase-apply" | "sequencer"
            );
        if !linked && !acted_on {
            continue;
        }
        // A link is refused wherever it is, by any name.
        plain(&path, false)?;
        let is_dir = std::fs::symlink_metadata(&path).is_ok_and(|meta| meta.is_dir());
        match name.as_str() {
            "logs" | "rebase-merge" if !is_dir => {
                return Err(wrong_kind(&path, "a file", "a folder"));
            }
            "logs" => below(&path, 0)?,
            "rebase-merge" => rebase_state(&path)?,
            "rebase-apply" | "sequencer" if linked => return Err(left_over(&path)),
            "rebase-apply" | "sequencer" => below(&path, 0)?,
            "config.worktree" if linked => return Err(config_worktree(&path)),
            "refs" if linked => no_refs(&path, 0)?,
            _ if is_state_file(&name) => {
                if is_dir {
                    return Err(wrong_kind(&path, "a folder", "a file"));
                }
                if name != "head" && (name.ends_with("_head") || name == "auto_merge") {
                    not_a_symbolic_ref(&path)?;
                }
            }
            _ => {}
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

/// `refs/` of a linked record: git reads per-worktree refs there (`refs/bisect/*`, `refs/rewritten/*`),
/// so one that holds a file is refused. Folders alone (what a bisect or `rebase -r` leaves
/// behind) hold no ref and are ignored.
fn no_refs(dir: &Path, depth: u8) -> Result<(), String> {
    if depth >= DEEPEST {
        return Err(refusal(dir, "nested deeper than git ever nests it"));
    }
    for entry in std::fs::read_dir(dir).map_err(|e| unreadable(dir, &e))? {
        let entry = entry.map_err(|e| unreadable(dir, &e))?;
        let path = entry.path();
        plain(&path, false)?;
        if std::fs::symlink_metadata(&path).is_ok_and(|meta| meta.is_dir()) {
            no_refs(&path, depth + 1)?;
        } else {
            return Err(refusal(
                &path,
                "a ref kept for this folder alone, which git reads as a branch the chat chose",
            ));
        }
    }
    Ok(())
}

fn wrong_kind(path: &Path, is: &str, wanted: &str) -> String {
    refusal(path, &format!("{is}, where git keeps {wanted}"))
}

/// `sequencer/` and `rebase-apply/` hold the state of a cherry-pick, a revert or an `--apply`
/// rebase, which names programs (`sequencer/opts`) and which the app's own update never leaves.
fn left_over(path: &Path) -> String {
    refusal(
        path,
        "the state of a cherry-pick, a revert or a rebase the app did not start (it can name a \
         program or a branch for git to act on)",
    )
}

/// `config.worktree` is read by git on every call (`core.hooksPath`, `core.fsmonitor` and the
/// like) and the app's `-c` settings do not cover every key in it.
fn config_worktree(path: &Path) -> String {
    refusal(
        path,
        "a git config kept for this folder alone (`git sparse-checkout` and `git config --worktree` leave one), \
         which git reads on every call and can make it run a program, and purlis's own settings \
         do not cover every key in it. Move it aside from your own terminal, or run git in this \
         folder yourself instead of asking purlis",
    )
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
        "not something git or purlis's own update leaves in a rebase's state (it may make git \
         act on a file or a branch the chat chose)",
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
            "this folder's HEAD names no branch, and purlis kept no note of the one it was cut \
             for, so it does not act on it for a chat"
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
        let repo = clone
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            .unwrap_or("<repo>");
        let piece = piece.unwrap_or("<piece>");
        return Err(format!(
            "this folder has no note of the branch purlis cut it for (it was cut before purlis \
             kept one), and its HEAD names {}, which is not the folder's own name or is checked \
             out in another folder, so purlis does not act on it for a chat. Nothing was run. \
             Check from your own terminal that the folder is on the branch it was cut for, then \
             run `purlis worktree adopt {repo} {piece}` there once to record it (add `-w \
             <workspace>` outside the active workspace)",
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

/// `purlis worktree adopt`: records the branch the piece at `folder` is on as the one it was cut
/// for (#7594, round 4), for a piece cut before the app kept the note whose folder is not named
/// as its branch (`--branch`), which [`own_branch`] cannot note by itself. Run by the person from
/// their own terminal, never by a chat: it trusts what the folder is on now. Refused where
/// another worktree has that branch checked out, where `HEAD` names no branch, and where the
/// record holds what [`refused`] refuses. Says what was recorded, and what was there.
pub fn adopt(root: &Path, clone: &Path, folder: &Path) -> Result<String, String> {
    let git_dir = git_dir_of(folder)?;
    if !crate::pieceupdate::own_record_of(clone, &git_dir, folder) {
        return Err(
            "this folder keeps its git directory somewhere other than its clone's \
             `.git/worktrees`, so purlis does not adopt it"
                .to_owned(),
        );
    }
    refused(&git_dir)?;
    let said = super::git::run(folder, &["branch", "--show-current"], super::git::READ)
        .map_err(|e| e.to_string())?;
    let branch = said.line().to_owned();
    if !said.ok() || branch.is_empty() {
        return Err("this folder is on no branch, so there is none to record".to_owned());
    }
    super::name::branch_name_ok(&branch).map_err(|e| e.to_string())?;
    if held_elsewhere(clone, folder, &branch) {
        return Err(format!(
            "{} is checked out in another branch folder (or that could not be told), so it is not \
             this folder's to adopt",
            shown_name(&branch)
        ));
    }
    let notes = crate::pieceupdate::notes_of(root, &git_dir);
    let before = std::fs::read_to_string(notes.join(BRANCH_NOTE))
        .ok()
        .map(|text| text.trim().to_owned());
    crate::pieceupdate::note(root, &notes, BRANCH_NOTE, &format!("{branch}\n"))
        .map_err(|e| e.to_string())?;
    Ok(match before {
        Some(before) if before != branch => format!(
            "recorded {} as the branch this folder was cut for (the note said {})",
            shown_name(&branch),
            shown_name(&before)
        ),
        _ => format!(
            "recorded {} as the branch this folder was cut for",
            shown_name(&branch)
        ),
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
            "this folder was cut for branch {} and its HEAD names {}, so purlis \
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
        "{} is {what}, so purlis runs no git in this branch folder for a chat. Nothing was run. \
         Remove it from your own terminal, then ask again",
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
    fn a_linked_record_refuses_what_git_acts_on_whatever_the_case_and_ignores_the_rest() {
        let dir = tempfile::tempdir().unwrap();
        let git_dir = dir.path().join("worktrees/piece");
        std::fs::create_dir_all(git_dir.join("rebase-merge")).unwrap();
        std::fs::write(git_dir.join("HEAD"), "ref: refs/heads/piece\n").unwrap();
        std::fs::write(git_dir.join("MERGE_MODE"), "no-ff").unwrap();
        std::fs::write(git_dir.join("rebase-merge/head-name"), "refs/heads/piece\n").unwrap();
        std::fs::write(git_dir.join("rebase-merge/reschedule-failed-exec"), "").unwrap();
        // The survey's residue: ignored, and not looked inside.
        std::fs::create_dir_all(git_dir.join("glab-cli")).unwrap();
        std::fs::create_dir_all(git_dir.join("modules/sub")).unwrap();
        std::fs::create_dir_all(git_dir.join("refs/bisect")).unwrap();
        std::fs::write(git_dir.join("gc.log"), "x").unwrap();
        std::fs::write(git_dir.join("purlis-updated"), "").unwrap();
        std::fs::write(git_dir.join("sharedindex.abc"), "").unwrap();
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

        std::fs::write(git_dir.join("refs/bisect/bad"), "0\n").unwrap();
        assert!(
            refused(&git_dir)
                .unwrap_err()
                .contains("ref kept for this folder alone")
        );
        std::fs::remove_file(git_dir.join("refs/bisect/bad")).unwrap();

        std::fs::write(git_dir.join("config.worktree"), "[core]\n").unwrap();
        assert!(
            refused(&git_dir)
                .unwrap_err()
                .contains("git config kept for this folder alone")
        );
    }
}

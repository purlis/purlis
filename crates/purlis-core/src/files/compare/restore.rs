// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A turn's files put back as its start had them** (#5411; ADR 0081 as amended 2026-11-05):
//! **Undo a turn**, every file the turn changed, and the turn's side of Discard, one hunk of one
//! file ([`super::discard`]). Both read the turn's start from its ref ([`super::super::turns`]),
//! and both write as a discard writes, through the folder held open ([`write_back`]).
//!
//! **A file is the turn's while it is as the turn ended it** ([`as_it_ended`]): its bytes on disk
//! are the end's blob, as they are or as `git add` would store them, and a path the turn removed is
//! not there. A file changed since then refuses the undo, naming the file, and a turn's diff draws
//! that file from the turn's start to the file as it is now, so each hunk left can be discarded.
//!
//! **Only the latest turn not yet undone can be undone** ([`super::super::turns::offered_on`]),
//! and the turn's `undone` ref, written once the files are back, makes the one before it next.
//! Whether the folder is idle is the host's to check, under the run's state lock, in the same step
//! as this write (ADR 0084 §8): the host holds the chats.
//!
//! **Nothing starts, not even git**: the start's and end's blobs are gitoxide's, the writes are
//! this process's own, and every check is made before the first write.

use std::path::Path;

use super::super::{Branch, Refused};
use super::{
    Comparison, Kind, Side, ToGit, Whitespace, compared_in, line_diff_as, on_disk, side_at, tree_of,
};
use crate::humanedit::{EditKind, EditedFile, HumanEdit};

/// The files a turn changed, every path a write would touch, sorted: what Undo's confirmation
/// lists. A rename is both of its paths.
pub(super) fn paths_of(files: &[super::FileChange]) -> Vec<String> {
    let mut paths: Vec<String> = files
        .iter()
        .flat_map(|change| [Some(change.path.clone()), change.from.clone()])
        .flatten()
        .collect();
    paths.sort_unstable();
    paths.dedup();
    paths
}

/// Whether `path` is on disk as `end` has it. `Err` names why it cannot be told.
pub(super) fn as_it_ended(
    repo: &gix::Repository,
    held: &super::super::Held,
    to_git: Option<&mut ToGit<'_>>,
    end: Option<&gix::Tree<'_>>,
    path: &str,
) -> Result<bool, &'static str> {
    let (ended, ended_kind) = side_at(repo, end, path, super::super::LARGEST);
    let (disk, disk_kind) = on_disk(held, path, super::super::LARGEST);
    match (&ended, &disk) {
        (Side::Missing, Side::Missing) => Ok(true),
        (Side::Missing, Side::Bytes(_)) | (Side::Bytes(_), Side::Missing) => Ok(false),
        (Side::Bytes(blob), Side::Bytes(bytes)) => {
            if ended_kind == Kind::Submodule {
                return Err("is a submodule");
            }
            if ended_kind != Kind::of_entry(disk_kind) {
                return Ok(false);
            }
            Ok(blob == bytes
                || (ended_kind == Kind::File
                    && to_git.is_some_and(|to_git| to_git.clean(path, bytes.clone()) == *blob)))
        }
        (Side::TooLarge(_), _) | (_, Side::TooLarge(_)) => Err("is past the size purlis compares"),
        (Side::Unreadable, _) | (_, Side::Unreadable) => Err("cannot be read"),
    }
}

/// One path of an undo, checked and ready to write.
enum Back {
    /// The start has no file there.
    Gone,
    /// The start's bytes, with its mode, and line ends turned to CRLF where the folder's are.
    File {
        bytes: Vec<u8>,
        executable: bool,
        crlf: bool,
    },
}

/// What an undo did: the record to keep.
pub type UndoneTurn = HumanEdit;

/// [`undo_turn`](super::super::undo_turn), in this process: only the bounded reader's child calls
/// it. Every file `turn` changed is checked first, and then each is written as the turn's start had
/// it. It writes no ref: the host marks the turn undone once this answers.
pub(in super::super) fn undo_here(
    plane: &Path,
    branch: Branch<'_>,
    turn: &super::TurnKey,
) -> Result<UndoneTurn, Refused> {
    let opened = super::open(plane, branch)?;
    let on = opened
        .repo
        .head_name()
        .ok()
        .flatten()
        .map(|name| name.shorten().to_string())
        .ok_or_else(|| not_undone("the folder is on no branch"))?;
    let offered = super::super::turns::offered_on(&opened.repo, &on);
    if offered.as_ref() != Some(turn) {
        return Err(not_undone(&match offered {
            Some(latest) if latest.chat == turn.chat => format!(
                "only the latest turn not yet undone can be, and that is turn {}",
                latest.number
            ),
            Some(_) => {
                "only the latest turn not yet undone can be, and that is another chat's".to_string()
            }
            None => "this branch has no turn left to undo".to_string(),
        }));
    }
    let comparison = Comparison::Turn {
        chat: turn.chat.clone(),
        number: turn.number,
    };
    let compared = compared_in(plane, branch, &opened, &comparison, Whitespace::Shown)?;
    if compared.more > 0 || !compared.refused.is_empty() {
        return Err(not_undone(
            "it changed more files than purlis compares, or ones it cannot read",
        ));
    }
    let unreadable = |why: String| Refused::Unreadable {
        what: format!("the turns of {}", branch.called()),
        why,
    };
    let start = compared.sides.base.as_deref().and_then(super::commit_id);
    let end = match &compared.sides.head {
        super::Head::Commit(hex) => super::commit_id(hex),
        super::Head::WorkingTree => None,
    };
    let start_tree = tree_of(&opened.repo, start).map_err(unreadable)?;
    let end_tree = tree_of(&opened.repo, end).map_err(unreadable)?;
    let held = super::hold(plane, &opened.base).map_err(unreadable)?;
    let paths = paths_of(&compared.files);
    let attributes = super::HeldAttributes::read(
        &mut opened.repo.clone(),
        Some(&held),
        paths.iter().map(String::as_str),
    );
    let mut to_git = ToGit::of(&attributes);

    let mut planned = Vec::with_capacity(paths.len());
    for path in &paths {
        let refused = |why: &str| Refused::NotUndone {
            path: path.clone(),
            why: format!("{why}. Nothing was changed"),
        };
        if super::discard::converted_content(&attributes, path) {
            return Err(refused(
                "is converted by the repo with a working-tree-encoding or ident, which purlis \
                 does not run",
            ));
        }
        match as_it_ended(
            &opened.repo,
            &held,
            to_git.as_mut(),
            end_tree.as_ref(),
            path,
        ) {
            Ok(true) => {}
            Ok(false) => return Err(refused("changed since the turn ended")),
            Err(why) => return Err(refused(why)),
        }
        let (was, kind) = side_at(
            &opened.repo,
            start_tree.as_ref(),
            path,
            super::super::LARGEST,
        );
        let back = match (was, kind) {
            (Side::Missing, _) => Back::Gone,
            (Side::Bytes(bytes), Kind::File) => {
                let executable = start_tree
                    .as_ref()
                    .and_then(|tree| tree.lookup_entry_by_path(path.as_str()).ok().flatten())
                    .is_some_and(|entry| entry.mode().is_executable());
                let crlf = crlf_here(
                    &opened.repo,
                    end_tree.as_ref(),
                    &held,
                    to_git.as_mut(),
                    path,
                );
                Back::File {
                    bytes,
                    executable,
                    crlf,
                }
            }
            (Side::Bytes(_), Kind::Link) => {
                return Err(refused("was a link before the turn"));
            }
            (Side::Bytes(_), Kind::Submodule) => return Err(refused("is a submodule")),
            (Side::TooLarge(_), _) => return Err(refused("is past the size purlis compares")),
            (Side::Unreadable, _) => return Err(refused("cannot be read")),
        };
        planned.push((path.clone(), back));
    }

    let mut files = Vec::with_capacity(planned.len());
    for (path, back) in planned {
        let at = Path::new(&path);
        let ended = side_at(
            &opened.repo,
            end_tree.as_ref(),
            &path,
            super::super::LARGEST,
        );
        let lines = match &back {
            Back::File { bytes, .. } => match ended.0.bytes() {
                Some(end)
                    if !ended.0.is_binary()
                        && !bytes[..bytes.len().min(super::SNIFF)].contains(&0) =>
                {
                    line_diff_as(bytes, end, Whitespace::Shown)
                        .0
                        .into_iter()
                        .map(super::discard::lines_of)
                        .collect()
                }
                _ => Vec::new(),
            },
            Back::Gone => Vec::new(),
        };
        write_back(&held, at, back).map_err(|why| Refused::NotUndone {
            path: path.clone(),
            why: format!(
                "could not be written ({why}). The files before it in the list are back as the \
                 turn found them"
            ),
        })?;
        files.push(EditedFile { path, lines });
    }
    Ok(HumanEdit {
        kind: EditKind::UndoneTurn,
        repo: branch.repo.to_string(),
        piece: branch.piece.map(str::to_string),
        files,
        busy: false,
        chats: Vec::new(),
        chat_ids: Vec::new(),
    })
}

/// Whether the folder writes `path` with CRLF line ends that `git add` turns into LF, as the file
/// the turn ended with shows: its bytes are not its blob, and are once cleaned. A file the turn
/// removed shows nothing, and is written with the start's own bytes.
fn crlf_here(
    repo: &gix::Repository,
    end: Option<&gix::Tree<'_>>,
    held: &super::super::Held,
    to_git: Option<&mut ToGit<'_>>,
    path: &str,
) -> bool {
    let Some(to_git) = to_git else {
        return false;
    };
    let (ended, _) = side_at(repo, end, path, super::super::LARGEST);
    match (&ended, on_disk(held, path, super::super::LARGEST)) {
        (Side::Bytes(blob), (Side::Bytes(disk), _)) => {
            blob != &disk && to_git.clean(path, disk) == *blob
        }
        _ => false,
    }
}

/// `bytes` with each line's LF made CRLF, where it is not already.
fn with_crlf(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    for line in bytes.split_inclusive(|byte| *byte == b'\n') {
        match line.strip_suffix(b"\n") {
            Some(text) if !text.ends_with(b"\r") => {
                out.extend_from_slice(text);
                out.extend_from_slice(b"\r\n");
            }
            _ => out.extend_from_slice(line),
        }
    }
    out
}

/// One path put back: removed where the start had nothing, else written whole.
fn write_back(held: &super::super::Held, at: &Path, back: Back) -> std::io::Result<()> {
    match back {
        Back::Gone => match super::super::remove_inside(held, at) {
            Err(why) if why.kind() == std::io::ErrorKind::NotFound => Ok(()),
            done => done,
        },
        Back::File {
            bytes,
            executable,
            crlf,
        } => {
            let bytes = if crlf { with_crlf(&bytes) } else { bytes };
            super::super::replace_inside(held, at, &bytes, executable)
        }
    }
}

fn not_undone(why: &str) -> Refused {
    Refused::Read(format!(
        "The turn was not undone: {why}. Nothing was changed."
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_ends_become_crlf_once_and_a_last_line_without_one_stays_so() {
        assert_eq!(with_crlf(b"a\nb\r\nc"), b"a\r\nb\r\nc");
        assert_eq!(with_crlf(b""), b"");
    }
}

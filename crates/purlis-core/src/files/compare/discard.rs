// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Discard a hunk** (#5406; ADR 0081 as amended 2026-11-05, ADR 0084 §8): one hunk of a file's
//! "what changed" put back as the branch's base has it, or a binary file's change whole. The
//! files module's first write, under the guards its reads have.
//!
//! **The window names a branch and a path, never a directory.** The path is confined as a read's
//! is, then written through the folder held open, one component at a time and following no link
//! ([`super::super::replace_inside`]), by a temporary file renamed over it, so a reader or a kill
//! finds the old file or the new one and never half of either.
//!
//! **Nothing starts, not even git.** The base's lines are gitoxide's blob, the hunks are the
//! comparison's own, and the splice is on the file's bytes in the bounded reader's child, as the
//! comparison is read. So no program a repo names can run, and no setting of its own
//! (`apply.whitespace`, say) can make the write differ from the hunk drawn. A file the repo runs
//! through a `filter`, a `working-tree-encoding` or `ident` is refused: its blob is not what its
//! file holds, and only running or redoing those would turn one into the other.
//!
//! **The stale check never merges** (ADR 0084 §8). A [`stamp`](stamp_of) drawn with the diff names
//! the base and the file's bytes; if either moved the answer is [`Discarded::Stale`], and the
//! window offers Reload, which draws it again, or Overwrite ([`Against::Overwrite`]), which
//! writes the file as it was drawn less the hunk, over whatever is there.
//!
//! It answers the [`HumanEdit`] to record: the host holds the event log and the chats, so the
//! host records it and says who was busy (`app/src-tauri/src/humanedits.rs`).

use std::path::Path;

use sha2::{Digest as _, Sha256};

use super::super::{Branch, Refused};
use super::{
    Comparison, FileDiff, HeldAttributes, Hunk, Kind, Shown, Side, ToGit, WhatChanged, Whitespace,
    commit_id, compared_in, file_diff_in, hold, line_diff_as, on_disk, plain, side_at, tree_of,
    what_changed_here,
};
use crate::humanedit::{EditKind, EditedFile, EditedLines, HumanEdit};

/// What to put back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Target {
    /// One hunk of a text file, as the diff drew it.
    Hunk(Hunk),
    /// A binary file's change, which has no hunks.
    Whole,
}

/// What the discard is checked against.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Against {
    /// The stamp the diff was drawn with ([`WhatChanged::stamp`]): stale if it no longer holds.
    Opened(String),
    /// The operator's Overwrite: no check, and the file written as it was drawn less the hunk.
    /// `head` is the head side as drawn, for a [`Target::Hunk`]; a text file drawn with bytes that
    /// are not UTF-8 lost them in the drawing and is refused.
    Overwrite { head: Option<String> },
}

/// One discard the window asks for.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Discard {
    /// The file, on the head's side, relative to the branch's folder.
    pub path: String,
    pub target: Target,
    pub against: Against,
}

/// What a discard did.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Discarded {
    /// It is written. `edit` is the record to keep, and `now` the file's diff without the hunk, or
    /// `None` when nothing of its change is left to draw.
    Done {
        edit: HumanEdit,
        now: Option<Box<WhatChanged>>,
    },
    /// The file or the base moved since the diff was drawn, and nothing was written.
    Stale,
}

/// What the file holds on disk and the base it is compared with, as a digest: what a diff is
/// drawn with and a discard checked against. `None` for a file that cannot be read as one.
pub(super) fn stamp_of(
    base: Option<&str>,
    (side, kind): &(Side, gix::object::tree::EntryKind),
) -> Option<String> {
    let mut digest = Sha256::new();
    digest.update(base.unwrap_or_default());
    digest.update(format!("\0{kind:?}\0"));
    match side {
        Side::Bytes(bytes) => digest.update(bytes),
        Side::Missing => digest.update(b"missing"),
        Side::TooLarge(_) | Side::Unreadable => return None,
    }
    Some(
        digest
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
    )
}

/// [`discard`](super::super::discard), in this process: only the bounded reader's child calls it.
pub(in super::super) fn discard_here(
    plane: &Path,
    branch: Branch<'_>,
    ask: &Discard,
) -> Result<Discarded, Refused> {
    let opened = super::open(plane, branch)?;
    let (relative, _) = super::super::named_or_gone(
        &super::super::Kept::of(plane, &opened.base, branch),
        &opened.base,
        &ask.path,
    )?;
    let compared = compared_in(
        plane,
        branch,
        &opened,
        &Comparison::BranchAndUncommitted,
        Whitespace::Shown,
    )?;
    let Some(change) = compared.files.into_iter().find(|one| one.path == relative) else {
        return Err(Refused::NotChanged(ask.path.clone()));
    };
    let path = plain(&change.path).ok_or_else(|| Refused::NotInPiece(change.path.clone()))?;
    let from = match change.from.as_deref() {
        Some(from) => Some(plain(from).ok_or_else(|| Refused::NotInPiece(from.to_string()))?),
        None => None,
    };
    let refused = |why: &str| Refused::NotDiscarded {
        path: path.clone(),
        why: why.to_string(),
    };
    let unreadable = |why: String| Refused::Unreadable {
        what: path.clone(),
        why,
    };
    let held = hold(plane, &opened.base).map_err(unreadable)?;
    let on_it = on_disk(&held, &path, super::super::LARGEST);
    let base = compared.sides.base.as_deref();
    let opened_stamp = match &ask.against {
        Against::Opened(stamp) => Some(stamp.as_str()),
        Against::Overwrite { .. } => None,
    };
    if opened_stamp.is_some() && stamp_of(base, &on_it).as_deref() != opened_stamp {
        return Ok(Discarded::Stale);
    }

    let diff = file_diff_in(
        plane,
        branch,
        &opened,
        &compared.sides,
        path.clone(),
        from.clone(),
        Whitespace::Shown,
    )?;
    let old_path = from.as_deref().unwrap_or(&path);
    let tree = tree_of(&opened.repo, base.and_then(commit_id)).map_err(unreadable)?;
    let (old, old_kind) = side_at(&opened.repo, tree.as_ref(), old_path, super::super::LARGEST);
    let (disk, disk_kind) = &on_it;
    if old_kind != Kind::File || Kind::of_entry(*disk_kind) != Kind::File {
        return Err(refused("it is a link or a submodule on one side"));
    }
    let attributes = HeldAttributes::read(
        &mut opened.repo.clone(),
        Some(&held),
        [old_path, path.as_str()],
    );
    if converted(&attributes, old_path) || converted(&attributes, &path) {
        return Err(refused(
            "the repo runs it through a filter or a conversion that purlis does not run",
        ));
    }
    let (Some(base_bytes), Some(disk_bytes)) = (old.bytes(), disk.bytes()) else {
        return Err(refused(
            "it is past what the comparison draws, or cannot be read",
        ));
    };

    let (bytes, lines) = match (ask.target, &diff, &ask.against) {
        (Target::Hunk(hunk), FileDiff::Text { hunks, .. }, Against::Opened(_)) => {
            if !hunks.contains(&hunk) {
                return Ok(Discarded::Stale);
            }
            // A file `git add` would change only in its line ends keeps its own (#704's eol
            // conversion): the base's lines are written with the ends the file has.
            let crlf = ToGit::of(&attributes).is_some_and(|mut to_git| {
                to_git.clean(&path, disk_bytes.to_vec()).as_slice() != disk_bytes
            });
            let Some(spliced) = spliced(disk_bytes, base_bytes, hunk, crlf) else {
                return Ok(Discarded::Stale);
            };
            (spliced, vec![lines_of(hunk)])
        }
        (Target::Hunk(hunk), FileDiff::Text { .. }, Against::Overwrite { head }) => {
            let head = head.as_deref().unwrap_or_default();
            if head.contains(char::REPLACEMENT_CHARACTER) {
                return Err(refused(
                    "it holds bytes that are not text, which the diff does not draw as they are. \
                     Reload it, then discard",
                ));
            }
            let drew = line_diff_as(base_bytes, head.as_bytes(), Whitespace::Shown).0;
            let Some(spliced) = drew
                .contains(&hunk)
                .then(|| spliced(head.as_bytes(), base_bytes, hunk, false))
                .flatten()
            else {
                return Err(refused("the hunk is not one of the diff drawn. Reload it"));
            };
            (spliced, vec![lines_of(hunk)])
        }
        (Target::Whole, FileDiff::Binary, _) => (base_bytes.to_vec(), Vec::new()),
        (Target::Hunk(_), FileDiff::Binary, Against::Opened(_)) => return Ok(Discarded::Stale),
        (Target::Whole, FileDiff::Text { .. }, Against::Opened(_)) => return Ok(Discarded::Stale),
        _ => {
            return Err(refused(
                "it is past what the comparison draws, or the diff drawn is not this one",
            ));
        }
    };

    // What is written, checked once more against the file as close to the write as can be.
    if opened_stamp.is_some()
        && stamp_of(base, &on_disk(&held, &path, super::super::LARGEST)).as_deref() != opened_stamp
    {
        return Ok(Discarded::Stale);
    }
    let at = Path::new(&path);
    let wrote = if matches!(old, Side::Missing) && bytes.is_empty() {
        super::super::remove_inside(&held, at)
    } else {
        let executable = tree
            .as_ref()
            .and_then(|tree| tree.lookup_entry_by_path(old_path).ok().flatten())
            .is_some_and(|entry| entry.mode().is_executable());
        super::super::replace_inside(&held, at, &bytes, executable)
    };
    wrote.map_err(|why| Refused::NotDiscarded {
        path: path.clone(),
        why: format!("it could not be written: {why}"),
    })?;

    let now = match what_changed_here(plane, branch, &path) {
        Ok(Shown::Changed(shown)) => Some(Box::new(shown)),
        Ok(Shown::NotListed { .. }) | Err(_) => None,
    };
    Ok(Discarded::Done {
        edit: HumanEdit {
            kind: EditKind::DiscardedHunk,
            repo: branch.repo.to_string(),
            piece: branch.piece.map(str::to_string),
            files: vec![EditedFile { path, lines }],
            busy: false,
            chats: Vec::new(),
            chat_ids: Vec::new(),
        },
        now,
    })
}

/// Whether the attributes have git do more to `path` than its line ends on its way to and from
/// its blob: a `filter`, a `working-tree-encoding` or `ident`. If they cannot be read, not: as the
/// comparison reads it then, by its bytes.
fn converted(attributes: &HeldAttributes, path: &str) -> bool {
    use gix::attrs::StateRef;
    let Some(mut stack) = attributes.stack() else {
        return false;
    };
    let mut outcome =
        stack.selected_attribute_matches(["filter", "working-tree-encoding", "ident"]);
    let Ok(at) = stack.at_path(Path::new(path), Some(gix::index::entry::Mode::FILE)) else {
        return false;
    };
    at.matching_attributes(&mut outcome);
    outcome.iter_selected().any(|matched| {
        !matches!(
            matched.assignment.state,
            StateRef::Unspecified | StateRef::Unset
        )
    })
}

/// `head` with `hunk`'s lines replaced by what `base` has there, in `git diff -U0`'s numbers.
/// `None` when the file has no such lines. `crlf`: the base's lines are written ending as a
/// working tree that converts them does.
fn spliced(head: &[u8], base: &[u8], hunk: Hunk, crlf: bool) -> Option<Vec<u8>> {
    let lines = |bytes: &'_ [u8]| -> Vec<Vec<u8>> {
        bytes
            .split_inclusive(|byte| *byte == b'\n')
            .map(<[u8]>::to_vec)
            .collect()
    };
    let (head, base) = (lines(head), lines(base));
    let (at, gone) = (
        from_line(hunk.new_start, hunk.new_lines),
        hunk.new_lines as usize,
    );
    let (from, back) = (
        from_line(hunk.old_start, hunk.old_lines),
        hunk.old_lines as usize,
    );
    let (kept_before, replaced) = (head.get(..at)?, base.get(from..from + back)?);
    let kept_after = head.get(at + gone..)?;
    let mut out = kept_before.concat();
    for line in replaced {
        match line.strip_suffix(b"\n") {
            Some(text) if crlf && !text.ends_with(b"\r") => {
                out.extend_from_slice(text);
                out.extend_from_slice(b"\r\n");
            }
            _ => out.extend_from_slice(line),
        }
    }
    out.extend_from_slice(&kept_after.concat());
    Some(out)
}

/// How many lines come before a side of a hunk: `-U0` names the line a side of no lines comes
/// after, and otherwise its first line.
fn from_line(start: u32, lines: u32) -> usize {
    if lines == 0 {
        start as usize
    } else {
        start.saturating_sub(1) as usize
    }
}

/// Where `hunk` left the file: the base's lines now stand where the head's did.
fn lines_of(hunk: Hunk) -> EditedLines {
    let before = from_line(hunk.new_start, hunk.new_lines) as u32;
    EditedLines {
        start: before + u32::from(hunk.old_lines > 0),
        count: hunk.old_lines,
    }
}

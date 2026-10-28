// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The one diff engine** (RC-2, #704; ADR 0084 §1–§2, R1, R4): every comparison charter
//! shows — a chat's branch against its base, any two refs, what is not committed, a cross-repo
//! change's members — is computed here, and nothing else in charter computes one. The
//! explorer's change markers ([`super::status`]) are this engine's file list without the line
//! counts, and the Review tab and "Show what changed" (RC-4, FM-11) draw what it answers.
//!
//! **A comparison is a base and a head in one repo** (ADR 0084 §1). [`Comparison`] names R1's
//! kinds; [`compare`] resolves one to its two [`Sides`] and answers the file list, and
//! [`compare_file`] answers one file's hunks between the sides the list was computed at, so a
//! large comparison paints its list first and a file's lines when its row is drawn (§2).
//!
//! **Read by gitoxide in the bounded reader's child, starting no program** (V88a, D-88f,
//! D-88h; amending ADR 0084 §2's "runs git as a program", which predates them). Every repo a
//! comparison reads is one an agent can write, so it is opened as [`super::status`] opens a
//! branch — the config read once and cut to the keys a read needs — and read in a child of
//! charter's own binary that is killed past a deadline and a memory cap. No diff driver, text
//! conversion, filter, external diff or pager the repo names can run, because nothing here
//! runs anything: the lines are compared in-process with git's own algorithm (Myers, then
//! git's indent heuristic), and a file's working-tree content is read through a descriptor held
//! from the project's root, one component at a time, following no link.
//!
//! **What git reads besides the bytes, read the same way** (#704). The `diff` attribute decides
//! binary before the bytes do, read from the working tree's attribute files as `git diff` reads
//! them, each opened through the folder held open following no link, as the content is (train
//! 80 XG); a driver's own settings are config this reader does not keep, so a named driver
//! leaves it to the bytes. A working-tree file is compared as `git add` would store it: its line ends
//! converted by gitoxide's built-in eol conversion (`core.autocrlf`, `core.eol`, and the `text`
//! and `eol` attributes), which is code in this process and starts no program; a `filter`
//! driver is never run. A type change is drawn as git draws it, the old kind out and the new
//! one in, and a submodule's pointer as git's `Subproject commit` line, the commit it names
//! never read. A submodule's folder that checked out another commit is that commit on the
//! working tree's side, as `git add` would stage it, read from its git folder as plain files
//! and never by following its `.git` file's `gitdir:` line.
//!
//! **Whitespace hidden is git's `-w`** ([`Whitespace`], R3): recomputed here, never filtered in
//! the window. **A path the engine will not read is named, never dropped** (#704): a `.git`
//! component in any case, or a name that is not a plain path, is in [`Compared::refused`] with
//! its mark, and nothing at it is opened.
//!
//! **What git answers is the reference** (RC-2's acceptance): each kind's file list, marks,
//! renames, line counts and hunks are checked against `git diff` on fixtures
//! (`tests/it/a_comparison_matches_git_diff.rs`).

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::{Path, PathBuf};

use super::status::{Change, MARKED, Mark, Opened, RefusedChange, open, plain, refused_for};
use super::{Branch, Refused};
use crate::worktree::name;

/// What to compare (R1). Each kind names its two sides (ADR 0084 §1's table).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Comparison {
    /// (a) The branch checked out in the folder, from where it left its base to its last
    /// commit. Its base is the one charter recorded when it cut the branch, else the repo's
    /// default branch.
    Branch,
    /// (a) with what is not committed yet: from where the branch left its base to the working
    /// tree. A branch with no base recorded is compared from its last commit. The explorer's
    /// markers, and a file's "Show what changed" (FM-11).
    BranchAndUncommitted,
    /// (a) for a branch the repo holds by name, checked out or not: a cross-repo change's
    /// member (e).
    NamedBranch { branch: String },
    /// (b) Any two refs: from where they meet (their merge base) to `to`, or from `from`
    /// itself when `exact`.
    Refs {
        from: String,
        to: String,
        exact: bool,
    },
    /// (c) What is not committed: the folder's last commit to its working tree, untracked
    /// files git does not ignore included, the index never touched.
    Uncommitted,
}

/// The head of a comparison.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Head {
    /// A commit, by its full hex id.
    Commit(String),
    /// The branch folder's files as they are on disk.
    WorkingTree,
}

/// The two sides a comparison was resolved to: what [`compare_file`] is handed back, so a
/// file's hunks are of the same comparison as the list, however the branch moved since.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Sides {
    /// The base commit, by its full hex id; `None` for nothing (a branch with no commit yet).
    pub base: Option<String>,
    pub head: Head,
}

/// How many lines a file's change added and removed, as `git diff --numstat` counts them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Lines {
    pub added: u64,
    pub removed: u64,
}

/// How a comparison compares lines (R3, ADR 0084 §2): what "Hide whitespace" asks of the engine.
/// It changes only the line counts and the hunks, never the file list: which files changed,
/// their marks and their renames are git's whatever is hidden, as `git diff -w --name-status`
/// lists them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Whitespace {
    /// Every byte is compared, as `git diff` compares.
    #[default]
    Shown,
    /// Whitespace is not compared, as `git diff -w` compares: two lines are the same when
    /// they are once their spaces, tabs and line ends are taken out. A file whose only change
    /// is whitespace is still listed, with no lines added or removed and no hunk, where git's
    /// `--numstat -w` leaves it out.
    Hidden,
}

/// One file of a comparison.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FileChange {
    /// Its path relative to the branch's folder, on the head's side.
    pub path: String,
    pub mark: Mark,
    /// Where a renamed file came from: a name, never followed.
    pub from: Option<String>,
    /// Lines added and removed; `None` when either side is binary or past [`COUNTED`].
    pub lines: Option<Lines>,
    /// Whether git would call either side binary: as its `diff` attribute says (`-diff` and the
    /// `binary` macro, binary; `diff` set, text), else a NUL in its first 8,000 bytes.
    pub binary: bool,
    /// Whether the change is not committed yet (a working-tree head only).
    pub uncommitted: bool,
}

/// A comparison's file list: the first step (ADR 0084 §2).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Compared {
    /// The sides it was computed at: hand them to [`compare_file`].
    pub sides: Sides,
    /// The base's name — a branch, a ref as given, or a commit — or `None` when there is none.
    pub base: Option<String>,
    /// Sorted by path, the first [`MARKED`].
    pub files: Vec<FileChange>,
    /// The paths it changed that are named and never read (#704): a `.git` component in any
    /// case, or a name that is not a plain path. Sorted by path, the first [`MARKED`]; neither
    /// side of one is read, so none has line counts or hunks.
    pub refused: Vec<RefusedChange>,
    /// How many files past [`MARKED`] are not in `files`, and refused ones not in `refused`.
    pub more: usize,
}

/// One hunk, in `git diff -U0`'s numbers: lines counted from 1, and a side with no lines naming
/// the line the hunk comes after (`0` for the top). The window's `GitHunk` (`hunks.ts`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Hunk {
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
}

/// One file of a comparison, the second step (ADR 0084 §2): both sides and git's hunks.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FileDiff {
    /// Text: each side (empty where the file is absent) and the hunks between them. Bytes that
    /// are not UTF-8 are drawn as U+FFFD; the hunks are git's, computed on the bytes.
    Text {
        base: String,
        head: String,
        hunks: Vec<Hunk>,
    },
    /// A side git would call binary: drawn as a sentence (R3's previews are RC-6's).
    Binary,
    /// A side past [`super::LARGEST`], by its size.
    TooLarge { bytes: u64 },
}

/// The most bytes of one side a file list counts the lines of: 16 MiB. Past it the file is
/// listed without counts, which keeps one comparison inside the reader's memory cap.
pub const COUNTED: u64 = 16 * 1024 * 1024;

/// How much of a side is looked at to call it binary: git's `buffer_is_binary`.
const SNIFF: usize = 8000;

/// A comparison of `branch`, read by `reader`: its sides and its file list (see the module).
pub fn compare(
    reader: &super::Reader,
    plane: &Path,
    branch: Branch<'_>,
    comparison: &Comparison,
) -> Result<Compared, Refused> {
    compare_with(reader, plane, branch, comparison, Whitespace::Shown)
}

/// [`compare`], its line counts compared as `whitespace` says (R3).
pub fn compare_with(
    reader: &super::Reader,
    plane: &Path,
    branch: Branch<'_>,
    comparison: &Comparison,
    whitespace: Whitespace,
) -> Result<Compared, Refused> {
    let ask = super::Ask::Compare {
        comparison: comparison.clone(),
        whitespace,
    };
    match reader.ask(plane, branch, ask)? {
        super::Answer::Compared(compared) => Ok(compared),
        _ => Err(Refused::Read("the reader answered something else".into())),
    }
}

/// One file of a comparison of `branch` at `sides` (from [`compare`]), read by `reader`: its
/// path on the head's side, and `from`, its path on the base's side when it was renamed.
pub fn compare_file(
    reader: &super::Reader,
    plane: &Path,
    branch: Branch<'_>,
    sides: &Sides,
    path: &str,
    from: Option<&str>,
) -> Result<FileDiff, Refused> {
    compare_file_with(reader, plane, branch, sides, path, from, Whitespace::Shown)
}

/// [`compare_file`], its hunks compared as `whitespace` says (R3).
pub fn compare_file_with(
    reader: &super::Reader,
    plane: &Path,
    branch: Branch<'_>,
    sides: &Sides,
    path: &str,
    from: Option<&str>,
    whitespace: Whitespace,
) -> Result<FileDiff, Refused> {
    let ask = super::Ask::CompareFile {
        sides: sides.clone(),
        path: path.to_string(),
        from: from.map(str::to_string),
        whitespace,
    };
    match reader.ask(plane, branch, ask)? {
        super::Answer::FileDiff(diff) => Ok(diff),
        _ => Err(Refused::Read("the reader answered something else".into())),
    }
}

/// One file of a branch against the branch's base: what "Show what changed" draws (FM-11).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct WhatChanged {
    /// The file as the branch's file list has it: its mark, and where it came from if renamed.
    pub change: FileChange,
    /// The base's name, as [`Compared::base`]; `None` when compared against the last commit.
    pub base: Option<String>,
    /// Its lines and hunks, or the kind that is drawn as a sentence instead.
    pub diff: FileDiff,
}

/// One file of `branch`, by its path relative to the branch's folder, compared against the
/// branch's base with what is not committed yet ([`Comparison::BranchAndUncommitted`], the
/// explorer's markers' own comparison): "Show what changed" (FM-11, #1103 story 21).
///
/// **The path is confined first, as every file command confines one** ([`super::named`]):
/// refused when it is empty, absolute or walks up, names git's folder in any case, or reaches
/// its file through a link; nothing is read for it then. Then the comparison's file list is
/// read, and a file not in it is refused with [`Refused::NotChanged`], never answered with an
/// empty diff. Its hunks are read at the sides the list was read at, so the two agree however
/// the branch moved between them.
///
/// **One ask of the reader, and nothing of the branch read in this process** (#1189). The
/// path's own spelling is checked here, before any child starts. Everything that needs the
/// branch — finding its folder, confining the path in it, the file list, the file's hunks — is
/// done in one child, which finds the folder as the status read does ([`super::status`]),
/// starting no git. A refusal the child answers with keeps its sentence word for word.
pub fn what_changed(
    reader: &super::Reader,
    plane: &Path,
    branch: Branch<'_>,
    path: &str,
) -> Result<WhatChanged, Refused> {
    super::spelled_inside(path)?;
    let ask = super::Ask::WhatChanged {
        path: path.to_string(),
    };
    match reader.ask(plane, branch, ask)? {
        super::Answer::WhatChanged(Shown::Changed(shown)) => Ok(shown),
        super::Answer::WhatChanged(Shown::NotListed { more: 0 }) => {
            Err(Refused::NotChanged(path.to_string()))
        }
        super::Answer::WhatChanged(Shown::NotListed { .. }) => Err(Refused::Read(format!(
            "purlis compares the first {MARKED} files this branch changed, and '{path}' is not \
             among them"
        ))),
        _ => Err(Refused::Read("the reader answered something else".into())),
    }
}

/// What the reader's child answers "Show what changed" with: the file compared, or that the
/// comparison's file list does not hold it — and how many files past [`MARKED`] it left out, so
/// the app says whether the file is unchanged or only not among the first.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Shown {
    Changed(WhatChanged),
    NotListed { more: usize },
}

/// [`what_changed`], in this process: only the bounded reader's child calls it. The branch is
/// opened once, and the path confined in its resolved folder before anything else is read.
pub(super) fn what_changed_here(
    plane: &Path,
    branch: Branch<'_>,
    path: &str,
) -> Result<Shown, Refused> {
    let opened = open(plane, branch)?;
    let relative = super::named_in(
        &super::Kept::of(plane, &opened.base, branch),
        &opened.base,
        path,
    )?;
    let compared = compared_in(
        plane,
        branch,
        &opened,
        &Comparison::BranchAndUncommitted,
        Whitespace::Shown,
    )?;
    let Some(change) = compared.files.into_iter().find(|one| one.path == relative) else {
        return Ok(Shown::NotListed {
            more: compared.more,
        });
    };
    let Some(at) = plain(&change.path) else {
        return Err(Refused::NotInPiece(change.path));
    };
    let from = match change.from.as_deref() {
        Some(from) => Some(plain(from).ok_or_else(|| Refused::NotInPiece(from.to_string()))?),
        None => None,
    };
    let diff = file_diff_in(
        plane,
        branch,
        &opened,
        &compared.sides,
        at,
        from,
        Whitespace::Shown,
    )?;
    Ok(Shown::Changed(WhatChanged {
        change,
        base: compared.base,
        diff,
    }))
}

/// One member of a cross-repo change, compared (e).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MemberCompared {
    pub repo: String,
    pub branch: String,
    /// The member's branch compared as (a), or why it could not be.
    pub compared: Result<Compared, String>,
}

/// A cross-repo change compared (R1 (e), ADR 0084 §1): one comparison per member, as (a) for the
/// member's branch in the workspace's clone of its repo, in the order its `needs` give (ADR
/// 0060): a member after every member it needs, and otherwise in the record's order.
pub fn compare_change(
    reader: &super::Reader,
    plane: &Path,
    ws: &str,
    slug: &str,
) -> Result<Vec<MemberCompared>, Refused> {
    let record = crate::change::store::read(plane, ws, slug)
        .map_err(|e| Refused::Read(format!("purlis could not read the change {slug}: {e}")))?;
    let mut out = Vec::new();
    for member in needs_order(&record.members) {
        let comparison = Comparison::NamedBranch {
            branch: member.branch.clone(),
        };
        let compared = compare(reader, plane, Branch::repo(ws, &member.repo), &comparison)
            .map_err(|refused| refused.to_string());
        out.push(MemberCompared {
            repo: member.repo.clone(),
            branch: member.branch.clone(),
            compared,
        });
    }
    Ok(out)
}

/// The members, each after every member it needs, ties in the record's order. A record that
/// validated has no cycle; one that has is answered in the record's order past the cycle.
fn needs_order(members: &[crate::change::Member]) -> Vec<&crate::change::Member> {
    let mut placed: BTreeSet<&str> = BTreeSet::new();
    let mut out = Vec::with_capacity(members.len());
    while out.len() < members.len() {
        let next = members.iter().find(|m| {
            !placed.contains(m.repo.as_str())
                && m.needs
                    .iter()
                    .all(|n| placed.contains(n.as_str()) || !members.iter().any(|o| &o.repo == n))
        });
        let next = next.or_else(|| members.iter().find(|m| !placed.contains(m.repo.as_str())));
        let Some(next) = next else { break };
        placed.insert(next.repo.as_str());
        out.push(next);
    }
    out
}

// ---------------------------------------------------------------------------------------------
// In the reader's child
// ---------------------------------------------------------------------------------------------

/// A head, resolved.
#[derive(Clone, Copy)]
pub(super) enum At {
    Commit(gix::ObjectId),
    WorkingTree,
}

/// Whether a change at `path`, moved from `from`, is left out of a list of a place's changes:
/// either name is something purlis keeps to itself ([`super::Kept::listed_private`]).
pub(super) fn kept_from_a_list(kept: &super::Kept, path: &str, from: Option<&str>) -> bool {
    kept.listed_private(Path::new(path))
        || from.is_some_and(|from| kept.listed_private(Path::new(from)))
}

/// [`compare`], in this process: only the bounded reader's child calls it.
pub(super) fn compare_here(
    plane: &Path,
    branch: Branch<'_>,
    comparison: &Comparison,
    whitespace: Whitespace,
) -> Result<Compared, Refused> {
    compared_in(plane, branch, &open(plane, branch)?, comparison, whitespace)
}

/// [`compare_here`] of a branch already opened: so one child that reads a comparison and then
/// one of its files opens the branch once ([`what_changed_here`]).
fn compared_in(
    plane: &Path,
    branch: Branch<'_>,
    opened: &Opened,
    comparison: &Comparison,
    whitespace: Whitespace,
) -> Result<Compared, Refused> {
    let unreadable = |why: String| Refused::Unreadable {
        what: format!("the changes of {}", branch.called()),
        why,
    };
    let kept = super::Kept::of(plane, &opened.base, branch);
    kept.whole()?;
    let (since, head, named) = resolve(plane, branch, opened, comparison)?;
    // The folder is held for a commit's head too: git reads the attributes from the working
    // tree whatever it compares. Not held, only the bytes and `info/attributes` decide then.
    let held = match head {
        At::WorkingTree => Some(hold(plane, &opened.base).map_err(unreadable)?),
        At::Commit(_) => hold(plane, &opened.base).ok(),
    };
    let Changed {
        mut repo,
        head_tree,
        changes: mut all,
        unread,
        refused: mut refused_all,
    } = changed_with(&opened.repo, since, head, held.as_ref(), true).map_err(unreadable)?;
    // What purlis keeps to itself is not among a comparison's files, at either end of a move.
    all.retain(|change| !kept_from_a_list(&kept, &change.path, change.from.as_deref()));
    refused_all.retain(|change| !kept_from_a_list(&kept, &change.path, None));
    let more = all.len().saturating_sub(MARKED) + refused_all.len().saturating_sub(MARKED);
    refused_all.truncate(MARKED);
    let listed = all.iter().take(MARKED);
    let attributes = HeldAttributes::read(
        &mut repo,
        held.as_ref(),
        listed
            .flat_map(|change| [Some(change.path.as_str()), change.from.as_deref()])
            .flatten(),
    );
    let since_tree = tree_of(&repo, since).map_err(unreadable)?;
    let head_tree = repo
        .find_tree(head_tree)
        .map_err(|e| unreadable(e.to_string()))?;
    let mut said = DiffAttribute::of(&attributes);
    let mut files = Vec::with_capacity(all.len().min(MARKED));
    for change in all.into_iter().take(MARKED) {
        let from = change.from.as_deref().unwrap_or(&change.path);
        let old = side_at(&repo, since_tree.as_ref(), from, COUNTED);
        let new = if unread.contains(&change.path) {
            (Side::Unreadable, Kind::File)
        } else {
            side_at(&repo, Some(&head_tree), &change.path, COUNTED)
        };
        let binary = said.binary(&old, from) || said.binary(&new, &change.path);
        let lines = match (old.0.bytes(), new.0.bytes()) {
            // Counted as `git diff --numstat` counts, a line diff of the two sides, even for a
            // type change, which is drawn whole ([`lines_between`]) but not counted so.
            (Some(a), Some(b)) if !binary => {
                let (_, added, removed) = line_diff_as(a, b, whitespace);
                Some(Lines { added, removed })
            }
            _ => None,
        };
        files.push(FileChange {
            path: change.path,
            mark: change.mark,
            from: change.from,
            lines,
            binary,
            uncommitted: change.uncommitted,
        });
    }
    Ok(Compared {
        sides: Sides {
            base: since.map(|id| id.to_string()),
            head: match head {
                At::Commit(id) => Head::Commit(id.to_string()),
                At::WorkingTree => Head::WorkingTree,
            },
        },
        base: named,
        files,
        refused: refused_all,
        more,
    })
}

/// [`compare_file`], in this process: only the bounded reader's child calls it.
pub(super) fn compare_file_here(
    plane: &Path,
    branch: Branch<'_>,
    sides: &Sides,
    path: &str,
    from: Option<&str>,
    whitespace: Whitespace,
) -> Result<FileDiff, Refused> {
    let Some(path) = plain(path) else {
        return Err(Refused::NotInPiece(path.to_string()));
    };
    let from = match from {
        Some(from) => Some(plain(from).ok_or_else(|| Refused::NotInPiece(from.to_string()))?),
        None => None,
    };
    file_diff_in(
        plane,
        branch,
        &open(plane, branch)?,
        sides,
        path,
        from,
        whitespace,
    )
}

/// [`compare_file_here`] of a branch already opened, for a path and its source already checked
/// to be plain.
fn file_diff_in(
    plane: &Path,
    branch: Branch<'_>,
    opened: &Opened,
    sides: &Sides,
    path: String,
    from: Option<String>,
    whitespace: Whitespace,
) -> Result<FileDiff, Refused> {
    // What purlis keeps to itself is compared in no place, under either name of a move.
    let kept = super::Kept::of(plane, &opened.base, branch);
    kept.refuse(Path::new(&path), &path)?;
    if let Some(from) = &from {
        kept.refuse(Path::new(from), from)?;
    }
    let what = path.clone();
    let unreadable = |why: String| Refused::Unreadable {
        what: what.clone(),
        why,
    };
    let commit = |hex: &str| -> Result<gix::ObjectId, Refused> {
        let full = hex.len() == 40 || hex.len() == 64;
        let id = (full && hex.bytes().all(|b| b.is_ascii_hexdigit()))
            .then(|| gix::ObjectId::from_hex(hex.as_bytes()).ok())
            .flatten()
            .ok_or_else(|| unreadable(format!("'{hex}' is not a commit")))?;
        opened
            .repo
            .find_commit(id)
            .map_err(|_| unreadable(format!("'{hex}' is not a commit in this repo")))?;
        Ok(id)
    };
    let since = sides.base.as_deref().map(commit).transpose()?;
    let since_tree = tree_of(&opened.repo, since).map_err(unreadable)?;
    let largest = super::LARGEST;
    let old_path = from.as_deref().unwrap_or(&path);
    let old = side_at(&opened.repo, since_tree.as_ref(), old_path, largest);
    // Held for a commit's head too, for the working tree's attributes ([`HeldAttributes`]).
    let held = hold(plane, &opened.base);
    let attributes = HeldAttributes::read(
        &mut opened.repo.clone(),
        held.as_ref().ok(),
        [old_path, path.as_str()],
    );
    let new = match &sides.head {
        Head::Commit(hex) => {
            let tree = tree_of(&opened.repo, Some(commit(hex)?)).map_err(unreadable)?;
            side_at(&opened.repo, tree.as_ref(), &path, largest)
        }
        Head::WorkingTree => {
            // **Only what a review can show** (ADR 0084 §2, as `files::open`): a file git
            // tracks, or one it does not ignore. An ignored `.env` is never read here.
            let shown = super::watch::matters_here(plane, branch, &[path.clone().into()])?;
            if !shown {
                return Err(Refused::NotOffered(path));
            }
            let held = held.as_ref().map_err(|why| unreadable(why.clone()))?;
            let index = opened
                .repo
                .index_or_empty()
                .map_err(|e| unreadable(e.to_string()))?;
            match pointer_in(&index, &path) {
                // A submodule's pointer is the commit its folder has checked out, as `git add`
                // would stage it ([`checked_out`]), else what the index holds while its folder
                // is there; the folder is never read as a file.
                Some(id) => match on_disk(held, &path, 0).0 {
                    Side::Missing => (Side::Missing, Kind::File),
                    _ => (
                        pointer(checked_out(&opened.repo, held, &path).unwrap_or(id)),
                        Kind::Submodule,
                    ),
                },
                None => {
                    let (side, kind) = on_disk(held, &path, largest);
                    let kind = Kind::of_entry(kind);
                    let side = match (side, kind) {
                        (Side::Bytes(bytes), Kind::File) => match ToGit::of(&attributes) {
                            Some(mut to_git) => Side::Bytes(to_git.clean(&path, bytes)),
                            None => Side::Bytes(bytes),
                        },
                        (side, _) => side,
                    };
                    (side, kind)
                }
            }
        }
    };
    let mut said = DiffAttribute::of(&attributes);
    let binary = said.binary(&old, old_path) || said.binary(&new, &path);
    let ((old, old_kind), (new, new_kind)) = (old, new);
    for side in [&old, &new] {
        match side {
            Side::TooLarge(bytes) => return Ok(FileDiff::TooLarge { bytes: *bytes }),
            // Never drawn as an empty file: a side that is there and cannot be read is said.
            Side::Unreadable => {
                return Err(unreadable("it is not a file purlis can read".into()));
            }
            Side::Missing | Side::Bytes(_) => {}
        }
    }
    if binary {
        return Ok(FileDiff::Binary);
    }
    let (a, b) = (
        old.bytes().unwrap_or_default(),
        new.bytes().unwrap_or_default(),
    );
    let (hunks, _, _) = lines_between(a, old_kind, b, new_kind, whitespace);
    Ok(FileDiff::Text {
        base: String::from_utf8_lossy(a).into_owned(),
        head: String::from_utf8_lossy(b).into_owned(),
        hunks,
    })
}

/// The base and the head `comparison` names in the opened branch, and the base's name.
fn resolve(
    plane: &Path,
    branch: Branch<'_>,
    opened: &Opened,
    comparison: &Comparison,
) -> Result<(Option<gix::ObjectId>, At, Option<String>), Refused> {
    let repo = &opened.repo;
    let what = || format!("the changes of {}", branch.called());
    let refused = |why: String| Refused::Unreadable { what: what(), why };
    let head_commit = || repo.head_commit().ok().map(|commit| commit.id);
    // The recorded base, else the repo's own folder's upstream for the branch it is on (#1130).
    let based = |name: Option<&str>, tip: gix::ObjectId| {
        name.and_then(|name| opened.based(name, tip))
            .map(|b| (b.fork, b.named))
    };
    let current = || {
        repo.head_name()
            .ok()
            .flatten()
            .map(|name| name.shorten().to_string())
    };
    match comparison {
        Comparison::Branch => {
            let tip = head_commit().ok_or_else(|| refused("it has no commit yet".into()))?;
            let (fork, named) = based(current().as_deref(), tip)
                .or_else(|| default_base(plane, branch.repo, repo, tip))
                .ok_or_else(|| no_base(branch.called()))?;
            Ok((Some(fork), At::Commit(tip), Some(named)))
        }
        Comparison::BranchAndUncommitted => {
            let tip = head_commit();
            match tip.and_then(|tip| based(current().as_deref(), tip)) {
                Some((fork, named)) => Ok((Some(fork), At::WorkingTree, Some(named))),
                None => Ok((tip, At::WorkingTree, None)),
            }
        }
        Comparison::NamedBranch { branch: wanted } => {
            name::branch_name_ok(wanted).map_err(|e| refused(e.to_string()))?;
            let tip = repo
                .find_reference(name::as_ref(wanted).as_str())
                .ok()
                .and_then(|mut found| found.peel_to_commit().ok())
                .map(|commit| commit.id)
                .ok_or_else(|| refused(format!("it has no branch called '{wanted}'")))?;
            let (fork, named) = based(Some(wanted), tip)
                .or_else(|| default_base(plane, branch.repo, repo, tip))
                .ok_or_else(|| no_base(wanted))?;
            Ok((Some(fork), At::Commit(tip), Some(named)))
        }
        Comparison::Refs { from, to, exact } => {
            let commit = |spec: &str| -> Result<gix::ObjectId, Refused> {
                repo.rev_parse_single(spec)
                    .ok()
                    .and_then(|id| id.object().ok()?.peel_to_commit().ok())
                    .map(|commit| commit.id)
                    .ok_or_else(|| refused(format!("'{spec}' does not name a commit here")))
            };
            let (a, b) = (commit(from)?, commit(to)?);
            let since = if *exact {
                a
            } else {
                let no_history = || refused(format!("'{from}' and '{to}' share no history"));
                repo.merge_base(a, b)
                    .map_err(|_| no_history())?
                    .ok_or_else(no_history)?
                    .detach()
            };
            Ok((Some(since), At::Commit(b), Some(from.clone())))
        }
        Comparison::Uncommitted => Ok((head_commit(), At::WorkingTree, None)),
    }
}

fn no_base(called: &str) -> Refused {
    Refused::Read(format!(
        "purlis does not know which branch {called} was cut from, so it has nothing to compare \
         it with. Compare it with a branch you name instead."
    ))
}

/// Where `tip` left the repo's default branch, and that branch's name: the one the project's
/// inventory records for the repo, else the one the clone's `origin/HEAD` names.
fn default_base(
    plane: &Path,
    repo_name: &str,
    repo: &gix::Repository,
    tip: gix::ObjectId,
) -> Option<(gix::ObjectId, String)> {
    let listed = crate::inventory::load(plane, "")
        .ok()
        .map(|doc| crate::inventory::listed(&doc))
        .unwrap_or_default();
    let recorded = crate::inventory::find(&listed, repo_name)
        .and_then(|record| record.get("default_branch")?.as_str())
        .filter(|b| name::branch_name_ok(b).is_ok())
        .map(str::to_owned);
    let candidates: Vec<String> = match recorded {
        Some(b) => vec![
            format!("refs/heads/{b}"),
            format!("refs/remotes/origin/{b}"),
        ],
        None => vec!["refs/remotes/origin/HEAD".to_string()],
    };
    for candidate in candidates {
        let Ok(mut found) = repo.find_reference(candidate.as_str()) else {
            continue;
        };
        let Ok(at) = found.peel_to_commit().map(|commit| commit.id) else {
            continue;
        };
        let named = found.name().shorten().to_string();
        let named = if candidate.ends_with("/HEAD") {
            // What `origin/HEAD` points at, by name, when it is a symbolic ref.
            repo.find_reference(candidate.as_str())
                .ok()
                .and_then(|r| match r.target() {
                    gix::refs::TargetRef::Symbolic(to) => Some(to.shorten().to_string()),
                    gix::refs::TargetRef::Object(_) => None,
                })
                .unwrap_or(named)
        } else {
            named
        };
        // A candidate that shares no history with the branch is not its base: the next one may.
        let Ok(Some(fork)) = repo.merge_base(at, tip) else {
            continue;
        };
        return Some((fork.detach(), named));
    }
    None
}

/// What changed between `since` and `head`: the engine's file list without its line counts,
/// which is also the explorer's markers ([`super::status`]), and where to read each side.
pub(super) struct Changed {
    /// The repo both sides are read from. For a working-tree head, the branch's repo with the
    /// working tree's changed files written into memory, never to disk.
    pub(super) repo: gix::Repository,
    /// The head's tree: a commit's, or the working tree's as `git add -A` would stage it.
    pub(super) head_tree: gix::ObjectId,
    /// Sorted by path.
    pub(super) changes: Vec<Change>,
    /// Paths whose working-tree side was not read — past [`COUNTED`], past [`READ_MOST`], or
    /// not a file that reads — and so have no line counts.
    pub(super) unread: BTreeSet<String>,
    /// The paths changed that are not plain ([`plain`]), sorted by path: named, never read.
    pub(super) refused: Vec<RefusedChange>,
}

/// The most changed working-tree files read for one comparison: past it, a file is marked by
/// whether it is there and not read, so a branch that generated a hundred thousand files git
/// does not ignore costs the walk git's status makes, not a read of each.
pub const READ_MOST: usize = 20_000;

/// The most bytes of working-tree content one comparison reads and keeps: 128 MiB, an eighth
/// of the reader's memory cap. Every file read is kept (in memory) until the comparison ends,
/// so past this a file is marked by its size, as there and changed, and not read: many large
/// files a branch does not ignore cost the read nothing more, and never its markers.
pub const READ_BUDGET: u64 = 128 * 1024 * 1024;

/// What changed between `since` and `head` (see [`Changed`]), `held` being the branch's folder
/// for a working-tree head.
///
/// **Every kind is one tree against another**, with renames found as `git diff -M` finds them
/// (gitoxide's rewrite tracking, git's 50% similarity and 1,000-file limit). A working-tree
/// head is first made a tree, as `git add -A` would stage it and in memory only: HEAD's tree,
/// with each path git's status reports as changed, untracked or gone read from the folder held
/// open (following no link) and put in or taken out. So a file the branch committed under one
/// name and then moved again in the working tree is one rename, as git shows it.
///
/// **Files and links only**: the explorer's markers leave a submodule out. A comparison lists
/// its pointers too ([`changed_with`]).
pub(super) fn changed(
    repo: &gix::Repository,
    since: Option<gix::ObjectId>,
    head: At,
    held: Option<&super::Held>,
) -> Result<Changed, String> {
    changed_with(repo, since, head, held, false)
}

/// [`changed`], with each submodule whose pointer moved, came or went listed too when
/// `submodules`, as `git diff` lists one.
fn changed_with(
    repo: &gix::Repository,
    since: Option<gix::ObjectId>,
    head: At,
    held: Option<&super::Held>,
    submodules: bool,
) -> Result<Changed, String> {
    let (repo, head_tree, loose, unread, not_staged) = match head {
        At::Commit(id) => {
            let tree = tree_of(repo, Some(id))?
                .map(|tree| tree.id)
                .ok_or("no tree")?;
            (
                repo.clone(),
                tree,
                BTreeSet::new(),
                BTreeSet::new(),
                BTreeMap::new(),
            )
        }
        At::WorkingTree => working_tree(repo, held.ok_or("the folder is not held")?)?,
    };
    let (changes, mut refused) = between_trees(&repo, since, head_tree, &loose, submodules)?;
    // A path the working tree changed and the tree could not stage is the working tree's
    // change too: one row, said to be not committed.
    for (path, mark) in not_staged {
        match refused.iter_mut().find(|one| one.path == path) {
            Some(one) => one.uncommitted = true,
            None => refused.push(RefusedChange {
                why: refused_for(&path),
                path,
                mark,
                uncommitted: true,
            }),
        }
    }
    refused.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(Changed {
        repo,
        head_tree,
        changes,
        unread,
        refused,
    })
}

/// [`working_tree`]'s answer, in its order.
type WorkingTree = (
    gix::Repository,
    gix::ObjectId,
    BTreeSet<String>,
    BTreeSet<String>,
    BTreeMap<String, Mark>,
);

/// The working tree as a tree, in a copy of `repo` that writes objects to memory: the copy,
/// the tree, every path not committed, the paths not read, and each path not committed that is
/// not plain ([`plain`]), with what the working tree did to it: never read, never staged.
fn working_tree(repo: &gix::Repository, held: &super::Held) -> Result<WorkingTree, String> {
    let mut memory = repo.clone().with_object_memory();
    let mut loose: BTreeSet<String> = BTreeSet::new();
    let mut untracked: BTreeSet<String> = BTreeSet::new();
    let mut gone: BTreeSet<String> = BTreeSet::new();
    uncommitted(&memory, &mut loose, &mut untracked, &mut gone)?;
    let start = match memory.head_commit() {
        Ok(commit) => commit.tree_id().map_err(|e| e.to_string())?.detach(),
        Err(_) => gix::ObjectId::empty_tree(memory.object_hash()),
    };
    let index = memory.index_or_empty().map_err(|e| e.to_string())?;
    // A submodule whose folder checked out another commit than the index points at is not
    // committed either, as `git add -A` would stage the commit it checked out (#704).
    let mut moved: BTreeMap<String, gix::ObjectId> = BTreeMap::new();
    for entry in index.entries() {
        if entry.mode != gix::index::entry::Mode::COMMIT || entry.stage_raw() != 0 {
            continue;
        }
        let Some(path) = plain(&entry.path(&index).to_string()) else {
            continue;
        };
        if let Some(at) = checked_out(&memory, held, &path).filter(|at| *at != entry.id) {
            loose.insert(path.clone());
            moved.insert(path, at);
        }
    }
    let attributes = HeldAttributes::read(
        &mut memory,
        Some(held),
        loose.iter().chain(untracked.iter()).map(String::as_str),
    );
    let mut to_git = ToGit::of(&attributes);
    let mut editor = memory.edit_tree(start).map_err(|e| e.to_string())?;
    let mut unread = BTreeSet::new();
    let mut read = 0usize;
    let mut budget = READ_BUDGET;
    let mut not_staged: BTreeMap<String, Mark> = BTreeMap::new();
    for path in &gone {
        match plain(path) {
            Some(plain) => editor.remove(plain.as_str()).map_err(|e| e.to_string())?,
            None => {
                not_staged.insert(path.clone(), Mark::Deleted);
                continue;
            }
        };
    }
    for path in loose.iter().chain(untracked.iter()) {
        let Some(plain) = plain(path) else {
            if !gone.contains(path) {
                let mark = if untracked.contains(path) {
                    Mark::Added
                } else {
                    Mark::Changed
                };
                not_staged.insert(path.clone(), mark);
            }
            continue;
        };
        if gone.contains(path) {
            continue;
        }
        // A submodule's pointer is staged as the index holds it while its folder is there, as
        // `git add -A` keeps it: the folder is never read as a file.
        if let Some(id) = pointer_in(&index, &plain) {
            let id = moved.get(&plain).copied().unwrap_or(id);
            match on_disk(held, &plain, 0).0 {
                Side::Missing => editor.remove(plain.as_str()),
                _ => editor.upsert(plain.as_str(), gix::object::tree::EntryKind::Commit, id),
            }
            .map_err(|e| e.to_string())?;
            continue;
        }
        read += 1;
        let (side, kind) = if read > READ_MOST {
            (Side::Unreadable, gix::object::tree::EntryKind::Blob)
        } else {
            on_disk(held, &plain, COUNTED.min(budget))
        };
        if let Side::Bytes(bytes) = &side {
            budget = budget.saturating_sub(bytes.len() as u64);
        }
        let bytes = match side {
            Side::Missing => {
                editor.remove(plain.as_str()).map_err(|e| e.to_string())?;
                continue;
            }
            // Stored as `git add` would store it: its line ends converted, a link's target not.
            Side::Bytes(bytes) => match to_git.as_mut() {
                Some(to_git) if Kind::of_entry(kind) == Kind::File => to_git.clean(&plain, bytes),
                _ => bytes,
            },
            Side::TooLarge(_) | Side::Unreadable => {
                // There, and not read: a blob of its own that matches nothing, so it is marked
                // and never taken for a rename.
                unread.insert(plain.clone());
                format!("\0charter: {plain} was not read\0").into_bytes()
            }
        };
        let id = memory
            .write_blob(bytes)
            .map_err(|e| e.to_string())?
            .detach();
        editor
            .upsert(plain.as_str(), kind, id)
            .map_err(|e| e.to_string())?;
    }
    let tree = editor.write().map_err(|e| e.to_string())?.detach();
    drop(editor);
    drop(to_git);
    loose.extend(untracked);
    loose.extend(gone);
    Ok((memory, tree, loose, unread, not_staged))
}

/// The difference of `since`'s tree and the tree `head`, file by file, sorted, renames found.
/// A path in `loose` is one not committed yet. A submodule's pointer is listed only when
/// `submodules`: a pointer turned into a file is then a change, else the file is added.
///
/// **A path that is not plain is named as refused, never dropped** (#704): a `.git` component
/// in any case, or a spelling [`plain`] refuses, comes back in the second list with its mark,
/// and nothing at it is read. A rename with one such end is the plain end's own change — added
/// where it came to, deleted where it left — and the other end is refused.
fn between_trees(
    repo: &gix::Repository,
    since: Option<gix::ObjectId>,
    head: gix::ObjectId,
    loose: &BTreeSet<String>,
    submodules: bool,
) -> Result<(Vec<Change>, Vec<RefusedChange>), String> {
    // A blob or a link, and a submodule's pointer when one is asked for: never a tree.
    let is_file = |mode: gix::object::tree::EntryMode| {
        mode.is_blob_or_symlink() || (submodules && mode.is_commit())
    };
    let old = tree_of(repo, since)?;
    let new = repo.find_tree(head).map_err(|e| e.to_string())?;
    let options = gix::diff::Options::default().with_rewrites(Some(gix::diff::Rewrites::default()));
    let found = repo
        .diff_tree_to_tree(old.as_ref(), &new, options)
        .map_err(|e| e.to_string())?;
    let mut out: BTreeMap<String, Change> = BTreeMap::new();
    let mut refused: BTreeMap<String, RefusedChange> = BTreeMap::new();
    let mut put = |path: String, mark: Mark, from: Option<String>| {
        let uncommitted =
            loose.contains(&path) || from.as_ref().is_some_and(|from| loose.contains(from));
        let mut refuse = |path: String, mark: Mark| {
            refused.insert(
                path.clone(),
                RefusedChange {
                    why: refused_for(&path),
                    path,
                    mark,
                    uncommitted,
                },
            );
        };
        let (path, mark, from) = match (plain(&path), from) {
            (Some(path), None) => (path, mark, None),
            (Some(path), Some(from)) => match plain(&from) {
                Some(from) => (path, mark, Some(from)),
                None => {
                    refuse(from, Mark::Deleted);
                    (path, Mark::Added, None)
                }
            },
            (None, Some(from)) => {
                refuse(path, Mark::Added);
                match plain(&from) {
                    Some(from) => (from, Mark::Deleted, None),
                    None => return refuse(from, Mark::Deleted),
                }
            }
            (None, None) => return refuse(path, mark),
        };
        out.insert(
            path.clone(),
            Change {
                path,
                mark,
                from,
                uncommitted,
            },
        );
    };
    use gix::object::tree::diff::ChangeDetached;
    for change in found {
        match change {
            ChangeDetached::Addition {
                location,
                entry_mode,
                ..
            } if is_file(entry_mode) => put(location.to_string(), Mark::Added, None),
            ChangeDetached::Deletion {
                location,
                entry_mode,
                ..
            } if is_file(entry_mode) => put(location.to_string(), Mark::Deleted, None),
            ChangeDetached::Modification {
                location,
                previous_entry_mode,
                entry_mode,
                ..
            } => match (is_file(previous_entry_mode), is_file(entry_mode)) {
                (true, true) => put(location.to_string(), Mark::Changed, None),
                (false, true) => put(location.to_string(), Mark::Added, None),
                (true, false) => put(location.to_string(), Mark::Deleted, None),
                (false, false) => {}
            },
            ChangeDetached::Rewrite {
                source_location,
                location,
                entry_mode,
                copy,
                ..
            } if is_file(entry_mode) => {
                if copy {
                    put(location.to_string(), Mark::Added, None);
                } else {
                    put(
                        location.to_string(),
                        Mark::Renamed,
                        Some(source_location.to_string()),
                    );
                }
            }
            _ => {}
        }
    }
    Ok((out.into_values().collect(), refused.into_values().collect()))
}

/// What is not committed, as git's status reads it: each path HEAD → index → working tree
/// changed into `loose`, each file git does not track and does not ignore into `untracked`, and
/// each tracked file gone from the working tree into `gone`. Renames are not looked for here:
/// they are found once, between the two trees ([`between_trees`]).
///
/// The untracked files come from a walk of their own ([`untracked_files`]), run beside the
/// status on a second thread as gix's status runs its own walk, so a folder too deep to read
/// costs only itself and not the whole read (#1130).
fn uncommitted(
    repo: &gix::Repository,
    loose: &mut BTreeSet<String>,
    untracked: &mut BTreeSet<String>,
    gone: &mut BTreeSet<String>,
) -> Result<(), String> {
    let walker = repo.clone();
    std::thread::scope(|scope| {
        let walk = scope.spawn(move || untracked_files(&walker));
        let tracked = tracked_changes(repo, loose, gone);
        let walked = walk
            .join()
            .unwrap_or_else(|_| Err("the walk for untracked files stopped".to_string()));
        tracked?;
        untracked.extend(walked?);
        Ok(())
    })
}

/// [`uncommitted`]'s tracked half: git's status with no walk for untracked files.
fn tracked_changes(
    repo: &gix::Repository,
    loose: &mut BTreeSet<String>,
    gone: &mut BTreeSet<String>,
) -> Result<(), String> {
    use gix::status::{Item, index_worktree};
    let items = repo
        .status(gix::progress::Discard)
        .map_err(|e| e.to_string())?
        .untracked_files(gix::status::UntrackedFiles::None)
        .index_worktree_submodules(None)
        .index_worktree_options_mut(|options| options.thread_limit = Some(super::status::threads()))
        .tree_index_track_renames(gix::status::tree_index::TrackRenames::Disabled)
        .into_iter(None)
        .map_err(|e| e.to_string())?;
    for item in items {
        let item = item.map_err(|e| e.to_string())?;
        match item {
            Item::TreeIndex(change) => {
                use gix::diff::index::ChangeRef;
                if let ChangeRef::Rewrite {
                    source_location, ..
                } = &change
                {
                    loose.insert(source_location.to_string());
                }
                loose.insert(change.location().to_string());
            }
            Item::IndexWorktree(index_worktree::Item::Modification {
                rela_path, status, ..
            }) => {
                use gix::status::plumbing::index_as_worktree::{Change as Worktree, EntryStatus};
                match status {
                    EntryStatus::Change(Worktree::Removed) => {
                        gone.insert(rela_path.to_string());
                    }
                    EntryStatus::Change(Worktree::SubmoduleModification(_)) => {}
                    EntryStatus::NeedsUpdate(_) => {}
                    _ => {
                        loose.insert(rela_path.to_string());
                    }
                }
            }
            // No walk was asked for: nothing comes from one.
            Item::IndexWorktree(index_worktree::Item::DirectoryContents { .. })
            | Item::IndexWorktree(index_worktree::Item::Rewrite { .. }) => {}
        }
    }
    Ok(())
}

/// The longest path, in bytes, a call into the file system takes here. Past it an open fails
/// (`ENAMETOOLONG`), and gix's walk reads each folder by its whole path from the working
/// tree's root, so one such folder ends the walk. Windows long paths are taken by the standard
/// library, so there is no limit to keep below there.
#[cfg(target_os = "linux")]
const PATH_MOST: usize = 4096;
#[cfg(all(unix, not(target_os = "linux")))]
const PATH_MOST: usize = 1024;
#[cfg(not(unix))]
const PATH_MOST: usize = usize::MAX;

/// The longest name of one entry in a folder: room for it is kept when a folder is entered.
const NAME_MOST: usize = 255;

/// Whether a folder at `rela` (from the working tree's root, `root_len` bytes long) is too
/// deep to walk: its path, a separator and the longest name in it, with the terminating zero,
/// would not fit in [`PATH_MOST`]. So every folder the walk enters, and every entry it looks at
/// in one, can be named in full.
fn too_deep(root_len: usize, rela: usize, most: usize) -> bool {
    root_len
        .saturating_add(1)
        .saturating_add(rela)
        .saturating_add(1 + NAME_MOST + 1)
        > most
}

/// Each file git does not track and does not ignore, as `git status` lists them, and each
/// untracked folder too deep to walk ([`too_deep`]) or that does not open ([`opens`]) as one
/// entry of its own: a mark that says "not read past here", which the working tree takes as a
/// file it could not read. A tracked
/// folder that deep is walked as before: holding it back would hide the untracked files in it,
/// so a folder there too long to name still fails the read rather than drop a change.
fn untracked_files(repo: &gix::Repository) -> Result<BTreeSet<String>, String> {
    untracked_files_entering(repo, &mut |_| {})
}

/// How many times the walk for untracked files is begun again when a folder it was let into
/// has gone, or no longer opens, by the time it failed ([`untracked_files_entering`]).
const WALKS: usize = 3;

/// [`untracked_files`], with `entering` told each folder the walk is let into, once [`opens`]
/// said it opens: the seam a test removes a folder through between that check and the walk's
/// own read.
///
/// **A folder that goes in that moment ends the walk** (#1130): gix reads it, fails, and the
/// whole walk fails with it. So when the walk fails and a folder it was let into no longer
/// opens, it is begun again — up to [`WALKS`] times — and the next walk does not find it or
/// holds it back as one entry. A walk that fails while every folder it was let into still
/// opens fails as before: walking again would only fail again.
fn untracked_files_entering(
    repo: &gix::Repository,
    entering: &mut dyn FnMut(&Path),
) -> Result<BTreeSet<String>, String> {
    let mut walks = 1;
    loop {
        match untracked_walk(repo, entering) {
            Ok(found) => return Ok(found),
            Err((why, entered)) => {
                if walks == WALKS || entered.iter().all(|folder| opens(folder)) {
                    return Err(why);
                }
                walks += 1;
            }
        }
    }
}

/// One walk for [`untracked_files_entering`]: the files, or why it failed with the folders it
/// was let into.
fn untracked_walk(
    repo: &gix::Repository,
    entering: &mut dyn FnMut(&Path),
) -> Result<BTreeSet<String>, (String, Vec<PathBuf>)> {
    use gix::dir::entry::{Kind, Status};
    use gix::dir::walk::{Action, Delegate, EmissionMode, ForDeletionMode};
    use gix::dir::{EntryRef, entry};

    struct Untracked<'a> {
        root: &'a Path,
        root_len: usize,
        found: BTreeSet<String>,
        /// The untracked folders not walked into, each marked in its own place.
        held_back: BTreeSet<String>,
        /// The untracked folders the walk was let into.
        entered: Vec<PathBuf>,
        entering: &'a mut dyn FnMut(&Path),
    }
    impl Delegate for Untracked<'_> {
        fn emit(&mut self, entry: EntryRef<'_>, _: Option<entry::Status>) -> Action {
            if entry.status == Status::Untracked {
                let path = entry.rela_path.to_string();
                if matches!(entry.disk_kind, Some(Kind::File | Kind::Symlink))
                    || self.held_back.contains(&path)
                {
                    self.found.insert(path);
                }
            }
            Action::Continue(())
        }

        fn can_recurse(
            &mut self,
            entry: EntryRef<'_>,
            for_deletion: Option<ForDeletionMode>,
            worktree_root_is_repository: bool,
        ) -> bool {
            // Only a folder git does not track is held back: it is marked in its own place.
            // A tracked folder may hold untracked files a mark could not stand for, so it is
            // walked as before and a failure there still fails the read, never hides a file.
            let untracked = entry.status == Status::Untracked;
            if untracked && too_deep(self.root_len, entry.rela_path.len(), PATH_MOST) {
                self.held_back.insert(entry.rela_path.to_string());
                return false;
            }
            let recurse = entry.status.can_recurse(
                entry.disk_kind,
                entry.pathspec_match,
                for_deletion,
                worktree_root_is_repository,
            );
            // An untracked folder the walk could not read — no permission, gone since it was
            // listed, or a name this system cannot spell as a path — would end the walk, and the
            // status with it: it is held back the same way.
            if recurse && untracked {
                let folder = gix::path::from_bstr(&*entry.rela_path)
                    .ok()
                    .map(|rela| self.root.join(rela))
                    .filter(|folder| opens(folder));
                let Some(folder) = folder else {
                    self.held_back.insert(entry.rela_path.to_string());
                    return false;
                };
                (self.entering)(&folder);
                self.entered.push(folder);
            }
            recurse
        }
    }

    let Some(root) = repo.workdir() else {
        return Ok(BTreeSet::new());
    };
    let failed = |e: &dyn std::fmt::Display| (e.to_string(), Vec::new());
    let index = repo.index_or_empty().map_err(|e| failed(&e))?;
    let options = repo
        .dirwalk_options()
        .map_err(|e| failed(&e))?
        .emit_untracked(EmissionMode::Matching);
    let mut delegate = Untracked {
        root,
        root_len: root.as_os_str().len(),
        found: BTreeSet::new(),
        held_back: BTreeSet::new(),
        entered: Vec::new(),
        entering,
    };
    let walked = repo.dirwalk(
        &index,
        None::<&str>,
        &std::sync::atomic::AtomicBool::new(false),
        options,
        &mut delegate,
    );
    match walked {
        Ok(_) => Ok(delegate.found),
        Err(e) => Err((e.to_string(), delegate.entered)),
    }
}

/// Whether `folder` opens as a folder, following no link: what the walk is about to read. The
/// open is closed at once; a folder that goes between this and the walk's read ends that walk,
/// which is then begun again ([`untracked_files_entering`], #1130).
#[cfg(unix)]
fn opens(folder: &Path) -> bool {
    use rustix::fs::{Mode, OFlags};
    rustix::fs::open(
        folder,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .is_ok()
}

#[cfg(not(unix))]
fn opens(folder: &Path) -> bool {
    std::fs::symlink_metadata(folder).is_ok_and(|meta| meta.is_dir())
        && std::fs::read_dir(folder).is_ok()
}

/// The tree of `commit`, or `None` for none.
fn tree_of<'r>(
    repo: &'r gix::Repository,
    commit: Option<gix::ObjectId>,
) -> Result<Option<gix::Tree<'r>>, String> {
    commit
        .map(|id| {
            repo.find_commit(id)
                .and_then(|commit| commit.tree())
                .map_err(|e| e.to_string())
        })
        .transpose()
}

/// One side of one file.
enum Side {
    /// Not on this side.
    Missing,
    /// Its bytes.
    Bytes(Vec<u8>),
    /// Past the size it is read to, by its size.
    TooLarge(u64),
    /// Not a file that can be read as one (a FIFO, a socket, an unreadable one).
    Unreadable,
}

impl Side {
    fn bytes(&self) -> Option<&[u8]> {
        match self {
            Side::Missing => Some(&[]),
            Side::Bytes(bytes) => Some(bytes),
            Side::TooLarge(_) | Side::Unreadable => None,
        }
    }

    fn is_binary(&self) -> bool {
        match self {
            Side::Bytes(bytes) => bytes[..bytes.len().min(SNIFF)].contains(&0),
            _ => false,
        }
    }
}

/// What kind of entry a side is: a type change between two kinds is drawn as git draws it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// A blob, executable or not.
    File,
    /// A link: its target, as git stores it.
    Link,
    /// A submodule's pointer: git's `Subproject commit` line.
    Submodule,
}

impl Kind {
    fn of_mode(mode: gix::object::tree::EntryMode) -> Self {
        if mode.is_link() {
            Kind::Link
        } else if mode.is_commit() {
            Kind::Submodule
        } else {
            Kind::File
        }
    }

    fn of_entry(kind: gix::object::tree::EntryKind) -> Self {
        Kind::of_mode(kind.into())
    }
}

/// `path` in `tree`: a blob's bytes, a link's target as git stores it, or a submodule's pointer
/// as git draws it, and its kind.
fn side_at(
    repo: &gix::Repository,
    tree: Option<&gix::Tree<'_>>,
    path: &str,
    most: u64,
) -> (Side, Kind) {
    let Some(entry) = tree.and_then(|tree| tree.lookup_entry_by_path(path).ok().flatten()) else {
        return (Side::Missing, Kind::File);
    };
    let kind = Kind::of_mode(entry.mode());
    let id = entry.object_id();
    if kind == Kind::Submodule {
        // The commit it names is in another repo, and never read: only its id is drawn.
        return (pointer(id), kind);
    }
    if !entry.mode().is_blob_or_symlink() {
        return (Side::Missing, Kind::File);
    }
    match repo.find_header(id) {
        Ok(header) if header.size() > most => return (Side::TooLarge(header.size()), kind),
        Ok(_) => {}
        Err(_) => return (Side::Unreadable, kind),
    }
    match repo.find_blob(id) {
        Ok(blob) => (Side::Bytes(blob.data.clone()), kind),
        Err(_) => (Side::Unreadable, kind),
    }
}

/// A submodule's pointer to `id`, as git draws it.
fn pointer(id: gix::ObjectId) -> Side {
    Side::Bytes(format!("Subproject commit {id}\n").into_bytes())
}

/// The commit `index` points `path` at when it holds a submodule's pointer there.
fn pointer_in(index: &gix::index::State, path: &str) -> Option<gix::ObjectId> {
    use gix::bstr::ByteSlice as _;
    index
        .entry_by_path(path.as_bytes().as_bstr())
        .filter(|entry| entry.mode == gix::index::entry::Mode::COMMIT)
        .map(|entry| entry.id)
}

/// The commit the submodule whose folder is `path` has checked out, read under a confinement rule
/// (#704); `None` when it cannot be read so, and the index's pointer stands then.
///
/// **Its git folder is found without following anything the submodule's folder says.** A chat
/// writes that folder, so it can write a `.git` file whose `gitdir:` line points anywhere, and
/// a config that names programs. So the `gitdir:` line is never read, nor the submodule's
/// config, and no git starts. The git folder is the superproject's `modules/<name>` (its own
/// git folder's, then the common one's), `<name>` read from the `.gitmodules` the superproject's
/// index holds, else its last commit's, never the working tree's copy, and refused when it
/// could walk out of `modules/`; else the submodule's own `.git` folder, reached through the
/// held folder following no link. Its `HEAD`, then the ref that names, loose or in
/// `packed-refs`, are read as plain files following no link. A chat that writes a `.git`
/// folder in the submodule can make the pointer drawn name another commit: that only says what
/// the chat could also do by moving the submodule's HEAD.
fn checked_out(repo: &gix::Repository, held: &super::Held, path: &str) -> Option<gix::ObjectId> {
    if let Some(name) = submodule_name(repo, path) {
        let mut tops = vec![repo.git_dir()];
        if repo.common_dir() != repo.git_dir() {
            tops.push(repo.common_dir());
        }
        for top in tops {
            let at = top.join("modules").join(&name);
            let read = |relative: &str| {
                let file = crate::contain::open_no_link(top, &at.join(relative)).ok()?;
                plain_file(file, GIT_FILE_MOST)
            };
            if let Some(id) = head_in(&read) {
                return Some(id);
            }
        }
    }
    let read = |relative: &str| {
        let file = super::open_inside(held, &Path::new(path).join(".git").join(relative)).ok()?;
        plain_file(file, GIT_FILE_MOST)
    };
    head_in(&read)
}

/// The most bytes of one file of a submodule's git folder [`checked_out`] reads: `packed-refs`
/// is the largest of them.
const GIT_FILE_MOST: u64 = 64 * 1024 * 1024;

/// An opened file's bytes when it is a plain file of at most `most` bytes.
fn plain_file(mut file: std::fs::File, most: u64) -> Option<Vec<u8>> {
    let meta = file.metadata().ok()?;
    if !meta.is_file() || meta.len() > most {
        return None;
    }
    let mut bytes = Vec::with_capacity(meta.len() as usize);
    file.by_ref().take(most + 1).read_to_end(&mut bytes).ok()?;
    (bytes.len() as u64 <= most).then_some(bytes)
}

/// The name `.gitmodules` gives the submodule at `path`: from the superproject's index, else its
/// last commit, never the working tree's copy. `None` when neither names one, or the name could
/// walk out of the folder it names in `modules/` (empty, absolute, or a `.`, `..` or empty
/// component, either separator).
fn submodule_name(repo: &gix::Repository, path: &str) -> Option<String> {
    use gix::bstr::ByteSlice as _;
    let staged = repo.index_or_empty().ok().and_then(|index| {
        let entry = index.entry_by_path(".gitmodules".as_bytes().as_bstr())?;
        (entry.mode == gix::index::entry::Mode::FILE).then_some(entry.id)
    });
    let committed = || {
        let tree = repo.head_commit().ok()?.tree().ok()?;
        let entry = tree.lookup_entry_by_path(".gitmodules").ok()??;
        entry.mode().is_blob().then(|| entry.object_id())
    };
    let id = staged.or_else(committed)?;
    let blob = repo.find_blob(id).ok()?;
    let modules = gix::config::File::from_bytes_no_includes(
        &blob.data,
        gix::config::file::Metadata::from(gix::config::Source::Local),
        Default::default(),
    )
    .ok()?;
    let name = modules
        .sections_by_name("submodule")?
        .filter(|section| {
            section
                .value("path")
                .is_some_and(|at| at.as_slice() == path.as_bytes())
        })
        .find_map(|section| section.header().subsection_name().map(|n| n.to_string()))?;
    let walks_out = name.is_empty()
        || name
            .split(['/', '\\'])
            .any(|step| step.is_empty() || step == "." || step == "..")
        || name.contains(':');
    (!walks_out).then_some(name)
}

/// The commit a git folder's `HEAD` names, `read` reading one of its files by its path in it:
/// a commit's id, or a ref under `refs/` that names one, loose or in `packed-refs`, followed
/// through at most a few symbolic refs.
fn head_in(read: &dyn Fn(&str) -> Option<Vec<u8>>) -> Option<gix::ObjectId> {
    let mut name = "HEAD".to_string();
    for _ in 0..5 {
        if name != "HEAD" && !ref_inside(&name) {
            return None;
        }
        let Some(bytes) = read(&name) else {
            return packed(read, &name);
        };
        let text = std::str::from_utf8(&bytes).ok()?.trim_end();
        match text.strip_prefix("ref:") {
            Some(to) => name = to.trim().to_string(),
            None => return commit_id(text),
        }
    }
    None
}

/// Whether `name` is a ref under `refs/` that stays in the git folder: plain components only.
fn ref_inside(name: &str) -> bool {
    name.starts_with("refs/")
        && name
            .split('/')
            .all(|step| !step.is_empty() && step != "." && step != ".." && !step.contains('\\'))
}

/// The commit `packed-refs` gives `name`.
fn packed(read: &dyn Fn(&str) -> Option<Vec<u8>>, name: &str) -> Option<gix::ObjectId> {
    let bytes = read("packed-refs")?;
    let text = std::str::from_utf8(&bytes).ok()?;
    text.lines()
        .filter(|line| !line.starts_with('#') && !line.starts_with('^'))
        .find_map(|line| {
            let (id, at) = line.split_once(' ')?;
            (at == name).then(|| commit_id(id)).flatten()
        })
}

/// A full commit id in hex.
fn commit_id(hex: &str) -> Option<gix::ObjectId> {
    let full = hex.len() == 40 || hex.len() == 64;
    (full && hex.bytes().all(|b| b.is_ascii_hexdigit()))
        .then(|| gix::ObjectId::from_hex(hex.as_bytes()).ok())
        .flatten()
}

/// git's line diff of two sides of `old_kind` and `new_kind` ([`line_diff`]'s answer). A type
/// change is drawn as git's patch draws it: the whole old side out in one hunk and the whole new
/// side in in another, never a line diff of the two. Its counts are the whole sides; a
/// comparison's listing counts as `--numstat` does instead, a plain line diff.
fn lines_between(
    old: &[u8],
    old_kind: Kind,
    new: &[u8],
    new_kind: Kind,
    whitespace: Whitespace,
) -> (Vec<Hunk>, u64, u64) {
    if old_kind == new_kind || old.is_empty() && new.is_empty() {
        return line_diff_as(old, new, whitespace);
    }
    let (mut hunks, _, removed) = line_diff_as(old, &[], whitespace);
    let (added_hunks, added, _) = line_diff_as(&[], new, whitespace);
    hunks.extend(added_hunks);
    (hunks, added, removed)
}

/// The working tree's attribute files on the way to some paths, read through the branch's
/// folder held open (train 80 XG): each `.gitattributes` from the top down to a path's folder is
/// opened one component at a time following no link, as a file's content is ([`on_disk`]), and
/// gitoxide's attribute stack is built from those bytes, never from a path it opens itself. So a
/// `.gitattributes` that is a link, or one in a folder swapped for a link, says nothing — as git
/// ignores a `.gitattributes` that is a link — and no file outside the folder is read as one.
/// `info/attributes` and the user's attributes file are read as git reads them: they are not in
/// the folder a chat writes.
struct HeldAttributes {
    /// The branch's repo, with each attribute file read written into memory as a blob, never to
    /// disk.
    repo: gix::Repository,
    /// An index holding only those blobs, each at its file's path: what gitoxide's stack reads
    /// the attribute files from ([`gix::worktree::stack::state::attributes::Source::IdMapping`]).
    files: gix::index::State,
}

/// The largest attribute file read, as git's `ATTR_MAX_FILE_SIZE`: past it, git ignores the file.
const ATTRIBUTES_MOST: u64 = 100 * 1024 * 1024;

impl HeldAttributes {
    /// The attribute files on the way to each of `paths` in `held`; none when the folder is not
    /// held (`None`), so only `info/attributes` and the user's are read then.
    fn read<'p>(
        repo: &mut gix::Repository,
        held: Option<&super::Held>,
        paths: impl IntoIterator<Item = &'p str>,
    ) -> Self {
        use gix::bstr::ByteSlice as _;
        // A clone copies the objects `repo` keeps in memory, every blob of the working tree a
        // comparison wrote there among them. The attribute files need none of them, so they are
        // taken out for the clone and put back: nothing is held twice.
        let kept = repo.objects.take_object_memory();
        let memory = repo.clone().with_object_memory();
        if let Some(kept) = kept {
            repo.objects.set_object_memory(kept);
        }
        let mut files = gix::index::State::new(memory.object_hash());
        if let Some(held) = held {
            let mut folders: BTreeSet<&str> = BTreeSet::new();
            for path in paths {
                folders.insert("");
                let mut at = path;
                while let Some((parent, _)) = at.rsplit_once('/') {
                    if !folders.insert(parent) {
                        break;
                    }
                    at = parent;
                }
            }
            for folder in folders {
                let file = if folder.is_empty() {
                    ".gitattributes".to_string()
                } else {
                    format!("{folder}/.gitattributes")
                };
                let Some(bytes) = attribute_file(held, &file) else {
                    continue;
                };
                let Ok(id) = memory.write_blob(bytes) else {
                    continue;
                };
                files.dangerously_push_entry(
                    gix::index::entry::Stat::default(),
                    id.detach(),
                    gix::index::entry::Flags::empty(),
                    gix::index::entry::Mode::FILE,
                    file.as_bytes().as_bstr(),
                );
            }
            files.sort_entries();
        }
        HeldAttributes {
            repo: memory,
            files,
        }
    }

    /// gitoxide's attribute stack over the files read, `info/attributes` and the user's; `None`
    /// when those could not be read.
    fn stack(&self) -> Option<gix::AttributeStack<'_>> {
        use gix::worktree::stack::state::attributes::Source;
        self.repo
            .attributes_only(&self.files, Source::IdMapping)
            .ok()
    }
}

/// The attribute file `path` names in the held folder, opened one component at a time following
/// no link ([`super::open_inside`]); `None` when it is not there, is a link or under one, is not a
/// plain file, or is past [`ATTRIBUTES_MOST`].
fn attribute_file(held: &super::Held, path: &str) -> Option<Vec<u8>> {
    let mut file = super::open_inside(held, Path::new(path)).ok()?;
    let meta = file.metadata().ok()?;
    if !meta.is_file() || meta.len() > ATTRIBUTES_MOST {
        return None;
    }
    let mut bytes = Vec::with_capacity(meta.len() as usize);
    file.by_ref()
        .take(ATTRIBUTES_MOST + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    (bytes.len() as u64 <= ATTRIBUTES_MOST).then_some(bytes)
}

/// What the `diff` attribute says of a path, as `git diff` reads it: from the working tree's
/// attribute files ([`HeldAttributes`]), then `info/attributes` and the user's, never from the
/// index or a commit. The stack is `None` when they could not be read: the bytes decide then.
struct DiffAttribute<'r> {
    stack: Option<(gix::AttributeStack<'r>, gix::attrs::search::Outcome)>,
    repo: &'r gix::Repository,
}

impl<'r> DiffAttribute<'r> {
    fn of(attributes: &'r HeldAttributes) -> Self {
        DiffAttribute {
            stack: attributes.stack().map(|stack| {
                let outcome = stack.selected_attribute_matches(["diff"]);
                (stack, outcome)
            }),
            repo: &attributes.repo,
        }
    }

    /// `Some(true)` for `-diff` (and the `binary` macro), `Some(false)` for `diff` set, a named
    /// driver's `diff.<driver>.binary` where the config says it as a boolean, and `None` for
    /// anything else: the bytes decide then. Only that one key of a driver is kept when a
    /// branch is opened, so no driver's program is ever seen here, let alone run.
    fn says(&mut self, path: &str) -> Option<bool> {
        use gix::attrs::StateRef;
        let (stack, outcome) = self.stack.as_mut()?;
        outcome.reset();
        let at = stack
            .at_path(Path::new(path), Some(gix::index::entry::Mode::FILE))
            .ok()?;
        at.matching_attributes(outcome);
        match outcome.iter_selected().next()?.assignment.state {
            StateRef::Unset => Some(true),
            StateRef::Set => Some(false),
            StateRef::Value(driver) => self
                .repo
                .config_snapshot()
                .plumbing()
                .boolean_by("diff", Some(driver.as_bstr()), "binary")
                .ok()
                .flatten(),
            StateRef::Unspecified => None,
        }
    }

    /// Whether git calls `side`, at `path`, binary: what the attribute says, else a NUL in its
    /// first 8,000 bytes. A submodule's pointer never is.
    fn binary(&mut self, (side, kind): &(Side, Kind), path: &str) -> bool {
        if *kind == Kind::Submodule || matches!(side, Side::Missing) {
            return false;
        }
        self.says(path).unwrap_or_else(|| side.is_binary())
    }
}

/// A working-tree file made what `git add` would store (#704): gitoxide's built-in eol and
/// `ident` conversions and a `working-tree-encoding`, as the repo's kept config and the
/// attributes say. Every `filter` driver is dropped first, so no program can run.
struct ToGit<'r> {
    pipeline: gix::filter::plumbing::Pipeline,
    attributes: gix::AttributeStack<'r>,
    index: gix::worktree::Index,
    repo: &'r gix::Repository,
}

impl<'r> ToGit<'r> {
    /// `None` when the attributes or the index could not be read: the bytes are compared as
    /// they are then. The attributes are the working tree's as [`HeldAttributes`] read them,
    /// never the pipeline's own stack, which would read them by path.
    fn of(attributes: &'r HeldAttributes) -> Option<Self> {
        let repo = &attributes.repo;
        let (pipeline, _) = repo.filter_pipeline(None).ok()?;
        let (mut pipeline, _) = pipeline.into_parts();
        let options = pipeline.options_mut();
        options.drivers.clear();
        options.crlf_roundtrip_check = gix::filter::plumbing::pipeline::CrlfRoundTripCheck::Skip;
        Some(ToGit {
            pipeline,
            attributes: attributes.stack()?,
            index: repo.index_or_empty().ok()?,
            repo,
        })
    }

    /// `bytes`, read from `path`, as `git add` would store them; as they are when they cannot
    /// be converted.
    fn clean(&mut self, path: &str, bytes: Vec<u8>) -> Vec<u8> {
        use gix::bstr::ByteSlice as _;
        use gix::filter::plumbing::pipeline::convert::ToGitOutcome;
        use gix::prelude::Find as _;
        let ToGit {
            pipeline,
            attributes,
            index,
            repo,
        } = self;
        let Ok(at) = attributes.at_path(Path::new(path), Some(gix::index::entry::Mode::FILE))
        else {
            return bytes;
        };
        let staged = index
            .entry_by_path(path.as_bytes().as_bstr())
            .filter(|entry| entry.mode != gix::index::entry::Mode::COMMIT)
            .map(|entry| entry.id);
        let converted = pipeline.convert_to_git(
            bytes.as_slice(),
            Path::new(path),
            &mut |_, outcome| {
                at.matching_attributes(outcome);
            },
            // The staged blob: whether it holds a CR decides `core.autocrlf`'s conversion.
            &mut |buf| {
                Ok(staged.and_then(|id| {
                    let found = repo.objects.try_find(&id, buf).ok().flatten()?;
                    (found.kind == gix::object::Kind::Blob).then_some(())
                }))
            },
        );
        let clean = match converted {
            Ok(ToGitOutcome::Buffer(clean)) => Some(clean.to_vec()),
            Ok(ToGitOutcome::Unchanged(_)) | Ok(ToGitOutcome::Process(_)) | Err(_) => None,
        };
        clean.unwrap_or(bytes)
    }
}

/// The branch's folder, held open from the project's root following no link (see
/// [`super::hold_folder`]).
pub(super) fn hold(plane: &Path, base: &Path) -> Result<super::Held, String> {
    let root = std::fs::canonicalize(plane).map_err(|e| e.to_string())?;
    super::hold_folder(&root, base).map_err(|e| e.to_string())
}

/// `path` as it is on disk in the held folder, and the kind of entry git would stage it as: a
/// regular file's bytes (executable or not), or a link's target, as git compares them. Opened
/// one component at a time following no link, non-blocking, so a folder swapped for a link is
/// not followed and a FIFO is not waited on.
fn on_disk(held: &super::Held, path: &str, most: u64) -> (Side, gix::object::tree::EntryKind) {
    use gix::object::tree::EntryKind;
    let relative = Path::new(path);
    if let Some(target) = super::link_inside(held, relative) {
        return (Side::Bytes(target), EntryKind::Link);
    }
    let mut file = match super::open_inside(held, relative) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return (Side::Missing, EntryKind::Blob);
        }
        Err(_) => return (Side::Unreadable, EntryKind::Blob),
    };
    let Ok(meta) = file.metadata() else {
        return (Side::Unreadable, EntryKind::Blob);
    };
    let kind = if executable(&meta) {
        EntryKind::BlobExecutable
    } else {
        EntryKind::Blob
    };
    if !meta.is_file() {
        return (Side::Unreadable, kind);
    }
    if meta.len() > most {
        return (Side::TooLarge(meta.len()), kind);
    }
    let mut read = Vec::with_capacity(meta.len() as usize);
    if file.by_ref().take(most + 1).read_to_end(&mut read).is_err() {
        return (Side::Unreadable, kind);
    }
    if read.len() as u64 > most {
        return (Side::TooLarge(read.len() as u64), kind);
    }
    (Side::Bytes(read), kind)
}

#[cfg(unix)]
fn executable(meta: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt as _;
    meta.permissions().mode() & 0o100 != 0
}

#[cfg(not(unix))]
fn executable(_: &std::fs::Metadata) -> bool {
    false
}

/// git's line diff of `old` and `new`: Myers, then git's indent heuristic for where an
/// ambiguous hunk sits (git's defaults, `diff.algorithm` and `diff.indentHeuristic`, which a
/// branch's config cannot change here). Lines keep their line ends, so a last line that gains
/// one is a changed line, as git shows it. The hunks in `-U0`'s numbers, and the lines added and
/// removed.
#[cfg(test)]
fn line_diff(old: &[u8], new: &[u8]) -> (Vec<Hunk>, u64, u64) {
    line_diff_as(old, new, Whitespace::Shown)
}

/// **git's hunks between two texts**, in `-U0`'s numbers: the same line diff every comparison
/// draws ([`line_diff_as`], whitespace shown), for a change no branch holds yet, such as a
/// proposed change on its own tab (#2121), so the lines it marks are found as a branch's are.
pub fn line_hunks(old: &str, new: &str) -> Vec<Hunk> {
    line_diff_as(old.as_bytes(), new.as_bytes(), Whitespace::Shown).0
}

/// [`line_diff`], comparing whitespace as `whitespace` says.
///
/// With [`Whitespace::Hidden`], git's `-w` on the same engine: each line is interned without
/// the whitespace `-w` hides ([`hidden_by_dash_w`]), so two lines that differ only there are one
/// token, and a line that only gained its line end, or turned CRLF, is no change. Myers and the
/// indent heuristic then run on those tokens. The heuristic reads a token's indent from **the
/// first line spelled as that token**, the base side's first, then the head side's: where one
/// token is indented differently in different places, a hunk that could sit in more than one
/// place may sit elsewhere than `git diff -w` puts it, which measures each line where it stands
/// (ADR 0084's note of 2026-10-13). With [`Whitespace::Shown`] every spelling of a token is the
/// same, and this is the plain line diff.
fn line_diff_as(old: &[u8], new: &[u8], whitespace: Whitespace) -> (Vec<Hunk>, u64, u64) {
    use std::borrow::Cow;

    use gix::diff::blob::sources::byte_lines;
    use gix::diff::blob::{Algorithm, Diff, IndentHeuristic, IndentLevel, InternedInput, Token};
    fn compared(line: &[u8], whitespace: Whitespace) -> Cow<'_, [u8]> {
        match whitespace {
            Whitespace::Shown => Cow::Borrowed(line),
            Whitespace::Hidden => Cow::Owned(
                line.iter()
                    .copied()
                    .filter(|byte| !hidden_by_dash_w(*byte))
                    .collect(),
            ),
        }
    }
    let mut input: InternedInput<Cow<'_, [u8]>> = InternedInput::default();
    // Each token's indent, by token: read from the first line interned as it.
    let mut indents: Vec<IndentLevel> = Vec::new();
    for (text, before) in [(old, true), (new, false)] {
        for line in byte_lines(text) {
            let token = input.interner.intern(compared(line, whitespace));
            if token.0 as usize == indents.len() {
                indents.push(IndentLevel::for_ascii_line(line.iter().copied(), 8));
            }
            if before {
                input.before.push(token);
            } else {
                input.after.push(token);
            }
        }
    }
    let mut diff = Diff::compute(Algorithm::Myers, &input);
    diff.postprocess_with_heuristic(
        &input,
        IndentHeuristic::new(|token: Token| indents[token.0 as usize]),
    );
    let hunks = diff
        .hunks()
        .map(|hunk| {
            let old_lines = hunk.before.end - hunk.before.start;
            let new_lines = hunk.after.end - hunk.after.start;
            Hunk {
                old_start: hunk.before.start + u32::from(old_lines > 0),
                old_lines,
                new_start: hunk.after.start + u32::from(new_lines > 0),
                new_lines,
            }
        })
        .collect();
    (
        hunks,
        u64::from(diff.count_additions()),
        u64::from(diff.count_removals()),
    )
}

/// Whether `byte` is whitespace `git diff -w` takes out of a line: space, tab, line feed and
/// carriage return. git leaves the vertical tab and the form feed in, as a test checks.
fn hidden_by_dash_w(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | b'\r')
}

#[cfg(test)]
mod tests;

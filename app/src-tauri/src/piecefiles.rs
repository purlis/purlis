// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A branch's files, as the light editor's view tabs and the explorer's tree reach them (RC-5,
//! FM-1).
//!
//! Thin, as `worktrees.rs` is: which folder, which paths, what a folder holds and what a file
//! holds are `purlis_core::files`'s answers, and a refusal crosses as the core's sentence.
//!
//! Every command names a branch as a workspace, a repo and a piece — or no piece, for the
//! repo's own folder (#948) — and never as a directory.
//!
//! **The plane root's files have two commands of their own** (issue 1916): `plane_tree` and
//! `plane_file`, which take a project and a path and name no branch. They are the window's
//! alone (`purlis_session_protocol::ui::WINDOW_ONLY`), and the commands above cannot be
//! pointed at the plane root: the core's `Branch` for it is made by no workspace, repo or
//! piece name ([`branch`]). What they leave out is the core's to decide, in one place
//! (`purlis_core::files`, its `kept` module).

use std::path::Path;

use base64::Engine as _;
use purlis_core::files::{self, Branch, Entry, Kind, Mark, Opened, Reader, workspace};
use purlis_core::youreditor::{self, Checked, Editor, Launch};

use crate::chats::NoTurnsFrom;
use crate::planes::{PlaneId, Planes};

/// One file of a branch, as the light editor draws it.
#[derive(Debug, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum PieceFile {
    /// Text, to draw.
    Text { text: String },
    /// An image, known by its first bytes (FM-2): its media type and its bytes as base64. The
    /// window decodes it into a canvas, so nothing is loaded from a URL.
    Image { mime: String, base64: String },
    /// A file git would call binary, by its size in bytes.
    Binary { bytes: u32 },
    /// An image whose header declares more pixels than the preview draws (40 megapixels): its
    /// type and declared size, and none of its bytes.
    HugeImage {
        mime: String,
        width: u32,
        height: u32,
    },
    /// Past the largest file the preview draws (2 MiB), by its size in bytes.
    TooLarge { bytes: u32 },
}

/// One file of a branch, by its path relative to the branch's folder. Refused for a path that
/// leaves it.
// It reads up to the 5 MiB a file may be, so on a blocking thread and never the one that draws
// (#1007), as every command here that reads a branch is. Not a doc comment, because the
// generated bindings carry those.
#[tauri::command]
#[specta::specta]
pub async fn piece_file(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    workspace: String,
    repo: String,
    piece: Option<String>,
    path: String,
) -> Result<PieceFile, String> {
    // Resolved here, so a project that is not open refuses here rather than in the thread.
    let root = planes.held(&plane)?.root().to_path_buf();
    crate::off_the_window("reading the file", move || {
        file_of(
            &crate::reader(),
            &root,
            branch(&workspace, &repo, &piece),
            &path,
        )
    })
    .await
}

impl From<Opened> for PieceFile {
    fn from(opened: Opened) -> Self {
        // A size crosses as a `u32`: specta refuses a `u64` for TypeScript, which has no integer
        // that wide. A binary or oversized file past 4 GiB is said as 4 GiB.
        let bytes = |n: u64| u32::try_from(n).unwrap_or(u32::MAX);
        match opened {
            Opened::Text { text } => Self::Text { text },
            Opened::Image { mime, data } => Self::Image {
                mime: mime.to_string(),
                base64: base64::engine::general_purpose::STANDARD.encode(data),
            },
            Opened::HugeImage {
                mime,
                width,
                height,
            } => Self::HugeImage {
                mime: mime.to_string(),
                width,
                height,
            },
            Opened::Binary { bytes: n } => Self::Binary { bytes: bytes(n) },
            Opened::TooLarge { bytes: n } => Self::TooLarge { bytes: bytes(n) },
        }
    }
}

/// What one entry of a branch's folder is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "kebab-case")]
pub enum EntryKind {
    Folder,
    File,
    /// A symbolic link: never expanded, and opened only when it leads to another file the
    /// branch offers.
    Link,
}

/// One entry of a branch's folder, as the explorer's tree draws it (FM-1).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct FolderEntry {
    /// Its name in the folder, not its path.
    pub name: String,
    pub kind: EntryKind,
    /// Whether git ignores it: hidden unless the operator asks to see ignored files.
    pub ignored: bool,
    /// Why it does not open, in the core's sentence; `null` when it opens or expands.
    pub refused: Option<String>,
    /// At the plane root, the workspace whose repo this folder is: one row that leads to that
    /// workspace's tab, and never a folder that opens here. Left out everywhere else.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    /// In a workspace's own files (#5375), whether purlis keeps it: marked *managed*, and
    /// opened read-only. Left out where it is not.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub managed: bool,
    /// In a workspace's own files, whether it is a dotfile, drawn only with Show hidden. Left
    /// out where it is not.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub hidden: bool,
    /// In a LIVE workspace's own files (#5377), where it is kept. Left out everywhere else, a
    /// LOCAL workspace's files included: they carry no mark.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kept: Option<KeptMark>,
}

/// Where a LIVE workspace's file is kept (#5377).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "kebab-case")]
pub enum KeptMark {
    /// Ignored by the project's git: it stays on this machine.
    ThisMachineOnly,
    /// In the workspace's LIVE allowlist: a save shares it with the project.
    WithTheProject,
}

impl From<workspace::Kept> for KeptMark {
    fn from(kept: workspace::Kept) -> Self {
        match kept {
            workspace::Kept::ThisMachineOnly => Self::ThisMachineOnly,
            workspace::Kept::WithTheProject => Self::WithTheProject,
        }
    }
}

impl From<Kind> for EntryKind {
    fn from(kind: Kind) -> Self {
        match kind {
            Kind::Folder => Self::Folder,
            Kind::File => Self::File,
            Kind::Link => Self::Link,
        }
    }
}

impl From<Entry> for FolderEntry {
    fn from(entry: Entry) -> Self {
        Self {
            name: entry.name,
            kind: entry.kind.into(),
            ignored: entry.ignored,
            refused: entry.refused,
            workspace: entry.workspace,
            managed: false,
            hidden: false,
            kept: None,
        }
    }
}

impl From<workspace::Entry> for FolderEntry {
    fn from(entry: workspace::Entry) -> Self {
        Self {
            name: entry.name,
            kind: entry.kind.into(),
            ignored: false,
            refused: entry.refused,
            workspace: None,
            managed: entry.managed,
            hidden: entry.hidden,
            kept: entry.kept.map(KeptMark::from),
        }
    }
}

/// One folder of a branch, one level deep: its first entries, and how many more it holds.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct FolderListing {
    /// Folders first, then files, each in the order a person reads names; at most 5,000.
    pub entries: Vec<FolderEntry>,
    /// How many entries past those are not listed.
    pub more: u32,
}

/// One folder of a branch, one level deep: folders first, then files, each in the order a
/// person reads names, the first 5,000 and a count of the rest. `""` is the branch's own folder.
// Lazy by design (#1103 story 2): one directory listing and one ignore check per call, so
// nothing below a folder is read until it is expanded. The ignore check and the submodules are
// read by gitoxide in the core's bounded reader, so this process starts no git (#1189, FM-11).
// On a blocking thread and never the one that draws (SC-2): the reader answers in milliseconds
// on a warm disk, and in seconds on a cold one.
// Not a doc comment, because the generated bindings carry those.
#[tauri::command]
#[specta::specta]
pub async fn branch_tree(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    workspace: String,
    repo: String,
    piece: Option<String>,
    folder: String,
) -> Result<FolderListing, String> {
    // Resolved here, so a project that is not open refuses here rather than in the thread.
    let root = planes.held(&plane)?.root().to_path_buf();
    tauri::async_runtime::spawn_blocking(move || {
        tree_of(
            &crate::reader(),
            &root,
            branch(&workspace, &repo, &piece),
            &folder,
        )
    })
    .await
    .map_err(|err| format!("reading the folder did not finish: {err}"))?
}

/// What a branch did to one path (FM-4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "kebab-case")]
pub enum ChangeMark {
    /// Its content or its kind changed.
    Changed,
    /// It is new against the branch it was cut from.
    Added,
    /// It is gone.
    Deleted,
    /// It moved here from another path.
    Renamed,
}

impl From<Mark> for ChangeMark {
    fn from(mark: Mark) -> Self {
        match mark {
            Mark::Changed => Self::Changed,
            Mark::Added => Self::Added,
            Mark::Deleted => Self::Deleted,
            Mark::Renamed => Self::Renamed,
        }
    }
}

/// One path a branch changed.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct FileChange {
    /// Its path relative to the branch's folder.
    pub path: String,
    pub mark: ChangeMark,
    /// Where a renamed file came from: a name to show, never a path to open.
    pub from: Option<String>,
    /// Whether it is not committed yet.
    pub uncommitted: bool,
}

/// One folder holding changes: the mark they share (`changed` when they differ) and how many.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct FolderChanges {
    /// Its path relative to the branch's folder; `""` is the branch's own.
    pub folder: String,
    pub mark: ChangeMark,
    pub count: u32,
}

/// A path a branch changed that purlis names and never reads (#704): a folder on its path is
/// called `.git`, or its name is not a plain path inside the branch. Drawn with its mark and
/// why, and never opened, compared or offered as a file.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct RefusedFile {
    /// Its name as git spelled it: shown, never a path to open.
    pub path: String,
    pub mark: ChangeMark,
    /// Why it is not read, in the core's sentence.
    pub why: String,
    /// Whether it is not committed yet.
    pub uncommitted: bool,
}

impl From<files::RefusedChange> for RefusedFile {
    fn from(refused: files::RefusedChange) -> Self {
        Self {
            path: refused.path,
            mark: refused.mark.into(),
            why: refused.why.said().to_owned(),
            uncommitted: refused.uncommitted,
        }
    }
}

/// What a branch changed against the branch it was cut from, committed or not.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct BranchStatus {
    /// Sorted by path, at most 10,000.
    pub changes: Vec<FileChange>,
    /// The paths it changed that are named and never read, sorted by path, at most 10,000. On
    /// no folder's count, and never among `changes`.
    pub refused: Vec<RefusedFile>,
    /// Every folder holding a change, sorted by path.
    pub folders: Vec<FolderChanges>,
    /// How many changes past those are not listed.
    pub more: u32,
    /// The branch the changes are counted against; `null` when against the last commit.
    pub base: Option<String>,
}

/// What a branch changed against the branch it was cut from, committed or not, file by file and
/// rolled up onto its folders: the explorer's markers and its "Changed only" (FM-4).
// Read by gitoxide in the core's bounded reader, a short-lived child of this binary killed past
// its deadline and memory cap; no git process reads the branch's config for this (D-88f,
// D-88h). On a blocking thread and never the one that draws (SC-2). Not a doc comment, because
// the generated bindings carry those.
#[tauri::command]
#[specta::specta]
pub async fn branch_status(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    workspace: String,
    repo: String,
    piece: Option<String>,
) -> Result<BranchStatus, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    tauri::async_runtime::spawn_blocking(move || {
        status_of(&root, branch(&workspace, &repo, &piece))
    })
    .await
    .map_err(|err| format!("reading what the branch changed did not finish: {err}"))?
}

/// What one repo's own folder changed, rolled up onto its top: a closed repo row of the explorer
/// (#3343, #3489).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct RepoChanged {
    pub repo: String,
    /// Its own folder's mark and how many files it holds that changed; `null` when it changed
    /// nothing, or when it was refused.
    pub changes: Option<FolderChanges>,
    /// Why it could not be read, in the sentence the window shows; `null` when it was.
    pub trouble: Option<String>,
}

/// What each of `repos` changed in its own folder, rolled up onto its top: what the explorer's
/// closed repo rows say, for every repo of a workspace in one ask (#3489).
// Each repo's answer is kept per clone in the core (`purlis_core::files::repo_changes`), on the
// store the save standings share and the one watch per repo touches, so an ask with nothing moved
// reads no status at all, and one after a write reads only the repo it was in. A repo that is
// read is read by the bounded reader, as `branch_status` is, a few at a time on threads of their
// own (`AT_ONCE`). Not a doc comment, because the generated bindings carry
// those.
#[tauri::command]
#[specta::specta]
pub async fn repos_changed(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    workspace: String,
    repos: Vec<String>,
) -> Result<Vec<RepoChanged>, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    tauri::async_runtime::spawn_blocking(move || {
        let reader = crate::reader();
        changed_in(&root, &workspace, &repos, |branch| {
            files::status(&reader, &root, branch)
        })
    })
    .await
    .map_err(|err| format!("reading what the repos changed did not finish: {err}"))
}

/// [`repos_changed`]'s answer, each repo's from `read` where the core has none kept.
fn changed_in(
    plane: &Path,
    workspace: &str,
    repos: &[String],
    read: impl Fn(Branch<'_>) -> Result<files::Status, files::Refused> + Sync,
) -> Vec<RepoChanged> {
    // At most `AT_ONCE` threads at a time, whatever the workspace holds: a kept answer costs a
    // lookup, and a read waits for the bounded reader's line anyway.
    repos
        .chunks(AT_ONCE)
        .flat_map(|some| changed_at_once(plane, workspace, some, &read))
        .collect()
}

/// How many repos [`changed_in`] asks the core about at once.
const AT_ONCE: usize = 8;

fn changed_at_once(
    plane: &Path,
    workspace: &str,
    repos: &[String],
    read: &(impl Fn(Branch<'_>) -> Result<files::Status, files::Refused> + Sync),
) -> Vec<RepoChanged> {
    std::thread::scope(|scope| {
        let asked: Vec<_> = repos
            .iter()
            .map(|repo| scope.spawn(move || files::repo_changes(plane, workspace, repo, read)))
            .collect();
        repos
            .iter()
            .zip(asked)
            .map(|(repo, asked)| {
                let answer = asked
                    .join()
                    .unwrap_or_else(|_| Err("reading what it changed stopped".to_string()));
                let (changes, trouble) = match answer {
                    Ok(rolled) => (rolled.map(folder_changes), None),
                    Err(why) => (None, Some(why)),
                };
                RepoChanged {
                    repo: repo.clone(),
                    changes,
                    trouble,
                }
            })
            .collect()
    })
}

fn folder_changes(one: files::Rolled) -> FolderChanges {
    FolderChanges {
        folder: one.folder,
        mark: one.mark.into(),
        count: u32::try_from(one.count).unwrap_or(u32::MAX),
    }
}

/// How far a branch is from the branch it was cut from: the branch cockpit's header (FM-5).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct AheadBehind {
    /// Commits the branch has that its base does not.
    pub ahead: u32,
    /// Commits its base gained that the branch does not have.
    pub behind: u32,
    /// The base they are counted against; `null` when the branch has no base recorded, and the
    /// counts then mean nothing.
    pub base: Option<String>,
}

/// How far a branch is from the branch it was cut from, in commits ahead and behind.
// Read by gitoxide in the core's bounded reader, as `branch_status` is: no git process reads
// the branch's config for this (V88a, D-88f, D-88h). Not a doc comment, because the generated
// bindings carry those.
#[tauri::command]
#[specta::specta]
pub async fn branch_ahead_behind(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    workspace: String,
    repo: String,
    piece: Option<String>,
) -> Result<AheadBehind, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    tauri::async_runtime::spawn_blocking(move || {
        ahead_behind_of(&root, branch(&workspace, &repo, &piece))
    })
    .await
    .map_err(|err| format!("reading how far the branch is from its base did not finish: {err}"))?
}

/// One hunk, in `git diff -U0`'s numbers: lines counted from 1, and a side with no lines naming
/// the line the hunk comes after (`0` for the top). The window's `GitHunk`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GitHunk {
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
}

impl From<GitHunk> for files::Hunk {
    fn from(one: GitHunk) -> Self {
        Self {
            old_start: one.old_start,
            old_lines: one.old_lines,
            new_start: one.new_start,
            new_lines: one.new_lines,
        }
    }
}

impl From<files::Hunk> for GitHunk {
    fn from(one: files::Hunk) -> Self {
        Self {
            old_start: one.old_start,
            old_lines: one.old_lines,
            new_start: one.new_start,
            new_lines: one.new_lines,
        }
    }
}

/// One file's change, as the comparison tab draws it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum FileDiff {
    /// Text: each side (empty where the file is absent) and git's hunks between them.
    Text {
        base: String,
        head: String,
        hunks: Vec<GitHunk>,
    },
    /// A side git would call binary: said, not drawn.
    Binary,
    /// A side past the largest file the preview draws (2 MiB), by its size in bytes.
    TooLarge { bytes: u32 },
}

/// One file of a branch against the branch it was cut from: "Show what changed" (FM-11).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct WhatChanged {
    pub mark: ChangeMark,
    /// Where a renamed file came from: a name to show, never a path to open.
    pub from: Option<String>,
    /// Whether some of its change is not committed yet.
    pub uncommitted: bool,
    /// The branch it is compared against; `null` when against the last commit.
    pub base: Option<String>,
    pub diff: FileDiff,
    /// What a discard of a hunk here is checked against (`null` for a file past what is drawn).
    pub stamp: Option<String>,
    /// The branch's check results as this file shows them (ADR 0117).
    pub checks: Box<CheckResults>,
    /// On a turn's diff, that the file changed since the turn ended: it is drawn from the turn's
    /// start to the file as it is now (#5411).
    #[serde(rename = "sinceTurnEnded")]
    pub since_turn_ended: bool,
}

/// How much a check result matters, in SARIF's words.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum CheckSeverity {
    Error,
    Warning,
    Note,
    None,
}

/// One tool's results for a branch, and whether the branch has moved since they were reported.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CheckSet {
    pub tool: String,
    /// The commit they were reported for; `null` for a branch with no commit then.
    pub head: Option<String>,
    pub out_of_date: bool,
    pub results: u32,
}

/// One result of a check.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CheckResult {
    pub tool: String,
    pub out_of_date: bool,
    pub severity: CheckSeverity,
    /// The file as the tool named it, relative to the branch's folder where it could be placed
    /// there; `null` for a result on no file.
    pub path: Option<String>,
    /// Its first line, from 1; `null` for a result on the file as a whole.
    pub line: Option<u32>,
    /// The first of its lines a hunk shows, where one does: where the gutter marks it.
    pub shown_at: Option<u32>,
    pub rule: Option<String>,
    pub message: String,
}

/// A branch's check results as one file of its comparison shows them. Every result is in a list or
/// counted past it.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CheckResults {
    pub sets: Vec<CheckSet>,
    /// On a line a hunk shows.
    pub on_lines: Vec<CheckResult>,
    /// In this file, on no line a hunk shows.
    pub outside: Vec<CheckResult>,
    /// In any other file, or in none.
    pub elsewhere: Vec<CheckResult>,
    pub more_on_lines: u32,
    pub more_outside: u32,
    pub more_elsewhere: u32,
    /// Why they could not be read, where they could not.
    pub trouble: Option<String>,
    /// Each Check reporter whose last answer could not be recorded, for the whole branch (#5410).
    pub faults: Vec<CheckFault>,
}

/// A Check reporter whose last answer could not be recorded: what the review says in a Notice
/// naming it (#5410). The results before it stay.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CheckFault {
    /// The extension, by its id.
    pub extension: String,
    /// What was wrong, in the core's sentence, naming it.
    pub said: String,
}

impl From<purlis_core::checks::OnFile> for CheckResults {
    fn from(on: purlis_core::checks::OnFile) -> Self {
        let count = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
        let list = |placed: Vec<purlis_core::checks::Placed>| -> Vec<CheckResult> {
            placed
                .into_iter()
                .map(|one| CheckResult {
                    tool: one.tool,
                    out_of_date: one.out_of_date,
                    severity: match one.diagnostic.severity {
                        purlis_core::checks::Severity::Error => CheckSeverity::Error,
                        purlis_core::checks::Severity::Warning => CheckSeverity::Warning,
                        purlis_core::checks::Severity::Note => CheckSeverity::Note,
                        purlis_core::checks::Severity::None => CheckSeverity::None,
                    },
                    path: one.diagnostic.path,
                    line: one.diagnostic.line,
                    shown_at: one.shown_at,
                    rule: one.diagnostic.rule,
                    message: one.diagnostic.message,
                })
                .collect()
        };
        Self {
            sets: on
                .sets
                .into_iter()
                .map(|set| CheckSet {
                    tool: set.tool,
                    head: set.head,
                    out_of_date: set.out_of_date,
                    results: count(set.results),
                })
                .collect(),
            on_lines: list(on.on_lines),
            outside: list(on.outside),
            elsewhere: list(on.elsewhere),
            more_on_lines: count(on.more_on_lines),
            more_outside: count(on.more_outside),
            more_elsewhere: count(on.more_elsewhere),
            trouble: on.trouble,
            faults: on
                .faults
                .into_iter()
                .map(|fault| CheckFault {
                    extension: fault.extension,
                    said: fault.said,
                })
                .collect(),
        }
    }
}

/// One file of a branch compared against the branch it was cut from, committed or not: its
/// lines and git's hunks, or what it is when it is not drawn as lines. Refused, in the core's
/// sentence, for a path any file command would refuse and for a file the branch did not change.
// Read by gitoxide in the core's bounded reader, as `branch_status` is (RC-2, D-88f, D-88h): one
// ask, in which the child finds the branch's folder, confines the path in it, and reads the
// comparison, so this process starts no git for it (#1189). On a blocking thread and never the
// one that draws (SC-2). Not a doc comment, because the generated bindings carry those.
#[tauri::command]
#[specta::specta]
pub async fn what_changed(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    workspace: String,
    repo: String,
    piece: Option<String>,
    path: String,
) -> Result<WhatChanged, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    tauri::async_runtime::spawn_blocking(move || {
        what_changed_of(&root, branch(&workspace, &repo, &piece), &path)
    })
    .await
    .map_err(|err| format!("comparing the file did not finish: {err}"))?
}

/// One file across one turn on a branch: the file diff's turn picker (#5405, #5408).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum TurnChanged {
    /// No turn to show, and why, in a sentence.
    NoTurn { why: String },
    /// The file across turn `number` of `chat`, and that chat's turns on the branch, newest
    /// first: what the picker lists.
    Of {
        chat: String,
        number: u32,
        turns: Vec<u32>,
        file: Box<InTurn>,
        /// Where this turn is the one Undo is offered on, the latest not yet undone: every path
        /// the undo would write, which its confirmation lists (#5411). `null` on any other.
        undo: Option<Vec<String>>,
    },
}

/// What one turn did to one file.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum InTurn {
    /// The turn changed no file at all.
    Nothing,
    /// The turn changed other files and left this one as it found it.
    NotThisFile,
    /// The file, compared across the turn.
    Changed { shown: WhatChanged },
}

/// A turn the picker names: a chat's id and the turn's number.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, specta::Type)]
pub struct TurnPick {
    pub chat: String,
    pub number: u32,
}

/// One file of a branch across a turn that ended there, the latest when `turn` is null, or why
/// there is none.
// Read as `what_changed` reads. The sentence for no turn is the app's, since only the app knows
// which chats work in the folder. Not a doc comment, because the generated bindings carry those.
#[tauri::command]
#[specta::specta]
pub async fn turn_changed(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    workspace: String,
    repo: String,
    piece: Option<String>,
    path: String,
    turn: Option<TurnPick>,
) -> Result<TurnChanged, String> {
    let held = planes.held(&plane)?;
    let root = held.root().to_path_buf();
    let silent = held
        .chats()
        .no_turns_from_in(&root, &workspace, &repo, piece.as_deref());
    tauri::async_runtime::spawn_blocking(move || {
        turn_changed_of(
            &root,
            branch(&workspace, &repo, &piece),
            &path,
            turn,
            silent.as_ref(),
        )
    })
    .await
    .map_err(|err| format!("comparing the file did not finish: {err}"))?
}

fn turn_changed_of(
    plane: &Path,
    branch: Branch<'_>,
    path: &str,
    turn: Option<TurnPick>,
    silent: Option<&NoTurnsFrom>,
) -> Result<TurnChanged, String> {
    let turn = turn.map(|TurnPick { chat, number }| files::TurnKey { chat, number });
    let read = files::turn_changed(&crate::reader(), plane, branch, path, turn.as_ref())
        .map_err(|e| e.to_string())?;
    Ok(match read {
        files::TurnChanged::NoTurn => TurnChanged::NoTurn {
            why: no_turn_why(silent),
        },
        files::TurnChanged::Of {
            chat,
            number,
            turns,
            file,
            undo,
        } => TurnChanged::Of {
            chat,
            number,
            turns,
            undo,
            file: Box::new(match file {
                files::InTurn::Nothing => InTurn::Nothing,
                files::InTurn::NotThisFile => InTurn::NotThisFile,
                files::InTurn::Changed(shown) => InTurn::Changed {
                    shown: what_changed_shown(*shown),
                },
            }),
        },
    })
}

/// Why a branch has no turn to show: a chat working there that records none, by name, or no turn
/// yet.
fn no_turn_why(silent: Option<&NoTurnsFrom>) -> String {
    match silent {
        Some(NoTurnsFrom::Unreported { chat }) => format!(
            "{chat} runs a program that does not report its turns to purlis, so it has no turn \
             to show here. Against base still shows this file."
        ),
        Some(NoTurnsFrom::OnARunner { chat, runner }) => {
            on_a_runner(chat, &format!("runner {runner}"))
        }
        Some(NoTurnsFrom::OnAPool { chat, pool }) => format!(
            "{chat} runs on a runner of pool {pool} whose purlis is too old to record its turns, \
             so it has no turn to show here. Against base still shows this file."
        ),
        None => "No chat has finished a turn on this branch yet.".to_owned(),
    }
}

/// A chat on a runner works in a folder of the runner's, so purlis records none of its turns
/// (#5427): `on` is where it runs.
fn on_a_runner(chat: &str, on: &str) -> String {
    format!(
        "{chat} runs on {on}, and purlis does not record the turns of a chat on a runner yet, so \
         it has no turn to show here. Against base still shows this file."
    )
}

fn what_changed_of(plane: &Path, branch: Branch<'_>, path: &str) -> Result<WhatChanged, String> {
    let shown =
        files::what_changed(&crate::reader(), plane, branch, path).map_err(|e| e.to_string())?;
    Ok(what_changed_shown(shown))
}

/// The core's answer, as the comparison tab draws it.
pub(crate) fn what_changed_shown(shown: files::WhatChanged) -> WhatChanged {
    let diff = match shown.diff {
        files::FileDiff::Text { base, head, hunks } => FileDiff::Text {
            base,
            head,
            hunks: hunks.into_iter().map(GitHunk::from).collect(),
        },
        files::FileDiff::Binary => FileDiff::Binary,
        files::FileDiff::TooLarge { bytes } => FileDiff::TooLarge {
            bytes: u32::try_from(bytes).unwrap_or(u32::MAX),
        },
    };
    WhatChanged {
        mark: shown.change.mark.into(),
        from: shown.change.from,
        uncommitted: shown.change.uncommitted,
        base: shown.base,
        diff,
        stamp: shown.stamp,
        checks: Box::new((*shown.checks).into()),
        since_turn_ended: shown.since_turn_ended,
    }
}

pub(crate) fn ahead_behind_of(plane: &Path, branch: Branch<'_>) -> Result<AheadBehind, String> {
    let count = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
    files::ahead_behind(&crate::reader(), plane, branch)
        .map(|apart| AheadBehind {
            ahead: count(apart.ahead),
            behind: count(apart.behind),
            base: apart.base,
        })
        .map_err(|refused| refused.to_string())
}

pub(crate) fn status_of(plane: &Path, branch: Branch<'_>) -> Result<BranchStatus, String> {
    status_by(&crate::reader(), plane, branch)
}

fn status_by(reader: &Reader, plane: &Path, branch: Branch<'_>) -> Result<BranchStatus, String> {
    let count = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
    files::status(reader, plane, branch)
        .map(|status| BranchStatus {
            changes: status
                .changes
                .into_iter()
                .map(|one| FileChange {
                    path: one.path,
                    mark: one.mark.into(),
                    from: one.from,
                    uncommitted: one.uncommitted,
                })
                .collect(),
            refused: status.refused.into_iter().map(RefusedFile::from).collect(),
            folders: status
                .folders
                .into_iter()
                .map(|one| FolderChanges {
                    folder: one.folder,
                    mark: one.mark.into(),
                    count: count(one.count),
                })
                .collect(),
            more: count(status.more),
            base: status.base,
        })
        .map_err(|refused| refused.to_string())
}

/// One folder of the plane root, one level deep, as [`branch_tree`] answers a branch's: the
/// whole project's files, without what purlis keeps to itself (its vaults, their keys and its
/// own records), and with each workspace's repo as one row that names its workspace. `""` is
/// the project's own folder.
// The window's alone (`WINDOW_ONLY`, issue 1916): it lists the project's own files, which no
// link is served. Lazy, as `branch_tree` is: one listing and one ignore check per call, read by
// gitoxide in the bounded reader, on a blocking thread. Not a doc comment, because the generated
// bindings carry those.
#[tauri::command]
#[specta::specta]
pub async fn plane_tree(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    folder: String,
) -> Result<FolderListing, String> {
    // Resolved here, so a project that is not open refuses here rather than in the thread.
    let root = planes.held(&plane)?.root().to_path_buf();
    crate::off_the_window("reading the folder", move || {
        tree_of(&crate::reader(), &root, Branch::plane_root(), &folder)
    })
    .await
}

/// One file of the plane root, by its path relative to the project's folder, as [`piece_file`]
/// answers a branch's. Refused for a path that leaves the project, for one purlis keeps to
/// itself, and for one inside a workspace's repo.
// The window's alone (`WINDOW_ONLY`, issue 1916), and on a blocking thread, as `piece_file` is.
// Not a doc comment, because the generated bindings carry those.
#[tauri::command]
#[specta::specta]
pub async fn plane_file(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    path: String,
) -> Result<PieceFile, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    crate::off_the_window("reading the file", move || {
        file_of(&crate::reader(), &root, Branch::plane_root(), &path)
    })
    .await
}

/// One folder of a workspace's own files (#5375), one level deep, as [`branch_tree`] answers a
/// branch's: everything in the workspace's folder but its repos and their branches' folders, each
/// entry saying whether purlis keeps it and whether it is hidden. At the top, the workspace's
/// repos, each with where it is. `""` is the workspace's folder.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct WorkspaceListing {
    /// Folders first, then files, each in the order a person reads names; at most 5,000.
    pub entries: Vec<FolderEntry>,
    /// How many entries past those are not listed.
    pub more: u32,
    /// At the top, the workspace's repos by name; none below it.
    pub repos: Vec<WorkspaceRepo>,
}

/// One repo of a workspace, where its folder's top holds it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct WorkspaceRepo {
    pub name: String,
    /// In `repos/`, or at the top as a clone made before structure version 6 (ADR 0114).
    pub inside_repos: bool,
}

/// A file of a workspace's own files, as the light editor draws it, and whether purlis keeps it.
#[derive(Debug, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct WorkspaceFile {
    pub file: PieceFile,
    /// Whether purlis keeps it: opened read-only.
    pub managed: bool,
    /// The view of its own a managed file has, where the window has one.
    pub view: Option<OwnFileView>,
}

/// A managed workspace file's own view (#5375).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "kebab-case")]
pub enum OwnFileView {
    Memory,
    Todos,
    Sessions,
    /// `workspace.md`'s vision, which has an editor of its own.
    Vision,
}

impl From<workspace::View> for OwnFileView {
    fn from(view: workspace::View) -> Self {
        match view {
            workspace::View::Memory => Self::Memory,
            workspace::View::Todos => Self::Todos,
            workspace::View::Sessions => Self::Sessions,
            workspace::View::Vision => Self::Vision,
        }
    }
}

// The window's alone (`WINDOW_ONLY`, #5375), as `plane_tree` is: it lists the person's own files,
// which no link is served. Read here, on a blocking thread, by no git: the plane's `.gitignore`
// ignores a workspace's files. Not a doc comment, because the generated bindings carry those.
#[tauri::command]
#[specta::specta]
pub async fn workspace_tree(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    workspace: String,
    folder: String,
) -> Result<WorkspaceListing, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    crate::off_the_window("reading the folder", move || {
        workspace_tree_of(&root, &workspace, &folder, &extension_folders())
    })
    .await
}

/// One file of a workspace's own files, by its path in the workspace's folder, and whether purlis
/// keeps it. Refused for a path that leaves the folder or is a repo's, and through a link.
// The window's alone (`WINDOW_ONLY`, #5375), on a blocking thread, as `plane_file` is. Not a doc
// comment, because the generated bindings carry those.
#[tauri::command]
#[specta::specta]
pub async fn workspace_file(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    workspace: String,
    path: String,
) -> Result<WorkspaceFile, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    crate::off_the_window("reading the file", move || {
        workspace_file_of(&root, &workspace, &path, &extension_folders())
    })
    .await
}

/// **Save with the project** (#5377): one file or folder at the top of a LIVE workspace's folder
/// gets one line in the workspace's LIVE allowlist, so the next save shares it. `true` when the
/// line was added, `false` when it was there already.
// The window's alone (`WINDOW_ONLY`): only the person's press shares a file of theirs, since a
// script may hold a token, so no link may ask for it. Not a doc comment's place, as above.
#[tauri::command]
#[specta::specta]
pub async fn workspace_save_with_the_project(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    workspace: String,
    path: String,
) -> Result<bool, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    crate::off_the_window("saving the file with the project", move || {
        workspace_save_of(&root, &workspace, &path, &extension_folders())
    })
    .await
}

pub(crate) fn workspace_save_of(
    plane: &Path,
    ws: &str,
    path: &str,
    extension_folders: &[String],
) -> Result<bool, String> {
    in_workspace_files(plane, ws, |files| {
        files.save_with_the_project(path, extension_folders)
    })
    .map_err(|why| why.to_string())
}

/// The folders the extensions this machine approved keep in a workspace: managed there.
pub(crate) fn extension_folders() -> Vec<String> {
    purlis_core::machine::config_root()
        .map(|config| {
            purlis_core::extension::events::carried(&config, &crate::extensions::built_in())
        })
        .unwrap_or_default()
}

/// What `read` answers of workspace `ws`'s own files (#5375). Its error is named here, so the
/// window's copy check (`copy.rust.test.ts`) reads the core's sentences each caller passes on.
fn in_workspace_files<T>(
    plane: &Path,
    ws: &str,
    read: impl FnOnce(&workspace::Files) -> Result<T, workspace::NotHere>,
) -> Result<T, workspace::NotHere> {
    read(&workspace::Files::of(plane, ws)?)
}

pub(crate) fn workspace_tree_of(
    plane: &Path,
    ws: &str,
    folder: &str,
    extension_folders: &[String],
) -> Result<WorkspaceListing, String> {
    let listing = in_workspace_files(plane, ws, |files| files.tree(folder, extension_folders))
        .map_err(|why| why.to_string())?;
    Ok(WorkspaceListing {
        entries: listing.entries.into_iter().map(FolderEntry::from).collect(),
        more: u32::try_from(listing.more).unwrap_or(u32::MAX),
        repos: listing
            .repos
            .into_iter()
            .map(|repo| WorkspaceRepo {
                name: repo.name,
                inside_repos: repo.layout == purlis_core::repofolder::Layout::Repos,
            })
            .collect(),
    })
}

/// **One file of the place the window names**, read as its preview reads it (#5953): the plane
/// root's with no workspace and no repo, a workspace's own with no repo, else a branch's. What
/// an `.html` file's private preview and its Pin to workspace read.
pub(crate) fn file_at(
    plane: &Path,
    workspace: &str,
    repo: &str,
    piece: &Option<String>,
    path: &str,
) -> Result<PieceFile, String> {
    if workspace.is_empty() && repo.is_empty() && piece.is_none() {
        file_of(&crate::reader(), plane, Branch::plane_root(), path)
    } else if repo.is_empty() && piece.is_none() {
        workspace_file_of(plane, workspace, path, &extension_folders()).map(|read| read.file)
    } else {
        file_of(
            &crate::reader(),
            plane,
            branch(workspace, repo, piece),
            path,
        )
    }
}

pub(crate) fn workspace_file_of(
    plane: &Path,
    ws: &str,
    path: &str,
    extension_folders: &[String],
) -> Result<WorkspaceFile, String> {
    let opened =
        in_workspace_files(plane, ws, |files| files.open(path)).map_err(|why| why.to_string())?;
    let managed = workspace::managed(path, extension_folders);
    Ok(WorkspaceFile {
        file: opened.into(),
        managed,
        view: managed
            .then(|| workspace::view_of(path))
            .flatten()
            .map(OwnFileView::from),
    })
}

/// The branch the window named: a piece of a repo, or the repo's own folder. Shared with
/// `filewatch.rs`, which names branches the same way.
///
/// **Never the plane root** (issue 1916), whatever the names: an empty workspace or repo is a
/// name the core refuses, so every command that reads through here stays a branch's.
pub(crate) fn branch<'a>(
    workspace: &'a str,
    repo: &'a str,
    piece: &'a Option<String>,
) -> Branch<'a> {
    match piece {
        Some(piece) => Branch::piece(workspace, repo, piece),
        // **A workspace's own files** (#5375): a workspace and no repo, which no branch is, so
        // each command below reads them through `files::workspace` and nothing else.
        None if repo.is_empty() && !workspace.is_empty() => Branch::workspace_files(workspace),
        None => Branch::repo(workspace, repo),
    }
}

/// **The place a folder watch names** (issue 1916): a branch, as [`branch`] answers, or — with
/// no workspace and no repo, which name no branch — the plane root. Only the folder watch
/// reads through here: it resolves folders with the tree's own checks and tells the window
/// that one moved, and hands back nothing a folder holds.
pub(crate) fn watched_place<'a>(
    workspace: &'a str,
    repo: &'a str,
    piece: &'a Option<String>,
) -> Branch<'a> {
    if workspace.is_empty() && repo.is_empty() && piece.is_none() {
        Branch::plane_root()
    } else {
        branch(workspace, repo, piece)
    }
}

// The branch's folder is found by the bounded reader's child (#1189), and so are what git
// ignores in it and its submodules, so this process starts no git; the folder is then listed
// here, as `files::tree` lists one.
fn tree_of(
    reader: &Reader,
    plane: &Path,
    branch: Branch<'_>,
    folder: &str,
) -> Result<FolderListing, String> {
    files::root(reader, plane, branch)
        .and_then(|root| root.tree(reader, folder))
        .map(|level| FolderListing {
            entries: level.entries.into_iter().map(FolderEntry::from).collect(),
            more: u32::try_from(level.more).unwrap_or(u32::MAX),
        })
        .map_err(|refused| refused.to_string())
}

/// Which editor the operator chose in Settings (RC-20, ADR 0081 §3). The window
/// names one of these four and nothing else: never a program, never a URL.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, specta::Type)]
#[serde(rename_all = "kebab-case")]
pub enum YourEditor {
    /// Visual Studio Code, through `vscode://`.
    Vscode,
    /// Zed, through `zed://`.
    Zed,
    /// A JetBrains IDE, through `idea://`.
    Idea,
    /// `$VISUAL`, else `$EDITOR`, from charter's own environment, with `+line`.
    Variable,
}

impl From<YourEditor> for Editor {
    fn from(editor: YourEditor) -> Self {
        match editor {
            YourEditor::Vscode => Self::VsCode,
            YourEditor::Zed => Self::Zed,
            YourEditor::Idea => Self::Idea,
            YourEditor::Variable => Self::Variable,
        }
    }
}

/// `$VISUAL`/`$EDITOR` started, on a blocking thread: [`youreditor::start`] waits up to
/// [`youreditor::AT_ONCE`] to hear whether the editor exited at once (#1044), which the window's
/// own thread must never wait for.
pub(crate) async fn started(program: String, args: Vec<std::ffi::OsString>) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || youreditor::start(&program, &args))
        .await
        .map_err(|err| format!("starting your editor did not finish: {err}"))?
}

/// One file of a branch, opened in your editor at a line (RC-20). Refused, in the core's
/// sentence, for any path the light editor would refuse.
// The path is checked by `purlis_core::files::in_your_editor` exactly as `piece_file`
// checks it, and held as it was checked (`Checked`, #3185): the launch is asked for right
// before it happens, and refused when the path no longer leads to that file. The editor's own
// open, later, is narrowed, not closed. The editor is handed the resolved absolute path: as
// a URL to the operating system's opener, or as one argument of the program
// `$VISUAL`/`$EDITOR` names, never through a shell: a terminal editor in a shell tab of its
// own (`editortab`, #1042), which is why the command is the window's alone (`WINDOW_ONLY`).
// Not a doc comment, because the generated bindings carry those.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
#[specta::specta]
pub async fn open_in_your_editor(
    app: tauri::AppHandle,
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    workspace: String,
    repo: String,
    piece: Option<String>,
    path: String,
    line: u32,
    editor: YourEditor,
) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt as _;
    let var = purlis_core::envvar::var;
    let held = planes.held(&plane)?;
    let checked = launch_of(
        &crate::reader(),
        held.root(),
        branch(&workspace, &repo, &piece),
        &path,
        line,
        editor,
        &var,
    )?;
    // Asked again right before the launch: still the file checked (#3185).
    match checked.launch().map_err(|refused| refused.to_string())? {
        Launch::Url(url) => app
            .opener()
            .open_url(&url, None::<&str>)
            .map_err(|e| format!("the system did not open {url}: {e}")),
        Launch::Program { program, args } => {
            let cwd = || shell_folder(held.root(), branch(&workspace, &repo, &piece), "");
            crate::editortab::started(&app, &held, &plane, &workspace, cwd, program, args).await
        }
    }
}

/// **Open file** on a chat's start notice (NO-4): the operator's `AGENTS.md` at the top of a
/// branch, which charter's exclude line hides from `git status`, opened in your editor.
/// Refused, in the core's sentence, for an `AGENTS.md` that is not theirs.
// Placed by name by `purlis_core::guest::their_agents_md_in_your_editor`, never a path the
// window sent; handed to the editor as `open_in_your_editor` hands one. Not a doc comment,
// because the generated bindings carry those.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
#[specta::specta]
pub async fn open_their_agents_md(
    app: tauri::AppHandle,
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    workspace: String,
    repo: String,
    piece: Option<String>,
    editor: YourEditor,
) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt as _;
    let var = |name: &str| std::env::var(name).ok();
    let held = planes.held(&plane)?;
    let checked = purlis_core::guest::their_agents_md_in_your_editor(
        held.root(),
        branch(&workspace, &repo, &piece),
        editor.into(),
        &var,
    )?;
    // Asked again right before the launch: still the file checked (#3185).
    match checked.launch().map_err(|refused| refused.to_string())? {
        Launch::Url(url) => app
            .opener()
            .open_url(&url, None::<&str>)
            .map_err(|e| format!("the system did not open {url}: {e}")),
        Launch::Program { program, args } => {
            let cwd = || shell_folder(held.root(), branch(&workspace, &repo, &piece), "");
            crate::editortab::started(&app, &held, &plane, &workspace, cwd, program, args).await
        }
    }
}

/// **Move aside…** on a chat's start notice (NO-4), after the window asked: the operator's
/// `AGENTS.md` at the top of a branch renamed to `AGENTS.aside.md` (or the next free
/// `AGENTS.aside-N.md`), never over a file, so `git status` shows it again. Answers its new
/// name. Refused, touching nothing, for an `AGENTS.md` that is not theirs.
// Off the window's thread (#1007): it asks git whether the file is the operator's before it
// renames anything. Not a doc comment, because the generated bindings carry those.
#[tauri::command]
#[specta::specta]
pub async fn move_their_agents_md_aside(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    workspace: String,
    repo: String,
    piece: Option<String>,
) -> Result<String, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    crate::off_the_window("moving AGENTS.md aside", move || {
        purlis_core::guest::move_agents_md_aside(&root, branch(&workspace, &repo, &piece))
    })
    .await
}

// The offered list the path is checked against is read by the bounded reader's child (#1189),
// so this process starts no git.
fn launch_of(
    reader: &Reader,
    plane: &Path,
    branch: Branch<'_>,
    path: &str,
    line: u32,
    editor: YourEditor,
    var: &dyn Fn(&str) -> Option<String>,
) -> Result<Checked, String> {
    if branch.is_workspace_files() {
        return in_workspace_files(plane, branch.ws, |files| {
            files.in_your_editor(path, line, editor.into(), var)
        })
        .map_err(|why| why.to_string());
    }
    files::root(reader, plane, branch)
        .and_then(|root| root.in_your_editor(reader, path, line, editor.into(), var))
        .map_err(|refused| refused.to_string())
}

/// One file or folder of a branch, its path put on the clipboard: relative to the branch's
/// folder, or absolute (FM-10). Refused, in the core's sentence, for a path that leaves the
/// branch, a link or git's own folder.
// The path is placed by `purlis_core::files::place` and the absolute one is the core's, never
// one the window joined; it goes to the clipboard here and never back to the window. Placed on
// a blocking thread, never the one that draws (#1007). Not a doc comment, because the generated
// bindings carry those.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
#[specta::specta]
pub async fn copy_branch_path(
    planes: tauri::State<'_, Planes>,
    clipboard: tauri::State<'_, crate::vaults::SystemClipboard>,
    plane: PlaneId,
    workspace: String,
    repo: String,
    piece: Option<String>,
    path: String,
    absolute: bool,
) -> Result<(), String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    let text = crate::off_the_window("placing the path", move || {
        path_text(
            &crate::reader(),
            &root,
            branch(&workspace, &repo, &piece),
            &path,
            absolute,
        )
    })
    .await?;
    clipboard.put_text(&text)
}

/// One file or folder of a branch, shown in the operating system's file manager: Finder,
/// Files or File Explorer (FM-10). Refused as Copy path is refused.
// The opener plugin's reveal, called here with the path the core placed: the window names a
// branch and a path inside it, never a directory. Placed on a blocking thread, never the one
// that draws (#1007). Not a doc comment, because the generated bindings carry those.
#[tauri::command]
#[specta::specta]
pub async fn reveal_branch_path(
    app: tauri::AppHandle,
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    workspace: String,
    repo: String,
    piece: Option<String>,
    path: String,
) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt as _;
    let root = planes.held(&plane)?.root().to_path_buf();
    let asked = path.clone();
    let placed = crate::off_the_window("placing the path", move || {
        placed_of(
            &crate::reader(),
            &root,
            branch(&workspace, &repo, &piece),
            &asked,
        )
    })
    .await?;
    app.opener()
        .reveal_item_in_dir(&placed.absolute)
        .map_err(|e| format!("the system did not show '{path}': {e}"))
}

/// The file or folder `path` names, or with `""` the branch's own folder (#1143), placed by the
/// core: the branch's row and its *Files* row copy and reveal the folder itself. The branch's
/// folder is found by `reader`'s child (#1189).
pub(crate) fn placed_of(
    reader: &Reader,
    plane: &Path,
    branch: Branch<'_>,
    path: &str,
) -> Result<files::Placed, String> {
    if branch.is_workspace_files() {
        return in_workspace_files(plane, branch.ws, |files| files.place(path))
            .map_err(|why| why.to_string());
    }
    let root = files::root(reader, plane, branch).map_err(|refused| refused.to_string())?;
    if path.is_empty() {
        root.place_branch_folder()
    } else {
        root.place(path)
    }
    .map_err(|refused| refused.to_string())
}

/// What Copy path puts on the clipboard: the path in the branch, or the absolute path the core
/// placed. An absolute path is said as a person writes it, without Windows' `\\?\` prefix.
fn path_text(
    reader: &Reader,
    plane: &Path,
    branch: Branch<'_>,
    path: &str,
    absolute: bool,
) -> Result<String, String> {
    if !absolute {
        if branch.is_workspace_files() {
            return in_workspace_files(plane, branch.ws, |files| files.named(path))
                .map_err(|why| why.to_string());
        }
        // A link's own path, followed nowhere: what its row copies (`files::named`).
        return files::named_by(reader, plane, branch, path).map_err(|refused| refused.to_string());
    }
    let placed = placed_of(reader, plane, branch, path)?;
    Ok(dunce::simplified(&placed.absolute).display().to_string())
}

/// The folder of a branch a shell tab starts in (FM-10), resolved as the tree resolves it: inside
/// the branch, reached through no link, not git's. `""` is the branch's own folder, placed as
/// Copy path and Reveal place it (#1143): refused when that folder is itself a link.
pub(crate) fn shell_folder(
    plane: &Path,
    branch: Branch<'_>,
    folder: &str,
) -> Result<std::path::PathBuf, String> {
    shell_folder_by(&crate::reader(), plane, branch, folder)
}

/// [`shell_folder`], the branch's folder found by `reader`'s child (#1189).
fn shell_folder_by(
    reader: &Reader,
    plane: &Path,
    branch: Branch<'_>,
    folder: &str,
) -> Result<std::path::PathBuf, String> {
    if folder.is_empty() {
        return placed_of(reader, plane, branch, folder).map(|placed| placed.absolute);
    }
    if branch.is_workspace_files() {
        return in_workspace_files(plane, branch.ws, |files| files.listed_folder(folder))
            .map_err(|why| why.to_string());
    }
    files::root(reader, plane, branch)
        .and_then(|root| root.folder(folder))
        .map_err(|refused| refused.to_string())
}

// The offered list the path is checked against is read by the bounded reader's child (#1189),
// so this process starts no git.
fn file_of(
    reader: &Reader,
    plane: &Path,
    branch: Branch<'_>,
    path: &str,
) -> Result<PieceFile, String> {
    files::root(reader, plane, branch)
        .and_then(|root| root.open(reader, path))
        .map(PieceFile::from)
        .map_err(|refused| refused.to_string())
}

#[cfg(test)]
mod tests;

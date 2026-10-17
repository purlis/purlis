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
use purlis_core::files::{self, Branch, Entry, Kind, Mark, Opened, Reader};
use purlis_core::youreditor::{self, Editor, Launch};

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
}

impl From<Entry> for FolderEntry {
    fn from(entry: Entry) -> Self {
        Self {
            name: entry.name,
            kind: match entry.kind {
                Kind::Folder => EntryKind::Folder,
                Kind::File => EntryKind::File,
                Kind::Link => EntryKind::Link,
            },
            ignored: entry.ignored,
            refused: entry.refused,
            workspace: entry.workspace,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GitHunk {
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
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

fn what_changed_of(plane: &Path, branch: Branch<'_>, path: &str) -> Result<WhatChanged, String> {
    let shown =
        files::what_changed(&crate::reader(), plane, branch, path).map_err(|e| e.to_string())?;
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
    Ok(WhatChanged {
        mark: shown.change.mark.into(),
        from: shown.change.from,
        uncommitted: shown.change.uncommitted,
        base: shown.base,
        diff,
    })
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
// checks it, and the editor is handed the resolved absolute path: as a URL to the operating
// system's opener, or as one argument of the program `$VISUAL`/`$EDITOR` names, never through
// a shell: a terminal editor in a shell tab of its own (`editortab`, #1042), which is why the
// command is the window's alone (`WINDOW_ONLY`). Not a doc comment, because the generated
// bindings carry those.
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
    let launch = launch_of(
        &crate::reader(),
        held.root(),
        branch(&workspace, &repo, &piece),
        &path,
        line,
        editor,
        &var,
    )?;
    match launch {
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
    let launch = purlis_core::guest::their_agents_md_in_your_editor(
        held.root(),
        branch(&workspace, &repo, &piece),
        editor.into(),
        &var,
    )?;
    match launch {
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
) -> Result<Launch, String> {
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
fn placed_of(
    reader: &Reader,
    plane: &Path,
    branch: Branch<'_>,
    path: &str,
) -> Result<files::Placed, String> {
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
    // A size crosses as a `u32`: specta refuses a `u64` for TypeScript, which has no integer
    // that wide. A binary or oversized file past 4 GiB is said as 4 GiB.
    let bytes = |n: u64| u32::try_from(n).unwrap_or(u32::MAX);
    files::root(reader, plane, branch)
        .and_then(|root| root.open(reader, path))
        .map(|opened| match opened {
            Opened::Text { text } => PieceFile::Text { text },
            Opened::Image { mime, data } => PieceFile::Image {
                mime: mime.to_string(),
                base64: base64::engine::general_purpose::STANDARD.encode(data),
            },
            Opened::HugeImage {
                mime,
                width,
                height,
            } => PieceFile::HugeImage {
                mime: mime.to_string(),
                width,
                height,
            },
            Opened::Binary { bytes: n } => PieceFile::Binary { bytes: bytes(n) },
            Opened::TooLarge { bytes: n } => PieceFile::TooLarge { bytes: bytes(n) },
        })
        .map_err(|refused| refused.to_string())
}

#[cfg(test)]
mod tests;

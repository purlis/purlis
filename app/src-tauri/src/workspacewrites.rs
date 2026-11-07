// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The person's own workspace files, written from Explorer** (ADR 0081, amended 2026-11-05;
//! #5376): new file, new folder, rename, delete to the system's Trash, and import.
//!
//! Thin, as `piecefiles.rs` is: what may be written, and where, is
//! `purlis_core::files::workspace`'s answer, and a refusal crosses as the core's sentence. Each
//! command names a workspace and a path relative to its folder, never an absolute path; an
//! import's files are picked here, in the system's own picker, so no path from outside the
//! workspace ever crosses the window's seam.
//!
//! **Every command is the window's alone** (`purlis_session_protocol::ui::WINDOW_ONLY`): only
//! the person's own press writes their files. No link and no chat runs any of them.

use std::path::{Path, PathBuf};

use purlis_core::files::workspace;
use tauri_plugin_dialog::DialogExt;

use crate::piecefiles::extension_folders;
use crate::planes::{PlaneId, Planes};

/// A new empty file `name` in `folder` of a workspace's own files (`""` for its top): its path.
/// Refused where the files are not the person's, and for a name already taken.
// The window's alone (`WINDOW_ONLY`). Not a doc comment, because the generated bindings carry
// those.
#[tauri::command]
#[specta::specta]
pub async fn workspace_files_new_file(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    workspace: String,
    folder: String,
    name: String,
) -> Result<String, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    crate::off_the_window("making the file", move || {
        written(&root, &workspace, |files| {
            files.new_file(&folder, &name, &extension_folders())
        })
        .map_err(|why| why.to_string())
    })
    .await
}

/// A new empty folder `name` in `folder` of a workspace's own files: its path.
#[tauri::command]
#[specta::specta]
pub async fn workspace_files_new_folder(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    workspace: String,
    folder: String,
    name: String,
) -> Result<String, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    crate::off_the_window("making the folder", move || {
        written(&root, &workspace, |files| {
            files.new_folder(&folder, &name, &extension_folders())
        })
        .map_err(|why| why.to_string())
    })
    .await
}

/// A file or folder of a workspace's own files renamed to `name`, in its folder: its new path.
/// Never over anything that has the name already.
#[tauri::command]
#[specta::specta]
pub async fn workspace_files_rename(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    workspace: String,
    path: String,
    name: String,
) -> Result<String, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    crate::off_the_window("renaming", move || {
        written(&root, &workspace, |files| {
            files.rename(&path, &name, &extension_folders())
        })
        .map_err(|why| why.to_string())
    })
    .await
}

/// A file or folder of a workspace's own files sent to the system's Trash. Refused, and left
/// where it is, when the Trash will not take it: never deleted for good.
#[tauri::command]
#[specta::specta]
pub async fn workspace_files_trash(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    workspace: String,
    path: String,
) -> Result<(), String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    crate::off_the_window("moving it to the Trash", move || {
        written(&root, &workspace, |files| {
            files.trash(&path, &extension_folders(), &workspace::SystemTrash)
        })
        .map_err(|why| why.to_string())
    })
    .await
}

/// Import files…: the system's file picker, then each file picked copied into `folder` of a
/// workspace's own files, a name already taken given a ` (2)` suffix. The paths they were given;
/// none when the picker was cancelled.
#[tauri::command]
#[specta::specta]
pub async fn workspace_files_import(
    app: tauri::AppHandle,
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    workspace: String,
    folder: String,
) -> Result<Vec<String>, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    let picked = pick_files(&app).await?;
    if picked.is_empty() {
        return Ok(Vec::new());
    }
    crate::off_the_window("copying the files in", move || {
        written(&root, &workspace, |files| {
            files.import(&folder, &picked, &extension_folders())
        })
        .map_err(|why| why.to_string())
    })
    .await
}

/// The system's file picker, several files at once: the files picked, or none for a cancel.
async fn pick_files(app: &tauri::AppHandle) -> Result<Vec<PathBuf>, String> {
    let (chose, chosen) = std::sync::mpsc::channel();
    app.dialog().file().pick_files(move |picked| {
        // The window may have gone while the dialog was up; then there is nobody to tell.
        let _ = chose.send(picked);
    });
    tauri::async_runtime::spawn_blocking(move || chosen.recv().ok().flatten())
        .await
        .map(|picked| {
            picked
                .unwrap_or_default()
                .into_iter()
                .filter_map(|path| path.into_path().ok())
                .collect()
        })
        .map_err(|err| format!("the file picker did not finish: {err}"))
}

/// What `write` answers of workspace `ws`'s own files. Its error is named here, so the window's
/// copy check (`copy.rust.test.ts`) reads the core's sentences each command passes on.
fn written<T>(
    plane: &Path,
    ws: &str,
    write: impl FnOnce(&workspace::Files) -> Result<T, workspace::NotWritten>,
) -> Result<T, workspace::NotWritten> {
    write(&workspace::Files::of(plane, ws)?)
}

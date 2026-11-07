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
//! **A drop from the file manager** (#5378) imports as Import files… does, into the folder the
//! window says it landed on. Its files are the ones the system dropped on that webview, held
//! here as the drop is heard ([`Dropped`]), never paths the window sends: the window says only
//! where, so the seam still carries no path from outside the workspace.
//!
//! **Every command is the window's alone** (`purlis_session_protocol::ui::WINDOW_ONLY`): only
//! the person's own press writes their files. No link and no chat runs any of them.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

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

/// **A drop from the file manager**, onto the folder `folder` of a workspace's own files
/// (#5378): the files the system dropped on this webview last, copied in as Import files… copies
/// them, each imported once. The paths they were given; none when nothing was dropped.
#[tauri::command]
#[specta::specta]
pub async fn workspace_files_import_dropped(
    webview: tauri::Webview,
    dropped: tauri::State<'_, Dropped>,
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    workspace: String,
    folder: String,
) -> Result<Vec<String>, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    let files = dropped.take(webview.label());
    if files.is_empty() {
        return Ok(Vec::new());
    }
    crate::off_the_window("copying the files in", move || {
        written(&root, &workspace, |held| {
            held.import(&folder, &files, &extension_folders())
        })
        .map_err(|why| why.to_string())
    })
    .await
}

/// **The files the system last dropped on each webview** (#5378), by its label, until the window
/// imports them or another drag begins. Heard from the system's own drag events, before the
/// window hears of the drop, so the files are here when its import is asked.
#[derive(Default)]
pub struct Dropped(Mutex<HashMap<String, Vec<PathBuf>>>);

impl Dropped {
    /// What the system says of a drag over `webview`: a drop is held; a new drag forgets the
    /// last drop. A drag that leaves keeps it, since a drop may be told after its leave.
    pub fn heard(&self, webview: &str, event: &tauri::DragDropEvent) {
        let mut held = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        match event {
            tauri::DragDropEvent::Drop { paths, .. } => {
                held.insert(webview.to_string(), paths.clone());
            }
            tauri::DragDropEvent::Enter { .. } => {
                held.remove(webview);
            }
            _ => {}
        }
    }

    /// The files dropped on `webview` last, given up: none when they were already taken.
    fn take(&self, webview: &str) -> Vec<PathBuf> {
        let mut held = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        held.remove(webview).unwrap_or_default()
    }
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

#[cfg(test)]
mod tests {
    use super::Dropped;
    use std::path::PathBuf;
    use tauri::{DragDropEvent, PhysicalPosition};

    fn drop_of(paths: &[&str]) -> DragDropEvent {
        DragDropEvent::Drop {
            paths: paths.iter().map(PathBuf::from).collect(),
            position: PhysicalPosition::new(10.0, 20.0),
        }
    }

    #[test]
    fn a_drop_is_imported_once() {
        let dropped = Dropped::default();
        dropped.heard("main", &drop_of(&["/Users/me/a.txt", "/Users/me/b.txt"]));

        assert_eq!(
            dropped.take("main"),
            [
                PathBuf::from("/Users/me/a.txt"),
                PathBuf::from("/Users/me/b.txt")
            ]
        );
        assert!(dropped.take("main").is_empty());
    }

    #[test]
    fn a_new_drag_forgets_the_last_drop_and_a_leave_does_not() {
        let dropped = Dropped::default();
        dropped.heard("main", &drop_of(&["/Users/me/a.txt"]));
        dropped.heard("main", &DragDropEvent::Leave);
        assert_eq!(dropped.take("main"), [PathBuf::from("/Users/me/a.txt")]);

        dropped.heard("main", &drop_of(&["/Users/me/a.txt"]));
        dropped.heard(
            "main",
            &DragDropEvent::Enter {
                paths: vec![PathBuf::from("/Users/me/c.txt")],
                position: PhysicalPosition::new(0.0, 0.0),
            },
        );
        assert!(dropped.take("main").is_empty());
    }

    #[test]
    fn each_webview_imports_only_what_was_dropped_on_it() {
        let dropped = Dropped::default();
        dropped.heard("main", &drop_of(&["/Users/me/a.txt"]));

        assert!(dropped.take("split-1").is_empty());
        assert_eq!(dropped.take("main"), [PathBuf::from("/Users/me/a.txt")]);
    }
}

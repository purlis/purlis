// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A skill's folder, for its page's Files tab** (#5183): its files nested, one of them opened,
//! and the folder opened in the system's file manager.
//!
//! Thin, as `piecefiles.rs` is. The window names the folder by the studio row's own `folder`,
//! and the core answers only a folder a skill the studio lists for the same chat is in
//! (`purlis_core::skills::folder::listed_folder`), so a built-in skill's folder in the app and a
//! third-party one's in a harness's folder are reached, and nothing else. Reading inside it is
//! `purlis_core::files::folder`'s. Each is the window's alone
//! (`purlis_session_protocol::ui::WINDOW_ONLY`): the folder may be outside the project.

use std::path::PathBuf;

use purlis_core::files::folder;

use crate::piecefiles::{EntryKind, PieceFile};
use crate::planes::{PlaneId, Planes};

/// One entry of a skill's folder.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct SkillFolderEntry {
    /// Its path in the folder, `/` between its parts.
    pub path: String,
    pub kind: EntryKind,
    /// Why it does not open, in the core's sentence; `null` for a file that opens and a folder.
    pub refused: Option<String>,
}

/// A skill's folder, walked whole: each folder's entries straight after it, folders first.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct SkillFolder {
    /// The first 2,000.
    pub entries: Vec<SkillFolderEntry>,
    /// How many more were listed past them.
    pub more: u32,
}

/// The folder `folder` names, where a skill the studio lists for the same chat is in.
fn listed(
    root: &std::path::Path,
    workspace: Option<&str>,
    persona: Option<&str>,
    folder: &str,
) -> Result<PathBuf, String> {
    purlis_core::skills::folder::listed_folder(
        crate::skillchange::built_in().as_deref(),
        root,
        workspace,
        persona,
        &purlis_core::harness_plugin::Env::of(&[]),
        folder,
    )
}

/// Every file and folder in the folder of a skill the studio lists, nested.
// Each command here lists the studio again to find the folder, so on a blocking thread.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_files(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    workspace: Option<String>,
    persona: Option<String>,
    folder: String,
) -> Result<SkillFolder, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("reading the skill's folder", move || {
        let at = listed(
            held.root(),
            workspace.as_deref(),
            persona.as_deref(),
            &folder,
        )?;
        let walked = folder::walk(&at)?;
        Ok(SkillFolder {
            entries: walked
                .entries
                .into_iter()
                .map(|one| SkillFolderEntry {
                    path: one.path,
                    kind: one.kind.into(),
                    refused: one.refused,
                })
                .collect(),
            more: u32::try_from(walked.more).unwrap_or(u32::MAX),
        })
    })
    .await
}

/// One file of the folder of a skill the studio lists, by its path in the folder, as the light
/// editor draws it.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_file(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    workspace: Option<String>,
    persona: Option<String>,
    folder: String,
    path: String,
) -> Result<PieceFile, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("reading the file", move || {
        let at = listed(
            held.root(),
            workspace.as_deref(),
            persona.as_deref(),
            &folder,
        )?;
        folder::open(&at, &path).map(PieceFile::from)
    })
    .await
}

/// The folder of a skill the studio lists, opened in the system's file manager: Finder, Files
/// or File Explorer.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_folder_open(
    app: tauri::AppHandle,
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    workspace: Option<String>,
    persona: Option<String>,
    folder: String,
) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt as _;
    let held = planes.held(&plane)?;
    let at = crate::off_the_window("finding the skill's folder", move || {
        listed(
            held.root(),
            workspace.as_deref(),
            persona.as_deref(),
            &folder,
        )
    })
    .await?;
    app.opener()
        .open_path(at.to_string_lossy(), None::<&str>)
        .map_err(|e| {
            format!(
                "purlis could not open the skill's folder, because the system refused it ({e}). \
                 Check that this computer has an app set to open folders, then try again"
            )
        })
}

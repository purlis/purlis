// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The project's harness declarations, as Settings shows them** (#1389, spec #1221's
//! collection shapes): each `harnesses/<name>.toml` read-only, with Open file, and each one
//! purlis would not read with its reason. Editing them is the file's, in the person's editor.
//!
//! What a declaration says is a committed file's words: drawn as text, never followed. Open
//! file opens only a file this listing named, inside the project's `harnesses/` folder.

use std::path::{Path, PathBuf};

use purlis_core::harness_declaration::{self, Declarations};
use purlis_core::youreditor::{self, Launch};

use crate::piecefiles::YourEditor;
use crate::planes::{PlaneId, Planes};

/// One harness the project declares.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct DeclaredHarness {
    /// The word a profile's `kind` names.
    pub name: String,
    /// What the declaration calls it.
    pub title: String,
    /// The program it starts, a bare name found on `PATH`.
    pub program: String,
    /// `harnesses/<name>.toml`: what Open file is sent.
    pub file: String,
    /// Whether it says how to start its ACP agent (level 3). Level 1, the terminal, it always
    /// has; level 2 is purlis's own and never a project's.
    pub acp: bool,
}

/// A declaration file purlis would not read.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct RefusedHarness {
    /// `harnesses/<file>`, or the folder itself: what Open file is sent, where it is a file.
    pub file: String,
    /// Why, in one sentence.
    pub reason: String,
}

/// **The project's harness declarations** (#1389): its own, in file-name order, and each
/// refused with its reason. The built-ins purlis ships are not the project's, so not listed.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct ProjectHarnesses {
    pub declared: Vec<DeclaredHarness>,
    pub refused: Vec<RefusedHarness>,
}

/// [`ProjectHarnesses`] of what the core read.
fn listed(read: &Declarations) -> ProjectHarnesses {
    ProjectHarnesses {
        declared: read
            .projects()
            .map(|one| DeclaredHarness {
                name: one.name.clone(),
                title: one.title.clone(),
                program: one.program.clone(),
                file: one.file.clone(),
                acp: one.levels.acp.is_some(),
            })
            .collect(),
        // The project's own: what this machine's `charter.local.toml` declares (#968) is
        // listed, and refused, with the profiles there.
        refused: read
            .refused
            .iter()
            .filter(|one| one.file.starts_with(harness_declaration::DIR))
            .map(|one| RefusedHarness {
                file: one.file.clone(),
                reason: one.reason.clone(),
            })
            .collect(),
    }
}

/// The project's harness declarations, read-only, for Settings (#1389).
#[tauri::command]
#[specta::specta]
pub fn project_harness_declarations(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
) -> Result<ProjectHarnesses, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    Ok(listed(&harness_declaration::read(&root)))
}

/// The file `file` names in the project at `root`, when it is one [`listed`] names and it is a
/// file inside the project's `harnesses/` folder, through no link that leaves it.
fn declaration_file(root: &Path, file: &str) -> Result<PathBuf, String> {
    let read = listed(&harness_declaration::read(root));
    let named = read.declared.iter().any(|one| one.file == file)
        || read.refused.iter().any(|one| one.file == file);
    let refused = || {
        format!("{file} is not one of this project's harness declarations, so nothing was opened.")
    };
    if !named {
        return Err(refused());
    }
    // The folder is the project's own: a link in its place could point anywhere, and the
    // core's reader refuses it too.
    let folder = root.join(harness_declaration::DIR);
    if !std::fs::symlink_metadata(&folder).is_ok_and(|meta| meta.is_dir()) {
        return Err(refused());
    }
    let folder = std::fs::canonicalize(&folder).map_err(|_| refused())?;
    let resolved = std::fs::canonicalize(root.join(file)).map_err(|_| refused())?;
    // A file directly in it, wherever a link on the way resolves.
    if resolved.parent() != Some(folder.as_path()) || !resolved.is_file() {
        return Err(refused());
    }
    Ok(resolved)
}

/// What starts your editor on the declaration `file` names in the project at `root`.
fn launch_of(root: &Path, file: &str, editor: YourEditor) -> Result<Launch, String> {
    let path = declaration_file(root, file)?;
    youreditor::launch(editor.into(), &path, 1, &purlis_core::envvar::var)
        .map_err(|refused| refused.to_string())
}

/// **Open file** on a harness declaration in Settings (#1389): the file in your editor.
// A terminal editor opens in a shell tab at the project root, standing in `harnesses/`, as a
// branch's file does (#1042's follow-up).
// Placed by the name the listing gave it, never a path the window made up, and handed to the
// editor as `open_in_your_editor` hands one. Not a doc comment, because the generated bindings
// carry those.
#[tauri::command]
#[specta::specta]
pub async fn open_harness_declaration(
    app: tauri::AppHandle,
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    file: String,
    editor: YourEditor,
) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt as _;
    let held = planes.held(&plane)?;
    let root = held.root().to_path_buf();
    let launch = launch_of(&root, &file, editor)?;
    match launch {
        Launch::Url(url) => app
            .opener()
            .open_url(&url, None::<&str>)
            .map_err(|e| format!("the system did not open {url}: {e}")),
        Launch::Program { program, args } => {
            // The folder the file was placed in, resolved as the file was.
            let cwd = || {
                declaration_file(&root, &file)?
                    .parent()
                    .map(Path::to_path_buf)
                    .ok_or_else(|| format!("{file} is in no folder, so nothing was opened."))
            };
            crate::editortab::started(
                &app,
                &held,
                &plane,
                crate::editortab::PROJECT_ROOT,
                cwd,
                program,
                args,
            )
            .await
        }
    }
}

#[cfg(test)]
#[path = "harnessdecls_tests.rs"]
mod tests;

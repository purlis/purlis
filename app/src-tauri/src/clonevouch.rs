// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A clone made outside purlis, and the person's Trust this clone**: the window's half of
//! `purlis_core::forge::gitcredential::vouch`.
//!
//! [`clones_to_vouch`] lists every clone of the project's workspaces that purlis did not clone
//! and the person has not trusted, with the address its remote names, which the project draws as
//! one Notice per clone, and which the core keeps as the project's one ask, Trust these clones
//! (R-336). [`vouch_clone`] is that Notice's fix: it records the clone at the address the
//! Notice showed, while its remote still names exactly that one. It is also the Trust this
//! clone of the ask a refused GitHub App token raises (R-330), and clears that ask.
//!
//! Both are the window's alone (`WINDOW_ONLY`): trusting a clone is what lets saves push it and
//! chats in its workspace use the person's GitHub access with it, so no link and no chat may
//! press it. A chat's refused mint raises the ask (Q181), and only the person presses it.

use purlis_core::forge::route::HostScope;

use crate::planes::{PlaneId, Planes};

/// A clone of the project's workspaces that purlis did not clone and the person has not trusted.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct CloneToVouch {
    pub workspace: String,
    /// Its folder, relative to the project.
    pub folder: String,
    /// The one `<host>/<owner>/<repo>` its remote names now; `null` when it names more than one,
    /// which is never trusted until origin has one address.
    pub place: Option<String>,
    /// Every address its remote names now, each once.
    pub places: Vec<String>,
}

/// Every clone of the project's workspaces made outside purlis and not trusted yet.
///
/// On a blocking thread: it asks git a question or two per clone, and reaches no network. None
/// in a test build: the config home a test's window would read is outside its fence.
#[tauri::command]
#[specta::specta]
pub async fn clones_to_vouch(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
) -> Result<Vec<CloneToVouch>, String> {
    let held = planes.held(&plane)?;
    let root = held.root().to_path_buf();
    if purlis_core::fence::FENCED {
        return Ok(Vec::new());
    }
    let writes = live_writes(&held);
    let config_root = purlis_core::machine::config_root()
        .ok_or_else(|| "purlis found no home directory its record of clones is in".to_owned())?;
    tauri::async_runtime::spawn_blocking(move || {
        // The same read raises or settles the project's one Trust these clones (R-336), which
        // leaves out what the live chats may write (Q257).
        purlis_core::windowonly::look_at_clones(&root, &config_root, &writes)
            .into_iter()
            .map(|clone| CloneToVouch {
                workspace: clone.workspace,
                folder: clone.folder,
                place: clone.place,
                places: clone.places,
            })
            .collect()
    })
    .await
    .map_err(|err| format!("reading the workspaces' clones did not finish: {err}"))
}

/// **The folders the project's live chats may write** (Q257, Q249): each sandboxed chat's grants
/// and cache grants as it started, as its GitHub App mint holds them. A clone inside one is never
/// in Trust these clones.
fn live_writes(held: &crate::planes::Held) -> Vec<std::path::PathBuf> {
    held.chats()
        .open_now()
        .into_iter()
        .filter_map(|open| held.chats().confines_of(open.session))
        .flat_map(|confines| {
            let mut writes = confines.writable;
            if let Some(caches) = confines.widened.caches {
                writes.extend(caches.trees.into_iter().chain(caches.bare));
            }
            writes
        })
        .collect()
}

/// The person's Trust this clone: records the clone at `folder` (relative to the project) at
/// `place`, the address the Notice showed. Answers the address trusted, or why nothing was.
#[tauri::command]
#[specta::specta]
pub async fn vouch_clone(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    folder: String,
    place: String,
) -> Result<String, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    tauri::async_runtime::spawn_blocking(move || {
        let scope = HostScope::claim()
            .ok_or_else(|| "A clone is trusted from the window alone.".to_owned())?;
        let config_root = purlis_core::machine::config_root()
            .ok_or_else(|| "purlis found no home directory to record the clone in".to_owned())?;
        // Any folder that is not one of the project's workspace clones is refused by the core.
        // The same press clears every chat's refusal that waited on it, from its ask or its
        // Notice (R-328).
        purlis_core::windowonly::trust_this_clone(&scope, &config_root, &root, &folder, &place)
    })
    .await
    .map_err(|err| format!("trusting the clone did not finish: {err}"))?
}

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
    /// When it was made, in seconds since 1970, where its disk keeps a creation time (R-337).
    pub made: Option<u32>,
}

impl CloneToVouch {
    /// **The clone `clone`, as the window lists it.**
    pub fn of(clone: purlis_core::forge::gitcredential::vouch::Unvouched) -> Self {
        Self {
            workspace: clone.workspace,
            folder: clone.folder,
            place: clone.place,
            places: clone.places,
            made: clone.made.map(|at| u32::try_from(at).unwrap_or(u32::MAX)),
        }
    }
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
            .map(CloneToVouch::of)
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
        let trusted = purlis_core::windowonly::trust_this_clone(
            &scope,
            &config_root,
            &root,
            &folder,
            &place,
        )?;
        // ADR 0128 D4: the repo's number and account are recorded with the trust, best effort.
        crate::onelogin::confirm_after_trust(&root.join(&folder));
        Ok(trusted)
    })
    .await
    .map_err(|err| format!("trusting the clone did not finish: {err}"))?
}

/// **The person's Use one address** (A1, A5; Q4): sets the push address of the clone at `folder`
/// (relative to the project) to `to`, the address its row showed, and only while its origin still
/// fetches from `fetch` and pushes to `push`, as the row showed. Answers the address set, or why
/// nothing was changed. It trusts nothing: the chat that waited is offered Trust this clone next,
/// a second press.
///
/// Window only (`WINDOW_ONLY`): the push address decides where the person's sign-in is sent.
#[tauri::command]
#[specta::specta]
pub async fn use_one_address(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    folder: String,
    fetch: String,
    push: String,
    to: String,
) -> Result<String, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    tauri::async_runtime::spawn_blocking(move || {
        let scope = HostScope::claim()
            .ok_or_else(|| "An address is set from the window alone.".to_owned())?;
        let config_root = purlis_core::machine::config_root().ok_or_else(|| {
            "purlis found no home directory its record of clones is in".to_owned()
        })?;
        purlis_core::windowonly::use_one_address(
            &scope,
            &config_root,
            &root,
            &purlis_core::windowonly::Control::OneAddress {
                folder,
                fetch,
                push,
                to,
            },
        )
    })
    .await
    .map_err(|err| format!("setting the address did not finish: {err}"))?
}

/// **The person's Clone it** (A2): clones the inventory's repo `name` into `workspace` with their
/// own sign-in, as the window's own clone, which records it, and only while that row's address is
/// still `repo` (`<host>/<owner>/<repo>`), the one the ask showed: a row that now has another
/// address is refused and nothing is cloned. Answers what the clone said, or why it did not land.
/// The ask is cleared for every chat that waited on it once the clone is made.
///
/// Window only (`WINDOW_ONLY`): it releases the person's sign-in.
#[tauri::command]
#[specta::specta]
pub async fn clone_with_sign_in(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    workspace: String,
    name: String,
    repo: String,
) -> Result<String, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    tauri::async_runtime::spawn_blocking(move || {
        HostScope::claim()
            .ok_or_else(|| "A clone uses your sign-in from the window alone.".to_owned())?;
        let control = purlis_core::windowonly::Control::CloneWithSignIn {
            workspace: workspace.clone(),
            name: name.clone(),
            repo: repo.clone(),
        };
        // The row the ask named, by the name it carried, must still be the address it showed.
        purlis_core::windowonly::clone_it_matches(&root, &name, &repo)?;
        crate::workspaces::clone_for_the_person(&root, &workspace, &name)?;
        purlis_core::windowonly::pressed_by_the_window(&root, &control);
        Ok(format!("{repo} was cloned into {workspace}"))
    })
    .await
    .map_err(|err| format!("the clone did not finish: {err}"))?
}

/// **The person's Record this repo** (A3): records the repo of the clone `clone` of `workspace`
/// in the project's inventory, as `purlis discover` does for that one clone, and only while its
/// origin still names `place`, the address the row showed. Answers the address recorded, or why
/// nothing was. The ask is cleared for every chat that waited on it.
///
/// Window only (`WINDOW_ONLY`): the record is what decides which repo a chat may read.
#[tauri::command]
#[specta::specta]
pub async fn record_repo(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    workspace: String,
    clone: String,
    place: String,
) -> Result<String, String> {
    let held = planes.held(&plane)?;
    let root = held.root().to_path_buf();
    // The folders the live chats may write: a clone whose git directory is in one is never
    // recorded, as the ask is never raised for it.
    let writes = live_writes(&held);
    tauri::async_runtime::spawn_blocking(move || {
        HostScope::claim().ok_or_else(|| "A repo is recorded from the window alone.".to_owned())?;
        purlis_core::windowonly::record_this_repo(
            &root,
            &purlis_core::windowonly::Control::RecordRepo {
                workspace,
                clone,
                place,
            },
            &writes,
        )
    })
    .await
    .map_err(|err| format!("recording the repo did not finish: {err}"))?
}

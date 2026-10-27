// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The old `[harness_plugins]` keys, said once** (#3467, the operator's rulings of
//! 2026-10-26/27 on spec #2762): purlis no longer turns another harness's plugins on or off per
//! project, so a project that still holds the keys starts as any other, with them ignored and
//! never rewritten behind the person's back
//! ([`purlis_core::harness_plugin::old`]).
//!
//! - **One Notice** ([`old_harness_plugins`]) says which files hold them, until the person
//!   answers it once: **Remove** or **Dismiss**. The answer is kept in this machine's state for
//!   the project, where no chat writes, so the Notice never comes back for it.
//! - **Remove is an edit the person approves** ([`remove_old_harness_plugins`]): each of the
//!   project's two files that holds them gets a proposed change that takes the table out, held
//!   as purlis's own ([`crate::proposals::BY_PURLIS`]) and drawn, shown whole and applied as a
//!   chat's proposal is. Nothing is written until the person presses Apply on it.

use std::path::{Path, PathBuf};

use purlis_core::harness_plugin::old;
use purlis_core::proposed::{Change, base_of};

use crate::planes::{Held, PlaneId, Planes};
use crate::proposals::{BY_PURLIS, Proposals};

/// The Notice, as the window draws it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct OldHarnessPlugins {
    /// The sentence: which files hold the keys, that they are ignored, and what Remove reaches.
    pub said: String,
    /// The files that hold them, relative to the project.
    pub files: Vec<String>,
    /// Whether Remove has a change to propose: one of the project's own two files holds them.
    pub removable: bool,
}

/// Where this machine keeps that the project's Notice was answered.
fn answered_at(root: &Path) -> PathBuf {
    purlis_core::names::state(root)
        .join("app")
        .join("old-harness-plugins-answered")
}

/// The Notice for the project at `root`, or none: no file holds the keys, or it was answered.
pub fn notice_of(root: &Path) -> Option<OldHarnessPlugins> {
    if answered_at(root).exists() {
        return None;
    }
    let found = old::found(root);
    if found.is_empty() {
        return None;
    }
    Some(OldHarnessPlugins {
        said: old::said(&found),
        files: found.iter().map(|one| one.file.clone()).collect(),
        removable: found.iter().any(|one| one.which.is_some()),
    })
}

/// Keeps that the project's Notice was answered, so it is said once.
fn answer(root: &Path) -> Result<(), String> {
    let at = answered_at(root);
    let keep = || -> std::io::Result<()> {
        if let Some(folder) = at.parent() {
            std::fs::create_dir_all(folder)?;
        }
        std::fs::write(&at, b"")
    };
    keep().map_err(|e| format!("purlis could not keep your answer ({e})"))
}

/// One change Remove held: its proposal's id, and the file it changes.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct HeldRemoval {
    pub id: u32,
    pub file: String,
}

/// **Remove**'s work in the project at `root`: a proposed change per file of its own two that
/// holds the keys, held in `proposals` at `now` as purlis's, in file order. The Notice is
/// answered only once a change is held, so it never goes with nothing to apply.
pub fn removal_held(
    root: &Path,
    proposals: &Proposals,
    now: u64,
) -> Result<Vec<HeldRemoval>, String> {
    let mut held = Vec::new();
    for one in old::found(root)
        .into_iter()
        .filter(|one| one.which.is_some())
    {
        let content = old::removed(&one.text)?;
        let base = base_of(Some(one.text.as_bytes()));
        if let Some(id) = proposals.note(BY_PURLIS, &one.file, base, Change::Whole { content }, now)
        {
            held.push(HeldRemoval { id, file: one.file });
        }
    }
    if held.is_empty() {
        return Err("purlis found no change to propose, so nothing waits for you".to_owned());
    }
    answer(root)?;
    Ok(held)
}

/// **The old keys' Notice**, for the project's window: none where nothing holds them or it was
/// answered.
#[tauri::command]
#[specta::specta]
pub async fn old_harness_plugins(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
) -> Result<Option<OldHarnessPlugins>, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("reading the old harness plugin keys", move || {
        Ok(notice_of(held.root()))
    })
    .await
}

/// **Remove** on the Notice: each file's change held for the person to apply, each with its
/// file. Window-only: only the person proposes purlis's own change.
#[tauri::command]
#[specta::specta]
pub async fn remove_old_harness_plugins(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
) -> Result<Vec<HeldRemoval>, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("proposing the old keys' removal", move || removed(&held)).await
}

/// [`remove_old_harness_plugins`]'s work, for project `held`.
fn removed(held: &Held) -> Result<Vec<HeldRemoval>, String> {
    let ids = removal_held(held.root(), held.proposals(), crate::sandboxing::now_secs())?;
    held.chats().asks_may_have_moved();
    Ok(ids)
}

/// **Dismiss** on the Notice: kept, so it is not said again. Window-only.
#[tauri::command]
#[specta::specta]
pub async fn dismiss_old_harness_plugins(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
) -> Result<(), String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("dismissing the old harness plugin keys", move || {
        answer(held.root())
    })
    .await
}

#[cfg(test)]
mod tests;

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A task's fresh runner kept, since its work is not all here** (#4569, the steward's
//! decision of 2026-11-02).
//!
//! A chat's fresh runner goes only once its project is proved to hold every commit and change it
//! holds ([`purlis_core::codelink::held`]). Where that proof fails, its claim stays, which costs
//! money but keeps the work, and the window says so in a Notice, never silently: what is missing,
//! with **Fetch again**, which asks the runner again, and **Discard**, which lets it go with what
//! it holds. A runner its cluster says is gone already is said too, and stays named until the
//! person forgets it.

use crate::planes::{PlaneId, Planes};

/// The event the window is sent each time a plane's kept fresh runners change.
pub const EVENT: &str = "fresh-runners-kept";

/// One fresh runner kept since its work is not all here.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct FreshKept {
    /// Its chat's stable id, which Fetch again and Discard name it by.
    pub chat: String,
    /// The runner, `<pool>/fresh-<hash>`.
    pub runner: String,
    /// The workspace its work goes back to.
    pub workspace: String,
    /// What is not here, in sentences.
    pub why: Vec<String>,
    /// Whether its cluster says the runner is gone already, with what it held.
    pub gone: bool,
}

/// A plane's kept fresh runners, as they are now.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct FreshRunnersKept {
    pub plane: PlaneId,
    pub kept: Vec<FreshKept>,
}

/// Told a plane's kept fresh runners each time they change.
pub type Tell = std::sync::Arc<dyn Fn(Vec<FreshKept>) + Send + Sync + 'static>;

/// Told every plane's kept fresh runners, by plane.
pub type Teller = std::sync::Arc<dyn Fn(FreshRunnersKept) + Send + Sync + 'static>;

/// The fresh runners of `plane` kept now: what the window's Notice starts from.
#[tauri::command]
#[specta::specta]
pub fn fresh_runners_kept(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
) -> Result<Vec<FreshKept>, String> {
    Ok(planes.held(&plane)?.chats().fresh_runners_kept())
}

/// **Fetch again**: the kept runner of chat `chat` is asked again for its work, off the window,
/// and goes once all of it is here. The Notice hears how it came out on [`EVENT`].
#[tauri::command]
#[specta::specta]
pub async fn fetch_fresh_runner_again(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    chat: String,
) -> Result<(), String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("fetching a task's runner again", move || {
        held.chats().fetches_fresh_runner_again(&chat)
    })
    .await
}

/// **Discard**: the person lets the kept runner of chat `chat` go with what it holds that is not
/// here. The Notice hears it went on [`EVENT`].
#[tauri::command]
#[specta::specta]
pub async fn discard_fresh_runner(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    chat: String,
) -> Result<(), String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("discarding a task's runner", move || {
        held.chats().discards_fresh_runner(&chat)
    })
    .await
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Before you turn your sandbox down** (#2071): the plain-file vaults a project registers,
//! which a chat with no sandbox can read like any file, and their move into the system keychain
//! ([`purlis_core::secrets::tokeyring`]). Settings › Project › Sandbox › Your level draws them
//! beside the level, which is kept in this machine's `purlis.local.toml`
//! ([`purlis_core::sandbox::personal`]).
//!
//! Beside them, **the levels that wait for your Confirm** (ADR 0096 §7): a level
//! `purlis.local.toml` says that is lower than the one you set or confirmed here, which a
//! chat's start does not honour until you confirm it ([`purlis_core::sandbox::personal`]).
//!
//! Every command here is the window's alone: a link is served none.

use purlis_core::sandbox::personal;
use purlis_core::secrets::{Ctx, Env, tokeyring};

use crate::planes::{PlaneId, Planes};

/// A plain-file vault, and whether this machine may move it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PlainFileVault {
    pub name: String,
    /// `false` for one the team's file registers, which is the team's to move.
    pub movable: bool,
}

fn listed(plain: Vec<tokeyring::PlainFile>) -> Vec<PlainFileVault> {
    plain
        .into_iter()
        .map(|one| PlainFileVault {
            name: one.name,
            movable: one.movable,
        })
        .collect()
}

/// What a move says, as the person reads it. `moved.secrets` is how many values moved, never
/// one of them: it is written as the number it is, beside the noun that agrees with it.
fn said(name: &str, moved: &tokeyring::Moved) -> String {
    let count: usize = moved.secrets;
    let noun = purlis_core::counts::plural(count, "secret", "secrets");
    match &moved.file_kept {
        None => format!("Moved {name} into the keychain: {count} {noun}, and its file is gone."),
        Some(file) => format!(
            "Moved {name} into the keychain: {count} {noun}. Its old file at {file} could not be \
             removed, and it still holds the values: remove it yourself."
        ),
    }
}

/// The plain-file vaults the project registers. Never a value.
#[tauri::command]
#[specta::specta]
pub async fn plain_file_vaults(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
) -> Result<Vec<PlainFileVault>, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    tauri::async_runtime::spawn_blocking(move || {
        let ctx = Ctx::new(&root, Env::from_process());
        tokeyring::plain_file_vaults(&ctx)
            .map(listed)
            .map_err(|why| why.message.to_string())
    })
    .await
    .map_err(|err| format!("reading the vaults did not finish: {err}"))?
}

/// Moves plain-file vault `name` into the system keychain, or says why it was not moved.
#[tauri::command]
#[specta::specta]
pub async fn move_vault_to_keychain(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    name: String,
) -> Result<String, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    tauri::async_runtime::spawn_blocking(move || {
        let ctx = Ctx::new(&root, Env::from_process());
        tokeyring::move_to_keyring(&ctx, &name)
            .map(|moved| said(&name, &moved))
            .map_err(|why| why.message.to_string())
    })
    .await
    .map_err(|err| format!("moving the vault did not finish: {err}"))?
}

/// A level the file says that waits for your Confirm.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct LevelWaiting {
    /// The persona's level, or the project's for `None`.
    pub persona: Option<String>,
    /// What the file says: `on`, `floor` or `off`.
    pub level: String,
}

fn waiting_of(all: Vec<personal::Awaiting>) -> Vec<LevelWaiting> {
    all.into_iter()
        .map(|one| LevelWaiting {
            persona: one.persona,
            level: one.level.word().to_owned(),
        })
        .collect()
}

/// The levels of the project that wait for your Confirm.
#[tauri::command]
#[specta::specta]
pub async fn sandbox_levels_waiting(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
) -> Result<Vec<LevelWaiting>, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    tauri::async_runtime::spawn_blocking(move || waiting_of(personal::waiting(&root)))
        .await
        .map_err(|err| format!("reading your levels did not finish: {err}"))
}

/// Confirms `level`, the level the row showed for `persona` (the project's for `None`), as
/// yours: refused where the file no longer says it.
#[tauri::command]
#[specta::specta]
pub async fn confirm_sandbox_level(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    persona: Option<String>,
    level: String,
) -> Result<String, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    let shown = personal::Choice::of_word(&level)
        .ok_or_else(|| format!("'{}' is not a level", level.escape_debug()))?;
    tauri::async_runtime::spawn_blocking(move || {
        let level = personal::confirm(&root, persona.as_deref(), shown)?;
        Ok(confirmed_said(persona.as_deref(), level))
    })
    .await
    .map_err(|err| format!("confirming your level did not finish: {err}"))?
}

/// What a Confirm says.
fn confirmed_said(persona: Option<&str>, level: personal::Choice) -> String {
    let whose = persona.unwrap_or("this project");
    format!(
        "Confirmed: {} for {whose}, from each chat's next start.",
        level.said()
    )
}

#[cfg(test)]
mod tests;

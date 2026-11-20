// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **GitLab access, for the window** (#1349): the commands of Settings › GitLab access. The flow
//! is the core's ([`purlis_core::forge::gitlabaccess`]); this file runs it where the person is and
//! records their Allow of what it wrote.
//!
//! **The token crosses once, inward.** The window hands it to [`gitlab_access_set`], which keeps
//! it in each persona's vault; it is never returned, logged or put in an error.
//!
//! Every command is the window's alone (`WINDOW_ONLY`): no link and no chat runs any of them.

use purlis_core::forge::gitlabaccess as core;
use purlis_core::secrets::{Ctx, Env, cmd};
use serde::Serialize;

use crate::planes::{PlaneId, Planes};

/// One GitLab host of the project's clones.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct GitlabRow {
    pub host: String,
    /// Personas whose `glab` line this flow wrote for the host.
    pub personas: Vec<String>,
    /// Personas with a `glab` line of their own for the host: shown, never replaced.
    pub by_hand: Vec<String>,
}

/// What the tab draws.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct GitlabShown {
    pub rows: Vec<GitlabRow>,
    pub personas: Vec<String>,
}

/// What setting up did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct GitlabFinished {
    /// Personas with an Allow of their own still to give.
    pub to_allow: Vec<String>,
}

fn ctx_of(planes: &Planes, plane: &PlaneId) -> Result<Ctx, String> {
    Ok(Ctx::new(planes.held(plane)?.root(), Env::from_process()))
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|e| format!("GitLab access did not finish: {e}"))?
}

/// Each persona with the vault its token goes in, or why one has none.
fn vaults_of(ctx: &Ctx, personas: &[String]) -> Result<Vec<(String, String)>, String> {
    personas
        .iter()
        .map(|persona| cmd::persona_vault(ctx, persona).map(|vault| (persona.clone(), vault)))
        .collect()
}

/// **What the tab draws**, read fresh: each GitLab host the project's clones are on, with the
/// personas set up for it.
#[tauri::command]
#[specta::specta]
pub async fn gitlab_access_list(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
) -> Result<GitlabShown, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    blocking(move || {
        let held = core::held(&root)?;
        let rows = core::hosts(&root)
            .into_iter()
            .map(|host| {
                let on = |ours: bool| {
                    held.iter()
                        .filter(|one| one.host == host && one.ours() == ours)
                        .map(|one| one.persona.clone())
                        .collect::<Vec<_>>()
                };
                GitlabRow {
                    personas: on(true),
                    by_hand: on(false),
                    host,
                }
            })
            .collect();
        Ok(GitlabShown {
            rows,
            personas: purlis_core::workspaces::Plane::open(root.clone())
                .personas()
                .map_err(|_| {
                    "purlis couldn't read this project's personas. Try again.".to_owned()
                })?,
        })
    })
    .await
}

/// **The exact lines setting up adds**, from the one source it writes from.
#[tauri::command]
#[specta::specta]
pub async fn gitlab_access_lines(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    host: String,
    personas: Vec<String>,
) -> Result<String, String> {
    let ctx = ctx_of(&planes, &plane)?;
    blocking(move || Ok(core::lines_shown(&host, &vaults_of(&ctx, &personas)?))).await
}

/// **Sets GitLab access up**: writes what the page showed, or nothing, then allows it on this
/// machine where only those lines wait. The token is kept in each persona's vault and goes
/// nowhere else.
#[tauri::command]
#[specta::specta]
pub async fn gitlab_access_set(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    host: String,
    personas: Vec<String>,
    token: String,
    shown: String,
) -> Result<GitlabFinished, String> {
    if personas.is_empty() {
        return Err("Tick at least one persona.".to_owned());
    }
    let ctx = ctx_of(&planes, &plane)?;
    let held = planes.held(&plane)?;
    let network = planes.network().cloned();
    blocking(move || {
        let root = held.root().to_path_buf();
        let known = purlis_core::workspaces::Plane::open(root.clone())
            .personas()
            .map_err(|_| "purlis couldn't read this project's personas. Try again.".to_owned())?;
        if !personas.iter().all(|persona| known.contains(persona)) {
            return Err("One of those personas isn't in this project. Reopen this tab.".to_owned());
        }
        let host = core::host_in_project(&root, &host)?;
        if shown != core::lines_shown(&host, &vaults_of(&ctx, &personas)?) {
            return Err(
                "The lines on this page changed. Look them over and press Set up again.".to_owned(),
            );
        }
        let at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since| since.as_secs());
        let audit = held.audit_then_record(network.as_ref());
        let done = core::set_up(
            &ctx,
            &root,
            &host,
            token.trim(),
            &personas,
            Some(&mut |shown| crate::sandboxing::allow_shown(&root, shown, &audit, at)),
        )?;
        Ok(GitlabFinished {
            to_allow: done.left,
        })
    })
    .await
}

/// **Takes GitLab access for a host away**: only what this flow wrote.
#[tauri::command]
#[specta::specta]
pub async fn gitlab_access_remove(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    host: String,
) -> Result<(), String> {
    let ctx = ctx_of(&planes, &plane)?;
    let root = planes.held(&plane)?.root().to_path_buf();
    blocking(move || core::remove(&ctx, &root, &host)).await
}

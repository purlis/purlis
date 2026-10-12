// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The welcome's "Get a project from your forge" (#1815): the window's door to
//! `purlis_core::getproject`.
//!
//! Three commands, all of them the window's alone (`WINDOW_ONLY`, and a channel besides): the
//! folder a pasted URL goes into, the clone itself with its progress, and the person's Cancel.
//! The clone answers with the folder it made; the window then opens that folder through the
//! same open-and-trust question as every other open, so nothing here trusts what it fetched.
//!
//! One clone runs at a time, which is all a welcome page can start.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use purlis_core::getproject::{self, Got, ProjectUrl};
use tauri::ipc::Channel;

/// The running clone's stop, while one runs.
static RUNNING: Mutex<Option<Arc<AtomicBool>>> = Mutex::new(None);

/// Where a pasted URL's project would go.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct ProjectFolder {
    /// The project's name, from the URL's last part.
    pub name: String,
    /// The folder it goes into: `<parent>/<name>`.
    pub folder: String,
}

/// How far a clone has got, as the window shows it.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct GetProgress {
    /// The stage, in the person's words ("Downloading").
    pub stage: String,
    /// The share of that stage done, when git says.
    pub percent: Option<u8>,
}

/// The URL as a project URL: in the scenario build, a `file://` stand-in for a forge too.
fn project_url(url: &str) -> Result<ProjectUrl, String> {
    #[cfg(feature = "e2e")]
    if let Some(path) = url.trim().strip_prefix("file://") {
        return Ok(ProjectUrl::stand_in(Path::new(path)));
    }
    ProjectUrl::parse(url).map_err(|refused| refused.sentence)
}

/// Where the project at `url` goes: `<parent>/<name>`, with `parent` the place the person picked
/// last, or `~/Projects`. Answers the bad URL's sentence when `url` is not one purlis clones.
#[tauri::command]
#[specta::specta]
pub fn project_url_folder(url: String, parent: Option<String>) -> Result<ProjectFolder, String> {
    let url = project_url(&url)?;
    let home = getproject::home()
        .ok_or("purlis could not tell which folder is your home. Choose a folder with Choose….")?;
    let folder = getproject::folder_for(&url, parent.as_deref().map(Path::new), &home);
    Ok(ProjectFolder {
        name: url.name().to_owned(),
        folder: folder.display().to_string(),
    })
}

/// Clones the project at `url` into `folder`, telling `progress` how far it has got. Answers
/// the folder, to be opened through the trust question; `null` when the person called it off;
/// or the sentence that says why it failed. Either way nothing it made is left behind.
#[tauri::command]
#[specta::specta]
pub async fn get_project(
    url: String,
    folder: String,
    progress: Channel<GetProgress>,
) -> Result<Option<String>, String> {
    let url = project_url(&url)?;
    let off = Arc::new(AtomicBool::new(false));
    {
        let mut running = RUNNING
            .lock()
            .map_err(|_| "the clone's state is unreadable")?;
        if running.is_some() {
            return Err("A project is being got already. Wait for it, or cancel it.".to_owned());
        }
        *running = Some(Arc::clone(&off));
    }
    let into = PathBuf::from(folder);
    let answer = tauri::async_runtime::spawn_blocking(move || {
        getproject::get(&url, &into, &off, &mut |now| {
            // A window that has gone has nobody to tell; the clone goes on until it is stopped.
            let _ = progress.send(GetProgress {
                stage: now.stage.words().to_owned(),
                percent: now.percent,
            });
        })
    })
    .await;
    if let Ok(mut running) = RUNNING.lock() {
        *running = None;
    }
    match answer.map_err(|err| format!("purlis could not get the project: {err}"))? {
        Ok(Got::Cloned(at)) => Ok(Some(at.display().to_string())),
        Ok(Got::CalledOff) => Ok(None),
        Err(refused) => Err(refused.sentence),
    }
}

/// The person's Cancel: stops the running clone, which then removes what it made. Nothing
/// happens when none runs.
#[tauri::command]
#[specta::specta]
pub fn cancel_get_project() {
    if let Ok(running) = RUNNING.lock()
        && let Some(off) = running.as_ref()
    {
        off.store(true, Ordering::SeqCst);
    }
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The Pool setup tab's commands** (spec #4469, #4480): what `purlis_core::poolsetup` answers, as
//! the window reads it, and the choices the person makes there. Reading changes nothing here;
//! each change happens on its own button.

use crate::planes::{PlaneId, Planes};

/// One step's next action.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PoolSetupAction {
    pub id: String,
    pub label: String,
    pub does: String,
    pub command: Option<String>,
    pub url: Option<String>,
}

/// One step as it stands.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PoolSetupStep {
    pub id: String,
    pub name: String,
    pub state: String,
    pub line: Option<String>,
    pub findings: Vec<String>,
    pub next: Option<PoolSetupAction>,
    pub skippable: bool,
}

/// What the tab draws.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PoolSetupShown {
    /// The pool being set up, once it is made.
    pub pool: Option<String>,
    /// The name given on the Pool step, for the box to hold.
    pub name: Option<String>,
    /// The step the person has reached.
    pub at: Option<String>,
    pub steps: Vec<PoolSetupStep>,
}

/// A core value's word, as it serializes.
#[cfg(unix)]
fn word(value: &impl serde::Serialize) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|said| said.as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// What the core answered, as the window takes it.
#[cfg(unix)]
pub fn shown(setup: &purlis_core::poolsetup::Setup, name: Option<String>) -> PoolSetupShown {
    PoolSetupShown {
        pool: setup.pool.clone(),
        name,
        at: setup.at.as_ref().map(word),
        steps: setup
            .steps
            .iter()
            .map(|step| PoolSetupStep {
                id: word(&step.id),
                name: step.name.to_owned(),
                state: word(&step.state),
                line: step.line.clone(),
                findings: step.findings.clone(),
                next: step.next.as_ref().map(|next| PoolSetupAction {
                    id: word(&next.id),
                    label: next.label.clone(),
                    does: next.does.clone(),
                    command: next.command.clone(),
                    url: next.url.clone(),
                }),
                skippable: step.skippable,
            })
            .collect(),
    }
}

#[cfg(unix)]
fn config_root() -> Result<std::path::PathBuf, String> {
    purlis_core::machine::config_root()
        .ok_or_else(|| "There is no config home on this machine to keep a pool in.".to_owned())
}

/// **Each step's state, read now**: the pool the person named once it is made, else a new one's.
#[tauri::command]
#[specta::specta]
pub async fn pool_setup(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
) -> Result<PoolSetupShown, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    #[cfg(unix)]
    {
        crate::off_the_window("reading Pool setup", move || {
            use purlis_core::dockerpool::Docker;
            use purlis_core::poolsetup::{self, Here};
            let config = config_root()?;
            let docker = poolsetup::docker_here();
            let here = Here {
                config: &config,
                plane: Some(&root),
                docker: docker.as_ref(),
                engine: Docker::engine_on_this_machine(),
            };
            let (setup, name) = poolsetup::resumed(&here).map_err(|why| why.to_string())?;
            Ok(shown(&setup, name))
        })
        .await
    }
    #[cfg(not(unix))]
    {
        let _ = root;
        Err(crate::dockerpools::NOT_HERE.to_owned())
    }
}

/// **Keeps that the person chose Docker on Where.** Nothing else changes.
#[tauri::command]
#[specta::specta]
pub async fn pool_setup_choose_docker() -> Result<(), String> {
    #[cfg(unix)]
    {
        crate::off_the_window("keeping your choice", || {
            purlis_core::poolsetup::choose_kind(
                &config_root()?,
                purlis_core::poolsetup::Kind::Docker,
            )
            .map_err(|why| why.to_string())
        })
        .await
    }
    #[cfg(not(unix))]
    {
        Err(crate::dockerpools::NOT_HERE.to_owned())
    }
}

/// **Keeps the name given on the Pool step**, or none, so coming back finds it.
#[tauri::command]
#[specta::specta]
pub async fn pool_setup_name(name: Option<String>) -> Result<(), String> {
    #[cfg(unix)]
    {
        crate::off_the_window("keeping your choice", move || {
            purlis_core::poolsetup::choose_name(&config_root()?, name.as_deref())
                .map_err(|why| why.to_string())
        })
        .await
    }
    #[cfg(not(unix))]
    {
        let _ = name;
        Err(crate::dockerpools::NOT_HERE.to_owned())
    }
}

/// **Opens the Docker app**, on the person's press, where this system has one to open. Refused
/// elsewhere, with what to do instead.
#[tauri::command]
#[specta::specta]
pub async fn pool_setup_open_docker() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        crate::off_the_window("opening Docker", || {
            let opened = purlis_core::forklock::output(
                std::process::Command::new("/usr/bin/open").args(["-a", "Docker"]),
            )
            .map_err(|why| why.to_string())?;
            if opened.status.success() {
                Ok(())
            } else {
                Err("purlis couldn't open Docker. Open it the way you usually do.".to_owned())
            }
        })
        .await
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("purlis can't open Docker here. Start it the way you usually do.".to_owned())
    }
}

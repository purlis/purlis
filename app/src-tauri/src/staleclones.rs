// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A workspace's repo left on a chat's branch, or on a moved remote** (#1985): the window's
//! half of `purlis_core::staleclone`.
//!
//! [`stale_clones`] lists every repo of the project's workspaces that is stale, which the
//! Inbox draws as one Notice per repo. [`fix_stale_clone`] is that Notice's Fix: it reads the
//! repo again, and either switches it back to its default branch (only where nothing would be
//! lost) or sets `origin` to the address the forge named, or says what blocks it and does
//! nothing. The Fix is the window's alone (`WINDOW_ONLY`): it moves a repo's tree or its remote.

use purlis_core::forge::route::HostScope;
use purlis_core::staleclone::{self, Fix, Stale};

use crate::planes::{PlaneId, Planes};

/// One stale repo, as the Inbox draws it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct StaleClone {
    pub workspace: String,
    pub repo: String,
    /// On a branch purlis cut for a chat or a task: that branch, and the remote's default.
    pub branch: Option<StaleBranch>,
    /// On a remote the forge said moved: where it is now, and where it went.
    pub remote: Option<StaleRemote>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct StaleBranch {
    pub branch: String,
    pub default: String,
}

/// Both as `<host>/<owner>/<repo>`: a recorded address can carry a user, which is never drawn.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct StaleRemote {
    pub origin: String,
    pub now: String,
}

/// Which way out the person pressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum StaleFix {
    Branch,
    Remote,
}

/// Every stale repo of the project's workspaces, workspace by workspace.
///
/// On a blocking thread: it asks git a few questions per repo, and runs nothing that reaches a
/// network.
#[tauri::command]
#[specta::specta]
pub async fn stale_clones(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
) -> Result<Vec<StaleClone>, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    tauri::async_runtime::spawn_blocking(move || shown(staleclone::found(&root)))
        .await
        .map_err(|err| format!("reading the workspaces' repos did not finish: {err}"))
}

/// The person's Fix on a stale repo's Notice: answers what was done, or what blocks it.
#[tauri::command]
#[specta::specta]
pub async fn fix_stale_clone(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    workspace: String,
    repo: String,
    fix: StaleFix,
) -> Result<String, String> {
    let held = planes.held(&plane)?;
    let root = held.root().to_path_buf();
    let fix = match fix {
        StaleFix::Branch => Fix::Branch,
        StaleFix::Remote => Fix::Remote,
    };
    // A switch moves the tree of every chat standing in the repo itself: never mid-turn.
    if fix == Fix::Branch
        && let Some(clone) = purlis_core::repos::clone_at(&root, &workspace, &repo)
    {
        let busy = working_in(&held, &clone.path);
        if !busy.is_empty() {
            return Err(format!(
                "Not switched: {} is working in the repo. Fix it again when its turn ends.",
                busy.join(", ")
            ));
        }
    }
    let done = tauri::async_runtime::spawn_blocking(move || {
        let fixed = staleclone::fix(&root, &workspace, &repo, fix)?;
        // The window's saves push with the person's sign-in only to the remote recorded for the
        // repo: a record of the old address follows the repo to the new one.
        if let Some(moved) = &fixed.moved
            && let (Some(scope), Some(config_root)) =
                (HostScope::claim(), purlis_core::machine::config_root())
            && !purlis_core::fence::FENCED
        {
            purlis_core::forge::gitcredential::remote::moved(
                &scope,
                &config_root,
                &moved.clone,
                &moved.was,
                &moved.now,
            );
        }
        Ok::<_, String>(fixed.said)
    })
    .await
    .map_err(|err| format!("the fix did not finish: {err}"))??;
    held.tree_moved();
    Ok(done)
}

/// The chats mid-turn whose folder is the repo at `clone` or one inside it, by name.
fn working_in(held: &crate::planes::Held, clone: &std::path::Path) -> Vec<String> {
    held.chats()
        .open_now()
        .into_iter()
        .filter(|open| {
            open.cwd.as_deref().is_some_and(|cwd| {
                cwd.starts_with(clone)
                    || std::fs::canonicalize(cwd).is_ok_and(|real| real.starts_with(clone))
            })
        })
        .filter(|open| {
            held.board().glance(open.session).state == purlis_core::state::State::Running
        })
        .map(|open| open.label.clone().unwrap_or(open.name))
        .collect()
}

/// What the Inbox is given of what the core found.
fn shown(found: Vec<staleclone::Found>) -> Vec<StaleClone> {
    found
        .into_iter()
        .map(|found| {
            let mut out = StaleClone {
                workspace: found.workspace,
                repo: found.repo,
                branch: None,
                remote: None,
            };
            for stale in found.stale {
                match stale {
                    Stale::OnAChatsBranch { branch, default } => {
                        out.branch = Some(StaleBranch { branch, default });
                    }
                    Stale::RemoteMoved { origin, now } => {
                        out.remote = Some(StaleRemote {
                            origin: staleclone::place(&origin),
                            now: staleclone::place(&now),
                        });
                    }
                }
            }
            out
        })
        .collect()
}

#[cfg(test)]
mod tests;

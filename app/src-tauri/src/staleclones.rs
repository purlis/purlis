// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A workspace's repo left off its default branch, or on a moved remote** (#1985): the window's
//! half of `purlis_core::staleclone`.
//!
//! [`stale_clones`] lists every repo of the project's workspaces that is stale, which the
//! Inbox draws as one Notice per repo. [`fix_stale_clone`] is that Notice's Fix: it reads the
//! repo again, and either switches it back to its default branch (only where nothing would be
//! lost) or sets `origin` to the address the forge named, or says what blocks it and does
//! nothing. The Fix is the window's alone (`WINDOW_ONLY`): it moves a repo's tree or its remote.

use purlis_core::forge::route::HostScope;
use purlis_core::staleclone::{self, Fix, Pushes, Stale};

use crate::planes::{PlaneId, Planes};

/// What the Inbox is given: every stale repo, and what could not be read.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct StaleClones {
    pub stale: Vec<StaleClone>,
    /// A sentence for each workspace or repo purlis could not read.
    pub unread: Vec<String>,
}

/// One stale repo, as the Inbox draws it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct StaleClone {
    pub workspace: String,
    pub repo: String,
    /// Off its default branch: the branch it is on, and the remote's default.
    pub branch: Option<StaleBranch>,
    /// On a remote the forge said moved: where it is now, and where it went.
    pub remote: Option<StaleRemote>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct StaleBranch {
    pub branch: String,
    pub default: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct StaleRemote {
    /// `origin` as `<host>/<owner>/<repo>`: a recorded address can carry a user, never drawn.
    pub origin: String,
    /// The address the Fix sets, whole, as it is set.
    pub now: String,
    /// Whether `origin`'s push URLs, each its old address, go with it, so pushes go to `now` too
    /// (#2117).
    pub pushes_follow: bool,
    /// The places `origin` pushes to that the Fix leaves as they are, as `<host>/<owner>/<repo>`
    /// (a user never drawn), each once: empty where it has no push URL of its own or they follow
    /// it (#2117).
    pub pushes_kept: Vec<String>,
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
) -> Result<StaleClones, String> {
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
    shown: Option<String>,
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
        let fixed = staleclone::fix(&root, &workspace, &repo, fix, shown.as_deref())?;
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

/// Whether a chat glanced as `glance` may be in a turn: running, asking something, stopped on a
/// prompt in its terminal, waiting on work of its own (#3878), or not yet heard from. A chat
/// whose turn ended with nothing in flight is not.
fn mid_turn(glance: &crate::host::Glance) -> bool {
    use purlis_core::state::State;
    match glance.state {
        State::Running | State::Unknown => true,
        State::Waiting => glance.asking || glance.prompt.is_some() || glance.own_work,
        State::Done | State::Failed => false,
    }
}

/// The chats that may be mid-turn whose folder is the repo at `clone` or one inside it, by name.
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
        .filter(|open| mid_turn(&held.board().glance(open.session)))
        .map(|open| open.label.clone().unwrap_or(open.name))
        .collect()
}

/// What the Inbox is given of what the core found.
fn shown(findings: staleclone::Findings) -> StaleClones {
    let stale = findings
        .found
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
                    Stale::OffDefault { branch, default } => {
                        out.branch = Some(StaleBranch { branch, default });
                    }
                    Stale::RemoteMoved {
                        origin,
                        now,
                        pushes,
                    } => {
                        // Each place once, however many push URLs name it.
                        let mut kept: Vec<String> = Vec::new();
                        if let Pushes::Kept(urls) = &pushes {
                            for place in urls.iter().map(|url| staleclone::place(url)) {
                                if !kept.contains(&place) {
                                    kept.push(place);
                                }
                            }
                        }
                        out.remote = Some(StaleRemote {
                            origin: staleclone::place(&origin),
                            now,
                            pushes_follow: pushes == Pushes::Follow,
                            pushes_kept: kept,
                        });
                    }
                }
            }
            out
        })
        .collect();
    StaleClones {
        stale,
        unread: findings.unread,
    }
}

#[cfg(test)]
mod tests;

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The Saving view's question about a clone's remote** (#1848): the window's half of
//! `purlis_core::forge::gitcredential::remote`.
//!
//! An automatic save pushes with the person's purlis sign-in only to the remote recorded for the
//! clone. Where it cannot, the project's and each repo's save standing carries the question
//! ([`RemoteQuestion`]), which the Saving view draws as a Notice naming the remote, and the
//! person's press answers it here: [`confirm_remote`] records the remote, then saves as the
//! window's Save does.
//!
//! Both are the window's alone (`WINDOW_ONLY`): recording a remote is what lets later automatic
//! saves push to it as the person, so no link and no chat may press it.

use std::path::{Path, PathBuf};

use purlis_core::forge::gitcredential::remote::{self, Question};
use purlis_core::forge::route::HostScope;

use crate::planes::{PlaneId, Planes};

/// What the window asks about a clone's remote.
#[derive(Debug, Clone, PartialEq, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RemoteQuestion {
    /// `not-saved`: purlis has no remote recorded for the clone and no forge CLI to push it
    /// with, so its automatic saves wait for one save from the window.
    pub kind: String,
    /// The clone's remote now, as `<host>/<owner>/<repo>`.
    pub now: String,
}

/// The question about the clone at `clone` in the project at `plane`, or `null` when there is
/// none. None in a test build: the config home a test's window would read is outside its fence,
/// as `forgeaccounts::window_git` says.
pub fn of(plane: &Path, clone: &Path) -> Option<RemoteQuestion> {
    if purlis_core::fence::FENCED {
        return None;
    }
    let scope = HostScope::claim()?;
    let config_root = purlis_core::machine::config_root()?;
    let forge = purlis_core::gitpolicy::forge_for(clone, plane)?;
    remote::question(&scope, &config_root, &forge, clone).map(|question| match question {
        Question::NotSavedYet { now } => RemoteQuestion {
            kind: "not-saved".to_owned(),
            now,
        },
    })
}

/// The person's press on the question about the project's remote, or a repo's when `workspace`
/// and `repo` name one: records the remote as it is now as the one its automatic saves push to,
/// then saves it as the window's Save does. Answers what the save said, or why it was refused.
#[tauri::command]
#[specta::specta]
pub async fn confirm_remote(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    workspace: Option<String>,
    repo: Option<String>,
) -> Result<Vec<String>, String> {
    let held = planes.held(&plane)?;
    let root = held.root().to_path_buf();
    tauri::async_runtime::spawn_blocking(move || {
        let clone = clone_of(&root, workspace.as_deref(), repo.as_deref())?;
        let scope = HostScope::claim()
            .ok_or_else(|| "A remote is confirmed from the window alone.".to_owned())?;
        let config_root = purlis_core::machine::config_root()
            .ok_or_else(|| "purlis found no home directory to record the remote in".to_owned())?;
        remote::confirm(&scope, &config_root, &clone)?;
        save(&root, workspace.as_deref(), repo.as_deref(), &|workspace| {
            held.mid_turn_in(workspace)
        })
    })
    .await
    .map_err(|err| format!("confirming the remote did not finish: {err}"))?
}

/// The clone a question is about: the project's own, or `repo` of `workspace`.
fn clone_of(root: &Path, workspace: Option<&str>, repo: Option<&str>) -> Result<PathBuf, String> {
    let (Some(workspace), Some(repo)) = (workspace, repo) else {
        return Ok(root.to_path_buf());
    };
    let found = purlis_core::repos::clones(root, workspace).map_err(|why| why.to_string())?;
    found
        .repos
        .into_iter()
        .find(|found| found.name == repo)
        .map(|found| found.path)
        .ok_or_else(|| format!("{workspace} holds no clone called {repo}"))
}

/// The window's own Save of the project, or of `repo` of `workspace`.
fn save(
    root: &Path,
    workspace: Option<&str>,
    repo: Option<&str>,
    mid_turn: &dyn Fn(&str) -> Vec<String>,
) -> Result<Vec<String>, String> {
    match (workspace, repo) {
        (Some(workspace), Some(repo)) => {
            crate::saving::save_repo_in(root, workspace, repo, None, &|| mid_turn(workspace))
        }
        _ => crate::saving::save(root, None),
    }
}

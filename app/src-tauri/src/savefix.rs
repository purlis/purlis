// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Fix**, on a save's red pull request (the operator's ruling P2, 2026-10-13; ADR 0051):
//! a chat started at once, at the plane root or in the workspace whose repo the request is
//! of, on a message purlis writes about what failed.
//!
//! **The message is the core's** (`purlis_core::savefix`): the request and its page, each
//! failed check with what it found, asked of the forge on the press with the window's own
//! sign-in, and what to do. What the checks say is quoted there as data. This layer starts the
//! chat and decides nothing about its words.
//!
//! **The chat starts at once**, on that message as its first: the one exception to "a prepared
//! prompt is typed and never sent" (ADR 0061, amended 2026-10-14;
//! [`crate::curation::start_sent`]). It is an ordinary chat: the project's default profile,
//! the project's own default persona, its sandbox, and nothing granted past it.
//!
//! **One chat per request.** A second press while the first chat's program still runs answers
//! that chat, for the window to bring forward ([`Fixing::existing`]), and starts nothing.
//!
//! **The window's alone** (`WINDOW_ONLY`): the person's own press starts it.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use purlis_core::engine::Size;
use purlis_core::forge::Caller;
use purlis_core::planegit::{self, MergeState};
use purlis_core::reposave;
use purlis_core::savefix::Trouble;

use crate::curation::{self, ChatTyped, SentFor};
use crate::planes::{Held, PlaneId, Planes};

/// The chat a Fix opened, or the one already open for the same request: what the window needs
/// to put its tab on the right strip, or to bring it forward.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Fixing {
    pub session: u32,
    /// The chat's name, which is its number.
    pub name: String,
    /// What its tab says.
    pub label: String,
    pub persona: Option<String>,
    /// The harness, by the word the plane calls it.
    pub harness: Option<String>,
    /// The workspace it is filed under, or none at the plane root.
    pub workspace: Option<String>,
    /// The chat was open already, from an earlier press for the same request: nothing was
    /// started, and the window brings it forward.
    pub existing: bool,
}

/// The Fix chats this process started, by the project's root and what each mends.
static OPEN: Mutex<Option<HashMap<(PathBuf, String), Fixing>>> = Mutex::new(None);
/// One Fix at a time: two presses are answered one after the other, so the second finds the
/// chat the first started.
static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

/// **Fix a save's red pull request**: the project's own, or the one of `repo` in `workspace`
/// when both are named. Starts a chat on what failed, or answers the one already open for the
/// same request.
///
/// The window's alone (`WINDOW_ONLY`): no link and no chat may press it.
///
/// On a blocking thread: it asks the forge what failed, and starts a program.
#[tauri::command]
#[specta::specta]
pub async fn fix_save_request(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    workspace: Option<String>,
    repo: Option<String>,
    columns: u16,
    rows: u16,
) -> Result<Fixing, String> {
    let held = planes.held(&plane)?;
    tauri::async_runtime::spawn_blocking(move || {
        fix_request(
            &held,
            workspace.as_deref(),
            repo.as_deref(),
            Size { columns, rows },
        )
    })
    .await
    .map_err(|err| format!("the chat did not start: {err}"))?
}

/// **Resolve a save's conflict in a chat** (qw382): the project's rebase is stopped on a
/// conflict the window started (`settle_save_conflict`), and a chat is started at the plane
/// root on a message that names the files and the two sides, asking it to edit them and run
/// no git. The window stages and continues; the chat's sandbox and A3c are as they are.
///
/// Refused when no rebase is stopped on a conflict, and when a file in conflict is one a
/// chat's sandbox may not write: that one is the person's to settle, in a terminal.
///
/// The window's alone (`WINDOW_ONLY`): no link and no chat may press it.
#[tauri::command]
#[specta::specta]
pub async fn resolve_save_in_a_chat(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    columns: u16,
    rows: u16,
) -> Result<Fixing, String> {
    let held = planes.held(&plane)?;
    tauri::async_runtime::spawn_blocking(move || resolve_request(&held, Size { columns, rows }))
        .await
        .map_err(|err| format!("the chat did not start: {err}"))?
}

/// [`resolve_save_in_a_chat`], without a runtime.
fn resolve_request(held: &Arc<Held>, size: Size) -> Result<Fixing, String> {
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(PoisonError::into_inner);
    let root = held.root().to_path_buf();
    let standing = planegit::standing(&root);
    let found = planegit::resolve::conflicts(&root);
    if standing.stopped_by != Some("rebase")
        || (found.marked.is_empty() && found.by_hand.is_empty())
    {
        return Err(
            "No replay is stopped on a conflict in this project, so there is nothing for a \
             chat to settle."
                .into(),
        );
    }
    // A conflict git could not mark is never handed to a chat: the person settles it.
    if !found.by_hand.is_empty() {
        return Err(planegit::resolve::said_by_hand(&found.by_hand));
    }
    let held_back = planegit::resolve::held_from_chats(&root, &found.marked);
    if !held_back.is_empty() {
        return Err(format!(
            "A chat may not write {}, so it cannot settle this conflict. Settle it in a \
             terminal here, then Continue, or Abort.",
            held_back
                .iter()
                .map(|path| format!("`{path}`"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    let (remote, own) = planegit::resolve::sides(&root)
        .ok_or("purlis cannot tell which commits the conflict is between")?;
    let what = format!("conflict {}", found.marked.join("\0"));
    let files = found.marked;
    fix_with(held, &what, &root, size, || {
        Ok(purlis_core::savefix::conflict(&files, &remote, &own))
    })
}

/// [`fix_save_request`], without a runtime.
fn fix_request(
    held: &Arc<Held>,
    workspace: Option<&str>,
    repo: Option<&str>,
    size: Size,
) -> Result<Fixing, String> {
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(PoisonError::into_inner);
    let root = held.root().to_path_buf();
    // What the window drew, read again: the request must still be red, and its page names
    // which request a chat already open is about.
    let (page, cwd, found) = match (workspace, repo) {
        (None, None) => {
            let standing = planegit::shared_standing(&root);
            (
                red_page(standing.pr, standing.merge.as_ref())?,
                root.clone(),
                None,
            )
        }
        (Some(workspace), Some(name)) => {
            let found =
                purlis_core::repos::clones(&root, workspace).map_err(|why| why.to_string())?;
            let repo = found
                .repos
                .into_iter()
                .find(|repo| repo.name == name)
                .ok_or_else(|| format!("{workspace} holds no clone called {name}"))?;
            let standing = reposave::shared_standing(&root, workspace, &repo);
            (
                red_page(standing.pr, standing.merge.as_ref())?,
                root.join("workspaces").join(workspace),
                Some((workspace, repo)),
            )
        }
        _ => {
            return Err(
                "name both the workspace and the repo, or neither for the project's own".into(),
            );
        }
    };
    let what = match &found {
        None => format!("plane {page}"),
        Some((workspace, repo)) => format!("repo:{workspace}/{} {page}", repo.name),
    };
    fix_with(held, &what, &cwd, size, || {
        // Asked of the forge as the person in the window: their press, their sign-in.
        Ok(match &found {
            None => planegit::trouble(&root, &Caller::window())?,
            Some((workspace, repo)) => {
                reposave::trouble(&root, workspace, repo, &Caller::window())?
            }
        }
        .1)
    })
}

/// A Fix for `what` at `cwd`: the chat already open for it, or one started on what `read`
/// answers the forge says failed now. A read that refuses (the request is not red any more,
/// its checks pass or run again, the forge cannot be asked) starts nothing, and its sentence
/// is the answer.
fn fix_with(
    held: &Arc<Held>,
    what: &str,
    cwd: &Path,
    size: Size,
    read: impl FnOnce() -> Result<Trouble, String>,
) -> Result<Fixing, String> {
    if let Some(open) = already_open(held, what) {
        return Ok(open);
    }
    start(held, what, &read()?, cwd, size)
}

/// The page of a request the standing says is red, or why there is nothing to fix.
fn red_page(page: Option<String>, merge: Option<&planegit::Merging>) -> Result<String, String> {
    match (page, merge) {
        (Some(page), Some(merge)) if merge.state == MergeState::Red => Ok(page),
        _ => Err("Its pull request is not red any more, so there is nothing to fix.".into()),
    }
}

/// The chat an earlier Fix started for `what`, while its program still runs.
fn already_open(held: &Arc<Held>, what: &str) -> Option<Fixing> {
    let key = (held.root().to_path_buf(), what.to_owned());
    let mut open = OPEN.lock().unwrap_or_else(PoisonError::into_inner);
    let fixes = open.get_or_insert_with(HashMap::new);
    let fixing = fixes.get(&key)?;
    let there = held.chats().recorded_chat(fixing.session).is_some()
        && crate::handoff::still_working(held, fixing.session);
    if !there {
        fixes.remove(&key);
        return None;
    }
    Some(Fixing {
        existing: true,
        ..fixing.clone()
    })
}

/// **Starts the Fix chat for `what`** at `cwd`, on `trouble` as its first message, or answers
/// the one already open for it. Private to this module: its one caller is the window's Fix.
/// A failing step of a save (the operator's ruling P6) will hand its own [`Trouble`] in here,
/// from this module, when that lands.
fn start(
    held: &Arc<Held>,
    what: &str,
    trouble: &Trouble,
    cwd: &Path,
    size: Size,
) -> Result<Fixing, String> {
    if let Some(open) = already_open(held, what) {
        return Ok(open);
    }
    let root = held.root().to_path_buf();
    let (profile, _kind) = curation::default_profile(&root, "no chat was started to fix it")?;
    let started = curation::start_sent(
        held,
        ChatTyped {
            profile,
            // The project's own default persona, as a new chat's is.
            persona: None,
            cwd: cwd.to_path_buf(),
            label: trouble.label.clone(),
            prompt: purlis_core::savefix::prompt(trouble),
            then_a_space: false,
            starts_runs: None,
        },
        size,
        SentFor::FixASave,
    )?;
    let fixing = Fixing {
        session: started.session,
        name: started.name,
        label: trouble.label.clone(),
        persona: None,
        harness: started.ready.declaration().map(|d| d.name.clone()),
        workspace: purlis_core::workspaces::Plane::open(&root)
            .workspace_of(started.ready.cwd.as_deref().unwrap_or(cwd)),
        existing: false,
    };
    OPEN.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get_or_insert_with(HashMap::new)
        .insert((root, what.to_owned()), fixing.clone());
    Ok(fixing)
}

#[cfg(test)]
mod tests;

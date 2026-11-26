// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The person's answers to Let this chat push a branch** (ADR 0128, amended 2026-11-23, #7363):
//! the window's half of `purlis_core::windowonly::let_chat_push`.
//!
//! A chat's update of a branch it has no record of raised the ask ([`PushAsks`]); [`allow_chat_push`]
//! is its **Allow** and [`keep_chat_push_blocked`] its **Keep blocked**. Allow records the chat as
//! the branch's owner at the commit the forge shows the person when they press
//! ([`crate::forgewiring::branch_tip_as_the_person`]), never one the chat claimed, and the tip rule
//! applies from there. Both are the window's alone (`WINDOW_ONLY`): a recorded owner is what lets
//! a chat move a branch, so no link, no chat and no command line may press it.
//!
//! [`PushAsks`]: purlis_core::windowonly::PushAsks

use purlis_core::forge::route::HostScope;
use purlis_core::windowonly::Control;

use crate::planes::{PlaneId, Planes};

/// The ask's control, from what the row sent. The core presses only an ask it raised: a control
/// that was not, or whose project or chat is not the one pressing, does nothing.
fn control(
    chat: String,
    host: String,
    repo: String,
    repo_id: &str,
    branch: String,
) -> Result<Control, String> {
    let repo_id = repo_id
        .parse::<u64>()
        .map_err(|_| "This ask names no repo purlis can read.".to_owned())?;
    Ok(Control::LetChatPush {
        chat,
        host,
        repo,
        repo_id,
        branch,
    })
}

/// **Allow**: records chat `chat` (session `session`) as the owner of `branch` in `repo` at the
/// tip the forge has on it now. Answers that tip, or why nothing was recorded.
#[tauri::command]
#[specta::specta]
#[allow(clippy::too_many_arguments)]
pub async fn allow_chat_push(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    session: u32,
    chat: String,
    host: String,
    repo: String,
    repo_id: String,
    branch: String,
) -> Result<String, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    let control = control(chat, host, repo, &repo_id, branch)?;
    // On a blocking thread: it asks the forge.
    tauri::async_runtime::spawn_blocking(move || {
        let scope = HostScope::claim()
            .ok_or_else(|| "A chat is let push from the window alone.".to_owned())?;
        let config_root = purlis_core::machine::config_root()
            .ok_or_else(|| "purlis found no home directory to record the branch in".to_owned())?;
        let Control::LetChatPush {
            host,
            repo,
            repo_id,
            ..
        } = &control
        else {
            return Err("This ask does not let a chat push.".to_owned());
        };
        purlis_core::windowonly::let_chat_push(
            &scope,
            &config_root,
            &root,
            session,
            &control,
            &|_, _, branch| {
                crate::forgewiring::branch_tip_as_the_person(
                    &root,
                    &config_root,
                    host,
                    repo,
                    *repo_id,
                    branch,
                )
            },
        )
    })
    .await
    .map_err(|err| format!("letting the chat push did not finish: {err}"))?
}

/// **Keep blocked**: the push stays refused, and the chat is not asked again.
#[tauri::command]
#[specta::specta]
#[allow(clippy::too_many_arguments)]
pub async fn keep_chat_push_blocked(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    session: u32,
    chat: String,
    host: String,
    repo: String,
    repo_id: String,
    branch: String,
) -> Result<(), String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    let control = control(chat, host, repo, &repo_id, branch)?;
    purlis_core::windowonly::keep_push_blocked(&root, session, &control)
}

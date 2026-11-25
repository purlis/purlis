// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A refusal a chat's own `purlis` handed the app to raise as an ask** (A3, R-328): the app's
//! answer to [`Ask::WindowOnly`], beside [`crate::removalasks`]'s.
//!
//! The chat's `forge_milestones` tool runs in the `purlis mcp` process, which holds no window,
//! so it cannot raise the ask itself ([`purlis_core::windowonly::raise`] is in-memory, by
//! project). It hands the refusal here. The app raises it, by its own record of the chat's
//! workspace and the clone's own origin, and answers with the sentence the chat is told: the
//! person was asked, or nobody was, in which case the block is written to the chat's Chat log
//! by the writer this process registered ([`purlis_core::windowonly::blocking`]). **The app
//! records nothing for it:** only the person's press of Record this repo does.

use purlis_core::hookwire::{Answer, WindowOnlyAsk, WindowOnlyRefusal};

use crate::planes::Held;

/// What the app answers chat `ask.chat`'s refusal: [`Answer::No`] with the sentence the chat is
/// told. Held to the rate a chat's brokered writes are, so it cannot fill the person's Inbox.
pub fn asked(held: &Held, ask: &WindowOnlyAsk) -> Answer {
    let no = |why: String| Answer::No { why };
    // The workspace is the app's record of the chat, never a word of the line's.
    let Some(workspace) = held.chats().workspace_of(ask.chat) else {
        return no(format!("chat {} is not one this app has open", ask.chat));
    };
    let Some(workspace) = workspace else {
        return no("A chat at the project's root has no workspace of repos to record.".to_owned());
    };
    if let Err(why) = held.brokered().allow(ask.chat, std::time::Instant::now()) {
        return no(why);
    }
    let WindowOnlyRefusal::RecordRepo { clone } = &ask.refused;
    match purlis_core::windowonly::ask_to_record(held.root(), ask.chat, &workspace, clone) {
        Ok(said) | Err(said) => no(said),
    }
}

/// **A removal's row, as the window sent it back** (T1.1-67): what the row showed, so that the
/// press removes exactly that and nothing else. `what` is `persona` or `branch-folder`.
#[derive(Debug, Clone, serde::Deserialize, specta::Type)]
pub struct RemovalRow {
    pub workspace: String,
    pub what: String,
    pub name: String,
    pub piece: String,
    pub force: bool,
    pub ignored: bool,
    pub listed: Vec<String>,
}

impl RemovalRow {
    /// The control the row was raised as. A row is pressed only where its listing was exact.
    fn control(self) -> Result<purlis_core::windowonly::Control, String> {
        use purlis_core::hookwire::Removing;
        let removing = match self.what.as_str() {
            "persona" => Removing::Persona { name: self.name },
            "branch-folder" => Removing::BranchFolder {
                repo: self.name,
                piece: self.piece,
                force: self.force,
                ignored: self.ignored,
            },
            other => return Err(format!("'{other}' is not something a chat asks to remove")),
        };
        Ok(purlis_core::windowonly::Control::Remove {
            removing,
            workspace: self.workspace,
            listed: self.listed,
            exact: true,
        })
    }
}

/// **The person's press on a removal ask** (T1.1-67, Q304 Q2): removes the persona or branch
/// folder the row listed, as their own `purlis persona remove` or `purlis worktree remove`
/// would, and only while it still holds exactly what the row listed. Anything that differs (a new
/// change, a new commit, another ignored file) refreshes the row and removes nothing. Answers what
/// the removal said, or why nothing was removed.
///
/// Window only (`WINDOW_ONLY`): a removal is for good.
#[tauri::command]
#[specta::specta]
pub async fn remove_listed(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    row: RemovalRow,
) -> Result<String, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    tauri::async_runtime::spawn_blocking(move || {
        purlis_core::forge::route::HostScope::claim()
            .ok_or_else(|| "A removal is pressed in the window alone.".to_owned())?;
        purlis_core::windowonly::remove_listed(&root, &row.control()?)
    })
    .await
    .map_err(|err| format!("the removal did not finish: {err}"))?
}

/// **The person kept it** (Keep on a removal ask): every chat's ask for it goes, and nothing is
/// removed.
///
/// Window only (`WINDOW_ONLY`), as the press it answers.
#[tauri::command]
#[specta::specta]
pub async fn keep_removal(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    row: RemovalRow,
) -> Result<(), String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    tauri::async_runtime::spawn_blocking(move || {
        purlis_core::windowonly::keep_listed(&root, &row.control()?);
        Ok(())
    })
    .await
    .map_err(|err| format!("keeping it did not finish: {err}"))?
}

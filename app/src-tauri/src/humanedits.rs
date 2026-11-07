// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Discard a hunk, from the window** (#5406; ADR 0084 §8): the core writes it through its own
//! guards (`purlis_core::files::discard`), and this says the two things only the host knows,
//! which chats are working in the folder and in what state, and keeps the record.
//!
//! **Not idle warns; the choice is the operator's.** Idle is ADR 0084 §8's, read from the board as
//! a repo's Fix reads it ([`crate::staleclones::mid_turn`]). The check is made in this command,
//! just before the write, and a warning the operator passed is asked for by name (`busy_too`).
//!
//! **One edit, one event** (`purlis_core::humanedit`), under the first chat working in the folder
//! and naming them all. The write is done by then, so a log that refuses it does not undo the
//! discard: the window is told it is not in the chat's history.
//!
//! **Told at the chat's next turn** ([`line_for`]), which every chat named in the edit is whose
//! harness purlis arms a prompt hook on. A chat on one it does not, a harness run at level 1, is
//! never told, so the answer names it for the window to say (#5477; ADR 0084 §8, as amended
//! 2026-11-07).
//!
//! **Undo a turn** (#5411; ADR 0081 as amended 2026-11-05) is the other edit made here: every file
//! the latest turn not yet undone changed, put back as its start had it (`files::undo_turn`). Not
//! idle **refuses** it, naming each chat and its state, and the check is made under the run's
//! state lock in the same step as the write and its record ([`undo_in`]), as ADR 0081 §4 checks
//! `place`: a turn heard beginning meanwhile waits for the undo, and then is told of it.

use std::path::Path;

use purlis_core::files::{self, Against, Branch, Discard, Discarded, Target};
use purlis_core::humanedit::{self, HumanEdit};

use crate::piecefiles::{GitHunk, TurnPick, WhatChanged, branch, placed_of, what_changed_shown};
use crate::planes::{Held, PlaneId, Planes};

/// What of the file's change a discard puts back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum DiscardTarget {
    /// One hunk of a text file, as the diff drew it.
    Hunk { hunk: GitHunk },
    /// A binary file's change, whole.
    Whole,
}

/// What a discard is checked against.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum DiscardAgainst {
    /// The stamp the diff came with: the file must still be as it was drawn.
    Opened { stamp: String },
    /// The operator's Overwrite: the file is written as it was drawn, less the hunk.
    Overwrite { head: Option<String> },
}

/// A chat working in the folder that is not idle.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct BusyChat {
    pub name: String,
    /// Its state, as a sentence ends: "is running".
    pub state: String,
}

/// What a discard came to.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum DiscardAnswer {
    /// It is written. `now` is the file's diff without the hunk, or `null` when nothing of its
    /// change is left to draw; `unrecorded` says why the log did not take it, if it did not;
    /// `untold` names each chat in the folder whose agent is not told of it
    /// ([`crate::staleclones::untold_in`]).
    Discarded {
        now: Option<WhatChanged>,
        unrecorded: Option<String>,
        untold: Vec<String>,
    },
    /// Nothing was written: these chats are not idle, and the choice is the operator's.
    Busy { chats: Vec<BusyChat> },
    /// Nothing was written: the file changed on disk since the diff was drawn.
    Stale,
}

// One hunk of a file of a branch put back as its base has it, or a binary file's change whole: the
// window's control on a hunk of the diff (#5406). Window only: a chat has its own files to write.
// On a blocking thread and never the one that draws (SC-2). Not a doc comment, because the
// generated bindings carry those.
#[tauri::command]
#[specta::specta]
#[allow(clippy::too_many_arguments)]
pub async fn discard_hunk(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    workspace: String,
    repo: String,
    piece: Option<String>,
    path: String,
    target: DiscardTarget,
    against: DiscardAgainst,
    busy_too: bool,
    turn: Option<TurnPick>,
) -> Result<DiscardAnswer, String> {
    let held = planes.held(&plane)?;
    tauri::async_runtime::spawn_blocking(move || {
        let at = branch(&workspace, &repo, &piece);
        let ask = Discard {
            path,
            target: match target {
                DiscardTarget::Hunk { hunk } => Target::Hunk(hunk.into()),
                DiscardTarget::Whole => Target::Whole,
            },
            against: match against {
                DiscardAgainst::Opened { stamp } => Against::Opened(stamp),
                DiscardAgainst::Overwrite { head } => Against::Overwrite { head },
            },
            turn: turn.map(|TurnPick { chat, number }| files::TurnKey { chat, number }),
        };
        discard_in(&held, at, &ask, busy_too)
    })
    .await
    .map_err(|err| format!("the discard did not finish: {err}"))?
}

pub(crate) fn discard_in(
    held: &Held,
    at: Branch<'_>,
    ask: &Discard,
    busy_too: bool,
) -> Result<DiscardAnswer, String> {
    let root = held.root();
    let reader = crate::reader();
    let folder = placed_of(&reader, root, at, "")?.absolute;
    let chats = crate::staleclones::chats_in(held, &folder);
    let busy: Vec<BusyChat> = chats
        .iter()
        .filter_map(|(_, name, glance)| {
            state_of(glance).map(|state| BusyChat {
                name: name.clone(),
                state,
            })
        })
        .collect();
    if !busy.is_empty() && !busy_too {
        return Ok(DiscardAnswer::Busy { chats: busy });
    }
    match files::discard(&reader, root, at, ask).map_err(|refused| refused.to_string())? {
        Discarded::Stale => Ok(DiscardAnswer::Stale),
        Discarded::Done { edit, now } => {
            let edit = HumanEdit {
                busy: !busy.is_empty(),
                chats: chats.iter().map(|(number, ..)| *number).collect(),
                chat_ids: chats
                    .iter()
                    .filter_map(|(number, ..)| held.chats().id_of(*number))
                    .collect(),
                ..edit
            };
            Ok(DiscardAnswer::Discarded {
                now: now.map(|now| what_changed_shown(*now)),
                unrecorded: record(held, root, &edit).err(),
                untold: crate::staleclones::untold_in(held, &folder),
            })
        }
    }
}

/// What an undo came to.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum UndoAnswer {
    /// Every file is back. `unrecorded` says why the log did not take it, `unmarked` why the turn
    /// is not marked undone, if either; `untold` names each chat in the folder whose agent is not
    /// told of it ([`crate::staleclones::untold_in`]).
    Undone {
        unrecorded: Option<String>,
        unmarked: Option<String>,
        untold: Vec<String>,
    },
    /// Nothing was written: these chats are not idle, and an undo waits until they are.
    Busy { chats: Vec<BusyChat> },
}

// Every file a turn changed put back as it was before the turn, without committing and without
// moving a branch: Undo a turn, on the file diff's latest turn not yet undone (#5411). Refused
// while the folder is not idle. Window only. Not a doc comment, because the generated bindings
// carry those.
#[tauri::command]
#[specta::specta]
pub async fn undo_turn(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    workspace: String,
    repo: String,
    piece: Option<String>,
    turn: TurnPick,
) -> Result<UndoAnswer, String> {
    let held = planes.held(&plane)?;
    tauri::async_runtime::spawn_blocking(move || {
        let at = branch(&workspace, &repo, &piece);
        let turn = files::TurnKey {
            chat: turn.chat,
            number: turn.number,
        };
        undo_in(&held, at, &turn)
    })
    .await
    .map_err(|err| format!("the undo did not finish: {err}"))?
}

pub(crate) fn undo_in(
    held: &Held,
    at: Branch<'_>,
    turn: &files::TurnKey,
) -> Result<UndoAnswer, String> {
    let root = held.root();
    let reader = crate::reader();
    let folder = placed_of(&reader, root, at, "")?.absolute;
    let chats = crate::staleclones::chats_placed_in(held, &folder);
    let untold = crate::staleclones::untold_in(held, &folder);
    held.hooks().while_held(|glance| {
        let recording = |number: u32| {
            held.chats()
                .id_of(number)
                .is_some_and(|id| held.turns().recording(&id))
        };
        let busy: Vec<BusyChat> = chats
            .iter()
            .filter_map(|(number, name)| {
                undo_waits_on(&glance(*number), recording(*number)).map(|state| BusyChat {
                    name: name.clone(),
                    state,
                })
            })
            .collect();
        if !busy.is_empty() {
            return Ok(UndoAnswer::Busy { chats: busy });
        }
        let undone =
            files::undo_turn(&reader, root, at, turn).map_err(|refused| refused.to_string())?;
        let edit = HumanEdit {
            chats: chats.iter().map(|(number, _)| *number).collect(),
            chat_ids: chats
                .iter()
                .filter_map(|(number, _)| held.chats().id_of(*number))
                .collect(),
            ..undone.edit
        };
        Ok(UndoAnswer::Undone {
            unrecorded: record(held, root, &edit).err(),
            unmarked: undone.unmarked,
            untold,
        })
    })
}

fn record(held: &Held, root: &Path, edit: &HumanEdit) -> Result<(), String> {
    held.hooks()
        .record_human_edit(root, edit.chats.first().copied(), edit)
}

/// **The line chat `chat`'s turn is told of the operator's edits** (R8, V34a; ADR 0084 §8), read
/// from the event log. A chat with no id, or a log that cannot be read, is told nothing: the
/// turn goes on, and the reason is left for whoever reads the app's log.
pub(crate) fn line_for(held: &Held, chat: u32) -> Option<String> {
    let id = held.chats().id_of(chat)?;
    let logs = held.hooks().event_log_dir()?;
    match humanedit::since_last_turn(&logs, &id) {
        Ok(edits) => humanedit::announcement(&edits),
        Err(why) => {
            tracing::warn!("purlis: the event log did not read for a turn's human edits ({why})");
            None
        }
    }
}

/// How a sentence says what an undo waits on in a chat working in the folder; `None` for nothing.
/// `recording`: a boundary of its turn is still being recorded, so the turn's refs, and the turn
/// Undo is offered on, may not be its latest yet (#5411).
fn undo_waits_on(glance: &crate::host::Glance, recording: bool) -> Option<String> {
    state_of(glance).or_else(|| recording.then(|| "is still recording its last turn".to_string()))
}

/// How a sentence says a chat is not idle (ADR 0084 §8 names the chat and its state); `None` for
/// one that is.
fn state_of(glance: &crate::host::Glance) -> Option<String> {
    use purlis_core::state::State;
    if !crate::staleclones::mid_turn(glance) {
        return None;
    }
    Some(
        match glance.state {
            State::Running => "is running",
            State::Waiting if glance.asking => "is asking you something",
            State::Waiting if glance.prompt.is_some() => "is stopped on a prompt",
            State::Waiting => "is waiting on work of its own",
            _ => "has not been heard from yet",
        }
        .to_string(),
    )
}

#[cfg(test)]
mod tests;

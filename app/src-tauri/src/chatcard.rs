// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat looked up by its number, and a closed one's card** (#6214, parent #5820): the
//! window's half of `purlis_core::chatlookup`. The palette's finished row and, later, a pane's
//! `chat 641` link (#6217) ask [`chat_by_number`]. It answers whether the chat is open, open
//! again under another number, or ended, and an ended one comes with what its card shows. The
//! card's *Read it* asks [`chat_screen`] for the task's kept last screen
//! (`purlis_core::chatscreens`), and its *Reopen* is the finished row's
//! (`reopen_finished_task`).
//!
//! Both commands are the window's alone (ADR 0052), and answer only what the app's own records
//! say: the dispatch records and the screens the app kept. Nothing a chat says reaches either.

use purlis_core::chatlookup::{self, Found};
use purlis_core::dispatchrecord;

use crate::finished::FinishedTask;
use crate::planes::{Held, PlaneId, Planes};

/// What a number names, as the window is sent it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(tag = "found", rename_all = "snake_case")]
pub(crate) enum ChatLookup {
    /// An open chat answers to it: the window shows that chat.
    Open { number: u32 },
    /// The chat it was is open now under `now`: started again, or reopened.
    Restarted { now: u32 },
    /// The chat has ended: its card.
    Ended { card: ChatCard },
    /// No chat had it, or none purlis kept a record of.
    NoChat,
}

/// **A closed or finished chat's card** (#6214, the prototype's S6): "#641 · <title>", how and
/// when it ended, who started it, where it ran, its report, and whether *Read it* and *Reopen*
/// can do anything.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub(crate) struct ChatCard {
    /// The number it was looked up by: the chat's when it worked.
    pub number: u32,
    /// Its ULID: what *Read it* asks for its screen by.
    pub chat: String,
    /// Its task as its finished row has it: its name, how it ended and when, its report
    /// (masked), where it ran, and whether it reopens. `id` is its dispatch record's, which
    /// *Reopen* names.
    pub task: FinishedTask,
    /// Who started it: "you" where the person dispatched it, else the asking chat's name as
    /// the person saw it then.
    pub started_by: String,
    /// Whether its last screen was kept: *Read it* is disabled where it was not.
    pub screen_kept: bool,
}

/// What a card says of a task the person dispatched themselves.
const STARTED_BY_YOU: &str = "you";

/// **What `number` names in `held`'s project**: the one entry point the palette and a pane's
/// links call.
pub(crate) fn lookup(held: &Held, number: u32) -> ChatLookup {
    let open: Vec<chatlookup::OpenChat> = crate::dispatches::open_chats(held)
        .into_iter()
        .map(|one| chatlookup::OpenChat {
            number: one.session,
            id: one.id,
        })
        .collect();
    match chatlookup::by_number_in(held.root(), number, &open) {
        Found::Open { number } => ChatLookup::Open { number },
        Found::Restarted { now } => ChatLookup::Restarted { now },
        Found::NoChat => ChatLookup::NoChat,
        Found::Ended(record) => {
            let asker = open
                .iter()
                .find(|one| record.asker.chat.id.is_some() && one.id == record.asker.chat.id)
                .map_or(0, |one| one.number);
            let (Some(chat), Some(task)) = (
                record.worker.chat.id.clone(),
                crate::finished::card_row(held, &record, asker),
            ) else {
                // A handoff, or a task's chat with no id: no card to draw.
                return ChatLookup::NoChat;
            };
            ChatLookup::Ended {
                card: ChatCard {
                    number,
                    screen_kept: purlis_core::chatscreens::is_kept(held.root(), &chat),
                    chat,
                    task,
                    started_by: if record.asker.by_person {
                        STARTED_BY_YOU.to_owned()
                    } else {
                        record.asker.chat.name.clone()
                    },
                },
            }
        }
    }
}

/// **Keeps chat `session`'s last screen as it closes for good**, where it is a dispatched task
/// (a dispatch record names chat `id` as its worker) and this machine keeps task screens
/// (`purlis_core::chatscreens::keeps`). Read from the session host while the session is still
/// there, as a grade's chat is (`skillgrade::screen_of`). A screen that could not be read or
/// kept is said in the log, and the card's *Read it* says it was not kept.
pub(crate) fn keep_screen_on_close(
    root: &std::path::Path,
    host: &dyn crate::host::SessionHost,
    session: u32,
    id: &str,
) {
    if !purlis_core::chatscreens::keeps(root) {
        return;
    }
    let me = dispatchrecord::ChatRef {
        chat: session,
        id: Some(id.to_owned()),
        ..Default::default()
    };
    if dispatchrecord::ledger(root).latest_for(root, &me).is_none() {
        return;
    }
    let Some(screen) = crate::skillgrade::screen_of(host, session) else {
        tracing::info!("purlis: task chat {session}'s screen could not be read as it closed");
        return;
    };
    if let Err(why) = purlis_core::chatscreens::keep(root, id, &screen) {
        tracing::info!("purlis: task chat {session}'s screen was not kept ({why})");
    }
}

/// **What `number` names** (#6214): an open chat, one open again under another number, a
/// closed one's card, or no chat. On a blocking thread, as it reads the dispatch records.
#[tauri::command]
#[specta::specta]
pub(crate) async fn chat_by_number(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    number: u32,
) -> Result<ChatLookup, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("looking a chat up by its number", move || {
        Ok(lookup(&held, number))
    })
    .await
}

/// **What chat `chat` showed as it closed** (#6214), where this machine kept it: what *Read it*
/// draws read-only. Null where nothing was kept.
#[tauri::command]
#[specta::specta]
pub(crate) async fn chat_screen(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    chat: String,
) -> Result<Option<String>, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("reading a chat's kept screen", move || {
        Ok(purlis_core::chatscreens::kept(held.root(), &chat))
    })
    .await
}

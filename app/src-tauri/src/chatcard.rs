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
//! say: the dispatch records, the closed chats' records (#6304, `purlis_core::chatrecords`) and
//! the screens the app kept. Nothing a chat says reaches either.

use purlis_core::chatlookup::{self, Found};
use purlis_core::dispatchrecord;

use crate::finished::FinishedTask;
use crate::planes::{Held, PlaneId, Planes};

/// What a number names, as the window is sent it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(tag = "is", rename_all = "snake_case")]
pub(crate) enum ChatLookup {
    /// An open chat answers to it: the window shows that chat.
    Open { number: u32 },
    /// The chat it was is open now under `now`: started again, or reopened.
    Restarted { now: u32 },
    /// The chat has ended: its card, boxed as it is the one large answer.
    Ended { card: Box<ChatCard> },
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
    /// What it is called: its task's name, or an ordinary chat's title as its tab said it.
    pub title: String,
    /// When it ended (UTC, RFC 3339), where its record keeps an end.
    pub ended: Option<String>,
    /// Where it ran: its workspace's name, or `project root`.
    pub place: String,
    /// Its task as its finished row has it: how it ended, its report (masked) and whether it
    /// reopens. `id` is its dispatch record's, which *Reopen* names. **None for a chat no
    /// dispatch started** (#6304): it has no report and no Reopen, and the card says Closed.
    pub task: Option<FinishedTask>,
    /// Who started it: "you" where the person started or dispatched it, else the chat that
    /// asked or handed off, by its name as the person saw it then.
    pub started_by: String,
    /// Whether its last screen was kept: *Read it* is disabled where it was not.
    pub screen_kept: bool,
}

/// What a card says of a task the person dispatched themselves.
const STARTED_BY_YOU: &str = "you";

/// **What `number` names in `held`'s project**: the one entry point the palette and a pane's
/// links call.
pub(crate) fn lookup(held: &Held, number: u32) -> ChatLookup {
    let open = open_chats(held);
    match chatlookup::by_number_in(held.root(), number, &open) {
        Found::Open { number } => ChatLookup::Open { number },
        Found::Restarted { now } => ChatLookup::Restarted { now },
        Found::NoChat => ChatLookup::NoChat,
        Found::Closed(chat) => ChatLookup::Ended {
            card: Box::new(card_of_chat(held, &chat)),
        },
        Found::Ended(record) => match card_of(held, number, &record, &open) {
            Some(card) => ChatLookup::Ended {
                card: Box::new(card),
            },
            None => ChatLookup::NoChat,
        },
    }
}

/// The open chats of `held`'s project, as the core's lookup is handed them.
pub(crate) fn open_chats(held: &Held) -> Vec<chatlookup::OpenChat> {
    crate::dispatches::open_chats(held)
        .into_iter()
        .map(|one| chatlookup::OpenChat {
            number: one.session,
            id: one.id,
        })
        .collect()
}

/// **The card of chat `number`, ended with `record`**, given the chats open now: what the
/// palette's finished row and a pane's `chat 641` link (#6217) show. None where the record
/// makes no card (`chatlookup::makes_a_card`): a handoff's, or a task's chat with no id.
pub(crate) fn card_of(
    held: &Held,
    number: u32,
    record: &dispatchrecord::Record,
    open: &[chatlookup::OpenChat],
) -> Option<ChatCard> {
    let asker = open
        .iter()
        .find(|one| chatlookup::is(&record.asker.chat, one))
        .map_or(0, |one| one.number);
    if !chatlookup::makes_a_card(record) {
        return None;
    }
    let chat = record.worker.chat.id.clone()?;
    let task = crate::finished::card_row(held, record, asker)?;
    Some(ChatCard {
        number,
        screen_kept: purlis_core::chatscreens::is_kept(held.root(), &chat),
        chat,
        title: task.name.clone(),
        ended: task.ended.clone(),
        place: task.place.clone(),
        task: Some(task),
        started_by: if record.asker.by_person {
            STARTED_BY_YOU.to_owned()
        } else {
            record.asker.chat.name.clone()
        },
    })
}

/// **The card of a chat no dispatch started** (#6304), from the record the app kept as it
/// closed: its title, when it closed, who started it and where it ran. No task, so no report
/// and no Reopen; and its screen was never kept (the person's ruling keeps a task's only), so
/// *Read it* is disabled with "Its screen was not kept."
pub(crate) fn card_of_chat(held: &Held, chat: &purlis_core::chatrecords::ChatRecord) -> ChatCard {
    ChatCard {
        number: chat.number,
        // A task's screen is kept by its ULID; an ordinary chat's never is.
        screen_kept: purlis_core::chatscreens::is_kept(held.root(), &chat.id),
        chat: chat.id.clone(),
        title: chat.title.clone(),
        ended: Some(chat.ended.clone()),
        place: chat.place.clone(),
        task: None,
        started_by: match &chat.started_by {
            purlis_core::chatrecords::StartedBy::Person => STARTED_BY_YOU.to_owned(),
            purlis_core::chatrecords::StartedBy::Chat { name } => name.clone(),
        },
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
    // Where this machine keeps none, nothing is kept, and what was kept goes now.
    if !purlis_core::chatscreens::keeps(root) {
        purlis_core::chatscreens::forget_all(root);
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

/// **The record chat `session` (ULID `id`) leaves as it closes for good** (#6304): what its card
/// shows once its number is looked up, dispatched or not. Its title as its tab said it, when it
/// started (its ULID's time: when it was first minted), `now` as its end, who started it (the
/// person, or the chat that handed off to it) and the workspace it ran in. None for an id that
/// is not a ULID.
pub(crate) fn closed_record(
    session: u32,
    id: &str,
    chat: &purlis_core::reopen::Chat,
    harness: Option<purlis_core::harness::Harness>,
    workspace: Option<&str>,
    now: chrono::DateTime<chrono::Utc>,
) -> Option<purlis_core::chatrecords::ChatRecord> {
    use purlis_core::chatrecords::{ChatRecord, StartedBy};
    let minted = purlis_core::eventlog::written_at_ms(id)?;
    let started =
        chrono::DateTime::<chrono::Utc>::from_timestamp_millis(i64::try_from(minted).ok()?)?;
    let stamp =
        |at: chrono::DateTime<chrono::Utc>| at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    Some(ChatRecord {
        number: session,
        id: id.to_owned(),
        title: purlis_core::reopen::shown_name(
            chat,
            harness.map(purlis_core::harness::Harness::name),
        ),
        started: stamp(started),
        ended: stamp(now),
        // A chat the person started from another chat's tab is theirs, as its report says.
        started_by: match chat.from.as_ref() {
            Some(from) if !from.by_person => StartedBy::Chat {
                name: from.name.clone(),
            },
            _ => StartedBy::Person,
        },
        place: workspace.map_or_else(|| PROJECT_ROOT.to_owned(), str::to_owned),
    })
}

/// Where a chat ran that ran in no workspace, as a finished task's row says it.
const PROJECT_ROOT: &str = "project root";

#[cfg(test)]
mod closed_record_tests {
    use super::*;

    const ID: &str = "01K6CHATS0000000000000000C";

    fn now() -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::parse_from_rfc3339("2026-11-12T09:40:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc)
    }

    fn chat() -> purlis_core::reopen::Chat {
        purlis_core::reopen::Chat {
            program: "claude".to_owned(),
            name: "steward 702".to_owned(),
            label: Some("Tidy the changelog".to_owned()),
            ..Default::default()
        }
    }

    #[test]
    fn a_chat_the_person_started_is_recorded_with_its_title_times_and_place() {
        let record = closed_record(702, ID, &chat(), None, Some("smart-ide"), now()).unwrap();
        assert_eq!(record.number, 702);
        assert_eq!(record.id, ID);
        assert_eq!(record.title, "Tidy the changelog");
        assert_eq!(record.ended, "2026-11-12T09:40:00Z");
        assert_eq!(
            record.started,
            chrono::DateTime::<chrono::Utc>::from_timestamp_millis(
                ulid::Ulid::from_string(ID).unwrap().timestamp_ms() as i64
            )
            .unwrap()
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
        );
        assert_eq!(
            record.started_by,
            purlis_core::chatrecords::StartedBy::Person
        );
        assert_eq!(record.place, "smart-ide");
    }

    #[test]
    fn a_chat_with_no_workspace_ran_in_the_project_root() {
        let record = closed_record(702, ID, &chat(), None, None, now()).unwrap();
        assert_eq!(record.place, "project root");
    }

    #[test]
    fn an_id_that_is_no_ulid_leaves_no_record() {
        assert!(closed_record(702, "702", &chat(), None, None, now()).is_none());
    }
}

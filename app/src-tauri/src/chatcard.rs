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
    /// Whether a dispatch started it. True with no task for a handoff, or a task that ended
    /// owing no report: its card is drawn from its chat's record, and *Reopen* says why it does
    /// nothing in words of its own.
    pub dispatched: bool,
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
        dispatched: true,
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
        dispatched: dispatched(held.root(), chat),
    }
}

/// Whether a dispatch record names chat `chat` as its worker, by its ULID: a handoff, or a task
/// that ended owing no report, whose card the lookup draws from its chat's record.
fn dispatched(root: &std::path::Path, chat: &purlis_core::chatrecords::ChatRecord) -> bool {
    let worker = dispatchrecord::ChatRef {
        chat: chat.number,
        id: Some(chat.id.clone()),
        ..Default::default()
    };
    dispatchrecord::ledger(root)
        .latest_for(root, &worker)
        .is_some()
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

/// How long a quit waits, over every project together, for the screens of the tasks that had
/// finished (#6305): as long as it gives every plane's saving to stop
/// (`crate::autosave::QUIT_STOPS_WITHIN`).
pub(crate) const SCREENS_AT_QUIT_WITHIN: std::time::Duration = std::time::Duration::from_secs(3);

/// [`SCREENS_AT_QUIT_WITHIN`] from now: the deadline a quit's task screens are given.
pub(crate) fn screens_by_from_now() -> std::time::Instant {
    std::time::Instant::now() + SCREENS_AT_QUIT_WITHIN
}

/// **Keeps the last screens of the task chats `chats` as the app quits** (#6305), by number and
/// id: the ones the next launch leaves out of its put back, having finished
/// (`finished::left_out`), which never close through `Chats::close_as`. Read while their
/// sessions are still there, before every program is ended, each on a thread of its own.
///
/// **It returns by `by` whatever the host does** (#6451's review): a view can wait out a link's
/// and an answer's wait, or a pool's answer wait of minutes, and a quit is not held for one.
/// Each read runs on a thread of its own that nothing joins: one still waiting at `by` is left
/// to finish or to end with the app, and the screen it would have kept is not kept (a write cut
/// off leaves only a temporary file, which the store sweeps).
pub(crate) fn keep_screens_at_quit(
    root: &std::path::Path,
    host: &std::sync::Arc<dyn crate::host::SessionHost>,
    chats: &[(u32, String)],
    by: std::time::Instant,
) {
    if chats.is_empty() {
        return;
    }
    let (done, finished) = std::sync::mpsc::channel();
    for (session, id) in chats {
        let (root, host, session, id, done) = (
            root.to_path_buf(),
            std::sync::Arc::clone(host),
            *session,
            id.clone(),
            done.clone(),
        );
        let spawned = std::thread::Builder::new()
            .name("keeping a task's screen".to_owned())
            .spawn(move || {
                keep_screen_on_close(&root, &*host, session, &id);
                let _ = done.send(());
            });
        if let Err(why) = spawned {
            tracing::info!("purlis: task chat {session}'s screen was not kept ({why})");
        }
    }
    drop(done);
    let mut left = chats.len();
    while left > 0 {
        let wait = by.saturating_duration_since(std::time::Instant::now());
        match finished.recv_timeout(wait) {
            Ok(()) => left -= 1,
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                tracing::info!(
                    "purlis: {left} task screen(s) did not come in time as purlis quit, and \
                     were not kept"
                );
                break;
            }
        }
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

/// **Whether this machine keeps a task's last screen in a project, and how many it holds**
/// (#6306): what the row Settings › Project › Dispatch › Keep task screens draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, specta::Type)]
pub(crate) struct TaskScreens {
    /// `[task_screens] keep` in the project's local file, `true` where it is not set.
    pub keep: bool,
    /// How many screens are kept now: none where `keep` is off.
    pub kept: u32,
}

/// The project at `root`'s [`TaskScreens`], deleting every kept screen where keeping is off.
pub(crate) fn task_screens_at(root: &std::path::Path) -> TaskScreens {
    let (kept, _) = purlis_core::chatscreens::held(root);
    TaskScreens {
        keep: purlis_core::chatscreens::keeps(root),
        kept: u32::try_from(kept).unwrap_or(u32::MAX),
    }
}

/// Sets keeping in the project at `root`, which off deletes every kept screen at once
/// (`purlis_core::chatscreens::set_keep`), and answers what is in force then.
pub(crate) fn set_task_screens_at(
    root: &std::path::Path,
    keep: bool,
) -> Result<TaskScreens, String> {
    purlis_core::chatscreens::set_keep(root, keep)?;
    Ok(task_screens_at(root))
}

/// **Whether tasks' screens are kept in project `plane`, and how many are** (#6306).
#[tauri::command]
#[specta::specta]
pub(crate) async fn task_screens(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
) -> Result<TaskScreens, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("reading whether task screens are kept", move || {
        Ok(task_screens_at(held.root()))
    })
    .await
}

/// **Turns keeping tasks' screens on or off in project `plane`** (#6306), in its local file
/// alone; off deletes every kept screen. The window asks the person before it turns it off.
#[tauri::command]
#[specta::specta]
pub(crate) async fn set_task_screens(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    keep: bool,
) -> Result<TaskScreens, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("setting whether task screens are kept", move || {
        set_task_screens_at(held.root(), keep)
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use purlis_core::dispatchrecord::{self, Asker, ChatRef, Mode, Opening, Place, Worker};

    use super::*;
    use crate::host::SessionHost;
    use crate::host::pretend::Pretend;

    const TASK: &str = "01K6W0RKER000000000000000B";

    /// A project whose task chat 7 (`TASK`) has a dispatch record, running on `host` and
    /// showing a screen of its own.
    fn a_task_on(host: &Pretend) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().expect("a folder");
        let root = std::fs::canonicalize(dir.path()).expect("canonical");
        let worker = ChatRef {
            chat: 7,
            id: Some(TASK.to_owned()),
            name: "devops 7".to_owned(),
            persona: Some("devops".to_owned()),
        };
        dispatchrecord::open(
            &root,
            Opening {
                mode: Mode::Task,
                asker: Asker {
                    chat: ChatRef {
                        chat: 4,
                        id: Some("01K6ASKER0000000000000000A".to_owned()),
                        name: "steward 4".to_owned(),
                        persona: Some("steward".to_owned()),
                    },
                    workspace: None,
                    by_person: false,
                    session_record: None,
                },
                persona: Some("devops".to_owned()),
                worker: Worker {
                    chat: worker,
                    harness: Some("claude".to_owned()),
                    profile: None,
                    session_record: None,
                },
                task: Some("check prod".to_owned()),
                place: Place::default(),
                brief: "check prod".to_owned(),
                report_owed: true,
            },
            chrono::Utc::now(),
        )
        .expect("recorded");
        host.open(Some(7), &crate::host::Opening::default(), &|_| {})
            .expect("running");
        host.shows(7, "check prod: all green\r\n");
        (dir, root)
    }

    #[test]
    fn a_task_screen_is_kept_at_a_quit_that_has_time_for_it() {
        let host = Pretend::default();
        let (_dir, root) = a_task_on(&host);
        let shared: Arc<dyn SessionHost> = Arc::new(host);

        keep_screens_at_quit(
            &root,
            &shared,
            &[(7, TASK.to_owned())],
            Instant::now() + SCREENS_AT_QUIT_WITHIN,
        );

        assert_eq!(
            purlis_core::chatscreens::kept(&root, TASK).as_deref(),
            Some("check prod: all green\r\n")
        );
    }

    #[test]
    fn a_quit_waits_for_a_task_screen_no_longer_than_its_deadline() {
        // #6451's review: a view a host never answers (a pool's runner on a half-open link)
        // holds the quit only until the deadline every project shares.
        let host = Pretend::default();
        let (_dir, root) = a_task_on(&host);
        host.terminal_takes(Duration::from_secs(30));
        let shared: Arc<dyn SessionHost> = Arc::new(host);
        let began = Instant::now();

        keep_screens_at_quit(
            &root,
            &shared,
            &[(7, TASK.to_owned())],
            began + Duration::from_millis(300),
        );

        assert!(
            began.elapsed() < Duration::from_secs(2),
            "the quit waited {:?}",
            began.elapsed()
        );
    }
}

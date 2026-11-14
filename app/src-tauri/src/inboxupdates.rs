// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The Inbox's updates, for the window** (#1693, spec #1688): the app's half of
//! [`purlis_core::inboxupdates`], which argues what an update is, where it is kept and for how
//! long.
//!
//! Three commands, each about one project the window holds:
//!
//! - [`inbox_updates`] reads its updates, newest first;
//! - [`note_inbox_updates`] notes what the window derived from a source (a finished row, the
//!   doctor's report, a chat put back, a block, the away list, a Smart close's step): one whose
//!   key is kept already is left as it is, so noting is safe to repeat;
//! - [`settle_inbox_updates`] marks read or dismisses the ones named, or every one: Mark all
//!   read and Dismiss all (I-10), which act on updates only. **No ask is ever settled here**:
//!   an ask is answered through its own source (`asking`), one at a time.
//!
//! **The window's alone** (`purlis_session_protocol::ui::WINDOW_ONLY`): what happened to the
//! person's chats, by name, is no link's to read, and what they put away no link's to change.
//! Each answers off the window's thread, since a write flushes the file to the disk.

use std::path::PathBuf;

use purlis_core::inboxupdates::{Kind, Settle, Store, Update};

use crate::planes::{PlaneId, Planes};

/// What kind of thing happened, as the window names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "kebab-case")]
pub enum UpdateKind {
    /// A task finished: done, cancelled, or ended by the person.
    TaskDone,
    /// A task came to nothing: failed, blocked, ended without a report, or did not start.
    TaskFailed,
    /// The doctor found something.
    Doctor,
    /// A chat came back: resumed, or started fresh in its place.
    Resumed,
    /// A chat's sandbox refused something that is no ask.
    Sandbox,
    /// A dispatch was refused while nobody was at its chat (#1507).
    RefusedAway,
    /// A Smart close stopped without its record (SI-8f).
    SmartClose,
    /// A chat's report has nowhere to go: the chat that asked for it has gone (#1694).
    ReportUndelivered,
    /// purlis's `pre-commit` refused a commit a chat made (#1694).
    CommitRefused,
    /// purlis could not update the project's root for a chat, for a reason no chat can settle,
    /// so bringing the remote's commits in is the person's (qw348, #3877).
    RootUpdateRefused,
    /// A chat asked the person to take its browser over, and the browser ended first (#2015).
    BrowserAskEnded,
    /// A task ended with its question unanswered (#4556).
    QuestionClosed,
    /// A task's question could not be taken its route (#4557).
    QuestionFailed,
    /// A chat another chat started arrived in no pane, and was announced (#4759): a handoff,
    /// or a task that can't report asks.
    Arrived,
    /// The person filed a report a chat drafted (#5694).
    ReportSent,
    /// A working chat may not be getting anywhere (#6014).
    Stuck,
    /// A chat's turn ended (T1-31): its finish row, with its blocks still unresolved.
    TurnFinished,
    /// A chat's own run ended failed (T1-31).
    ChatFailed,
}

impl From<UpdateKind> for Kind {
    fn from(kind: UpdateKind) -> Self {
        match kind {
            UpdateKind::TaskDone => Kind::TaskDone,
            UpdateKind::TaskFailed => Kind::TaskFailed,
            UpdateKind::Doctor => Kind::Doctor,
            UpdateKind::Resumed => Kind::Resumed,
            UpdateKind::Sandbox => Kind::Sandbox,
            UpdateKind::RefusedAway => Kind::RefusedAway,
            UpdateKind::SmartClose => Kind::SmartClose,
            UpdateKind::ReportUndelivered => Kind::ReportUndelivered,
            UpdateKind::CommitRefused => Kind::CommitRefused,
            UpdateKind::RootUpdateRefused => Kind::RootUpdateRefused,
            UpdateKind::BrowserAskEnded => Kind::BrowserAskEnded,
            UpdateKind::QuestionClosed => Kind::QuestionClosed,
            UpdateKind::QuestionFailed => Kind::QuestionFailed,
            UpdateKind::Arrived => Kind::Arrived,
            UpdateKind::ReportSent => Kind::ReportSent,
            UpdateKind::Stuck => Kind::Stuck,
            UpdateKind::TurnFinished => Kind::TurnFinished,
            UpdateKind::ChatFailed => Kind::ChatFailed,
        }
    }
}

impl From<Kind> for UpdateKind {
    fn from(kind: Kind) -> Self {
        match kind {
            Kind::TaskDone => UpdateKind::TaskDone,
            Kind::TaskFailed => UpdateKind::TaskFailed,
            Kind::Doctor => UpdateKind::Doctor,
            Kind::Resumed => UpdateKind::Resumed,
            Kind::Sandbox => UpdateKind::Sandbox,
            Kind::RefusedAway => UpdateKind::RefusedAway,
            Kind::SmartClose => UpdateKind::SmartClose,
            Kind::ReportUndelivered => UpdateKind::ReportUndelivered,
            Kind::CommitRefused => UpdateKind::CommitRefused,
            Kind::RootUpdateRefused => UpdateKind::RootUpdateRefused,
            Kind::BrowserAskEnded => UpdateKind::BrowserAskEnded,
            Kind::QuestionClosed => UpdateKind::QuestionClosed,
            Kind::QuestionFailed => UpdateKind::QuestionFailed,
            Kind::Arrived => UpdateKind::Arrived,
            Kind::ReportSent => UpdateKind::ReportSent,
            Kind::Stuck => UpdateKind::Stuck,
            Kind::TurnFinished => UpdateKind::TurnFinished,
            Kind::ChatFailed => UpdateKind::ChatFailed,
        }
    }
}

/// One update, as the window draws it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct InboxUpdate {
    /// Its source's own name for it: what the window notes, settles and matches it by.
    pub key: String,
    pub kind: UpdateKind,
    /// When it happened, in seconds since 1970.
    pub at: u32,
    /// The chat it is about, while that number means one.
    pub session: Option<u32>,
    /// Who it is about, the session first: names, as data.
    pub chain: Vec<String>,
    /// What happened, in one line: data, never markup.
    pub says: String,
    /// The person has read it.
    pub read: bool,
    /// The first-open baseline set it aside (#5648): listed From the project, read. Absent for
    /// every other update.
    #[specta(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub baselined: Option<bool>,
}

impl From<Update> for InboxUpdate {
    fn from(one: Update) -> Self {
        Self {
            key: one.key,
            kind: one.kind.into(),
            at: u32::try_from(one.at).unwrap_or(u32::MAX),
            session: one.session,
            chain: one.chain,
            says: one.says,
            read: one.read,
            baselined: one.baselined.then_some(true),
        }
    }
}

/// An update the window derived from its source, to be noted.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, specta::Type)]
pub struct UpdateNoted {
    pub key: String,
    pub kind: UpdateKind,
    /// When it happened, by its source's own time, in seconds since 1970.
    pub at: u32,
    pub session: Option<u32>,
    pub chain: Vec<String>,
    pub says: String,
}

impl From<UpdateNoted> for Update {
    fn from(one: UpdateNoted) -> Self {
        Self {
            key: one.key,
            kind: one.kind.into(),
            at: u64::from(one.at),
            session: one.session,
            chain: one.chain,
            says: one.says,
            read: false,
            dismissed: false,
            baselined: false,
        }
    }
}

/// What the person did with updates: Mark read, or Dismiss.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, specta::Type)]
#[serde(rename_all = "kebab-case")]
pub enum UpdateSettled {
    Read,
    Dismissed,
}

impl From<UpdateSettled> for Settle {
    fn from(how: UpdateSettled) -> Self {
        match how {
            UpdateSettled::Read => Settle::Read,
            UpdateSettled::Dismissed => Settle::Dismissed,
        }
    }
}

/// What a write says where this machine keeps no data home.
const NOWHERE: &str =
    "purlis has nowhere to keep the Inbox's updates on this machine: it has no data home.";

/// The project's root, for a command about it.
fn root_of(planes: &Planes, plane: &PlaneId) -> Result<PathBuf, String> {
    Ok(planes.held(plane)?.root().to_path_buf())
}

/// Runs `work` off the window's thread, and says so where it did not finish.
async fn off_the_window<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|err| format!("reading the Inbox's updates did not finish: {err}"))?
}

/// [`Store::read`] as the window draws it.
pub fn read_in(store: Option<&Store>, root: &std::path::Path, now: u64) -> Vec<InboxUpdate> {
    store
        .map(|store| store.read(root, now))
        .unwrap_or_default()
        .into_iter()
        .map(InboxUpdate::from)
        .collect()
}

/// [`Store::note`] as the window asks it.
pub fn note_in(
    store: Option<&Store>,
    root: &std::path::Path,
    noted: Vec<UpdateNoted>,
    now: u64,
) -> Result<Vec<InboxUpdate>, String> {
    let store = store.ok_or_else(|| NOWHERE.to_owned())?;
    store
        .note(root, noted.into_iter().map(Update::from).collect(), now)
        .map(|all| all.into_iter().map(InboxUpdate::from).collect())
        .map_err(|err| format!("purlis could not keep the Inbox's updates: {err}"))
}

/// [`Store::settle`] as the window asks it.
pub fn settle_in(
    store: Option<&Store>,
    root: &std::path::Path,
    keys: Option<Vec<String>>,
    how: UpdateSettled,
    now: u64,
) -> Result<Vec<InboxUpdate>, String> {
    let store = store.ok_or_else(|| NOWHERE.to_owned())?;
    store
        .settle(root, keys.as_deref(), how.into(), now)
        .map(|all| all.into_iter().map(InboxUpdate::from).collect())
        .map_err(|err| format!("purlis could not change the Inbox's updates: {err}"))
}

/// **A project's updates** (#1693): kept a day on this machine, newest first, dismissed ones
/// left out. None where this machine keeps no data home.
#[tauri::command]
#[specta::specta]
pub async fn inbox_updates(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
) -> Result<Vec<InboxUpdate>, String> {
    let root = root_of(&planes, &plane)?;
    off_the_window(move || {
        Ok(read_in(
            Store::here().as_ref(),
            &root,
            crate::sandboxing::now_secs(),
        ))
    })
    .await
}

/// **Notes what the window derived from a source** (#1693): each update whose key the project
/// does not keep yet is kept, unread; one it keeps, dismissed or not, is left as it is. One
/// older than a day is not kept. Answers the project's updates as [`inbox_updates`] does.
#[tauri::command]
#[specta::specta]
pub async fn note_inbox_updates(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    noted: Vec<UpdateNoted>,
) -> Result<Vec<InboxUpdate>, String> {
    let root = root_of(&planes, &plane)?;
    off_the_window(move || {
        note_in(
            Store::here().as_ref(),
            &root,
            noted,
            crate::sandboxing::now_secs(),
        )
    })
    .await
}

/// **Mark read, or Dismiss** (#1693, I-10): the updates named by `keys`, or every one the
/// project keeps where `keys` is null — Mark all read and Dismiss all. Updates only: no ask is
/// answered here. Answers the project's updates as [`inbox_updates`] does.
#[tauri::command]
#[specta::specta]
pub async fn settle_inbox_updates(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    keys: Option<Vec<String>>,
    how: UpdateSettled,
) -> Result<Vec<InboxUpdate>, String> {
    let root = root_of(&planes, &plane)?;
    off_the_window(move || {
        settle_in(
            Store::here().as_ref(),
            &root,
            keys,
            how,
            crate::sandboxing::now_secs(),
        )
    })
    .await
}

/// One chat Snooze all puts away (#5647): its session, and the keys of the asks it holds now.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, specta::Type)]
pub struct SnoozedChat {
    pub session: u32,
    pub asks: Vec<String>,
}

/// **Snooze all** (#5647, ADR 0119): puts each of `chats` away with the asks it holds, until it
/// holds an ask it did not have or, where `until` is given, until then (seconds since 1970: for
/// 1 hour, or until tomorrow at 09:00 as the window reckons it; #6018, ADR 0121 decision 14),
/// whichever comes first. Nothing is answered. Kept on this machine per project, never in the
/// plane.
#[tauri::command]
#[specta::specta]
pub async fn snooze_inbox_asks(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    chats: Vec<SnoozedChat>,
    until: Option<u32>,
) -> Result<(), String> {
    let root = root_of(&planes, &plane)?;
    off_the_window(move || {
        let store = Store::here().ok_or_else(|| NOWHERE.to_owned())?;
        store
            .snooze(
                &root,
                chats
                    .into_iter()
                    .map(|one| (one.session, one.asks))
                    .collect(),
                until.map(u64::from),
            )
            .map(|_| ())
            .map_err(|err| format!("purlis could not snooze the asks: {err}"))
    })
    .await
}

/// **Show them now, and Snooze all's Undo** (#5647): brings back the snoozed chats `sessions`,
/// or every one where it is null.
#[tauri::command]
#[specta::specta]
pub async fn wake_inbox_asks(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    sessions: Option<Vec<u32>>,
) -> Result<(), String> {
    let root = root_of(&planes, &plane)?;
    off_the_window(move || {
        let store = Store::here().ok_or_else(|| NOWHERE.to_owned())?;
        store
            .wake(&root, sessions.as_deref())
            .map(|_| ())
            .map_err(|err| format!("purlis could not bring the snoozed asks back: {err}"))
    })
    .await
}

/// **What the person has seen** (#6002, ADR 0121 decisions 10 and 11): the keys of the
/// project's asks, Notices and updates the window said were seen. One the window lists that is
/// not here is **New**. None where this machine keeps no data home.
#[tauri::command]
#[specta::specta]
pub async fn inbox_seen(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
) -> Result<Vec<String>, String> {
    let root = root_of(&planes, &plane)?;
    off_the_window(move || Ok(seen_in(Store::here().as_ref(), &root))).await
}

/// **Notes `keys` seen** (#6002): each row visible in an open view for about a second, or
/// whose chat was on screen in a focused window. One seen before is left as it is. **And where
/// `asks` is given**, the asks standing now by the same keys, every ask seen that is not among
/// them is forgotten, so the same ask asked again is New. Answers what [`inbox_seen`] does.
#[tauri::command]
#[specta::specta]
pub async fn see_inbox(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    keys: Vec<String>,
    asks: Option<Vec<String>>,
) -> Result<Vec<String>, String> {
    let root = root_of(&planes, &plane)?;
    off_the_window(move || {
        see_in(
            Store::here().as_ref(),
            &root,
            &keys,
            asks.as_deref(),
            crate::sandboxing::now_secs(),
        )
    })
    .await
}

/// What every ask's key in the seen record starts with, as the window makes it (`seen.ts`).
pub const ASK: &str = "ask:";

/// [`Store::seen`] as the window reads it: the keys alone.
pub fn seen_in(store: Option<&Store>, root: &std::path::Path) -> Vec<String> {
    store
        .map(|store| store.seen(root).into_keys().collect())
        .unwrap_or_default()
}

/// [`Store::see`] as the window asks it.
pub fn see_in(
    store: Option<&Store>,
    root: &std::path::Path,
    keys: &[String],
    asks: Option<&[String]>,
    now: u64,
) -> Result<Vec<String>, String> {
    let store = store.ok_or_else(|| NOWHERE.to_owned())?;
    if let Some(asks) = asks {
        store
            .forget(root, ASK, asks)
            .map_err(|err| format!("purlis could not keep what you have seen: {err}"))?;
    }
    store
        .see(root, keys, now)
        .map(|seen| seen.into_keys().collect())
        .map_err(|err| format!("purlis could not keep what you have seen: {err}"))
}

/// **The first-open baseline** (#5648, ADR 0119), as the window reads it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct InboxBaseline {
    /// When the project first opened on this machine, in seconds since 1970.
    pub at: u32,
    /// Whether that open set the past aside: false where this machine had seen the project.
    pub fresh: bool,
    /// The doctor findings already there, by the causes the window dismisses them by; null
    /// until the window has set aside the first report's.
    pub findings: Option<Vec<String>>,
}

impl From<purlis_core::inboxupdates::Baseline> for InboxBaseline {
    fn from(one: purlis_core::inboxupdates::Baseline) -> Self {
        Self {
            at: u32::try_from(one.at).unwrap_or(u32::MAX),
            fresh: one.fresh,
            findings: one.findings,
        }
    }
}

/// [`Store::first_open`] as the window asks it.
pub fn first_open_in(
    store: Option<&Store>,
    root: &std::path::Path,
    now: u64,
) -> Result<InboxBaseline, String> {
    let store = store.ok_or_else(|| NOWHERE.to_owned())?;
    store
        .first_open(root, now)
        .map(InboxBaseline::from)
        .map_err(|err| format!("purlis could not keep what this machine has seen: {err}"))
}

/// **The project opens in the window** (#5648, ADR 0119): its first-open baseline, made where
/// this machine has none. The first time a project opens on a machine with no seen record for
/// it, what is already there counts as seen, and its hosts and presets as told; a project with
/// a record keeps it, so updating purlis clears nothing.
#[tauri::command]
#[specta::specta]
pub async fn inbox_first_open(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
) -> Result<InboxBaseline, String> {
    let root = root_of(&planes, &plane)?;
    off_the_window(move || {
        first_open_in(Store::here().as_ref(), &root, crate::sandboxing::now_secs())
    })
    .await
}

/// **The doctor's first report after a fresh first open** (#5648): its findings, `keys`, were
/// already there, and the window puts each away until it changes. Kept once. Answers the
/// project's updates as [`inbox_updates`] does.
#[tauri::command]
#[specta::specta]
pub async fn set_aside_inbox_findings(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    keys: Vec<String>,
) -> Result<Vec<InboxUpdate>, String> {
    let root = root_of(&planes, &plane)?;
    off_the_window(move || {
        let store = Store::here().ok_or_else(|| NOWHERE.to_owned())?;
        store
            .set_aside_findings(&root, &keys, crate::sandboxing::now_secs())
            .map(|all| all.into_iter().map(InboxUpdate::from).collect())
            .map_err(|err| format!("purlis could not change the Inbox's updates: {err}"))
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 1_800_000_000;

    fn noted(key: &str, kind: UpdateKind, at: u64) -> UpdateNoted {
        UpdateNoted {
            key: key.to_owned(),
            kind,
            at: u32::try_from(at).expect("a time"),
            session: Some(4),
            chain: vec!["steward 12".to_owned()],
            says: "drill finished".to_owned(),
        }
    }

    #[test]
    fn the_window_notes_what_was_seen_and_an_ask_no_longer_standing_is_new_again() {
        let dir = tempfile::tempdir().expect("a directory");
        let store = Store::in_data(&dir.path().join("data"));
        let root = dir.path().join("project");
        std::fs::create_dir_all(&root).expect("a project");
        let keys = |names: &[&str]| {
            names
                .iter()
                .map(|one| (*one).to_owned())
                .collect::<Vec<_>>()
        };
        assert!(seen_in(Some(&store), &root).is_empty());

        let seen = see_in(
            Some(&store),
            &root,
            &keys(&["ask:3:wait", "notice:pin-gone:ide"]),
            None,
            NOW,
        )
        .expect("seen");
        assert_eq!(seen, ["ask:3:wait", "notice:pin-gone:ide"]);

        // Chat 3 was answered: its wait stands no more, so asked again it is New.
        let seen = see_in(Some(&store), &root, &[], Some(&[]), NOW).expect("seen");
        assert_eq!(seen, ["notice:pin-gone:ide"]);
        assert_eq!(seen_in(Some(&store), &root), seen);
        assert_eq!(
            see_in(None, &root, &[], None, NOW).expect_err("nowhere to keep it"),
            NOWHERE
        );
    }

    #[test]
    fn the_window_notes_reads_and_settles_a_project_s_updates() {
        let dir = tempfile::tempdir().expect("a directory");
        let store = Store::in_data(&dir.path().join("data"));
        let root = dir.path().join("project");
        std::fs::create_dir_all(&root).expect("a project");

        let after = note_in(
            Some(&store),
            &root,
            vec![
                noted("task:a", UpdateKind::TaskDone, NOW - 60),
                noted("task:b", UpdateKind::TaskFailed, NOW - 30),
            ],
            NOW,
        )
        .expect("noted");
        assert_eq!(
            after.iter().map(|one| one.key.as_str()).collect::<Vec<_>>(),
            ["task:b", "task:a"]
        );
        assert_eq!(after[0].kind, UpdateKind::TaskFailed);
        assert!(!after[0].read);

        let read = settle_in(Some(&store), &root, None, UpdateSettled::Read, NOW).expect("read");
        assert!(read.iter().all(|one| one.read));

        let left = settle_in(
            Some(&store),
            &root,
            Some(vec!["task:a".to_owned()]),
            UpdateSettled::Dismissed,
            NOW,
        )
        .expect("dismissed");
        assert_eq!(
            left.iter().map(|one| one.key.as_str()).collect::<Vec<_>>(),
            ["task:b"]
        );
        assert_eq!(read_in(Some(&store), &root, NOW), left);
    }

    #[test]
    fn a_machine_with_no_data_home_reads_none_and_says_why_it_keeps_none() {
        let root = std::path::Path::new("/nowhere");
        assert!(read_in(None, root, NOW).is_empty());
        let refused = note_in(None, root, vec![], NOW).expect_err("nowhere to keep it");
        assert!(refused.contains("data home"), "{refused}");
    }

    #[test]
    fn every_kind_is_the_core_s_and_back() {
        for kind in [
            UpdateKind::TaskDone,
            UpdateKind::TaskFailed,
            UpdateKind::Doctor,
            UpdateKind::Resumed,
            UpdateKind::Sandbox,
            UpdateKind::RefusedAway,
            UpdateKind::SmartClose,
            UpdateKind::BrowserAskEnded,
            UpdateKind::QuestionClosed,
            UpdateKind::QuestionFailed,
            UpdateKind::Arrived,
            UpdateKind::ReportSent,
            UpdateKind::Stuck,
            UpdateKind::TurnFinished,
            UpdateKind::ChatFailed,
        ] {
            assert_eq!(UpdateKind::from(Kind::from(kind)), kind);
        }
    }

    #[test]
    fn the_updates_are_the_window_s_alone() {
        for command in [
            "inbox_updates",
            "note_inbox_updates",
            "settle_inbox_updates",
            "inbox_first_open",
            "set_aside_inbox_findings",
            "inbox_seen",
            "see_inbox",
        ] {
            assert!(
                purlis_session_protocol::ui::WINDOW_ONLY.contains(&command),
                "{command} is the window's alone"
            );
        }
    }
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **`purlis.chats`: a workspace's chats as a tree, metadata only** (#6235, ADR 0122 as amended
//! 2026-11-11).
//!
//! The app holds the live chats, so it gathers each one's facts ([`Chat`]) from its own record,
//! the same record the window's Chats tree is drawn from (`OpenChat.from`), and this module
//! shapes the answer. Nothing here reads a harness's output (ADR 0076). A chat's state is one of
//! the window's eight Chat states (ADR 0121 decision 17), read in the window's order from the
//! facts the app's Rust holds ([`mark`]). Its cause names the fact that decided it.
//!
//! - **By default, the live chats**: every chat the app has open in the workspace.
//! - **With `history`, also ended dispatches** of the workspace, from the dispatch records
//!   (`app/dispatches/<id>.json`): the last [`HISTORY_DEFAULT`] to end, or `limit`, at most
//!   [`HISTORY_MOST`], each with the chat that asked it. A closed chat that dispatched nothing
//!   is in no record, so it is not listed (the operator's ruling of 2026-11-11).
//! - **Never a word a chat wrote**: no transcript, prompt, brief, report or message, and
//!   nothing from a vault. A name, a persona, a harness, a state and times.

use serde_json::{Map, Value, json};

use crate::dispatchrecord::Record;
use crate::state::State;

/// How many ended dispatches `history` lists when no `limit` is given.
pub const HISTORY_DEFAULT: u32 = 50;

/// The most ended dispatches `history` lists.
pub const HISTORY_MOST: u32 = 200;

/// **The window's eight Chat states** (ADR 0121 decision 17), in the order that wins where
/// several are true. The keys are `app/src/shownState.ts`'s `CHAT_STATES`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    Needs,
    Stuck,
    Failed,
    Working,
    Waiting,
    Idle,
    Ended,
    CantTell,
}

impl Mark {
    pub const ALL: [Mark; 8] = [
        Mark::Needs,
        Mark::Stuck,
        Mark::Failed,
        Mark::Working,
        Mark::Waiting,
        Mark::Idle,
        Mark::Ended,
        Mark::CantTell,
    ];

    /// Its key, as the window and the theme name it.
    pub fn key(self) -> &'static str {
        match self {
            Mark::Needs => "needs",
            Mark::Stuck => "stuck",
            Mark::Failed => "failed",
            Mark::Working => "working",
            Mark::Waiting => "waiting",
            Mark::Idle => "idle",
            Mark::Ended => "ended",
            Mark::CantTell => "cant-tell",
        }
    }

    /// Its name, as a list of every chat says it (`CHAT_STATE_NAMES`).
    pub fn said(self) -> &'static str {
        match self {
            Mark::Needs => "Waiting on you",
            Mark::Stuck => "Stuck?",
            Mark::Failed => "Failed",
            Mark::Working => "Working",
            Mark::Waiting => "Waiting on a chat",
            Mark::Idle => "Idle",
            Mark::Ended => "Done",
            Mark::CantTell => "Can't tell",
        }
    }
}

/// The one report a task owes the chat that dispatched it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Report {
    Owed,
    Sent,
    /// It ended without one, and purlis said so in its place.
    Failed,
}

/// What a task's own record says of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskFacts {
    pub report: Report,
    /// How it ended, in its dispatch record's word (`done`, `blocked`, `closed_by_person`, …).
    pub outcome: Option<String>,
}

/// **What a chat's state is read from**: the facts the app holds of it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Facts {
    /// The board's state, or none for a shell nothing has reported for.
    pub board: Option<State>,
    /// It is in the needs-you queue.
    pub needs_you: bool,
    /// The stuck watch says Stuck? of its turn.
    pub stuck: bool,
    /// Its record as a task; none for a chat that is not one.
    pub task: Option<TaskFacts>,
    /// How many of its tasks are working.
    pub tasks_at_work: u32,
    /// Its turn ended on work of its own that wakes it.
    pub own_work: bool,
    /// Whether its folder holds uncommitted work; none where it is not known.
    pub left_work: Option<bool>,
}

/// The outcomes that are the person's own doing, or purlis's at a limit they set: an end, not a
/// failure.
const BY_THE_PERSON: [&str; 7] = [
    "stopped_by_person",
    "closed_by_person",
    "stopped",
    "cancelled",
    "stopped_at_limit",
    "stopped_with_above",
    "stopped_at_token_limit",
];

/// The outcomes of a report that say the work did not get done.
fn failed_outcome(outcome: Option<&str>) -> bool {
    matches!(outcome, Some("failed" | "blocked"))
}

/// **A chat's Chat state, and the fact that decided it**, in the order `shownState.ts` reads
/// them: the needs-you queue; Stuck?; a task's record before its program's state; its
/// program's end; then how its turn ended.
pub fn mark(facts: &Facts) -> (Mark, &'static str) {
    let board = facts.board;
    let report = facts.task.as_ref().map(|t| t.report);
    let outcome = facts.task.as_ref().and_then(|t| t.outcome.as_deref());
    if facts.needs_you {
        return (Mark::Needs, "needs-you-queue");
    }
    if facts.stuck && board == Some(State::Running) && report != Some(Report::Failed) {
        return (Mark::Stuck, "stuck-watch");
    }
    match report {
        Some(Report::Sent) if board == Some(State::Running) => {
            return (Mark::Working, "turn-running");
        }
        Some(Report::Sent) if failed_outcome(outcome) => return (Mark::Failed, "reported-failed"),
        Some(Report::Sent) => return (Mark::Ended, "reported"),
        Some(Report::Failed) if outcome.is_some_and(|o| BY_THE_PERSON.contains(&o)) => {
            return (Mark::Ended, "ended-by-person");
        }
        Some(Report::Failed) => return (Mark::Failed, "ended-without-report"),
        Some(Report::Owed) if matches!(board, Some(State::Done | State::Failed)) => {
            return (Mark::Failed, "ended-without-report");
        }
        _ => {}
    }
    match board {
        Some(State::Done) => (Mark::Ended, "exited"),
        Some(State::Failed) => (Mark::Failed, "exited-failed"),
        Some(State::Waiting) if facts.tasks_at_work > 0 => (Mark::Waiting, "tasks-at-work"),
        Some(State::Waiting) if facts.own_work => (Mark::Working, "own-work"),
        Some(State::Waiting) if report == Some(Report::Owed) => {
            (Mark::Waiting, "turn-ended-owing-report")
        }
        Some(State::Waiting) if facts.left_work == Some(false) => {
            (Mark::Ended, "turn-ended-nothing-left")
        }
        Some(State::Waiting) => (Mark::Idle, "turn-ended"),
        Some(State::Running) => (Mark::Working, "turn-running"),
        Some(State::Unknown) => (Mark::CantTell, "no-hook-heard"),
        None => (Mark::CantTell, "shell"),
    }
}

/// How a chat came from the chat above it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Relation {
    /// It was dispatched as a task, and owes that chat a report.
    Dispatched,
    /// The work was handed off to it.
    HandedOffFrom,
}

impl Relation {
    pub fn word(self) -> &'static str {
        match self {
            Relation::Dispatched => "dispatched",
            Relation::HandedOffFrom => "handed-off-from",
        }
    }
}

/// The chat above one, by its [`Chat::id`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parent {
    pub id: String,
    pub relation: Relation,
    /// The workspace it works in; none for the project's root.
    pub workspace: Option<String>,
}

/// **One open chat, as the app holds it.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chat {
    /// Its id (ADR 0066), or [`number_id`] for one given none yet.
    pub id: String,
    pub name: String,
    pub persona: Option<String>,
    /// The harness, by the project's word for it; none for a shell.
    pub harness: Option<String>,
    /// Its workspace; none for the project's root.
    pub workspace: Option<String>,
    pub parent: Option<Parent>,
    pub facts: Facts,
    /// When it started, in seconds since 1970.
    pub started: Option<i64>,
    /// When it last moved on the board, in seconds since 1970.
    pub last_activity: Option<i64>,
    /// Its newest session record, by its file name in its workspace's `sessions/`.
    pub session_record: Option<String>,
}

/// What a chat with no id is named by: its number on this app's board.
pub fn number_id(chat: u32) -> String {
    format!("chat-{chat}")
}

/// What the query was asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Asked {
    pub history: bool,
    /// How many ended dispatches `history` lists.
    pub limit: u32,
}

/// **The parameters**, checked: `history`, true or false, and `limit`, 1 to [`HISTORY_MOST`].
pub fn asked(args: &Map<String, Value>) -> Result<Asked, String> {
    let history = match args.get("history") {
        None | Some(Value::Null) => false,
        Some(Value::Bool(history)) => *history,
        Some(_) => return Err("history is true or false".to_owned()),
    };
    let limit = match args.get("limit") {
        None | Some(Value::Null) => HISTORY_DEFAULT,
        Some(value) => value
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .filter(|n| (1..=HISTORY_MOST).contains(n))
            .ok_or_else(|| format!("limit is a whole number from 1 to {HISTORY_MOST}"))?,
    };
    Ok(Asked { history, limit })
}

/// The JSON Schema of what the query takes: its MCP tool's.
pub fn parameters() -> Value {
    json!({
        "type": "object",
        "properties": {
            "history": {
                "type": "boolean",
                "description": "Also list the workspace's ended dispatches (tasks and handoffs), \
                                the one that ended last first, each with the chat that asked \
                                for it, and each chat once. A closed \
                                chat that dispatched nothing is in no record, so it is not \
                                listed.",
            },
            "limit": {
                "type": "integer",
                "description": "With history: how many ended dispatches to list, from 1 to \
                                200. 50 when left out.",
            },
        },
        "additionalProperties": false,
    })
}

/// A time in seconds since 1970, as RFC 3339.
fn time(secs: Option<i64>) -> Value {
    secs.and_then(|secs| chrono::DateTime::from_timestamp(secs, 0))
        .map_or(Value::Null, |at| {
            json!(at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
        })
}

/// A dispatch record's stamp, as seconds since 1970.
fn stamped(stamp: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(stamp)
        .ok()
        .map(|at| at.timestamp())
}

/// One row of the answer, for a page of `workspace`. A parent that works elsewhere (another
/// workspace, or the project's root) is named by its relation only: its id is null, since a
/// page reads no chat of another workspace.
fn row(chat: &Chat, (mark, cause): (Mark, &str), open: bool, workspace: &str) -> Value {
    let parent = chat.parent.as_ref().map(|p| {
        let id = (p.workspace.as_deref() == Some(workspace)).then_some(p.id.as_str());
        json!({ "id": id, "relation": p.relation.word() })
    });
    json!({
        "id": chat.id,
        "name": chat.name,
        "persona": chat.persona,
        "harness": chat.harness,
        "state": mark.key(),
        "state_said": mark.said(),
        "cause": cause,
        "started": time(chat.started),
        "last_activity": time(chat.last_activity),
        "parent": parent,
        "session_record": chat.session_record,
        "open": open,
    })
}

/// A session record's project-relative path, as its file name, where it is one of
/// `workspace`'s own records.
pub fn record_file(path: &str, workspace: &str) -> Option<String> {
    let file = path.strip_prefix(&format!("workspaces/{workspace}/sessions/"))?;
    crate::sessionrecord::is_record_name(file).then(|| file.to_owned())
}

/// Which chat of a dispatch a record names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Side {
    Asker,
    Worker,
}

/// **The id each record names each of its chats by**: its own, or, for one given none, the
/// id-less chat it is shown to be, or else its number on the board of the run the record was
/// made in, after the record's id (a number alone may be another chat's in another run).
///
/// A chat given no id is shown to be:
/// - **the open chat with its number** whose run started no later than the record did, since
///   the app opens a dispatch's record once its chat has started, and a number is one chat's
///   at a time on a board;
/// - else **the one an earlier record named** with the same number, name, persona and
///   workspace, for the chat that asked: the chat that asked twice, or the task that went on to
///   ask one of its own. A dispatch's worker is a new chat at its record's start, so it is never
///   one an earlier record named, and the chats named after it by its number are it.
struct RecordedIds(std::collections::HashMap<(usize, Side), String>);

impl RecordedIds {
    fn of(open: &[Chat], records: &[Record]) -> RecordedIds {
        type Key<'a> = (Option<&'a str>, u32, &'a str, Option<&'a str>);
        let mut ids = std::collections::HashMap::new();
        let mut named: std::collections::HashMap<Key, String> = std::collections::HashMap::new();
        let mut by_start: Vec<(usize, &Record)> = records.iter().enumerate().collect();
        by_start.sort_by_key(|(_, r)| stamped(&r.started));
        for (at, record) in by_start {
            let started = stamped(&record.started);
            // The asker was there before the chat it started.
            for (side, chat, workspace) in [
                (Side::Asker, &record.asker.chat, &record.asker.workspace),
                (Side::Worker, &record.worker.chat, &record.place.workspace),
            ] {
                let id = match chat.id.as_deref().filter(|id| !id.is_empty()) {
                    Some(id) => id.to_owned(),
                    None => {
                        let key = (
                            workspace.as_deref(),
                            chat.chat,
                            chat.name.as_str(),
                            chat.persona.as_deref(),
                        );
                        let open_one = open.iter().find(|c| {
                            c.id == number_id(chat.chat)
                                && c.workspace == *workspace
                                && c.started.zip(started).is_some_and(|(c, r)| c <= r)
                        });
                        let id = match (open_one, named.get(&key)) {
                            (Some(open), _) => open.id.clone(),
                            (None, Some(earlier)) if side == Side::Asker => earlier.clone(),
                            _ => format!("{}.{}", record.id, number_id(chat.chat)),
                        };
                        named.insert(key, id.clone());
                        id
                    }
                };
                ids.insert((at, side), id);
            }
        }
        RecordedIds(ids)
    }

    /// The id the record at `at` names its chat on `side` by.
    fn of_chat(&self, at: usize, side: Side) -> String {
        self.0.get(&(at, side)).cloned().unwrap_or_default()
    }
}

/// When an ended dispatch ended, or, where its record says not, when it started.
fn ended_at(record: &Record) -> Option<i64> {
    record
        .ended
        .as_deref()
        .and_then(stamped)
        .or_else(|| stamped(&record.started))
}

/// An ended dispatch's chat, and the state its record says it ended in.
fn ended_worker(
    (at, record): (usize, &Record),
    ids: &RecordedIds,
    workspace: &str,
) -> (Chat, (Mark, &'static str)) {
    let outcome = record.report.as_ref().map(|r| r.outcome.word());
    let said = if record.did_not_start || failed_outcome(outcome) {
        (Mark::Failed, "reported-failed")
    } else if outcome.is_some_and(|o| BY_THE_PERSON.contains(&o)) {
        (Mark::Ended, "ended-by-person")
    } else if record.report.is_some() {
        (Mark::Ended, "reported")
    } else {
        (Mark::Ended, "exited")
    };
    let worker = &record.worker;
    let chat = Chat {
        id: ids.of_chat(at, Side::Worker),
        name: record
            .task
            .clone()
            .unwrap_or_else(|| worker.chat.name.clone()),
        persona: worker.chat.persona.clone(),
        harness: worker.harness.clone(),
        workspace: record.place.workspace.clone(),
        parent: Some(Parent {
            id: ids.of_chat(at, Side::Asker),
            relation: match record.mode {
                crate::dispatchrecord::Mode::Task => Relation::Dispatched,
                crate::dispatchrecord::Mode::Handoff => Relation::HandedOffFrom,
            },
            workspace: record.asker.workspace.clone(),
        }),
        facts: Facts::default(),
        started: stamped(&record.started),
        last_activity: record.ended.as_deref().and_then(stamped),
        session_record: worker
            .session_record
            .as_deref()
            .and_then(|path| record_file(path, workspace)),
    };
    (chat, said)
}

/// The chat that asked for an ended dispatch, closed.
fn closed_asker((at, record): (usize, &Record), ids: &RecordedIds, workspace: &str) -> Chat {
    let asker = &record.asker;
    Chat {
        id: ids.of_chat(at, Side::Asker),
        name: asker.chat.name.clone(),
        persona: asker.chat.persona.clone(),
        harness: None,
        workspace: asker.workspace.clone(),
        parent: None,
        facts: Facts::default(),
        started: None,
        last_activity: None,
        session_record: asker
            .session_record
            .as_deref()
            .and_then(|path| record_file(path, workspace)),
    }
}

/// How a chat is in the answer so far.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Listed {
    /// Open, or as the worker of an ended dispatch: what it is.
    AsItself,
    /// Only as the chat that asked for one, at this row.
    AsAsker(usize),
}

/// **The answer**: `open`'s chats of `workspace`, and, with `history`, the workspace's ended
/// dispatches of `records` that are not open, the one that ended last first, each followed by
/// the chat that asked it where that one is listed nowhere else. Each chat is listed once: one
/// first listed as an asker, and later found as an ended dispatch's worker, is listed as that.
pub fn answer(open: &[Chat], records: &[Record], workspace: &str, asked: Asked) -> Value {
    let here = |ws: &Option<String>| ws.as_deref() == Some(workspace);
    let mut listed: std::collections::HashMap<String, Listed> = std::collections::HashMap::new();
    let mut chats: Vec<Value> = Vec::new();
    for chat in open.iter().filter(|c| here(&c.workspace)) {
        listed.insert(chat.id.clone(), Listed::AsItself);
        chats.push(row(chat, mark(&chat.facts), true, workspace));
    }
    if asked.history {
        let mut ended: Vec<(usize, &Record)> = records
            .iter()
            .enumerate()
            .filter(|(_, r)| !r.running() && here(&r.place.workspace))
            .collect();
        // Stable, so dispatches that ended in one second keep the store's order.
        ended.sort_by_key(|(_, r)| std::cmp::Reverse(ended_at(r)));
        let ids = RecordedIds::of(open, records);
        let mut workers = 0;
        for dispatch in ended {
            if workers == asked.limit {
                break;
            }
            let (worker, said) = ended_worker(dispatch, &ids, workspace);
            let shown = row(&worker, said, false, workspace);
            match listed.get(&worker.id).copied() {
                Some(Listed::AsItself) => continue,
                Some(Listed::AsAsker(at)) => chats[at] = shown,
                None => chats.push(shown),
            }
            listed.insert(worker.id.clone(), Listed::AsItself);
            workers += 1;
            let asker = closed_asker(dispatch, &ids, workspace);
            if here(&asker.workspace) && !listed.contains_key(&asker.id) {
                listed.insert(asker.id.clone(), Listed::AsAsker(chats.len()));
                chats.push(row(&asker, (Mark::Ended, "closed"), false, workspace));
            }
        }
    }
    json!({ "history": asked.history, "chats": chats })
}

#[cfg(test)]
mod tests;

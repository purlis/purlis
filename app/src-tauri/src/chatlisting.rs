// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The app's answer to `purlis.chats`** (#6235): the chats of one workspace, gathered from
//! the app's own record (the one the window's Chats tree is drawn from) and the board, and
//! shaped by [`purlis_core::catalogue::chats`]. A page asks it through the artifact's port, and
//! a chat through the `chats` tool over its hook socket ([`Ask::Chats`]).
//!
//! **When a chat last moved** is this process's own note of the board's moves ([`moved`]),
//! held in memory and never written: a chat that has not moved since the app started says
//! none.
//!
//! [`Ask::Chats`]: purlis_core::hookwire::Ask::Chats

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use purlis_core::catalogue::chats::{self, Asked, Facts, Report, TaskFacts};
use purlis_core::reopen::{HandedFrom, Mode, Owed};
use purlis_core::state::State;
use serde_json::Value;

use crate::planes::{Held, PlaneId};

/// When each chat last moved on its board, by project and number, in seconds since 1970.
fn last_moves() -> &'static Mutex<HashMap<(PlaneId, u32), i64>> {
    static LAST: OnceLock<Mutex<HashMap<(PlaneId, u32), i64>>> = OnceLock::new();
    LAST.get_or_init(Mutex::default)
}

/// **Chat `session` of `plane` moved on its board now.**
pub fn moved(plane: &PlaneId, session: u32) {
    let now = chrono::Utc::now().timestamp();
    last_moves()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert((plane.clone(), session), now);
}

fn last_moved(plane: &PlaneId, session: u32) -> Option<i64> {
    last_moves()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(&(plane.clone(), session))
        .copied()
}

/// A task's record, as the facts read it: its report and how it ended.
fn task_facts(
    from: Option<&HandedFrom>,
    outcome: impl FnOnce() -> Option<String>,
) -> Option<TaskFacts> {
    let from = from.filter(|from| from.mode == Mode::Task)?;
    let report = match from.report {
        Owed::Sent | Owed::Again => Report::Sent,
        Owed::Failed => Report::Failed,
        Owed::Due | Owed::Nothing => Report::Owed,
    };
    let outcome = (report != Report::Owed).then(outcome).flatten();
    Some(TaskFacts { report, outcome })
}

/// **The `purlis.chats` data of workspace `workspace`** in the project `held`, `plane`.
pub fn answer(held: &Held, plane: &PlaneId, workspace: &str, asked: Asked) -> Value {
    let board = held.board();
    let mut outcomes = crate::dispatches::Outcomes::of(held);
    let facts_of = |session: u32, from: Option<&HandedFrom>, shell: bool| {
        let glance = board.glance(session);
        let (stuck, left_work) = held.noticed().told_of(session);
        Facts {
            board: (!(shell && glance.state == State::Unknown)).then_some(glance.state),
            needs_you: board.now(session).needs_you,
            stuck,
            task: task_facts(from, || None),
            tasks_at_work: 0,
            own_work: glance.own_work,
            left_work,
        }
    };
    let mut open = held
        .chats()
        .listed(&facts_of, &|session| last_moved(plane, session));
    // How each task ended, by its record, and the tasks at work below each chat.
    let working: Vec<String> = open
        .iter()
        .filter(|(_, chat)| chat.facts.board == Some(State::Running))
        // Only a task is below its asker: a handoff's chat is in no chat's count (#1492).
        .filter_map(|(_, chat)| chat.parent.as_ref())
        .filter(|p| p.relation == chats::Relation::Dispatched)
        .map(|p| p.id.clone())
        .collect();
    let ledger = purlis_core::dispatchrecord::ledger(held.root());
    for (session, chat) in &mut open {
        chat.facts.tasks_at_work =
            u32::try_from(working.iter().filter(|id| **id == chat.id).count()).unwrap_or(u32::MAX);
        if let Some(task) = chat.facts.task.as_mut()
            && task.report != Report::Owed
        {
            task.outcome = outcomes.of_task(*session).map(str::to_owned);
        }
        let record = crate::dispatches::chat_ref(held, *session)
            .and_then(|me| ledger.latest_for(held.root(), &me));
        chat.session_record = record
            .and_then(|r| r.worker.session_record)
            .and_then(|path| chats::record_file(&path, workspace));
    }
    let records = if asked.history {
        purlis_core::dispatchrecord::drawn(held.root()).records
    } else {
        Vec::new()
    };
    let open: Vec<chats::Chat> = open.into_iter().map(|(_, chat)| chat).collect();
    chats::answer(&open, &records, workspace, asked)
}

/// **The `chats` tool's answer for chat `asker`**: its own workspace's chats, by this app's
/// record of where it works, never by anything the line says.
pub fn for_chat(
    held: &Held,
    plane: &PlaneId,
    ask: &purlis_core::hookwire::ChatsAsk,
) -> Result<Value, String> {
    let Some(workspace) = held.chats().workspace_of(ask.chat) else {
        return Err(format!("chat {} is not one this app has open", ask.chat));
    };
    let Some(workspace) = workspace else {
        return Err(
            "chats lists a workspace's chats, and this chat works at the project's root".to_owned(),
        );
    };
    let asked = Asked {
        history: ask.history,
        limit: ask.limit.unwrap_or(chats::HISTORY_DEFAULT),
    };
    if !(1..=chats::HISTORY_MOST).contains(&asked.limit) {
        return Err(format!(
            "limit is a whole number from 1 to {}",
            chats::HISTORY_MOST
        ));
    }
    Ok(answer(held, plane, &workspace, asked))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn from(mode: Mode, report: Owed) -> HandedFrom {
        HandedFrom {
            chat: 3,
            id: None,
            name: "asker".to_owned(),
            workspace: purlis_core::active::Place::Workspace("alpha".to_owned()),
            report,
            mode,
            depth: 1,
            root: None,
            above: None,
            by_person: false,
            long: false,
        }
    }

    #[test]
    fn a_task_s_report_is_read_off_its_lineage_and_its_outcome_only_once_it_has_one() {
        let outcome = || Some("done".to_owned());
        assert_eq!(
            task_facts(Some(&from(Mode::Task, Owed::Due)), outcome),
            Some(TaskFacts {
                report: Report::Owed,
                outcome: None
            })
        );
        for sent in [Owed::Sent, Owed::Again] {
            assert_eq!(
                task_facts(Some(&from(Mode::Task, sent)), outcome),
                Some(TaskFacts {
                    report: Report::Sent,
                    outcome: Some("done".to_owned())
                })
            );
        }
        assert_eq!(
            task_facts(Some(&from(Mode::Task, Owed::Failed)), || None).map(|t| t.report),
            Some(Report::Failed)
        );
    }

    #[test]
    fn a_handoff_s_chat_and_a_chat_the_person_opened_are_no_task() {
        assert_eq!(
            task_facts(Some(&from(Mode::Handoff, Owed::Nothing)), || None),
            None
        );
        assert_eq!(task_facts(None, || None), None);
    }
}

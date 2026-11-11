// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The app's answer to `purlis.chats`** (#6235): the chats of one workspace, gathered from
//! the app's own record (the one the window's Chats tree is drawn from) and the board, and
//! shaped by [`purlis_core::catalogue::chats`]. A page asks it through the artifact's port, and
//! a chat through the `chats` tool over its hook socket ([`Ask::Chats`]).
//!
//! **When a chat last moved** is this process's own note of the board's moves ([`moved`]),
//! held in memory and never written, and let go of when the chat closes ([`closed`]): a chat
//! that has not moved since the app started says none.
//!
//! **History is read from the dispatch records once they change** ([`Drawing`]), not at each
//! move of a board.
//!
//! [`Ask::Chats`]: purlis_core::hookwire::Ask::Chats

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use purlis_core::catalogue::chats::{self, Asked, Chat, Facts, Report, TaskFacts};
use purlis_core::dispatchrecord::Drawing;
use purlis_core::reopen::{HandedFrom, Mode, Owed};
use purlis_core::state::State;
use serde_json::Value;

use crate::planes::{Held, PlaneId};

/// When each chat last moved on its board, by project and number, in seconds since 1970.
fn last_moves() -> &'static Mutex<HashMap<(PlaneId, u32), i64>> {
    static LAST: OnceLock<Mutex<HashMap<(PlaneId, u32), i64>>> = OnceLock::new();
    LAST.get_or_init(Mutex::default)
}

/// Each project's dispatch records, as history last drew them.
fn drawings() -> &'static Mutex<HashMap<PlaneId, Drawing>> {
    static DRAWN: OnceLock<Mutex<HashMap<PlaneId, Drawing>>> = OnceLock::new();
    DRAWN.get_or_init(Mutex::default)
}

/// **Chat `session` of `plane` moved on its board now.**
pub fn moved(plane: &PlaneId, session: u32) {
    let now = chrono::Utc::now().timestamp();
    last_moves()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert((plane.clone(), session), now);
}

/// **Chat `session` of `plane` closed**: when it last moved is let go of, so a number given
/// to another chat later starts with none.
pub fn closed(plane: &PlaneId, session: u32) {
    last_moves()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .remove(&(plane.clone(), session));
}

fn last_moved(plane: &PlaneId, session: u32) -> Option<i64> {
    last_moves()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(&(plane.clone(), session))
        .copied()
}

/// **The workspaces whose chats a move of chat `session` may change**: its own, and that of
/// the chat it came from, whose tasks at work it is counted in. `None` where it is not open
/// (it closed): any workspace's may.
pub fn moved_in(held: &Held, session: u32) -> Option<Vec<String>> {
    let chats = held.chats();
    let own = chats.workspace_of(session)?;
    let above = chats
        .handed_from(session)
        .and_then(|from| match from.workspace {
            purlis_core::active::Place::Workspace(name) => Some(name),
            purlis_core::active::Place::PlaneRoot => None,
        });
    Some(own.into_iter().chain(above).collect())
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

/// **The tasks at work below each chat of `open`**, whatever workspace either works in: the
/// running tasks that name it as their asker. Only a task is below its asker: a handoff's
/// chat is in no chat's count (#1492).
fn count_tasks_at_work(open: &mut [(u32, Chat)]) {
    let working: Vec<String> = open
        .iter()
        .filter(|(_, chat)| chat.facts.board == Some(State::Running))
        .filter_map(|(_, chat)| chat.parent.as_ref())
        .filter(|p| p.relation == chats::Relation::Dispatched)
        .map(|p| p.id.clone())
        .collect();
    for (_, chat) in open {
        chat.facts.tasks_at_work =
            u32::try_from(working.iter().filter(|id| **id == chat.id).count()).unwrap_or(u32::MAX);
    }
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
    count_tasks_at_work(&mut open);
    // How each task ended, by its record, and each chat's newest session record.
    let ledger = purlis_core::dispatchrecord::ledger(held.root());
    for (session, chat) in &mut open {
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
    let open: Vec<Chat> = open.into_iter().map(|(_, chat)| chat).collect();
    if !asked.history {
        return chats::answer(&open, &[], workspace, asked);
    }
    let mut drawings = drawings()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let records = &drawings
        .entry(plane.clone())
        .or_default()
        .drawn(held.root())
        .records;
    chats::answer(&open, records, workspace, asked)
}

/// **The workspace a `chats` ask lists, and what it asks**: the asking chat's workspace as
/// this app's record has it (`recorded`: none where the chat is not open, `Some(None)` at the
/// project's root), never anything the line says.
fn scoped(
    recorded: Option<Option<String>>,
    ask: &purlis_core::hookwire::ChatsAsk,
) -> Result<(String, Asked), String> {
    let Some(workspace) = recorded else {
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
    Ok((workspace, asked))
}

/// **The `chats` tool's answer for chat `asker`**: its own workspace's chats, by this app's
/// record of where it works, never by anything the line says.
pub fn for_chat(
    held: &Held,
    plane: &PlaneId,
    ask: &purlis_core::hookwire::ChatsAsk,
) -> Result<Value, String> {
    let (workspace, asked) = scoped(held.chats().workspace_of(ask.chat), ask)?;
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

    fn plane(name: &str) -> PlaneId {
        serde_json::from_value(serde_json::json!(name)).expect("a plane id")
    }

    #[test]
    fn a_chat_that_closed_no_longer_says_when_it_last_moved() {
        let plane = plane("a-plane-whose-chat-closes");
        moved(&plane, 4);
        assert!(last_moved(&plane, 4).is_some());
        closed(&plane, 4);
        assert_eq!(last_moved(&plane, 4), None);
    }

    fn ask(chat: u32, limit: Option<u32>) -> purlis_core::hookwire::ChatsAsk {
        purlis_core::hookwire::ChatsAsk {
            chat,
            history: true,
            limit,
        }
    }

    #[test]
    fn the_chats_tool_lists_the_asking_chat_s_recorded_workspace_and_refuses_any_other_chat() {
        assert_eq!(
            scoped(Some(Some("alpha".to_owned())), &ask(3, None)),
            Ok((
                "alpha".to_owned(),
                Asked {
                    history: true,
                    limit: chats::HISTORY_DEFAULT
                }
            ))
        );
        assert_eq!(
            scoped(None, &ask(9, None)),
            Err("chat 9 is not one this app has open".to_owned()),
            "a chat this app does not have open"
        );
        let at_root = scoped(Some(None), &ask(3, None)).unwrap_err();
        assert!(at_root.contains("project's root"), "{at_root}");
        for limit in [0, chats::HISTORY_MOST + 1] {
            assert!(scoped(Some(Some("alpha".to_owned())), &ask(3, Some(limit))).is_err());
        }
    }

    fn chat(
        id: &str,
        workspace: &str,
        board: State,
        parent: Option<(&str, chats::Relation)>,
    ) -> Chat {
        Chat {
            id: id.to_owned(),
            name: id.to_owned(),
            persona: None,
            harness: Some("claude".to_owned()),
            workspace: Some(workspace.to_owned()),
            parent: parent.map(|(id, relation)| chats::Parent {
                id: id.to_owned(),
                relation,
                workspace: Some("alpha".to_owned()),
            }),
            facts: Facts {
                board: Some(board),
                ..Facts::default()
            },
            started: None,
            last_activity: None,
            session_record: None,
        }
    }

    #[test]
    fn a_chat_counts_its_running_tasks_in_any_workspace_and_no_handoff() {
        use chats::Relation::{Dispatched, HandedOffFrom};
        let mut open = vec![
            (1, chat("asker", "alpha", State::Waiting, None)),
            (
                2,
                chat(
                    "task-here",
                    "alpha",
                    State::Running,
                    Some(("asker", Dispatched)),
                ),
            ),
            (
                3,
                chat(
                    "task-in-beta",
                    "beta",
                    State::Running,
                    Some(("asker", Dispatched)),
                ),
            ),
            (
                4,
                chat(
                    "task-at-rest",
                    "beta",
                    State::Waiting,
                    Some(("asker", Dispatched)),
                ),
            ),
            (
                5,
                chat(
                    "handed-off",
                    "beta",
                    State::Running,
                    Some(("asker", HandedOffFrom)),
                ),
            ),
        ];
        count_tasks_at_work(&mut open);
        let counts: Vec<u32> = open.iter().map(|(_, c)| c.facts.tasks_at_work).collect();
        assert_eq!(counts, [2, 0, 0, 0, 0]);
    }

    /// A project the app holds, with a shell chat at its root. It opens a terminal and the
    /// project's hook socket, which a sandboxed chat may not: CI runs it.
    fn a_held_chat_at_the_root() -> (tempfile::TempDir, std::sync::Arc<Held>, PlaneId, u32) {
        let dir = tempfile::tempdir().expect("a directory");
        let root = dir.path().join("project");
        std::fs::create_dir_all(root.join("workspaces").join("alpha")).expect("a workspace");
        std::fs::write(purlis_core::names::manifest(&root), "schema = 1\n").expect("the project");
        let planes = crate::planes::Planes::telling(
            std::sync::Arc::new(|_| {}),
            crate::Shipped::default(),
            None,
        );
        let id = planes.open(&root);
        let held = planes.held(&id).expect("held");
        let chat = purlis_core::reopen::Chat {
            program: "/bin/sh".to_owned(),
            args: vec!["-c".to_owned(), "sleep 5".to_owned()],
            name: "sh".to_owned(),
            ..Default::default()
        };
        let size = purlis_core::engine::Size {
            columns: 80,
            rows: 24,
        };
        let session = held.chats().start(&chat, size).expect("it runs");
        (dir, held, id, session)
    }

    #[test]
    fn for_chat_refuses_a_chat_it_does_not_have_open_and_one_at_the_root() {
        let (_dir, held, plane, session) = a_held_chat_at_the_root();
        let unknown = for_chat(&held, &plane, &ask(session + 100, None)).unwrap_err();
        assert!(unknown.contains("not one this app has open"), "{unknown}");
        let at_root = for_chat(&held, &plane, &ask(session, None)).unwrap_err();
        assert!(at_root.contains("project's root"), "{at_root}");
        // A workspace's answer leaves the root's chat out.
        let data = answer(
            &held,
            &plane,
            "alpha",
            Asked {
                history: true,
                limit: 50,
            },
        );
        assert_eq!(data["chats"], serde_json::json!([]));
    }
}

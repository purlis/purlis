// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A task on a pool's runner follows the task route** (#4560, ADR 0107): its question,
//! heard over the runner's link, goes to its asking chat, which answers it or passes it on; and
//! a link to the asking chat that is lost sends the question to the person, never nowhere.
//!
//! The seam is the app's answer to each line as the pool's link hands it on
//! (`crate::hooks::heard_from`), over a session host that runs its chats on that pool. The link
//! itself, and the runner's host, are not here: what reaches this seam is what the desktop's
//! host hands on, word for word.

use super::*;

use crate::asking::{AnswerPath, AskSource, ReachedWhy, every_ask};
use purlis_core::dispatchtalk::Kind;
use std::time::SystemTime;

const POOL: &str = "builds";

/// A `steward` chat and a task it dispatched, both on pool [`POOL`]'s runner, the task at work.
fn a_task_on_a_runner() -> (Plane, Pretend, Planes, PlaneId, Arc<Held>, u32, u32) {
    let plane = a_plane_with_personas();
    let host = Pretend::default();
    let (planes, id, steward) = a_steward_chat(&host, &plane);
    let held = planes.held(&id).expect("held");
    let task = a_task_of(&held, &id, steward, "check prod");
    // Both run on the pool's runner, as its router says.
    host.runs_on_pool(steward, POOL);
    host.runs_on_pool(task, POOL);
    works(&held, task);
    (plane, host, planes, id, held, steward, task)
}

/// Chat `chat`'s ask, heard over pool [`POOL`]'s link.
fn over_the_link(held: &Held, id: &PlaneId, chat: u32, what: What) -> Answer {
    let (held, id) = (kept(held), id.clone());
    let said = format!("chat {chat}'s ask over the runner's link ({what:?})");
    bounded(&said, move || {
        crate::hooks::heard_from(Some(POOL.to_owned()), || {
            answer(
                &held,
                &id,
                &Tickets::default(),
                1,
                Ask::Task(Box::new(Asked { chat, what })),
                &nothing_opens,
            )
        })
    })
}

/// Task `task` asks `text`, over the link, as its harness's question or `dispatch_ask` does.
fn asks_over_the_link(held: &Held, id: &PlaneId, task: u32, text: &str) -> Answer {
    over_the_link(
        held,
        id,
        task,
        What::Route {
            text: text.to_owned(),
        },
    )
}

/// What is waiting on the person of `task`'s question.
fn asks_of(held: &Held, task: u32) -> Vec<crate::asking::Shown> {
    every_ask(held)
        .asks
        .into_iter()
        .filter(|ask| ask.session == task && ask.source == AskSource::TaskQuestion)
        .collect()
}

/// The clock, `secs` from now.
fn in_secs(secs: u64) -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("the clock")
        .as_secs()
        + secs
}

#[test]
fn a_task_on_a_runner_asks_its_asking_chat_and_hears_its_answer_as_the_chat_s() {
    let (_plane, _host, _planes, id, held, steward, task) = a_task_on_a_runner();

    let said = asks_over_the_link(&held, &id, task, "Which one?");
    assert!(
        matches!(&said, Answer::Task(answered)
            if matches!(**answered, Answered::Sent { kind: Kind::Question, .. })),
        "the question goes to its asking chat, not {said:?}"
    );
    over_the_link(
        &held,
        &id,
        steward,
        What::Answer {
            to: task,
            text: "The blue one".to_owned(),
        },
    );

    let given = held.tasks().ledger().talk.answered(task).cloned();
    assert!(
        given
            .as_ref()
            .is_some_and(|given| given.text == "The blue one" && !given.by_person),
        "answered by its asking chat, never as the person's word: {given:?}"
    );
}

#[test]
fn a_task_on_a_runner_s_question_passed_on_is_one_ask_of_the_person() {
    let (_plane, _host, _planes, id, held, steward, task) = a_task_on_a_runner();
    asks_over_the_link(&held, &id, task, "Which one?");

    over_the_link(
        &held,
        &id,
        steward,
        What::PassOn {
            to: task,
            note: None,
        },
    );

    let listed = asks_of(&held, task);
    assert_eq!(listed.len(), 1, "{listed:?}");
    assert_eq!(listed[0].says, "Which one?");
    assert!(matches!(
        &listed[0].answer,
        AnswerPath::TaskQuestion {
            why: ReachedWhy::PassedOn,
            ..
        }
    ));
}

#[test]
fn a_question_whose_asking_chat_s_link_is_lost_reaches_the_person_after_its_wait() {
    let (_plane, host, _planes, id, held, steward, task) = a_task_on_a_runner();
    asks_over_the_link(&held, &id, task, "Which one?");

    // The asking chat can take no turn while its runner's link is down, so none hands it the
    // question: the wait starts from the look that finds the link lost.
    host.link_down(&[steward]);
    crate::dispatched::look_at_questions(&held);
    let gone = std::collections::HashSet::new();
    held.tasks().ledger().talk.look(
        in_secs(purlis_core::dispatchtalk::A_QUESTION_WAITS_SECS + 1),
        &gone,
        &gone,
    );

    let listed = asks_of(&held, task);
    assert_eq!(listed.len(), 1, "{listed:?}");
    assert!(matches!(
        &listed[0].answer,
        AnswerPath::TaskQuestion {
            why: ReachedWhy::NoReply,
            ..
        }
    ));
}

#[test]
fn a_link_lost_only_a_moment_leaves_the_question_with_its_asking_chat() {
    let (_plane, host, _planes, id, held, steward, task) = a_task_on_a_runner();
    asks_over_the_link(&held, &id, task, "Which one?");

    host.link_down(&[steward]);
    crate::dispatched::look_at_questions(&held);

    assert!(asks_of(&held, task).is_empty(), "the wait has only begun");
    assert_eq!(
        held.tasks().ledger().talk.standing(task),
        Some(&purlis_core::dispatchtalk::Standing::WithAskingChat)
    );
}

#[test]
fn a_question_whose_asking_chat_went_with_its_runner_reaches_the_person_at_once() {
    let (_plane, host, _planes, id, held, steward, task) = a_task_on_a_runner();
    asks_over_the_link(&held, &id, task, "Which one?");

    host.program_ends_with_its_runner(steward);

    let listed = asks_of(&held, task);
    assert_eq!(listed.len(), 1, "{listed:?}");
    assert!(matches!(
        &listed[0].answer,
        AnswerPath::TaskQuestion {
            why: ReachedWhy::AskerGone,
            ..
        }
    ));
}

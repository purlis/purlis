// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A message a chat on a pool's runner queued there, carried by the runner's host** (#4804):
//! the queue is on the runner's disk, so the runner's host sends each message on itself
//! ([`What::Carried`]), and the app delivers it as it delivers one from the queue beside it.
//!
//! The seam is the app's answer to each line as the pool's link hands it on
//! (`crate::hooks::heard_from`). The runner's host, and its queue, are
//! `purlis_core::hookwire::relay`'s, tested there.

use super::*;

use purlis_core::dispatchqueue::Queued;

const POOL: &str = "builds";

/// Chat `chat`'s queued message `id`, carried over pool [`POOL`]'s link.
fn carried(held: &Held, id: &PlaneId, chat: u32, message_id: &str, message: Queued) -> Answer {
    let (held, id) = (kept(held), id.clone());
    let said = format!("chat {chat}'s message {message_id}, carried ({message:?})");
    let message_id = message_id.to_owned();
    bounded(&said, move || {
        crate::hooks::heard_from(Some(POOL.to_owned()), || {
            answer(
                &held,
                &id,
                &Tickets::default(),
                1,
                Ask::Task(Box::new(Asked {
                    chat,
                    what: What::Carried {
                        id: message_id,
                        message,
                    },
                })),
                &nothing_opens,
            )
        })
    })
}

fn a_tell(to: u32, text: &str) -> Queued {
    Queued::Tell {
        to,
        text: text.to_owned(),
    }
}

#[test]
fn a_task_on_a_runner_asks_and_notes_through_its_queue_and_is_answered() {
    let (_plane, _planes, id, held, asking, task) = a_dispatched_task();

    let asked = carried(
        &held,
        &id,
        task,
        "0000000000000000000000000000000a",
        Queued::Question {
            text: "Which one?".to_owned(),
        },
    );
    let noted = carried(
        &held,
        &id,
        task,
        "0000000000000000000000000000000b",
        Queued::Note {
            text: Some("halfway".to_owned()),
            ack: None,
        },
    );
    carried(
        &held,
        &id,
        asking,
        "0000000000000000000000000000000c",
        Queued::Answer {
            to: task,
            text: "The blue one".to_owned(),
        },
    );

    assert!(
        matches!(&asked, Answer::Task(answered)
            if matches!(**answered, Answered::Sent { kind: Kind::Question, .. })),
        "the question goes to its asking chat, not {asked:?}"
    );
    assert_eq!(noted, sent(Kind::Note, "steward 1"));
    let given = held.tasks().ledger().talk.answered(task).cloned();
    assert!(
        given.is_some_and(|given| given.text == "The blue one" && !given.by_person),
        "answered by its asking chat"
    );
}

#[test]
fn a_message_carried_again_after_its_answer_was_lost_is_delivered_once() {
    let (_plane, _planes, id, held, asking, task) = a_dispatched_task();
    let message = "0123456789abcdef0123456789abcdef";

    let first = carried(&held, &id, asking, message, a_tell(task, "first"));
    let again = carried(&held, &id, asking, message, a_tell(task, "first"));

    assert_eq!(first, told("check the queue", 1));
    assert_eq!(
        again,
        Answer::Task(Box::new(Answered::Queued {
            id: message.to_owned(),
            delivered: true,
        }))
    );
    assert_eq!(dispatchtalk::take(held.root(), task).len(), 1, "left once");
}

#[test]
fn a_carried_message_a_limit_holds_back_is_answered_still_queued_and_not_delivered() {
    let (_plane, _planes, id, held, asking, task) = a_dispatched_task();
    for n in 0..10 {
        asks(&held, &id, asking, tell(task, &format!("follow-up {n}")));
    }
    let message = "0123456789abcdef0123456789abcdef";

    let said = carried(&held, &id, asking, message, a_tell(task, "one too many"));

    assert_eq!(
        said,
        Answer::Task(Box::new(Answered::Queued {
            id: message.to_owned(),
            delivered: false,
        })),
        "it stays on the runner, for the next carry"
    );
    assert_eq!(dispatchtalk::take(held.root(), task).len(), 10);
}

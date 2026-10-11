//! A persona chat coming to wait on the person counts once on its dispatch's record, heard at
//! the hook seam (#1457, #1452).
//!
//! The counter (`dispatches::needed_you`) and the record's own `note` are tested apart, beside
//! `a_dispatched_task`. This is the join: the task's own harness reports over the project's hook
//! socket, the board takes each report, and the listener the project wires at its open counts
//! the wait. Nothing here calls the counter. The socket is a real one, so it first runs on CI.

use purlis_core::harness::model::Prompt;
use purlis_core::hookwire::{Conversation, Report};
use purlis_core::state::{Detail, Event, Notified, State};

use super::*;

/// `event` from `task`'s own harness, in the conversation the board holds for it, over the
/// project's hook socket.
fn reports(held: &Held, task: u32, event: Event, detail: Detail) {
    let conversation = held
        .hooks()
        .board()
        .conversation(task)
        .map(str::to_owned)
        .expect("the task was started under a conversation purlis chose");
    purlis_core::hookwire::send(
        held.hooks().socket().expect("the project listens"),
        Some(&held.hooks().token_for(task)),
        &Report {
            chat: task,
            event,
            conversation: Conversation::Named(conversation),
            pid: Some(4242),
            agent: None,
            detail,
        },
    )
    .expect("the report is sent");
}

/// A mid-turn ask of kind `prompt`, as the harness's notification says it.
fn asks(prompt: Prompt) -> Detail {
    Detail {
        notified: Notified::Asks(prompt),
        ..Detail::default()
    }
}

/// Waits until `holds` does, or says `what` never came.
fn until(what: &str, holds: impl Fn() -> bool) {
    let deadline = Instant::now() + std::time::Duration::from_secs(10);
    while !holds() {
        assert!(Instant::now() < deadline, "{what} never came");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

#[test]
fn a_persona_chat_waiting_on_the_person_counts_once_however_often_it_reports() {
    let (_plane, _planes, _id, held, _asking, task) = a_dispatched_task();
    assert_eq!(record_of(&held, task).needed_you, 0);

    // Its turn begins: running, and nobody is needed.
    reports(&held, task, Event::UserPromptSubmit, Detail::default());
    until("the turn", || {
        held.hooks().board().state(task) == State::Running
    });

    // It asks for a permission mid-turn: it waits on the person, once.
    reports(&held, task, Event::Notification, asks(Prompt::Permission));
    until("the first wait on the record", || {
        record_of(&held, task).needed_you == 1
    });
    assert!(held.hooks().board().needs_you().contains(&task));

    // It reports again while it still waits (another prompt): the same wait.
    reports(&held, task, Event::Notification, asks(Prompt::Question));
    until("the second prompt on the board", || {
        held.hooks().board().its_prompt(task) == Some(Prompt::Question)
    });

    // It is answered and goes on, then waits again: a second wait.
    reports(&held, task, Event::UserPromptSubmit, Detail::default());
    until("the next turn", || {
        held.hooks().board().state(task) == State::Running
    });
    reports(&held, task, Event::Notification, asks(Prompt::Permission));
    until("the second wait on the record", || {
        record_of(&held, task).needed_you >= 2
    });
    // Anything still on its way has had time to land: the report made while waiting was not
    // counted.
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert_eq!(record_of(&held, task).needed_you, 2);
}

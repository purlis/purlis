// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A background turn doesn't clear a wait** (#4562): a turn that runs while a chat waits on
//! the person leaves their wait, its state and its needs-you item as they were.

use super::*;

/// A chat whose turn ended on the person: they have the next move.
fn waiting_on_the_person() -> Chat {
    let mut chat = Chat::new();
    chat.reported(Event::SessionStart);
    chat.reported(Event::UserPromptSubmit);
    chat.reported(Event::Stop);
    assert!(chat.needs_you());
    chat
}

fn background_turn_begins(chat: &mut Chat) -> bool {
    chat.heard_while(
        &Event::UserPromptSubmit.said(Detail::default()),
        Waits {
            background_turn: true,
            ..Waits::default()
        },
    )
}

#[test]
fn a_background_turn_leaves_a_wait_on_the_person_as_it_was() {
    let mut chat = waiting_on_the_person();

    background_turn_begins(&mut chat);

    assert!(chat.needs_you(), "the wait went with the turn");
    assert_eq!(chat.shown_state(), State::Waiting);
    assert_eq!(chat.state(), State::Running, "the chat really is at work");
}

#[test]
fn the_person_s_reply_clears_the_wait() {
    let mut chat = waiting_on_the_person();

    chat.reported(Event::UserPromptSubmit);

    assert!(!chat.needs_you());
    assert_eq!(chat.shown_state(), State::Running);
}

#[test]
fn the_person_s_reply_after_a_background_turn_clears_the_wait() {
    let mut chat = waiting_on_the_person();
    background_turn_begins(&mut chat);
    chat.reported(Event::Stop);
    assert!(chat.needs_you());

    chat.reported(Event::UserPromptSubmit);

    assert!(!chat.needs_you());
    assert_eq!(chat.shown_state(), State::Running);
}

#[test]
fn the_person_s_reply_sent_during_a_background_turn_clears_the_wait_at_once() {
    let mut chat = waiting_on_the_person();
    background_turn_begins(&mut chat);

    assert!(chat.replied(), "the window must hear of it");

    assert!(!chat.needs_you());
    assert_eq!(chat.shown_state(), State::Running);
    assert!(!chat.background_turn());
}

#[test]
fn a_reply_with_no_background_turn_running_moves_nothing() {
    let mut chat = waiting_on_the_person();

    assert!(!chat.replied(), "the turn it begins says so");

    assert!(chat.needs_you());
}

#[test]
fn a_background_turn_s_end_leaves_the_wait_as_it_was_and_raises_nothing_new() {
    let mut chat = waiting_on_the_person();
    background_turn_begins(&mut chat);

    let moved = chat.heard_while(
        &Event::Stop.said(Detail::default()),
        Waits {
            tasks: 2,
            ..Waits::default()
        },
    );

    assert!(
        chat.needs_you(),
        "other tasks at work must not hide the wait"
    );
    assert_eq!(chat.shown_state(), State::Waiting);
    assert!(!chat.background_turn());
    assert!(
        moved,
        "the turn it ran is over, which a pause owed waits for"
    );
}

#[test]
fn a_background_turn_is_handed_its_reports_and_keeps_what_the_person_owes() {
    let mut chat = waiting_on_the_person();
    chat.reported_back("task");
    chat.commit_refused("a key in a file");
    chat.needs_the_person(Need::ReportUndelivered {
        asker: "chat".into(),
    });

    background_turn_begins(&mut chat);

    assert!(chat.reports().is_empty(), "the turn was handed its reports");
    assert_eq!(chat.refusals().len(), 1);
    assert_eq!(chat.needs().len(), 1);
}

#[test]
fn a_background_turn_over_no_wait_is_only_a_turn() {
    let mut chat = waiting_on_the_person();
    chat.ignored();

    background_turn_begins(&mut chat);

    assert!(!chat.needs_you(), "ignored stays ignored");
    assert_eq!(chat.shown_state(), State::Running);
    assert!(!chat.background_turn());
}

#[test]
fn ignore_during_a_background_turn_drops_the_item_and_shows_the_turn() {
    let mut chat = waiting_on_the_person();
    background_turn_begins(&mut chat);

    chat.ignored();

    assert!(!chat.needs_you());
    assert_eq!(chat.shown_state(), State::Running);
}

#[test]
fn answering_a_prompt_a_background_turn_asked_leaves_the_earlier_wait() {
    let mut chat = waiting_on_the_person();
    background_turn_begins(&mut chat);
    chat.reported(Event::Notification);
    assert!(chat.waits_on_its_prompt());

    chat.answered();

    assert!(chat.needs_you(), "the question it waited on is unanswered");
    assert!(chat.background_turn());
}

#[test]
fn the_board_shows_the_wait_through_a_background_turn_and_a_reply_ends_it() {
    let report = |event| crate::hookwire::Report {
        agent: None,
        chat: 3,
        event,
        conversation: crate::hookwire::Conversation::Unknown,
        pid: None,
        detail: Detail::default(),
    };
    let mut board = Board::new();
    board.opened(3, None, None);
    board.reported(&report(Event::UserPromptSubmit));
    board.reported(&report(Event::Stop));
    assert!(board.needs_you().contains(&3));

    board.reported_while(
        &report(Event::UserPromptSubmit),
        Waits {
            background_turn: true,
            ..Waits::default()
        },
    );

    assert!(board.needs_you().contains(&3));
    assert_eq!(board.shown_state(3), State::Waiting);
    assert_eq!(board.state(3), State::Running);
    assert!(board.background_turn(3));

    assert!(board.replied(3));

    assert!(!board.needs_you().contains(&3));
    assert_eq!(board.shown_state(3), State::Running);
    assert!(!board.background_turn(3));
}

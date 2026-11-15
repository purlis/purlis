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

/// The clock a turn's end on its own work is stamped at, in seconds since 1970.
const NOW: i64 = 1_800_000_000;

/// A chat whose turn ended on a background command of its own, with a limit on the wait.
fn waiting_on_its_own_shell() -> Chat {
    let mut chat = Chat::new();
    chat.reported(Event::SessionStart);
    chat.reported(Event::UserPromptSubmit);
    chat.heard_while(
        &Event::Stop.said(Detail {
            own_work: crate::ownwork::Said {
                shells: 1,
                lapses_in: Some(60),
                ..crate::ownwork::Said::default()
            },
            ..Detail::default()
        }),
        Waits {
            now: NOW,
            ..Waits::default()
        },
    );
    assert!(chat.own_work().is_some());
    chat
}

#[test]
fn its_own_work_may_still_wake_it_after_its_wait_lapsed_or_a_block_ended_it() {
    let mut lapsed = waiting_on_its_own_shell();
    assert!(lapsed.own_work_lapsed(NOW + 60, Waits::default()));
    let mut blocked = waiting_on_its_own_shell();
    assert!(blocked.blocked_while_waiting());

    for chat in [&lapsed, &blocked] {
        assert!(chat.needs_you());
        assert_eq!(chat.own_work(), None);
        assert!(chat.wakes_on_its_own_work(), "{chat:?}");
    }
}

#[test]
fn a_notice_in_the_turn_that_ended_on_its_own_work_leaves_that_work_able_to_wake_it() {
    let mut chat = Chat::new();
    chat.reported(Event::UserPromptSubmit);

    chat.heard_while(
        &Event::Stop.said(Detail {
            own_work: crate::ownwork::Said {
                shells: 1,
                ..crate::ownwork::Said::default()
            },
            ..Detail::default()
        }),
        Waits {
            now: NOW,
            on_a_notice: true,
            ..Waits::default()
        },
    );

    assert!(chat.needs_you());
    assert!(chat.wakes_on_its_own_work());
}

#[test]
fn its_own_work_s_wake_after_a_lapse_is_a_background_turn_and_keeps_that_work_known() {
    let mut chat = waiting_on_its_own_shell();
    chat.own_work_lapsed(NOW + 60, Waits::default());

    background_turn_begins(&mut chat);

    assert!(chat.needs_you(), "the wait went with the wake");
    assert!(
        chat.wakes_on_its_own_work(),
        "a second wake is its work's too"
    );
}

#[test]
fn a_turn_that_is_no_background_turn_forgets_its_own_work_after_a_lapse() {
    let mut chat = waiting_on_its_own_shell();
    chat.own_work_lapsed(NOW + 60, Waits::default());

    chat.reported(Event::UserPromptSubmit);

    assert!(!chat.wakes_on_its_own_work());
}

#[test]
fn a_turn_that_ends_with_nothing_of_its_own_in_flight_forgets_its_own_work() {
    let mut chat = waiting_on_its_own_shell();
    chat.own_work_lapsed(NOW + 60, Waits::default());
    background_turn_begins(&mut chat);

    chat.reported(Event::Stop);

    assert!(!chat.wakes_on_its_own_work());
    assert!(chat.needs_you());
}

/// **A report applied with no `WaitsOf`** (#4647): no window holds the project, so nothing says
/// whether the person sent the line, and an unknown turn clears the wait, as #4562 chose.
#[test]
fn a_late_wake_heard_with_nothing_said_of_it_is_an_unknown_turn_and_clears_the_wait() {
    let mut board = Board::default();
    board.opened(7, None, None);
    let stop = crate::hookwire::Report {
        chat: 7,
        event: Event::Stop,
        conversation: crate::hookwire::Conversation::Unknown,
        pid: None,
        agent: None,
        detail: Detail {
            own_work: crate::ownwork::Said {
                shells: 1,
                lapses_in: Some(60),
                ..crate::ownwork::Said::default()
            },
            ..Detail::default()
        },
    };
    board.reported_while(
        &stop,
        Waits {
            now: NOW,
            ..Waits::default()
        },
    );
    assert!(board.own_work_lapsed(7, NOW + 60, Waits::default()));
    assert!(board.needs_you().contains(&7));

    board.hear(&crate::hookwire::relay::Heard::Report(
        crate::hookwire::Report {
            event: Event::UserPromptSubmit,
            detail: Detail::default(),
            ..stop
        },
    ));

    assert!(!board.needs_you().contains(&7));
}

/// R-290 (Q240): a held connection's ask that ended a wait on the chat's own work, and is then
/// withdrawn as the chat stops waiting on it, takes back the needs-you mark it set: the chat
/// waits on its own work again. A withdrawal that set nothing changes nothing.
#[test]
fn a_withdrawn_block_takes_back_the_needs_you_mark_its_ask_set() {
    let mut blocked = waiting_on_its_own_shell();
    let work = blocked.own_work();
    assert!(blocked.blocked_while_waiting());
    assert!(blocked.needs_you());

    assert!(blocked.block_withdrawn(), "a reader sees it");
    assert!(!blocked.needs_you(), "the mark is taken back");
    assert_eq!(blocked.own_work(), work, "it waits on its own work again");
    assert!(!blocked.block_withdrawn(), "once");

    let mut quiet = waiting_on_its_own_shell();
    assert!(!quiet.block_withdrawn(), "nothing set, nothing taken back");
    assert_eq!(quiet.own_work(), work);
}

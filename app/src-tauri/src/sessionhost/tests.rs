// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;

use super::*;

#[test]
fn the_window_starts_its_own_binary_as_its_session_host_holding_its_input() {
    let command = command(Path::new(
        "/Applications/purlis.app/Contents/MacOS/charter-app",
    ));
    assert_eq!(
        command.get_program(),
        "/Applications/purlis.app/Contents/MacOS/charter-app"
    );
    let args: Vec<_> = command.get_args().collect();
    assert_eq!(args, ["purlis-serve-for-the-window"]);
}

#[test]
fn a_host_that_found_another_serving_is_said_so_and_not_connected_to() {
    assert!(why_not_ready(Some(3)).contains("another session host already serves"));
    for code in [None, Some(0), Some(1)] {
        assert_eq!(
            why_not_ready(code),
            "the session host ended before it was ready",
            "{code:?}"
        );
    }
}

/// A report of chat `chat`'s, whose harness names no conversation and no process.
fn a_report(chat: u32, event: purlis_core::state::Event) -> purlis_core::hookwire::Report {
    purlis_core::hookwire::Report {
        chat,
        event,
        conversation: purlis_core::hookwire::Conversation::Unknown,
        pid: None,
        agent: None,
        detail: purlis_core::state::Detail::default(),
    }
}

/// What the host's board keeps, read back as a window takes it.
fn as_a_window_takes(board: &KeptBoard) -> purlis_core::state::Board {
    let mut taken = purlis_core::state::Board::new();
    taken.keep(serde_json::from_value(board.whole()).expect("a whole board"));
    taken
}

#[test]
fn a_line_no_window_took_moves_the_hosts_board_and_one_a_window_took_does_not() {
    use purlis_core::hookwire::relay::Heard;
    use purlis_core::state::{Event, Move, State};
    let board = KeptBoard::default();
    board.moved(
        1,
        serde_json::to_value(Move::Opened {
            reports_its_process: false,
            conversation: None,
        })
        .unwrap(),
    );
    let window_there = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let hand_on = board.handing_on({
        let window_there = std::sync::Arc::clone(&window_there);
        std::sync::Arc::new(move |_, _| window_there.load(std::sync::atomic::Ordering::SeqCst))
    });

    assert!(hand_on(
        1,
        &Heard::Report(a_report(1, Event::UserPromptSubmit))
    ));
    assert_eq!(
        as_a_window_takes(&board).state(1),
        State::Unknown,
        "the window's to decide"
    );

    window_there.store(false, std::sync::atomic::Ordering::SeqCst);
    assert!(!hand_on(2, &Heard::Report(a_report(1, Event::Stop))));

    let kept = as_a_window_takes(&board);
    assert_eq!(kept.state(1), State::Waiting);
    assert_eq!(kept.needs_you(), [1]);
}

#[test]
fn a_program_that_ends_with_no_window_is_told_to_the_hosts_board_with_its_code() {
    use purlis_core::state::{Move, State};
    let board = KeptBoard::default();
    for chat in [1, 2] {
        board.moved(
            chat,
            serde_json::to_value(Move::Opened {
                reports_its_process: false,
                conversation: None,
            })
            .unwrap(),
        );
    }

    board.ended(1, &session::Exit::Code(0));
    board.ended(2, &session::Exit::Signal("SIGKILL".into()));

    let kept = as_a_window_takes(&board);
    assert_eq!(kept.state(1), State::Done);
    assert_eq!(kept.state(2), State::Failed);
}

#[test]
fn a_move_the_host_cannot_read_moves_nothing() {
    let board = KeptBoard::default();

    board.moved(1, serde_json::json!({"move": "something else"}));

    assert!(as_a_window_takes(&board).chats().next().is_none());
}

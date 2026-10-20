// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn a_stop_says_purlis_stopped_it_and_how_it_exited() {
    let ending = Ending {
        by: EndedBy::Stopped,
        exit: Some(EngineExit::Code(0)),
        closed: None,
    };
    assert_eq!(
        ending.to_string(),
        "purlis stopped its engine (exit code 0)"
    );
}

#[test]
fn an_engine_that_ended_by_itself_says_its_signal() {
    let ending = Ending {
        by: EndedBy::Itself,
        exit: Some(EngineExit::Signal("SIGSEGV".into())),
        closed: Some(Closed::PipeEnded),
    };
    assert_eq!(
        ending.to_string(),
        "its engine ended by itself (signal SIGSEGV)"
    );
}

#[test]
fn a_lost_connection_says_why_it_closed_by_the_method_alone() {
    let ending = Ending {
        by: EndedBy::LostConnection,
        exit: Some(EngineExit::Signal("SIGKILL".into())),
        closed: Some(Closed::TooLarge {
            method: Some("Page.frameNavigated".into()),
            most: 64 << 20,
        }),
    };
    assert_eq!(
        ending.to_string(),
        "purlis ended its engine (signal SIGKILL) because its DevTools connection closed: the \
         engine sent an event (Page.frameNavigated) longer than 67108864 bytes, which purlis does \
         not read"
    );
}

#[test]
fn an_unknown_exit_is_left_out() {
    let ending = Ending {
        by: EndedBy::Itself,
        exit: None,
        closed: Some(Closed::NotDevTools { method: None }),
    };
    assert_eq!(
        ending.to_string(),
        "its engine ended by itself; the engine sent a message that does not read as DevTools \
         JSON"
    );
}

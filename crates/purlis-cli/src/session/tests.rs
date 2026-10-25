// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::io::{Error, ErrorKind};

use super::*;

#[test]
fn an_app_that_went_quiet_may_have_written_the_record_so_it_is_not_written_again() {
    for kind in [ErrorKind::TimedOut, ErrorKind::WouldBlock] {
        let Forwarded::Unsure(why) = answered(Err(Error::new(kind, "slow"))) else {
            panic!("a {kind:?} was taken as no app");
        };
        assert!(why.contains("may already be written"), "{why}");
        assert!(why.contains("purlis session list"), "{why}");
    }
}

#[test]
fn an_app_that_ended_the_connection_at_once_did_not_take_the_record() {
    for kind in [
        ErrorKind::UnexpectedEof,
        ErrorKind::BrokenPipe,
        ErrorKind::ConnectionReset,
    ] {
        assert!(
            matches!(
                answered(Err(Error::new(kind, "gone"))),
                Forwarded::NotTaken(_)
            ),
            "{kind:?}"
        );
    }
}

#[test]
fn a_written_record_under_a_pass_says_the_tab_closes_when_the_turn_ends() {
    let Forwarded::Written(saved) = answered(Ok(hookwire::Answer::Recorded {
        record: "sessions/r.md".to_owned(),
        closes: true,
        warnings: Vec::new(),
    })) else {
        panic!("not written");
    };
    assert!(saved.tab.contains("when this turn ends"), "{}", saved.tab);
}

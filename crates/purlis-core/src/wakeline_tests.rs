// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::{Ended, MOST_BYTES};
use crate::dispatchrestart::CARRY_ON;
use crate::ownwork::OwnWork;
use crate::ownwork::pieces::{Kind, Piece};

/// The wake line of a chat that had `own_work` running, by count alone.
fn of(own_work: Option<&OwnWork>, carry_on: bool) -> Option<String> {
    super::of(
        &Ended {
            own_work,
            ..Ended::default()
        },
        carry_on,
    )
}

fn one_shell() -> OwnWork {
    OwnWork {
        shells: 1,
        until: Some(1_000),
        ..OwnWork::default()
    }
}

#[test]
fn a_chat_with_no_own_work_that_is_no_task_is_told_nothing() {
    assert_eq!(of(None, false), None);
    assert_eq!(of(Some(&OwnWork::default()), false), None);
}

#[test]
fn a_chat_that_had_left_one_shell_running_is_told_the_restart_ended_it() {
    let line = of(Some(&one_shell()), false).expect("a wake line");
    assert!(line.starts_with("purlis: purlis was restarted"), "{line}");
    assert!(line.contains(": 1 background shell."), "{line}");
    assert!(
        line.contains("start again whatever is still needed"),
        "{line}"
    );
}

#[test]
fn each_kind_in_flight_is_named_with_how_many() {
    let line = of(
        Some(&OwnWork {
            shells: 2,
            monitors: 1,
            helpers: 3,
            workflows: 1,
            ..OwnWork::default()
        }),
        false,
    )
    .expect("a wake line");
    assert!(
        line.contains(": 2 background shells, 1 monitor, 3 subagents, 1 workflow."),
        "{line}"
    );
}

#[test]
fn a_task_with_no_own_work_is_told_to_carry_on_word_for_word() {
    assert_eq!(of(None, true).as_deref(), Some(CARRY_ON));
}

#[test]
fn a_task_that_had_left_a_shell_running_gets_one_line_that_carries_on_and_names_it() {
    let line = of(Some(&one_shell()), true).expect("a wake line");
    assert!(line.starts_with(CARRY_ON), "{line}");
    assert!(
        line.contains("The restart also ended what this chat had left running in the background: 1 background shell."),
        "{line}"
    );
    assert_eq!(line.matches("purlis: ").count(), 1, "one message: {line}");
}

#[test]
fn a_wake_line_is_a_first_message_a_harness_can_be_started_with() {
    for line in [of(Some(&one_shell()), false), of(Some(&one_shell()), true)] {
        let line = line.expect("a wake line");
        assert_eq!(crate::handoff::bad_message(&line), None, "{line}");
        assert!(!line.contains('\n'), "{line}");
    }
}

fn shell(id: &str, command: &str) -> Piece {
    Piece::new(Kind::Shell, id, command).expect("a piece")
}

#[test]
fn each_piece_recorded_is_named_by_kind_and_what_it_was() {
    let pieces = [
        shell("b1", "npm test"),
        Piece {
            schedule: Some("*/5 * * * *".to_owned()),
            prompt: Some("check the queue".to_owned()),
            ..Piece::new(Kind::Cron, "c1", "").expect("a piece")
        },
    ];
    let line = super::of(
        &Ended {
            own_work: Some(&one_shell()),
            pieces: &pieces,
            mid_turn: false,
        },
        false,
    )
    .expect("a wake line");
    assert!(
        line.contains(
            "in the background: a background shell \"npm test\"; a cron on \"*/5 * * * *\" \
             with the prompt \"check the queue\"."
        ),
        "{line}"
    );
}

#[test]
fn a_chat_cut_off_mid_turn_is_told_so_though_nothing_ran_in_the_background() {
    let line = super::of(
        &Ended {
            mid_turn: true,
            ..Ended::default()
        },
        false,
    )
    .expect("a wake line");
    assert!(line.starts_with("purlis: purlis was restarted"), "{line}");
    assert!(
        line.contains("ended this chat's last turn mid-way"),
        "{line}"
    );
    assert!(!line.contains("in the background"), "{line}");
}

#[test]
fn a_task_cut_off_mid_turn_with_a_shell_gets_one_line_with_all_of_it() {
    let pieces = [shell("b1", "cargo build")];
    let line = super::of(
        &Ended {
            own_work: None,
            pieces: &pieces,
            mid_turn: true,
        },
        true,
    )
    .expect("a wake line");
    assert!(line.starts_with(CARRY_ON), "{line}");
    assert!(line.contains("mid-way"), "{line}");
    assert!(
        line.contains("a background shell \"cargo build\""),
        "{line}"
    );
    assert_eq!(line.matches("purlis: ").count(), 1, "one message: {line}");
}

#[test]
fn a_wake_line_of_many_long_pieces_stays_a_first_message() {
    let long = "x ".repeat(200);
    let pieces: Vec<Piece> = (0..crate::ownwork::pieces::MOST_PIECES)
        .map(|n| Piece {
            prompt: Some(long.clone()),
            schedule: Some(long.clone()),
            ..Piece::new(Kind::Cron, &format!("c{n}"), &long).expect("a piece")
        })
        .collect();
    let line = super::of(
        &Ended {
            own_work: None,
            pieces: &pieces,
            mid_turn: true,
        },
        true,
    )
    .expect("a wake line");
    assert!(line.len() <= MOST_BYTES, "{}", line.len());
    assert_eq!(crate::handoff::bad_message(&line), None);
    assert!(!line.contains('\n'), "{line}");
    assert!(line.contains(" more."), "{line}");
}

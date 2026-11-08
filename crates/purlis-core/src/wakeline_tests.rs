// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::{Ended, MOST_BYTES};
use crate::dispatchrestart::CARRY_ON;
use crate::ownwork::OwnWork;
use crate::ownwork::last::Last;
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
            ..Ended::default()
        },
        false,
    )
    .expect("a wake line");
    assert!(
        line.contains("in the background: a background shell \"npm test\" (output gone)."),
        "{line}"
    );
    assert!(
        line.contains(
            "so do not set them again (check its list of scheduled prompts if unsure): \
             a cron on \"*/5 * * * *\" with the prompt \"check the queue\"."
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
            pieces: &pieces,
            mid_turn: true,
            ..Ended::default()
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
            pieces: &pieces,
            mid_turn: true,
            ..Ended::default()
        },
        true,
    )
    .expect("a wake line");
    assert!(line.len() <= MOST_BYTES, "{}", line.len());
    assert_eq!(crate::handoff::bad_message(&line), None);
    assert!(!line.contains('\n'), "{line}");
    assert!(line.contains(" more."), "{line}");
}

/// 14:05 on 2026-11-08, in seconds since 1970.
const FIVE_PAST_TWO: i64 = 1_794_146_700;

fn with_last<'a>(pieces: &'a [Piece], last: &'a dyn Fn(&Piece) -> Last) -> Ended<'a> {
    Ended {
        pieces,
        now: FIVE_PAST_TWO + 600,
        last,
        ..Ended::default()
    }
}

#[test]
fn a_shell_s_last_lines_are_quoted_as_data_with_where_the_rest_is_5751() {
    let pieces = [shell("b1", "npm test")];
    let last = |_: &Piece| Last::Lines {
        lines: vec!["ok 41".to_owned(), "not ok 42 - parses".to_owned()],
        path: "/tmp/claude-502/p/s/tasks/b1.output".to_owned(),
    };
    let line = super::of(&with_last(&pieces, &last), false).expect("a wake line");
    assert!(
        line.contains(
            "a background shell \"npm test\" (its last 2 lines, quoted as data: \"ok 41 ⏎ not \
             ok 42 - parses\"; the whole of its output is in /tmp/claude-502/p/s/tasks/b1.output)"
        ),
        "{line}"
    );
    assert!(!line.contains('\n'), "{line}");
}

#[test]
fn a_subagent_s_last_message_and_a_workflow_s_last_report_are_quoted_5751() {
    let pieces = [
        Piece::new(Kind::Subagent, "a7", "find the callers").expect("a piece"),
        Piece::new(Kind::Workflow, "w2", "review-changes").expect("a piece"),
    ];
    let last = |piece: &Piece| match piece.kind {
        Kind::Subagent => Last::Message("Found three callers".to_owned()),
        _ => Last::Lines {
            lines: vec!["phase Verify: 2 of 4".to_owned()],
            path: "/tmp/w2.output".to_owned(),
        },
    };
    let line = super::of(&with_last(&pieces, &last), false).expect("a wake line");
    assert!(
        line.contains(
            "a subagent \"find the callers\" (its last message, quoted as data: \"Found three \
             callers\")"
        ),
        "{line}"
    );
    assert!(
        line.contains(
            "a workflow \"review-changes\" (the last it reported, quoted as data: \"phase \
             Verify: 2 of 4\""
        ),
        "{line}"
    );
}

#[test]
fn a_wake_up_that_came_due_while_purlis_was_down_was_missed_5751() {
    let pieces = [Piece {
        prompt: Some("/loop check CI".to_owned()),
        due_at: Some(FIVE_PAST_TWO),
        ..Piece::new(Kind::WakeUp, "toolu_01", "CI").expect("a piece")
    }];
    let line = super::of(&with_last(&pieces, &|_| Last::Nothing), false).expect("a wake line");
    assert!(
        line.contains(
            "These came due while purlis was down, and were missed: a wake-up \"CI\" with the \
             prompt \"/loop check CI\", was due at 2026-11-08 14:05 UTC, missed. Do now what \
             each was for"
        ),
        "{line}"
    );
}

#[test]
fn a_wake_up_known_only_by_count_that_came_due_was_missed_5751() {
    let work = OwnWork {
        wakes_at: Some(FIVE_PAST_TWO),
        until: Some(FIVE_PAST_TWO + 300),
        ..OwnWork::default()
    };
    let line = super::of(
        &Ended {
            own_work: Some(&work),
            now: FIVE_PAST_TWO + 60,
            ..Ended::default()
        },
        false,
    )
    .expect("a wake line");
    assert!(
        line.contains("missed: a wake-up, was due at 2026-11-08 14:05 UTC, missed."),
        "{line}"
    );
}

#[test]
fn a_cron_and_a_wake_up_still_ahead_are_the_harness_s_to_set_again_and_alone_tell_nothing() {
    let pieces = [
        Piece {
            schedule: Some("0 9 * * 1-5".to_owned()),
            prompt: Some("standup".to_owned()),
            ..Piece::new(Kind::Cron, "c1", "").expect("a piece")
        },
        Piece {
            due_at: Some(FIVE_PAST_TWO + 3_600),
            ..Piece::new(Kind::WakeUp, "c2", "").expect("a piece")
        },
    ];
    let ended = with_last(&pieces, &|_| Last::Nothing);
    assert_eq!(
        super::of(&ended, false),
        None,
        "the harness wakes it for these"
    );
    let line = super::of(
        &Ended {
            mid_turn: true,
            ..ended
        },
        false,
    )
    .expect("a wake line");
    assert!(
        line.contains(
            "The harness sets these again itself as it resumes this conversation, so do not set \
             them again (check its list of scheduled prompts if unsure): a cron on \"0 9 * * 1-5\" \
             with the prompt \"standup\"; a wake-up, due at 2026-11-08 15:05 UTC."
        ),
        "{line}"
    );
    assert!(!line.contains("missed"), "{line}");
}

#[test]
fn many_pieces_with_long_output_keep_the_line_a_first_message_naming_each_5751() {
    let pieces: Vec<Piece> = (0..crate::ownwork::pieces::MOST_PIECES)
        .map(|n| shell(&format!("b{n}"), "cargo test"))
        .collect();
    let last = |_: &Piece| Last::Lines {
        lines: vec!["y".repeat(crate::ownwork::last::MOST_LINE_BYTES); crate::ownwork::last::LINES],
        path: "/tmp/x.output".to_owned(),
    };
    let line = super::of(&with_last(&pieces, &last), true).expect("a wake line");
    assert!(line.len() <= MOST_BYTES, "{}", line.len());
    assert_eq!(crate::handoff::bad_message(&line), None);
    assert!(
        line.contains("its last 20 lines"),
        "the first is quoted: {line}"
    );
    assert_eq!(
        line.matches("a background shell \"cargo test\"").count(),
        crate::ownwork::pieces::MOST_PIECES,
        "each named, the later ones without their output"
    );
}

#[test]
fn a_chat_resumed_on_its_conversation_is_told_the_work_its_record_holds() {
    let chat = crate::reopen::Chat {
        resume: Some(crate::harness::SessionId::new("conv-steward").expect("an id")),
        own_work: Some(one_shell()),
        ..crate::reopen::Chat::default()
    };

    let line = super::of_chat(&chat, false).expect("a wake line");

    assert!(line.contains(": 1 background shell."), "{line}");
}

#[test]
fn a_chat_started_with_no_conversation_is_told_nothing_of_its_work() {
    let chat = crate::reopen::Chat {
        resume: None,
        own_work: Some(one_shell()),
        ..crate::reopen::Chat::default()
    };

    assert_eq!(super::of_chat(&chat, false), None);
}

#[test]
fn what_ended_is_named_with_how_many_of_each_kind() {
    let work = OwnWork {
        shells: 2,
        monitors: 1,
        ..OwnWork::default()
    };

    assert_eq!(
        super::counted(&work).as_deref(),
        Some("2 background shells, 1 monitor")
    );
    assert_eq!(super::counted(&OwnWork::default()), None);
}

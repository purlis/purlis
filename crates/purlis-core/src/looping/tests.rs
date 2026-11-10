// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

const CHAT: u32 = 4;
const OTHER: u32 = 5;

/// Stuck? at 3 failures in a row, as by default: looping is at 6.
fn limits() -> Limits {
    Limits::of(&crate::stuck::Limits {
        silent_minutes: 15,
        failing_times: 3,
        long_minutes: 60,
    })
}

#[test]
fn looping_is_twice_what_reads_stuck_for_the_same_command_and_for_refusals() {
    assert_eq!(
        limits(),
        Limits {
            failing_times: 6,
            refused_times: 6
        }
    );
}

#[test]
fn the_same_command_failing_the_loop_limit_times_in_a_row_is_looping_once() {
    let mut loops = Loops::default();
    for _ in 0..5 {
        assert_eq!(loops.ran(CHAT, "cargo test", true, &limits()), None);
    }
    assert_eq!(
        loops.ran(CHAT, " cargo test ", true, &limits()),
        Some(Why::Failing { times: 6 })
    );
    // Told once: the chat is paused on it, and the person answers.
    assert_eq!(loops.ran(CHAT, "cargo test", true, &limits()), None);
}

#[test]
fn another_command_or_a_success_starts_the_count_again() {
    let mut loops = Loops::default();
    for _ in 0..5 {
        loops.ran(CHAT, "cargo test", true, &limits());
    }
    loops.ran(CHAT, "cargo test", false, &limits());
    for _ in 0..5 {
        assert_eq!(loops.ran(CHAT, "cargo test", true, &limits()), None);
    }
    loops.ran(CHAT, "cargo build", true, &limits());
    assert_eq!(loops.ran(CHAT, "cargo test", true, &limits()), None);
}

#[test]
fn refusals_piling_up_without_an_allow_between_are_looping() {
    let mut loops = Loops::default();
    for _ in 0..3 {
        assert_eq!(loops.answered(CHAT, true, &limits()), None);
    }
    // An allow between starts the count again.
    loops.answered(CHAT, false, &limits());
    for _ in 0..5 {
        assert_eq!(loops.answered(CHAT, true, &limits()), None);
    }
    assert_eq!(
        loops.answered(CHAT, true, &limits()),
        Some(Why::Refused { times: 6 })
    );
}

#[test]
fn its_fine_starts_every_count_again_so_the_next_loop_is_told_afresh() {
    let mut loops = Loops::default();
    for _ in 0..6 {
        loops.ran(CHAT, "cargo test", true, &limits());
    }
    loops.fine(CHAT);
    for _ in 0..5 {
        assert_eq!(loops.ran(CHAT, "cargo test", true, &limits()), None);
    }
    assert_eq!(
        loops.ran(CHAT, "cargo test", true, &limits()),
        Some(Why::Failing { times: 6 })
    );
}

#[test]
fn each_chat_is_counted_apart_and_a_closed_one_is_forgotten() {
    let mut loops = Loops::default();
    for _ in 0..5 {
        loops.ran(CHAT, "make", true, &limits());
    }
    assert_eq!(loops.ran(OTHER, "make", true, &limits()), None);
    loops.forget(CHAT);
    assert_eq!(loops.ran(CHAT, "make", true, &limits()), None);
}

#[test]
fn what_looping_says_names_the_rule_and_its_count() {
    assert_eq!(
        Why::Failing { times: 6 }.said(),
        "the same command failed 6 times in a row"
    );
    assert_eq!(
        Why::Refused { times: 6 }.said(),
        "6 of its asks were refused in a row"
    );
}

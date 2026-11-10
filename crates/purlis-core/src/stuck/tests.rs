// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

const CHAT: u32 = 4;
const LIMITS: Limits = Limits {
    silent_minutes: 15,
    failing_times: 3,
    long_minutes: 60,
};

fn min(n: i64) -> i64 {
    n * 60
}

/// A watch whose chat began a turn at 0.
fn working() -> Watch {
    let mut watch = Watch::default();
    watch.state(CHAT, true, 0);
    watch
}

#[test]
fn a_working_chat_with_no_new_command_or_output_for_the_silent_limit_is_stuck() {
    let mut watch = working();
    watch.heard(CHAT, min(2));
    assert_eq!(watch.stuck(CHAT, min(16), &LIMITS), None);
    assert_eq!(
        watch.stuck(CHAT, min(17), &LIMITS),
        Some(Why::Silent { minutes: 15 })
    );
    // Heard from again: no longer silent.
    watch.heard(CHAT, min(18));
    assert_eq!(watch.stuck(CHAT, min(19), &LIMITS), None);
}

#[test]
fn the_same_command_failing_the_limit_times_in_a_row_is_stuck_and_a_success_clears_it() {
    let mut watch = working();
    watch.ran(CHAT, "cargo test", true, min(1));
    watch.ran(CHAT, "cargo test", true, min(2));
    assert_eq!(watch.stuck(CHAT, min(3), &LIMITS), None);
    watch.ran(CHAT, "cargo test", true, min(3));
    assert_eq!(
        watch.stuck(CHAT, min(4), &LIMITS),
        Some(Why::Failing { times: 3 })
    );
    // Failing again says the same: the line does not move with each failure.
    watch.ran(CHAT, "cargo test", true, min(4));
    assert_eq!(
        watch.stuck(CHAT, min(4), &LIMITS),
        Some(Why::Failing { times: 3 })
    );
    watch.ran(CHAT, "cargo test", false, min(5));
    assert_eq!(watch.stuck(CHAT, min(6), &LIMITS), None);
}

#[test]
fn another_command_between_failures_starts_the_count_again() {
    let mut watch = working();
    watch.ran(CHAT, "cargo test", true, min(1));
    watch.ran(CHAT, "cargo test", true, min(2));
    watch.ran(CHAT, "ls", true, min(3));
    watch.ran(CHAT, "cargo test", true, min(4));
    assert_eq!(watch.stuck(CHAT, min(5), &LIMITS), None);
}

#[test]
fn a_turn_running_longer_than_the_long_limit_is_stuck_however_busy() {
    let mut watch = working();
    for at in 1..=61 {
        watch.heard(CHAT, min(at));
    }
    assert_eq!(watch.stuck(CHAT, min(59), &LIMITS), None);
    assert_eq!(
        watch.stuck(CHAT, min(61), &LIMITS),
        Some(Why::Long { minutes: 60 })
    );
}

#[test]
fn its_fine_clears_it_until_the_chat_changes_state() {
    let mut watch = working();
    assert_eq!(
        watch.stuck(CHAT, min(20), &LIMITS),
        Some(Why::Silent { minutes: 15 })
    );
    watch.fine(CHAT);
    assert_eq!(
        watch.stuck(CHAT, min(90), &LIMITS),
        None,
        "still the same turn"
    );
    // Its turn ends and another begins: a new state, watched afresh.
    watch.state(CHAT, false, min(91));
    watch.state(CHAT, true, min(92));
    assert_eq!(
        watch.stuck(CHAT, min(108), &LIMITS),
        Some(Why::Silent { minutes: 15 })
    );
}

#[test]
fn a_chat_not_working_or_never_heard_or_gone_is_never_stuck() {
    let mut watch = Watch::default();
    assert_eq!(watch.stuck(CHAT, min(100), &LIMITS), None, "never heard");
    watch.state(CHAT, true, 0);
    watch.ran(CHAT, "make", true, 1);
    watch.ran(CHAT, "make", true, 2);
    watch.ran(CHAT, "make", true, 3);
    watch.state(CHAT, false, min(1));
    assert_eq!(watch.stuck(CHAT, min(100), &LIMITS), None, "its turn ended");
    watch.state(CHAT, true, min(101));
    assert_eq!(
        watch.stuck(CHAT, min(102), &LIMITS),
        None,
        "a new turn's count begins at none"
    );
    watch.forget(CHAT);
    assert_eq!(watch.stuck(CHAT, min(200), &LIMITS), None, "gone");
}

#[test]
fn the_same_state_told_again_is_no_change() {
    let mut watch = working();
    watch.fine(CHAT);
    assert!(!watch.state(CHAT, true, min(30)), "no change");
    assert_eq!(watch.stuck(CHAT, min(40), &LIMITS), None, "still fine");
}

#[test]
fn due_is_the_next_time_a_rule_of_time_can_come_true() {
    let mut watch = working();
    watch.heard(CHAT, min(10));
    assert_eq!(watch.due(CHAT, min(11), &LIMITS), Some(min(25)));
    // Silent already: the next is the long limit.
    assert_eq!(watch.due(CHAT, min(30), &LIMITS), Some(min(60)));
    // Both passed: nothing more to wait for.
    assert_eq!(watch.due(CHAT, min(70), &LIMITS), None);
    watch.fine(CHAT);
    assert_eq!(
        watch.due(CHAT, min(11), &LIMITS),
        None,
        "fine: no look owed"
    );
    assert_eq!(Watch::default().due(CHAT, 0, &LIMITS), None);
}

#[test]
fn why_is_said_in_purlis_s_words() {
    assert_eq!(
        Why::Silent { minutes: 15 }.said(),
        "no new command or output for 15 minutes"
    );
    assert_eq!(
        Why::Failing { times: 3 }.said(),
        "the same command failed 3 times in a row"
    );
    assert_eq!(
        Why::Long { minutes: 1 }.said(),
        "one turn running for 1 minute"
    );
}

#[test]
fn the_first_state_told_of_a_chat_is_a_change_so_a_restored_chat_is_looked_at() {
    let mut watch = Watch::default();
    assert!(watch.state(CHAT, false, 0), "first heard, not working");
    assert!(!watch.state(CHAT, false, 1));
    assert_eq!(watch.working_since(CHAT), None);
    assert!(watch.state(CHAT, true, 2));
    assert_eq!(
        watch.working_since(CHAT),
        Some(2),
        "the turn a timer is for"
    );
}

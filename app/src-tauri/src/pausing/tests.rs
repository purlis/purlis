// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

const TASK: u32 = 7;
const BELOW: u32 = 8;
const AT: i64 = 1_000;

fn at_time(limit: u32, worked: u32) -> Why {
    Why::Budget(Reached::Time { limit, worked })
}

fn looping_failing() -> Why {
    Why::Looping(looping::Why::Failing { times: 6 })
}

const LIMITS: looping::Limits = looping::Limits {
    failing_times: 6,
    refused_times: 6,
};

#[test]
fn a_chat_paused_holds_it_and_its_tasks_at_work_and_is_listed_since_then() {
    let pauses = Pauses::default();
    assert_eq!(
        pauses.pause(TASK, at_time(30, 30), AT, vec![BELOW]),
        Some(vec![TASK, BELOW])
    );
    assert!(pauses.holds(TASK) && pauses.holds(BELOW));
    let now = pauses.now();
    assert_eq!(now.len(), 1, "one ask, for the chat that reached it");
    assert_eq!(now[0].0, TASK);
    assert_eq!(now[0].1.since, 1_000);
    assert_eq!(
        pauses.at_budgets(),
        vec![(
            TASK,
            Reached::Time {
                limit: 30,
                worked: 30
            }
        )]
    );
}

#[test]
fn a_chat_a_pause_holds_already_is_not_paused_again() {
    let pauses = Pauses::default();
    pauses.pause(TASK, at_time(30, 30), AT, vec![BELOW]);
    assert_eq!(pauses.pause(TASK, at_time(30, 31), AT, vec![]), None);
    assert_eq!(pauses.pause(BELOW, looping_failing(), AT, vec![]), None);
    assert_eq!(pauses.now().len(), 1);
}

#[test]
fn continue_lets_it_go_on_and_that_budget_never_pauses_it_again_but_a_new_limit_does() {
    let pauses = Pauses::default();
    pauses.pause(TASK, at_time(30, 30), AT, vec![BELOW]);

    let answered = pauses.answer(TASK, CONTINUE).expect("answered");
    assert_eq!(answered.held, vec![TASK, BELOW], "what goes on");
    assert_eq!(answered.then, Then::GoOn);
    assert!(!pauses.holds(TASK) && !pauses.holds(BELOW));
    assert!(pauses.now().is_empty(), "its ask has gone");

    // Past the same limit at the next look: not paused again.
    assert_eq!(pauses.pause(TASK, at_time(30, 45), AT, vec![]), None);
    // The person set another, and it reached that: paused again.
    assert!(pauses.pause(TASK, at_time(60, 60), AT, vec![]).is_some());
}

#[test]
fn stop_at_a_budget_lets_it_go_on_only_to_stop_it_at_that_limit() {
    let pauses = Pauses::default();
    let reached = Reached::Tokens {
        limit: 100_000,
        used: 105_000,
    };
    pauses.pause(TASK, Why::Budget(reached), AT, vec![]);
    let answered = pauses.answer(TASK, STOP).expect("answered");
    assert_eq!(answered.then, Then::StopAt(reached));
    assert!(pauses.now().is_empty());
}

#[test]
fn its_fine_on_a_loop_lets_it_go_on_and_counts_again_from_nothing() {
    let pauses = Pauses::default();
    for _ in 0..5 {
        assert_eq!(pauses.ran(TASK, "make", true, &LIMITS), None);
    }
    let why = pauses.ran(TASK, "make", true, &LIMITS).expect("a loop");
    pauses.pause(TASK, Why::Looping(why), AT, vec![]);

    let answered = pauses.answer(TASK, FINE).expect("answered");
    assert_eq!(answered.then, Then::GoOn);
    for _ in 0..5 {
        assert_eq!(
            pauses.ran(TASK, "make", true, &LIMITS),
            None,
            "counted afresh"
        );
    }
}

/// Q261: **Raise the budget answers its ask**, so the ask is no longer Not answered, and the
/// chat stays held: it goes on only once a look finds it within the limit the person raised
/// ([`let_go_of_budgets`]).
#[test]
fn raise_the_budget_answers_its_ask_and_keeps_it_held_until_a_look_finds_it_within() {
    let pauses = Pauses::default();
    pauses.pause(TASK, at_time(30, 30), AT, vec![BELOW]);

    let answered = pauses.answer(TASK, RAISE).expect("answered");
    assert_eq!(answered.then, Then::Raise);
    assert!(answered.held.is_empty(), "nothing goes on yet");
    assert!(pauses.holds(TASK) && pauses.holds(BELOW), "held still");
    assert_eq!(
        pauses.at_budgets(),
        vec![(
            TASK,
            Reached::Time {
                limit: 30,
                worked: 30
            }
        )],
        "the look that finds it within the new limit lets it go"
    );
}

#[test]
fn stop_on_a_loop_is_the_windows_stop() {
    let pauses = Pauses::default();
    pauses.pause(TASK, looping_failing(), AT, vec![]);
    assert_eq!(
        pauses.answer(TASK, STOP).expect("answered").then,
        Then::Stop
    );
}

#[test]
fn an_answer_the_ask_does_not_offer_or_to_no_pause_is_refused_and_changes_nothing() {
    let pauses = Pauses::default();
    assert!(pauses.answer(TASK, CONTINUE).is_err(), "nothing is paused");
    pauses.pause(TASK, looping_failing(), AT, vec![]);
    assert!(
        pauses.answer(TASK, CONTINUE).is_err(),
        "a loop offers no Continue"
    );
    assert!(
        pauses.answer(TASK, RAISE).is_err(),
        "a loop offers no Raise the budget"
    );
    assert!(pauses.holds(TASK), "still paused");
}

#[test]
fn refusals_piling_up_are_a_loop() {
    let pauses = Pauses::default();
    for _ in 0..5 {
        assert_eq!(pauses.answered(TASK, true, &LIMITS), None);
    }
    assert_eq!(
        pauses.answered(TASK, true, &LIMITS),
        Some(looping::Why::Refused { times: 6 })
    );
}

#[test]
fn a_chat_that_closes_is_forgotten_and_held_by_no_pause() {
    let pauses = Pauses::default();
    pauses.pause(TASK, at_time(30, 30), AT, vec![BELOW]);
    pauses.forget(BELOW);
    assert!(!pauses.holds(BELOW));
    assert!(pauses.holds(TASK));
    pauses.forget(TASK);
    assert!(pauses.now().is_empty());
}

#[test]
fn a_time_limit_and_the_one_above_it_are_the_same_limit_and_tokens_are_another() {
    assert!(same_limit(
        Reached::Time {
            limit: 30,
            worked: 30
        },
        Reached::Above { limit: 30 }
    ));
    assert!(!same_limit(
        Reached::Time {
            limit: 30,
            worked: 30
        },
        Reached::Tokens {
            limit: 30,
            used: 31
        }
    ));
}

/// A sandbox block of a `Bash` call whose arguments hash to `aa`, reported plainly.
fn sandbox_block() -> purlis_core::eventlog::stops::Stop {
    use purlis_core::eventlog::stops::{Block, Source, Stop};
    Stop::Block(Block {
        tool: "Bash".to_owned(),
        source: Source::Sandbox,
        qualifier: None,
        target: None,
        args: Some("aa".to_owned()),
    })
}

/// R-211 (Q253): the person's Allow on a sandbox ask starts the chat's block count again.
#[test]
fn an_allow_between_blocks_starts_the_block_count_again() {
    let pauses = Pauses::default();
    let block = sandbox_block();
    assert_eq!(pauses.stopped(TASK, &block, None), None);
    assert_eq!(pauses.stopped(TASK, &block, None), None);
    pauses.allowed(TASK);
    assert_eq!(pauses.stopped(TASK, &block, None), None);
    assert_eq!(
        pauses.stopped(TASK, &block, None),
        None,
        "2 blocks, an Allow, 2 blocks: no pause"
    );
}

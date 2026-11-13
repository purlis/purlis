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
        pauses.answer(TASK, "raise").is_err(),
        "Raise the budget is the window's"
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

/// Chat `TASK` blocked on the same target until the block loop pauses it (R-206): answers why.
fn paused_at_the_block_loop(pauses: &Pauses) -> looping::Why {
    let block = sandbox_block();
    assert_eq!(pauses.stopped(TASK, &block, None), None);
    assert_eq!(pauses.stopped(TASK, &block, None), None);
    let why = pauses
        .stopped(TASK, &block, None)
        .expect("3 blocks: a loop");
    assert_eq!(why, looping::Why::Blocked { times: 3 });
    pauses.pause(TASK, Why::Looping(why), AT, vec![BELOW]);
    why
}

/// Two more blocks on the same target pause nothing: the count started again.
fn counted_afresh(pauses: &Pauses) {
    let block = sandbox_block();
    assert_eq!(pauses.stopped(TASK, &block, None), None, "counted afresh");
    assert_eq!(pauses.stopped(TASK, &block, None), None, "counted afresh");
    assert_eq!(
        pauses.stopped(TASK, &block, None),
        Some(looping::Why::Blocked { times: 3 }),
        "and a third pauses it again"
    );
}

/// **T1-36 (R-217, R-218): Let it keep trying lets it go on and starts its block count again.**
#[test]
fn let_it_keep_trying_lets_it_go_on_and_starts_the_block_count_again() {
    let pauses = Pauses::default();
    paused_at_the_block_loop(&pauses);
    let answered = pauses.answer(TASK, KEEP_TRYING).expect("answered");
    assert_eq!(answered.held, vec![TASK, BELOW], "what goes on");
    assert_eq!(answered.then, Then::GoOn);
    assert!(!pauses.holds(TASK));
    counted_afresh(&pauses);
}

/// **T1-36 (R-217): Tell it to stop trying lets it go on, told to stop trying, and starts its
/// block count again**, so a chat that tries once more is counted from nothing.
#[test]
fn tell_it_to_stop_trying_lets_it_go_on_told_and_starts_the_block_count_again() {
    let pauses = Pauses::default();
    paused_at_the_block_loop(&pauses);
    let answered = pauses.answer(TASK, STOP_TRYING).expect("answered");
    assert_eq!(answered.held, vec![TASK, BELOW]);
    assert_eq!(answered.then, Then::TellToStopTrying);
    assert!(!pauses.holds(TASK));
    counted_afresh(&pauses);
}

/// **T1-36 (R-217): Stop chat is the window's Stop**, its counts started again.
#[test]
fn stop_chat_on_a_block_loop_is_the_windows_stop() {
    let pauses = Pauses::default();
    paused_at_the_block_loop(&pauses);
    assert_eq!(
        pauses.answer(TASK, STOP).expect("answered").then,
        Then::Stop
    );
    counted_afresh(&pauses);
}

/// **T1-36: each loop pause takes only the answers its ask offers**: It's fine, continue is
/// no answer to a block loop, nor Let it keep trying or Tell it to stop trying to another loop.
#[test]
fn a_block_loop_and_another_loop_each_take_only_their_own_answers() {
    let pauses = Pauses::default();
    paused_at_the_block_loop(&pauses);
    assert!(pauses.answer(TASK, FINE).is_err());
    assert!(pauses.holds(TASK), "still paused");

    let other = Pauses::default();
    other.pause(TASK, looping_failing(), AT, vec![]);
    assert!(other.answer(TASK, KEEP_TRYING).is_err());
    assert!(other.answer(TASK, STOP_TRYING).is_err());
    assert!(other.holds(TASK), "still paused");
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

/// A chat as the app knows it: a measured harness, heard from, open, and `then`.
fn seen(then: impl FnOnce(&mut Seen)) -> Seen {
    let mut seen = Seen {
        measured: true,
        heard: true,
        ..Seen::default()
    };
    then(&mut seen);
    seen
}

fn waiting() -> Seen {
    seen(|one| one.waiting = true)
}

fn running() -> Seen {
    seen(|one| one.running = true)
}

/// What was typed into each chat, through a send that records it.
fn typing() -> (
    std::rc::Rc<std::cell::RefCell<Vec<(u32, Vec<u8>)>>>,
    impl Fn(u32, &[u8]) -> Result<(), String>,
) {
    let typed = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let into = std::rc::Rc::clone(&typed);
    (typed, move |chat: u32, bytes: &[u8]| {
        into.borrow_mut().push((chat, bytes.to_vec()));
        Ok(())
    })
}

/// **Q267 amended: Tell it to stop trying types purlis's line as a bracketed paste, then
/// Enter**, the line whole inside the paste and nothing after the Enter.
#[test]
fn the_stop_trying_line_is_typed_as_one_bracketed_paste_then_enter() {
    assert_eq!(
        stop_trying_typed(),
        format!("\x1b[200~{STOP_TRYING_LINE}\x1b[201~\r")
    );
}

/// **Q267 amended: a chat that takes no keys is told nothing**: a shell (no measured
/// harness), a harness not yet heard from, a chat that ended, one showing a prompt, and one in
/// a stop's last turn. Nothing is kept for its turn's end either.
#[test]
fn a_chat_that_takes_no_keys_is_never_told_to_stop_trying() {
    let shell = seen(|one| {
        one.measured = false;
        one.waiting = true;
    });
    let unheard = seen(|one| one.heard = false);
    let ended = seen(|one| {
        one.ended = true;
        one.waiting = true;
    });
    let prompt = seen(|one| {
        one.waiting = true;
        one.asking = true;
    });
    for (said, one) in [
        ("a shell", shell),
        ("unheard", unheard),
        ("ended", ended),
        ("a prompt", prompt),
    ] {
        assert_eq!(telling(one, false), Telling::Never, "{said}");
    }
    assert_eq!(telling(waiting(), true), Telling::Never, "its last turn");

    let pauses = Pauses::default();
    let (typed, send) = typing();
    tell_sending(&pauses, TASK, || telling(Seen::default(), false), &send);
    assert!(typed.borrow().is_empty(), "nothing typed");
    turn_ended_sending(&pauses, TASK, || at_turn_end(waiting(), false), &send);
    assert!(typed.borrow().is_empty(), "nothing kept for its turn's end");
}

/// **Q267 amended: a waiting chat is told at once**, and nothing is kept for later.
#[test]
fn a_waiting_chat_is_told_to_stop_trying_at_once() {
    assert_eq!(telling(waiting(), false), Telling::Now);
    let pauses = Pauses::default();
    let (typed, send) = typing();
    tell_sending(&pauses, TASK, || telling(waiting(), false), &send);
    assert_eq!(
        *typed.borrow(),
        [(TASK, stop_trying_typed().into_bytes())],
        "typed once, as the paste and Enter"
    );
    turn_ended_sending(&pauses, TASK, || at_turn_end(waiting(), false), &send);
    assert_eq!(typed.borrow().len(), 1, "never typed twice");
}

/// **Q267 amended: a chat mid-turn is never typed into**: the line is kept, and typed once
/// its turn ends with it waiting, never while it runs or shows a prompt.
#[test]
fn a_running_chat_is_told_to_stop_trying_only_at_its_turns_end() {
    assert_eq!(telling(running(), false), Telling::AtTurnEnd);
    let pauses = Pauses::default();
    let (typed, send) = typing();
    tell_sending(&pauses, TASK, || telling(running(), false), &send);
    assert!(typed.borrow().is_empty(), "nothing typed mid-turn");

    turn_ended_sending(&pauses, TASK, || at_turn_end(running(), false), &send);
    let prompt = seen(|one| {
        one.waiting = true;
        one.asking = true;
    });
    turn_ended_sending(&pauses, TASK, || at_turn_end(prompt, false), &send);
    assert!(
        typed.borrow().is_empty(),
        "nothing typed while it runs or asks"
    );

    turn_ended_sending(&pauses, TASK, || at_turn_end(waiting(), false), &send);
    assert_eq!(*typed.borrow(), [(TASK, stop_trying_typed().into_bytes())]);
    turn_ended_sending(&pauses, TASK, || at_turn_end(waiting(), false), &send);
    assert_eq!(typed.borrow().len(), 1, "typed once");
}

/// **Q267 amended: a line kept for a turn's end is dropped** where the chat ended, was put in a
/// stop's last turn, or was forgotten before its turn ended.
#[test]
fn a_kept_stop_trying_line_is_dropped_when_its_chat_ends_or_is_being_stopped() {
    let ended = seen(|one| {
        one.ended = true;
        one.waiting = true;
    });
    for (said, look) in [
        ("ended", (ended, false)),
        ("its last turn", (waiting(), true)),
    ] {
        let pauses = Pauses::default();
        let (typed, send) = typing();
        tell_sending(&pauses, TASK, || telling(running(), false), &send);
        turn_ended_sending(&pauses, TASK, || at_turn_end(look.0, look.1), &send);
        turn_ended_sending(&pauses, TASK, || at_turn_end(waiting(), false), &send);
        assert!(typed.borrow().is_empty(), "{said}");
    }

    let pauses = Pauses::default();
    let (typed, send) = typing();
    tell_sending(&pauses, TASK, || telling(running(), false), &send);
    pauses.forget(TASK);
    turn_ended_sending(&pauses, TASK, || at_turn_end(waiting(), false), &send);
    assert!(typed.borrow().is_empty(), "forgotten");
}

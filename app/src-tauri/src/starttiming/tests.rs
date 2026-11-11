// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn at(base: Instant, ms: u64) -> Instant {
    base + Duration::from_millis(ms)
}

#[test]
fn a_start_line_says_each_phase_as_the_time_since_the_one_before_it() {
    let asked = Instant::now();
    let mut clock = Clock::asked_at(asked, at(asked, 20_000));
    clock.passed(Phase::Ready, at(asked, 20_040));
    clock.passed(Phase::Proxy, at(asked, 20_100));
    clock.passed(Phase::Line, at(asked, 20_150));
    clock.passed(Phase::Host, at(asked, 245_150));

    let line = clock.line("chat 7", &Ok(7), at(asked, 245_200));

    assert_eq!(
        line,
        "purlis: start of chat 7: 225200 ms after 20000 ms at the gate (ready +40, proxy +60, \
         line +50, host +225000); live as chat 7"
    );
}

#[test]
fn a_start_that_did_not_start_says_so_and_how_far_it_got() {
    let asked = Instant::now();
    let clock = Clock::asked_at(asked, at(asked, 5));

    let line = clock.line(
        "chat 3",
        &Err("the host closed the link".to_owned()),
        at(asked, 9),
    );

    assert_eq!(
        line,
        "purlis: start of chat 3: 4 ms after 5 ms at the gate (no phase passed); did not start"
    );
}

#[test]
fn a_chat_is_said_to_come_alive_once_after_its_timed_start() {
    let asked = Instant::now();
    let mut waiting = Waiting::default();
    waiting.asked(4, asked);

    assert_eq!(
        waiting.heard(4, at(asked, 3_000)),
        Some(Duration::from_millis(3_000))
    );
    assert_eq!(waiting.heard(4, at(asked, 4_000)), None);
}

#[test]
fn a_chat_no_timed_start_brought_up_is_never_said_to_come_alive() {
    let mut waiting = Waiting::default();

    assert_eq!(waiting.heard(9, Instant::now()), None);
}

#[test]
fn a_mark_on_a_thread_with_no_clock_does_nothing() {
    mark(Phase::Host);

    assert!(CLOCK.with_borrow(Option::is_none));
}

#[test]
fn a_timed_start_takes_the_marks_its_start_made() {
    let asked = Instant::now();
    let mut seen = None;
    let started = timed("chat 5", asked, asked, || {
        mark(Phase::Ready);
        mark(Phase::Host);
        seen = CLOCK.with_borrow(|clock| {
            clock
                .as_ref()
                .map(|c| c.passed.iter().map(|(p, _)| *p).collect::<Vec<_>>())
        });
        Ok(5)
    });

    assert_eq!(started, Ok(5));
    assert_eq!(seen, Some(vec![Phase::Ready, Phase::Host]));
    assert!(
        CLOCK.with_borrow(Option::is_none),
        "the clock is taken off the thread"
    );
}

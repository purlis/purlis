// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The seam for grading several skills under one limit (#5356): a [`Batch`] the host asks what
//! to start and tells what each grade spent.

use super::*;

/// Starts every grade the batch lets start now, by their places.
fn start_all(batch: &mut Batch) -> Vec<usize> {
    std::iter::from_fn(|| batch.next()).collect()
}

#[test]
fn a_few_run_at_a_time_and_the_next_starts_as_one_ends() {
    let mut batch = Batch::new(5, 10.0, 3);
    assert_eq!(start_all(&mut batch), vec![0, 1, 2]);
    batch.ended(1, 0.5);
    assert_eq!(start_all(&mut batch), vec![3]);
    batch.ended(0, 0.5);
    batch.ended(2, 0.5);
    assert_eq!(start_all(&mut batch), vec![4]);
    assert_eq!(batch.ending(), None, "one still runs");
    batch.ended(3, 0.5);
    batch.ended(4, 0.5);
    assert_eq!(batch.ending(), Some(Ending::Whole));
    assert!((batch.spent_usd() - 2.5).abs() < 1e-9);
}

#[test]
fn reaching_the_limit_stops_those_running_starts_no_more_and_keeps_those_graded() {
    let mut batch = Batch::new(5, 1.0, 2);
    assert_eq!(start_all(&mut batch), vec![0, 1]);
    batch.ended(0, 0.4);
    assert_eq!(start_all(&mut batch), vec![2]);
    assert!(
        !batch.spent(1, 1, Some(0.3)),
        "0.7 of 1.00: under the limit"
    );
    assert!(
        batch.spent(2, 1, Some(0.3)),
        "1.00 of 1.00: the running grades stop"
    );
    assert_eq!(
        start_all(&mut batch),
        Vec::<usize>::new(),
        "none starts past the limit"
    );
    assert_eq!(batch.ending(), None, "two still end");
    batch.ended(1, 0.3);
    batch.ended(2, 0.35);
    assert_eq!(batch.ending(), Some(Ending::Limit));
    let states: Vec<_> = batch.states().to_vec();
    assert_eq!(
        states,
        vec![
            State::Ended,
            State::Ended,
            State::Ended,
            State::NotRun { why: NotRun::Limit },
            State::NotRun { why: NotRun::Limit },
        ]
    );
}

#[test]
fn a_grade_already_over_the_limit_when_it_ends_is_kept_as_graded() {
    let mut batch = Batch::new(2, 1.0, 1);
    assert_eq!(start_all(&mut batch), vec![0]);
    batch.ended(0, 1.2);
    assert_eq!(batch.ending(), Some(Ending::Limit));
    assert_eq!(
        batch.states(),
        &[State::Ended, State::NotRun { why: NotRun::Limit }]
    );
}

#[test]
fn a_chat_that_did_not_say_what_it_spent_stops_the_batch_as_it_cannot_be_held() {
    let mut batch = Batch::new(3, 5.0, 2);
    start_all(&mut batch);
    assert!(
        !batch.spent(0, 0, None),
        "nothing ran yet, so nothing was left unsaid"
    );
    assert!(batch.spent(1, 1, None));
    batch.ended(0, 0.1);
    batch.ended(1, 0.0);
    assert_eq!(batch.ending(), Some(Ending::SpendNotReported));
    assert_eq!(
        batch.states()[2],
        State::NotRun {
            why: NotRun::SpendNotReported
        }
    );
}

#[test]
fn stop_ends_those_running_and_none_waiting_starts() {
    let mut batch = Batch::new(4, 5.0, 2);
    start_all(&mut batch);
    batch.stop();
    assert_eq!(batch.next(), None);
    batch.ended(0, 0.1);
    batch.ended(1, 0.1);
    assert_eq!(batch.ending(), Some(Ending::Stopped));
    assert_eq!(
        batch.states()[3],
        State::NotRun {
            why: NotRun::Stopped
        }
    );
}

#[test]
fn a_grade_refused_at_its_start_is_named_with_why_and_the_next_takes_its_place() {
    let mut batch = Batch::new(3, 5.0, 1);
    assert_eq!(batch.next(), Some(0));
    batch.refused(0, "It changed since the estimate.".to_owned());
    assert_eq!(batch.next(), Some(1));
    batch.ended(1, 0.2);
    assert_eq!(batch.next(), Some(2));
    batch.ended(2, 0.2);
    assert_eq!(batch.ending(), Some(Ending::Whole));
    assert_eq!(
        batch.states()[0],
        State::NotRun {
            why: NotRun::Refused("It changed since the estimate.".to_owned())
        }
    );
}

#[test]
fn a_spend_told_twice_counts_once() {
    let mut batch = Batch::new(1, 1.0, 1);
    start_all(&mut batch);
    assert!(!batch.spent(0, 1, Some(0.6)));
    assert!(
        !batch.spent(0, 2, Some(0.9)),
        "the grade's spend so far, not more spend"
    );
    assert!((batch.spent_usd() - 0.9).abs() < 1e-9);
}

#[test]
fn the_estimate_adds_what_is_known_and_counts_what_is_not() {
    let estimate = estimate(&[Some(0.5), None, Some(1.25), None]);
    assert_eq!(
        estimate,
        Estimate {
            known_usd: 1.75,
            known: 2,
            unknown: 2
        }
    );
}

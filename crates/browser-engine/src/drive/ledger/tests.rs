// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;

use super::{Begin, Ledger};
use crate::drive::CallId;

fn id(text: &str) -> CallId {
    CallId::new(text).unwrap()
}

/// Runs `call` under `id` the way the browser does: act only when the ledger says the call is
/// fresh, and keep the answer.
fn run(
    ledger: &Ledger<&'static str, String>,
    id: &CallId,
    call: &'static str,
    acted: &AtomicUsize,
) -> Result<String, &'static str> {
    match ledger.begin(id, &call) {
        Begin::Fresh(ticket) => {
            acted.fetch_add(1, Ordering::SeqCst);
            let answer = format!("did {call}");
            ticket.finish(answer.clone());
            Ok(answer)
        }
        Begin::Repeat(answer) => Ok(answer),
        Begin::Reused => Err("reused"),
    }
}

#[test]
fn a_retried_call_with_the_same_id_answers_again_without_acting() {
    let ledger = Ledger::new(8);
    let acted = AtomicUsize::new(0);
    let first = run(&ledger, &id("c1"), "click e3", &acted);
    let again = run(&ledger, &id("c1"), "click e3", &acted);
    assert_eq!(first, Ok("did click e3".to_owned()));
    assert_eq!(again, first);
    assert_eq!(acted.load(Ordering::SeqCst), 1);
}

#[test]
fn a_new_id_acts_again() {
    let ledger = Ledger::new(8);
    let acted = AtomicUsize::new(0);
    run(&ledger, &id("c1"), "click e3", &acted).unwrap();
    run(&ledger, &id("c2"), "click e3", &acted).unwrap();
    assert_eq!(acted.load(Ordering::SeqCst), 2);
}

#[test]
fn an_id_given_to_a_different_call_is_refused_and_nothing_acts() {
    let ledger = Ledger::new(8);
    let acted = AtomicUsize::new(0);
    run(&ledger, &id("c1"), "click e3", &acted).unwrap();
    assert_eq!(run(&ledger, &id("c1"), "click e4", &acted), Err("reused"));
    assert_eq!(acted.load(Ordering::SeqCst), 1);
}

#[test]
fn a_retry_that_arrives_while_the_first_still_runs_waits_for_its_answer() {
    let ledger = Arc::new(Ledger::<&'static str, String>::new(8));
    let Begin::Fresh(ticket) = ledger.begin(&id("c1"), &"click e3") else {
        panic!("the first call is fresh");
    };
    let retry = {
        let ledger = ledger.clone();
        thread::spawn(move || match ledger.begin(&id("c1"), &"click e3") {
            Begin::Repeat(answer) => answer,
            _ => panic!("a retry of a running call repeats its answer"),
        })
    };
    thread::sleep(Duration::from_millis(50));
    assert!(!retry.is_finished(), "the retry waits for the first call");
    ticket.finish("did click e3".to_owned());
    assert_eq!(retry.join().unwrap(), "did click e3");
}

#[test]
fn a_call_that_ends_without_an_answer_leaves_its_id_free() {
    let ledger = Ledger::<&'static str, String>::new(8);
    let acted = AtomicUsize::new(0);
    match ledger.begin(&id("c1"), &"click e3") {
        Begin::Fresh(ticket) => drop(ticket),
        _ => panic!("fresh"),
    }
    run(&ledger, &id("c1"), "click e3", &acted).unwrap();
    assert_eq!(acted.load(Ordering::SeqCst), 1);
}

#[test]
fn the_ledger_keeps_the_latest_answers_up_to_its_capacity() {
    let ledger = Ledger::new(2);
    let acted = AtomicUsize::new(0);
    run(&ledger, &id("c1"), "a", &acted).unwrap();
    run(&ledger, &id("c2"), "b", &acted).unwrap();
    run(&ledger, &id("c3"), "c", &acted).unwrap();
    // c2 and c3 are kept, c1 is forgotten.
    run(&ledger, &id("c3"), "c", &acted).unwrap();
    run(&ledger, &id("c2"), "b", &acted).unwrap();
    assert_eq!(acted.load(Ordering::SeqCst), 3);
    run(&ledger, &id("c1"), "a", &acted).unwrap();
    assert_eq!(acted.load(Ordering::SeqCst), 4);
}

#[test]
fn a_call_id_is_short_and_not_empty() {
    assert!(CallId::new("").is_err());
    assert!(CallId::new("x".repeat(129)).is_err());
    assert!(CallId::new("call-7").is_ok());
}

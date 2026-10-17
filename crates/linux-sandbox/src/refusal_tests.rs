// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

use std::sync::mpsc;
use std::time::{Duration, Instant};

fn write(target: &str) -> Refusal {
    Refusal {
        operation: "file-write-create",
        target: target.to_owned(),
        process: Some("touch".to_owned()),
    }
}

/// A teller whose listener hands each refusal it is told to the test.
fn heard() -> (Teller, mpsc::Receiver<Refusal>) {
    let (send, heard) = mpsc::channel();
    let send = std::sync::Mutex::new(send);
    let teller = Teller::new(Box::new(move |refusal: &Refusal| {
        let _ = send.lock().unwrap().send(refusal.clone());
    }));
    (teller, heard)
}

fn all(heard: &mpsc::Receiver<Refusal>) -> Vec<Refusal> {
    std::iter::from_fn(|| heard.recv_timeout(Duration::from_millis(300)).ok()).collect()
}

#[test]
fn each_refusal_is_told_with_its_operation_target_and_process() {
    let (teller, heard) = heard();
    teller.refused(write("/home/dev/outside"), Instant::now());
    assert_eq!(all(&heard), vec![write("/home/dev/outside")]);
}

#[test]
fn the_same_refusal_again_within_its_window_is_told_once() {
    let (teller, heard) = heard();
    let now = Instant::now();
    for _ in 0..1000 {
        teller.refused(write("/home/dev/outside"), now);
    }
    teller.refused(write("/home/dev/other"), now);
    assert_eq!(
        all(&heard),
        vec![write("/home/dev/outside"), write("/home/dev/other")]
    );
    // Its window over, it is told again: the person may have put the first Notice away.
    teller.refused(write("/home/dev/outside"), now + WINDOW);
    assert_eq!(all(&heard), vec![write("/home/dev/outside")]);
}

#[test]
fn a_flood_of_different_refusals_is_told_at_most_a_handful_a_window() {
    let (teller, heard) = heard();
    let now = Instant::now();
    for n in 0..1000 {
        teller.refused(write(&format!("/home/dev/{n}")), now);
    }
    assert_eq!(all(&heard).len(), PER_WINDOW);
}

#[test]
fn a_listener_that_never_returns_holds_up_no_refusal() {
    // Nothing is ever sent on `_held`, so the listener waits for ever.
    let (_held, never) = mpsc::channel::<()>();
    let never = std::sync::Mutex::new(never);
    let teller = Teller::new(Box::new(move |_: &Refusal| {
        let _ = never.lock().unwrap().recv();
    }));
    let started = Instant::now();
    let now = Instant::now();
    for n in 0..1000 {
        teller.refused(write(&format!("/home/dev/{n}")), now + WINDOW * n);
    }
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "telling waited on its listener: {:?}",
        started.elapsed()
    );
}

#[test]
fn a_listener_that_panics_holds_up_no_refusal_after_it() {
    let teller = Teller::new(Box::new(|_: &Refusal| panic!("a listener that fails")));
    let now = Instant::now();
    for n in 0..100 {
        teller.refused(write(&format!("/home/dev/{n}")), now + WINDOW * n);
    }
}

#[test]
fn what_is_still_being_told_as_the_chat_ends_is_waited_for_a_moment() {
    let (send, heard) = mpsc::channel();
    let send = std::sync::Mutex::new(send);
    let teller = Teller::new(Box::new(move |refusal: &Refusal| {
        std::thread::sleep(Duration::from_millis(200));
        let _ = send.lock().unwrap().send(refusal.clone());
    }));
    teller.refused(write("/home/dev/outside"), Instant::now());
    teller.flush(Duration::from_secs(5));
    assert_eq!(heard.try_recv().ok(), Some(write("/home/dev/outside")));
}

#[test]
fn a_listener_that_never_returns_holds_up_the_end_no_longer_than_the_moment() {
    let (_held, never) = mpsc::channel::<()>();
    let never = std::sync::Mutex::new(never);
    let teller = Teller::new(Box::new(move |_: &Refusal| {
        let _ = never.lock().unwrap().recv();
    }));
    teller.refused(write("/home/dev/outside"), Instant::now());
    let started = Instant::now();
    teller.flush(Duration::from_millis(300));
    assert!(started.elapsed() < Duration::from_secs(2));
}

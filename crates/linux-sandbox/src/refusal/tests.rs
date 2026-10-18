// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

use std::sync::atomic::AtomicUsize;
use std::sync::mpsc;
use std::time::{Duration, Instant};

const WRITE: &str = "file-write-create";

fn write(target: &str) -> Refusal {
    Refusal {
        operation: WRITE,
        target: target.to_owned(),
        process: Some("touch".to_owned()),
    }
}

/// Tells `teller` a write of `target` refused at `now`, by `touch`.
fn refuse(teller: &Teller, target: &str, now: Instant) {
    teller.refused(WRITE, target.to_owned(), || Some("touch".to_owned()), now);
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
    refuse(&teller, "/home/dev/outside", Instant::now());
    assert_eq!(all(&heard), vec![write("/home/dev/outside")]);
}

#[test]
fn the_same_refusal_again_within_its_window_is_told_once() {
    let (teller, heard) = heard();
    let now = Instant::now();
    for _ in 0..1000 {
        refuse(&teller, "/home/dev/outside", now);
    }
    refuse(&teller, "/home/dev/other", now);
    assert_eq!(
        all(&heard),
        vec![write("/home/dev/outside"), write("/home/dev/other")]
    );
    // Its window over, it is told again: the person may have put the first Notice away.
    refuse(&teller, "/home/dev/outside", now + WINDOW);
    assert_eq!(all(&heard), vec![write("/home/dev/outside")]);
}

#[test]
fn the_process_is_read_only_for_a_refusal_that_is_told() {
    let (teller, heard) = heard();
    let read = AtomicUsize::new(0);
    let now = Instant::now();
    for _ in 0..100 {
        teller.refused(
            WRITE,
            "/home/dev/outside".to_owned(),
            || {
                read.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Some("touch".to_owned())
            },
            now,
        );
    }
    assert_eq!(all(&heard), vec![write("/home/dev/outside")]);
    assert_eq!(read.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[test]
fn a_flood_of_different_refusals_is_told_at_most_a_handful_a_window() {
    let (teller, heard) = heard();
    let now = Instant::now();
    for n in 0..1000 {
        refuse(&teller, &format!("/home/dev/{n}"), now);
    }
    assert_eq!(all(&heard).len(), PER_WINDOW);
}

#[test]
fn a_refusal_the_queue_could_not_take_is_told_when_it_comes_again() {
    // The listener waits on `gate` for the first refusal, so the queue behind it fills.
    let (open, gate) = mpsc::channel::<()>();
    let gate = std::sync::Mutex::new(gate);
    let (send, heard) = mpsc::channel();
    let send = std::sync::Mutex::new(send);
    let teller = Teller::new(Box::new(move |refusal: &Refusal| {
        if refusal.target == "/home/dev/first" {
            let _ = gate.lock().unwrap().recv();
        }
        let _ = send.lock().unwrap().send(refusal.clone());
    }));
    let now = Instant::now();
    refuse(&teller, "/home/dev/first", now);
    // Taken off the queue by the listener, which now waits.
    std::thread::sleep(Duration::from_millis(100));
    // Half the queue in this window, half in the next: the queue is full, and the next
    // window's throttle has room.
    let half = QUEUED / 2;
    for n in 0..half {
        refuse(&teller, &format!("/home/dev/{n}"), now);
    }
    let later = now + WINDOW;
    for n in half..QUEUED {
        refuse(&teller, &format!("/home/dev/{n}"), later);
    }
    refuse(&teller, "/home/dev/untaken", later);
    open.send(()).unwrap();
    assert_eq!(
        all(&heard).len(),
        1 + QUEUED,
        "the untaken one was not told"
    );
    // It was never told, so it is told when it comes again, in the same window.
    refuse(&teller, "/home/dev/untaken", later);
    assert_eq!(all(&heard), vec![write("/home/dev/untaken")]);
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
        refuse(&teller, &format!("/home/dev/{n}"), now + WINDOW * n);
    }
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "telling waited on its listener: {:?}",
        started.elapsed()
    );
}

/// Where a build unwinds (tests do; a release build aborts, ending the wrap and its chat), a
/// listener that failed on one refusal is told the next.
#[test]
fn a_listener_that_fails_on_one_refusal_is_told_the_next() {
    let (send, heard) = mpsc::channel();
    let send = std::sync::Mutex::new(send);
    let teller = Teller::new(Box::new(move |refusal: &Refusal| {
        assert_ne!(refusal.target, "/home/dev/failing", "a listener that fails");
        let _ = send.lock().unwrap().send(refusal.clone());
    }));
    let now = Instant::now();
    refuse(&teller, "/home/dev/failing", now);
    refuse(&teller, "/home/dev/next", now);
    assert_eq!(all(&heard), vec![write("/home/dev/next")]);
}

#[test]
fn what_is_still_being_told_as_the_chat_ends_is_waited_for_a_moment() {
    let (send, heard) = mpsc::channel();
    let send = std::sync::Mutex::new(send);
    let teller = Teller::new(Box::new(move |refusal: &Refusal| {
        std::thread::sleep(Duration::from_millis(200));
        let _ = send.lock().unwrap().send(refusal.clone());
    }));
    refuse(&teller, "/home/dev/outside", Instant::now());
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
    refuse(&teller, "/home/dev/outside", Instant::now());
    let started = Instant::now();
    teller.flush(Duration::from_millis(300));
    assert!(started.elapsed() < Duration::from_secs(2));
}

fn dialled(target: &str) -> Refusal {
    Refusal {
        operation: "network-outbound",
        target: target.to_owned(),
        process: Some("curl".to_owned()),
    }
}

#[test]
fn an_address_dialled_past_the_proxy_is_told_once_a_window_whatever_the_address() {
    let (teller, heard) = heard();
    let now = Instant::now();
    for n in 0..50 {
        teller.past_the_proxy(
            format!("93.184.216.{n}:443"),
            || Some("curl".to_owned()),
            now,
        );
    }
    assert_eq!(all(&heard), vec![dialled("93.184.216.0:443")]);
    teller.past_the_proxy(
        "93.184.216.9:443".to_owned(),
        || Some("curl".to_owned()),
        now + WINDOW,
    );
    assert_eq!(all(&heard), vec![dialled("93.184.216.9:443")]);
}

#[test]
fn addresses_dialled_past_the_proxy_take_nothing_of_the_other_refusals_window() {
    let (teller, heard) = heard();
    let now = Instant::now();
    for n in 0..50 {
        teller.past_the_proxy(format!("10.0.0.{n}:443"), || None, now);
    }
    let _ = all(&heard);
    for n in 0..PER_WINDOW {
        refuse(&teller, &format!("/home/dev/{n}"), now);
    }
    assert_eq!(all(&heard).len(), PER_WINDOW);
}

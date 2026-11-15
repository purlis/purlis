// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The one clock deadlines are read against, moved by hand in a test (R-696).

use std::sync::Arc;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use purlis_core::clock::{Clock, Deadline, InjectedClock};

#[test]
fn a_deadline_fires_when_the_injected_clock_passes_it() {
    let started = Instant::now();
    let clock = Arc::new(InjectedClock::new());
    let deadline = Deadline::after(&*clock, Duration::from_secs(30));

    clock.advance(Duration::from_secs(29));
    assert!(!deadline.passed(&*clock), "fired at 29 s");

    // A waiter on the deadline wakes only once the clock reaches it.
    let (woke, waking) = mpsc::channel();
    let waiter = {
        let clock = Arc::clone(&clock);
        std::thread::spawn(move || {
            clock.sleep_until(deadline.at());
            let _ = woke.send(());
        })
    };
    while clock.sleepers() == 0 {
        std::thread::yield_now();
    }
    assert!(waking.try_recv().is_err(), "woke at 29 s");

    clock.advance(Duration::from_secs(1));
    assert!(deadline.passed(&*clock), "did not fire at 30 s");
    waiter.join().unwrap();
    assert!(waking.try_recv().is_ok());
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "the test waited on the wall clock"
    );
}

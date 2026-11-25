// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::sync::mpsc;
use std::time::{Duration, Instant};

use super::*;

#[test]
fn posting_an_event_never_waits_on_a_handler_that_is_stuck() {
    let (stuck, release) = mpsc::channel::<()>();
    let (started, is_stuck) = mpsc::channel::<()>();
    let lane = Lane::new("test lane", move |_: u32| {
        let _ = started.send(());
        let _ = release.recv_timeout(Duration::from_secs(5));
    });
    lane.post("a", 1);
    is_stuck.recv().unwrap();

    let began = Instant::now();
    for frame in 0..1_000 {
        lane.post("moved", frame);
    }

    assert!(
        began.elapsed() < Duration::from_millis(100),
        "a thousand posts took {:?} behind a stuck handler",
        began.elapsed()
    );
    let _ = stuck.send(());
}

#[test]
fn events_waiting_behind_a_slow_handler_are_one_per_key_with_the_latest_and_in_order() {
    let (stuck, release) = mpsc::channel::<()>();
    let (started, is_stuck) = mpsc::channel::<()>();
    let (handled, seen) = mpsc::channel::<u32>();
    let lane = Lane::new("test lane", move |event: u32| {
        if event == 0 {
            let _ = started.send(());
            let _ = release.recv_timeout(Duration::from_secs(5));
        }
        let _ = handled.send(event);
    });
    lane.post("first", 0);
    is_stuck.recv().unwrap();
    lane.post("moved", 1);
    lane.post("focused", 2);
    lane.post("moved", 3);
    lane.post("moved", 4);
    stuck.send(()).unwrap();

    let got: Vec<u32> = (0..3)
        .map(|_| seen.recv_timeout(Duration::from_secs(5)).unwrap())
        .collect();
    assert_eq!(got, vec![0, 4, 2]);
    assert!(seen.recv_timeout(Duration::from_millis(100)).is_err());
}

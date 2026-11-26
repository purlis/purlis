// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;

/// A main thread that runs each ping as it is handed over, except the first, which it runs
/// only after `held_for` spent on the command `holding`: `pings` pings, then no more.
fn a_main_thread(
    running: &'static Running,
    holding: &'static str,
    held_for: Duration,
    pings: usize,
) -> impl Fn(Box<dyn FnOnce() + Send>) -> bool {
    let handed = Arc::new(AtomicUsize::new(0));
    move |ping| {
        let n = handed.fetch_add(1, Ordering::SeqCst);
        if n >= pings {
            return false;
        }
        if n == 0 && held_for > Duration::ZERO {
            std::thread::spawn(move || {
                held(holding, running, None, HELD_LONG, || {
                    std::thread::sleep(held_for)
                });
                ping();
            });
        } else {
            ping();
        }
        true
    }
}

#[test]
fn a_main_thread_held_past_the_budget_is_a_core_stall_naming_the_command_that_held_it() {
    static RUNNING: Running = Running::new();
    let (sink, seen) = mpsc::channel();

    watch(
        a_main_thread(&RUNNING, "chat_order", Duration::from_millis(350), 3),
        &RUNNING,
        &sink,
        Duration::from_millis(5),
        STALLED,
    );

    let stalls: Vec<Slow> = seen.try_iter().collect();
    assert_eq!(stalls.len(), 1, "{stalls:?}");
    let Slow::CoreStall { ms, at, running } = &stalls[0] else {
        panic!("not a core stall: {:?}", stalls[0]);
    };
    assert!(*ms >= 350, "{ms}");
    assert!(*at > 0);
    assert_eq!(running.as_deref(), Some("chat_order"));
}

#[test]
fn a_main_thread_that_runs_each_ping_at_once_is_no_stall() {
    static RUNNING: Running = Running::new();
    let (sink, seen) = mpsc::channel();

    watch(
        a_main_thread(&RUNNING, "send_input", Duration::ZERO, 5),
        &RUNNING,
        &sink,
        Duration::from_millis(5),
        STALLED,
    );

    assert_eq!(seen.try_iter().count(), 0);
}

#[test]
fn a_command_that_holds_the_main_thread_long_is_written_with_when_it_was_received() {
    let running = Running::default();
    let (sink, seen) = mpsc::channel();
    let before = now_ms();

    let answer = held("resize_session", &running, Some(&sink), HELD_LONG, || {
        assert_eq!(running.now().as_deref(), Some("resize_session"));
        std::thread::sleep(Duration::from_millis(120));
        7
    });

    assert_eq!(answer, 7);
    assert_eq!(running.now(), None, "named only while it runs");
    let Ok(Slow::CoreCommand { command, ms, at }) = seen.try_recv() else {
        panic!("no core command written");
    };
    assert_eq!(command, "resize_session");
    assert!(ms >= 120, "{ms}");
    assert!(
        at >= before && at <= before + 50,
        "{at} received, asked at {before}"
    );
}

#[test]
fn a_command_that_answers_quickly_is_not_written() {
    let running = Running::default();
    let (sink, seen) = mpsc::channel();

    held("send_input", &running, Some(&sink), HELD_LONG, || ());

    assert_eq!(seen.try_iter().count(), 0);
}

#[test]
fn a_command_off_the_main_thread_is_run_and_never_timed() {
    // No setup ran on this thread, so it is not the main thread.
    assert_eq!(command("send_input", || 3), 3);
    assert_eq!(RUNNING.now(), None);
}

#[test]
fn a_main_thread_waiting_on_a_lock_another_thread_holds_is_a_core_stall_naming_the_line_that_waits()
{
    static RUNNING: Running = Running::new();
    static HELD: Mutex<u8> = Mutex::new(0);
    let (sink, seen) = mpsc::channel();
    let (holding, held_now) = mpsc::channel::<()>();
    // A hook's thread, holding the lock across work that takes a while.
    let holder = std::thread::spawn(move || {
        let _held = HELD.lock().unwrap();
        holding.send(()).unwrap();
        std::thread::sleep(Duration::from_millis(400));
    });
    held_now.recv().unwrap();

    // The main thread: a window event, which asks for the lock on this line.
    let asks_at = std::panic::Location::caller();
    let waiting = |ping: Box<dyn FnOnce() + Send>| {
        std::thread::spawn(move || {
            held("window event: focused", &RUNNING, None, HELD_LONG, || {
                drop(lock_in(&HELD, asks_at, &RUNNING, None));
            });
            ping();
        });
        true
    };
    let handed = std::sync::atomic::AtomicBool::new(false);
    watch(
        |ping| {
            if handed.swap(true, Ordering::SeqCst) {
                return false;
            }
            waiting(ping)
        },
        &RUNNING,
        &sink,
        Duration::from_millis(5),
        STALLED,
    );
    holder.join().unwrap();

    let stalls: Vec<Slow> = seen.try_iter().collect();
    let Some(Slow::CoreStall { running, .. }) = stalls.first() else {
        panic!("no core stall: {stalls:?}");
    };
    let running = running.as_deref().unwrap_or("nothing");
    assert!(
        running.starts_with("window event: focused > waiting on a lock at ")
            && running.contains("mainwatch/tests.rs"),
        "{running}"
    );
}

#[test]
fn what_the_main_thread_runs_is_told_outermost_first_and_taken_off_as_each_ends() {
    let running = Running::default();
    held("window event: resized", &running, None, HELD_LONG, || {
        held("inner", &running, None, HELD_LONG, || {
            assert_eq!(
                running.now().as_deref(),
                Some("window event: resized > inner")
            );
        });
        assert_eq!(running.now().as_deref(), Some("window event: resized"));
    });
    assert_eq!(running.now(), None);
}

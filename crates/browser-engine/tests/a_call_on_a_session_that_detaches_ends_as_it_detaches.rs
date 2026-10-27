// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A call made on a session the engine then detaches ends as the engine says so (#3081). The
//! engine does not always answer a call to a frame's session that went away while the call
//! waited, for instance a frame that loads another document every 100 ms: such a call waited
//! its whole limit, and where it was made on the thread that reads the engine's events, no
//! other event was read meanwhile.

use std::io::{BufRead, BufReader, Write};
use std::sync::Arc;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use purlis_browser_engine::cdp::{CallError, Connection};
use serde_json::{Value, json};

#[test]
fn a_call_on_a_session_that_detaches_unanswered_ends_as_it_detaches_and_others_wait_on() {
    let (purlis_reads, mut engine_writes) = std::io::pipe().unwrap();
    let (engine_reads, purlis_writes) = std::io::pipe().unwrap();
    let (connection, events) = Connection::new(purlis_reads, purlis_writes);
    let connection = Arc::new(connection);
    let mut from_purlis = BufReader::new(engine_reads);

    let (done, answered) = mpsc::channel();
    for (method, session) in [("Page.enable", "FRAME-1"), ("Page.enable", "PAGE-1")] {
        let connection = Arc::clone(&connection);
        let done = done.clone();
        std::thread::spawn(move || {
            let started = Instant::now();
            let answer =
                connection.call_within(method, json!({}), Some(session), Duration::from_secs(20));
            let _ = done.send((session, answer, started.elapsed()));
        });
    }
    // Both calls reach the engine, which answers neither.
    for _ in 0..2 {
        let mut message = Vec::new();
        from_purlis.read_until(0, &mut message).unwrap();
    }

    // The frame's session goes, with its call unanswered.
    let detached = json!({
        "method": "Target.detachedFromTarget",
        "params": { "sessionId": "FRAME-1", "targetId": "FRAME" },
        "sessionId": "PAGE-1",
    });
    let mut bytes = serde_json::to_vec(&detached).unwrap();
    bytes.push(0);
    engine_writes.write_all(&bytes).unwrap();

    let (session, answer, took) = answered
        .recv_timeout(Duration::from_secs(5))
        .expect("the call on the detached session ended as it detached, not at its limit");
    assert_eq!(session, "FRAME-1");
    assert!(
        matches!(answer, Err(CallError::Engine { .. })),
        "{answer:?}"
    );
    assert!(took < Duration::from_secs(5), "{took:?}");
    // The page's own call still waits for its answer.
    assert!(answered.recv_timeout(Duration::from_millis(300)).is_err());
    // And whoever reads the events is told of the detach, as before.
    let told = events.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(told.method, "Target.detachedFromTarget");
    assert_eq!(told.params["sessionId"], Value::from("FRAME-1"));
}

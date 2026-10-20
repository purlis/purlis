// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Every call over the engine's pipe ends within its limit, writing included (#2123): an engine
//! that stops reading fills the pipe, and a call whose message cannot all be written fails in
//! time, as does every call made while it waits.

use std::io::PipeReader;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use purlis_browser_engine::cdp::{CallError, Connection};
use serde_json::json;

/// A connection to an engine that never reads: the end it would read from is held open.
fn to_an_engine_that_never_reads() -> (Connection, PipeReader, std::io::PipeWriter) {
    let (purlis_reads, engine_writes) = std::io::pipe().unwrap();
    let (engine_reads, purlis_writes) = std::io::pipe().unwrap();
    let (connection, _events) = Connection::new(purlis_reads, purlis_writes);
    (connection, engine_reads, engine_writes)
}

/// Runs `call` on a thread of its own and answers what it answered and how long it took, or
/// fails the test when it has not answered within ten seconds.
fn within_ten_seconds<T: Send + 'static>(
    call: impl FnOnce() -> T + Send + 'static,
) -> (T, Duration) {
    let (done, answered) = mpsc::channel();
    std::thread::spawn(move || {
        let started = Instant::now();
        let answer = call();
        let _ = done.send((answer, started.elapsed()));
    });
    answered
        .recv_timeout(Duration::from_secs(10))
        .expect("the call never ended")
}

#[test]
fn a_call_too_large_for_a_pipe_nobody_reads_fails_within_its_limit() {
    let (connection, _unread, _unanswered) = to_an_engine_that_never_reads();
    let ((answer, _connection), took) = within_ten_seconds(move || {
        let answer = connection.call_within(
            "Page.setDocumentContent",
            json!({ "html": "x".repeat(4 << 20) }),
            None,
            Duration::from_millis(200),
        );
        (answer, connection)
    });
    assert!(matches!(answer, Err(CallError::TimedOut)), "{answer:?}");
    assert!(took < Duration::from_secs(2), "{took:?}");
}

#[test]
fn a_call_made_while_another_cannot_be_written_fails_within_its_own_limit() {
    let (connection, _unread, _unanswered) = to_an_engine_that_never_reads();
    let connection = std::sync::Arc::new(connection);
    let stuck = std::sync::Arc::clone(&connection);
    std::thread::spawn(move || {
        let _ = stuck.call_within(
            "Page.setDocumentContent",
            json!({ "html": "x".repeat(4 << 20) }),
            None,
            Duration::from_secs(30),
        );
    });
    std::thread::sleep(Duration::from_millis(100));
    let (answer, took) = within_ten_seconds(move || {
        connection.call_within("Browser.close", json!({}), None, Duration::from_millis(300))
    });
    assert!(matches!(answer, Err(CallError::TimedOut)), "{answer:?}");
    assert!(took < Duration::from_secs(2), "{took:?}");
}

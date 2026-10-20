// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! DevTools over `--remote-debugging-pipe` is JSON, one message per NUL byte, in each
//! direction. These tests play the engine's end of two pipes and check what the connection
//! writes and what it makes of what it reads.

use std::io::{BufRead, BufReader, PipeReader, PipeWriter, Write};
use std::thread;
use std::time::Duration;

use purlis_browser_engine::cdp::{CallError, Connection, MAX_MESSAGE_BYTES, MAX_QUEUED_EVENTS};
use serde_json::{Value, json};

/// The engine's end: what the connection wrote, and a way to answer.
struct FakeEngine {
    from_purlis: BufReader<PipeReader>,
    to_purlis: PipeWriter,
}

impl FakeEngine {
    fn read(&mut self) -> Value {
        let mut message = Vec::new();
        self.from_purlis.read_until(0, &mut message).unwrap();
        assert_eq!(
            message.pop(),
            Some(0),
            "every message ends with one NUL byte"
        );
        serde_json::from_slice(&message).unwrap()
    }

    fn send(&mut self, message: Value) {
        let mut bytes = serde_json::to_vec(&message).unwrap();
        bytes.push(0);
        self.to_purlis.write_all(&bytes).unwrap();
    }
}

fn connect() -> (
    Connection,
    std::sync::mpsc::Receiver<purlis_browser_engine::cdp::Event>,
    FakeEngine,
) {
    let (purlis_reads, engine_writes) = std::io::pipe().unwrap();
    let (engine_reads, purlis_writes) = std::io::pipe().unwrap();
    let (connection, events) = Connection::new(purlis_reads, purlis_writes);
    (
        connection,
        events,
        FakeEngine {
            from_purlis: BufReader::new(engine_reads),
            to_purlis: engine_writes,
        },
    )
}

#[test]
fn a_call_is_one_nul_ended_message_and_its_answer_is_matched_by_id() {
    let (connection, _events, mut engine) = connect();
    let engine = thread::spawn(move || {
        let first = engine.read();
        let second = engine.read();
        assert_eq!(first["method"], "Browser.getVersion");
        assert_eq!(second["method"], "Target.getTargets");
        assert_eq!(second["sessionId"], "S1");
        assert_ne!(first["id"], second["id"]);
        // Answered out of order: each call still gets its own.
        engine.send(json!({"id": second["id"], "result": {"targetInfos": []}}));
        engine.send(json!({"id": first["id"], "result": {"product": "Chrome/155"}}));
        engine
    });
    let connection = std::sync::Arc::new(connection);
    let other = connection.clone();
    let version = thread::spawn(move || other.call("Browser.getVersion", json!({}), None));
    // The second call is written after the first, so the engine's reads are in this order.
    thread::sleep(Duration::from_millis(100));
    let targets = connection
        .call("Target.getTargets", json!({}), Some("S1"))
        .unwrap();
    assert_eq!(targets, json!({"targetInfos": []}));
    assert_eq!(
        version.join().unwrap().unwrap(),
        json!({"product": "Chrome/155"})
    );
    drop(engine.join().unwrap());
}

#[test]
fn a_message_without_an_id_is_an_event_with_its_session() {
    let (connection, events, mut engine) = connect();
    engine.send(
        json!({"method": "Page.loadEventFired", "params": {"timestamp": 1.5}, "sessionId": "S2"}),
    );
    let event = events.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(event.method, "Page.loadEventFired");
    assert_eq!(event.params, json!({"timestamp": 1.5}));
    assert_eq!(event.session_id.as_deref(), Some("S2"));
    drop(connection);
}

#[test]
fn an_error_answer_is_the_engines_own_words() {
    let (connection, _events, mut engine) = connect();
    let engine = thread::spawn(move || {
        let call = engine.read();
        engine.send(json!({"id": call["id"], "error": {"code": -32601, "message": "'Nope.nope' wasn't found"}}));
        engine
    });
    let error = connection.call("Nope.nope", json!({}), None).unwrap_err();
    assert!(
        matches!(&error, CallError::Engine { code: -32601, message } if message.contains("wasn't found")),
        "{error:?}"
    );
    drop(engine.join().unwrap());
}

#[test]
fn every_waiting_call_ends_when_the_engine_closes_its_pipe_and_later_calls_are_refused() {
    let (connection, _events, mut engine) = connect();
    let engine = thread::spawn(move || {
        let _ = engine.read();
        drop(engine);
    });
    let error = connection
        .call("Browser.getVersion", json!({}), None)
        .unwrap_err();
    assert!(matches!(error, CallError::Ended), "{error:?}");
    engine.join().unwrap();
    let error = connection
        .call("Browser.getVersion", json!({}), None)
        .unwrap_err();
    assert!(matches!(error, CallError::Ended), "{error:?}");
}

#[test]
fn a_call_the_engine_never_answers_times_out() {
    let (connection, _events, engine) = connect();
    let error = connection
        .call_within(
            "Browser.getVersion",
            json!({}),
            None,
            Duration::from_millis(200),
        )
        .unwrap_err();
    assert!(matches!(error, CallError::TimedOut), "{error:?}");
    drop(engine);
}

#[test]
fn a_message_past_the_bound_that_is_no_devtools_message_ends_the_connection() {
    let (connection, _events, mut engine) = connect();
    let flood = thread::spawn(move || {
        let chunk = vec![b'x'; 1 << 20];
        let mut sent = 0;
        while sent <= MAX_MESSAGE_BYTES {
            if engine.to_purlis.write_all(&chunk).is_err() {
                break;
            }
            sent += chunk.len();
        }
        engine
    });
    let error = connection
        .call_within(
            "Browser.getVersion",
            json!({}),
            None,
            Duration::from_secs(30),
        )
        .unwrap_err();
    assert!(matches!(error, CallError::Ended), "{error:?}");
    drop(flood.join().unwrap());
}

#[test]
fn events_nobody_reads_are_held_to_a_bound_and_the_ones_past_it_are_counted() {
    let (connection, events, mut engine) = connect();
    let engine = thread::spawn(move || {
        for n in 0..MAX_QUEUED_EVENTS + 50 {
            engine.send(json!({"method": "Page.frameNavigated", "params": {"n": n}}));
        }
        let call = engine.read();
        engine.send(json!({"id": call["id"], "result": {}}));
        engine
    });
    // Calls are still answered while the events wait.
    connection
        .call("Browser.getVersion", json!({}), None)
        .unwrap();
    assert_eq!(connection.dropped_events(), 50);
    assert_eq!(events.try_iter().count(), MAX_QUEUED_EVENTS);
    drop(engine.join().unwrap());
}

#[test]
fn what_a_page_writes_to_its_console_is_never_queued_nor_counted() {
    // #2138: the driver reads no console events, so they never take a place another event
    // needs.
    let (connection, events, mut engine) = connect();
    let engine = thread::spawn(move || {
        for n in 0..MAX_QUEUED_EVENTS + 50 {
            for method in [
                "Runtime.consoleAPICalled",
                "Runtime.exceptionThrown",
                "Runtime.exceptionRevoked",
                "Runtime.inspectRequested",
            ] {
                engine.send(json!({"method": method, "params": {"n": n}}));
            }
        }
        engine.send(json!({"method": "Runtime.executionContextCreated", "params": {}}));
        let call = engine.read();
        engine.send(json!({"id": call["id"], "result": {}}));
        engine
    });
    connection
        .call("Browser.getVersion", json!({}), None)
        .unwrap();
    assert_eq!(connection.dropped_events(), 0);
    let queued: Vec<String> = events.try_iter().map(|e| e.method).collect();
    assert_eq!(queued, ["Runtime.executionContextCreated"]);
    drop(engine.join().unwrap());
}

/// An answer of `bytes` bytes or so to the call `call`.
fn answer_of(call: &Value, bytes: usize) -> Value {
    json!({"id": call["id"], "result": {"data": "x".repeat(bytes)}})
}

#[test]
fn an_answer_past_its_calls_bound_fails_only_that_call_and_the_connection_still_answers() {
    // #2694: the answer is read past and dropped; the next one is matched as ever.
    let (connection, _events, mut engine) = connect();
    let engine = thread::spawn(move || {
        let first = engine.read();
        engine.send(answer_of(&first, 64 * 1024));
        let second = engine.read();
        engine.send(json!({"id": second["id"], "result": {"product": "Chrome/155"}}));
        engine
    });
    let error = connection
        .call_within_bounded(
            "DOMSnapshot.captureSnapshot",
            json!({}),
            None,
            Duration::from_secs(30),
            16 * 1024,
        )
        .unwrap_err();
    assert!(
        matches!(error, CallError::TooLarge { most: 16384 }),
        "{error:?}"
    );
    assert!(error.to_string().contains("too large"), "{error}");
    assert_eq!(
        connection
            .call("Browser.getVersion", json!({}), None)
            .unwrap(),
        json!({"product": "Chrome/155"})
    );
    drop(engine.join().unwrap());
}

#[test]
fn an_answer_past_the_connections_bound_fails_only_its_call() {
    let (connection, _events, mut engine) = connect();
    let engine = thread::spawn(move || {
        let first = engine.read();
        engine.send(answer_of(&first, MAX_MESSAGE_BYTES + (1 << 20)));
        let second = engine.read();
        engine.send(json!({"id": second["id"], "result": {}}));
        engine
    });
    let error = connection
        .call("Accessibility.getFullAXTree", json!({}), None)
        .unwrap_err();
    assert!(
        matches!(error, CallError::TooLarge { most } if most == MAX_MESSAGE_BYTES),
        "{error:?}"
    );
    connection
        .call("Browser.getVersion", json!({}), None)
        .unwrap();
    drop(engine.join().unwrap());
}

#[test]
fn an_event_past_the_bound_that_nobody_reads_is_dropped_and_calls_are_still_answered() {
    let (connection, events, mut engine) = connect();
    let engine = thread::spawn(move || {
        engine.send(json!({"method": "Runtime.consoleAPICalled", "params": {"data": "x".repeat(MAX_MESSAGE_BYTES + (1 << 20))}}));
        engine.send(json!({"method": "Page.loadEventFired", "params": {}}));
        let call = engine.read();
        engine.send(json!({"id": call["id"], "result": {}}));
        engine
    });
    connection
        .call("Browser.getVersion", json!({}), None)
        .unwrap();
    assert_eq!(connection.dropped_events(), 0);
    let kept: Vec<_> = events.try_iter().map(|e| e.method).collect();
    assert_eq!(kept, vec!["Page.loadEventFired".to_owned()]);
    drop(engine.join().unwrap());
}

#[test]
fn an_event_past_the_bound_the_driver_reads_ends_the_connection() {
    // A frame's navigation keeps the driver's view of the page: never lost, so never dropped.
    let (connection, _events, mut engine) = connect();
    let engine = thread::spawn(move || {
        let call = engine.read();
        engine.send(json!({"method": "Page.frameNavigated", "params": {"frame": {"name": "x".repeat(MAX_MESSAGE_BYTES + (1 << 20))}}}));
        engine.send(json!({"id": call["id"], "result": {}}));
        engine
    });
    let error = connection
        .call("Browser.getVersion", json!({}), None)
        .unwrap_err();
    assert!(matches!(error, CallError::Ended), "{error:?}");
    drop(engine.join());
}

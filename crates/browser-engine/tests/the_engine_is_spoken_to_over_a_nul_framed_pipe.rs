// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! DevTools over `--remote-debugging-pipe` is JSON, one message per NUL byte, in each
//! direction. These tests play the engine's end of two pipes and check what the connection
//! writes and what it makes of what it reads.

use std::io::{BufRead, BufReader, PipeReader, PipeWriter, Write};
use std::thread;
use std::time::Duration;

use purlis_browser_engine::cdp::{CallError, Connection, MAX_MESSAGE_BYTES};
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
fn a_message_past_the_bound_without_its_nul_ends_the_connection() {
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

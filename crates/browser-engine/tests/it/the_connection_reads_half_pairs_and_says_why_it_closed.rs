// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What the connection makes of a message JSON allows and a strict reader refuses, and what it
//! says when it closes (#2745). These play the engine's end of two pipes.
//!
//! The engine writes every character of a JavaScript string past ASCII as a `\uXXXX` escape, so
//! a string that holds half of a surrogate pair (an emoji cut in two by a script) reaches the
//! pipe as a lone `\ud83d`. That is valid JSON, and the connection reads it with U+FFFD in the
//! half's place. Whatever does close the connection is said, without anything the message held.

use std::io::{BufRead, BufReader, PipeReader, PipeWriter, Write};
use std::thread;

use purlis_browser_engine::cdp::{CallError, Closed, Connection, MAX_MESSAGE_BYTES};
use serde_json::{Value, json};

struct FakeEngine {
    from_purlis: BufReader<PipeReader>,
    to_purlis: PipeWriter,
}

impl FakeEngine {
    fn read(&mut self) -> Value {
        let mut message = Vec::new();
        self.from_purlis.read_until(0, &mut message).unwrap();
        message.pop();
        serde_json::from_slice(&message).unwrap()
    }

    fn send(&mut self, message: Value) {
        let mut bytes = serde_json::to_vec(&message).unwrap();
        bytes.push(0);
        self.to_purlis.write_all(&bytes).unwrap();
    }

    /// Sends `bytes` as one message, as the engine wrote them.
    fn send_raw(&mut self, bytes: &[u8]) {
        self.to_purlis.write_all(bytes).unwrap();
        self.to_purlis.write_all(&[0]).unwrap();
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
fn an_event_holding_half_of_a_surrogate_pair_is_read_with_a_replacement_character() {
    let (connection, events, mut engine) = connect();
    let engine = thread::spawn(move || {
        engine.send_raw(
            br#"{"method":"Target.targetInfoChanged","params":{"targetInfo":{"title":"Blog \ud83d"}}}"#,
        );
        let call = engine.read();
        engine.send(json!({"id": call["id"], "result": {}}));
        engine
    });
    connection
        .call("Browser.getVersion", json!({}), None)
        .expect("the connection still answers");
    let event = events.try_recv().expect("the event was read");
    assert_eq!(event.method, "Target.targetInfoChanged");
    assert_eq!(event.params["targetInfo"]["title"], "Blog \u{fffd}");
    assert_eq!(connection.closed(), None);
    drop(engine.join().unwrap());
}

#[test]
fn an_answer_holding_half_pairs_answers_its_call_and_keeps_whole_pairs_and_escaped_backslashes() {
    let (connection, _events, mut engine) = connect();
    let engine = thread::spawn(move || {
        let call = engine.read();
        let id = call["id"].as_u64().unwrap();
        let answer = format!(
            r#"{{"id":{id},"result":{{"lone_trail":"a\udc00b","lone_lead_last":"c\ud83d","whole":"\ud83d\ude00","written":"\\ud83d","lead_then_lead":"\ud83d\ud83d\ude00"}}}}"#
        );
        engine.send_raw(answer.as_bytes());
        engine
    });
    let result = connection
        .call("Accessibility.getFullAXTree", json!({}), None)
        .expect("the call is answered");
    assert_eq!(result["lone_trail"], "a\u{fffd}b");
    assert_eq!(result["lone_lead_last"], "c\u{fffd}");
    assert_eq!(result["whole"], "\u{1f600}");
    assert_eq!(result["written"], "\\ud83d");
    assert_eq!(result["lead_then_lead"], "\u{fffd}\u{1f600}");
    drop(engine.join().unwrap());
}

#[test]
fn bytes_that_are_not_utf8_are_read_as_replacement_characters() {
    let (connection, _events, mut engine) = connect();
    let engine = thread::spawn(move || {
        let call = engine.read();
        let id = call["id"].as_u64().unwrap();
        let mut answer = format!(r#"{{"id":{id},"result":{{"name":"a"#).into_bytes();
        answer.extend_from_slice(&[0xff, 0xfe]);
        answer.extend_from_slice(br#"b"}}"#);
        engine.send_raw(&answer);
        engine
    });
    let result = connection
        .call("Runtime.evaluate", json!({}), None)
        .expect("the call is answered");
    assert_eq!(result["name"], "a\u{fffd}\u{fffd}b");
    drop(engine.join().unwrap());
}

#[test]
fn a_network_event_past_the_bound_is_dropped_and_calls_are_still_answered() {
    // A page's request for a huge `data:` address: the driver reads its network events only to
    // tell when the page has settled, and the reach proxy judges the request itself.
    let (connection, events, mut engine) = connect();
    let engine = thread::spawn(move || {
        engine.send(json!({"method": "Network.requestWillBeSent", "params": {"request": {"url": format!("data:text/plain,{}", "x".repeat(MAX_MESSAGE_BYTES + (1 << 20)))}}}));
        engine.send(json!({"method": "Page.loadEventFired", "params": {}}));
        let call = engine.read();
        engine.send(json!({"id": call["id"], "result": {}}));
        engine
    });
    connection
        .call("Browser.getVersion", json!({}), None)
        .expect("the connection still answers");
    let kept: Vec<_> = events.try_iter().map(|e| e.method).collect();
    assert_eq!(kept, vec!["Page.loadEventFired".to_owned()]);
    assert_eq!(connection.closed(), None);
    drop(engine.join().unwrap());
}

#[test]
fn the_connection_says_the_engine_closed_its_pipe() {
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
    assert_eq!(connection.closed(), Some(Closed::PipeEnded));
}

#[test]
fn an_event_past_the_bound_the_driver_reads_closes_the_connection_naming_its_method_alone() {
    let (connection, _events, mut engine) = connect();
    let engine = thread::spawn(move || {
        let call = engine.read();
        engine.send(json!({"method": "Page.frameNavigated", "params": {"frame": {"name": "secret-frame-name".repeat(MAX_MESSAGE_BYTES / 16)}}}));
        engine.send(json!({"id": call["id"], "result": {}}));
        engine
    });
    let error = connection
        .call("Browser.getVersion", json!({}), None)
        .unwrap_err();
    assert!(matches!(error, CallError::Ended), "{error:?}");
    let closed = connection.closed().expect("it says why it closed");
    assert_eq!(
        closed,
        Closed::TooLarge {
            method: Some("Page.frameNavigated".to_owned()),
            most: MAX_MESSAGE_BYTES,
        }
    );
    let said = closed.to_string();
    assert!(said.contains("Page.frameNavigated"), "{said}");
    assert!(!said.contains("secret-frame-name"), "{said}");
    drop(engine.join());
}

#[test]
fn a_message_that_is_not_json_closes_the_connection_naming_its_method_alone() {
    let (connection, _events, mut engine) = connect();
    let engine = thread::spawn(move || {
        let call = engine.read();
        engine.send_raw(br#"{"method":"Page.frameNavigated","params":{"name":"private words"#);
        engine.send(json!({"id": call["id"], "result": {}}));
        engine
    });
    let error = connection
        .call("Browser.getVersion", json!({}), None)
        .unwrap_err();
    assert!(matches!(error, CallError::Ended), "{error:?}");
    let closed = connection.closed().expect("it says why it closed");
    assert_eq!(
        closed,
        Closed::NotDevTools {
            method: Some("Page.frameNavigated".to_owned())
        }
    );
    assert!(!closed.to_string().contains("private words"));
    drop(engine.join());
}

#[test]
fn an_answer_that_is_not_json_closes_the_connection_naming_the_method_of_its_call() {
    let (connection, _events, mut engine) = connect();
    let engine = thread::spawn(move || {
        let call = engine.read();
        let id = call["id"].as_u64().unwrap();
        engine.send_raw(format!(r#"{{"id":{id},"result":{{"oops"#).as_bytes());
        engine
    });
    let error = connection
        .call("DOMSnapshot.captureSnapshot", json!({}), None)
        .unwrap_err();
    assert!(matches!(error, CallError::Ended), "{error:?}");
    assert_eq!(
        connection.closed(),
        Some(Closed::NotDevTools {
            method: Some("DOMSnapshot.captureSnapshot".to_owned())
        })
    );
    drop(engine.join());
}

#[test]
fn a_method_name_that_is_not_one_is_not_repeated() {
    let (connection, _events, mut engine) = connect();
    let engine = thread::spawn(move || {
        let call = engine.read();
        engine.send_raw(br#"{"method":"what a page wrote","params":"#);
        engine.send(json!({"id": call["id"], "result": {}}));
        engine
    });
    let _ = connection.call("Browser.getVersion", json!({}), None);
    assert_eq!(
        connection.closed(),
        Some(Closed::NotDevTools { method: None })
    );
    drop(engine.join());
}

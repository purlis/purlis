// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A tapped session is relayed whole and in order** (#2028): the DevTools of an Inspect speak
//! to the inspected page over the browser's one pipe. Every message the engine writes on a
//! tapped session, or on a session attached under it, goes to the tap in the order the engine
//! wrote it, console events included, and never to the driver's events; an answer to a call
//! sent on the tap comes back to it under the tap's own id. Nothing of the tap ever closes the
//! connection the driver reads.

use std::io::{BufRead, BufReader, PipeReader, PipeWriter, Write};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use purlis_browser_engine::cdp::{Connection, Event, Tapped};
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
}

fn connect() -> (Connection, Receiver<Event>, FakeEngine) {
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

/// A tap that hands each message it is given on to a channel.
fn tap_sink() -> (Arc<dyn Fn(Tapped) -> bool + Send + Sync>, Receiver<Tapped>) {
    let (to, from) = mpsc::channel();
    let to = Mutex::new(to);
    (
        Arc::new(move |tapped| to.lock().unwrap().send(tapped).is_ok()),
        from,
    )
}

fn next(from: &Receiver<Tapped>) -> Tapped {
    from.recv_timeout(Duration::from_secs(5))
        .expect("the tap is handed the next message")
}

#[test]
fn a_tapped_sessions_events_and_those_under_it_reach_the_tap_in_order_and_not_the_driver() {
    let (connection, events, mut engine) = connect();
    let (sink, tapped) = tap_sink();
    let _tap = connection.tap("INSPECT", sink);
    engine.send(json!({"method": "Runtime.consoleAPICalled", "params": {"type": "log"}, "sessionId": "INSPECT"}));
    engine.send(json!({"method": "Target.attachedToTarget", "params": {"sessionId": "CHILD", "targetInfo": {"type": "iframe"}}, "sessionId": "INSPECT"}));
    engine.send(json!({"method": "DOM.documentUpdated", "params": {}, "sessionId": "CHILD"}));
    engine.send(json!({"method": "Page.loadEventFired", "params": {}, "sessionId": "DRIVER"}));
    let methods: Vec<(String, Option<String>)> = (0..3)
        .map(|_| match next(&tapped) {
            Tapped::Event(event) => (event.method, event.session_id),
            other => panic!("an event, not {other:?}"),
        })
        .collect();
    assert_eq!(
        methods,
        vec![
            (
                "Runtime.consoleAPICalled".to_owned(),
                Some("INSPECT".to_owned())
            ),
            (
                "Target.attachedToTarget".to_owned(),
                Some("INSPECT".to_owned())
            ),
            ("DOM.documentUpdated".to_owned(), Some("CHILD".to_owned())),
        ]
    );
    let driver = events.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(
        driver.method, "Page.loadEventFired",
        "the driver hears only its own session"
    );
    assert!(events.try_recv().is_err());
}

#[test]
fn an_answer_to_a_call_sent_on_the_tap_comes_back_under_the_taps_own_id_among_its_events() {
    let (connection, events, mut engine) = connect();
    let (sink, tapped) = tap_sink();
    let tap = connection.tap("INSPECT", sink);
    connection
        .send_tapped(&tap, 7, "DOM.getDocument", json!({"depth": 1}), "INSPECT")
        .unwrap();
    let written = engine.read();
    assert_eq!(written["method"], "DOM.getDocument");
    assert_eq!(written["sessionId"], "INSPECT");
    assert_eq!(written["params"], json!({"depth": 1}));
    engine.send(json!({"id": written["id"], "result": {"root": {"nodeId": 1}}}));
    engine.send(
        json!({"method": "DOM.setChildNodes", "params": {"parentId": 1}, "sessionId": "INSPECT"}),
    );
    match next(&tapped) {
        Tapped::Answer { id, outcome } => {
            assert_eq!(id, 7);
            assert_eq!(outcome, Ok(json!({"root": {"nodeId": 1}})));
        }
        other => panic!("the answer first, not {other:?}"),
    }
    assert!(matches!(next(&tapped), Tapped::Event(e) if e.method == "DOM.setChildNodes"));
    // An error is handed over as the engine wrote it.
    connection
        .send_tapped(&tap, 8, "DOM.nope", json!({}), "INSPECT")
        .unwrap();
    let written = engine.read();
    engine.send(json!({"id": written["id"], "error": {"code": -32601, "message": "'DOM.nope' wasn't found"}}));
    match next(&tapped) {
        Tapped::Answer { id, outcome } => {
            assert_eq!(id, 8);
            assert_eq!(
                outcome,
                Err(json!({"code": -32601, "message": "'DOM.nope' wasn't found"}))
            );
        }
        other => panic!("the error, not {other:?}"),
    }
    assert!(events.try_recv().is_err(), "the driver hears none of it");
}

#[test]
fn a_session_detached_under_the_tap_is_heard_by_the_tap_and_then_no_longer_tapped() {
    let (connection, events, mut engine) = connect();
    let (sink, tapped) = tap_sink();
    let _tap = connection.tap("INSPECT", sink);
    // The engine says at the browser's own level that the tapped session itself went.
    engine.send(json!({"method": "Target.detachedFromTarget", "params": {"sessionId": "INSPECT"}}));
    assert!(matches!(next(&tapped), Tapped::Event(e) if e.method == "Target.detachedFromTarget"));
    engine.send(json!({"method": "Page.loadEventFired", "params": {}, "sessionId": "INSPECT"}));
    let driver = events.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(driver.session_id.as_deref(), Some("INSPECT"));
}

#[test]
fn an_untapped_session_goes_back_to_the_driver_and_a_tap_that_takes_nothing_more_is_let_go() {
    let (connection, events, mut engine) = connect();
    let (sink, tapped) = tap_sink();
    let tap = connection.tap("INSPECT", sink);
    connection.untap(&tap);
    engine.send(json!({"method": "Page.loadEventFired", "params": {}, "sessionId": "INSPECT"}));
    assert_eq!(
        events.recv_timeout(Duration::from_secs(5)).unwrap().method,
        "Page.loadEventFired"
    );
    assert!(tapped.try_recv().is_err());

    let refusing: Arc<dyn Fn(Tapped) -> bool + Send + Sync> = Arc::new(|_| false);
    let _full = connection.tap("FULL", refusing);
    engine.send(json!({"method": "DOM.documentUpdated", "params": {}, "sessionId": "FULL"}));
    engine.send(json!({"method": "DOM.documentUpdated", "params": {}, "sessionId": "FULL"}));
    // The tap that took nothing is gone: the next of its session is the driver's, and the
    // connection still reads.
    let driver = events.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(driver.session_id.as_deref(), Some("FULL"));
    assert!(connection.closed().is_none());
}

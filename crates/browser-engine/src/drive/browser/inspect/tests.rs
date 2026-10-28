//! What of the inspected page's events DevTools are told, event by event ([`passed`]).

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::{Arc, Mutex, mpsc};

use serde_json::{Value, json};

use super::{Inspection, MOST_HELD, passed};
use crate::cdp::Event;

/// An Inspection with nothing open: enough to sort the page's events.
fn bare() -> Inspection {
    let (work, _todo) = mpsc::sync_channel(1);
    Inspection {
        context: "CONTEXT".into(),
        front_end: "FRONT".into(),
        front_session: "S-FRONT".into(),
        inspected: "PAGE".into(),
        root: Mutex::new(Some("S-A".into())),
        tap: Mutex::new(None),
        front_tap: Mutex::new(None),
        work,
        behind: Arc::new(AtomicBool::new(false)),
        sink: Mutex::new(None),
        shows: AtomicU64::new(0),
        ended: AtomicBool::new(false),
        showing: AtomicBool::new(false),
        tell: Mutex::new(None),
        main_context: Mutex::new(None),
        ours: Mutex::new(HashSet::new()),
        ours_unique: Mutex::new(HashSet::new()),
        our_scripts: Mutex::new(HashSet::new()),
        known: Mutex::new(HashSet::new()),
        held: Mutex::new(HashMap::new()),
        heard: Mutex::new(Vec::new()),
        told: Mutex::new(Vec::new()),
    }
}

fn event(method: &str, params: Value, session: &str) -> Event {
    Event {
        method: method.into(),
        params,
        session_id: Some(session.into()),
    }
}

/// The methods `open` passes on for `method`.
fn told(open: &Inspection, method: &str, params: Value, session: &str) -> Vec<Event> {
    passed(open, event(method, params, session))
}

fn world(id: i64, name: &str, kind: &str) -> Value {
    json!({"context":{"id":id,"origin":"http://fixture.test","name":name,"uniqueId":format!("{name}-{id}"),"auxData":{"isDefault":kind == "default","type":kind,"frameId":"PAGE-1"}}})
}

fn script(id: &str, context: i64) -> Value {
    json!({"scriptId":id,"url":"","executionContextId":context,"hash":"x"})
}

#[test]
fn a_page_world_in_another_session_with_the_id_of_purlis_own_is_the_pages() {
    let open = bare();
    told(
        &open,
        "Runtime.executionContextCreated",
        world(9, "purlis", "isolated"),
        "S-A",
    );
    let created = told(
        &open,
        "Runtime.executionContextCreated",
        world(9, "", "default"),
        "S-B",
    );
    assert_eq!(created.len(), 1);
    let said = told(
        &open,
        "Runtime.consoleAPICalled",
        json!({"type":"log","args":[],"executionContextId":9,"timestamp":1.0}),
        "S-B",
    );
    assert_eq!(
        said.len(),
        1,
        "a message of the page's world in S-B is told"
    );
    let ours = told(
        &open,
        "Runtime.consoleAPICalled",
        json!({"type":"log","args":[],"executionContextId":9,"timestamp":2.0}),
        "S-A",
    );
    assert!(ours.is_empty(), "a message of purlis's world in S-A is not");
    let gone = told(
        &open,
        "Runtime.executionContextDestroyed",
        json!({"executionContextId":9,"executionContextUniqueId":"-9"}),
        "S-B",
    );
    assert_eq!(gone.len(), 1, "the page's world going is told");
}

#[test]
fn a_breakpoint_resolved_in_a_waiting_script_is_told_after_it() {
    let open = bare();
    assert!(told(&open, "Debugger.scriptParsed", script("72", 3), "S-A").is_empty());
    assert!(
        told(
            &open,
            "Debugger.breakpointResolved",
            json!({"breakpointId":"b","location":{"scriptId":"72","lineNumber":0}}),
            "S-A",
        )
        .is_empty()
    );
    let released = told(
        &open,
        "Runtime.executionContextCreated",
        world(3, "", "default"),
        "S-A",
    );
    let methods: Vec<&str> = released.iter().map(|e| e.method.as_str()).collect();
    assert_eq!(
        methods,
        [
            "Runtime.executionContextCreated",
            "Debugger.scriptParsed",
            "Debugger.breakpointResolved"
        ]
    );
    assert_eq!(released[2].params["location"]["scriptId"], "72");
}

#[test]
fn a_breakpoint_resolved_in_a_waiting_script_of_purlis_is_never_told() {
    let open = bare();
    told(&open, "Debugger.scriptParsed", script("71", 9), "S-A");
    told(
        &open,
        "Debugger.breakpointResolved",
        json!({"breakpointId":"b","location":{"scriptId":"71","lineNumber":0}}),
        "S-A",
    );
    let released = told(
        &open,
        "Runtime.executionContextCreated",
        world(9, "purlis", "isolated"),
        "S-A",
    );
    assert!(released.is_empty(), "{released:?}");
}

#[test]
fn past_the_bound_a_waiting_script_is_dropped_and_never_shown() {
    let open = bare();
    for n in 0..MOST_HELD {
        assert!(
            told(
                &open,
                "Debugger.scriptParsed",
                script(&format!("s{n}"), 3),
                "S-A"
            )
            .is_empty()
        );
    }
    assert!(told(&open, "Debugger.scriptParsed", script("past", 3), "S-A").is_empty());
    let released = told(
        &open,
        "Runtime.executionContextCreated",
        world(3, "", "default"),
        "S-A",
    );
    assert_eq!(released.len(), 1 + MOST_HELD);
    assert!(
        released.iter().all(|e| e.params["scriptId"] != "past"),
        "the script past the bound is never shown"
    );
    assert!(
        super::lock(&open.our_scripts).contains("past"),
        "and stays out of DevTools' reach"
    );
}

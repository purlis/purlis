// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::sync::mpsc;
use std::time::Duration;

use super::*;

/// Long enough that every telling a test makes lands inside the first interval.
const A_WHILE: Duration = Duration::from_millis(200);

fn plane(root: &str) -> PlaneId {
    serde_json::from_value(serde_json::json!(root)).expect("a plane id")
}

fn touched(root: &str, session: u32, path: &str) -> ChatEvent {
    ChatEvent::Touching(ChatTouching {
        plane: plane(root),
        session,
        path: path.into(),
    })
}

fn ran(root: &str, session: u32) -> ChatEvent {
    ChatEvent::SkillRun(ChatSkillRun {
        plane: plane(root),
        session,
        run: None,
    })
}

/// What a batch says, in the order it says it: each event's kind, chat and path.
fn said(batch: &ChatEvents) -> Vec<(String, u32, String)> {
    batch
        .events
        .iter()
        .map(|event| {
            let json = serde_json::to_value(event).expect("an event serialises");
            let told = &json["told"];
            (
                json["kind"].as_str().unwrap_or_default().to_owned(),
                u32::try_from(told["session"].as_u64().unwrap_or_default()).unwrap_or(0),
                told["path"].as_str().unwrap_or_default().to_owned(),
            )
        })
        .collect()
}

fn batches() -> (Batches, mpsc::Receiver<ChatEvents>) {
    let (sent, heard) = mpsc::channel();
    let batches = Batches::start(A_WHILE, move |batch| {
        let _ = sent.send(batch);
    });
    (batches, heard)
}

#[test]
fn what_many_chats_tell_within_an_interval_is_one_event_in_the_order_they_told_it() {
    let (batches, heard) = batches();

    for session in 1..=20 {
        batches.tell(touched("/p", session, "a.rs"));
        batches.tell(ran("/p", session));
        batches.tell(touched("/p", session, "b.rs"));
    }

    let batch = heard.recv_timeout(A_WHILE * 10).expect("a batch is sent");
    let mut expected = Vec::new();
    for session in 1..=20 {
        expected.push(("touching".to_owned(), session, "a.rs".to_owned()));
        expected.push(("skill-run".to_owned(), session, String::new()));
        expected.push(("touching".to_owned(), session, "b.rs".to_owned()));
    }
    assert_eq!(said(&batch), expected);
    assert!(
        heard.recv_timeout(A_WHILE * 2).is_err(),
        "sixty tellings were sent as more than one event"
    );
}

#[test]
fn each_project_is_sent_its_own_batch() {
    let (batches, heard) = batches();

    batches.tell(touched("/one", 1, "a.rs"));
    batches.tell(touched("/two", 1, "b.rs"));
    batches.tell(touched("/one", 2, "c.rs"));

    let first = heard.recv_timeout(A_WHILE * 10).expect("a batch for /one");
    let second = heard.recv_timeout(A_WHILE * 10).expect("a batch for /two");
    assert_eq!(first.plane, plane("/one"));
    assert_eq!(
        said(&first),
        [
            ("touching".to_owned(), 1, "a.rs".to_owned()),
            ("touching".to_owned(), 2, "c.rs".to_owned()),
        ]
    );
    assert_eq!(second.plane, plane("/two"));
    assert_eq!(
        said(&second),
        [("touching".to_owned(), 1, "b.rs".to_owned())]
    );
}

#[test]
fn nothing_is_sent_before_the_interval_has_passed_nor_when_nothing_was_told() {
    let (batches, heard) = batches();
    assert!(
        heard.recv_timeout(A_WHILE * 2).is_err(),
        "an idle queue sent a batch"
    );

    batches.tell(touched("/p", 1, "a.rs"));
    assert!(
        heard.recv_timeout(A_WHILE / 2).is_err(),
        "a telling was sent before its interval passed"
    );
    assert!(
        heard.recv_timeout(A_WHILE * 10).is_ok(),
        "it is sent after it"
    );
}

#[test]
fn a_telling_after_a_batch_was_sent_goes_in_the_next_one() {
    let (batches, heard) = batches();

    batches.tell(touched("/p", 1, "a.rs"));
    let first = heard.recv_timeout(A_WHILE * 10).expect("the first batch");
    batches.tell(touched("/p", 1, "b.rs"));
    let second = heard.recv_timeout(A_WHILE * 10).expect("the second batch");

    assert_eq!(
        said(&first),
        [("touching".to_owned(), 1, "a.rs".to_owned())]
    );
    assert_eq!(
        said(&second),
        [("touching".to_owned(), 1, "b.rs".to_owned())]
    );
}

#[test]
fn a_batch_carries_the_plane_and_each_event_as_its_kind_and_what_it_told() {
    let batch = ChatEvents {
        plane: plane("/p"),
        events: vec![touched("/p", 3, "src/a.rs")],
    };
    assert_eq!(
        serde_json::to_value(&batch).expect("it serialises"),
        serde_json::json!({
            "plane": "/p",
            "events": [{
                "kind": "touching",
                "told": { "plane": "/p", "session": 3, "path": "src/a.rs" },
            }],
        })
    );
}

#[test]
#[cfg(not(feature = "e2e"))]
fn a_build_anyone_is_given_batches_at_its_interval_whatever_its_environment_says() {
    assert_eq!(interval(|_| Some("0".to_owned())), INTERVAL);
}

#[test]
#[cfg(feature = "e2e")]
fn the_scenario_build_batches_at_the_interval_its_run_names_and_at_its_own_otherwise() {
    let named = |value: &'static str| {
        move |name: &str| (name == INTERVAL_VARIABLE).then(|| value.to_owned())
    };
    assert_eq!(interval(named("0")), Duration::ZERO);
    assert_eq!(interval(named("250")), Duration::from_millis(250));
    assert_eq!(interval(named("soon")), INTERVAL);
    assert_eq!(interval(|_| None), INTERVAL);
}

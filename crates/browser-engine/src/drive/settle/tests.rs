// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::time::{Duration, Instant};

use serde_json::Value;

use super::{Settle, activity};

/// What the engine sent while the fixture site's late-data page loaded: the page, a slow fetch
/// for its data, and a stream that never ends.
const LATE_DATA: &str = include_str!("../../../tests/fixtures/devtools/events-late-data.jsonl");

const QUIET: Duration = Duration::from_millis(500);

fn events() -> Vec<(String, Value, String)> {
    LATE_DATA
        .lines()
        .map(|line| {
            let event: Value = serde_json::from_str(line).unwrap();
            (
                event["method"].as_str().unwrap().to_owned(),
                event["params"].clone(),
                event["sessionId"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

/// Feeds the first `count` events, one millisecond apart from `start`.
fn fed(count: usize, start: Instant) -> (Settle, Instant) {
    let mut settle = Settle::new(start);
    let mut now = start;
    for (method, params, session) in events().into_iter().take(count) {
        now += Duration::from_millis(1);
        if let Some(seen) = activity(&method, &params) {
            settle.observe(&session, seen, now);
        }
    }
    (settle, now)
}

#[test]
fn a_page_whose_data_is_still_loading_has_not_settled_and_says_what_it_waits_for() {
    let start = Instant::now();
    // The page itself has loaded; its fetch for data has not.
    let (settle, now) = fed(6, start);
    let waiting = settle.waiting_for(now + Duration::from_secs(2), QUIET);
    assert_eq!(
        waiting,
        vec!["1 request still loading: http://fixture.test/slow-data".to_owned()]
    );
}

#[test]
fn a_page_settles_once_nothing_loads_and_it_has_been_quiet_a_while() {
    let start = Instant::now();
    let (settle, now) = fed(7, start);
    assert_eq!(
        settle.waiting_for(now + Duration::from_millis(100), QUIET),
        vec!["the page changed 100 ms ago".to_owned()]
    );
    assert!(settle.waiting_for(now + QUIET, QUIET).is_empty());
}

#[test]
fn a_frame_still_loading_keeps_a_page_from_settling() {
    let start = Instant::now();
    let (settle, now) = fed(1, start);
    assert_eq!(
        settle.waiting_for(now + Duration::from_secs(2), QUIET),
        vec!["1 frame still loading".to_owned()]
    );
}

#[test]
fn a_stream_that_never_ends_does_not_keep_a_page_from_settling() {
    // Events 5 (the stream's start) and nothing after: the stream stays open.
    let start = Instant::now();
    let (settle, now) = fed(7, start);
    assert!(
        settle
            .waiting_for(now + Duration::from_secs(1), QUIET)
            .is_empty()
    );
}

#[test]
fn a_request_that_failed_or_a_frame_that_went_away_no_longer_counts() {
    let start = Instant::now();
    let mut settle = Settle::new(start);
    let failed: Value = serde_json::json!({"requestId":"9.1","timestamp":1.0,"type":"Fetch","errorText":"net::ERR_ABORTED","canceled":true});
    let sent: Value = serde_json::json!({"requestId":"9.1","request":{"url":"http://fixture.test/x"},"type":"Fetch"});
    settle.observe(
        "S",
        activity("Network.requestWillBeSent", &sent).unwrap(),
        start,
    );
    settle.observe(
        "S",
        activity(
            "Page.frameStartedLoading",
            &serde_json::json!({"frameId":"F"}),
        )
        .unwrap(),
        start,
    );
    settle.observe(
        "S",
        activity("Network.loadingFailed", &failed).unwrap(),
        start,
    );
    settle.observe(
        "S",
        activity(
            "Page.frameDetached",
            &serde_json::json!({"frameId":"F","reason":"remove"}),
        )
        .unwrap(),
        start,
    );
    assert!(settle.waiting_for(start + QUIET, QUIET).is_empty());
}

#[test]
fn the_same_request_id_in_two_frames_sessions_is_two_requests() {
    let start = Instant::now();
    let mut settle = Settle::new(start);
    let sent: Value = serde_json::json!({"requestId":"1.1","request":{"url":"http://fixture.test/a"},"type":"Fetch"});
    let done: Value = serde_json::json!({"requestId":"1.1"});
    settle.observe(
        "PAGE",
        activity("Network.requestWillBeSent", &sent).unwrap(),
        start,
    );
    settle.observe(
        "FRAME",
        activity("Network.requestWillBeSent", &sent).unwrap(),
        start,
    );
    settle.observe(
        "PAGE",
        activity("Network.loadingFinished", &done).unwrap(),
        start,
    );
    assert_eq!(
        settle.waiting_for(start + QUIET, QUIET),
        vec!["1 request still loading: http://fixture.test/a".to_owned()]
    );
}

/// A frame from another site's document: asked for in the page's session, and answered in the
/// frame's own, which the engine gives the frame as it moves it to a process of its own (#2689).
fn a_cross_site_frame_s_document(kind: &str) -> (Value, Value) {
    (
        serde_json::json!({"requestId":"5F3C1E0D9A","loaderId":"5F3C1E0D9A","frameId":"FRAME-1","request":{"url":"http://other.test/frame.html"},"type":kind}),
        serde_json::json!({"requestId":"5F3C1E0D9A"}),
    )
}

#[test]
fn a_frame_s_document_answered_in_the_frame_s_own_session_no_longer_counts() {
    let start = Instant::now();
    let mut settle = Settle::new(start);
    let (sent, done) = a_cross_site_frame_s_document("Document");
    settle.observe(
        "PAGE",
        activity("Network.requestWillBeSent", &sent).unwrap(),
        start,
    );
    settle.observe(
        "FRAME",
        activity("Network.loadingFinished", &done).unwrap(),
        start,
    );
    assert!(settle.waiting_for(start + QUIET, QUIET).is_empty());
}

#[test]
fn a_request_other_than_a_document_ended_in_another_session_still_counts() {
    let start = Instant::now();
    let mut settle = Settle::new(start);
    let (sent, done) = a_cross_site_frame_s_document("Fetch");
    settle.observe(
        "PAGE",
        activity("Network.requestWillBeSent", &sent).unwrap(),
        start,
    );
    settle.observe(
        "FRAME",
        activity("Network.loadingFinished", &done).unwrap(),
        start,
    );
    assert_eq!(
        settle.waiting_for(start + QUIET, QUIET),
        vec!["1 request still loading: http://other.test/frame.html".to_owned()]
    );
}

/// A request a frame of the page made, in the page's own session (#2841).
fn a_request_of(frame: &str, id: &str, url: &str) -> Value {
    serde_json::json!({"requestId":id,"frameId":frame,"request":{"url":url},"type":"Fetch"})
}

#[test]
fn a_frame_that_detaches_takes_its_open_requests_and_those_of_frames_inside_it_with_it() {
    let start = Instant::now();
    let mut settle = Settle::new(start);
    for (frame, id, url) in [
        ("CHILD", "1.1", "http://fixture.test/child-data"),
        ("GRANDCHILD", "1.2", "http://fixture.test/grandchild-data"),
        ("PAGE", "1.3", "http://fixture.test/page-data"),
    ] {
        settle.observe(
            "PAGE",
            activity("Network.requestWillBeSent", &a_request_of(frame, id, url)).unwrap(),
            start,
        );
    }
    let gone_at = start + Duration::from_secs(1);
    settle.frames_gone(
        "PAGE",
        &["CHILD".to_owned(), "GRANDCHILD".to_owned()],
        gone_at,
    );
    // What the page itself has open still counts.
    assert_eq!(
        settle.waiting_for(gone_at + QUIET, QUIET),
        vec!["1 request still loading: http://fixture.test/page-data".to_owned()]
    );
}

#[test]
fn a_page_whose_frame_detached_is_quiet_a_quiet_while_from_the_detach() {
    let start = Instant::now();
    let mut settle = Settle::new(start);
    settle.observe(
        "PAGE",
        activity(
            "Network.requestWillBeSent",
            &a_request_of("CHILD", "1.1", "http://fixture.test/child-data"),
        )
        .unwrap(),
        start,
    );
    let gone_at = start + Duration::from_secs(5);
    settle.frames_gone("PAGE", &["CHILD".to_owned()], gone_at);
    assert_eq!(
        settle.waiting_for(gone_at + Duration::from_millis(100), QUIET),
        vec!["the page changed 100 ms ago".to_owned()]
    );
    assert!(settle.waiting_for(gone_at + QUIET, QUIET).is_empty());
}

#[test]
fn a_frame_that_detaches_leaves_another_session_s_requests_counted() {
    let start = Instant::now();
    let mut settle = Settle::new(start);
    settle.observe(
        "FRAME-SESSION",
        activity(
            "Network.requestWillBeSent",
            &a_request_of("CHILD", "1.1", "http://other.test/data"),
        )
        .unwrap(),
        start,
    );
    settle.frames_gone("PAGE", &["CHILD".to_owned()], start);
    assert_eq!(
        settle.waiting_for(start + QUIET, QUIET),
        vec!["1 request still loading: http://other.test/data".to_owned()]
    );
}

#[test]
fn a_frame_that_stopped_loading_has_no_document_left_to_wait_for() {
    // A frame from another site finishes its document in its own session, which purlis may not
    // be reading yet: what the page's session tells of it is all there is (#4472).
    let start = Instant::now();
    let mut settle = Settle::new(start);
    let seen = |method: &str, params: Value| activity(method, &params).unwrap();
    settle.observe(
        "S-PAGE",
        seen(
            "Network.requestWillBeSent",
            serde_json::json!({
                "requestId": "R-1",
                "type": "Document",
                "frameId": "FRAME",
                "request": { "url": "http://other.test/frame.html" },
            }),
        ),
        start,
    );
    settle.observe(
        "S-PAGE",
        seen(
            "Page.frameStoppedLoading",
            serde_json::json!({ "frameId": "FRAME" }),
        ),
        start,
    );
    assert_eq!(
        settle.waiting_for(start + QUIET * 2, QUIET),
        Vec::<String>::new()
    );
}

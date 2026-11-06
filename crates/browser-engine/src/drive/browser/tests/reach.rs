// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Where a browser may go (#2018): every request a page is about to send is held until it is
//! judged, told as what it is (a read, an address the agent wrote, or something sent), and let
//! on or failed as the judge says; a site the judge asks about is in the act's answer.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::json;

use super::super::{Frame, Origin, Unjudged};
use super::{
    MOVED, browser_judged, events_taken_in, frame_address, frame_attaches, frame_moves, frame_read,
    id, ref_of, snapshot, wait_for,
};
use crate::WebAddress;
use crate::drive::{Call, Driver, Field, Gate, Judge, Outbound, Why};

/// A judge that keeps every request it is told, and asks about each address `asks` holds.
fn judge(asks: &'static [&'static str]) -> (Judge, Arc<Mutex<Vec<Outbound>>>) {
    let told = Arc::new(Mutex::new(Vec::new()));
    let keep = Arc::clone(&told);
    let judge: Judge = Arc::new(move |out: &Outbound| {
        keep.lock().unwrap().push(out.clone());
        match asks.iter().find(|ask| out.address.starts_with(**ask)) {
            Some(_) => Gate::Ask {
                site: "new.test:443".to_owned(),
            },
            None => Gate::Go,
        }
    });
    (judge, told)
}

/// The engine is about to send `method` to `url` for frame `frame`, as a `kind` of resource.
fn paused(engine: &super::Engine, request: &str, url: &str, method: &str, kind: &str, frame: &str) {
    engine.emit(
        "Fetch.requestPaused",
        json!({
            "requestId": request,
            "request": { "url": url, "method": method, "headers": {} },
            "frameId": frame,
            "resourceType": kind,
        }),
        None,
    );
}

/// Waits until the driver answered request `request`, and says how: `continue` or `fail`.
fn answered(engine: &super::Engine, request: &str) -> String {
    let start = Instant::now();
    loop {
        for (method, said) in [
            ("Fetch.continueRequest", "continue"),
            ("Fetch.failRequest", "fail"),
            ("Fetch.fulfillRequest", "no content"),
        ] {
            if engine
                .sent(method)
                .iter()
                .any(|s| s.params["requestId"] == request)
            {
                return said.to_owned();
            }
        }
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "request {request} was never answered"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn whys(told: &Mutex<Vec<Outbound>>) -> Vec<(String, Why, Driver)> {
    told.lock()
        .unwrap()
        .iter()
        .map(|out| (out.address.clone(), out.why, out.by))
        .collect()
}

#[test]
fn a_judged_browser_holds_every_request_of_every_page_until_it_is_judged() {
    let (judge, _) = judge(&[]);
    let (_browser, engine) = browser_judged(judge);
    let enabled = engine.sent("Fetch.enable");
    assert_eq!(enabled.len(), 1, "requests are held once, browser-wide");
    assert_eq!(enabled[0].session, None);
    assert_eq!(
        enabled[0].params["patterns"],
        json!([
            { "urlPattern": "http://*", "requestStage": "Request" },
            { "urlPattern": "https://*", "requestStage": "Request" },
        ])
    );
}

#[test]
fn what_a_page_reads_is_told_as_a_read_and_let_on() {
    let (judge, told) = judge(&[]);
    let (_browser, engine) = browser_judged(judge);
    paused(
        &engine,
        "R1",
        "https://new.test/logo.png",
        "GET",
        "Image",
        "PAGE-1",
    );
    // A link the page offered, followed: a document of the page's own, nobody wrote it.
    paused(
        &engine,
        "R2",
        "https://new.test/next",
        "GET",
        "Document",
        "PAGE-1",
    );
    assert_eq!(answered(&engine, "R1"), "continue");
    assert_eq!(answered(&engine, "R2"), "continue");
    assert_eq!(
        whys(&told),
        vec![
            (
                "https://new.test/logo.png".into(),
                Why::Reads,
                Driver::Agent
            ),
            ("https://new.test/next".into(), Why::Reads, Driver::Agent),
        ]
    );
}

#[test]
fn a_request_with_a_body_is_told_as_sending() {
    let (judge, told) = judge(&["https://new.test/"]);
    let (_browser, engine) = browser_judged(judge);
    paused(
        &engine,
        "R1",
        "https://new.test/save",
        "POST",
        "XHR",
        "PAGE-1",
    );
    assert_eq!(answered(&engine, "R1"), "fail");
    let failed = &engine.sent("Fetch.failRequest")[0];
    assert_eq!(failed.params["errorReason"], "BlockedByClient");
    assert_eq!(whys(&told)[0].1, Why::Sends);
}

#[test]
fn a_form_the_page_submits_is_told_as_sending_though_it_is_a_get() {
    let (judge, told) = judge(&[]);
    let (_browser, engine) = browser_judged(judge);
    engine.emit(
        "Page.frameRequestedNavigation",
        json!({
            "frameId": "PAGE-1",
            "reason": "formSubmissionGet",
            "url": "https://new.test/search?q=what+was+typed#top",
            "disposition": "currentTab",
        }),
        Some("S-PAGE-1"),
    );
    paused(
        &engine,
        "R1",
        "https://new.test/search?q=what+was+typed",
        "GET",
        "Document",
        "PAGE-1",
    );
    assert_eq!(answered(&engine, "R1"), "continue");
    assert_eq!(whys(&told)[0].1, Why::Sends);
}

#[test]
fn an_address_the_agent_writes_and_where_it_redirects_are_told_as_addressed() {
    let (judge, told) = judge(&["https://new.test/"]);
    let (browser, engine) = browser_judged(judge);
    engine.on(move |sent, engine| {
        if sent.method == "Page.navigate" {
            // The address the agent wrote, then the site it redirects to.
            paused(
                engine,
                "R1",
                "https://known.test/go",
                "GET",
                "Document",
                "PAGE-1",
            );
            paused(
                engine,
                "R2",
                "https://new.test/landing",
                "GET",
                "Document",
                "PAGE-1",
            );
        }
        None
    });
    let answer = browser
        .call(
            &id(1),
            Call::Navigate {
                page: None,
                address: WebAddress::parse("https://known.test/go").unwrap(),
            },
        )
        .unwrap();
    assert_eq!(answered(&engine, "R1"), "continue");
    assert_eq!(
        answered(&engine, "R2"),
        "no content",
        "a document held back leaves its frame where it was"
    );
    assert_eq!(
        engine.sent("Fetch.fulfillRequest")[0].params["responseCode"],
        204
    );
    assert_eq!(
        whys(&told)
            .into_iter()
            .map(|(_, why, _)| why)
            .collect::<Vec<_>>(),
        vec![Why::Addressed, Why::Addressed]
    );
    assert_eq!(answer.asked, vec!["new.test:443".to_owned()]);
    // Once the act has answered, the page's own documents are reads again.
    paused(
        &engine,
        "R3",
        "https://known.test/next",
        "GET",
        "Document",
        "PAGE-1",
    );
    assert_eq!(answered(&engine, "R3"), "continue");
    assert_eq!(whys(&told)[2].1, Why::Reads);
}

#[test]
fn typing_on_a_page_of_a_site_the_judge_asks_about_types_nothing_and_says_so() {
    // The stand-in's page is at http://fixture.test/frames.html.
    let (judge, told) = judge(&["http://fixture.test/"]);
    let (browser, engine) = browser_judged(judge);
    let text = snapshot(&browser, 1);
    let name = ref_of(&text, "- textbox \"Name\"");
    let answer = browser
        .call(
            &id(2),
            Call::Type {
                page: None,
                fields: vec![Field {
                    into: Some(name),
                    text: "Grace".into(),
                    replace: true,
                }],
            },
        )
        .unwrap();
    assert!(engine.sent("Input.insertText").is_empty());
    assert!(engine.sent("Input.dispatchKeyEvent").is_empty());
    assert_eq!(answer.asked, vec!["new.test:443".to_owned()]);
    assert!(answer.note.unwrap().contains("nothing was typed"));
    let typing = whys(&told);
    assert_eq!(
        typing.last().unwrap(),
        &(
            "http://fixture.test/frames.html".to_owned(),
            Why::Sends,
            Driver::Agent
        )
    );
}

#[test]
fn keys_pressed_on_a_page_of_a_site_the_judge_asks_about_are_held_back() {
    let (judge, _) = judge(&["http://fixture.test/"]);
    let (browser, engine) = browser_judged(judge);
    let answer = browser
        .call(
            &id(1),
            Call::Press {
                page: None,
                keys: vec!["Enter".into()],
            },
        )
        .unwrap();
    assert!(engine.sent("Input.dispatchKeyEvent").is_empty());
    assert_eq!(answer.asked, vec!["new.test:443".to_owned()]);
}

#[test]
fn while_the_person_drives_every_request_is_told_as_theirs() {
    let (judge, told) = judge(&[]);
    let (browser, engine) = browser_judged(judge);
    browser.take_over();
    paused(
        &engine,
        "R1",
        "https://new.test/save",
        "POST",
        "Document",
        "PAGE-1",
    );
    assert_eq!(answered(&engine, "R1"), "continue");
    assert_eq!(whys(&told)[0].2, Driver::Person);
    browser.hand_back();
    paused(
        &engine,
        "R2",
        "https://new.test/save",
        "POST",
        "Document",
        "PAGE-1",
    );
    assert_eq!(answered(&engine, "R2"), "continue");
    assert_eq!(whys(&told)[1].2, Driver::Agent);
}

#[test]
fn a_site_asked_about_between_acts_is_told_in_the_next_answer_once() {
    let (judge, _) = judge(&["https://new.test/"]);
    let (browser, engine) = browser_judged(judge);
    paused(
        &engine,
        "R1",
        "https://new.test/beacon",
        "POST",
        "Ping",
        "PAGE-1",
    );
    paused(
        &engine,
        "R2",
        "https://new.test/beacon",
        "POST",
        "Ping",
        "PAGE-1",
    );
    assert_eq!(answered(&engine, "R2"), "fail");
    let first = browser.call(&id(1), Call::Pages).unwrap();
    assert_eq!(first.asked, vec!["new.test:443".to_owned()]);
    let next = browser.call(&id(2), Call::Pages).unwrap();
    assert!(next.asked.is_empty());
}

#[test]
fn typing_into_a_frame_of_a_site_the_judge_asks_about_types_nothing() {
    // The stand-in's page embeds a frame from http://other.test/pay.html.
    let (judge, told) = judge(&["http://other.test/"]);
    let (browser, engine) = browser_judged(judge);
    let text = snapshot(&browser, 1);
    let name = ref_of(&text, "- textbox \"Name\"");
    let pay = ref_of(&text, "- button \"Pay now\"");
    let typed = |n: u32, into: crate::drive::Ref| {
        browser
            .call(
                &id(n),
                Call::Type {
                    page: None,
                    fields: vec![Field {
                        into: Some(into),
                        text: "4111".into(),
                        replace: false,
                    }],
                },
            )
            .unwrap()
    };
    let on_the_page = typed(2, name);
    assert!(
        on_the_page.asked.is_empty(),
        "the page's own field is judged by its own site"
    );
    let sent = engine.sent("Input.insertText").len();
    let in_the_frame = typed(3, pay);
    assert_eq!(in_the_frame.asked, vec!["new.test:443".to_owned()]);
    assert_eq!(
        engine.sent("Input.insertText").len(),
        sent,
        "nothing typed in the frame"
    );
    assert_eq!(
        whys(&told).last().unwrap().0,
        "http://other.test/pay.html",
        "judged by the frame's own document"
    );
}

/// Types into the payment frame's button, as a fresh snapshot of the page deals it.
fn typed_into_the_frame(
    browser: &crate::drive::Browser,
) -> Result<crate::drive::Answer, crate::drive::Refusal> {
    typed_into_the_frame_as(browser, 50)
}

/// [`typed_into_the_frame`] by calls `n` and `n + 1`: a call whose id was answered before
/// answers the same again, so typing once more takes ids of its own.
fn typed_into_the_frame_as(
    browser: &crate::drive::Browser,
    n: u32,
) -> Result<crate::drive::Answer, crate::drive::Refusal> {
    let text = snapshot(browser, n);
    let pay = ref_of(&text, "- button \"Pay now\"");
    browser.call(
        &id(n + 1),
        Call::Type {
            page: None,
            fields: vec![Field {
                into: Some(pay),
                text: "4111".into(),
                replace: false,
            }],
        },
    )
}

#[test]
fn a_frame_goes_by_the_address_it_attached_at_while_its_tree_is_not_answered() {
    let (judge, told) = judge(&["http://third.test/"]);
    let (browser, engine) = browser_judged(judge);
    let third = "http://third.test/pay.html";
    frame_moves(
        &engine,
        third,
        "http://fourth.test/pay.html",
        Duration::from_secs(3),
    );
    let answer = typed_into_the_frame(&browser).unwrap();
    assert_eq!(answer.asked, vec!["new.test:443".to_owned()]);
    assert!(engine.sent("Input.insertText").is_empty());
    assert_eq!(whys(&told).last().unwrap().0, third);
}

#[test]
fn a_frame_that_attached_at_no_address_takes_nothing_until_its_tree_is_read() {
    let (judge, _) = judge(&[]);
    let (browser, engine) = browser_judged(judge);
    frame_moves(
        &engine,
        "",
        "http://third.test/pay.html",
        Duration::from_secs(3),
    );
    let refused = typed_into_the_frame(&browser).unwrap_err();
    assert!(
        refused.to_string().contains("nothing was sent"),
        "{refused}"
    );
    assert!(engine.sent("Input.insertText").is_empty());
    assert!(engine.sent("Input.dispatchKeyEvent").is_empty());
}

#[test]
fn a_frame_that_attached_at_no_address_goes_by_the_address_its_tree_reads() {
    let (judge, told) = judge(&["http://third.test/"]);
    let (browser, engine) = browser_judged(judge);
    let third = "http://third.test/pay.html";
    frame_moves(&engine, "", third, Duration::from_millis(1));
    frame_read(&browser);
    assert_eq!(frame_address(&browser, "PAYMENT-FRAME"), third);
    let answer = typed_into_the_frame(&browser).unwrap();
    assert_eq!(answer.asked, vec!["new.test:443".to_owned()]);
    assert!(engine.sent("Input.insertText").is_empty());
    assert_eq!(whys(&told).last().unwrap().0, third);
}

#[test]
fn a_blank_frame_in_a_session_of_its_own_is_not_known_until_its_document_says_its_origin() {
    let (judge, told) = judge(&["http://third.test/"]);
    let (browser, engine) = browser_judged(judge);
    frame_moves(&engine, "", "about:blank", Duration::from_millis(1));
    frame_read(&browser);
    assert_eq!(frame_address(&browser, "PAYMENT-FRAME"), "about:blank");
    assert_eq!(
        typed_into_the_frame(&browser).err(),
        Some(crate::drive::Refusal::FrameUnread)
    );
    assert!(engine.sent("Input.insertText").is_empty());
    engine.emit(
        "Runtime.executionContextCreated",
        json!({"context": {
            "id": 72,
            "origin": "http://third.test",
            "name": "",
            "uniqueId": "U-72",
            "auxData": {"isDefault": true, "type": "default", "frameId": "PAYMENT-FRAME"},
        }}),
        Some(MOVED),
    );
    wait_for("the document's origin is seen", || {
        super::super::lock(&browser.inner.state)
            .document_origins
            .origin(MOVED, "PAYMENT-FRAME")
            .is_some()
    });
    let answer = typed_into_the_frame_as(&browser, 60).unwrap();
    assert_eq!(answer.asked, vec!["new.test:443".to_owned()]);
    assert!(engine.sent("Input.insertText").is_empty());
    assert_eq!(whys(&told).last().unwrap().0, "http://third.test/");
}

#[test]
fn a_frame_whose_tree_fails_stays_not_known_and_stands_on_no_ancestor() {
    let (judge, _) = judge(&[]);
    let (browser, engine) = browser_judged(judge);
    engine.on(|sent, _| {
        (sent.method == "Page.getFrameTree" && sent.session.as_deref() == Some(MOVED))
            .then_some(Err(crate::cdp::CallError::TimedOut))
    });
    frame_attaches(&engine, "");
    // Set-ups run in turn: the next one's read means the failed one is over.
    engine.emit(
        "Target.attachedToTarget",
        json!({"sessionId":"S-LATER","targetInfo":{"targetId":"LATER-FRAME","type":"iframe","title":"","url":"http://other.test/","attached":true},"waitingForDebugger":false}),
        Some("S-PAGE-1"),
    );
    wait_for("the next frame's tree is asked for", || {
        engine
            .sent("Page.getFrameTree")
            .iter()
            .any(|s| s.session.as_deref() == Some("S-LATER"))
    });
    let state = super::super::lock(&browser.inner.state);
    assert_eq!(
        state.documents_of("PAGE-1", "PAYMENT-FRAME"),
        Err(Unjudged::Unread)
    );
    assert_eq!(state.every_document("PAGE-1"), Err(Unjudged::Unread));
}

#[test]
fn a_blank_frame_inside_a_frame_not_known_is_not_known_either() {
    let (judge, _) = judge(&[]);
    let (browser, engine) = browser_judged(judge);
    frame_moves(
        &engine,
        "",
        "http://third.test/pay.html",
        Duration::from_secs(3),
    );
    engine.emit(
        "Page.frameNavigated",
        json!({"frame": {"id": "INSIDE", "parentId": "PAYMENT-FRAME", "url": "about:blank"}}),
        Some("S-PAYMENT-FRAME"),
    );
    wait_for("the frame inside is seen", || {
        super::super::lock(&browser.inner.state)
            .page("PAGE-1")
            .is_some_and(|page| page.frames.contains_key("INSIDE"))
    });
    let state = super::super::lock(&browser.inner.state);
    assert_eq!(
        state.documents_of("PAGE-1", "INSIDE"),
        Err(Unjudged::Unread)
    );
}

#[test]
fn a_browser_with_no_judge_has_nothing_to_hold_back_for_a_frame_not_known() {
    let (browser, engine) = super::browser();
    frame_moves(
        &engine,
        "",
        "http://third.test/pay.html",
        Duration::from_secs(3),
    );
    assert_ne!(
        typed_into_the_frame(&browser).err(),
        Some(crate::drive::Refusal::FrameUnread)
    );
}

#[test]
fn a_frames_address_follows_the_engines_word_of_its_target() {
    let (judge, _) = judge(&[]);
    let (browser, engine) = browser_judged(judge);
    let third = "http://third.test/pay.html";
    frame_moves(&engine, third, third, Duration::from_millis(1));
    engine.emit(
        "Target.targetInfoChanged",
        json!({"targetInfo":{"targetId":"PAYMENT-FRAME","type":"iframe","title":"","url":"http://fourth.test/","attached":true}}),
        None,
    );
    wait_for("the frame's address is told", || {
        frame_address(&browser, "PAYMENT-FRAME") == "http://fourth.test/"
    });
}

#[test]
fn a_tree_read_before_a_navigation_does_not_put_the_frame_back() {
    let (judge, _) = judge(&[]);
    let (browser, engine) = browser_judged(judge);
    let before = "http://third.test/before.html";
    frame_moves(&engine, before, before, Duration::from_millis(300));
    engine.emit(
        "Page.frameNavigated",
        json!({"frame": {"id": "PAYMENT-FRAME", "parentId": "PAGE-1", "name": "payment", "url": "http://third.test/after.html"}}),
        Some("S-PAYMENT-FRAME"),
    );
    wait_for("the navigation is seen", || {
        frame_address(&browser, "PAYMENT-FRAME") == "http://third.test/after.html"
    });
    std::thread::sleep(Duration::from_millis(600));
    assert_eq!(
        frame_address(&browser, "PAYMENT-FRAME"),
        "http://third.test/after.html"
    );
}

#[test]
fn a_frames_older_session_does_not_move_it_once_a_newer_one_attached() {
    let (judge, _) = judge(&[]);
    let (browser, engine) = browser_judged(judge);
    let third = "http://third.test/pay.html";
    frame_moves(&engine, third, third, Duration::from_millis(1));
    engine.emit(
        "Page.frameNavigated",
        json!({"frame": {"id": "PAYMENT-FRAME", "parentId": "PAGE-1", "name": "payment", "url": "http://other.test/older.html"}}),
        Some("S-PAYMENT"),
    );
    events_taken_in(&browser, &engine);
    assert_eq!(frame_address(&browser, "PAYMENT-FRAME"), third);
}

#[test]
fn a_frames_older_session_does_not_remove_it_once_a_newer_one_attached() {
    let (judge, _) = judge(&[]);
    let (browser, engine) = browser_judged(judge);
    let third = "http://third.test/pay.html";
    frame_moves(&engine, third, third, Duration::from_millis(1));
    frame_read(&browser);
    engine.emit(
        "Page.frameDetached",
        json!({"frameId":"PAYMENT-FRAME","reason":"remove"}),
        Some("S-PAYMENT"),
    );
    events_taken_in(&browser, &engine);
    assert_eq!(frame_address(&browser, "PAYMENT-FRAME"), third);
}

#[test]
fn a_frame_placed_again_while_a_session_holds_it_is_not_known_until_read() {
    let (judge, _) = judge(&[]);
    let (browser, engine) = browser_judged(judge);
    let frame_there = || {
        super::super::lock(&browser.inner.state)
            .page("PAGE-1")
            .is_some_and(|page| page.frames.contains_key("PAYMENT-FRAME"))
    };
    engine.emit(
        "Page.frameDetached",
        json!({"frameId":"PAYMENT-FRAME","reason":"remove"}),
        Some("S-PAGE-1"),
    );
    wait_for("the frame is gone", || !frame_there());
    engine.emit(
        "Page.frameAttached",
        json!({"frameId":"PAYMENT-FRAME","parentFrameId":"PAGE-1"}),
        Some("S-PAGE-1"),
    );
    wait_for("the frame is back", frame_there);
    let state = super::super::lock(&browser.inner.state);
    assert_eq!(
        state.documents_of("PAGE-1", "PAYMENT-FRAME"),
        Err(Unjudged::Unread)
    );
}

#[test]
fn a_frames_address_follows_its_parent_once_its_target_is_destroyed() {
    let (judge, _) = judge(&[]);
    let (browser, engine) = browser_judged(judge);
    engine.emit(
        "Target.targetDestroyed",
        json!({"targetId":"PAYMENT-FRAME"}),
        None,
    );
    let after = "http://fixture.test/after.html";
    engine.emit(
        "Page.frameNavigated",
        json!({"frame": {"id": "PAYMENT-FRAME", "parentId": "PAGE-1", "name": "payment", "url": after}}),
        Some("S-PAGE-1"),
    );
    wait_for("the parent's word is taken", || {
        frame_address(&browser, "PAYMENT-FRAME") == after
    });
}

#[test]
fn a_destroyed_targets_frame_session_leaves_nothing_of_its_own_loading() {
    let (browser, engine) = super::browser();
    let waits = || {
        super::super::lock(&browser.inner.state)
            .page("PAGE-1")
            .expect("the frames page")
            .settle
            .waiting_for(Instant::now(), Duration::ZERO)
            .join("; ")
    };
    engine.emit(
        "Network.requestWillBeSent",
        json!({"requestId":"9.1","request":{"url":"http://other.test/slow-data","method":"GET"},"type":"Fetch","frameId":"PAYMENT-FRAME"}),
        Some("S-PAYMENT"),
    );
    wait_for("the request is seen", || {
        waits().contains("request still loading")
    });
    engine.emit(
        "Target.targetDestroyed",
        json!({"targetId":"PAYMENT-FRAME"}),
        None,
    );
    wait_for("the request is forgotten", || {
        !waits().contains("request still loading")
    });
}

#[test]
fn a_tree_at_no_address_does_not_replace_the_address_a_frame_attached_at() {
    let (judge, _) = judge(&[]);
    let (browser, engine) = browser_judged(judge);
    let third = "http://third.test/pay.html";
    frame_moves(&engine, third, "about:blank", Duration::from_millis(1));
    frame_read(&browser);
    assert_eq!(frame_address(&browser, "PAYMENT-FRAME"), third);
}

#[test]
fn keys_go_by_the_document_with_the_focus_and_by_every_document_when_it_is_unknown() {
    let (judge, _) = judge(&["http://other.test/"]);
    let (browser, engine) = browser_judged(judge);
    let press = |n: u32| {
        browser
            .call(
                &id(n),
                Call::Press {
                    page: None,
                    keys: vec!["a".into()],
                },
            )
            .unwrap()
    };
    // The page's own document has the focus: its site is listed, and the key goes.
    assert!(press(1).asked.is_empty());
    let sent = engine.sent("Input.dispatchKeyEvent").len();
    assert!(sent > 0);
    // Nothing the page shows says it has the focus: every document is judged, the frame's too.
    {
        let mut model = super::super::lock(&engine.model);
        for nodes in model.ax.values_mut() {
            for node in nodes.as_array_mut().unwrap() {
                if let Some(properties) = node["properties"].as_array_mut() {
                    properties.retain(|one| one["name"] != "focused");
                }
            }
        }
    }
    assert_eq!(press(2).asked, vec!["new.test:443".to_owned()]);
    assert_eq!(
        engine.sent("Input.dispatchKeyEvent").len(),
        sent,
        "no key reached the page"
    );
}

#[test]
fn a_document_the_page_asked_for_with_a_query_and_no_word_of_how_sends() {
    let (judge, told) = judge(&[]);
    let (_browser, engine) = browser_judged(judge);
    paused(
        &engine,
        "R1",
        "https://new.test/search?q=typed",
        "GET",
        "Document",
        "PAGE-1",
    );
    assert_eq!(answered(&engine, "R1"), "continue");
    assert_eq!(whys(&told)[0].1, Why::Sends);
}

#[test]
fn a_link_the_page_followed_reads_and_so_do_its_redirects() {
    let (judge, told) = judge(&[]);
    let (_browser, engine) = browser_judged(judge);
    engine.emit(
        "Page.frameRequestedNavigation",
        json!({
            "frameId": "PAGE-1",
            "reason": "anchorClick",
            "url": "https://new.test/go?to=next",
            "disposition": "currentTab",
        }),
        Some("S-PAGE-1"),
    );
    paused(
        &engine,
        "R1",
        "https://new.test/go?to=next",
        "GET",
        "Document",
        "PAGE-1",
    );
    assert_eq!(answered(&engine, "R1"), "continue");
    engine.emit(
        "Fetch.requestPaused",
        json!({
            "requestId": "R2",
            "request": { "url": "https://elsewhere.test/landing?from=go", "method": "GET", "headers": {} },
            "frameId": "PAGE-1",
            "resourceType": "Document",
            "redirectedRequestId": "R1",
        }),
        None,
    );
    assert_eq!(answered(&engine, "R2"), "continue");
    assert_eq!(
        whys(&told)
            .into_iter()
            .map(|(_, why, _)| why)
            .collect::<Vec<_>>(),
        vec![Why::Reads, Why::Reads]
    );
}

#[test]
fn a_frame_at_no_web_address_is_judged_by_its_nearest_web_ancestor_and_an_unknown_one_by_every_document()
 {
    let (judge, _) = judge(&[]);
    let (browser, engine) = browser_judged(judge);
    // A blank frame inside the cross-site frame, and a srcdoc frame of the page's own.
    engine.emit(
        "Page.frameNavigated",
        json!({"frame": {"id": "BLANK", "parentId": "PAYMENT-FRAME", "url": "about:blank"}}),
        Some("S-PAYMENT"),
    );
    engine.emit(
        "Page.frameNavigated",
        json!({"frame": {"id": "SRCDOC", "parentId": "PAGE-1", "url": "about:srcdoc"}}),
        Some("S-PAGE-1"),
    );
    let start = Instant::now();
    while super::super::lock(&browser.inner.state)
        .page("PAGE-1")
        .is_none_or(|page| {
            !page.frames.contains_key("SRCDOC") || !page.frames.contains_key("BLANK")
        })
    {
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "the frames were never seen"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    browser.read_sandboxes("PAGE-1");
    let state = super::super::lock(&browser.inner.state);
    assert_eq!(
        state.documents_of("PAGE-1", "BLANK").unwrap(),
        ["http://other.test/pay.html"],
        "the cross-site frame holds it"
    );
    assert_eq!(
        state.documents_of("PAGE-1", "SRCDOC").unwrap(),
        ["http://fixture.test/frames.html"]
    );
    let every = state.documents_of("PAGE-1", "NO-SUCH-FRAME").unwrap();
    assert_eq!(every[0], "http://fixture.test/frames.html");
    assert!(
        every.contains(&"http://other.test/pay.html".to_owned()),
        "{every:?}"
    );
}

#[test]
fn a_form_the_page_submits_sends_though_its_address_carries_no_query() {
    // The form's own word alone: with no query, nothing else would make it a send.
    let (judge, told) = judge(&[]);
    let (_browser, engine) = browser_judged(judge);
    engine.emit(
        "Page.frameRequestedNavigation",
        json!({
            "frameId": "PAGE-1",
            "reason": "formSubmissionGet",
            "url": "https://new.test/subscribe",
            "disposition": "currentTab",
        }),
        Some("S-PAGE-1"),
    );
    paused(
        &engine,
        "R1",
        "https://new.test/subscribe",
        "GET",
        "Document",
        "PAGE-1",
    );
    paused(
        &engine,
        "R2",
        "https://new.test/subscribe",
        "GET",
        "Document",
        "PAGE-1",
    );
    assert_eq!(answered(&engine, "R2"), "continue");
    assert_eq!(
        whys(&told)
            .into_iter()
            .map(|(_, why, _)| why)
            .collect::<Vec<_>>(),
        vec![Why::Sends, Why::Reads],
        "the form's word is taken once; the same address unmarked reads"
    );
}

/// The engine says the payment frame now shows `url`, with `origin` as its `securityOrigin`
/// where it gives one.
fn the_frame_shows(
    engine: &super::Engine,
    browser: &crate::drive::Browser,
    url: &str,
    origin: Option<&str>,
) {
    let mut frame =
        json!({"id": "PAYMENT-FRAME", "parentId": "PAGE-1", "name": "payment", "url": url});
    if let Some(origin) = origin {
        frame["securityOrigin"] = json!(origin);
    }
    engine.emit(
        "Page.frameNavigated",
        json!({"frame": frame}),
        Some("S-PAYMENT"),
    );
    wait_for("the frame's new document is seen", || {
        frame_address(browser, "PAYMENT-FRAME") == url
    });
}

/// Chrome makes the payment frame's document's own context, in which the document's origin is
/// `origin`: Chrome's `://` where it has none.
fn the_document_has(engine: &super::Engine, browser: &crate::drive::Browser, origin: &str) {
    engine.emit(
        "Runtime.executionContextCreated",
        json!({"context": {
            "id": 71,
            "origin": origin,
            "name": "",
            "uniqueId": "U-71",
            "auxData": {"isDefault": true, "type": "default", "frameId": "PAYMENT-FRAME"},
        }}),
        Some("S-PAYMENT"),
    );
    wait_for("the document's origin is seen", || {
        super::super::lock(&browser.inner.state)
            .document_origins
            .origin("S-PAYMENT", "PAYMENT-FRAME")
            == Some(origin)
    });
}

/// The payment frame shows a srcdoc document, told as Chrome tells it: the frame's
/// `securityOrigin` is read off its address alone, so it is `://` whatever the document's
/// origin; the document's own context has `origin`.
fn chrome_shows_a_srcdoc_frame(
    engine: &super::Engine,
    browser: &crate::drive::Browser,
    origin: &str,
) {
    engine.emit(
        "Page.frameNavigated",
        json!({
            "frame": {
                "id": "PAYMENT-FRAME",
                "parentId": "PAGE-1",
                "loaderId": "LOADER-7",
                "name": "",
                "url": "about:srcdoc",
                "domainAndRegistry": "",
                "securityOrigin": "://",
                "securityOriginDetails": {"isLocalhost": false},
                "mimeType": "text/html",
                "adFrameStatus": {"adFrameType": "none"},
                "secureContextType": "InsecureScheme",
                "crossOriginIsolatedContextType": "NotIsolated",
                "gatedAPIFeatures": [],
            },
            "type": "Navigation",
        }),
        Some("S-PAYMENT"),
    );
    wait_for("the frame's new document is seen", || {
        frame_address(browser, "PAYMENT-FRAME") == "about:srcdoc"
    });
    the_document_has(engine, browser, origin);
}

#[test]
fn a_frames_origin_follows_the_webs_rules_for_its_address() {
    let frame = |url: &str, origin: &str| Frame {
        session: String::new(),
        parent: None,
        name: String::new(),
        url: url.to_owned(),
        generation: 0,
        loaded: 0,
        told: 0,
        unread: false,
        security_origin: origin.to_owned(),
        sandboxed: None,
    };
    let web = |address: &str| Origin::Web(address.to_owned());
    for (url, origin, document, judged) in [
        (
            "https://shop.test/pay",
            "",
            None,
            web("https://shop.test/pay"),
        ),
        // A document sandboxed without its origin allowed has none, whatever its address
        // says: its own context says so.
        ("https://shop.test/pay", "null", Some("://"), Origin::Opaque),
        ("https://shop.test/pay", "", Some("null"), Origin::Opaque),
        (
            "https://shop.test/pay",
            "https://shop.test",
            Some("https://shop.test"),
            web("https://shop.test/pay"),
        ),
        (
            "blob:https://maker.test/4d2c",
            "",
            None,
            web("https://maker.test/4d2c"),
        ),
        (
            "BLOB:https://maker.test/4d2c",
            "",
            None,
            web("https://maker.test/4d2c"),
        ),
        (
            "blob:https://maker.test/4d2c",
            "https://maker.test",
            None,
            web("https://maker.test/"),
        ),
        ("blob:null/4d2c", "", None, Origin::Opaque),
        ("about:blank", "", None, Origin::Inherited),
        ("about:blank#top", "", None, Origin::Inherited),
        ("about:srcdoc", "", None, Origin::Inherited),
        ("", "", None, Origin::Inherited),
        // Chrome says `://` of every such address, whatever origin its document has.
        ("about:srcdoc", "://", None, Origin::Inherited),
        ("about:blank", "://", None, Origin::Inherited),
        (
            "about:srcdoc",
            "://",
            Some("http://fixture.test"),
            web("http://fixture.test/"),
        ),
        (
            "about:blank",
            "://",
            Some("https://maker.test:8443"),
            web("https://maker.test:8443/"),
        ),
        ("about:srcdoc", "://", Some("://"), Origin::Opaque),
        ("about:srcdoc", "", Some("null"), Origin::Opaque),
        ("about:blank", "://", Some(""), Origin::Opaque),
        ("data:text/html,<p>hi</p>", "", None, Origin::Opaque),
        (
            "data:text/html,<p>hi</p>",
            "://",
            Some("://"),
            Origin::Opaque,
        ),
        ("data:text/html,<p>hi</p>", "null", None, Origin::Opaque),
        ("javascript:void(0)", "", None, Origin::Opaque),
        ("about:other", "", None, Origin::Opaque),
        ("chrome-error://chromewebdata/", "", None, Origin::Engine),
        (
            "chrome-error://chromewebdata/",
            "null",
            None,
            Origin::Engine,
        ),
    ] {
        assert_eq!(
            frame(url, origin).origin(document),
            judged,
            "{url} at {origin:?}, its document at {document:?}"
        );
    }
    // Nor has one whose frame element's sandbox leaves it none, with no word of its context.
    let sandboxed = Frame {
        sandboxed: Some(true),
        ..frame("https://shop.test/pay", "")
    };
    assert_eq!(sandboxed.origin(None), Origin::Opaque);
    let allowed = Frame {
        sandboxed: Some(false),
        ..frame("https://shop.test/pay", "")
    };
    assert_eq!(allowed.origin(None), web("https://shop.test/pay"));
}

#[test]
fn a_frame_with_no_web_origin_is_opaque_and_nothing_is_typed_into_it() {
    let (judge, told) = judge(&[]);
    let (browser, engine) = browser_judged(judge);
    the_frame_shows(
        &engine,
        &browser,
        "data:text/html,<button>Pay now</button>",
        None,
    );
    assert_eq!(
        typed_into_the_frame(&browser).err(),
        Some(crate::drive::Refusal::FrameOpaque)
    );
    assert!(engine.sent("Input.insertText").is_empty());
    assert!(
        !whys(&told)
            .iter()
            .any(|(address, ..)| address.starts_with("data:")),
        "the judge is never told an address it cannot place"
    );
    let state = super::super::lock(&browser.inner.state);
    assert_eq!(
        state.every_document("PAGE-1"),
        Err(Unjudged::Opaque),
        "a key with no known focus may land in it"
    );
}

#[test]
fn a_sandboxed_srcdoc_frame_is_opaque() {
    let (judge, _) = judge(&[]);
    let (browser, engine) = browser_judged(judge);
    chrome_shows_a_srcdoc_frame(&engine, &browser, "://");
    assert_eq!(
        typed_into_the_frame(&browser).err(),
        Some(crate::drive::Refusal::FrameOpaque)
    );
    assert!(engine.sent("Input.insertText").is_empty());
}

#[test]
fn a_srcdoc_frame_is_judged_by_the_origin_its_document_has() {
    let (judge, told) = judge(&["http://fixture.test/"]);
    let (browser, engine) = browser_judged(judge);
    chrome_shows_a_srcdoc_frame(&engine, &browser, "http://fixture.test");
    let answer = typed_into_the_frame(&browser).unwrap();
    assert_eq!(answer.asked, vec!["new.test:443".to_owned()]);
    assert_eq!(whys(&told).last().unwrap().0, "http://fixture.test/");
}

#[test]
fn a_srcdoc_frame_in_a_session_of_its_own_whose_document_has_ended_is_not_known() {
    let (judge, _) = judge(&["http://fixture.test/"]);
    let (browser, engine) = browser_judged(judge);
    chrome_shows_a_srcdoc_frame(&engine, &browser, "http://fixture.test");
    engine.emit(
        "Runtime.executionContextDestroyed",
        json!({"executionContextId": 71, "executionContextUniqueId": "U-71"}),
        Some("S-PAYMENT"),
    );
    wait_for("the document's context is gone", || {
        super::super::lock(&browser.inner.state)
            .document_origins
            .origin("S-PAYMENT", "PAYMENT-FRAME")
            .is_none()
    });
    assert_eq!(
        typed_into_the_frame(&browser).err(),
        Some(crate::drive::Refusal::FrameUnread)
    );
    assert!(engine.sent("Input.insertText").is_empty());
}

#[test]
fn a_blank_frame_is_judged_by_the_origin_its_document_has() {
    let (judge, told) = judge(&["http://third.test/"]);
    let (browser, engine) = browser_judged(judge);
    the_frame_shows(&engine, &browser, "about:blank", Some("://"));
    the_document_has(&engine, &browser, "http://third.test");
    let answer = typed_into_the_frame(&browser).unwrap();
    assert_eq!(answer.asked, vec!["new.test:443".to_owned()]);
    assert_eq!(whys(&told).last().unwrap().0, "http://third.test/");
}

/// The payment frame, on its own session, starts on another document: `method` is the
/// engine's word of it, with `more` beside the frame's id. No word comes that its old
/// document's context ended.
fn the_frame_starts_another_document(
    engine: &super::Engine,
    browser: &crate::drive::Browser,
    method: &str,
    more: serde_json::Value,
) {
    let mut params = json!({"frameId": "PAYMENT-FRAME"});
    params
        .as_object_mut()
        .unwrap()
        .extend(more.as_object().unwrap().clone());
    engine.emit(method, params, Some("S-PAYMENT"));
    events_taken_in(browser, engine);
}

/// A blank payment frame whose document's origin is `http://third.test`, and a frame that
/// starts another document by `method`: its origin is not known until the new document's own
/// context says it, and then it is judged there.
fn a_blank_frame_reads_its_origin_again_when_it_starts(method: &str, more: serde_json::Value) {
    let (judge, told) = judge(&["http://third.test/", "http://fourth.test/"]);
    let (browser, engine) = browser_judged(judge);
    the_frame_shows(&engine, &browser, "about:blank", Some("://"));
    the_document_has(&engine, &browser, "http://third.test");
    the_frame_starts_another_document(&engine, &browser, method, more);
    assert_eq!(
        typed_into_the_frame(&browser).err(),
        Some(crate::drive::Refusal::FrameUnread)
    );
    assert!(engine.sent("Input.insertText").is_empty());
    the_document_has(&engine, &browser, "http://fourth.test");
    let answer = typed_into_the_frame_as(&browser, 60).unwrap();
    assert_eq!(answer.asked, vec!["new.test:443".to_owned()]);
    assert_eq!(whys(&told).last().unwrap().0, "http://fourth.test/");
}

#[test]
fn a_frames_origin_is_read_again_when_it_starts_loading_another_document() {
    a_blank_frame_reads_its_origin_again_when_it_starts("Page.frameStartedLoading", json!({}));
}

#[test]
fn a_frames_origin_is_read_again_when_it_starts_navigating_to_another_document() {
    a_blank_frame_reads_its_origin_again_when_it_starts(
        "Page.frameStartedNavigating",
        json!({"url": "about:blank", "loaderId": "LOADER-8", "navigationType": "differentDocument"}),
    );
}

#[test]
fn a_frame_that_moves_within_its_document_keeps_its_origin() {
    let (judge, told) = judge(&["http://third.test/"]);
    let (browser, engine) = browser_judged(judge);
    the_frame_shows(&engine, &browser, "about:blank", Some("://"));
    the_document_has(&engine, &browser, "http://third.test");
    for kind in ["sameDocument", "historySameDocument"] {
        the_frame_starts_another_document(
            &engine,
            &browser,
            "Page.frameStartedNavigating",
            json!({"url": "about:blank#top", "loaderId": "LOADER-7", "navigationType": kind}),
        );
    }
    let answer = typed_into_the_frame(&browser).unwrap();
    assert_eq!(answer.asked, vec!["new.test:443".to_owned()]);
    assert_eq!(whys(&told).last().unwrap().0, "http://third.test/");
}

#[test]
fn a_frames_origin_is_read_again_when_it_moves_to_another_process() {
    let (judge, _) = judge(&[]);
    let (browser, engine) = browser_judged(judge);
    // The frame's session had told of a document of this frame before, and no word came that
    // it ended.
    engine.emit(
        "Runtime.executionContextCreated",
        json!({"context": {
            "id": 73,
            "origin": "http://third.test",
            "name": "",
            "uniqueId": "U-73",
            "auxData": {"isDefault": true, "type": "default", "frameId": "PAYMENT-FRAME"},
        }}),
        Some(MOVED),
    );
    wait_for("the earlier document's origin is kept", || {
        super::super::lock(&browser.inner.state)
            .document_origins
            .origin(MOVED, "PAYMENT-FRAME")
            .is_some()
    });
    frame_moves(&engine, "", "about:blank", Duration::from_millis(1));
    frame_read(&browser);
    assert_eq!(
        super::super::lock(&browser.inner.state)
            .document_origins
            .origin(MOVED, "PAYMENT-FRAME"),
        None
    );
    assert_eq!(
        typed_into_the_frame(&browser).err(),
        Some(crate::drive::Refusal::FrameUnread)
    );
    assert!(engine.sent("Input.insertText").is_empty());
}

#[test]
fn a_blob_frame_is_judged_by_the_origin_that_made_it() {
    let (judge, told) = judge(&["http://third.test/"]);
    let (browser, engine) = browser_judged(judge);
    the_frame_shows(&engine, &browser, "blob:http://third.test/4d2c", None);
    let answer = typed_into_the_frame(&browser).unwrap();
    assert_eq!(answer.asked, vec!["new.test:443".to_owned()]);
    assert!(engine.sent("Input.insertText").is_empty());
    assert_eq!(whys(&told).last().unwrap().0, "http://third.test/4d2c");
}

#[test]
fn a_srcdoc_frame_in_a_session_of_its_own_with_no_word_of_its_origin_is_not_known() {
    let (judge, told) = judge(&["http://fixture.test/"]);
    let (browser, engine) = browser_judged(judge);
    the_frame_shows(&engine, &browser, "about:srcdoc", None);
    assert_eq!(
        typed_into_the_frame(&browser).err(),
        Some(crate::drive::Refusal::FrameUnread)
    );
    assert!(engine.sent("Input.insertText").is_empty());
    assert!(
        !whys(&told)
            .iter()
            .any(|(address, ..)| address == "http://fixture.test/frames.html"),
        "the page around it is not taken for it"
    );
}

/// The page shows a frame `id` in its own session at `url`, and the driver has seen it.
fn the_page_shows_a_frame(
    engine: &super::Engine,
    browser: &crate::drive::Browser,
    id: &str,
    parent: &str,
    url: &str,
) {
    engine.emit(
        "Page.frameNavigated",
        json!({"frame": {"id": id, "parentId": parent, "url": url}}),
        Some("S-PAGE-1"),
    );
    wait_for("the frame is seen", || {
        super::super::lock(&browser.inner.state)
            .page("PAGE-1")
            .is_some_and(|page| page.frames.get(id).is_some_and(|frame| frame.url == url))
    });
}

/// The page's session makes the own context of frame `frame`'s document, at `origin`.
fn the_pages_frame_has(
    engine: &super::Engine,
    browser: &crate::drive::Browser,
    frame: &str,
    id: i64,
    origin: &str,
) {
    engine.emit(
        "Runtime.executionContextCreated",
        json!({"context": {
            "id": id,
            "origin": origin,
            "name": "",
            "uniqueId": format!("U-{id}"),
            "auxData": {"isDefault": true, "type": "default", "frameId": frame},
        }}),
        Some("S-PAGE-1"),
    );
    wait_for("the document's origin is seen", || {
        super::super::lock(&browser.inner.state)
            .document_origins
            .origin("S-PAGE-1", frame)
            == Some(origin)
    });
}

/// Waits until every event the engine sent before this one is taken in: the engine's word of
/// another session's context, as an event the driver takes in order behind the rest.
fn everything_before_is_taken_in(engine: &super::Engine, browser: &crate::drive::Browser) {
    static NEXT: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(7000);
    let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let frame = format!("MARK-{id}");
    engine.emit(
        "Runtime.executionContextCreated",
        json!({"context": {
            "id": id,
            "origin": "http://mark.test",
            "name": "",
            "uniqueId": format!("U-{id}"),
            "auxData": {"isDefault": true, "type": "default", "frameId": frame},
        }}),
        Some("S-ELSEWHERE"),
    );
    wait_for("the events before are taken in", || {
        super::super::lock(&browser.inner.state)
            .document_origins
            .origin("S-ELSEWHERE", &frame)
            .is_some()
    });
}

/// The page's session tells of frame `frame`: `method`, with `more` beside the frame's id.
fn the_pages_frame_does(
    engine: &super::Engine,
    browser: &crate::drive::Browser,
    frame: &str,
    method: &str,
    more: serde_json::Value,
) {
    let mut params = json!({"frameId": frame});
    params
        .as_object_mut()
        .unwrap()
        .extend(more.as_object().unwrap().clone());
    engine.emit(method, params, Some("S-PAGE-1"));
    everything_before_is_taken_in(engine, browser);
}

/// The page's session ends context `id`.
fn the_pages_context_ends(engine: &super::Engine, browser: &crate::drive::Browser, id: i64) {
    engine.emit(
        "Runtime.executionContextDestroyed",
        json!({"executionContextId": id, "executionContextUniqueId": format!("U-{id}")}),
        Some("S-PAGE-1"),
    );
    everything_before_is_taken_in(engine, browser);
}

fn documents_of(browser: &crate::drive::Browser, frame: &str) -> Result<Vec<String>, Unjudged> {
    super::super::lock(&browser.inner.state).documents_of("PAGE-1", frame)
}

#[test]
fn a_srcdoc_frame_in_the_pages_session_whose_document_has_ended_is_not_judged_until_its_next_is_known()
 {
    let (judge, _) = judge(&[]);
    let (browser, engine) = browser_judged(judge);
    the_page_shows_a_frame(&engine, &browser, "SRCDOC", "PAGE-1", "about:srcdoc");
    the_pages_frame_has(&engine, &browser, "SRCDOC", 81, "://");
    assert_eq!(documents_of(&browser, "SRCDOC"), Err(Unjudged::Opaque));
    the_pages_context_ends(&engine, &browser, 81);
    browser.read_sandboxes("PAGE-1");
    assert_eq!(documents_of(&browser, "SRCDOC"), Err(Unjudged::Unread));
    the_pages_frame_has(&engine, &browser, "SRCDOC", 82, "http://fixture.test");
    assert_eq!(
        documents_of(&browser, "SRCDOC").unwrap(),
        ["http://fixture.test/"]
    );
}

/// A blank frame in the page's session whose document is of `http://third.test`, not the
/// page's, read for its sandbox.
fn a_blank_frame_of_another_origin_in_the_pages_session()
-> (crate::drive::Browser, Arc<super::Engine>) {
    let (judge, _) = judge(&[]);
    let (browser, engine) = browser_judged(judge);
    the_page_shows_a_frame(&engine, &browser, "BLANK", "PAGE-1", "about:blank");
    the_pages_frame_has(&engine, &browser, "BLANK", 84, "http://third.test");
    browser.read_sandboxes("PAGE-1");
    assert_eq!(
        documents_of(&browser, "BLANK").unwrap(),
        ["http://third.test/"]
    );
    (browser, engine)
}

/// A blank frame in the page's session that starts another document by `method` is not
/// judged, not even by the page around it, until the new document's own context says its
/// origin.
fn a_blank_frame_in_the_pages_session_is_not_judged_once_it_starts(
    method: &str,
    more: serde_json::Value,
) {
    let (browser, engine) = a_blank_frame_of_another_origin_in_the_pages_session();
    the_pages_frame_does(&engine, &browser, "BLANK", method, more);
    browser.read_sandboxes("PAGE-1");
    assert_eq!(documents_of(&browser, "BLANK"), Err(Unjudged::Unread));
    the_pages_context_ends(&engine, &browser, 84);
    browser.read_sandboxes("PAGE-1");
    assert_eq!(documents_of(&browser, "BLANK"), Err(Unjudged::Unread));
    the_pages_frame_has(&engine, &browser, "BLANK", 85, "http://fourth.test");
    assert_eq!(
        documents_of(&browser, "BLANK").unwrap(),
        ["http://fourth.test/"]
    );
}

#[test]
fn a_blank_frame_in_the_pages_session_that_starts_loading_is_not_judged_until_its_document_is_known()
 {
    a_blank_frame_in_the_pages_session_is_not_judged_once_it_starts(
        "Page.frameStartedLoading",
        json!({}),
    );
}

#[test]
fn a_blank_frame_in_the_pages_session_that_starts_navigating_is_not_judged_until_its_document_is_known()
 {
    a_blank_frame_in_the_pages_session_is_not_judged_once_it_starts(
        "Page.frameStartedNavigating",
        json!({"url": "about:blank", "loaderId": "LOADER-9", "navigationType": "differentDocument"}),
    );
}

#[test]
fn a_frame_not_judged_refuses_what_would_reach_it_and_says_to_try_again() {
    let (browser, engine) = a_blank_frame_of_another_origin_in_the_pages_session();
    the_pages_frame_does(
        &engine,
        &browser,
        "BLANK",
        "Page.frameStartedLoading",
        json!({}),
    );
    browser.read_sandboxes("PAGE-1");
    let refused = browser
        .known_documents("PAGE-1", documents_of(&browser, "BLANK"))
        .unwrap_err();
    assert_eq!(refused, crate::drive::Refusal::FrameUnread);
    assert!(refused.to_string().contains("try again"));
}

#[test]
fn a_load_that_stops_with_the_old_document_still_shown_judges_the_frame_by_that_document() {
    let (browser, engine) = a_blank_frame_of_another_origin_in_the_pages_session();
    the_pages_frame_does(
        &engine,
        &browser,
        "BLANK",
        "Page.frameStartedLoading",
        json!({}),
    );
    assert_eq!(documents_of(&browser, "BLANK"), Err(Unjudged::Unread));
    the_pages_frame_does(
        &engine,
        &browser,
        "BLANK",
        "Page.frameStoppedLoading",
        json!({}),
    );
    browser.read_sandboxes("PAGE-1");
    assert_eq!(
        documents_of(&browser, "BLANK").unwrap(),
        ["http://third.test/"]
    );
}

#[test]
fn a_load_that_stops_after_the_old_document_ended_leaves_a_blank_frame_not_judged() {
    let (browser, engine) = a_blank_frame_of_another_origin_in_the_pages_session();
    the_pages_frame_does(
        &engine,
        &browser,
        "BLANK",
        "Page.frameStartedLoading",
        json!({}),
    );
    the_pages_context_ends(&engine, &browser, 84);
    the_pages_frame_does(
        &engine,
        &browser,
        "BLANK",
        "Page.frameStoppedLoading",
        json!({}),
    );
    browser.read_sandboxes("PAGE-1");
    assert_eq!(documents_of(&browser, "BLANK"), Err(Unjudged::Unread));
}

#[test]
fn a_srcdoc_document_that_stops_loading_with_no_context_is_judged_by_the_page_around_it() {
    let (judge, _) = judge(&[]);
    let (browser, engine) = browser_judged(judge);
    the_page_shows_a_frame(&engine, &browser, "SRCDOC", "PAGE-1", "about:blank");
    the_pages_frame_has(&engine, &browser, "SRCDOC", 86, "http://third.test");
    the_pages_frame_does(
        &engine,
        &browser,
        "SRCDOC",
        "Page.frameStartedLoading",
        json!({}),
    );
    the_pages_context_ends(&engine, &browser, 86);
    the_page_shows_a_frame(&engine, &browser, "SRCDOC", "PAGE-1", "about:srcdoc");
    the_pages_frame_does(
        &engine,
        &browser,
        "SRCDOC",
        "Page.frameStoppedLoading",
        json!({}),
    );
    browser.read_sandboxes("PAGE-1");
    assert_eq!(
        documents_of(&browser, "SRCDOC").unwrap(),
        ["http://fixture.test/frames.html"]
    );
}

#[test]
fn a_frame_whose_contexts_are_cleared_is_not_judged_until_its_next_document_is_known() {
    let (browser, engine) = a_blank_frame_of_another_origin_in_the_pages_session();
    engine.emit(
        "Runtime.executionContextsCleared",
        json!({}),
        Some("S-PAGE-1"),
    );
    everything_before_is_taken_in(&engine, &browser);
    browser.read_sandboxes("PAGE-1");
    assert_eq!(documents_of(&browser, "BLANK"), Err(Unjudged::Unread));
}

#[test]
fn a_frame_whose_own_target_ends_is_still_awaited() {
    let (judge, _) = judge(&[]);
    let (browser, engine) = browser_judged(judge);
    the_page_shows_a_frame(&engine, &browser, "BLANK", "PAGE-1", "about:blank");
    super::super::lock(&browser.inner.state)
        .document_origins
        .started("PAGE-1", "BLANK");

    engine.emit("Target.targetDestroyed", json!({"targetId": "BLANK"}), None);
    everything_before_is_taken_in(&engine, &browser);

    assert!(
        super::super::lock(&browser.inner.state)
            .document_origins
            .awaits("PAGE-1", "BLANK")
    );
}

#[test]
fn a_frame_no_page_holds_is_not_left_awaited_when_its_contexts_are_cleared() {
    let mut origins = super::super::DocumentOrigins::default();
    origins.told(
        "S-PAGE-1",
        &json!({"id": 1, "origin": "http://else.test", "auxData": {"isDefault": true, "frameId": "F-1"}}),
        None,
    );
    origins.cleared("S-PAGE-1", |_| None);
    assert!(!origins.awaits("PAGE-1", "F-1"));
}

#[test]
fn a_frame_that_is_gone_is_no_longer_awaited() {
    let mut origins = super::super::DocumentOrigins::default();
    origins.started("PAGE-1", "F-1");
    assert!(origins.awaits("PAGE-1", "F-1"));
    origins.gone("F-1");
    assert!(!origins.awaits("PAGE-1", "F-1"));
}

/// Document origins with more frames of page `PAGE-1` awaited than are kept.
fn past_the_most_frames_awaited_on_the_first_page() -> super::super::DocumentOrigins {
    let mut origins = super::super::DocumentOrigins::default();
    for id in 0..=super::super::FRAME_ORIGINS_KEPT {
        origins.started("PAGE-1", &format!("F-{id}"));
    }
    origins
}

#[test]
fn past_the_most_frames_awaited_every_frame_of_that_page_is_awaited() {
    let origins = past_the_most_frames_awaited_on_the_first_page();
    assert!(origins.awaits("PAGE-1", "NEVER-STARTED"));
    assert!(
        origins.replaced("PAGE-1", "NEVER-STARTED"),
        "no frame of that page is judged"
    );
    assert!(!origins.awaits("PAGE-2", "NEVER-STARTED"), "another page's");
}

#[test]
fn a_page_past_the_most_frames_awaited_that_goes_leaves_nothing_awaited_behind() {
    let mut origins = past_the_most_frames_awaited_on_the_first_page();
    origins.page_gone("PAGE-1");
    assert!(!origins.awaits("PAGE-1", "NEVER-STARTED"));
}

#[test]
fn a_page_past_the_most_frames_awaited_is_judged_again_once_its_top_document_is_replaced() {
    let mut origins = past_the_most_frames_awaited_on_the_first_page();
    origins.top_navigated("PAGE-1", &[]);
    assert!(!origins.awaits("PAGE-1", "NEVER-STARTED"));
    assert!(!origins.replaced("PAGE-1", "NEVER-STARTED"));
    // While as many frames as are kept are still awaited, the next one past them is not
    // kept, and the page is not judged again.
    origins.started("PAGE-1", "ONE-MORE");
    assert!(origins.awaits("PAGE-1", "NEVER-STARTED"));
}

#[test]
fn the_engines_word_that_a_page_replaced_its_top_document_judges_it_again() {
    let (judge, _) = judge(&[]);
    let (browser, engine) = browser_judged(judge);
    {
        let mut state = super::super::lock(&browser.inner.state);
        for id in 0..=super::super::FRAME_ORIGINS_KEPT {
            state.document_origins.started("PAGE-1", &format!("F-{id}"));
        }
        assert!(state.document_origins.awaits("PAGE-1", "NEVER-STARTED"));
    }
    engine.emit(
        "Page.frameNavigated",
        json!({"frame": {"id": "PAGE-1", "url": "http://fixture.test/long.html"}}),
        Some("S-PAGE-1"),
    );
    wait_for("the page's frames are judged again", || {
        !super::super::lock(&browser.inner.state)
            .document_origins
            .awaits("PAGE-1", "NEVER-STARTED")
    });
}

#[test]
fn a_frame_of_a_top_document_the_page_replaced_does_not_keep_the_page_unjudged() {
    let (browser, engine) = a_web_frame_that_starts_another_document();
    the_pages_context_ends(&engine, &browser, 92);
    assert_eq!(documents_of(&browser, "WEB"), Err(Unjudged::Unread));
    engine.emit(
        "Page.frameNavigated",
        json!({"frame": {"id": "PAGE-1", "url": "http://fixture.test/long.html"}}),
        Some("S-PAGE-1"),
    );
    everything_before_is_taken_in(&engine, &browser);
    assert!(
        !super::super::lock(&browser.inner.state)
            .document_origins
            .replaced("PAGE-1", "WEB")
    );
}

#[test]
fn the_engines_word_that_a_page_is_gone_leaves_nothing_of_it_awaited() {
    let (judge, _) = judge(&[]);
    let (browser, engine) = browser_judged(judge);
    the_page_shows_a_frame(&engine, &browser, "BLANK", "PAGE-1", "about:blank");
    the_pages_frame_does(
        &engine,
        &browser,
        "BLANK",
        "Page.frameStartedLoading",
        json!({}),
    );
    super::super::lock(&browser.inner.state)
        .document_origins
        .overflowed
        .insert("PAGE-1".to_owned());
    engine.emit(
        "Target.targetDestroyed",
        json!({"targetId": "PAGE-1"}),
        None,
    );
    wait_for("the page is gone", || {
        super::super::lock(&browser.inner.state)
            .page("PAGE-1")
            .is_none()
    });
    let state = super::super::lock(&browser.inner.state);
    assert!(!state.document_origins.awaits("PAGE-1", "BLANK"));
    assert!(state.document_origins.overflowed.is_empty());
}

/// Session `session` makes the own context of frame `frame`'s document, at `origin`.
fn a_frames_context_on(
    engine: &super::Engine,
    browser: &crate::drive::Browser,
    session: &str,
    frame: &str,
    id: i64,
    origin: &str,
) {
    engine.emit(
        "Runtime.executionContextCreated",
        json!({"context": {
            "id": id,
            "origin": origin,
            "name": "",
            "uniqueId": format!("U-{id}"),
            "auxData": {"isDefault": true, "type": "default", "frameId": frame},
        }}),
        Some(session),
    );
    wait_for("the context is taken in", || {
        super::super::lock(&browser.inner.state)
            .document_origins
            .origin(session, frame)
            == Some(origin)
    });
}

#[test]
fn a_context_another_session_tells_of_leaves_a_frame_that_started_another_document_not_judged() {
    let (browser, engine) = a_blank_frame_of_another_origin_in_the_pages_session();
    the_pages_frame_does(
        &engine,
        &browser,
        "BLANK",
        "Page.frameStartedLoading",
        json!({}),
    );
    a_frames_context_on(
        &engine,
        &browser,
        "S-ELSEWHERE",
        "BLANK",
        87,
        "http://fixture.test",
    );
    browser.read_sandboxes("PAGE-1");
    assert_eq!(documents_of(&browser, "BLANK"), Err(Unjudged::Unread));
    the_pages_frame_has(&engine, &browser, "BLANK", 88, "http://fourth.test");
    assert_eq!(
        documents_of(&browser, "BLANK").unwrap(),
        ["http://fourth.test/"]
    );
}

const WEB_FRAME: &str = "http://other.test/inner.html";

/// A frame in the page's session at [`WEB_FRAME`], whose document's context says so, that
/// starts another document.
fn a_web_frame_that_starts_another_document() -> (crate::drive::Browser, Arc<super::Engine>) {
    let (judge, _) = judge(&[]);
    let (browser, engine) = browser_judged(judge);
    the_page_shows_a_frame(&engine, &browser, "WEB", "PAGE-1", WEB_FRAME);
    the_pages_frame_has(&engine, &browser, "WEB", 92, "http://other.test");
    assert_eq!(documents_of(&browser, "WEB").unwrap(), [WEB_FRAME]);
    the_pages_frame_does(
        &engine,
        &browser,
        "WEB",
        "Page.frameStartedLoading",
        json!({}),
    );
    assert_eq!(
        documents_of(&browser, "WEB").unwrap(),
        [WEB_FRAME],
        "until it is replaced, the document it shows is the old one"
    );
    (browser, engine)
}

#[test]
fn a_frame_whose_new_document_of_another_origin_is_told_before_its_address_is_not_judged_until_the_address_follows()
 {
    let (browser, engine) = a_web_frame_that_starts_another_document();
    the_pages_frame_has(&engine, &browser, "WEB", 93, "http://fixture.test");
    assert_eq!(documents_of(&browser, "WEB"), Err(Unjudged::Unread));
    the_page_shows_a_frame(&engine, &browser, "WEB", "PAGE-1", "about:blank");
    browser.read_sandboxes("PAGE-1");
    assert_eq!(
        documents_of(&browser, "WEB").unwrap(),
        ["http://fixture.test/"]
    );
}

#[test]
fn a_frame_whose_document_ended_is_not_judged_by_its_old_address() {
    let (browser, engine) = a_web_frame_that_starts_another_document();
    the_pages_context_ends(&engine, &browser, 92);
    assert_eq!(documents_of(&browser, "WEB"), Err(Unjudged::Unread));
    the_page_shows_a_frame(
        &engine,
        &browser,
        "WEB",
        "PAGE-1",
        "http://third.test/add.html",
    );
    assert_eq!(
        documents_of(&browser, "WEB").unwrap(),
        ["http://third.test/add.html"]
    );
}

#[test]
fn a_frame_whose_old_document_ends_after_its_new_address_is_judged_once_the_new_context_is_told() {
    let (browser, engine) = a_web_frame_that_starts_another_document();
    the_page_shows_a_frame(
        &engine,
        &browser,
        "WEB",
        "PAGE-1",
        "http://third.test/add.html",
    );
    the_pages_context_ends(&engine, &browser, 92);
    the_pages_frame_has(&engine, &browser, "WEB", 94, "http://third.test");
    assert_eq!(
        documents_of(&browser, "WEB").unwrap(),
        ["http://third.test/add.html"]
    );
}

#[test]
fn a_new_document_of_the_same_origin_told_before_its_address_is_judged_by_that_origin() {
    let (browser, engine) = a_web_frame_that_starts_another_document();
    the_pages_frame_has(&engine, &browser, "WEB", 95, "http://other.test");
    assert_eq!(documents_of(&browser, "WEB").unwrap(), [WEB_FRAME]);
}

#[test]
fn a_frame_at_a_web_address_whose_context_follows_with_no_web_origin_is_judged_by_its_address() {
    let (browser, engine) = a_web_frame_that_starts_another_document();
    the_page_shows_a_frame(
        &engine,
        &browser,
        "WEB",
        "PAGE-1",
        "http://third.test/add.html",
    );
    the_pages_frame_has(&engine, &browser, "WEB", 96, "://");
    assert_eq!(
        documents_of(&browser, "WEB").unwrap(),
        ["http://third.test/add.html"]
    );
}

#[test]
fn a_context_another_session_tells_of_before_a_frames_address_leaves_it_not_judged() {
    let (browser, engine) = a_web_frame_that_starts_another_document();
    a_frames_context_on(
        &engine,
        &browser,
        "S-ELSEWHERE",
        "WEB",
        97,
        "http://fixture.test",
    );
    assert_eq!(documents_of(&browser, "WEB"), Err(Unjudged::Unread));
}

#[test]
fn past_the_most_origins_kept_none_kept_is_forgotten_and_a_blank_frame_with_none_is_not_known() {
    let (judge, _) = judge(&[]);
    let (browser, engine) = browser_judged(judge);
    the_page_shows_a_frame(&engine, &browser, "KEPT", "PAGE-1", "about:blank");
    the_pages_frame_has(&engine, &browser, "KEPT", 90, "http://kept.test");
    the_page_shows_a_frame(&engine, &browser, "SRCDOC", "PAGE-1", "about:srcdoc");
    browser.read_sandboxes("PAGE-1");
    assert_eq!(
        super::super::lock(&browser.inner.state)
            .documents_of("PAGE-1", "SRCDOC")
            .unwrap(),
        ["http://fixture.test/frames.html"]
    );
    {
        let mut state = super::super::lock(&browser.inner.state);
        let mut id = 1000;
        while state.document_origins.kept.len() < super::super::FRAME_ORIGINS_KEPT {
            id += 1;
            state.document_origins.told(
                "S-ELSEWHERE",
                &json!({"id": id, "origin": "http://else.test", "auxData": {"isDefault": true, "frameId": format!("F-{id}")}}),
                None,
            );
        }
    }
    the_page_shows_a_frame(&engine, &browser, "LATE", "PAGE-1", "about:blank");
    engine.emit(
        "Runtime.executionContextCreated",
        json!({"context": {
            "id": 91,
            "origin": "://",
            "name": "",
            "uniqueId": "U-91",
            "auxData": {"isDefault": true, "type": "default", "frameId": "LATE"},
        }}),
        Some("S-PAGE-1"),
    );
    wait_for("the late context's word is taken in", || {
        !super::super::lock(&browser.inner.state)
            .document_origins
            .all_kept("S-PAGE-1")
    });
    let state = super::super::lock(&browser.inner.state);
    assert_eq!(
        state.documents_of("PAGE-1", "KEPT").unwrap(),
        ["http://kept.test/"],
        "what was kept stays"
    );
    assert_eq!(state.documents_of("PAGE-1", "LATE"), Err(Unjudged::Unread));
    assert_eq!(
        state.documents_of("PAGE-1", "SRCDOC"),
        Err(Unjudged::Unread)
    );
    assert!(
        state.document_origins.all_kept("S-ELSEWHERE"),
        "only the session whose word was not kept"
    );
}

#[test]
fn a_session_whose_contexts_are_cleared_has_every_new_one_kept_again() {
    let mut origins = super::super::DocumentOrigins::default();
    for id in 0..=super::super::FRAME_ORIGINS_KEPT as i64 {
        origins.told(
            "S-PAGE-1",
            &json!({"id": id, "origin": "http://else.test", "auxData": {"isDefault": true, "frameId": format!("F-{id}")}}),
            None,
        );
    }
    assert!(!origins.all_kept("S-PAGE-1"));
    assert_eq!(origins.kept.len(), super::super::FRAME_ORIGINS_KEPT);
    origins.told(
        "S-PAGE-1",
        &json!({"id": 5000, "origin": "http://moved.test", "auxData": {"isDefault": true, "frameId": "F-1"}}),
        None,
    );
    assert_eq!(
        origins.origin("S-PAGE-1", "F-1"),
        Some("http://moved.test"),
        "a frame already kept takes its new document's word"
    );
    origins.cleared("S-PAGE-1", |_| None);
    assert!(origins.all_kept("S-PAGE-1"));
    assert!(origins.kept.is_empty());
}

#[test]
fn a_frame_showing_the_engines_own_page_is_judged_by_the_document_around_it() {
    let (judge, told) = judge(&["http://fixture.test/"]);
    let (browser, engine) = browser_judged(judge);
    the_frame_shows(
        &engine,
        &browser,
        "chrome-error://chromewebdata/",
        Some("null"),
    );
    let answer = typed_into_the_frame(&browser).unwrap();
    assert_eq!(answer.asked, vec!["new.test:443".to_owned()]);
    assert!(engine.sent("Input.insertText").is_empty());
    assert_eq!(
        whys(&told).last().unwrap().0,
        "http://fixture.test/frames.html"
    );
}

#[test]
fn every_document_is_not_known_while_a_frame_stands_on_one_the_driver_does_not_know() {
    let (judge, _) = judge(&[]);
    let (browser, engine) = browser_judged(judge);
    the_page_shows_a_frame(
        &engine,
        &browser,
        "ERROR",
        "GONE",
        "chrome-error://chromewebdata/",
    );
    let state = super::super::lock(&browser.inner.state);
    assert_eq!(state.every_document("PAGE-1"), Err(Unjudged::Unread));
    assert_eq!(state.documents_of("PAGE-1", "ERROR"), Err(Unjudged::Unread));
}

#[test]
fn a_browser_with_no_judge_types_into_a_frame_with_no_web_origin() {
    let (browser, engine) = super::browser();
    the_frame_shows(
        &engine,
        &browser,
        "data:text/html,<button>Pay now</button>",
        None,
    );
    assert_ne!(
        typed_into_the_frame(&browser).err(),
        Some(crate::drive::Refusal::FrameOpaque)
    );
}

/// The page's own document is now at `url`, as Chrome tells it, with `origin` as its
/// `securityOrigin`.
fn the_page_shows(
    engine: &super::Engine,
    browser: &crate::drive::Browser,
    url: &str,
    origin: &str,
) {
    engine.emit(
        "Page.frameNavigated",
        json!({"frame": {"id": "PAGE-1", "loaderId": "LOADER-T", "name": "", "url": url, "securityOrigin": origin, "mimeType": "text/html"}, "type": "Navigation"}),
        Some("S-PAGE-1"),
    );
    wait_for("the page's new document is seen", || {
        frame_address(browser, "PAGE-1") == url
    });
}

/// Types into the page's own Name field, by calls `n` and `n + 1`.
fn typed_on_the_page(
    browser: &crate::drive::Browser,
    n: u32,
) -> Result<crate::drive::Answer, crate::drive::Refusal> {
    let text = snapshot(browser, n);
    let name = ref_of(&text, "- textbox \"Name\"");
    browser.call(
        &id(n + 1),
        Call::Type {
            page: None,
            fields: vec![Field {
                into: Some(name),
                text: "Grace".into(),
                replace: true,
            }],
        },
    )
}

#[test]
fn a_blank_page_is_judged_by_the_origin_its_own_context_has() {
    let (judge, told) = judge(&["http://third.test/"]);
    let (browser, engine) = browser_judged(judge);
    the_page_shows(&engine, &browser, "about:blank", "://");
    the_pages_frame_has(&engine, &browser, "PAGE-1", 61, "http://third.test");
    let answer = typed_on_the_page(&browser, 10).unwrap();
    assert_eq!(answer.asked, vec!["new.test:443".to_owned()]);
    assert!(engine.sent("Input.insertText").is_empty());
    assert_eq!(whys(&told).last().unwrap().0, "http://third.test/");
    assert_eq!(
        super::super::lock(&browser.inner.state)
            .every_document("PAGE-1")
            .unwrap()[0],
        "http://third.test/",
        "a key with no known focus goes by it too"
    );
}

#[test]
fn a_blank_page_with_no_word_of_its_own_context_is_not_known() {
    let (judge, told) = judge(&[]);
    let (browser, engine) = browser_judged(judge);
    the_page_shows(&engine, &browser, "about:blank", "://");
    assert_eq!(
        typed_on_the_page(&browser, 10).err(),
        Some(crate::drive::Refusal::FrameUnread)
    );
    assert!(engine.sent("Input.insertText").is_empty());
    assert!(
        !whys(&told)
            .iter()
            .any(|(address, ..)| address == "about:blank"),
        "the judge is never told an address it cannot place"
    );
    let state = super::super::lock(&browser.inner.state);
    assert_eq!(state.every_document("PAGE-1"), Err(Unjudged::Unread));
}

#[test]
fn a_page_with_no_web_origin_is_opaque_and_nothing_is_typed_into_it() {
    for (url, origin, context) in [
        ("data:text/html,<input aria-label=Name>", "://", None),
        ("blob:null/4d2c", "null", None),
    ] {
        let (judge, told) = judge(&[]);
        let (browser, engine) = browser_judged(judge);
        the_page_shows(&engine, &browser, url, origin);
        if let Some(context) = context {
            the_pages_frame_has(&engine, &browser, "PAGE-1", 62, context);
        }
        assert_eq!(
            typed_on_the_page(&browser, 10).err(),
            Some(crate::drive::Refusal::FrameOpaque),
            "{url}"
        );
        assert!(engine.sent("Input.insertText").is_empty(), "{url}");
        assert!(
            whys(&told)
                .iter()
                .all(|(address, ..)| address.starts_with("http")),
            "{url}: the judge is never told an address it cannot place"
        );
    }
}

#[test]
fn a_blob_page_is_judged_by_the_origin_that_made_it() {
    let (judge, told) = judge(&["http://third.test/"]);
    let (browser, engine) = browser_judged(judge);
    the_page_shows(
        &engine,
        &browser,
        "blob:http://third.test/4d2c",
        "http://third.test",
    );
    let answer = typed_on_the_page(&browser, 10).unwrap();
    assert_eq!(answer.asked, vec!["new.test:443".to_owned()]);
    assert!(engine.sent("Input.insertText").is_empty());
    assert_eq!(whys(&told).last().unwrap().0, "http://third.test/");
}

#[test]
fn a_frame_in_a_blank_page_is_judged_by_the_pages_own_origin() {
    let (judge, _) = judge(&[]);
    let (browser, engine) = browser_judged(judge);
    the_page_shows(&engine, &browser, "about:blank", "://");
    the_page_shows_a_frame(&engine, &browser, "SRCDOC", "PAGE-1", "about:srcdoc");
    browser.read_sandboxes("PAGE-1");
    assert_eq!(
        super::super::lock(&browser.inner.state).documents_of("PAGE-1", "SRCDOC"),
        Err(Unjudged::Unread),
        "the page's address says nothing of its origin"
    );
    the_pages_frame_has(&engine, &browser, "PAGE-1", 63, "http://third.test");
    assert_eq!(
        super::super::lock(&browser.inner.state)
            .documents_of("PAGE-1", "SRCDOC")
            .unwrap(),
        ["http://third.test/"]
    );
}

#[test]
fn the_engines_own_page_sends_nowhere() {
    let (judge, told) = judge(&["http://fixture.test/"]);
    let (browser, engine) = browser_judged(judge);
    the_page_shows(&engine, &browser, "chrome-error://chromewebdata/", "null");
    let state = super::super::lock(&browser.inner.state);
    assert_eq!(state.documents_of("PAGE-1", "PAGE-1"), Ok(Vec::new()));
    drop(state);
    assert!(
        !whys(&told)
            .iter()
            .any(|(address, ..)| address.starts_with("chrome-error")),
        "the judge is never told an address it cannot place"
    );
}

/// The engine answers that the element of every frame has `attributes`.
fn frame_elements_have(engine: &super::Engine, attributes: serde_json::Value) {
    engine.on(move |sent, _| {
        (sent.method == "DOM.describeNode" && sent.params["backendNodeId"] == 50).then(|| {
            Ok(json!({"node": {"nodeId": 0, "backendNodeId": 50, "nodeType": 1, "nodeName": "IFRAME", "localName": "iframe", "nodeValue": "", "attributes": attributes}}))
        })
    });
}

#[test]
fn a_sandboxed_srcdoc_frame_with_no_word_of_its_own_context_is_opaque() {
    let (judge, _) = judge(&[]);
    let (browser, engine) = browser_judged(judge);
    frame_elements_have(
        &engine,
        json!(["srcdoc", "<input>", "SANDBOX", "allow-forms"]),
    );
    the_page_shows_a_frame(&engine, &browser, "SRCDOC", "PAGE-1", "about:srcdoc");
    browser.read_sandboxes("PAGE-1");
    let state = super::super::lock(&browser.inner.state);
    assert_eq!(
        state.documents_of("PAGE-1", "SRCDOC"),
        Err(Unjudged::Opaque)
    );
    assert_eq!(
        state.every_document("PAGE-1"),
        Err(Unjudged::Opaque),
        "a key with no known focus may land in it"
    );
    drop(state);
    let owners = engine.sent("DOM.getFrameOwner");
    assert!(
        owners
            .iter()
            .any(|s| s.params["frameId"] == "SRCDOC" && s.session.as_deref() == Some("S-PAGE-1")),
        "{owners:?}"
    );
}

#[test]
fn a_sandboxed_frame_at_a_web_address_with_no_word_of_its_own_context_is_opaque() {
    let (judge, _) = judge(&[]);
    let (browser, engine) = browser_judged(judge);
    frame_elements_have(
        &engine,
        json!(["src", "https://shop.test/pay", "sandbox", ""]),
    );
    the_page_shows_a_frame(&engine, &browser, "SHOP", "PAGE-1", "https://shop.test/pay");
    browser.read_sandboxes("PAGE-1");
    let state = super::super::lock(&browser.inner.state);
    assert_eq!(state.documents_of("PAGE-1", "SHOP"), Err(Unjudged::Opaque));
    drop(state);
    let owners = engine.sent("DOM.getFrameOwner");
    assert!(
        owners
            .iter()
            .any(|s| s.params["frameId"] == "SHOP" && s.session.as_deref() == Some("S-PAGE-1")),
        "{owners:?}"
    );
}

#[test]
fn a_frames_element_is_read_in_the_session_of_the_document_around_it() {
    let (judge, _) = judge(&[]);
    let (browser, engine) = browser_judged(judge);
    frame_elements_have(&engine, json!(["sandbox", ""]));
    the_frame_shows(&engine, &browser, "https://shop.test/pay", None);
    browser.read_sandboxes("PAGE-1");
    let owners = engine.sent("DOM.getFrameOwner");
    assert!(
        owners
            .iter()
            .any(|s| s.params["frameId"] == "PAYMENT-FRAME"
                && s.session.as_deref() == Some("S-PAGE-1")),
        "the frame's own session holds no element of it: {owners:?}"
    );
    assert_eq!(
        super::super::lock(&browser.inner.state).documents_of("PAGE-1", "PAYMENT-FRAME"),
        Err(Unjudged::Opaque)
    );
}

#[test]
fn a_srcdoc_frame_sandboxed_with_its_origin_allowed_takes_the_page_origin() {
    let (judge, _) = judge(&[]);
    let (browser, engine) = browser_judged(judge);
    frame_elements_have(&engine, json!(["sandbox", "allow-forms Allow-Same-Origin"]));
    the_page_shows_a_frame(&engine, &browser, "SRCDOC", "PAGE-1", "about:srcdoc");
    browser.read_sandboxes("PAGE-1");
    assert_eq!(
        super::super::lock(&browser.inner.state)
            .documents_of("PAGE-1", "SRCDOC")
            .unwrap(),
        ["http://fixture.test/frames.html"]
    );
}

#[test]
fn a_srcdoc_frame_whose_element_cannot_be_read_is_not_known() {
    let (judge, _) = judge(&[]);
    let (browser, engine) = browser_judged(judge);
    engine.on(|sent, _| {
        (sent.method == "DOM.getFrameOwner").then_some(Err(crate::cdp::CallError::TimedOut))
    });
    the_page_shows_a_frame(&engine, &browser, "SRCDOC", "PAGE-1", "about:srcdoc");
    browser.read_sandboxes("PAGE-1");
    assert_eq!(
        super::super::lock(&browser.inner.state).documents_of("PAGE-1", "SRCDOC"),
        Err(Unjudged::Unread)
    );
}

#[test]
fn a_frames_sandbox_is_read_again_for_its_next_document() {
    let (judge, _) = judge(&[]);
    let (browser, engine) = browser_judged(judge);
    frame_elements_have(&engine, json!(["sandbox", ""]));
    the_page_shows_a_frame(&engine, &browser, "SRCDOC", "PAGE-1", "about:srcdoc");
    browser.read_sandboxes("PAGE-1");
    assert_eq!(
        super::super::lock(&browser.inner.state).documents_of("PAGE-1", "SRCDOC"),
        Err(Unjudged::Opaque)
    );
    frame_elements_have(&engine, json!([]));
    the_page_shows_a_frame(&engine, &browser, "SRCDOC", "PAGE-1", "about:blank");
    assert_eq!(
        super::super::lock(&browser.inner.state).documents_of("PAGE-1", "SRCDOC"),
        Err(Unjudged::Unread),
        "the old document's sandbox says nothing of the new one"
    );
    browser.read_sandboxes("PAGE-1");
    assert_eq!(
        super::super::lock(&browser.inner.state)
            .documents_of("PAGE-1", "SRCDOC")
            .unwrap(),
        ["http://fixture.test/frames.html"]
    );
}

#[test]
fn keys_with_no_known_focus_on_a_page_with_a_sandboxed_srcdoc_frame_are_held_back() {
    let (judge, _) = judge(&[]);
    let (browser, engine) = browser_judged(judge);
    frame_elements_have(&engine, json!(["sandbox", ""]));
    the_page_shows_a_frame(&engine, &browser, "SRCDOC", "PAGE-1", "about:srcdoc");
    // Nothing the page shows says it has the focus.
    {
        let mut model = super::super::lock(&engine.model);
        for nodes in model.ax.values_mut() {
            for node in nodes.as_array_mut().unwrap() {
                if let Some(properties) = node["properties"].as_array_mut() {
                    properties.retain(|one| one["name"] != "focused");
                }
            }
        }
    }
    let answer = browser.call(
        &id(10),
        Call::Press {
            page: None,
            keys: vec!["Enter".into()],
        },
    );
    assert!(engine.sent("Input.dispatchKeyEvent").is_empty());
    assert_eq!(answer.err(), Some(crate::drive::Refusal::FrameOpaque));
}

/// The page shows `count` srcdoc frames of its own beside what it shows already, each with no
/// word of its own context, so each one's sandbox is not read yet.
fn the_page_shows_srcdoc_frames(
    engine: &super::Engine,
    browser: &crate::drive::Browser,
    count: usize,
) {
    for n in 0..count {
        the_page_shows_a_frame(
            engine,
            browser,
            &format!("SRCDOC-{n}"),
            "PAGE-1",
            "about:srcdoc",
        );
    }
}

/// Takes the focus off everything the page shows.
fn nothing_has_the_focus(engine: &super::Engine) {
    let mut model = super::super::lock(&engine.model);
    for nodes in model.ax.values_mut() {
        for node in nodes.as_array_mut().unwrap() {
            if let Some(properties) = node["properties"].as_array_mut() {
                properties.retain(|one| one["name"] != "focused");
            }
        }
    }
}

/// How many frame elements' `sandbox` the driver read: each read describes the frame element
/// by the node the frame's owner names.
fn sandboxes_read(engine: &super::Engine) -> usize {
    engine
        .sent("DOM.describeNode")
        .iter()
        .filter(|sent| sent.params["backendNodeId"] == 50)
        .count()
}

/// The frames whose `sandbox` the driver read, as it asked for their owners just before.
fn frames_whose_sandbox_was_read(engine: &super::Engine) -> Vec<String> {
    let sent = engine.all_sent();
    sent.windows(2)
        .filter(|pair| {
            pair[0].method == "DOM.getFrameOwner"
                && pair[1].method == "DOM.describeNode"
                && pair[1].params["backendNodeId"] == 50
        })
        .map(|pair| {
            pair[0].params["frameId"]
                .as_str()
                .unwrap_or_default()
                .to_owned()
        })
        .collect()
}

#[test]
fn typing_into_the_page_beside_fifty_frames_reads_no_frames_sandbox() {
    let (judge, _) = judge(&[]);
    let (browser, engine) = browser_judged(judge);
    frame_elements_have(&engine, json!(["sandbox", "allow-same-origin"]));
    the_page_shows_srcdoc_frames(&engine, &browser, 50);
    typed_on_the_page(&browser, 10).unwrap();
    assert!(!engine.sent("Input.insertText").is_empty());
    assert_eq!(
        sandboxes_read(&engine),
        0,
        "the field is in the page's own document: no frame is on its way"
    );
}

#[test]
fn keys_into_the_page_beside_fifty_frames_read_no_frames_sandbox() {
    let (judge, _) = judge(&[]);
    let (browser, engine) = browser_judged(judge);
    frame_elements_have(&engine, json!(["sandbox", "allow-same-origin"]));
    the_page_shows_srcdoc_frames(&engine, &browser, 50);
    browser
        .call(
            &id(10),
            Call::Press {
                page: None,
                keys: vec!["a".into()],
            },
        )
        .unwrap();
    assert!(!engine.sent("Input.dispatchKeyEvent").is_empty());
    assert_eq!(sandboxes_read(&engine), 0);
}

#[test]
fn typing_into_a_frame_beside_fifty_frames_reads_only_that_frames_chain() {
    let (judge, _) = judge(&[]);
    let (browser, engine) = browser_judged(judge);
    frame_elements_have(&engine, json!(["sandbox", "allow-same-origin"]));
    the_page_shows_srcdoc_frames(&engine, &browser, 50);
    the_frame_shows(&engine, &browser, "about:srcdoc", None);
    let _ = typed_into_the_frame(&browser);
    let read = frames_whose_sandbox_was_read(&engine);
    assert_eq!(
        read,
        ["PAYMENT-FRAME"],
        "only the frame typed into is on the way"
    );
}

#[test]
fn a_frame_chain_is_read_from_the_frame_up_and_no_further() {
    let (judge, _) = judge(&[]);
    let (browser, engine) = browser_judged(judge);
    frame_elements_have(&engine, json!(["sandbox", "allow-same-origin"]));
    the_page_shows_a_frame(&engine, &browser, "OUTER", "PAGE-1", "about:srcdoc");
    the_page_shows_a_frame(&engine, &browser, "INNER", "OUTER", "about:srcdoc");
    the_page_shows_srcdoc_frames(&engine, &browser, 50);
    browser.read_chain_sandboxes("PAGE-1", "INNER");
    let mut read = frames_whose_sandbox_was_read(&engine);
    read.sort();
    assert_eq!(read, ["INNER", "OUTER"]);
    assert_eq!(
        super::super::lock(&browser.inner.state)
            .documents_of("PAGE-1", "INNER")
            .unwrap(),
        ["http://fixture.test/frames.html"]
    );
}

#[test]
fn keys_with_no_known_focus_read_at_most_one_chains_worth_of_frames_per_act() {
    let (judge, _) = judge(&[]);
    let (browser, engine) = browser_judged(judge);
    frame_elements_have(&engine, json!(["sandbox", "allow-same-origin"]));
    the_page_shows_srcdoc_frames(&engine, &browser, 50);
    nothing_has_the_focus(&engine);
    let press = |n: u32| {
        browser.call(
            &id(n),
            Call::Press {
                page: None,
                keys: vec!["a".into()],
            },
        )
    };
    assert_eq!(
        press(10).err(),
        Some(crate::drive::Refusal::FrameUnread),
        "frames not read yet may take the key"
    );
    assert!(engine.sent("Input.dispatchKeyEvent").is_empty());
    assert_eq!(sandboxes_read(&engine), super::super::FRAME_READS_PER_ACT);
    // Each act reads more, and what was read stays read: the key goes in the end.
    let mut n = 11;
    while press(n).is_err() {
        assert!(n < 30, "the frames are read in the end");
        n += 1;
    }
    assert!(!engine.sent("Input.dispatchKeyEvent").is_empty());
    assert_eq!(sandboxes_read(&engine), 50, "each frame is read once");
}

/// A fresh tab: its own document is blank, and its own context has no web origin.
fn a_fresh_blank_tab(engine: &super::Engine, browser: &crate::drive::Browser) {
    the_page_shows(engine, browser, "about:blank", "://");
    the_pages_frame_has(engine, browser, "PAGE-1", 62, "null");
}

#[test]
fn keys_on_a_fresh_blank_tab_are_refused_as_a_blank_tab() {
    let (judge, told) = judge(&[]);
    let (browser, engine) = browser_judged(judge);
    a_fresh_blank_tab(&engine, &browser);
    let refused = browser
        .call(
            &id(10),
            Call::Press {
                page: None,
                keys: vec!["a".into()],
            },
        )
        .err();
    assert_eq!(refused, Some(crate::drive::Refusal::BlankTab));
    assert!(
        refused.unwrap().to_string().contains("blank tab"),
        "the refusal names the blank tab"
    );
    assert!(engine.sent("Input.dispatchKeyEvent").is_empty());
    assert!(
        whys(&told)
            .iter()
            .all(|(address, ..)| address.starts_with("http")),
        "the judge is never told an address it cannot place"
    );
}

#[test]
fn typing_on_a_fresh_blank_tab_is_refused_as_a_blank_tab() {
    let (judge, _) = judge(&[]);
    let (browser, engine) = browser_judged(judge);
    a_fresh_blank_tab(&engine, &browser);
    assert_eq!(
        typed_on_the_page(&browser, 10).err(),
        Some(crate::drive::Refusal::BlankTab)
    );
    assert!(engine.sent("Input.insertText").is_empty());
}

/// The payment frame, held in a process of its own at a third site whose document's context
/// said so, is sent back into its page's process: it starts another document, which the page's
/// session tells of at `about:blank`.
fn a_frame_in_its_own_process_sent_back(engine: &super::Engine, browser: &crate::drive::Browser) {
    let third = "http://third.test/pay.html";
    frame_moves(engine, third, third, Duration::from_millis(1));
    frame_read(browser);
    a_frames_context_on(
        engine,
        browser,
        MOVED,
        "PAYMENT-FRAME",
        74,
        "http://third.test",
    );
    assert_eq!(documents_of(browser, "PAYMENT-FRAME").unwrap(), [third]);
    engine.emit(
        "Page.frameStartedLoading",
        json!({"frameId": "PAYMENT-FRAME"}),
        Some(MOVED),
    );
    everything_before_is_taken_in(engine, browser);
}

/// The page's session says the payment frame now shows `about:blank`.
fn the_page_says_the_frame_is_blank(engine: &super::Engine, browser: &crate::drive::Browser) {
    engine.emit(
        "Page.frameNavigated",
        json!({"frame": {"id": "PAYMENT-FRAME", "parentId": "PAGE-1", "name": "payment", "url": "about:blank"}}),
        Some("S-PAGE-1"),
    );
    everything_before_is_taken_in(engine, browser);
}

/// The payment frame's own session ends.
fn the_frames_session_ends(engine: &super::Engine, browser: &crate::drive::Browser) {
    engine.emit(
        "Target.detachedFromTarget",
        json!({"sessionId": MOVED, "targetId": "PAYMENT-FRAME"}),
        Some("S-PAGE-1"),
    );
    everything_before_is_taken_in(engine, browser);
}

#[test]
fn a_frame_sent_back_into_its_pages_process_before_its_session_ends_is_judged_by_its_new_document()
{
    let (judge, _) = judge(&[]);
    let (browser, engine) = browser_judged(judge);
    a_frame_in_its_own_process_sent_back(&engine, &browser);
    the_page_says_the_frame_is_blank(&engine, &browser);
    assert_eq!(
        documents_of(&browser, "PAYMENT-FRAME"),
        Err(Unjudged::Unread)
    );
    the_frames_session_ends(&engine, &browser);
    assert_eq!(
        documents_of(&browser, "PAYMENT-FRAME"),
        Err(Unjudged::Unread)
    );
    the_pages_frame_has(
        &engine,
        &browser,
        "PAYMENT-FRAME",
        75,
        "http://fixture.test",
    );
    assert_eq!(
        documents_of(&browser, "PAYMENT-FRAME").unwrap(),
        ["http://fixture.test/"]
    );
}

#[test]
fn a_frame_sent_back_into_its_pages_process_after_its_session_ends_is_judged_by_its_new_document() {
    let (judge, _) = judge(&[]);
    let (browser, engine) = browser_judged(judge);
    a_frame_in_its_own_process_sent_back(&engine, &browser);
    the_frames_session_ends(&engine, &browser);
    assert_eq!(
        documents_of(&browser, "PAYMENT-FRAME"),
        Err(Unjudged::Unread)
    );
    the_page_says_the_frame_is_blank(&engine, &browser);
    assert_eq!(
        documents_of(&browser, "PAYMENT-FRAME"),
        Err(Unjudged::Unread)
    );
    the_pages_frame_has(
        &engine,
        &browser,
        "PAYMENT-FRAME",
        75,
        "http://fixture.test",
    );
    assert_eq!(
        documents_of(&browser, "PAYMENT-FRAME").unwrap(),
        ["http://fixture.test/"]
    );
}

#[test]
fn a_frame_sent_back_whose_new_document_is_told_before_the_page_says_where_it_is_is_judged_by_it() {
    let (judge, _) = judge(&[]);
    let (browser, engine) = browser_judged(judge);
    a_frame_in_its_own_process_sent_back(&engine, &browser);
    the_pages_frame_has(
        &engine,
        &browser,
        "PAYMENT-FRAME",
        75,
        "http://fixture.test",
    );
    assert_eq!(
        documents_of(&browser, "PAYMENT-FRAME"),
        Err(Unjudged::Unread)
    );
    the_page_says_the_frame_is_blank(&engine, &browser);
    the_frames_session_ends(&engine, &browser);
    assert_eq!(
        documents_of(&browser, "PAYMENT-FRAME").unwrap(),
        ["http://fixture.test/"]
    );
}

#[test]
fn a_frame_whose_own_session_ends_is_not_judged_by_its_old_document_until_the_page_says_where_it_is()
 {
    let (judge, _) = judge(&[]);
    let (browser, engine) = browser_judged(judge);
    let third = "http://third.test/pay.html";
    frame_moves(&engine, third, third, Duration::from_millis(1));
    frame_read(&browser);
    // No context of its document was told: only its address said what it showed.
    assert_eq!(documents_of(&browser, "PAYMENT-FRAME").unwrap(), [third]);
    the_frames_session_ends(&engine, &browser);
    assert_eq!(
        documents_of(&browser, "PAYMENT-FRAME"),
        Err(Unjudged::Unread)
    );
}

#[test]
fn a_frames_session_that_no_longer_holds_it_does_not_move_it_back() {
    let (judge, _) = judge(&[]);
    let (browser, engine) = browser_judged(judge);
    a_frame_in_its_own_process_sent_back(&engine, &browser);
    the_page_says_the_frame_is_blank(&engine, &browser);
    engine.emit(
        "Page.frameNavigated",
        json!({"frame": {"id": "PAYMENT-FRAME", "parentId": "PAGE-1", "name": "payment", "url": "http://third.test/late.html"}}),
        Some(MOVED),
    );
    everything_before_is_taken_in(&engine, &browser);
    assert_eq!(frame_address(&browser, "PAYMENT-FRAME"), "about:blank");
}

/// What a secret purlis fills (#5115) is judged by in frame `frame` of the page.
fn fill_origin(browser: &crate::drive::Browser, frame: &str) -> Option<String> {
    super::super::lock(&browser.inner.state)
        .document_now("PAGE-1", frame)
        .and_then(|(_, origin)| origin)
}

#[test]
fn a_secret_is_filled_into_no_frame_whose_new_document_the_engine_has_not_placed_yet() {
    let (browser, engine) = a_web_frame_that_starts_another_document();
    assert_eq!(
        fill_origin(&browser, "WEB"),
        None,
        "though it is judged by the old document until it is replaced, nothing is filled there"
    );
    the_pages_frame_has(&engine, &browser, "WEB", 93, "http://fixture.test");
    assert_eq!(
        fill_origin(&browser, "WEB"),
        None,
        "its address still says the old origin, but its document may be on another"
    );
    the_page_shows_a_frame(
        &engine,
        &browser,
        "WEB",
        "PAGE-1",
        "http://fixture.test/b.html",
    );
    assert_eq!(
        fill_origin(&browser, "WEB").as_deref(),
        Some("http://fixture.test")
    );
}

#[test]
fn a_secret_is_filled_into_no_frame_whose_document_ended() {
    let (browser, engine) = a_web_frame_that_starts_another_document();
    the_pages_context_ends(&engine, &browser, 92);
    assert_eq!(fill_origin(&browser, "WEB"), None);
}

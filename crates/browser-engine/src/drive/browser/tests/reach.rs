// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Where a browser may go (#2018): every request a page is about to send is held until it is
//! judged, told as what it is (a read, an address the agent wrote, or something sent), and let
//! on or failed as the judge says; a site the judge asks about is in the act's answer.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::json;

use super::{
    MOVED, browser_judged, frame_address, frame_attaches, frame_moves, id, ref_of, snapshot,
    wait_for,
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
    let text = snapshot(browser, 50);
    let pay = ref_of(&text, "- button \"Pay now\"");
    browser.call(
        &id(51),
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
    frame_moves(&engine, third, third, Duration::from_secs(3));
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
fn a_frame_whose_tree_fails_stays_not_known_and_stands_on_no_ancestor() {
    let (judge, _) = judge(&[]);
    let (browser, engine) = browser_judged(judge);
    engine.on(|sent, _| {
        (sent.method == "Page.getFrameTree" && sent.session.as_deref() == Some(MOVED))
            .then_some(Err(crate::cdp::CallError::TimedOut))
    });
    frame_attaches(&engine, "");
    let state = super::super::lock(&browser.inner.state);
    assert_eq!(state.documents_of("PAGE-1", "PAYMENT-FRAME"), None);
    assert_eq!(state.every_document("PAGE-1"), None);
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
    assert_eq!(state.documents_of("PAGE-1", "INSIDE"), None);
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
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(frame_address(&browser, "PAYMENT-FRAME"), third);
}

#[test]
fn a_tree_at_no_address_does_not_replace_the_address_a_frame_attached_at() {
    let (judge, _) = judge(&[]);
    let (browser, engine) = browser_judged(judge);
    let third = "http://third.test/pay.html";
    frame_moves(&engine, third, "about:blank", Duration::from_millis(1));
    std::thread::sleep(Duration::from_millis(200));
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

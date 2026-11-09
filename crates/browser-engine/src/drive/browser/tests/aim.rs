// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! An act inside a cross-site frame the page turns, scales or zooms lands where the element is
//! drawn on the page (#2557). Where the element is not shown on the first read, each frame
//! element is scrolled into view, outermost first, and read again; and an act lands only where
//! the page's own window shows (#2690). An act lands only where what is drawn at its point is
//! the element aimed at, its label or an element holding it.

use serde_json::{Value, json};

use super::super::{DRAWN_FOR, DRAWN_TWICE};
use super::{browser, id, mouse, ref_of, snapshot};
use crate::drive::{Call, DragTo, Refusal};

/// The payment frame's element is drawn as `border` with its document in `content` (page CSS
/// pixels), laid out `width` by `height` CSS pixels before any transform.
fn frame_drawn_as(engine: &super::Engine, content: Value, border: Value, width: f64, height: f64) {
    engine.on(move |sent, _| {
        (sent.method == "DOM.getBoxModel").then(|| {
            Ok(
                json!({"model":{"content":content,"padding":content,"border":border,
                "margin":border,"width":width,"height":height}}),
            )
        })
    });
}

/// The frame element is laid out 220 by 80 with a border of 10 at (100, 200), and drawn a
/// quarter turn clockwise at half its size about its top left corner: a point (x, y) of its box
/// is drawn at (100 - y / 2, 200 + x / 2).
fn turned_and_halved(engine: &super::Engine) {
    frame_drawn_as(
        engine,
        json!([95, 205, 95, 305, 65, 305, 65, 205]),
        json!([100, 200, 100, 310, 60, 310, 60, 200]),
        220.0,
        80.0,
    );
}

fn presses_and_releases(engine: &super::Engine) -> Vec<(String, f64, f64)> {
    mouse(engine)
        .into_iter()
        .filter(|(kind, ..)| kind != "mouseMoved")
        .map(|(kind, x, y, _)| (kind, x, y))
        .collect()
}

#[test]
fn a_click_in_a_frame_the_page_turns_and_scales_lands_where_the_element_is_drawn() {
    let (browser, engine) = browser();
    turned_and_halved(&engine);
    let pay = ref_of(&snapshot(&browser, 1), "- button \"Pay now\"");

    browser.call(&id(2), Call::Click { target: pay }).unwrap();

    // The button's centre in its document (20, 15) is (30, 25) of the frame's box, inside its
    // border, which is drawn at (100 - 12.5, 200 + 15).
    assert_eq!(
        presses_and_releases(&engine),
        vec![
            ("mousePressed".into(), 87.5, 215.0),
            ("mouseReleased".into(), 87.5, 215.0),
        ]
    );
}

#[test]
fn a_click_in_a_frame_the_page_zooms_lands_where_the_element_is_drawn() {
    let (browser, engine) = browser();
    // Laid out 200 by 60 at (100, 200), drawn at 0.6 of that.
    let quad = json!([100, 200, 220, 200, 220, 236, 100, 236]);
    frame_drawn_as(&engine, quad.clone(), quad, 200.0, 60.0);
    let pay = ref_of(&snapshot(&browser, 1), "- button \"Pay now\"");

    browser.call(&id(2), Call::Click { target: pay }).unwrap();

    assert_eq!(
        presses_and_releases(&engine),
        vec![
            ("mousePressed".into(), 112.0, 209.0),
            ("mouseReleased".into(), 112.0, 209.0),
        ]
    );
}

#[test]
fn a_drag_onto_an_element_in_a_turned_frame_ends_where_the_element_is_drawn() {
    let (browser, engine) = browser();
    turned_and_halved(&engine);
    let text = snapshot(&browser, 1);
    let link = ref_of(&text, "- link \"Read more\"");
    let pay = ref_of(&text, "- button \"Pay now\"");

    browser
        .call(
            &id(2),
            Call::Drag {
                from: link,
                to: DragTo::Element(pay),
            },
        )
        .unwrap();

    let acts = presses_and_releases(&engine);
    assert_eq!(acts.last(), Some(&("mouseReleased".into(), 87.5, 215.0)));
}

#[test]
fn a_drag_by_an_offset_in_a_turned_frame_starts_where_the_element_is_drawn() {
    let (browser, engine) = browser();
    turned_and_halved(&engine);
    let pay = ref_of(&snapshot(&browser, 1), "- button \"Pay now\"");

    browser
        .call(
            &id(2),
            Call::Drag {
                from: pay,
                to: DragTo::Offset { x: 10.0, y: 0.0 },
            },
        )
        .unwrap();

    // The offset is on the page as it is shown, as the screenshot the agent reads shows it.
    assert_eq!(
        presses_and_releases(&engine),
        vec![
            ("mousePressed".into(), 87.5, 215.0),
            ("mouseReleased".into(), 97.5, 215.0),
        ]
    );
}

#[test]
fn a_click_in_a_frame_drawn_flat_to_a_line_is_answered_not_shown() {
    let (browser, engine) = browser();
    let line = json!([100, 200, 300, 200, 300, 200, 100, 200]);
    frame_drawn_as(&engine, line.clone(), line, 200.0, 60.0);
    let pay = ref_of(&snapshot(&browser, 1), "- button \"Pay now\"");

    let answer = browser.call(
        &id(2),
        Call::Click {
            target: pay.clone(),
        },
    );

    assert_eq!(answer, Err(Refusal::NotShown(pay)));
    assert!(presses_and_releases(&engine).is_empty());
}

/// The button of the payment frame's document is at `quad` of that document.
fn the_button_at(engine: &super::Engine, quad: Vec<f64>) {
    super::super::lock(&engine.model).quads.insert(7, quad);
}

fn refused_with_nothing_sent(browser: &super::Browser, engine: &super::Engine) {
    let pay = ref_of(&snapshot(browser, 1), "- button \"Pay now\"");

    let answer = browser.call(
        &id(2),
        Call::Click {
            target: pay.clone(),
        },
    );

    assert_eq!(answer, Err(Refusal::NotShown(pay)));
    assert!(mouse(engine).is_empty(), "nothing reaches the page");
}

#[test]
fn a_click_on_an_element_beyond_its_upright_frames_edge_is_answered_not_shown() {
    let (browser, engine) = browser();
    // The frame shows 200 by 60 of its document; the button is drawn right of that.
    the_button_at(
        &engine,
        vec![250.0, 10.0, 270.0, 10.0, 270.0, 20.0, 250.0, 20.0],
    );
    refused_with_nothing_sent(&browser, &engine);
}

#[test]
fn a_click_on_an_element_above_its_upright_frames_top_is_answered_not_shown() {
    let (browser, engine) = browser();
    the_button_at(
        &engine,
        vec![10.0, -40.0, 30.0, -40.0, 30.0, -30.0, 10.0, -30.0],
    );
    refused_with_nothing_sent(&browser, &engine);
}

#[test]
fn a_click_on_an_element_beyond_its_turned_frames_edge_is_answered_not_shown() {
    let (browser, engine) = browser();
    turned_and_halved(&engine);
    // Inside its border the turned frame shows 200 by 60 of its document.
    the_button_at(
        &engine,
        vec![10.0, 70.0, 30.0, 70.0, 30.0, 80.0, 10.0, 80.0],
    );
    refused_with_nothing_sent(&browser, &engine);
}

#[test]
fn a_click_on_an_element_centred_on_its_frames_edge_is_answered_not_shown() {
    let (browser, engine) = browser();
    // Centred on the frame's right edge, at x 200 of its 200 pixels: the edge is not inside.
    the_button_at(
        &engine,
        vec![190.0, 10.0, 210.0, 10.0, 210.0, 20.0, 190.0, 20.0],
    );
    refused_with_nothing_sent(&browser, &engine);
}

#[test]
fn a_click_a_pixel_inside_its_frames_edge_is_carried_out() {
    let (browser, engine) = browser();
    // Centred at x 199 of the frame's 200 pixels.
    the_button_at(
        &engine,
        vec![189.0, 10.0, 209.0, 10.0, 209.0, 20.0, 189.0, 20.0],
    );
    let pay = ref_of(&snapshot(&browser, 1), "- button \"Pay now\"");

    browser.call(&id(2), Call::Click { target: pay }).unwrap();

    assert_eq!(
        presses_and_releases(&engine),
        vec![
            ("mousePressed".into(), 299.0, 215.0),
            ("mouseReleased".into(), 299.0, 215.0),
        ]
    );
}

#[test]
fn a_frame_scrolled_to_show_the_element_is_read_again_and_aimed_at_where_it_is_now() {
    let (browser, engine) = browser();
    // The frame element (backend node 50) is at (100, 200) until the page scrolls it to show
    // the button, and at (100, 100) after.
    let quad = |top: f64| json!([100.0, top, 300.0, top, 300.0, top + 60.0, 100.0, top + 60.0]);
    let mut scrolled = false;
    engine.on(move |sent, _| {
        if sent.method == "DOM.scrollIntoViewIfNeeded" && sent.params["backendNodeId"] == 50 {
            scrolled = true;
        }
        (sent.method == "DOM.getBoxModel").then(|| {
            let q = quad(if scrolled { 100.0 } else { 200.0 });
            Ok(
                json!({"model":{"content":q,"padding":q,"border":q,"margin":q,
                "width":200,"height":60}}),
            )
        })
    });
    let pay = ref_of(&snapshot(&browser, 1), "- button \"Pay now\"");

    browser.call(&id(2), Call::Click { target: pay }).unwrap();

    // The frame was scrolled around the button's centre in its box, (20, 15).
    let owner_scrolls: Vec<_> = engine
        .sent("DOM.scrollIntoViewIfNeeded")
        .into_iter()
        .filter(|s| s.params["backendNodeId"] == 50)
        .map(|s| s.params["rect"].clone())
        .collect();
    assert_eq!(
        owner_scrolls,
        vec![json!({"x": 19.0, "y": 14.0, "width": 2.0, "height": 2.0})]
    );
    assert_eq!(
        presses_and_releases(&engine),
        vec![
            ("mousePressed".into(), 120.0, 115.0),
            ("mouseReleased".into(), 120.0, 115.0),
        ]
    );
}

#[test]
fn a_click_in_a_frame_whose_corners_cross_is_answered_not_shown() {
    let (browser, engine) = browser();
    let crossed = json!([100, 200, 300, 260, 300, 200, 100, 260]);
    frame_drawn_as(&engine, crossed.clone(), crossed, 200.0, 60.0);
    refused_with_nothing_sent(&browser, &engine);
}

#[test]
fn a_click_in_a_frame_that_does_not_come_to_rest_after_its_scroll_is_answered_not_shown() {
    let (browser, engine) = browser();
    // Each read finds the frame a pixel lower than the last.
    let mut top = 200.0;
    engine.on(move |sent, _| {
        (sent.method == "DOM.getBoxModel").then(|| {
            top += 1.0;
            let q = json!([100.0, top, 300.0, top, 300.0, top + 60.0, 100.0, top + 60.0]);
            Ok(
                json!({"model":{"content":q,"padding":q,"border":q,"margin":q,
                "width":200,"height":60}}),
            )
        })
    });
    refused_with_nothing_sent(&browser, &engine);
}

#[test]
fn an_element_not_shown_on_the_first_read_is_aimed_at_once_its_frames_are_scrolled_to_it() {
    let (browser, engine) = browser();
    // The button lies below what the frame shows of its document until the frame element is
    // scrolled into view plainly, as a frame nested below another frame's fold does.
    let mut plain = false;
    engine.on(move |sent, _| {
        if sent.method == "DOM.scrollIntoViewIfNeeded"
            && sent.params["backendNodeId"] == 50
            && sent.params["rect"].is_null()
        {
            plain = true;
        }
        (sent.method == "DOM.getContentQuads" && sent.params["backendNodeId"] == 7).then(|| {
            let top = if plain { 10.0 } else { 100.0 };
            Ok(json!({"quads": [[10.0, top, 30.0, top, 30.0, top + 10.0, 10.0, top + 10.0]]}))
        })
    });
    let pay = ref_of(&snapshot(&browser, 1), "- button \"Pay now\"");

    browser.call(&id(2), Call::Click { target: pay }).unwrap();

    let owner_scrolls: Vec<_> = engine
        .sent("DOM.scrollIntoViewIfNeeded")
        .into_iter()
        .filter(|s| s.params["backendNodeId"] == 50)
        .map(|s| s.params["rect"].clone())
        .collect();
    assert_eq!(
        owner_scrolls,
        vec![
            Value::Null,
            json!({"x": 19.0, "y": 14.0, "width": 2.0, "height": 2.0})
        ]
    );
    assert_eq!(
        presses_and_releases(&engine),
        vec![
            ("mousePressed".into(), 120.0, 215.0),
            ("mouseReleased".into(), 120.0, 215.0),
        ]
    );
}

#[test]
fn an_element_still_not_shown_once_its_frames_are_scrolled_is_answered_not_shown() {
    let (browser, engine) = browser();
    the_button_at(
        &engine,
        vec![10.0, 100.0, 30.0, 100.0, 30.0, 110.0, 10.0, 110.0],
    );
    refused_with_nothing_sent(&browser, &engine);
    assert!(
        engine
            .sent("DOM.scrollIntoViewIfNeeded")
            .iter()
            .any(|s| s.params["backendNodeId"] == 50 && s.params["rect"].is_null()),
        "the frame element was scrolled before the answer"
    );
}

#[test]
fn a_click_in_a_frame_below_the_pages_window_is_answered_not_shown() {
    let (browser, engine) = browser();
    // The frame stays below the page's 800 by 600 window, though its button is inside it.
    let q = json!([100, 700, 300, 700, 300, 760, 100, 760]);
    frame_drawn_as(&engine, q.clone(), q, 200.0, 60.0);
    refused_with_nothing_sent(&browser, &engine);
}

#[test]
fn a_click_on_an_element_of_the_page_left_of_its_window_is_answered_not_shown() {
    let (browser, engine) = browser();
    super::super::lock(&engine.model)
        .quads
        .insert(12, vec![-60.0, 40.0, -40.0, 40.0, -40.0, 50.0, -60.0, 50.0]);
    let more = ref_of(&snapshot(&browser, 1), "- link \"Read more\"");

    let answer = browser.call(
        &id(2),
        Call::Click {
            target: more.clone(),
        },
    );

    assert_eq!(answer, Err(Refusal::NotShown(more)));
    assert!(mouse(&engine).is_empty(), "nothing reaches the page");
}

#[test]
fn each_document_draws_its_scroll_before_an_act_in_a_frame_is_aimed() {
    let (browser, engine) = browser();
    let pay = ref_of(&snapshot(&browser, 1), "- button \"Pay now\"");

    browser.call(&id(2), Call::Click { target: pay }).unwrap();

    let sent = engine.all_sent();
    let last_scroll = sent
        .iter()
        .rposition(|s| s.method == "DOM.scrollIntoViewIfNeeded")
        .unwrap();
    let press = sent
        .iter()
        .position(|s| s.method == "Input.dispatchMouseEvent" && s.params["type"] == "mousePressed")
        .unwrap();
    for session in ["S-PAGE-1", "S-PAYMENT"] {
        let drawn = sent
            .iter()
            .enumerate()
            .find(|(_, s)| {
                s.method == "Runtime.evaluate"
                    && s.params["expression"] == DRAWN_TWICE
                    && s.session.as_deref() == Some(session)
            })
            .map(|(n, s)| (n, s.params["uniqueContextId"].clone()))
            .unwrap_or_else(|| panic!("{session} never drew its scroll"));
        assert!(last_scroll < drawn.0 && drawn.0 < press, "{session}");
        assert!(drawn.1.is_string(), "in purlis's own world");
    }
}

/// What the page's own document draws at any point: `backend`, an element of its own.
fn page_draws(engine: &super::Engine, backend: i64) {
    engine.on(move |sent, _| {
        (sent.method == "DOM.getNodeForLocation" && sent.session.as_deref() == Some("S-PAGE-1"))
            .then(|| Ok(json!({"backendNodeId": backend, "frameId": "PAGE-1"})))
    });
}

fn pressed(engine: &super::Engine) -> bool {
    mouse(engine)
        .iter()
        .any(|(kind, ..)| kind == "mousePressed")
}

#[test]
fn a_click_where_another_element_of_the_page_is_drawn_over_the_element_is_refused_as_not_shown() {
    let (browser, engine) = browser();
    let more = ref_of(&snapshot(&browser, 1), "- link \"Read more\"");
    page_draws(&engine, 99);

    let answer = browser.call(
        &id(2),
        Call::Click {
            target: more.clone(),
        },
    );

    assert_eq!(answer, Err(Refusal::NotShown(more)));
    assert!(!pressed(&engine), "nothing is pressed");
}

#[test]
fn a_click_where_the_elements_label_is_drawn_over_it_presses() {
    let (browser, engine) = browser();
    let more = ref_of(&snapshot(&browser, 1), "- link \"Read more\"");
    // A label whose control is the link (backend 12) is drawn over it, and does not hold it.
    super::super::lock(&engine.model).labels.insert(99, 12);
    page_draws(&engine, 99);

    browser.call(&id(2), Call::Click { target: more }).unwrap();

    assert!(pressed(&engine));
}

#[test]
fn a_click_where_the_page_draws_over_the_element_once_the_mouse_is_there_presses_nothing() {
    let (browser, engine) = browser();
    let more = ref_of(&snapshot(&browser, 1), "- link \"Read more\"");
    // The link (backend 12) is drawn at its centre until the mouse moves there; then the page
    // draws another element of its own over it.
    engine.on(|sent, engine| {
        (sent.method == "DOM.getNodeForLocation" && sent.session.as_deref() == Some("S-PAGE-1"))
            .then(|| {
                let moved = !engine.sent("Input.dispatchMouseEvent").is_empty();
                Ok(json!({"backendNodeId": if moved { 99 } else { 12 }, "frameId": "PAGE-1"}))
            })
    });

    let answer = browser.call(
        &id(2),
        Call::Click {
            target: more.clone(),
        },
    );

    assert!(
        matches!(&answer, Err(Refusal::Partly { why, .. }) if *why == Refusal::NotShown(more).to_string()),
        "{answer:?}"
    );
    assert_eq!(
        mouse(&engine)
            .iter()
            .map(|(kind, ..)| kind.as_str())
            .collect::<Vec<_>>(),
        vec!["mouseMoved"],
        "the mouse moved and nothing was pressed"
    );
}

#[test]
fn a_click_on_a_frames_element_where_the_page_draws_another_frame_over_it_is_refused() {
    let (browser, engine) = browser();
    // The page's own document holds another frame, of its own site, drawn over the payment
    // frame: what is drawn there is that frame's document, held by an element (51) of the
    // page's own that is not the payment frame's.
    engine.emit(
        "Page.frameAttached",
        json!({"frameId":"AD-FRAME","parentFrameId":"PAGE-1"}),
        Some("S-PAGE-1"),
    );
    engine.emit(
        "Page.frameNavigated",
        json!({"frame":{"id":"AD-FRAME","parentId":"PAGE-1","loaderId":"LOADER-8","url":"http://fixture.test/ad.html","securityOrigin":"http://fixture.test","mimeType":"text/html","name":""},"type":"Navigation"}),
        Some("S-PAGE-1"),
    );
    super::wait_for("the frame over it is seen", || {
        super::super::lock(&browser.inner.state)
            .page("PAGE-1")
            .is_some_and(|page| page.frames.contains_key("AD-FRAME"))
    });
    let pay = ref_of(&snapshot(&browser, 1), "- button \"Pay now\"");
    engine.on(|sent, _| match sent.method.as_str() {
        "DOM.getNodeForLocation" if sent.session.as_deref() == Some("S-PAGE-1") => {
            Some(Ok(json!({"backendNodeId": 80, "frameId": "AD-FRAME"})))
        }
        "DOM.getFrameOwner" if sent.params["frameId"] == "AD-FRAME" => {
            Some(Ok(json!({"backendNodeId": 51, "nodeId": 0})))
        }
        _ => None,
    });

    let answer = browser.call(
        &id(2),
        Call::Click {
            target: pay.clone(),
        },
    );

    assert_eq!(answer, Err(Refusal::NotShown(pay)));
    assert!(!pressed(&engine), "nothing is pressed");
}

#[test]
fn an_act_on_the_pages_own_document_draws_its_scroll_before_it_is_aimed() {
    let (browser, engine) = browser();
    let more = ref_of(&snapshot(&browser, 1), "- link \"Read more\"");

    browser.call(&id(2), Call::Click { target: more }).unwrap();

    let sent = engine.all_sent();
    let scroll = sent
        .iter()
        .rposition(|s| s.method == "DOM.scrollIntoViewIfNeeded")
        .expect("the link is scrolled into view");
    let drawn = sent
        .iter()
        .position(|s| {
            s.method == "Runtime.evaluate"
                && s.params["expression"] == DRAWN_TWICE
                && s.session.as_deref() == Some("S-PAGE-1")
        })
        .expect("the page draws its scroll");
    let aimed = sent
        .iter()
        .rposition(|s| s.method == "DOM.getContentQuads" && s.params["backendNodeId"] == 12)
        .unwrap();
    assert!(
        scroll < drawn && drawn < aimed,
        "{scroll} < {drawn} < {aimed}"
    );
    assert!(
        sent[drawn].params["uniqueContextId"].is_string(),
        "in purlis's own world"
    );
}

#[test]
fn a_click_where_a_box_the_pointer_passes_through_is_drawn_over_the_element_presses() {
    let (browser, engine) = browser();
    let more = ref_of(&snapshot(&browser, 1), "- link \"Read more\"");
    // A `pointer-events: none` box (99) of the page's own is drawn over the link: the press
    // passes through it to the link.
    super::super::lock(&engine.model).passed_through = Some(99);

    browser.call(&id(2), Call::Click { target: more }).unwrap();

    assert!(pressed(&engine));
}

/// The page's own document draws a pseudo-element (98) at any point, which belongs to the
/// element `owner`, listed after the link (12) among the elements it may stand for.
fn a_pseudo_element_of(engine: &super::Engine, owner: i64) {
    engine.on(move |sent, _| {
        let session = sent.session.as_deref();
        match sent.method.as_str() {
            "DOM.getNodeForLocation" if session == Some("S-PAGE-1") => {
                Some(Ok(json!({"backendNodeId": 98, "frameId": "PAGE-1"})))
            }
            "DOM.describeNode" if sent.params["backendNodeId"] == 98 => Some(Ok(
                json!({"node":{"nodeId":0,"backendNodeId":98,"nodeType":1,"nodeName":"::before","pseudoType":"before"}}),
            )),
            "Runtime.callFunctionOn" if sent.params["functionDeclaration"] == DRAWN_FOR => {
                Some(Ok(json!({"result":{"type":"object","objectId":"LISTED"}})))
            }
            "Runtime.getProperties" if sent.params["objectId"] == "LISTED" => Some(Ok(json!({
                "result": [
                    {"name":"0","value":{"type":"object","subtype":"node","objectId":"EL-12"}},
                    {"name":"1","value":{"type":"object","subtype":"node","objectId":format!("EL-{owner}")}},
                    {"name":"length","value":{"type":"number","value":2}}
                ]
            }))),
            "DOM.describeNode" if sent.params["objectId"] == "EL-12" => Some(Ok(
                json!({"node":{"nodeId":0,"backendNodeId":12,"nodeType":1,"nodeName":"A"}}),
            )),
            "DOM.describeNode" if sent.params["objectId"] == format!("EL-{owner}") => Some(Ok(
                json!({"node":{"nodeId":0,"backendNodeId":owner,"nodeType":1,"nodeName":"LABEL",
                "pseudoElements":[{"nodeId":0,"backendNodeId":98,"nodeType":1,"nodeName":"::before","pseudoType":"before"}]}}),
            )),
            _ => None,
        }
    });
}

#[test]
fn a_click_where_the_elements_labels_pseudo_element_is_drawn_over_it_presses() {
    let (browser, engine) = browser();
    let more = ref_of(&snapshot(&browser, 1), "- link \"Read more\"");
    // A label (99) whose control is the link draws a `::before` box over it.
    super::super::lock(&engine.model).labels.insert(99, 12);
    a_pseudo_element_of(&engine, 99);

    browser.call(&id(2), Call::Click { target: more }).unwrap();

    assert!(pressed(&engine));
}

#[test]
fn a_click_where_another_elements_pseudo_element_is_drawn_over_the_element_is_refused() {
    let (browser, engine) = browser();
    let more = ref_of(&snapshot(&browser, 1), "- link \"Read more\"");
    // The `::before` box drawn over the link is another element's (99), no label of it.
    a_pseudo_element_of(&engine, 99);

    let answer = browser.call(
        &id(2),
        Call::Click {
            target: more.clone(),
        },
    );

    assert_eq!(answer, Err(Refusal::NotShown(more)));
    assert!(!pressed(&engine), "nothing is pressed");
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! An act inside a cross-site frame the page turns, scales or zooms lands where the element is
//! drawn on the page (#2557).

use serde_json::{Value, json};

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

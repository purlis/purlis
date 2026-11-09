// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! An act lands only where what is drawn at its point is the element aimed at, an element that
//! holds it or its label: a button another part of the page draws a transparent box over is
//! refused as not shown and never pressed; a button under a box the pointer passes through is
//! pressed; and a hidden checkbox is still clicked through its label drawn over it, through the
//! box a web component draws around the slot it is in, and through its label's `::before`.
//!
//! These need a real Chrome and a loopback port for the fixture site, which a sandboxed agent
//! does not have: they run where `PURLIS_TEST_ENGINE` names a Chrome executable, and on CI.

#![cfg(unix)]

use crate::common;

use common::drive::{call, find, next_id, open, ref_on, sees, snapshot};
use common::site::started;
use purlis_browser_engine::drive::{Call, Refusal};

#[test]
fn a_click_on_a_button_another_element_is_drawn_over_is_refused_and_never_pressed() {
    let Some((browser, _folder, _site)) = started() else {
        return;
    };
    open(&browser, "drawn-over.html");
    let button = ref_on(&find(&browser, "Covered button"), "Covered button");

    assert_eq!(
        browser.call(
            &next_id(),
            Call::Click {
                target: button.clone()
            }
        ),
        Err(Refusal::NotShown(button))
    );
    let after = snapshot(&browser);
    assert!(after.contains("Not pressed"), "{after}");
}

#[test]
fn a_click_on_a_hidden_checkbox_its_label_is_drawn_over_toggles_it() {
    let Some((browser, _folder, _site)) = started() else {
        return;
    };
    open(&browser, "drawn-over.html");
    let tree = snapshot(&browser);
    let checkbox = ref_on(&tree, "checkbox \"Agree to the terms\"");

    call(&browser, Call::Click { target: checkbox });

    assert!(
        sees(&browser, "Agreed to the terms"),
        "{}",
        snapshot(&browser)
    );
}

#[test]
fn a_click_on_a_button_under_a_box_the_pointer_passes_through_presses_it() {
    let Some((browser, _folder, _site)) = started() else {
        return;
    };
    open(&browser, "drawn-over.html");
    let button = ref_on(
        &find(&browser, "Button under a decoration"),
        "Button under a decoration",
    );

    call(&browser, Call::Click { target: button });

    assert!(
        sees(&browser, "Pressed the button under the decoration"),
        "{}",
        snapshot(&browser)
    );
}

#[test]
fn a_click_on_a_hidden_checkbox_slotted_under_a_web_components_box_toggles_it() {
    let Some((browser, _folder, _site)) = started() else {
        return;
    };
    open(&browser, "drawn-over.html");
    let tree = snapshot(&browser);
    let checkbox = ref_on(&tree, "checkbox \"Slotted option\"");

    call(&browser, Call::Click { target: checkbox });

    assert!(sees(&browser, "Slotted on"), "{}", snapshot(&browser));
}

#[test]
fn a_click_on_a_hidden_checkbox_its_labels_before_is_drawn_over_toggles_it() {
    let Some((browser, _folder, _site)) = started() else {
        return;
    };
    open(&browser, "drawn-over.html");
    let tree = snapshot(&browser);
    let checkbox = ref_on(&tree, "checkbox \"Drawn option\"");

    call(&browser, Call::Click { target: checkbox });

    assert!(sees(&browser, "Drawn on"), "{}", snapshot(&browser));
}

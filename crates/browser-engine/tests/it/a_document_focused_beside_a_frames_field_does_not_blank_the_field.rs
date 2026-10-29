// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The agent typed into a field in a frame, then clicked the outer page, so the outer document
//! has the focus. The person takes the browser over and presses a key: the frame's field, which
//! no longer has the focus, is not what the key blanks (#2898). Its text is the agent's, and a
//! later snapshot still shows it.
//!
//! These need a real Chrome and the right to start it and to listen on a loopback port, which a
//! sandboxed agent does not have. They run where `PURLIS_TEST_ENGINE` names a Chrome
//! executable, and on CI against the runner's own Chrome. Anywhere else each test says why it
//! did not run and passes.

#![cfg(unix)]

use crate::common;

use common::drive::{call, find, open, ref_on, snapshot};
use common::site::started;
use purlis_browser_engine::drive::{Call, Field, PersonInput, Refusal};

const AGENTS: &str = "agentwrote";

#[test]
fn a_key_with_the_outer_document_focused_never_blanks_the_field_a_frame_last_focused() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    open(&browser, "focus-frames.html");
    let field = ref_on(&find(&browser, "Frame field"), "Frame field");
    call(
        &browser,
        Call::Type {
            page: None,
            fields: vec![Field {
                into: Some(field),
                text: AGENTS.into(),
                replace: true,
            }],
        },
    );
    // The outer page's heading takes no focus of its own: a click there gives the outer
    // document the focus, and the frame's field loses it.
    let heading = ref_on(&find(&browser, "Outer page"), "Outer page");
    call(&browser, Call::Click { target: heading });

    browser.take_over();
    let aim = browser.aim().expect("the browser shows a page");
    let pressed = browser.person_input(&PersonInput::Key {
        aim,
        down: true,
        key: "x".to_owned(),
        code: "KeyX".to_owned(),
        key_code: 88,
        modifiers: 0,
    });
    assert!(
        matches!(pressed, Ok(()) | Err(Refusal::FocusUnclear)),
        "the key reached the outer page or was held: {pressed:?}"
    );
    browser.hand_back();

    let read = snapshot(&browser);
    assert!(
        read.contains(AGENTS),
        "the frame's field was not blanked for a key it never got: {read}"
    );
}

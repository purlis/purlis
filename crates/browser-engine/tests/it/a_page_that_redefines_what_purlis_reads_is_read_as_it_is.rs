// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! purlis's own scripts run in a world of their own in the page (#2138): a page that redefines
//! what they read and use is still read and acted on as its document is.
//!
//! These need a real Chrome and the right to start it and to listen on a loopback port, which a
//! sandboxed agent does not have. They run where `PURLIS_TEST_ENGINE` names a Chrome
//! executable, and on CI against the runner's own Chrome. Anywhere else each test says why it
//! did not run and passes.

#![cfg(unix)]

use crate::common;

use common::drive::{call, find, open, ref_on, sees};
use common::site::started;
use purlis_browser_engine::drive::{Call, Settled};

#[test]
fn a_page_that_says_it_is_still_loading_is_seen_to_have_loaded() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    let opened = open(&browser, "redefined.html");
    assert_eq!(opened.settled, Settled::Yes, "{opened:?}");
}

#[test]
fn a_drop_down_on_a_page_that_redefines_what_choosing_uses_is_chosen_from() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    open(&browser, "redefined.html");
    let colour = ref_on(&find(&browser, "Colour"), "combobox \"Colour\"");
    let chose = call(
        &browser,
        Call::Select {
            target: colour,
            options: vec!["Blue".into()],
        },
    );
    assert_eq!(chose.settled, Settled::Yes, "{chose:?}");
    assert!(sees(&browser, "Chose: Blue"));
}

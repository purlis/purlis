// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Through a real engine (#2745): a page whose script cut an emoji in two is read, half of the
//! pair read as U+FFFD, and the browser goes on. The engine writes such a half over DevTools as a
//! lone `\ud83d` escape, which JSON allows.
//!
//! These need a real Chrome and the right to start it and to listen on a loopback port, which a
//! sandboxed agent does not have. They run where `PURLIS_TEST_ENGINE` names a Chrome
//! executable, and on CI against the runner's own Chrome. Anywhere else each test says why it
//! did not run and passes.

#![cfg(unix)]

use crate::common;

use common::drive::{call, open, snapshot};
use common::site::started;
use purlis_browser_engine::drive::{Call, Content, Settled};

#[test]
fn a_page_whose_script_cut_an_emoji_in_two_is_read_and_the_browser_still_answers() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    let opened = open(&browser, "cut-emoji.html");
    assert_eq!(opened.settled, Settled::Yes, "{opened:?}");
    let read = snapshot(&browser);
    assert!(read.contains("Email security tips \u{fffd}"), "{read}");
    // The pages are listed, as every call answers on.
    assert!(matches!(
        call(&browser, Call::Pages).content,
        Content::Pages(_)
    ));
    // And it still answers the next page.
    open(&browser, "frames.html");
    assert!(!snapshot(&browser).is_empty());
}

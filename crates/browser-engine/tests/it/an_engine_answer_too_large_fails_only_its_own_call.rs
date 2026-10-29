// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! An answer from the engine too large to read fails only the call that asked for it (#2694),
//! through a real engine: the browser goes on answering. The read of a page's layout that marks
//! what its boxes do not show is held to a smaller bound, and a page heavy in inline text has
//! nothing marked by it.
//!
//! These need a real Chrome and the right to start it and to listen on a loopback port, which a
//! sandboxed agent does not have. They run where `PURLIS_TEST_ENGINE` names a Chrome
//! executable, and on CI against the runner's own Chrome. Anywhere else each test says why it
//! did not run and passes.

#![cfg(unix)]

use crate::common;

use common::drive::{find, next_id, open, snapshot};
use common::site::started;
use purlis_browser_engine::WebAddress;
use purlis_browser_engine::drive::{Call, Refusal, Settled};

#[test]
fn a_page_heavy_in_inline_text_is_read_and_its_layout_marks_nothing() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    assert_eq!(open(&browser, "heavy-text.html").settled, Settled::Yes);
    // Out of the list's view, it would be marked so were the layout read.
    let last = find(&browser, "Last in the list");
    assert!(last.contains("Last in the list"), "{last}");
    assert!(!last.contains("(out of view)"), "{last}");
    assert!(snapshot(&browser).contains("Shown in the list"));
}

#[test]
fn a_page_whose_tree_is_past_every_bound_fails_that_read_and_the_browser_still_answers() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    let address = WebAddress::parse("http://fixture.test/huge-label.html").unwrap();
    let opened = browser.call(&next_id(), Call::Open { address });
    let read = browser.call(
        &next_id(),
        Call::Snapshot {
            page: None,
            within: None,
            from: 1,
            lines: None,
        },
    );
    assert!(
        matches!(opened, Err(Refusal::TooLarge)) || matches!(read, Err(Refusal::TooLarge)),
        "{opened:?} / {read:?}"
    );
    // The browser still answers.
    browser
        .call(&next_id(), Call::Pages)
        .expect("the pages are listed");
    open(&browser, "frames.html");
    assert!(!snapshot(&browser).is_empty());
}

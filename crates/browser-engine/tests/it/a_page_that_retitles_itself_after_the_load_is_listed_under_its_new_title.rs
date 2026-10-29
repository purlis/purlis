// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Through a real engine (#2753): a page whose script sets its title after the load is listed,
//! and its snapshot headed, under the new title as soon as the page shows it. Chrome tells of a
//! script's new title by `Target.targetInfoChanged`, which it does not always send, so the
//! driver reads the title from the document.
//!
//! These need a real Chrome and the right to start it and to listen on a loopback port, which a
//! sandboxed agent does not have. They run where `PURLIS_TEST_ENGINE` names a Chrome
//! executable, and on CI against the runner's own Chrome. Anywhere else each test says why it
//! did not run and passes.

#![cfg(unix)]

use crate::common;

use common::drive::{call, open, sees, snapshot};
use common::site::started;
use purlis_browser_engine::drive::{Call, Content};

const RETITLED: &str = "Retitled after the load";

#[test]
fn a_page_that_retitles_itself_after_the_load_is_listed_under_its_new_title() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    open(&browser, "retitled.html");
    // The script has run once the page says so; nothing waits on the engine's word of it.
    assert!(sees(&browser, "Title set"));
    let listed = call(&browser, Call::Pages);
    let Content::Pages(pages) = &listed.content else {
        panic!("{listed:?}")
    };
    assert!(pages.iter().any(|p| p.title == RETITLED), "{pages:?}");
    assert_eq!(listed.page.map(|p| p.title).as_deref(), Some(RETITLED));
    let read = snapshot(&browser);
    assert!(
        read.lines()
            .next()
            .is_some_and(|head| head.contains(&format!("\"{RETITLED}\""))),
        "{read}"
    );
}

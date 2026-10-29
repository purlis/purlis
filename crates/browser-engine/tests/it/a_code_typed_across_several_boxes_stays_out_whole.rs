// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A code typed one character a box, each box blanked as a login fills it, is kept out of every
//! answer as a whole once the page shows it whole somewhere else (#2144).
//!
//! These need a real Chrome and the right to start it and to listen on a loopback port, which a
//! sandboxed agent does not have. They run where `PURLIS_TEST_ENGINE` names a Chrome
//! executable, and on CI against the runner's own Chrome. Anywhere else each test says why it
//! did not run and passes.

#![cfg(unix)]

use crate::common;

use std::time::Duration;

use common::drive::{call, find, open, ref_on, snapshot, text};
use common::site::started;
use purlis_browser_engine::drive::{Call, Content, Field};

const CODE: &str = "739214";

#[test]
fn a_code_typed_into_six_boxes_shows_nowhere_once_the_page_shows_it_whole() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    open(&browser, "code-boxes.html");
    // Each box is blanked, then its digit typed into it, as a login fills one.
    for (at, digit) in CODE.chars().enumerate() {
        let label = format!("Digit {}", at + 1);
        let field = ref_on(&find(&browser, &label), &label);
        browser.blank(&field).unwrap();
        let typed = call(
            &browser,
            Call::Type {
                page: None,
                fields: vec![Field {
                    into: Some(field),
                    text: digit.to_string(),
                    replace: true,
                }],
            },
        );
        let page = typed.page.expect("a typing answers its page");
        assert!(!page.title.contains(CODE), "{page:?}");
        assert!(!page.address.contains(CODE), "{page:?}");
    }

    let read = snapshot(&browser);
    let found = text(&call(
        &browser,
        Call::Find {
            page: None,
            text: Some(CODE.into()),
            role: None,
            within: None,
        },
    ));
    let waited = call(
        &browser,
        Call::WaitForText {
            page: None,
            text: CODE.into(),
            gone: false,
            within: Duration::from_millis(500),
        },
    );
    let listed = call(&browser, Call::Pages);

    assert!(read.contains("You entered (blanked), thank you"), "{read}");
    assert!(!read.contains(CODE), "{read}");
    assert!(found.contains("Nothing on the page matches."), "{found}");
    assert_eq!(waited.content, Content::Seen(false));
    let page = listed.page.expect("the current page is listed");
    assert!(!page.title.contains(CODE), "{page:?}");
    assert!(page.title.contains("(blanked)"), "{page:?}");
    assert!(!page.address.contains(CODE), "{page:?}");
}

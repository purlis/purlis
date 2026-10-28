// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Every navigation the agent asks for takes a `WebAddress`, so it can name only http and https.
//! A page can still offer other addresses: a link or a redirect to a `file:` address or to an
//! app's own scheme. Followed in a real engine, none of them opens, in the page or in a new
//! page, and the browser lists no page at such an address.
//!
//! Like every test of a real engine, this runs where `PURLIS_TEST_ENGINE` names a Chrome, and on
//! CI against the runner's own Chrome; anywhere else it says why it did not run and passes.

#![cfg(unix)]

use crate::common;

use std::time::Duration;

use common::site;
use purlis_browser_engine::WebAddress;
use purlis_browser_engine::drive::{Call, CallId, Content, Ref};

/// Whether an address is one a page may be at: a web address, an empty page (or one with no
/// address yet), or the engine's page for a load that failed.
fn web_or_blank(address: &str) -> bool {
    blank(address) || WebAddress::parse(address).is_ok() || address.starts_with("chrome-error://")
}

fn blank(address: &str) -> bool {
    address.is_empty() || address == "about:blank"
}

#[test]
fn a_link_or_redirect_a_page_offers_to_a_file_or_an_apps_own_scheme_does_not_open() {
    let Some((browser, _data, _site)) = site::started() else {
        return;
    };
    let mut n = 0;
    let mut id = || {
        n += 1;
        CallId::new(format!("call-{n}")).unwrap()
    };
    let schemes = WebAddress::parse("http://fixture.test/schemes.html").unwrap();
    browser
        .call(
            &id(),
            Call::Open {
                address: schemes.clone(),
            },
        )
        .unwrap();
    let pages = |answer: purlis_browser_engine::drive::Answer| match answer.content {
        Content::Pages(pages) => pages,
        other => panic!("pages answers pages: {other:?}"),
    };
    // The engine's first, empty page and the one opened here, which is the last listed.
    let open_at_start = pages(browser.call(&id(), Call::Pages).unwrap()).len();
    for link in [
        "File link",
        "App link",
        "Redirect to a file",
        "Redirect to an app",
        "File in a new page",
        "App in a new page",
    ] {
        let found = browser
            .call(
                &id(),
                Call::Find {
                    page: None,
                    text: Some(link.into()),
                    role: Some("link".into()),
                    within: None,
                },
            )
            .unwrap();
        let Content::Text(found) = found.content else {
            panic!("{link}: a find answers text")
        };
        let line = found
            .lines()
            .find(|l| l.contains(link))
            .unwrap_or_else(|| panic!("{link} not found in\n{found}"));
        let start = line.find('[').unwrap() + 1;
        let end = line[start..].find(']').unwrap() + start;
        let clicked = browser
            .call(
                &id(),
                Call::Click {
                    target: Ref::new(&line[start..end]),
                },
            )
            .unwrap();
        // A page the engine made for the address is empty at most: it never reached it.
        for opened in &clicked.opened {
            assert!(blank(&opened.address), "{link} opened {clicked:?}");
        }
        // Give the engine time to have done whatever it was going to do.
        std::thread::sleep(Duration::from_millis(500));
        let listed = pages(browser.call(&id(), Call::Pages).unwrap());
        for page in &listed {
            assert!(web_or_blank(&page.address), "{link}: {page:?}");
            assert!(!page.address.starts_with("file:"), "{link}: {page:?}");
            assert!(
                !page.address.contains("purlis-test-app"),
                "{link}: {page:?}"
            );
        }
        let new: Vec<_> = listed.iter().skip(open_at_start).collect();
        assert!(new.iter().all(|p| blank(&p.address)), "{link}: {listed:?}");
        // Nothing of the machine's own files is on the current page.
        let shown = browser
            .call(
                &id(),
                Call::Snapshot {
                    page: None,
                    within: None,
                    from: 1,
                    lines: None,
                },
            )
            .unwrap();
        let Content::Text(shown) = shown.content else {
            panic!("{link}: a snapshot answers text")
        };
        assert!(!shown.contains("localhost"), "{link}: {shown}");
        browser
            .call(
                &id(),
                Call::SwitchPage {
                    page: listed[open_at_start - 1].id.clone(),
                },
            )
            .unwrap();
        browser
            .call(
                &id(),
                Call::Navigate {
                    page: None,
                    address: schemes.clone(),
                },
            )
            .unwrap();
    }
}

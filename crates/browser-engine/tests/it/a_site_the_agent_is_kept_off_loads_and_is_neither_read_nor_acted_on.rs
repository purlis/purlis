// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A browser told to keep the agent off a site (#5152, the person's ruling of 2026-11-04: the
//! site a login signs in on, outside a sign-in) loads that site's pages and frames as any other,
//! and the agent neither reads them nor acts on them: a frame of it inside another page is left
//! out of every read and painted over, a page of it refuses every read and act with its site
//! named, and says so where it shows a sign-in form, an address on it is not opened, and what a
//! page of it sends to it while the agent drives is not sent.
//!
//! These need a real Chrome and a loopback port for the fixture site, which a sandboxed agent
//! does not have: they run where `PURLIS_TEST_ENGINE` names a Chrome executable, and on CI.

#![cfg(unix)]

use crate::common;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use common::drive::{call, find, next_id, open, red_pixels, ref_on, screenshot, snapshot};
use common::site::started_judged;
use purlis_browser_engine::WebAddress;
use purlis_browser_engine::drive::{
    Browser, Call, Content, Field, Gate, Judge, KeptOff, Outbound, Refusal,
};

const PROVIDER: &str = "auth.fixture.test:80";
const FRAME_TEXT: &str = "Provider frame text";

/// The agent is kept off auth.fixture.test unless `signing_in`.
fn kept_off(signing_in: &Arc<AtomicBool>) -> KeptOff {
    let signing_in = Arc::clone(signing_in);
    Arc::new(move |address: &str| {
        let on = address == "http://auth.fixture.test"
            || address.starts_with("http://auth.fixture.test/");
        (on && !signing_in.load(Ordering::SeqCst)).then(|| PROVIDER.to_owned())
    })
}

fn going() -> Judge {
    Arc::new(|_: &Outbound| Gate::Go)
}

fn refused(browser: &Browser, made: Call) -> Refusal {
    let what = format!("{made:?}");
    match browser.call(&next_id(), made) {
        Ok(answer) => panic!("{what} answered: {answer:?}"),
        Err(why) => why,
    }
}

fn kept_off_refusal(asks_to_sign_in: bool) -> Refusal {
    Refusal::KeptOff {
        site: PROVIDER.to_owned(),
        asks_to_sign_in,
    }
}

/// The current page's title, as the pages are listed.
fn current_title(browser: &Browser) -> String {
    match call(browser, Call::Pages).content {
        Content::Pages(pages) => pages
            .into_iter()
            .find(|page| page.current)
            .map(|page| page.title)
            .unwrap_or_default(),
        other => panic!("pages: {other:?}"),
    }
}

/// Sends the current page back to the app's page.
fn home(browser: &Browser) {
    call(
        browser,
        Call::Navigate {
            page: None,
            address: WebAddress::parse("http://fixture.test/sso-home.html").unwrap(),
        },
    );
}

fn click(browser: &Browser, words: &str) {
    let link = ref_on(&find(browser, words), words);
    call(browser, Call::Click { target: link });
}

#[test]
fn a_site_the_agent_is_kept_off_loads_and_is_neither_read_nor_acted_on() {
    let Some((browser, _folder, _site)) = started_judged(going()) else {
        return;
    };
    let signing_in = Arc::new(AtomicBool::new(true));
    browser.keep_off(kept_off(&signing_in));

    // While signing in, the provider's frame inside the app's page is read and drawn.
    open(&browser, "sso-home.html");
    let start = Instant::now();
    while !snapshot(&browser).contains(FRAME_TEXT) {
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "the provider's frame never loaded"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(red_pixels(&screenshot(&browser)) > 0, "it is drawn");

    // Outside a sign-in it still loads, and is left out of every read and painted over.
    signing_in.store(false, Ordering::SeqCst);
    let read = snapshot(&browser);
    assert!(read.contains("Signed in to the fixture app"), "{read}");
    assert!(!read.contains(FRAME_TEXT), "{read}");
    assert!(!find(&browser, FRAME_TEXT).contains('['));
    assert_eq!(red_pixels(&screenshot(&browser)), 0, "painted over");

    // An address on it is not opened.
    let account = WebAddress::parse("http://auth.fixture.test/account.html").unwrap();
    assert_eq!(
        refused(
            &browser,
            Call::Open {
                address: account.clone()
            }
        ),
        kept_off_refusal(false)
    );
    assert_eq!(
        refused(
            &browser,
            Call::Navigate {
                page: None,
                address: account
            }
        ),
        kept_off_refusal(false)
    );

    // The app's own link to it is followed; on its page every read and act is refused.
    click(&browser, "Manage your account");
    let page = |made: Call| refused(&browser, made);
    for made in [
        Call::Snapshot {
            page: None,
            within: None,
            from: 1,
            lines: None,
        },
        Call::Find {
            page: None,
            text: Some("Allow".into()),
            role: None,
            within: None,
        },
        Call::Changes { page: None },
        Call::Screenshot { page: None },
        Call::WaitForText {
            page: None,
            text: "Your provider account".into(),
            gone: false,
            within: Duration::from_secs(1),
        },
        Call::Press {
            page: None,
            keys: vec!["Enter".into()],
        },
        Call::Type {
            page: None,
            fields: vec![Field {
                into: None,
                text: "x".into(),
                replace: false,
            }],
        },
        Call::Scroll {
            page: None,
            at: None,
            x: 0.0,
            y: 100.0,
        },
    ] {
        assert_eq!(page(made), kept_off_refusal(false));
    }
    assert_ne!(current_title(&browser), "Consented", "nothing was sent");
    home(&browser);

    // Where its page shows its sign-in form, the refusal says so.
    click(&browser, "Sign in again");
    assert_eq!(
        page(Call::Snapshot {
            page: None,
            within: None,
            from: 1,
            lines: None,
        }),
        kept_off_refusal(true)
    );
    home(&browser);

    // A page of it that sends to it by itself as it comes, while the agent drives, sends
    // nothing; while signing in, it does.
    click(&browser, "Approve the other app");
    std::thread::sleep(Duration::from_secs(2));
    assert_ne!(current_title(&browser), "Consented", "nothing was sent");
    home(&browser);
    signing_in.store(true, Ordering::SeqCst);
    click(&browser, "Approve the other app");
    let start = Instant::now();
    while current_title(&browser) != "Consented" {
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "while signing in, the page's own send went"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

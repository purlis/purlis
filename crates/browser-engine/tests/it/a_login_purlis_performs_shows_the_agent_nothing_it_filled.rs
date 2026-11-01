// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A login purlis performs (#2016), through a real engine on the fixture site: purlis fills the
//! sign-in form on its own origin and submits it, and nothing it filled reaches the agent, not
//! when the page lands on an address that carries it, nor when a show-password toggle shows it
//! as text.
//!
//! These need a real Chrome and the right to start it and to listen on a loopback port (the
//! site's stand-in reach proxy), which a sandboxed agent does not have. They run where
//! `PURLIS_TEST_ENGINE` names a Chrome executable, and on CI against the runner's own Chrome.
//! Anywhere else each test says why it did not run and passes.

#![cfg(unix)]

use crate::common;

use common::drive::{call, find, open, red_pixels, ref_on, screenshot, snapshot};
use common::site::started;
use purlis_browser_engine::WebAddress;
use purlis_browser_engine::drive::{Call, Content, Driver, Login, Refusal};

const USER: &str = "fixture-person";
const PASSWORD: &str = "fixture-word-7";

fn login_at(origin: &str) -> Login {
    login_on("login.html", origin)
}

fn login_on(page: &str, origin: &str) -> Login {
    Login {
        address: WebAddress::parse(&format!("http://fixture.test/{page}")).unwrap(),
        origin: origin.to_owned(),
        user: Some(USER.to_owned().into()),
        password: PASSWORD.to_owned().into(),
        only_if_asked: false,
        suggested: false,
    }
}

/// How a login ended, in words fixed in this file: what a login answers may carry what its page
/// showed, so a failing test names only the kind of answer, never its words.
fn ended_as<T>(outcome: &Result<T, Refusal>) -> &'static str {
    match outcome {
        Ok(_) => "it answered",
        Err(Refusal::NotLoggedIn(_)) => "not logged in",
        Err(Refusal::Ended) => "the browser ended",
        Err(_) => "another refusal",
    }
}

/// Whether `text` holds either value, in any case.
fn holds_a_value(text: &str) -> bool {
    let text = text.to_lowercase();
    text.contains(USER) || text.contains(PASSWORD)
}

#[test]
fn a_declared_login_goes_through_and_no_value_reaches_the_agent() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    open(&browser, "frames.html");
    let ended = browser
        .log_in(&login_at("http://fixture.test"), &|| Ok(()))
        .expect("the login runs");
    let answer = ended.outcome.expect("the login goes through");
    assert_eq!(ended.driver, Driver::Agent);
    assert_eq!(
        answer.note.as_deref(),
        Some(
            "the password field is gone from http://fixture.test since purlis submitted the login"
        )
    );
    let page = answer.page.expect("the page it left");
    // The page it landed on carries the values in its address: neither is shown.
    assert!(
        !holds_a_value(&page.address),
        "the address the login landed on shows a value"
    );
    let shown = snapshot(&browser);
    assert!(shown.contains("Account"), "{shown}");
    assert!(!holds_a_value(&shown), "{shown}");
    // The page's show-password toggle shows the password as text: still nothing.
    let show = ref_on(&find(&browser, "Show password"), "Show password");
    call(&browser, Call::Click { target: show });
    let shown = snapshot(&browser);
    assert!(!holds_a_value(&shown), "{shown}");
    assert!(
        !holds_a_value(&find(&browser, "fixture")),
        "nothing found by the value"
    );
    let seen = call(
        &browser,
        Call::WaitForText {
            page: None,
            text: PASSWORD.into(),
            gone: false,
            within: std::time::Duration::from_secs(2),
        },
    );
    assert_eq!(seen.content, Content::Seen(false));
    let pages = call(&browser, Call::Pages);
    assert!(!holds_a_value(&format!("{pages:?}")), "{pages:?}");
    // What the page draws in red is the values: a screenshot paints every line of them over.
    assert_eq!(red_pixels(&screenshot(&browser)), 0);
}

#[test]
fn a_login_that_does_not_go_through_is_cleared_and_its_toggle_shows_nothing() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    // A form whose page keeps it: the password field never goes.
    let ended = browser
        .log_in(
            &login_on("login-stays.html", "http://fixture.test"),
            &|| Ok(()),
        )
        .expect("the login runs");
    match ended.outcome {
        Err(Refusal::NotLoggedIn(why)) => {
            assert!(why.contains("did not go through"), "{why}");
            assert!(!holds_a_value(&why), "{why}");
        }
        other => panic!("refused otherwise: {}", ended_as(&other)),
    }
    assert_eq!(ended.driver, Driver::Agent);
    // What purlis filled is cleared: the page says its field holds nothing.
    let shown = snapshot(&browser);
    assert!(
        shown.contains("the password field holds 0 characters"),
        "{shown}"
    );
    let show = ref_on(&find(&browser, "Show password"), "Show password");
    call(&browser, Call::Click { target: show });
    let shown = snapshot(&browser);
    assert!(!holds_a_value(&shown), "{shown}");
}

#[test]
fn a_login_on_another_origin_fills_nothing() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    let ended = browser
        .log_in(&login_at("https://shop.example.test"), &|| Ok(()))
        .expect("the login runs");
    match ended.outcome {
        Err(Refusal::NotLoggedIn(why)) => assert!(why.contains("nothing was filled"), "{why}"),
        other => panic!("refused otherwise: {}", ended_as(&other)),
    }
    let shown = snapshot(&browser);
    assert!(shown.contains("Sign in"), "{shown}");
    assert!(!holds_a_value(&shown), "{shown}");
}

/// purlis's login scripts run in its own world (#2613): a page that redefines what finding and
/// focusing a field uses is still filled where its form is.
#[test]
fn a_login_on_a_page_that_redefines_finding_and_focusing_a_field_goes_through() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    let ended = browser
        .log_in(
            &login_on("login-redefined.html", "http://fixture.test"),
            &|| Ok(()),
        )
        .expect("the login runs");
    let answer = ended.outcome.expect("the login goes through");
    assert_eq!(ended.driver, Driver::Agent);
    let page = answer.page.expect("the page it left");
    assert!(
        page.address.contains("account.html"),
        "the login landed on the account page"
    );
    let shown = snapshot(&browser);
    assert!(shown.contains("Account"), "the account page shows");
    assert!(!holds_a_value(&shown), "the account page shows no value");
}

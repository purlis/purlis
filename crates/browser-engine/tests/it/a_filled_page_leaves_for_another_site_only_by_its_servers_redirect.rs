// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! How a page holding a secret purlis filled may leave for another site (#5230; the person's
//! rulings SF-14 and SF-15 on #5096), on a real engine and two sites: the sign-in's steps of the
//! ticket, and each thing that stays refused. The fixture site stands in for the browser's reach
//! proxy, held as a fill's holder holds that ([`Site::held_to`]), so what reached a site is what
//! its server was sent.
//!
//! These need a real Chrome and a loopback port, which a sandboxed agent does not have: they run
//! where `PURLIS_TEST_ENGINE` names a Chrome executable, and on CI.

#![cfg(unix)]

use crate::common;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::drive::{call, find, next_id, ref_on, sees};
use common::site::{Held, Site, started_judged};
use purlis_browser_engine::WebAddress;
use purlis_browser_engine::drive::{
    Answer, Browser, Call, FillHold, Gate, Judge, Leaving, Outbound, Ref, SecretFill,
};

const VALUE: &str = "fixture-key-5230";
const HELD_SITE: &str = "auth.fixture.test";
const OTHER: &str = "other.test";

/// How the stand-in's hold ends, as the real one's does: a moment after the page is gone.
const THE_HOLD_ENDS_AFTER: Duration = Duration::from_millis(600);

/// A holder as the core's is, on the fixture site in place of a reach proxy.
struct Holder {
    held_to: Held,
    /// The site the key is also approved for (SF-15).
    also: Option<&'static str>,
    /// The sites the person allowed.
    answered: Mutex<Vec<String>>,
    /// Told in order: "held", "released", and each site asked about.
    told: Mutex<Vec<String>>,
}

impl Holder {
    fn on(site: &Site, also: Option<&'static str>) -> Arc<Self> {
        Arc::new(Self {
            held_to: site.held_to(),
            also,
            answered: Mutex::default(),
            told: Mutex::default(),
        })
    }

    fn said(&self) -> Vec<String> {
        self.told.lock().unwrap().clone()
    }

    fn released_within_ten_seconds(&self) {
        let start = Instant::now();
        while !self.said().iter().any(|one| one == "released") {
            assert!(
                start.elapsed() < Duration::from_secs(10),
                "told {:?}, never released",
                self.said()
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

fn host_of(address: &str) -> String {
    address
        .split_once("://")
        .map_or(address, |(_, rest)| rest)
        .split(['/', ':', '?', '#'])
        .next()
        .unwrap_or_default()
        .to_owned()
}

impl FillHold for Holder {
    fn hold(&self) -> Result<(), String> {
        let mut held = self.held_to.lock().unwrap();
        held.push(HELD_SITE.to_owned());
        held.extend(self.also.map(str::to_owned));
        self.told.lock().unwrap().push("held".to_owned());
        Ok(())
    }

    fn released(&self) {
        self.released_then(Box::new(|| {}));
    }

    fn released_then(&self, then: Box<dyn FnOnce() + Send>) {
        self.told.lock().unwrap().push("released".to_owned());
        let held_to = Arc::clone(&self.held_to);
        std::thread::spawn(move || {
            std::thread::sleep(THE_HOLD_ENDS_AFTER);
            held_to.lock().unwrap().clear();
            then();
        });
    }

    fn may_leave_for(&self, address: &str, been_on: bool) -> Leaving {
        let host = host_of(address);
        if been_on || self.answered.lock().unwrap().contains(&host) {
            return Leaving::Go;
        }
        self.told.lock().unwrap().push(format!("asked {host}"));
        Leaving::Ask { site: host }
    }

    fn approved_for(&self, address: &str) -> bool {
        self.also.is_some_and(|also| host_of(address) == also)
    }
}

/// A browser whose judge sends everything: the fixture site decides, as the proxy would.
fn browser() -> Option<(Browser, tempfile::TempDir, Site)> {
    let judge: Judge = Arc::new(|_: &Outbound| Gate::Go);
    started_judged(judge)
}

fn opened(browser: &Browser, address: &str) -> Answer {
    let address = WebAddress::parse(address).unwrap();
    call(browser, Call::Open { address })
}

/// The address that sends the browser to `to` by its server's redirect.
fn redirecting_to(from: &str, to: &str) -> String {
    format!("http://{from}/redirect?to={to}")
}

/// Opens the sign-in form as an app sends a browser to it: by `app`'s redirect, so the browser was
/// on `app` though it never showed a page of it.
fn the_app_sends_it_to_sign_in(browser: &Browser, app: &str) {
    opened(
        browser,
        &redirecting_to(app, "http://auth.fixture.test/held-form.html"),
    );
    assert!(sees(browser, "Sign in"), "the form is open");
}

fn ref_of(browser: &Browser, line: &str) -> Ref {
    let label = line.split('"').nth(1).unwrap_or(line);
    let found = find(browser, label);
    ref_on(&found, line)
}

fn filled(browser: &Browser, holder: &Arc<Holder>) {
    let fill = SecretFill {
        target: ref_of(browser, "textbox \"API key\""),
        origin: "http://auth.fixture.test".to_owned(),
        key: "the-api-key".to_owned(),
        value: VALUE.to_owned().into(),
        multi_line: false,
    };
    let filled = browser.fill_secret(
        browser.epoch(),
        &fill,
        Arc::clone(holder) as Arc<dyn FillHold>,
    );
    // A refusal may carry what its page showed: only that there was one is said.
    assert!(filled.is_ok(), "the key was not filled");
}

fn click(browser: &Browser, line: &str) -> Answer {
    let target = ref_of(browser, line);
    call(browser, Call::Click { target })
}

/// The steps of #5230: the app sends the browser to sign in, the form is filled and sent, and the
/// server's redirect takes the page back to the app by itself, its query whole.
#[test]
fn a_sign_in_redirected_to_a_site_the_browser_was_on_finishes_by_itself() {
    let Some((browser, _data, site)) = browser() else {
        return;
    };
    the_app_sends_it_to_sign_in(&browser, OTHER);
    let holder = Holder::on(&site, None);
    filled(&browser, &holder);

    click(&browser, "button \"Sign in\"");

    assert!(sees(&browser, "Landed on the other site"), "it landed");
    assert_eq!(holder.said(), ["held", "released"], "nobody was asked");
    assert!(
        site.was_sent("GET other.test/landed.html"),
        "the server named it, and it went"
    );
}

/// A site the browser hasn't been on asks once; the page stays, and goes on once allowed.
#[test]
fn a_site_the_browser_has_not_been_on_asks_and_goes_on_once_allowed() {
    let Some((browser, _data, site)) = browser() else {
        return;
    };
    the_app_sends_it_to_sign_in(&browser, OTHER);
    let holder = Holder::on(&site, None);
    filled(&browser, &holder);

    let answer = click(&browser, "button \"Sign in elsewhere\"");

    assert_eq!(answer.asked, ["third.test"], "the answer says it asked");
    assert_eq!(holder.said(), ["held", "asked third.test"]);
    assert!(sees(&browser, "Sign in"), "the page stays");
    assert!(!site.heard_from("third.test"), "nothing went there");

    holder
        .answered
        .lock()
        .unwrap()
        .push("third.test".to_owned());
    browser.reach_changed();

    assert!(sees(&browser, "Landed on the third site"), "it went on");
    holder.released_within_ten_seconds();
}

/// A link, a link the fill's site redirects, a script and the chat's navigate don't take it there:
/// none began with a form. The page stays, so no history entry of the engine's own page holds what
/// it tried to send, and the hold lives on.
#[test]
fn links_scripts_and_the_chats_navigate_stay_refused() {
    let moves: [(&str, fn(&Browser)); 4] = [
        ("a link", |browser| {
            click(browser, "link \"Leave by a link\"");
        }),
        ("a redirected link", |browser| {
            click(browser, "link \"Leave by a redirected link\"");
        }),
        ("a script", |browser| {
            click(browser, "button \"Leave by a script\"");
        }),
        ("the chat's navigate", |browser| {
            let address = WebAddress::parse("http://other.test/landed.html").unwrap();
            let _ = browser.call(
                &next_id(),
                Call::Navigate {
                    page: None,
                    address,
                },
            );
        }),
    ];
    for (name, moved) in moves {
        let Some((browser, _data, site)) = browser() else {
            return;
        };
        the_app_sends_it_to_sign_in(&browser, OTHER);
        let before = site.sent_count("other.test");
        let holder = Holder::on(&site, None);
        filled(&browser, &holder);

        moved(&browser);
        // Whatever it was to do has been done by a moment after it answered.
        std::thread::sleep(Duration::from_millis(800));

        assert_eq!(
            site.sent_count("other.test"),
            before,
            "{name} reached the other site"
        );
        assert!(sees(&browser, "API key"), "{name}: the page stays");
        assert_eq!(holder.said(), ["held"], "{name}: the hold lives on");
    }
}

/// Everything the page sends the destination, while it is being redirected there and as it goes,
/// is refused: the destination hears the redirect alone, and only once the page is gone.
#[test]
fn what_a_script_sends_the_destination_as_the_page_leaves_is_refused() {
    let Some((browser, _data, site)) = browser() else {
        return;
    };
    the_app_sends_it_to_sign_in(&browser, OTHER);
    let holder = Holder::on(&site, None);
    filled(&browser, &holder);
    click(&browser, "button \"Send from a script\"");
    std::thread::sleep(Duration::from_millis(2000));

    click(&browser, "button \"Sign in\"");

    assert!(sees(&browser, "Landed on the other site"), "it landed");
    // What the old page set going has had its moment.
    std::thread::sleep(Duration::from_millis(500));
    for sent in [
        "GET other.test/from-script",
        "GET other.test/from-an-image",
        "POST other.test/from-a-beacon",
    ] {
        assert!(!site.was_sent(sent), "{sent} reached the other site");
    }
}

/// A redirect whose address holds the value is refused, plainly or URL-encoded and in a fragment
/// too, and the page stays.
#[test]
fn a_redirect_whose_address_holds_the_value_is_refused() {
    for line in [
        "button \"Sign in with the value\"",
        "button \"Sign in with the value encoded\"",
    ] {
        let Some((browser, _data, site)) = browser() else {
            return;
        };
        the_app_sends_it_to_sign_in(&browser, OTHER);
        let before = site.sent_count("other.test");
        let holder = Holder::on(&site, None);
        filled(&browser, &holder);

        let answer = click(&browser, line);

        let note = answer.note.unwrap_or_default();
        assert!(note.contains("holds the secret"), "{line}: {note}");
        assert!(!note.contains(VALUE), "{line}: {note}");
        assert_eq!(holder.said(), ["held"], "{line}: the hold lives on");
        assert!(sees(&browser, "Sign in"), "{line}: the page stays");
        assert_eq!(site.sent_count("other.test"), before, "{line}");
    }
}

/// A form posted straight to another site is refused where the key isn't approved there: the
/// answer and the page the person sees name it and say how, and nothing holds the browser after.
#[test]
fn a_form_posted_to_a_site_the_key_is_not_approved_for_is_refused_and_says_how() {
    let Some((browser, _data, site)) = browser() else {
        return;
    };
    the_app_sends_it_to_sign_in(&browser, "third.test");
    let holder = Holder::on(&site, None);
    filled(&browser, &holder);

    let answer = click(&browser, "button \"Post elsewhere\"");

    let note = answer.note.unwrap_or_default();
    assert!(note.contains("http://other.test"), "{note}");
    assert!(note.contains("secret-fill line"), "{note}");
    assert!(sees(&browser, "Form not sent"), "the person sees it");
    assert!(sees(&browser, "http://other.test"), "named on the page");
    assert!(!site.heard_from(OTHER), "nothing was sent");
    holder.released_within_ten_seconds();
}

/// Where the key is approved there too, the form goes.
#[test]
fn a_form_posted_to_a_site_the_key_is_approved_for_goes() {
    let Some((browser, _data, site)) = browser() else {
        return;
    };
    the_app_sends_it_to_sign_in(&browser, "third.test");
    let holder = Holder::on(&site, Some(OTHER));
    filled(&browser, &holder);

    click(&browser, "button \"Post elsewhere\"");

    let start = Instant::now();
    while !site.was_sent("POST other.test/posted.html") {
        assert!(start.elapsed() < Duration::from_secs(10), "it was not sent");
        std::thread::sleep(Duration::from_millis(50));
    }
    holder.released_within_ten_seconds();
}

/// A load on the fill's own site that ends on the engine's own page holds no value, and the browser
/// isn't stuck.
#[test]
fn an_error_page_ends_the_hold() {
    let Some((browser, _data, site)) = browser() else {
        return;
    };
    the_app_sends_it_to_sign_in(&browser, OTHER);
    let holder = Holder::on(&site, None);
    filled(&browser, &holder);

    click(&browser, "link \"Go to a page that fails\"");

    holder.released_within_ten_seconds();
    let start = Instant::now();
    while !site.held_to().lock().unwrap().is_empty() {
        assert!(start.elapsed() < Duration::from_secs(5), "it is still held");
        std::thread::sleep(Duration::from_millis(50));
    }
    opened(&browser, "http://other.test/landed.html");
    assert!(
        sees(&browser, "Landed on the other site"),
        "reaches as before"
    );
}

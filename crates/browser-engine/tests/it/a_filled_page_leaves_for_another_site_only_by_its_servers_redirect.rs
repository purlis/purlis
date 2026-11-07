// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! How a page holding a secret purlis filled may leave for another site (#5230; the person's
//! rulings SF-14 and SF-15 on #5096), on a real engine and two sites: the sign-in's steps of the
//! ticket, each thing that stays refused, and the windows related to the page. The fixture site stands in for the browser's reach
//! proxy, held as a fill's holder holds that ([`Site::held_to`]), so what reached a site is what
//! its server was sent.
//!
//! These need a real Chrome and a loopback port, which a sandboxed agent does not have: they run
//! where `PURLIS_TEST_ENGINE` names a Chrome executable, and on CI.

#![cfg(unix)]

use crate::common;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::drive::{call, find, next_id, ref_on, sees, snapshot};
use common::site::{Held, Site, started_judged};
use purlis_browser_engine::WebAddress;
use purlis_browser_engine::drive::{
    Answer, Browser, Call, Content, FillHold, Gate, Judge, Leaving, Outbound, PageId, Ref,
    SecretFill,
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

    fn may_leave_for(&self, address: &str, chain_began: bool) -> Leaving {
        let host = host_of(address);
        if chain_began || self.answered.lock().unwrap().contains(&host) {
            return Leaving::Go;
        }
        self.told.lock().unwrap().push(format!("asked {host}"));
        Leaving::Ask { site: host }
    }

    fn approved_for(&self, address: &str) -> bool {
        self.also.is_some_and(|also| host_of(address) == also)
    }
}

/// Something a page or the chat does to leave, and what its answer notes.
type Moved = fn(&Browser) -> Option<String>;

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

/// Opens the sign-in form as an app sends a browser to it: by `app`'s redirect, so the chain
/// leading to it began on `app`, though it never showed a page of it.
fn the_app_sends_it_to_sign_in(browser: &Browser, app: &str) {
    opened(
        browser,
        &redirecting_to(app, "http://auth.fixture.test/held-form.html"),
    );
    assert!(sees(browser, "Sign in"), "the form is open");
}

fn ref_of(browser: &Browser, line: &str) -> Ref {
    let label = line.split('"').nth(1).unwrap_or(line);
    let start = Instant::now();
    loop {
        let found = find(browser, label);
        if found.lines().any(|l| l.contains(line) && l.contains('[')) {
            return ref_on(&found, line);
        }
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "{label} never showed: {found}"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// The key filled into the field `label` names, its value `VALUE`.
fn filled_in(browser: &Browser, holder: &Arc<Holder>, label: &str) {
    filled_with(browser, holder, label, VALUE);
}

fn filled(browser: &Browser, holder: &Arc<Holder>) {
    filled_in(browser, holder, "API key");
}

fn filled_with(browser: &Browser, holder: &Arc<Holder>, label: &str, value: &str) {
    let fill = SecretFill {
        target: ref_of(browser, &format!("textbox \"{label}\"")),
        origin: "http://auth.fixture.test".to_owned(),
        key: "the-api-key".to_owned(),
        value: value.to_owned().into(),
        multi_line: false,
        under_a_grant: false,
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
fn a_sign_in_redirected_to_the_site_its_chain_began_from_finishes_by_itself() {
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
    assert!(
        sees(&browser, "Named nothing"),
        "nothing of the window went"
    );
}

/// Any other site asks once; the page stays, and goes on once allowed.
#[test]
fn a_site_its_chain_did_not_begin_from_asks_and_goes_on_once_allowed() {
    let Some((browser, _data, site)) = browser() else {
        return;
    };
    the_app_sends_it_to_sign_in(&browser, OTHER);
    let holder = Holder::on(&site, None);
    filled_in(&browser, &holder, "Key elsewhere");

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

/// A site a page moved on to by itself, while the chat's call waited on it, began no chain.
#[test]
fn a_site_a_page_moved_to_by_itself_still_asks() {
    let Some((browser, _data, site)) = browser() else {
        return;
    };
    opened(&browser, "http://other.test/moves-on.html");
    assert!(sees(&browser, "Sign in"), "the form is open");
    let holder = Holder::on(&site, None);
    filled_in(&browser, &holder, "Key elsewhere");

    let answer = click(&browser, "button \"Sign in elsewhere\"");

    assert_eq!(answer.asked, ["third.test"], "the answer says it asked");
}

/// A link, a link the fill's site redirects, a script and the chat's navigate don't take it there:
/// none began the form the field was filled in. The page stays, and the hold lives on.
#[test]
fn links_scripts_and_the_chats_navigate_stay_refused() {
    let moves: [(&str, Moved); 6] = [
        ("a link", |browser| {
            click(browser, "link \"Leave by a link\"").note
        }),
        ("a redirected link", |browser| {
            click(browser, "link \"Leave by a redirected link\"").note
        }),
        ("a script", |browser| {
            click(browser, "button \"Leave by a script\"").note
        }),
        ("the said page", |browser| {
            click(browser, "link \"Leave for the said page\"").note
        }),
        ("a form a script made", |browser| {
            click(browser, "button \"Leave by a scripted form\"").note
        }),
        ("the chat's navigate", |browser| {
            let address = WebAddress::parse("http://other.test/landed.html").unwrap();
            // The core refuses this one in words of its own before the driver hears of it: here
            // the driver's refusal is what is left of it.
            browser
                .call(
                    &next_id(),
                    Call::Navigate {
                        page: None,
                        address,
                    },
                )
                .ok()
                .and_then(|answer| answer.note)
        }),
    ];
    let mut went = Vec::new();
    for (name, moved) in moves {
        let Some((browser, _data, site)) = browser() else {
            return;
        };
        the_app_sends_it_to_sign_in(&browser, OTHER);
        let before = site.sent_count("other.test");
        let holder = Holder::on(&site, None);
        filled(&browser, &holder);

        let note = moved(&browser).unwrap_or_default();
        // Whatever it was to do has been done by a moment after it answered.
        std::thread::sleep(Duration::from_millis(800));
        // Each is said, so one that goes doesn't hide the next.
        if !note.contains("did not load")
            || site.sent_count("other.test") != before
            || holder.said() != ["held"]
            || !sees(&browser, "API key")
        {
            went.push(name);
        }
    }
    assert!(went.is_empty(), "not refused: {went:?}");
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
///
/// The redirect is judged on a thread of its own, so the refusal's note may reach the click's
/// answer or, where the click answered first, the very next one; it must reach one of them. What
/// was refused is checked before the note, so a run that fails says which it was: nothing left
/// and the note was late, or the page went.
#[test]
fn a_redirect_whose_address_holds_the_value_is_refused() {
    for (label, line) in [
        ("Key named", "button \"Sign in named\""),
        ("Key encoded", "button \"Sign in encoded\""),
    ] {
        let Some((browser, _data, site)) = browser() else {
            return;
        };
        the_app_sends_it_to_sign_in(&browser, OTHER);
        let before = site.sent_count(OTHER);
        let holder = Holder::on(&site, None);
        filled_in(&browser, &holder, label);

        let clicked_at = Instant::now();
        let clicked = click(&browser, line);
        let answered_after = clicked_at.elapsed();
        let next = call(
            &browser,
            Call::WaitForText {
                page: None,
                text: "Sign in".into(),
                gone: false,
                within: Duration::from_secs(10),
            },
        );
        let seen = format!(
            "{line}: the click answered after {answered_after:?} with note {:?} on {:?}; the next \
             answer's note {:?} on {:?}; {OTHER} was sent {:?}; the holder was told {:?}",
            clicked.note,
            clicked.page.as_ref().map(|page| &page.address),
            next.note,
            next.page.as_ref().map(|page| &page.address),
            site.sent_to(OTHER),
            holder.said(),
        );

        assert_eq!(site.sent_count(OTHER), before, "nothing reached it: {seen}");
        assert_eq!(holder.said(), ["held"], "the hold lives on: {seen}");
        assert_eq!(next.content, Content::Seen(true), "the page stays: {seen}");
        let notes = [&clicked.note, &next.note];
        assert!(
            notes
                .iter()
                .filter_map(|note| note.as_deref())
                .any(|note| note.contains("holds the secret")),
            "the refusal is said: {seen}"
        );
        assert!(
            notes
                .iter()
                .filter_map(|note| note.as_deref())
                .all(|note| !note.contains(VALUE)),
            "the value is not said: {seen}"
        );
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
    filled_in(&browser, &holder, "Key posted");

    let answer = click(&browser, "button \"Post elsewhere\"");

    let note = answer.note.unwrap_or_default();
    assert!(note.contains("http://other.test"), "{note}");
    assert!(note.contains("only over https"), "{note}");
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
    filled_in(&browser, &holder, "Key posted");

    click(&browser, "button \"Post elsewhere\"");

    let start = Instant::now();
    while !site.was_sent("POST other.test/posted.html") {
        assert!(start.elapsed() < Duration::from_secs(10), "it was not sent");
        std::thread::sleep(Duration::from_millis(50));
    }
    holder.released_within_ten_seconds();
}

/// A load that fails ends the hold, and the page the chat reads then is purlis's own.
#[test]
fn a_load_that_fails_ends_the_hold_and_shows_purlis_s_own_page() {
    let Some((browser, _data, site)) = browser() else {
        return;
    };
    the_app_sends_it_to_sign_in(&browser, OTHER);
    let holder = Holder::on(&site, None);
    filled_with(&browser, &holder, "Key failing", "p@ss+w/rd!");

    click(&browser, "button \"Send and fail\"");

    holder.released_within_ten_seconds();
    let shown = sees(&browser, "Page not loaded");
    let read = snapshot(&browser);
    assert!(shown, "purlis's own page: {} lines", read.lines().count());
    assert!(
        !read.contains("didn't send") && !read.contains("ERR_"),
        "{read}"
    );
    assert!(!read.contains("hangs-up"), "{read}");
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

/// A value with characters an address writes encoded.
const RESERVED: &str = "p@ss+w/rd!";

/// Whether `read` holds [`RESERVED`] or what its query was, as it is or as an address writes it.
fn holds_the_value(read: &str) -> bool {
    let lower = read.to_lowercase();
    ["p@ss", "w/rd", "p%40ss", "%2bw%2frd", "key="]
        .iter()
        .any(|part| lower.contains(part))
}

/// Everything the chat can read of the browser now: the page, what it finds of the value's parts,
/// and every page's address and title.
fn every_read(browser: &Browser) -> String {
    let mut read = snapshot(browser);
    for words in ["ss", "rd", "key", "other.test"] {
        read.push_str(&find(browser, words));
    }
    read.push_str(&format!("{:?}", call(browser, Call::Pages)));
    read
}

/// A form sent by GET to another site, its query holding the value, reaches nothing: not as it is
/// sent, and not from the history the person goes back through once the hold is over.
#[test]
fn a_form_sent_by_get_to_another_site_is_never_sent_later() {
    for (label, line) in [
        ("Key sent elsewhere", "button \"Send elsewhere\""),
        ("Key sent over https", "button \"Send over https\""),
    ] {
        let Some((browser, _data, site)) = browser() else {
            return;
        };
        the_app_sends_it_to_sign_in(&browser, "third.test");
        let holder = Holder::on(&site, None);
        filled_with(&browser, &holder, label, RESERVED);

        let clicked = click(&browser, line);
        // Whatever it was to do, and the hold's end where it ends, have had their moment.
        std::thread::sleep(THE_HOLD_ENDS_AFTER * 3);
        let back = call(&browser, Call::Back { page: None });
        std::thread::sleep(Duration::from_millis(800));
        let read = every_read(&browser);

        assert!(
            !site.heard_from(OTHER),
            "{line}: {OTHER} was sent {:?}; the holder was told {:?}",
            site.sent_to(OTHER),
            holder.said()
        );
        for (what, said) in [
            ("the click's answer", format!("{clicked:?}")),
            ("going back", format!("{back:?}")),
            ("a read", read),
        ] {
            assert!(!holds_the_value(&said), "{line}: {what} holds it: {said}");
        }
    }
}

/// A load that fails, its address holding the value, shows the chat nothing of that address: not
/// on the page the person is shown, and not in any page's address.
#[test]
fn a_load_that_fails_says_nothing_of_the_address_it_failed_at() {
    let Some((browser, _data, site)) = browser() else {
        return;
    };
    the_app_sends_it_to_sign_in(&browser, OTHER);
    let holder = Holder::on(&site, None);
    filled_with(&browser, &holder, "Key failing", RESERVED);

    let clicked = click(&browser, "button \"Send and fail\"");
    let waited = call(
        &browser,
        Call::WaitForText {
            page: None,
            text: "Page not loaded".into(),
            gone: false,
            within: Duration::from_secs(10),
        },
    );
    holder.released_within_ten_seconds();
    let failing = || {
        site.sent_to(HELD_SITE)
            .iter()
            .filter(|sent| sent.ends_with("/hangs-up"))
            .count()
    };
    let sent_before = failing();
    let back = call(&browser, Call::Back { page: None });
    std::thread::sleep(Duration::from_millis(800));
    let read = every_read(&browser);

    assert_eq!(
        failing(),
        sent_before,
        "going back sent it again: {:?}",
        site.sent_to(HELD_SITE)
    );
    assert_eq!(waited.content, Content::Seen(true), "purlis's own page");
    for (what, said) in [
        ("the click's answer", format!("{clicked:?}")),
        ("the wait's answer", format!("{waited:?}")),
        ("going back", format!("{back:?}")),
        ("a read", read),
    ] {
        assert!(!holds_the_value(&said), "{what} holds it: {said}");
    }
}

/// The page the answer is on: what a chat goes back to.
fn page_of(answer: &Answer) -> PageId {
    answer.page.as_ref().expect("a page").id.clone()
}

fn pages_open(browser: &Browser) -> usize {
    match call(browser, Call::Pages).content {
        Content::Pages(pages) => pages.len(),
        other => panic!("not pages: {other:?}"),
    }
}

/// The page the agent is on now.
fn current_page(browser: &Browser) -> PageId {
    match call(browser, Call::Pages).content {
        Content::Pages(pages) => {
            let current = pages.into_iter().find(|page| page.current);
            current.expect("a current page").id
        }
        other => panic!("not pages: {other:?}"),
    }
}

/// A site an earlier load showed, in this page or another, is no part of this sign-in's chain.
#[test]
fn a_site_an_earlier_load_showed_asks() {
    let Some((browser, _data, site)) = browser() else {
        return;
    };
    let first = opened(&browser, "http://other.test/landed.html");
    let address = WebAddress::parse("http://auth.fixture.test/held-form.html").unwrap();
    call(
        &browser,
        Call::Navigate {
            page: Some(page_of(&first)),
            address,
        },
    );
    assert!(sees(&browser, "Sign in"), "the form is open");
    let holder = Holder::on(&site, None);
    filled(&browser, &holder);

    let answer = click(&browser, "button \"Sign in\"");

    assert_eq!(answer.asked, ["other.test"], "the answer names the site");
    assert!(sees(&browser, "Sign in"), "the page stays");
}

/// A site a chain only passed through asks: it began elsewhere.
#[test]
fn a_site_its_chain_passed_through_asks() {
    let Some((browser, _data, site)) = browser() else {
        return;
    };
    opened(
        &browser,
        &redirecting_to(
            "third.test",
            &redirecting_to(OTHER, "http://auth.fixture.test/held-form.html"),
        ),
    );
    assert!(sees(&browser, "Sign in"), "the form is open");
    let holder = Holder::on(&site, None);
    filled(&browser, &holder);

    let answer = click(&browser, "button \"Sign in\"");

    assert_eq!(answer.asked, ["other.test"], "the answer names the site");
}

/// Going back to the form begins its chain again: the site the chat had gone on to is no part of
/// this sign-in.
#[test]
fn going_back_to_a_sign_in_begins_its_chain_again() {
    let Some((browser, _data, site)) = browser() else {
        return;
    };
    let first = opened(&browser, "http://auth.fixture.test/held-form.html");
    assert!(sees(&browser, "Sign in"), "the form is open");
    let address = WebAddress::parse("http://other.test/landed.html").unwrap();
    call(
        &browser,
        Call::Navigate {
            page: Some(page_of(&first)),
            address,
        },
    );
    assert!(sees(&browser, "Landed on the other site"), "it went on");
    call(
        &browser,
        Call::Back {
            page: Some(page_of(&first)),
        },
    );
    assert!(sees(&browser, "Sign in"), "the form is back");
    let holder = Holder::on(&site, None);
    filled(&browser, &holder);

    let answer = click(&browser, "button \"Sign in\"");

    assert_eq!(answer.asked, ["other.test"], "the answer names the site");
}

/// A link a page offers stays in the chain the chat began.
#[test]
fn a_link_followed_on_the_way_to_a_sign_in_keeps_its_chain() {
    let Some((browser, _data, site)) = browser() else {
        return;
    };
    opened(&browser, "http://other.test/opens-sign-in.html");
    click(&browser, "link \"Sign in here\"");
    assert!(sees(&browser, "API key"), "the form is open");
    let holder = Holder::on(&site, None);
    filled(&browser, &holder);

    click(&browser, "button \"Sign in\"");

    assert!(sees(&browser, "Landed on the other site"), "it landed");
    assert_eq!(holder.said(), ["held", "released"], "nobody was asked");
}

/// A window the filled page opens is closed before the hold ends, and what it was set to send
/// never goes.
#[test]
fn a_window_the_page_opened_is_gone_before_the_hold_ends() {
    let Some((browser, _data, site)) = browser() else {
        return;
    };
    the_app_sends_it_to_sign_in(&browser, OTHER);
    let holder = Holder::on(&site, None);
    filled(&browser, &holder);
    let form = current_page(&browser);
    let before = pages_open(&browser);
    let opened = click(&browser, "button \"Open a window\"");
    assert_eq!(opened.opened.len(), 1, "the window opened");
    call(&browser, Call::SwitchPage { page: form });

    click(&browser, "button \"Sign in\"");

    assert!(sees(&browser, "Landed on the other site"), "it landed");
    holder.released_within_ten_seconds();
    std::thread::sleep(Duration::from_millis(1500));
    assert_eq!(pages_open(&browser), before, "the window closed");
    assert!(
        !site.was_sent("GET other.test/from-a-window"),
        "nothing it was set to send went"
    );
}

/// A fill is refused beside a blank window the page opened: it could read the field.
#[test]
fn a_fill_is_refused_beside_a_window_the_page_opened() {
    let Some((browser, _data, site)) = browser() else {
        return;
    };
    the_app_sends_it_to_sign_in(&browser, OTHER);
    let form = current_page(&browser);
    click(&browser, "button \"Open a window\"");
    call(&browser, Call::SwitchPage { page: form });
    let holder = Holder::on(&site, None);
    let fill = SecretFill {
        target: ref_of(&browser, "textbox \"API key\""),
        origin: "http://auth.fixture.test".to_owned(),
        key: "the-api-key".to_owned(),
        value: VALUE.to_owned().into(),
        multi_line: false,
        under_a_grant: false,
    };

    let filled = browser.fill_secret(
        browser.epoch(),
        &fill,
        Arc::clone(&holder) as Arc<dyn FillHold>,
    );

    assert!(filled.is_err(), "nothing was filled");
    assert!(holder.said().is_empty(), "nothing was held");
}

/// The window that opened the filled page, on another site, closes when the page is done, and a
/// window's chain begins where its opener's did.
#[test]
fn the_window_that_opened_the_page_is_gone_before_the_hold_ends() {
    let Some((browser, _data, site)) = browser() else {
        return;
    };
    opened(&browser, "http://other.test/opens-sign-in.html");
    let before = pages_open(&browser);
    let window = click(&browser, "button \"Sign in in a window\"");
    assert_eq!(window.opened.len(), 1, "the window opened");
    assert!(sees(&browser, "API key"), "the form is open");
    let holder = Holder::on(&site, None);
    filled(&browser, &holder);

    click(&browser, "button \"Sign in\"");

    assert!(sees(&browser, "Landed on the other site"), "it landed");
    holder.released_within_ten_seconds();
    assert_eq!(holder.said(), ["held", "released"], "nobody was asked");
    assert_eq!(
        pages_open(&browser),
        before,
        "its opener closed, and the window is left"
    );
}

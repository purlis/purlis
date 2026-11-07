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

use common::drive::{call, find, open, red_pixels, ref_on, screenshot, sees, snapshot};
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
        clean_already: true,
        back_to: None,
    }
}

/// A login in a browser that has held something: every one that fills starts clean (#5272).
fn in_a_used_browser(page: &str) -> Login {
    Login {
        clean_already: false,
        ..login_on(page, "http://fixture.test")
    }
}

/// How a login ended, in words fixed in this file: what a login answers may carry what its page
/// showed, so a failing test names only the kind of answer, never its words.
fn ended_as<T>(outcome: &Result<T, Refusal>) -> &'static str {
    match outcome {
        Ok(_) => "it answered",
        Err(Refusal::NotLoggedIn(_)) => "not logged in",
        Err(Refusal::NoFormOnCleanLoad(_)) => "no form on the clean load",
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
        .log_in(&login_at("http://fixture.test"), binds())
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
            binds(),
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
        .log_in(&login_at("https://shop.example.test"), binds())
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
            binds(),
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

/// #5272, the person's ruling: every login starts clean on its own site, so what an earlier session
/// there showed isn't what it fills, whoever chose the login.
#[test]
fn a_login_fills_the_form_of_a_site_with_no_earlier_session() {
    for suggested in [false, true] {
        let Some((browser, _data, _site)) = started() else {
            return;
        };
        open(&browser, "clean-earlier-session.html");
        assert!(
            sees(&browser, "An earlier session"),
            "{}",
            snapshot(&browser)
        );
        let login = Login {
            suggested,
            ..in_a_used_browser("clean-site.html")
        };
        let ended = browser.log_in(&login, binds()).expect("the login runs");
        assert!(ended.outcome.is_ok(), "{}", ended_as(&ended.outcome));
        assert!(sees(&browser, "Account"), "{}", snapshot(&browser));
        assert!(!snapshot(&browser).contains("Kept for the earlier session"));
    }
}

/// #5272: each of the stores an earlier session may be held in goes, a tab's own and the HTTP cache
/// passed by: each fixture's page shows its settings to a session it finds in that store.
#[test]
fn a_login_finds_no_earlier_session_in_anything_its_site_stores() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    open(&browser, "clean-stores-a-session.html");
    assert!(sees(&browser, "Stored"), "{}", snapshot(&browser));
    let ended = browser
        .log_in(&in_a_used_browser("clean-stored-site.html"), binds())
        .expect("the login runs");
    assert!(ended.outcome.is_ok(), "{}", ended_as(&ended.outcome));
    assert!(sees(&browser, "Account"), "{}", snapshot(&browser));
    assert!(!snapshot(&browser).contains("Kept for the earlier session"));
}

/// #5272: a page of an earlier session in the HTTP cache isn't what the login loads: the cache has no
/// way to clear one site, so the login's page passes it by.
#[test]
fn a_login_loads_past_a_page_of_an_earlier_session_in_the_http_cache() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    open(&browser, "cached-once");
    assert!(sees(&browser, "Settings"), "{}", snapshot(&browser));
    let ended = browser
        .log_in(&in_a_used_browser("cached-once"), binds())
        .expect("the login runs");
    assert!(ended.outcome.is_ok(), "{}", ended_as(&ended.outcome));
    assert!(sees(&browser, "Account"), "{}", snapshot(&browser));
    assert!(!snapshot(&browser).contains("Kept for the earlier session"));
}

/// #5272: a page left open on the site goes blank with the clearing, and another site keeps its
/// pages and what it stores.
#[test]
fn a_login_leaves_no_page_of_its_site_open_and_others_as_they_are() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    let other = WebAddress::parse("http://other.test/stores.html").unwrap();
    call(&browser, Call::Open { address: other });
    assert!(sees(&browser, "Stored here"), "{}", snapshot(&browser));
    open(&browser, "clean-earlier-session.html");
    let login = in_a_used_browser("clean-site.html");
    let ended = browser.log_in(&login, binds()).expect("the login runs");
    assert!(ended.outcome.is_ok(), "{}", ended_as(&ended.outcome));
    let pages = call(&browser, Call::Pages);
    let Content::Pages(pages) = pages.content else {
        panic!("its pages");
    };
    assert!(
        !pages.iter().any(|page| page.title == "An earlier session"),
        "the page of its site went blank"
    );
    assert!(
        pages.iter().any(|page| page.title == "Stored here"),
        "the page of another stays"
    );
    // binds() holds nothing here, so the other site can be read again.
    let other = WebAddress::parse("http://other.test/stores.html").unwrap();
    call(&browser, Call::Open { address: other });
    assert!(
        sees(&browser, "Found from before"),
        "{}",
        snapshot(&browser)
    );
}

/// #5272: a page of another site can't put a session back while the login clears and fills: its timer
/// waits for the login to answer, though the login's form takes longer than the timer to show.
#[test]
fn a_page_elsewhere_cannot_put_a_session_back_during_a_login() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    let later = WebAddress::parse("http://other.test/comes-back-late.html").unwrap();
    call(&browser, Call::Open { address: later });
    let login = in_a_used_browser("clean-late-site.html");
    let ended = browser.log_in(&login, binds()).expect("the login runs");
    assert!(ended.outcome.is_ok(), "{}", ended_as(&ended.outcome));
    assert!(sees(&browser, "Account"), "{}", snapshot(&browser));
    assert!(!snapshot(&browser).contains("Kept for the earlier session"));
    // And it runs again once the login has answered: its timer goes off.
    let start = std::time::Instant::now();
    loop {
        let Content::Pages(pages) = call(&browser, Call::Pages).content else {
            panic!("its pages");
        };
        if pages.iter().any(|page| page.title == "An earlier session") {
            break;
        }
        assert!(
            start.elapsed() < std::time::Duration::from_secs(20),
            "held still still"
        );
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

/// #5272: a page the clean load can't show its form on is said by its own kind, with nothing filled.
#[test]
fn a_login_page_with_no_form_for_a_clean_browser_is_said_by_its_own_kind() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    open(&browser, "clean-earlier-session.html");
    let ended = browser
        .log_in(&in_a_used_browser("clean-earlier-session.html"), binds())
        .expect("the login runs");
    assert!(
        matches!(ended.outcome, Err(Refusal::NoFormOnCleanLoad(_))),
        "{}",
        ended_as(&ended.outcome)
    );
}

/// #5272, #5310: what the no-form answer says leaves by the way every answer does. A user name that
/// is a word of that sentence is taken out of it, as a site's name holding one would be.
#[test]
fn a_login_s_no_form_answer_holds_no_value_it_filled() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    open(&browser, "clean-earlier-session.html");
    let login = Login {
        user: Some("clearing".to_owned().into()),
        ..in_a_used_browser("clean-earlier-session.html")
    };
    let ended = browser.log_in(&login, binds()).expect("the login runs");
    let Err(Refusal::NoFormOnCleanLoad(why)) = &ended.outcome else {
        panic!("{}", ended_as(&ended.outcome));
    };
    assert!(why.contains("no login form appeared"), "{why}");
    assert!(!why.contains("clearing"), "{why}");
    assert!(
        why.contains("(blanked)"),
        "the sentence no longer holds the word: {why}"
    );
}

/// #5121, SF-7: a value is kept out as a form spells it, as any fill's is, where the page it sent
/// the form to writes what its address carries into the page.
#[test]
fn a_login_value_a_page_echoes_as_a_form_sent_it_is_kept_out() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    let login = Login {
        password: "fixture+word/7".to_owned().into(),
        ..login_on("login-echoes.html", "http://fixture.test")
    };
    let ended = browser.log_in(&login, binds()).expect("the login runs");
    assert!(ended.outcome.is_ok(), "{}", ended_as(&ended.outcome));
    assert!(sees(&browser, "Sent"), "the page it sent the form to");
    assert!(
        !snapshot(&browser).contains("fixture%2Bword"),
        "the page shows the value as a form spelled it"
    );
}

/// #5121, SF-7: a login's field gives nothing to a copy, a cut or a drag, whoever makes it. Here a
/// page of one document keeps its user-name field once the password field has gone.
#[test]
fn a_filled_login_field_gives_nothing_to_a_copy_a_cut_or_a_drag() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    // Before a fill, the page's own copy goes: so a stop below is the guard's.
    open(&browser, "login-one-page.html");
    let take = ref_on(
        &find(&browser, "Try to take the name"),
        "Try to take the name",
    );
    call(&browser, Call::Click { target: take });
    let went = "Tried: copy went, cut went, dragstart went, trusted copy went";
    assert!(sees(&browser, went), "{}", snapshot(&browser));

    let ended = browser
        .log_in(
            &login_on("login-one-page.html", "http://fixture.test"),
            binds(),
        )
        .expect("the login runs");
    assert!(ended.outcome.is_ok(), "{}", ended_as(&ended.outcome));
    let take = ref_on(
        &find(&browser, "Try to take the name"),
        "Try to take the name",
    );
    call(&browser, Call::Click { target: take });
    let tried = "Tried: copy stopped, cut stopped, dragstart stopped, trusted copy stopped";
    assert!(sees(&browser, tried), "the page's own copy went");
}

/// #5121, SF-5: a box named for the user isn't a field a login goes into, whichever comes first.
#[test]
fn a_login_fills_a_one_line_field_and_never_a_box_named_for_the_user() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    let ended = browser
        .log_in(
            &login_on("login-one-page.html", "http://fixture.test"),
            binds(),
        )
        .expect("the login runs");
    assert!(ended.outcome.is_ok(), "{}", ended_as(&ended.outcome));
    assert!(
        sees(&browser, "Signed in as 14 characters"),
        "the field took it"
    );
}

/// #5121: whether a page asks to sign in is read with nothing typed, and the page stays put.
#[test]
fn a_page_is_read_for_whether_it_asks_to_sign_in() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    open(&browser, "frames.html");
    let asked = call(&browser, Call::AsksToSignIn);
    assert_eq!(asked.content, Content::AsksToSignIn(false));
    open(&browser, "login.html");
    let asked = call(&browser, Call::AsksToSignIn);
    assert_eq!(asked.content, Content::AsksToSignIn(true));
    assert!(sees(&browser, "Sign in"), "the page stays where it was");
    // A form that asks for the name first, with nothing saying so but its field's name.
    open(&browser, "login-name-first.html");
    let asked = call(&browser, Call::AsksToSignIn);
    assert_eq!(asked.content, Content::AsksToSignIn(true));
    // And a page with a search box doesn't ask.
    open(&browser, "search-box.html");
    let asked = call(&browser, Call::AsksToSignIn);
    assert_eq!(asked.content, Content::AsksToSignIn(false));
}

/// A holder that binds, and is told nothing.
fn binds() -> std::sync::Arc<dyn purlis_browser_engine::drive::Hold> {
    std::sync::Arc::new(|| -> Result<(), String> { Ok(()) })
}

/// The titles of the browser's pages now.
fn titles(browser: &purlis_browser_engine::drive::Browser) -> Vec<String> {
    match call(browser, Call::Pages).content {
        Content::Pages(pages) => pages.into_iter().map(|page| page.title).collect(),
        other => panic!("not pages: {other:?}"),
    }
}

/// The person's ruling of 2026-11-05: a login's page is held as a fill's is. Its page opens a
/// window on its own site as the user name is sent, which could read the password field: the
/// password doesn't go in, and the user name is cleared.
#[test]
fn a_login_fills_no_password_beside_a_window_its_page_opened_on_its_site() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    let ended = browser
        .log_in(
            &login_on("login-opens-on-its-site.html", "http://fixture.test"),
            binds(),
        )
        .expect("the login runs");
    match &ended.outcome {
        Err(Refusal::NotLoggedIn(why)) => {
            assert!(why.contains("related to the login's page"), "{why}");
            assert!(why.contains("nothing more was filled"), "{why}");
        }
        other => panic!("refused otherwise: {}", ended_as(other)),
    }
    assert!(
        titles(&browser).iter().any(|title| title == "Opened"),
        "the window opened, so this is what refused it"
    );
}

/// The person's ruling of 2026-11-05: a window a login's page opened on another site closes once the
/// page is done with what was filled: here, as the form it was filled in goes on to the next page.
#[test]
fn a_window_a_logins_page_opened_elsewhere_closes_as_the_page_goes_on() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    let ended = browser
        .log_in(
            &login_on("login-opens-elsewhere.html", "http://fixture.test"),
            binds(),
        )
        .expect("the login runs");
    assert!(ended.outcome.is_ok(), "{}", ended_as(&ended.outcome));
    // The chat is told on its next answer, which says the window opened and why it closed.
    let start = std::time::Instant::now();
    let mut told = String::new();
    loop {
        let answer = call(&browser, Call::Pages);
        told.push_str(answer.note.as_deref().unwrap_or_default());
        let Content::Pages(pages) = answer.content else {
            panic!("its pages");
        };
        let stays = pages.iter().any(|page| page.title == "Landed");
        if !stays
            && told.contains("purlis closed 1 other window related to the page purlis logged in on")
        {
            break;
        }
        assert!(
            start.elapsed() < std::time::Duration::from_secs(10),
            "the window stays, or the chat isn't told: {told}"
        );
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    assert!(sees(&browser, "Account"), "{}", snapshot(&browser));
}

/// #5272: a frame of the login's site in a page of another origin runs nothing while the login
/// clears and fills, so it can't put back what was cleared: whether that page is of another site,
/// its frame in a process of its own, or of another origin of the login's site. The frame's timer
/// sends a request to another host every 50 ms while it runs, and leaves an earlier session's
/// cookie where its context may.
#[test]
fn a_frame_of_the_logins_site_in_another_page_runs_nothing_during_the_login() {
    for top in [
        "http://other.test/frames-the-login-site.html",
        "http://auth.fixture.test/frames-its-site.html",
    ] {
        let Some((browser, _data, site)) = started() else {
            return;
        };
        let address = WebAddress::parse(top).unwrap();
        call(&browser, Call::Open { address });
        let start = std::time::Instant::now();
        while site.sent_count("third.test") < 5 {
            assert!(
                start.elapsed() < std::time::Duration::from_secs(10),
                "{top}: no ticks"
            );
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        // The form shows 3 s after the clean load begins: a second inside that, the frame is
        // looked at twice, a second apart.
        let ticked = std::thread::scope(|scope| {
            let watch = scope.spawn(|| {
                std::thread::sleep(std::time::Duration::from_millis(1500));
                let then = site.sent_count("third.test");
                std::thread::sleep(std::time::Duration::from_millis(1000));
                site.sent_count("third.test") - then
            });
            let ended = browser
                .log_in(&in_a_used_browser("clean-late-site.html"), binds())
                .expect("the login runs");
            assert!(ended.outcome.is_ok(), "{top}: {}", ended_as(&ended.outcome));
            watch.join().unwrap()
        });
        assert!(
            ticked <= 1,
            "{top}: the frame ran {ticked} times while the login held it still"
        );
        assert!(sees(&browser, "Account"), "{top}: {}", snapshot(&browser));
        assert!(
            !snapshot(&browser).contains("Kept for the earlier session"),
            "{top}: the frame put an earlier session back"
        );
    }
}

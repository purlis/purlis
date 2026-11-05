// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What a chat at its project's limit is told of its own browsers, and the browser it made and
//! never used, which a kept login takes the place of (#5296), at the window's browser-call seam
//! with the engine stood in.

use super::*;

fn limited_to(per_project: usize) -> ChatBrowsers {
    ChatBrowsers::default().limited(purlis_core::browserlimits::Limits {
        per_project,
        ..purlis_core::browserlimits::Limits::for_memory(None)
    })
}

fn page_at(hosted: &Hosted, address: &str) {
    *hosted.at.lock().unwrap() = Some(address.to_owned());
}

fn note_of(answer: &Answer) -> String {
    let Answer::Browsed { browsed, .. } = answer else {
        panic!("{answer:?}");
    };
    browsed.note.clone().unwrap_or_default()
}

#[test]
fn a_chat_at_the_limit_is_told_what_each_of_its_browsers_holds() {
    let browsers = limited_to(3);
    let hosted = Arc::new(Hosted::default());
    page_at(
        &hosted,
        "https://docs.example.test/private?token=open-sesame",
    );
    allowed_and_acting(&browsers, &hosted, 3);
    answered(&browsers, &hosted, &ask(3, Browsing::New));
    page_at(&hosted, "chrome-error://chromewebdata/");
    answered(&browsers, &hosted, &ask(3, click()));
    *hosted.at.lock().unwrap() = None;
    answered(&browsers, &hosted, &ask(3, log_in("shop-login")));
    answered(&browsers, &hosted, &ask(3, Browsing::New));

    let why = refusal(answered(&browsers, &hosted, &ask(3, Browsing::New)).0);

    assert!(
        why.contains("this project's chats have 3 browsers open"),
        "{why}"
    );
    assert!(why.contains("b1 (on docs.example.test"), "{why}");
    assert!(
        why.contains("b2 (holds login shop-login, on an error page)"),
        "{why}"
    );
    assert!(why.contains("b3 (empty)"), "{why}");
    assert!(why.contains("browser_close"), "{why}");
    assert!(
        !why.contains("token"),
        "no part of an address but its site: {why}"
    );
    assert!(!why.contains("open-sesame"), "no value: {why}");
    assert!(!why.contains(LOGIN_VALUE), "no value: {why}");
}

#[test]
fn a_chat_at_the_limit_counts_the_browsers_of_other_chats_and_never_names_them() {
    let browsers = limited_to(3);
    let hosted = Arc::new(Hosted::default());
    page_at(&hosted, "https://theirs.example.test/");
    allowed_and_acting(&browsers, &hosted, 4);
    answered(&browsers, &hosted, &ask(4, log_in("other-login")));
    answered(&browsers, &hosted, &ask(4, Browsing::New));
    page_at(&hosted, "https://mine.example.test/");
    allowed_and_acting(&browsers, &hosted, 3);

    let why = refusal(answered(&browsers, &hosted, &ask(3, Browsing::New)).0);

    assert!(why.contains("b1 (on mine.example.test"), "{why}");
    assert!(why.contains("2 more are other chats'"), "{why}");
    assert!(!why.contains("theirs.example.test"), "{why}");
    assert!(!why.contains("other-login"), "{why}");
    assert!(!why.contains("b2"), "{why}");
}

#[test]
fn a_chat_with_no_browser_at_the_limit_is_not_told_to_close_one_of_its_own() {
    let browsers = limited_to(1);
    let hosted = Arc::new(Hosted::default());
    allowed_and_acting(&browsers, &hosted, 4);
    answered(&browsers, &hosted, &ask(3, click()));
    browsers.allow(3, false);

    let why = refusal(answered(&browsers, &hosted, &ask(3, click())).0);

    assert!(why.contains("None of them is this chat's"), "{why}");
    assert!(!why.contains("browser_close"), "{why}");
}

#[test]
fn a_browser_the_person_took_over_is_listed_as_theirs_at_the_limit() {
    let browsers = limited_to(1);
    let hosted = Arc::new(Hosted::default());
    allowed_and_acting(&browsers, &hosted, 3);
    browsers.driven("program-1", true, 1000);

    let why = refusal(answered(&browsers, &hosted, &ask(3, Browsing::New)).0);

    assert!(why.contains("b1 (the person has taken it over"), "{why}");
}

#[test]
fn a_kept_login_closes_the_browser_the_chat_made_and_never_used_and_says_so() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    answered(&browsers, &hosted, &ask(3, click()));
    browsers.allow(3, false);
    answered(&browsers, &hosted, &ask(3, Browsing::New));

    let answer = answered(&browsers, &hosted, &ask(3, log_in("kept-login"))).0;

    let note = note_of(&answer);
    assert!(
        note.contains("closed browser b1, which this chat opened and never used"),
        "{note}"
    );
    assert!(!note.contains("still open"), "{note}");
    assert_eq!(*hosted.closed.lock().unwrap(), ["program-1"]);
    let open: Vec<String> = browsers.listed().into_iter().map(|one| one.name).collect();
    assert_eq!(open, ["b2"], "one browser open, the login's");
    let moment = browsers.closed_unused_moment(3, 1000).expect("said");
    assert_eq!(moment.moment.browser, "b1");
    assert!(moment.moment.did.contains("never used"), "{moment:?}");
    assert!(browsers.closed_unused_moment(3, 1001).is_none(), "once");
}

#[test]
fn a_kept_login_at_the_limit_succeeds_in_the_place_of_a_browser_never_used() {
    let browsers = limited_to(1);
    let hosted = Arc::new(Hosted::default());
    answered(&browsers, &hosted, &ask(3, click()));
    browsers.allow(3, false);
    answered(&browsers, &hosted, &ask(3, Browsing::New));

    let answer = answered(&browsers, &hosted, &ask(3, log_in("kept-login"))).0;

    assert!(note_of(&answer).contains("closed browser b1"), "{answer:?}");
    assert_eq!(hosted.logins.lock().unwrap().len(), 1);
}

#[test]
fn a_kept_login_at_the_limit_closes_a_never_used_browser_though_the_chat_uses_another() {
    let browsers = limited_to(2);
    let hosted = Arc::new(Hosted::default());
    answered(&browsers, &hosted, &ask(3, click()));
    browsers.allow(3, false);
    answered(&browsers, &hosted, &ask(3, Browsing::New));
    answered(&browsers, &hosted, &ask(3, Browsing::New));
    answered(&browsers, &hosted, &ask(3, click()));

    let note = note_of(&answered(&browsers, &hosted, &ask(3, log_in("kept-login"))).0);

    assert!(note.contains("closed browser b1"), "{note}");
    assert!(note.contains("Browser b2 is still open"), "{note}");
    assert_eq!(*hosted.closed.lock().unwrap(), ["program-1"]);
}

#[test]
fn below_the_limit_a_kept_login_leaves_a_never_used_browser_the_chat_is_not_using() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    answered(&browsers, &hosted, &ask(3, click()));
    browsers.allow(3, false);
    answered(&browsers, &hosted, &ask(3, Browsing::New));
    answered(&browsers, &hosted, &ask(3, Browsing::New));
    answered(&browsers, &hosted, &ask(3, click()));

    answered(&browsers, &hosted, &ask(3, log_in("kept-login")));

    assert!(hosted.closed.lock().unwrap().is_empty());
}

#[test]
fn a_kept_login_never_closes_a_browser_with_an_act_in_it() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    allowed_and_acting(&browsers, &hosted, 3);
    answered(&browsers, &hosted, &ask(3, Browsing::New));
    answered(&browsers, &hosted, &ask(3, click()));

    let note = note_of(&answered(&browsers, &hosted, &ask(3, log_in("kept-login"))).0);

    assert!(hosted.closed.lock().unwrap().is_empty());
    assert!(note.contains("Browser b2 is still open"), "{note}");
}

#[test]
fn a_kept_login_never_closes_a_browser_the_person_took_over_even_once_handed_back() {
    let browsers = limited_to(1);
    let hosted = Arc::new(Hosted::default());
    answered(&browsers, &hosted, &ask(3, click()));
    browsers.allow(3, false);
    answered(&browsers, &hosted, &ask(3, Browsing::New));
    browsers.driven("program-1", true, 1000);
    browsers.driven("program-1", false, 1001);

    let why = refusal(answered(&browsers, &hosted, &ask(3, log_in("kept-login"))).0);

    assert!(hosted.closed.lock().unwrap().is_empty());
    assert!(
        why.contains("this project's chats have 1 browser open"),
        "{why}"
    );
}

#[test]
fn a_kept_login_never_closes_a_browser_the_person_pinned() {
    let browsers = limited_to(1);
    let hosted = Arc::new(Hosted::default());
    answered(&browsers, &hosted, &ask(3, click()));
    browsers.allow(3, false);
    answered(&browsers, &hosted, &ask(3, Browsing::New));
    browsers.pin(3, "program-1", true).expect("pinned");

    refusal(answered(&browsers, &hosted, &ask(3, log_in("kept-login"))).0);

    assert!(hosted.closed.lock().unwrap().is_empty());
}

#[test]
fn a_browser_the_chat_asked_the_person_about_is_never_closed_as_unused() {
    let browsers = limited_to(1);
    let hosted = Arc::new(Hosted::default());
    let needs = crate::browserneedsyou::NeedsYou::default();
    answered(&browsers, &hosted, &ask(3, click()));
    browsers.allow(3, false);
    answered(&browsers, &hosted, &ask(3, Browsing::New));
    crate::browserneedsyou::answered_first(
        &browsers,
        &needs,
        &BrowseAsk {
            browser: Some("b1".into()),
            ..ask(
                3,
                Browsing::NeedsYou {
                    question: "Sign in, please".into(),
                },
            )
        },
        1000,
    );

    refusal(answered(&browsers, &hosted, &ask(3, log_in("kept-login"))).0);

    assert!(hosted.closed.lock().unwrap().is_empty());
}

#[test]
fn a_kept_login_a_browser_already_holds_closes_nothing() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    allowed_and_acting(&browsers, &hosted, 3);
    answered(&browsers, &hosted, &ask(3, log_in("kept-login")));
    answered(&browsers, &hosted, &ask(3, Browsing::New));

    answered(&browsers, &hosted, &ask(3, log_in("kept-login")));

    assert!(hosted.closed.lock().unwrap().is_empty());
}

#[test]
fn a_kept_login_that_fails_still_says_the_never_used_browser_closed() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    answered(&browsers, &hosted, &ask(3, click()));
    browsers.allow(3, false);
    answered(&browsers, &hosted, &ask(3, Browsing::New));
    *hosted.login_fails.lock().unwrap() = 1;

    let why = refusal(answered(&browsers, &hosted, &ask(3, log_in("kept-login"))).0);

    assert!(why.contains("closed browser b1"), "{why}");
    assert!(why.contains("password field did not go"), "{why}");
}

#[test]
fn a_login_not_kept_goes_into_the_browser_never_used_and_closes_nothing() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    answered(&browsers, &hosted, &ask(3, click()));
    browsers.allow(3, false);
    answered(&browsers, &hosted, &ask(3, Browsing::New));

    answered(&browsers, &hosted, &ask(3, log_in("shop-login")));

    assert!(hosted.closed.lock().unwrap().is_empty());
    assert_eq!(hosted.logins.lock().unwrap().len(), 1);
}

#[test]
fn a_browser_the_person_used_and_handed_back_is_not_said_to_be_empty() {
    let browsers = limited_to(1);
    let hosted = Arc::new(Hosted::default());
    answered(&browsers, &hosted, &ask(3, click()));
    browsers.allow(3, false);
    answered(&browsers, &hosted, &ask(3, Browsing::New));
    browsers.driven("program-1", true, 1000);
    browsers.driven("program-1", false, 1001);

    let why = refusal(answered(&browsers, &hosted, &ask(3, Browsing::New)).0);

    assert!(why.contains("b1 (no page purlis has read)"), "{why}");
    assert!(!why.contains("empty"), "{why}");
}

#[test]
fn a_browser_the_person_has_is_not_offered_to_close() {
    let browsers = limited_to(1);
    let hosted = Arc::new(Hosted::default());
    allowed_and_acting(&browsers, &hosted, 3);
    browsers.driven("program-1", true, 1000);

    let why = refusal(answered(&browsers, &hosted, &ask(3, Browsing::New)).0);

    assert!(!why.contains("browser_close"), "{why}");
    assert!(why.contains("ask the person"), "{why}");
}

#[test]
fn a_never_used_browser_that_will_not_end_stays_this_chats_in_the_refusal() {
    let browsers = limited_to(1);
    let hosted = Arc::new(Hosted::default());
    answered(&browsers, &hosted, &ask(3, click()));
    browsers.allow(3, false);
    answered(&browsers, &hosted, &ask(3, Browsing::New));
    *hosted.closes_fail.lock().unwrap() = 100;
    *hosted.stops_fail.lock().unwrap() = 100;

    let why = refusal(answered(&browsers, &hosted, &ask(3, log_in("kept-login"))).0);

    assert!(why.contains("keeps trying"), "{why}");
    assert!(why.contains("b1 (purlis is still ending it)"), "{why}");
    assert!(!why.contains("None of them"), "{why}");
    assert!(!why.contains("another chat"), "{why}");
    assert_eq!(hosted.logins.lock().unwrap().len(), 0, "over no limit");
}

#[test]
fn a_sweep_and_a_refusal_at_the_limit_at_once_never_wait_on_each_other() {
    let browsers = Arc::new(limited_to(1));
    let hosted = Arc::new(Hosted::default());
    allowed_and_acting(&browsers, &hosted, 3);
    let (done, finished) = std::sync::mpsc::channel();
    let told = done.clone();
    {
        let (browsers, hosted) = (Arc::clone(&browsers), Arc::clone(&hosted));
        std::thread::spawn(move || {
            for _ in 0..2000 {
                refusal(answered(&browsers, &hosted, &ask(3, Browsing::New)).0);
            }
            told.send(()).expect("told");
        });
    }
    std::thread::spawn(move || {
        for _ in 0..2000 {
            browsers.idle(None, 0, &[]);
        }
        done.send(()).expect("told");
    });
    finished
        .recv_timeout(Duration::from_secs(60))
        .expect("neither waits for ever");
    finished
        .recv_timeout(Duration::from_secs(60))
        .expect("neither waits for ever");
}

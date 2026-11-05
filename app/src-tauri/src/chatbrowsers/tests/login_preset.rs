// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A browser login as a preset of secret fill (#5121, the person's ruling SF-7 on #5096): which
//! browser it goes into, when it's already logged in, and when a new browser opens, at the
//! window's browser-call seam, with the engine stood in.

use super::*;

/// The call as the window makes it: a browser holding the login answers first, then the normal
/// path, each with the same stand-in.
fn called_with(
    browsers: &ChatBrowsers,
    hosted: &Arc<Hosted>,
    browse: &BrowseAsk,
    still_it: &dyn Fn(&crate::chatbrowsers::LoggedIn) -> bool,
) -> Answer {
    let driver: Arc<dyn Driver> = hosted.clone();
    if let Some(answer) = browsers.before_a_login(
        Some(&driver),
        Path::new("/p"),
        browse,
        None,
        None,
        &|| reach(),
        still_it,
        1000,
    ) {
        return answer;
    }
    answered(browsers, hosted, browse).0
}

fn called(browsers: &ChatBrowsers, hosted: &Arc<Hosted>, browse: &BrowseAsk) -> Answer {
    called_with(browsers, hosted, browse, &|_| true)
}

/// A chat allowed to browse, with `b1` open and used.
fn with_a_used_browser() -> (ChatBrowsers, Arc<Hosted>) {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    answered(&browsers, &hosted, &ask(3, click()));
    browsers.allow(3, false);
    answered(&browsers, &hosted, &ask(3, click()));
    // Its page is on the test logins' site.
    *hosted.at.lock().unwrap() = Some("https://shop.example.test/account".to_owned());
    (browsers, hosted)
}

fn browser_and_note(answer: &Answer) -> (String, String) {
    let Answer::Browsed { browser, browsed } = answer else {
        panic!("{answer:?}");
    };
    (browser.clone(), browsed.note.clone().unwrap_or_default())
}

fn logged_into(hosted: &Hosted) -> Vec<String> {
    hosted
        .logins
        .lock()
        .unwrap()
        .iter()
        .map(|(program, _)| program.clone())
        .collect()
}

fn started_browsers(hosted: &Hosted) -> usize {
    hosted.kept.lock().unwrap().len()
}

#[test]
fn a_second_login_with_the_same_login_opens_no_browser_and_answers_already_logged_in() {
    let (browsers, hosted) = with_a_used_browser();
    called(&browsers, &hosted, &ask(3, log_in("shop-login")));
    *hosted.asks_to_sign_in.lock().unwrap() = Some(false);

    let answer = called(&browsers, &hosted, &ask(3, log_in("shop-login")));

    let (browser, note) = browser_and_note(&answer);
    assert_eq!(browser, "b1");
    assert!(note.contains("already logged in with shop-login"), "{note}");
    assert!(note.contains("purlis filled nothing"), "{note}");
    assert_eq!(logged_into(&hosted).len(), 1, "nothing is filled again");
    assert_eq!(started_browsers(&hosted), 1, "no new browser");
}

#[test]
fn a_login_whose_page_asks_to_sign_in_again_is_filled_again_in_the_same_browser() {
    // #5152: a site whose session ended shows its form, and the chat is told to call again.
    let (browsers, hosted) = with_a_used_browser();
    called(&browsers, &hosted, &ask(3, log_in("shop-login")));
    *hosted.asks_to_sign_in.lock().unwrap() = Some(true);

    let answer = called(&browsers, &hosted, &ask(3, log_in("shop-login")));

    let (browser, note) = browser_and_note(&answer);
    assert_eq!(browser, "b1");
    assert!(!note.contains("already logged in"), "{note}");
    assert_eq!(logged_into(&hosted), ["program-1", "program-1"]);
    assert_eq!(started_browsers(&hosted), 1, "no new browser");
}

#[test]
fn a_host_too_old_to_say_whether_a_page_asks_has_the_login_go_as_it_always_did() {
    let (browsers, hosted) = with_a_used_browser();
    called(&browsers, &hosted, &ask(3, log_in("shop-login")));
    *hosted.asks_to_sign_in.lock().unwrap() = None;

    let answer = called(&browsers, &hosted, &ask(3, log_in("shop-login")));

    assert!(!browser_and_note(&answer).1.contains("already logged in"));
    assert_eq!(logged_into(&hosted), ["program-1", "program-1"]);
}

#[test]
fn a_login_that_did_not_go_through_is_not_taken_for_a_session() {
    let (browsers, hosted) = with_a_used_browser();
    *hosted.login_fails.lock().unwrap() = 1;
    *hosted.asks_to_sign_in.lock().unwrap() = Some(false);
    called(&browsers, &hosted, &ask(3, log_in("shop-login")));

    let answer = called(&browsers, &hosted, &ask(3, log_in("shop-login")));

    assert!(!browser_and_note(&answer).1.contains("already logged in"));
    assert_eq!(logged_into(&hosted), ["program-1"], "it is tried again");
}

#[test]
fn a_failed_login_again_ends_the_session_a_browser_was_taken_to_hold() {
    let (browsers, hosted) = with_a_used_browser();
    called(&browsers, &hosted, &ask(3, log_in("shop-login")));
    *hosted.asks_to_sign_in.lock().unwrap() = Some(true);
    *hosted.login_fails.lock().unwrap() = 1;
    called(&browsers, &hosted, &ask(3, log_in("shop-login")));
    *hosted.asks_to_sign_in.lock().unwrap() = Some(false);

    let answer = called(&browsers, &hosted, &ask(3, log_in("shop-login")));

    assert!(!browser_and_note(&answer).1.contains("already logged in"));
}

#[test]
fn a_login_no_longer_the_same_login_is_not_answered_already_logged_in() {
    let (browsers, hosted) = with_a_used_browser();
    called(&browsers, &hosted, &ask(3, log_in("shop-login")));
    *hosted.asks_to_sign_in.lock().unwrap() = Some(false);

    let answer = called_with(&browsers, &hosted, &ask(3, log_in("shop-login")), &|_| {
        false
    });

    assert!(!browser_and_note(&answer).1.contains("already logged in"));
    assert_eq!(logged_into(&hosted).len(), 2);
    assert!(
        !hosted
            .acts
            .lock()
            .unwrap()
            .iter()
            .any(|(_, _, act)| matches!(act, Act::AsksToSignIn)),
        "no page is read for a login it isn't"
    );
}

#[test]
fn another_login_is_not_answered_by_the_session_of_the_first() {
    let (browsers, hosted) = with_a_used_browser();
    called(&browsers, &hosted, &ask(3, log_in("shop-login")));
    *hosted.asks_to_sign_in.lock().unwrap() = Some(false);

    let answer = called(&browsers, &hosted, &ask(3, log_in("other-login")));

    assert!(!browser_and_note(&answer).1.contains("already logged in"));
    assert_eq!(logged_into(&hosted).len(), 2);
}

#[test]
fn a_login_with_no_kept_store_fills_the_current_browser() {
    let (browsers, hosted) = with_a_used_browser();

    let answer = called(&browsers, &hosted, &ask(3, log_in("shop-login")));

    assert_eq!(browser_and_note(&answer).0, "b1");
    assert_eq!(logged_into(&hosted), ["program-1"]);
    assert_eq!(started_browsers(&hosted), 1);
}

#[test]
fn a_login_the_person_allowed_from_its_notice_fills_the_current_browser() {
    // It opened a new browser every time before SF-7 (#5096).
    let (browsers, hosted) = with_a_used_browser();
    let driver: Arc<dyn Driver> = hosted.clone();
    let granted = |name: &str| {
        logins(name).map(|read| crate::chatbrowsers::LoginRead {
            granted: true,
            login: LogIn {
                keep: None,
                ..read.login
            },
            ..read
        })
    };

    let (answer, _) = browsers.answer(
        Some(&driver),
        Path::new("/p"),
        None,
        &ask(3, log_in("shop-login")),
        &|| reach(),
        false,
        &AsPersona::default,
        &granted,
        &no_fills,
        &|| Err("no folder here".to_owned()),
        1000,
    );

    assert_eq!(browser_and_note(&answer).0, "b1");
    assert_eq!(logged_into(&hosted), ["program-1"]);
    assert_eq!(started_browsers(&hosted), 1, "no browser of its own");
}

#[test]
fn a_login_goes_into_the_browser_the_chat_names_though_it_uses_another() {
    let (browsers, hosted) = with_a_used_browser();
    answered(&browsers, &hosted, &ask(3, Browsing::New));
    let mut named = ask(3, log_in("shop-login"));
    named.browser = Some("b1".into());

    let answer = called(&browsers, &hosted, &named);

    assert_eq!(browser_and_note(&answer).0, "b1");
    assert_eq!(logged_into(&hosted), ["program-1"]);
}

#[test]
fn a_named_browser_is_filled_though_another_holds_the_login() {
    let (browsers, hosted) = with_a_used_browser();
    called(&browsers, &hosted, &ask(3, log_in("shop-login")));
    answered(&browsers, &hosted, &ask(3, Browsing::New));
    answered(&browsers, &hosted, &ask(3, click()));
    *hosted.asks_to_sign_in.lock().unwrap() = Some(false);
    let mut named = ask(3, log_in("shop-login"));
    named.browser = Some("b2".into());

    let answer = called(&browsers, &hosted, &named);

    assert_eq!(browser_and_note(&answer).0, "b2");
    assert_eq!(logged_into(&hosted), ["program-1", "program-2"]);
}

#[test]
fn a_browser_of_the_chats_that_holds_the_login_answers_though_it_uses_another_now() {
    let (browsers, hosted) = with_a_used_browser();
    called(&browsers, &hosted, &ask(3, log_in("shop-login")));
    answered(&browsers, &hosted, &ask(3, Browsing::New));
    answered(&browsers, &hosted, &ask(3, click()));
    *hosted.asks_to_sign_in.lock().unwrap() = Some(false);

    let answer = called(&browsers, &hosted, &ask(3, log_in("shop-login")));

    let (browser, note) = browser_and_note(&answer);
    assert_eq!(browser, "b1");
    assert!(note.contains("already logged in"), "{note}");
    assert_eq!(
        browsers.which(3, None).map(|(name, _)| name).as_deref(),
        Some("b1"),
        "the chat goes on in it"
    );
}

#[test]
fn a_browser_the_chat_just_asked_for_takes_the_login_though_another_holds_it() {
    // browser_new, then browser_login, is how a chat keeps a login, and how it asks for a new one.
    let (browsers, hosted) = with_a_used_browser();
    called(&browsers, &hosted, &ask(3, log_in("shop-login")));
    answered(&browsers, &hosted, &ask(3, Browsing::New));
    *hosted.asks_to_sign_in.lock().unwrap() = Some(false);

    let answer = called(&browsers, &hosted, &ask(3, log_in("shop-login")));

    assert_eq!(browser_and_note(&answer).0, "b2");
    assert_eq!(logged_into(&hosted), ["program-1", "program-2"]);
}

#[test]
fn a_kept_login_opens_a_browser_only_to_keep_it_says_why_and_offers_to_close_the_old_one() {
    let (browsers, hosted) = with_a_used_browser();

    let answer = called(&browsers, &hosted, &ask(3, log_in("kept-login")));

    let (browser, note) = browser_and_note(&answer);
    assert_eq!(browser, "b2");
    assert!(note.contains("kept for later reuse"), "{note}");
    assert!(
        note.contains("Browser b1 is still open: close it with browser_close"),
        "{note}"
    );
    assert_eq!(logged_into(&hosted), ["program-2"]);
    // And the person's view says so, once.
    let moment = browsers.left_behind_moment(3, 1000).expect("said");
    assert_eq!(moment.moment.browser, "b2");
    assert!(moment.moment.did.contains("kept for later"), "{moment:?}");
    assert!(moment.moment.did.contains("b1 is still open"), "{moment:?}");
    assert!(browsers.left_behind_moment(3, 1001).is_none(), "once");
}

/// #5272: a form that doesn't appear on the clean load leads to a browser that has held nothing,
/// with the reason and the offer to close the old one, in the answer and in the person's view.
#[test]
fn a_form_not_on_the_clean_load_opens_a_clean_browser_says_why_and_offers_to_close_the_old_one() {
    let (browsers, hosted) = with_a_used_browser();
    *hosted.no_form_on_clean_load.lock().unwrap() = 1;

    let answer = called(&browsers, &hosted, &ask(3, log_in("shop-login")));

    let (browser, note) = browser_and_note(&answer);
    assert_eq!(browser, "b2");
    assert!(note.contains("no login form appeared"), "{note}");
    assert!(note.contains("once its site was cleared"), "{note}");
    assert!(
        note.contains("Browser b1 is still open") && note.contains("close it with browser_close"),
        "{note}"
    );
    assert_eq!(
        *hosted.tried_on_clean_load.lock().unwrap(),
        ["program-1"],
        "tried where the chat was first"
    );
    assert_eq!(logged_into(&hosted), ["program-2"]);
    assert_eq!(
        hosted.kept.lock().unwrap().last().unwrap(),
        &None,
        "it opens from nothing kept"
    );
    let moment = browsers.left_behind_moment(3, 1000).expect("said");
    assert_eq!(moment.moment.browser, "b2");
    assert!(moment.moment.did.contains("No login form"), "{moment:?}");
    assert!(moment.moment.did.contains("b1 is still open"), "{moment:?}");
    assert!(browsers.left_behind_moment(3, 1001).is_none(), "once");
}

/// #5272: "and to nothing else": a second failure there is the answer, with no third browser.
#[test]
fn a_form_not_on_the_clean_load_opens_one_browser_and_no_more() {
    let (browsers, hosted) = with_a_used_browser();
    *hosted.no_form_on_clean_load.lock().unwrap() = 5;

    let answer = called(&browsers, &hosted, &ask(3, log_in("shop-login")));

    let Answer::No { why } = answer else {
        panic!("{answer:?}");
    };
    assert!(why.contains("purlis opened browser b2"), "{why}");
    assert_eq!(started_browsers(&hosted), 2);
    assert_eq!(hosted.tried_on_clean_load.lock().unwrap().len(), 2);
    assert!(logged_into(&hosted).is_empty());
}

/// #5272: nor does a later call, whatever the browser opened for it says then.
#[test]
fn a_browser_opened_as_a_clean_load_failed_opens_no_other_on_a_later_call() {
    let (browsers, hosted) = with_a_used_browser();
    *hosted.no_form_on_clean_load.lock().unwrap() = 5;
    called(&browsers, &hosted, &ask(3, log_in("shop-login")));

    let answer = called(&browsers, &hosted, &ask(3, log_in("shop-login")));

    assert!(matches!(answer, Answer::No { .. }), "{answer:?}");
    assert_eq!(started_browsers(&hosted), 2);
}

/// #5272: nor does the browser left behind, named again.
#[test]
fn a_browser_left_as_a_clean_load_failed_opens_no_other_when_named_again() {
    let (browsers, hosted) = with_a_used_browser();
    *hosted.no_form_on_clean_load.lock().unwrap() = 5;
    called(&browsers, &hosted, &ask(3, log_in("shop-login")));

    let again = BrowseAsk {
        browser: Some("b1".to_owned()),
        ..ask(3, log_in("shop-login"))
    };
    let answer = called(&browsers, &hosted, &again);

    assert!(matches!(answer, Answer::No { .. }), "{answer:?}");
    assert_eq!(started_browsers(&hosted), 2);
}

/// #5272: each browser left behind keeps its offer, in the answer and in the person's view.
#[test]
fn a_kept_login_with_no_form_on_its_clean_load_offers_to_close_both_browsers() {
    let (browsers, hosted) = with_a_used_browser();
    *hosted.no_form_on_clean_load.lock().unwrap() = 1;

    let answer = called(&browsers, &hosted, &ask(3, log_in("kept-login")));

    let (browser, note) = browser_and_note(&answer);
    assert_eq!(browser, "b3");
    assert!(note.contains("Browser b1 is still open"), "{note}");
    assert!(note.contains("Browser b2 is still open"), "{note}");
    let first = browsers.left_behind_moment(3, 1000).expect("said");
    assert!(first.moment.did.contains("b1 is still open"), "{first:?}");
    let second = browsers.left_behind_moment(3, 1000).expect("said");
    assert!(second.moment.did.contains("b2 is still open"), "{second:?}");
    assert!(browsers.left_behind_moment(3, 1000).is_none());
}

/// #5272: the first try may have taken a while: what policy says now holds the second.
#[test]
fn a_form_not_on_the_clean_load_opens_no_browser_where_policy_now_forbids_the_login() {
    use purlis_core::sandbox::policy::{Locks, set_for_this_test};
    let (browsers, hosted) = with_a_used_browser();
    *hosted.no_form_on_clean_load.lock().unwrap() = 1;
    set_for_this_test(Locks::parse(
        r#"{"sandbox": {"persona-tools": false}}"#,
        Path::new("/etc/purlis/policy.json"),
    ));

    let answer = called(&browsers, &hosted, &ask(3, log_in("shop-login")));

    let Answer::No { why } = answer else {
        panic!("{answer:?}");
    };
    assert!(why.contains("policy forbids"), "{why}");
    assert_eq!(started_browsers(&hosted), 1);
    assert!(logged_into(&hosted).is_empty());
}

/// #5272: no room for one says so, and leaves the chat where it is, with nothing filled.
#[test]
fn a_form_not_on_the_clean_load_with_no_room_for_a_browser_says_both() {
    let (browsers, hosted) = with_a_used_browser();
    *hosted.no_form_on_clean_load.lock().unwrap() = 1;
    *hosted.refuses.lock().unwrap() = Some(NotStarted::Failed("no room".into()));

    let answer = called(&browsers, &hosted, &ask(3, log_in("shop-login")));

    let Answer::No { why } = answer else {
        panic!("{answer:?}");
    };
    assert!(why.contains("no login form appeared"), "{why}");
    assert!(why.contains("no room"), "{why}");
    assert!(logged_into(&hosted).is_empty());
    assert!(browsers.left_behind_moment(3, 1000).is_none());
}

/// #5272: a session counts where the login went through, never where it only tried.
#[test]
fn a_login_moved_by_its_clean_load_holds_its_session_in_the_new_browser() {
    let (browsers, hosted) = with_a_used_browser();
    *hosted.no_form_on_clean_load.lock().unwrap() = 1;
    called(&browsers, &hosted, &ask(3, log_in("shop-login")));
    *hosted.asks_to_sign_in.lock().unwrap() = Some(false);

    let again = called(&browsers, &hosted, &ask(3, log_in("shop-login")));

    let (browser, note) = browser_and_note(&again);
    assert_eq!(browser, "b2");
    assert!(note.contains("already logged in"), "{note}");
    assert_eq!(logged_into(&hosted), ["program-2"], "filled once");
}

#[test]
fn a_kept_login_with_no_browser_open_leaves_nothing_behind_to_offer() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    answered(&browsers, &hosted, &ask(3, click()));
    browsers.allow(3, false);

    let answer = called(&browsers, &hosted, &ask(3, log_in("kept-login")));

    let (browser, note) = browser_and_note(&answer);
    assert_eq!(browser, "b1");
    assert!(!note.contains("still open"), "{note}");
    assert!(browsers.left_behind_moment(3, 1000).is_none());
}

#[test]
fn a_kept_login_called_again_opens_no_copy_of_its_store() {
    // It opened b2, then b3, from one store (#5096).
    let (browsers, hosted) = with_a_used_browser();
    called(&browsers, &hosted, &ask(3, log_in("kept-login")));
    *hosted.asks_to_sign_in.lock().unwrap() = Some(false);

    let answer = called(&browsers, &hosted, &ask(3, log_in("kept-login")));

    let (browser, note) = browser_and_note(&answer);
    assert_eq!(browser, "b2");
    assert!(note.contains("already logged in"), "{note}");
    assert_eq!(started_browsers(&hosted), 2, "no third browser");
}

#[test]
fn a_kept_login_whose_page_asks_again_goes_into_the_browser_holding_it() {
    let (browsers, hosted) = with_a_used_browser();
    called(&browsers, &hosted, &ask(3, log_in("kept-login")));
    *hosted.asks_to_sign_in.lock().unwrap() = Some(true);

    let answer = called(&browsers, &hosted, &ask(3, log_in("kept-login")));

    assert_eq!(browser_and_note(&answer).0, "b2");
    assert_eq!(logged_into(&hosted), ["program-2", "program-2"]);
    assert_eq!(started_browsers(&hosted), 2, "no third browser");
}

#[test]
fn a_chat_cannot_ask_for_the_read_of_whether_a_page_asks_to_sign_in() {
    let (browsers, hosted) = with_a_used_browser();
    let before = hosted.acts.lock().unwrap().len();

    let (answer, _) = answered(
        &browsers,
        &hosted,
        &ask(3, Browsing::Act(Act::AsksToSignIn)),
    );

    assert!(matches!(answer, Answer::No { .. }), "{answer:?}");
    assert_eq!(hosted.acts.lock().unwrap().len(), before);
}

#[test]
fn a_browser_the_person_has_taken_over_takes_no_login_and_nothing_is_asked_of_it() {
    // SF-10: said before a vault is read, so an Allow once isn't spent on it.
    let (browsers, hosted) = with_a_used_browser();
    let (_, program) = browsers.which(3, None).expect("b1");
    browsers.driven(&program, true, 1000);
    let before = hosted.acts.lock().unwrap().len();
    let driver: Arc<dyn Driver> = hosted.clone();

    let answer = browsers.before_a_login(
        Some(&driver),
        Path::new("/p"),
        &ask(3, log_in("shop-login")),
        None,
        None,
        &|| reach(),
        &|_| panic!("nothing is compared for a browser the person has"),
        1000,
    );

    let Some(Answer::No { why }) = answer else {
        panic!("{answer:?}");
    };
    assert!(why.contains("taken browser b1 over"), "{why}");
    assert!(why.contains("nothing was filled"), "{why}");
    assert_eq!(hosted.acts.lock().unwrap().len(), before, "nothing read");
}

#[test]
fn a_login_is_refused_where_the_person_has_the_browser_holding_it_though_the_chat_uses_another() {
    let (browsers, hosted) = with_a_used_browser();
    called(&browsers, &hosted, &ask(3, log_in("shop-login")));
    answered(&browsers, &hosted, &ask(3, Browsing::New));
    answered(&browsers, &hosted, &ask(3, click()));
    browsers.driven("program-1", true, 1000);

    let answer = called(&browsers, &hosted, &ask(3, log_in("shop-login")));

    let Answer::No { why } = answer else {
        panic!("{answer:?}");
    };
    assert!(why.contains("taken browser b1 over"), "{why}");
    assert_eq!(logged_into(&hosted).len(), 1, "nothing more was tried");
}

#[test]
fn a_browser_the_person_has_is_no_reason_to_refuse_a_login_going_elsewhere() {
    let (browsers, hosted) = with_a_used_browser();
    called(&browsers, &hosted, &ask(3, log_in("shop-login")));
    answered(&browsers, &hosted, &ask(3, Browsing::New));
    answered(&browsers, &hosted, &ask(3, click()));
    browsers.driven("program-2", true, 1000);
    *hosted.asks_to_sign_in.lock().unwrap() = Some(false);

    let answer = called(&browsers, &hosted, &ask(3, log_in("shop-login")));

    assert!(browser_and_note(&answer).1.contains("already logged in"));
}

#[test]
fn a_kept_login_that_fails_in_its_new_browser_still_says_it_moved() {
    let (browsers, hosted) = with_a_used_browser();
    *hosted.login_fails.lock().unwrap() = 1;

    let answer = called(&browsers, &hosted, &ask(3, log_in("kept-login")));

    let Answer::No { why } = answer else {
        panic!("{answer:?}");
    };
    assert!(why.contains("purlis opened browser b2"), "{why}");
    assert!(why.contains("Browser b1 is still open"), "{why}");
    assert!(why.contains("password field did not go"), "{why}");
}

#[test]
fn a_kept_login_goes_to_the_browser_holding_it_though_the_chat_just_opened_another() {
    // A second copy of a store keeps nothing, so a kept login isn't taken apart from it.
    let (browsers, hosted) = with_a_used_browser();
    called(&browsers, &hosted, &ask(3, log_in("kept-login")));
    answered(&browsers, &hosted, &ask(3, Browsing::New));

    let answer = called(&browsers, &hosted, &ask(3, log_in("kept-login")));

    assert_eq!(browser_and_note(&answer).0, "b2");
    assert_eq!(started_browsers(&hosted), 3, "no copy of its store");
    assert_eq!(logged_into(&hosted), ["program-2", "program-2"]);
}

#[test]
fn what_the_read_of_the_page_brought_is_told_with_the_answer() {
    let (browsers, hosted) = with_a_used_browser();
    called(&browsers, &hosted, &ask(3, log_in("shop-login")));
    *hosted.asks_to_sign_in.lock().unwrap() = Some(false);
    *hosted.asks.lock().unwrap() = vec!["new.example.test:443".to_owned()];

    let answer = called(&browsers, &hosted, &ask(3, log_in("shop-login")));

    let Answer::Browsed { browsed, .. } = answer else {
        panic!("{answer:?}");
    };
    assert_eq!(browsed.asked, ["new.example.test:443"]);
    assert_eq!(browsed.asks_to_sign_in, None, "the read is purlis's alone");
    let note = browsed.note.unwrap_or_default();
    assert!(note.contains("already logged in"), "{note}");
    assert!(note.contains("new to this project"), "{note}");
}

#[test]
fn a_login_switched_back_to_is_not_told_it_is_logged_in_as_the_site_is_signed_in_as_another() {
    let (browsers, hosted) = with_a_used_browser();
    called(&browsers, &hosted, &ask(3, log_in("shop-login")));
    called(&browsers, &hosted, &ask(3, log_in("other-login")));
    *hosted.asks_to_sign_in.lock().unwrap() = Some(false);

    let answer = called(&browsers, &hosted, &ask(3, log_in("shop-login")));

    assert!(
        !browser_and_note(&answer)
            .1
            .contains("already logged in with")
    );
    assert_eq!(logged_into(&hosted).len(), 3, "it runs again");
}

#[test]
fn a_page_off_the_logins_site_says_nothing_of_its_session() {
    // A sign-in host's account picker, another tab, a blank page.
    let (browsers, hosted) = with_a_used_browser();
    called(&browsers, &hosted, &ask(3, log_in("shop-login")));
    *hosted.asks_to_sign_in.lock().unwrap() = Some(false);
    *hosted.at.lock().unwrap() = Some("https://auth.example.test/pick".to_owned());

    let answer = called(&browsers, &hosted, &ask(3, log_in("shop-login")));

    assert!(
        !browser_and_note(&answer)
            .1
            .contains("already logged in with")
    );
    assert_eq!(logged_into(&hosted).len(), 2, "it looks on its own page");
}

#[test]
fn a_page_that_answers_with_no_address_says_nothing_of_a_session() {
    let (browsers, hosted) = with_a_used_browser();
    called(&browsers, &hosted, &ask(3, log_in("shop-login")));
    *hosted.asks_to_sign_in.lock().unwrap() = Some(false);
    *hosted.at.lock().unwrap() = None;

    let answer = called(&browsers, &hosted, &ask(3, log_in("shop-login")));

    assert!(
        !browser_and_note(&answer)
            .1
            .contains("already logged in with")
    );
}

#[test]
fn a_login_only_tried_in_a_browser_does_not_bring_the_chat_back_to_it() {
    // "The browser the chat names, or its current one": a refusal there isn't a session.
    let (browsers, hosted) = with_a_used_browser();
    *hosted.login_fails.lock().unwrap() = 1;
    called(&browsers, &hosted, &ask(3, log_in("shop-login")));
    answered(&browsers, &hosted, &ask(3, Browsing::New));
    answered(&browsers, &hosted, &ask(3, click()));

    let answer = called(&browsers, &hosted, &ask(3, log_in("shop-login")));

    assert_eq!(browser_and_note(&answer).0, "b2");
    assert_eq!(logged_into(&hosted), ["program-2"]);
}

#[test]
fn a_login_refused_in_a_browser_it_holds_leaves_the_chat_in_the_one_it_uses() {
    let (browsers, hosted) = with_a_used_browser();
    called(&browsers, &hosted, &ask(3, log_in("shop-login")));
    answered(&browsers, &hosted, &ask(3, Browsing::New));
    answered(&browsers, &hosted, &ask(3, click()));
    *hosted.asks_to_sign_in.lock().unwrap() = Some(true);
    *hosted.login_fails.lock().unwrap() = 1;

    let answer = called(&browsers, &hosted, &ask(3, log_in("shop-login")));

    assert!(matches!(answer, Answer::No { .. }), "{answer:?}");
    assert_eq!(
        browsers.which(3, None).map(|(name, _)| name).as_deref(),
        Some("b2"),
        "its later calls go where they went"
    );
}

#[test]
fn a_browser_holding_another_sites_login_says_so_before_anyone_is_asked() {
    // Before a notice, so an Allow once isn't spent on a login that can't go in.
    let (browsers, hosted) = with_a_used_browser();
    called(&browsers, &hosted, &ask(3, log_in("shop-login")));
    let driver: Arc<dyn Driver> = hosted.clone();
    let asked = |login: &str, origin: &str| {
        browsers.before_a_login(
            Some(&driver),
            Path::new("/p"),
            &ask(3, log_in(login)),
            None,
            Some(origin),
            &|| reach(),
            &|_| false,
            1000,
        )
    };

    let Some(Answer::No { why }) = asked("other-login", "https://other.example.test") else {
        panic!("refused");
    };
    assert!(why.contains("browser b1 holds login shop-login"), "{why}");
    assert!(why.contains("browser_new"), "{why}");
    assert!(
        asked("other-login", "https://shop.example.test").is_none(),
        "the same site goes on"
    );
    assert!(
        asked("shop-login", "https://other.example.test").is_none(),
        "its own login naming another site asks to replace it"
    );
}

mod the_same_login {
    use crate::chatbrowsers::{LoggedIn, is_the_same_login};

    fn session(granted: bool) -> LoggedIn {
        LoggedIn {
            persona: Some("steward".into()),
            login: "shop".into(),
            identity: "same".into(),
            address: "https://shop.example.test/login".into(),
            form: None,
            granted,
        }
    }

    fn now(identity: &str) -> Result<String, String> {
        Ok(identity.to_owned())
    }

    fn not_now() -> Result<String, String> {
        Err("not allowed now".to_owned())
    }

    #[test]
    fn is_the_same_identity_allowed_now() {
        assert!(is_the_same_login(
            &session(false),
            true,
            true,
            false,
            now("same")
        ));
    }

    #[test]
    fn is_not_one_whose_declaration_changed_since() {
        assert!(!is_the_same_login(
            &session(false),
            true,
            true,
            false,
            now("other")
        ));
        assert!(!is_the_same_login(
            &session(true),
            true,
            true,
            true,
            now("other")
        ));
    }

    #[test]
    fn is_not_a_declared_login_no_longer_allowed() {
        assert!(!is_the_same_login(
            &session(false),
            true,
            true,
            false,
            not_now()
        ));
    }

    #[test]
    fn is_an_allow_once_that_went_with_its_read() {
        assert!(is_the_same_login(
            &session(true),
            true,
            true,
            false,
            not_now()
        ));
    }

    #[test]
    fn is_not_a_grant_that_can_no_longer_be_read() {
        assert!(!is_the_same_login(
            &session(true),
            true,
            true,
            true,
            not_now()
        ));
    }

    #[test]
    fn is_not_a_call_naming_another_site() {
        assert!(!is_the_same_login(
            &session(false),
            false,
            true,
            false,
            now("same")
        ));
    }

    #[test]
    fn is_not_one_policy_or_a_runners_opt_in_forbids_since() {
        assert!(!is_the_same_login(
            &session(false),
            true,
            false,
            false,
            now("same")
        ));
        assert!(!is_the_same_login(
            &session(true),
            true,
            false,
            false,
            not_now()
        ));
    }
}

#[test]
fn a_chat_no_longer_allowed_a_browser_is_not_told_it_is_logged_in() {
    let (browsers, hosted) = with_a_used_browser();
    called(&browsers, &hosted, &ask(3, log_in("shop-login")));
    *hosted.asks_to_sign_in.lock().unwrap() = Some(false);
    browsers.lock().get_mut(&3).expect("chat 3").posture = crate::chatbrowsers::Posture::Off;
    let driver: Arc<dyn Driver> = hosted.clone();

    let answer = browsers.before_a_login(
        Some(&driver),
        Path::new("/p"),
        &ask(3, log_in("shop-login")),
        None,
        None,
        &|| reach(),
        &|_| true,
        1000,
    );

    assert!(answer.is_none(), "{answer:?}");
}

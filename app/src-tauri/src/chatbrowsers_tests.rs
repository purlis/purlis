// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;
use std::sync::{Arc, Mutex};

use purlis_core::hookwire::{Answer, BrowseAsk, Browsing};
use purlis_session_protocol::browsing::{Act, Browsed, BrowserReach};

use purlis_core::sandbox::persona::Browser;

use super::{AsPersona, ChatBrowsers, Driver, Ended, Posture, Refused};

/// A driver that stands in for the session host: it starts numbered browsers, answers every
/// act settled, and keeps what it was asked.
#[derive(Default)]
struct Hosted {
    started: Mutex<Vec<BrowserReach>>,
    acts: Mutex<Vec<(String, String, Act)>>,
    closed: Mutex<Vec<String>>,
    /// Programs ended by their id, when a close did not end them.
    stopped: Mutex<Vec<String>>,
    /// Programs that have ended in the host.
    ended: Mutex<Vec<String>>,
    /// How many closes, and how many stops by id, fail from now.
    closes_fail: Mutex<u32>,
    stops_fail: Mutex<u32>,
}

fn fails(left: &Mutex<u32>) -> bool {
    let mut left = left.lock().unwrap();
    let failing = *left > 0;
    *left = left.saturating_sub(1);
    failing
}

impl Driver for Hosted {
    fn start(&self, _root: &Path, reach: BrowserReach) -> Result<String, String> {
        let mut started = self.started.lock().unwrap();
        started.push(reach);
        Ok(format!("program-{}", started.len()))
    }

    fn act(&self, program: &str, call_id: &str, act: Act) -> Result<Browsed, Refused> {
        if self.ended.lock().unwrap().iter().any(|one| one == program) {
            return Err(Refused::Gone);
        }
        self.acts
            .lock()
            .unwrap()
            .push((program.to_owned(), call_id.to_owned(), act));
        Ok(Browsed {
            settled: true,
            ..Browsed::default()
        })
    }

    fn close(&self, program: &str) -> Result<(), String> {
        if fails(&self.closes_fail) {
            return Err("the session host did not answer in time".into());
        }
        self.closed.lock().unwrap().push(program.to_owned());
        Ok(())
    }

    fn stop(&self, program: &str) -> Result<(), String> {
        if fails(&self.stops_fail) {
            return Err("the session host did not answer in time".into());
        }
        self.stopped.lock().unwrap().push(program.to_owned());
        Ok(())
    }
}

fn ask(chat: u32, asked: Browsing) -> BrowseAsk {
    BrowseAsk {
        chat,
        browser: None,
        call_id: "c-1".into(),
        asked,
    }
}

fn click() -> Browsing {
    Browsing::Act(Act::Click {
        target: "e1".into(),
    })
}

fn reach() -> BrowserReach {
    BrowserReach {
        listed: vec![purlis_session_protocol::session::Listed {
            host: "docs.example.com".into(),
            layer: purlis_session_protocol::session::Layer::Open,
        }],
        never: Vec::new(),
        withdrawn: Vec::new(),
        open_web: false,
    }
}

fn answered(browsers: &ChatBrowsers, hosted: &Arc<Hosted>, browse: &BrowseAsk) -> (Answer, bool) {
    answered_as(browsers, hosted, browse, &AsPersona::default())
}

/// [`answered`] for a chat whose persona, declaration and Always are `as_persona`'s.
fn answered_as(
    browsers: &ChatBrowsers,
    hosted: &Arc<Hosted>,
    browse: &BrowseAsk,
    as_persona: &AsPersona,
) -> (Answer, bool) {
    let driver: Arc<dyn Driver> = hosted.clone();
    browsers.answer(
        Some(&driver),
        Path::new("/p"),
        browse,
        &|| reach(),
        &|| as_persona.clone(),
        1000,
    )
}

/// A chat running as `persona`, which declares `declared`, with Always kept as `always`.
fn as_persona(persona: &str, declared: Option<Browser>, always: Option<bool>) -> AsPersona {
    AsPersona {
        persona: Some(persona.to_owned()),
        declared,
        always,
    }
}

fn refusal(answer: Answer) -> String {
    match answer {
        Answer::No { why } => why,
        other => panic!("not a refusal: {other:?}"),
    }
}

#[test]
fn a_chats_first_browser_call_raises_one_ask_and_starts_no_browser() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());

    let (first, raised) = answered(&browsers, &hosted, &ask(3, click()));
    let (again, raised_again) = answered(&browsers, &hosted, &ask(3, click()));

    assert!(raised, "the first call raises the Ask");
    assert!(!raised_again, "one Ask, however often the chat calls");
    let why = refusal(first);
    assert!(why.contains("asked the person"), "{why}");
    assert!(why.contains("told"), "{why}");
    assert!(refusal(again).contains("asked the person"));
    assert!(hosted.started.lock().unwrap().is_empty());
    assert_eq!(
        browsers.waiting(),
        [super::Waits {
            session: 3,
            since: 1000,
            as_persona: AsPersona::default(),
        }]
    );
}

#[test]
fn once_allowed_the_chats_browser_starts_with_its_chats_reach_and_takes_the_act() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    answered(&browsers, &hosted, &ask(3, click()));

    assert!(browsers.allow(3, false));
    let (answer, _) = answered(&browsers, &hosted, &ask(3, click()));

    match answer {
        Answer::Browsed { browser, browsed } => {
            assert_eq!(browser, "b1");
            assert!(browsed.settled);
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(*hosted.started.lock().unwrap(), [reach()]);
    assert_eq!(
        *hosted.acts.lock().unwrap(),
        [(
            "program-1".to_owned(),
            "c-1".to_owned(),
            Act::Click {
                target: "e1".into()
            }
        )]
    );
    assert!(
        browsers.waiting().is_empty(),
        "answered, the Ask waits no more"
    );
}

#[test]
fn the_open_web_is_given_only_where_the_person_allowed_it() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    answered(&browsers, &hosted, &ask(3, click()));
    browsers.allow(3, true);

    answered(&browsers, &hosted, &ask(3, click()));

    assert!(hosted.started.lock().unwrap()[0].open_web);
}

#[test]
fn a_chat_whose_browser_the_person_kept_off_is_refused_in_words_that_say_not_to_retry() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    answered(&browsers, &hosted, &ask(3, click()));

    assert!(browsers.keep_off(3));
    let why = refusal(answered(&browsers, &hosted, &ask(3, click())).0);

    assert!(why.contains("don't retry"), "{why}");
    assert!(hosted.started.lock().unwrap().is_empty());
    assert_eq!(browsers.posture(3), Posture::Off);
}

#[test]
fn an_answer_to_an_ask_that_waits_no_more_changes_nothing() {
    let browsers = ChatBrowsers::default();
    assert!(!browsers.allow(3, true), "nothing asked");
    assert!(!browsers.keep_off(3));
    assert_eq!(browsers.posture(3), Posture::Unasked);
}

#[test]
fn another_chat_is_asked_for_itself() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    answered(&browsers, &hosted, &ask(3, click()));
    browsers.allow(3, false);

    let (_, raised) = answered(&browsers, &hosted, &ask(4, click()));

    assert!(raised);
}

#[test]
fn a_new_browser_is_apart_from_the_others_and_used_from_then() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    answered(&browsers, &hosted, &ask(3, click()));
    browsers.allow(3, false);
    answered(&browsers, &hosted, &ask(3, click()));

    let (made, _) = answered(&browsers, &hosted, &ask(3, Browsing::New));
    answered(&browsers, &hosted, &ask(3, click()));
    let named = BrowseAsk {
        browser: Some("b1".into()),
        ..ask(3, click())
    };
    answered(&browsers, &hosted, &named);

    match made {
        Answer::Browsed { browser, .. } => assert_eq!(browser, "b2"),
        other => panic!("{other:?}"),
    }
    let programs: Vec<String> = hosted
        .acts
        .lock()
        .unwrap()
        .iter()
        .map(|(program, ..)| program.clone())
        .collect();
    assert_eq!(programs, ["program-1", "program-2", "program-1"]);
}

#[test]
fn a_browser_of_another_chat_is_never_reached_by_its_name() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    for chat in [3, 4] {
        answered(&browsers, &hosted, &ask(chat, click()));
        browsers.allow(chat, false);
    }
    answered(&browsers, &hosted, &ask(3, click()));

    let why = refusal(
        answered(
            &browsers,
            &hosted,
            &BrowseAsk {
                browser: Some("b1".into()),
                ..ask(4, Browsing::Close)
            },
        )
        .0,
    );

    assert!(why.contains("no browser b1"), "{why}");
    assert!(hosted.closed.lock().unwrap().is_empty());
}

#[test]
fn a_project_holds_at_most_three_browsers_across_its_chats() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    for chat in [3, 4] {
        answered(&browsers, &hosted, &ask(chat, click()));
        browsers.allow(chat, false);
    }
    answered(&browsers, &hosted, &ask(3, Browsing::New));
    answered(&browsers, &hosted, &ask(3, Browsing::New));
    answered(&browsers, &hosted, &ask(4, Browsing::New));

    let why = refusal(answered(&browsers, &hosted, &ask(4, Browsing::New)).0);

    assert!(why.contains("this project"), "{why}");
    assert!(why.contains("browser_close"), "{why}");
    assert_eq!(hosted.started.lock().unwrap().len(), 3);
    // One closed makes room, whichever chat it was of.
    answered(&browsers, &hosted, &ask(3, Browsing::Close));
    answered(&browsers, &hosted, &ask(4, Browsing::New));
    assert_eq!(hosted.started.lock().unwrap().len(), 4);
}

#[test]
fn a_running_browser_ends_when_its_chats_reach_narrows_and_the_next_opens_on_what_it_is_now() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    answered(&browsers, &hosted, &ask(3, click()));
    browsers.allow(3, false);
    answered(&browsers, &hosted, &ask(3, click()));
    answered(&browsers, &hosted, &ask(4, click()));
    browsers.allow(4, false);
    answered(&browsers, &hosted, &ask(4, click()));
    let driver: Arc<dyn Driver> = hosted.clone();

    let ended = browsers.narrowed(Some(&driver), 3);

    assert_eq!(
        ended,
        Ended {
            ended: 1,
            unended: 0
        }
    );
    assert_eq!(
        *hosted.closed.lock().unwrap(),
        ["program-1"],
        "only chat 3's"
    );
    assert_eq!(
        browsers.narrowed(Some(&driver), 3),
        Ended::default(),
        "nothing left to end"
    );
    let narrower = BrowserReach {
        withdrawn: vec![5173],
        ..reach()
    };
    let (answer, _) = browsers.answer(
        Some(&driver),
        Path::new("/p"),
        &ask(3, click()),
        &|| narrower.clone(),
        &AsPersona::default,
        1000,
    );
    let Answer::Browsed { browsed, .. } = answer else {
        panic!("{answer:?}");
    };
    assert!(
        browsed
            .note
            .as_deref()
            .is_some_and(|note| note.contains("withdrew")),
        "{browsed:?}"
    );
    assert_eq!(hosted.started.lock().unwrap().last(), Some(&narrower));
    assert_eq!(browsers.posture(3), Posture::Allowed { open_web: false });
}

#[test]
fn a_browser_that_ended_is_forgotten_and_the_next_call_starts_a_new_one() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    answered(&browsers, &hosted, &ask(3, click()));
    browsers.allow(3, false);
    answered(&browsers, &hosted, &ask(3, click()));
    hosted.ended.lock().unwrap().push("program-1".into());

    let why = refusal(answered(&browsers, &hosted, &ask(3, click())).0);
    answered(&browsers, &hosted, &ask(3, click()));

    assert!(why.contains("ended"), "{why}");
    assert_eq!(hosted.started.lock().unwrap().len(), 2, "a new one started");
}

#[test]
fn closing_a_chat_ends_its_browsers_and_forgets_its_answer() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    answered(&browsers, &hosted, &ask(3, click()));
    browsers.allow(3, false);
    answered(&browsers, &hosted, &ask(3, click()));
    answered(&browsers, &hosted, &ask(3, Browsing::New));

    let driver: Arc<dyn Driver> = hosted.clone();
    browsers.forget(Some(&driver), 3);

    assert_eq!(*hosted.closed.lock().unwrap(), ["program-1", "program-2"]);
    assert_eq!(browsers.posture(3), Posture::Unasked);
}

#[test]
fn with_no_session_host_there_is_no_browser_and_the_chat_is_told_so() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    answered(&browsers, &hosted, &ask(3, click()));
    browsers.allow(3, false);

    let why = refusal(
        browsers
            .answer(
                None,
                Path::new("/p"),
                &ask(3, click()),
                &|| reach(),
                &AsPersona::default,
                0,
            )
            .0,
    );

    assert!(why.contains("session host"), "{why}");
}

#[test]
fn a_browser_whose_chats_reach_narrowed_is_ended_retried_until_it_is() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    answered(&browsers, &hosted, &ask(3, click()));
    browsers.allow(3, false);
    answered(&browsers, &hosted, &ask(3, click()));
    let driver: Arc<dyn Driver> = hosted.clone();
    // The host cannot be told: every close and every stop by its id fails, for now.
    *hosted.closes_fail.lock().unwrap() = u32::MAX;
    *hosted.stops_fail.lock().unwrap() = u32::MAX;

    let ended = browsers.narrowed(Some(&driver), 3);

    assert_eq!(
        ended,
        Ended {
            ended: 0,
            unended: 1
        }
    );
    assert_eq!(browsers.unended(), ["program-1"], "it stays known");
    // The chat's next act never reaches it: a new browser opens on what the chat reaches now.
    answered(&browsers, &hosted, &ask(3, click()));
    let acts: Vec<String> = hosted
        .acts
        .lock()
        .unwrap()
        .iter()
        .map(|(program, _, _)| program.clone())
        .collect();
    assert_eq!(acts, ["program-1", "program-2"]);
    // Once the host hears, the next call retries it, and it is ended.
    *hosted.closes_fail.lock().unwrap() = 0;
    *hosted.stops_fail.lock().unwrap() = 0;
    answered(&browsers, &hosted, &ask(3, click()));
    assert_eq!(*hosted.closed.lock().unwrap(), ["program-1"]);
    assert!(browsers.unended().is_empty());
}

#[test]
fn a_browser_a_close_did_not_end_is_ended_by_its_id() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    answered(&browsers, &hosted, &ask(3, click()));
    browsers.allow(3, false);
    answered(&browsers, &hosted, &ask(3, click()));
    let driver: Arc<dyn Driver> = hosted.clone();
    *hosted.closes_fail.lock().unwrap() = u32::MAX;

    let ended = browsers.narrowed(Some(&driver), 3);

    assert_eq!(
        ended,
        Ended {
            ended: 1,
            unended: 0
        }
    );
    assert_eq!(*hosted.stopped.lock().unwrap(), ["program-1"]);
    assert!(browsers.unended().is_empty());
}

#[test]
fn a_browser_not_yet_ended_counts_toward_the_projects_three() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    answered(&browsers, &hosted, &ask(3, click()));
    browsers.allow(3, false);
    for _ in 0..3 {
        answered(&browsers, &hosted, &ask(3, Browsing::New));
    }
    let driver: Arc<dyn Driver> = hosted.clone();
    *hosted.closes_fail.lock().unwrap() = u32::MAX;
    *hosted.stops_fail.lock().unwrap() = u32::MAX;
    browsers.narrowed(Some(&driver), 3);

    let why = refusal(answered(&browsers, &hosted, &ask(3, click())).0);

    assert!(why.contains("this project"), "{why}");
    assert_eq!(hosted.started.lock().unwrap().len(), 3);
}

/// #2245: a chat's browser tools run without the harness's own prompt, so the app refuses a
/// browser call from a chat this window does not have open, before any Ask: it raises none,
/// starts nothing, and says why. It holds the project and its hook socket, which a sandboxed
/// run may not: CI runs it first.
#[test]
fn a_browser_call_from_a_chat_the_window_does_not_have_open_is_refused_and_asks_nothing() {
    let dir = tempfile::tempdir().expect("a directory");
    let root = dir.path().join("project");
    std::fs::create_dir_all(&root).expect("the project");
    std::fs::write(purlis_core::names::manifest(&root), "schema = 1\n").expect("its manifest");
    let planes = crate::planes::Planes::telling(Arc::new(|_| {}), crate::Shipped::default(), None);
    let id = planes.open(&root);
    let held = planes.held(&id).expect("held");
    assert!(held.chats().open_now().is_empty(), "no chat is open");

    let why = refusal(super::answered(&held, &ask(7, click())));

    assert!(why.contains("not one this purlis window has open"), "{why}");
    assert_eq!(held.browsers().posture(7), Posture::Unasked);
    assert!(held.browsers().waiting().is_empty(), "no Ask is raised");
}

// ---- a persona's browser, and Always for this persona (#2241) --------------------------------

/// #2241: Always for this persona holds for every chat opened as that persona: its first
/// browser call opens a browser reaching what was answered, and nothing is asked.
#[test]
fn always_for_this_persona_opens_a_chats_browser_with_no_ask() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    let qa = as_persona("qa", None, Some(true));

    let (answer, raised) = answered_as(&browsers, &hosted, &ask(3, click()), &qa);
    let (_, raised_too) = answered_as(&browsers, &hosted, &ask(4, click()), &qa);

    assert!(matches!(answer, Answer::Browsed { .. }), "{answer:?}");
    assert!(!raised && !raised_too, "nothing is asked");
    let started = hosted.started.lock().unwrap();
    assert_eq!(started.len(), 2);
    assert!(started.iter().all(|reach| reach.open_web));
    assert!(browsers.waiting().is_empty());
}

/// #2241, ADR 0073: a persona's declaration only preselects the Ask's answer: the person is
/// still asked, nothing starts until they answer, and the Ask carries what it declares.
#[test]
fn a_declared_browser_is_preselected_and_the_person_still_answers() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    let qa = as_persona("qa", Some(Browser::OpenWeb), None);

    let (answer, raised) = answered_as(&browsers, &hosted, &ask(3, click()), &qa);

    assert!(raised);
    assert!(refusal(answer).contains("asked the person"));
    assert!(hosted.started.lock().unwrap().is_empty());
    assert_eq!(browsers.waiting()[0].as_persona, qa);
}

/// #2241 security: what a persona declares never widens what the person answered, whether
/// they answered for this chat or Always for the persona.
#[test]
fn what_a_persona_declares_never_widens_what_the_person_answered() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    // Always kept for this project's sites; the persona now declares the open web.
    let widened = as_persona("qa", Some(Browser::OpenWeb), Some(false));
    answered_as(&browsers, &hosted, &ask(3, click()), &widened);
    // Asked, declared the open web, and answered this project's sites for this chat.
    let declared = as_persona("qa", Some(Browser::OpenWeb), None);
    answered_as(&browsers, &hosted, &ask(4, click()), &declared);
    assert!(browsers.allow(4, false));
    answered_as(&browsers, &hosted, &ask(4, click()), &declared);

    let started = hosted.started.lock().unwrap();
    assert_eq!(started.len(), 2);
    assert!(started.iter().all(|reach| !reach.open_web), "{started:?}");
}

/// #2241: Always answered on one chat's Ask answers every chat as that persona that waits on
/// the same Ask, and no chat of another persona.
#[test]
fn always_answers_every_chat_as_that_persona_that_waits() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    let qa = as_persona("qa", None, None);
    answered_as(&browsers, &hosted, &ask(3, click()), &qa);
    answered_as(&browsers, &hosted, &ask(4, click()), &qa);
    answered_as(
        &browsers,
        &hosted,
        &ask(5, click()),
        &as_persona("devops", None, None),
    );

    let mut answered = browsers.allow_always("qa", false);
    answered.sort_unstable();

    assert_eq!(answered, [3, 4]);
    assert_eq!(browsers.posture(3), Posture::Allowed { open_web: false });
    assert_eq!(browsers.posture(5), Posture::Asked);
}

/// #2241 security, D-2013-11/16: removing Always narrows running chats: the browsers of each
/// chat it opened end, and the chat's next call asks the person again.
#[test]
fn removing_always_ends_the_browsers_it_opened_and_the_next_call_asks_again() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    let driver: Arc<dyn Driver> = hosted.clone();
    answered_as(
        &browsers,
        &hosted,
        &ask(3, click()),
        &as_persona("qa", None, Some(false)),
    );
    // A chat the person answered for itself keeps its browser.
    answered_as(&browsers, &hosted, &ask(4, click()), &AsPersona::default());
    browsers.allow(4, false);
    answered_as(&browsers, &hosted, &ask(4, click()), &AsPersona::default());

    let ended = browsers.always_removed(Some(&driver), "qa");

    assert_eq!(
        ended,
        [(
            3,
            Ended {
                ended: 1,
                unended: 0
            }
        )]
    );
    assert_eq!(*hosted.closed.lock().unwrap(), ["program-1"]);
    let (_, raised) = answered_as(
        &browsers,
        &hosted,
        &ask(3, click()),
        &as_persona("qa", None, None),
    );
    assert!(raised, "asked again");
    assert_eq!(browsers.posture(4), Posture::Allowed { open_web: false });
}

/// #2241: Always that is gone at a chat's next call (removed elsewhere, or held by an
/// administrator's policy) ends the browsers it opened and asks again; one narrowed to this
/// project's sites ends them and opens the next on that; one widened ends nothing.
#[test]
fn always_that_changed_since_a_chats_browser_opened_is_followed_at_its_next_call() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    answered_as(
        &browsers,
        &hosted,
        &ask(3, click()),
        &as_persona("qa", None, Some(true)),
    );
    // Narrowed to this project's sites.
    answered_as(
        &browsers,
        &hosted,
        &ask(3, click()),
        &as_persona("qa", None, Some(false)),
    );
    assert_eq!(*hosted.closed.lock().unwrap(), ["program-1"]);
    assert!(!hosted.started.lock().unwrap()[1].open_web);
    // Widened: what runs runs on.
    answered_as(
        &browsers,
        &hosted,
        &ask(3, click()),
        &as_persona("qa", None, Some(true)),
    );
    assert_eq!(hosted.closed.lock().unwrap().len(), 1);
    // Gone: ended, and asked again.
    let (answer, raised) = answered_as(
        &browsers,
        &hosted,
        &ask(3, click()),
        &as_persona("qa", None, None),
    );
    assert!(raised);
    assert!(refusal(answer).contains("asked the person"));
    assert_eq!(hosted.closed.lock().unwrap().len(), 2);
}

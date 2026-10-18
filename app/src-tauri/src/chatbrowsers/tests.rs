// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use purlis_core::hookwire::{Answer, BrowseAsk, Browsing};
use purlis_session_protocol::browsing::{Act, Browsed, BrowserReach};

use purlis_core::sandbox::persona::Browser;

use super::{AsPersona, ChatBrowsers, Driver, Ended, NotStarted, Posture, Refused};

/// A driver that stands in for the session host: it starts numbered browsers, answers every
/// act settled, and keeps what it was asked.
#[derive(Default)]
struct Hosted {
    started: Mutex<Vec<BrowserReach>>,
    acts: Mutex<Vec<(String, String, Act)>>,
    /// What each act was told its browser reaches from then on (#2018).
    reached: Mutex<Vec<BrowserReach>>,
    /// The sites each act answers it asked about (#2018).
    asks: Mutex<Vec<String>>,
    closed: Mutex<Vec<String>>,
    /// Programs ended by their id, when a close did not end them.
    stopped: Mutex<Vec<String>>,
    /// Programs that have ended in the host.
    ended: Mutex<Vec<String>>,
    /// How many closes, and how many stops by id, fail from now.
    closes_fail: Mutex<u32>,
    stops_fail: Mutex<u32>,
    /// The device's budget, where the host says it is reached.
    full: Mutex<Option<usize>>,
    /// While set, every close waits until it is let go of: a host that does not answer.
    holding: Mutex<bool>,
    let_go: Condvar,
    /// How many closes it was asked for, and from which one on it holds them.
    closes: Mutex<u32>,
    holds_from: Mutex<Option<u32>>,
    /// How many closes wait now.
    waiting: Mutex<u32>,
}

impl Hosted {
    /// Every close from the `nth` on waits until [`Self::answer_closes`].
    fn hold_closes_from(&self, nth: u32) {
        *self.holds_from.lock().unwrap() = Some(nth);
    }

    fn answer_closes(&self) {
        *self.holds_from.lock().unwrap() = None;
        *self.holding.lock().unwrap() = false;
        self.let_go.notify_all();
    }

    /// Waits until a close waits on the host.
    fn until_a_close_waits(&self) {
        until(|| *self.waiting.lock().unwrap() > 0);
    }
}

/// Waits, a few seconds at most, until `done`.
fn until(done: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !done() {
        assert!(Instant::now() < deadline, "it never came");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn fails(left: &Mutex<u32>) -> bool {
    let mut left = left.lock().unwrap();
    let failing = *left > 0;
    *left = left.saturating_sub(1);
    failing
}

impl Driver for Hosted {
    fn start(&self, _root: &Path, reach: BrowserReach) -> Result<String, NotStarted> {
        if let Some(most) = *self.full.lock().unwrap() {
            return Err(NotStarted::Full { most });
        }
        let mut started = self.started.lock().unwrap();
        started.push(reach);
        Ok(format!("program-{}", started.len()))
    }

    fn act(
        &self,
        program: &str,
        call_id: &str,
        act: Act,
        reach: BrowserReach,
    ) -> Result<Browsed, Refused> {
        if self.ended.lock().unwrap().iter().any(|one| one == program) {
            return Err(Refused::Gone);
        }
        self.acts
            .lock()
            .unwrap()
            .push((program.to_owned(), call_id.to_owned(), act));
        self.reached.lock().unwrap().push(reach);
        Ok(Browsed {
            settled: true,
            asked: self.asks.lock().unwrap().clone(),
            ..Browsed::default()
        })
    }

    fn close(&self, program: &str) -> Result<(), String> {
        let mut holding = self.holding.lock().unwrap();
        let nth = {
            let mut closes = self.closes.lock().unwrap();
            *closes += 1;
            *closes
        };
        if self
            .holds_from
            .lock()
            .unwrap()
            .is_some_and(|from| nth >= from)
        {
            *holding = true;
        }
        if *holding {
            *self.waiting.lock().unwrap() += 1;
            while *holding {
                holding = self.let_go.wait(holding).unwrap();
            }
            *self.waiting.lock().unwrap() -= 1;
        }
        drop(holding);
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
    answered_confined(browsers, hosted, browse, as_persona, true)
}

/// [`answered_as`] for a chat that has, or has not, a sandbox's record of its reach.
fn answered_confined(
    browsers: &ChatBrowsers,
    hosted: &Arc<Hosted>,
    browse: &BrowseAsk,
    as_persona: &AsPersona,
    confined: bool,
) -> (Answer, bool) {
    let driver: Arc<dyn Driver> = hosted.clone();
    browsers.answer(
        Some(&driver),
        Path::new("/p"),
        browse,
        &|| reach(),
        confined,
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
fn past_the_devices_budget_a_chats_browser_is_refused_in_words_that_say_not_to_retry() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    answered(&browsers, &hosted, &ask(3, click()));
    browsers.allow(3, false);
    *hosted.full.lock().unwrap() = Some(8);

    let why = refusal(answered(&browsers, &hosted, &ask(3, click())).0);

    assert!(why.contains("this device runs 8 browsers"), "{why}");
    assert!(why.contains("browser_close"), "{why}");
    assert!(!why.contains("Try once more"), "{why}");
}

#[test]
fn a_projects_limit_is_the_one_its_browsers_are_held_to() {
    let browsers = ChatBrowsers::default().limited(purlis_core::browserlimits::Limits {
        per_project: 1,
        ..purlis_core::browserlimits::Limits::for_memory(None)
    });
    let hosted = Arc::new(Hosted::default());
    allowed_and_acting(&browsers, &hosted, 3);

    let why = refusal(answered(&browsers, &hosted, &ask(3, Browsing::New)).0);

    assert!(
        why.contains("this project's chats have 1 browser open"),
        "{why}"
    );
    assert_eq!(hosted.started.lock().unwrap().len(), 1);
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
        true,
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
                true,
                &AsPersona::default,
                0,
            )
            .0,
    );

    assert!(why.contains("session host"), "{why}");
}

#[test]
fn a_browser_whose_chats_reach_narrowed_is_ended_retried_until_it_is() {
    let browsers = retrying_quickly();
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
    // Once the host hears, the retry ends it, with no call of the chat's.
    *hosted.closes_fail.lock().unwrap() = 0;
    *hosted.stops_fail.lock().unwrap() = 0;
    until(|| browsers.unended().is_empty());
    assert_eq!(*hosted.closed.lock().unwrap(), ["program-1"]);
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

#[test]
fn a_browser_whose_engine_ended_no_longer_counts_toward_the_projects_three() {
    // #2385: the host said it ended; the chat has not called since.
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    answered(&browsers, &hosted, &ask(3, click()));
    browsers.allow(3, false);
    for _ in 0..3 {
        answered(&browsers, &hosted, &ask(3, Browsing::New));
    }
    assert!(browsers.engine_ended("program-2"));

    let fourth = answered(&browsers, &hosted, &ask(3, Browsing::New)).0;

    assert!(matches!(fourth, Answer::Browsed { .. }), "{fourth:?}");
    assert_eq!(hosted.started.lock().unwrap().len(), 4);
}

/// #2385: the host's word that a browser's engine ended reaches the project its chat is in:
/// the browser leaves the listing, the window is told so, and the Ask about it ends with an
/// Update for the Inbox. It holds the project and its hook socket, which a sandboxed run may
/// not: CI runs it first.
#[test]
fn an_engine_that_ends_ends_the_ask_about_it_in_its_project_with_an_update() {
    let dir = tempfile::tempdir().expect("a directory");
    let root = dir.path().join("project");
    std::fs::create_dir_all(&root).expect("the project");
    std::fs::write(purlis_core::names::manifest(&root), "schema = 1\n").expect("its manifest");
    let hosted = Arc::new(Hosted::default());
    let asks_ended: Arc<Mutex<Vec<crate::browserneedsyou::AskEnded>>> = Arc::default();
    let listed: Arc<Mutex<u32>> = Arc::default();
    let planes = crate::planes::Planes::telling(Arc::new(|_| {}), crate::Shipped::default(), None)
        .driving_browsers_with(hosted.clone())
        .telling_browser_asks_ended({
            let told = Arc::clone(&asks_ended);
            Arc::new(move |one: crate::browserneedsyou::AskEnded| told.lock().unwrap().push(one))
        })
        .telling_browsers_listed({
            let told = Arc::clone(&listed);
            Arc::new(move |_: crate::planes::PlaneId| *told.lock().unwrap() += 1)
        });
    let id = planes.open(&root);
    let held = planes.held(&id).expect("held");
    answered(held.browsers(), &hosted, &ask(3, click()));
    held.browsers().allow(3, false);
    answered(held.browsers(), &hosted, &ask(3, click()));
    let question = BrowseAsk {
        browser: Some("b1".into()),
        ..ask(
            3,
            Browsing::NeedsYou {
                question: "letters".into(),
            },
        )
    };
    crate::browserneedsyou::answered_first(held.browsers(), held.needs_you(), &question, 1000);
    assert_eq!(held.needs_you().waiting().len(), 1);
    let before = *listed.lock().unwrap();

    super::engine_ended(&planes.every_held(), "program-1");

    assert!(held.needs_you().waiting().is_empty());
    let ended = asks_ended.lock().unwrap().clone();
    assert_eq!(ended.len(), 1);
    assert_eq!((ended[0].session, ended[0].browser.as_str()), (3, "b1"));
    assert!(held.browsers().listed().is_empty());
    assert!(
        *listed.lock().unwrap() > before,
        "the window reads the listing again"
    );
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

/// #2241, D-2013-11: Always answered again narrower ends the browsers of the chats it allowed at
/// once, and the next opens on the narrower reach; answered again wider ends nothing.
#[test]
fn always_answered_again_narrower_ends_the_browsers_it_opened_at_once() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    let driver: Arc<dyn Driver> = hosted.clone();
    answered_as(
        &browsers,
        &hosted,
        &ask(3, click()),
        &as_persona("qa", None, Some(true)),
    );

    assert!(
        browsers
            .always_changed(Some(&driver), "qa", true)
            .is_empty(),
        "the same answer ends nothing"
    );
    let ended = browsers.always_changed(Some(&driver), "qa", false);

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
    assert_eq!(browsers.posture(3), Posture::Allowed { open_web: false });
    answered_as(
        &browsers,
        &hosted,
        &ask(3, click()),
        &as_persona("qa", None, Some(false)),
    );
    assert!(!hosted.started.lock().unwrap()[1].open_web);
}

/// Chat `chat`'s browser allowed, and one act of it carried out.
fn allowed_and_acting(browsers: &ChatBrowsers, hosted: &Arc<Hosted>, chat: u32) {
    answered(browsers, hosted, &ask(chat, click()));
    browsers.allow(chat, false);
    answered(browsers, hosted, &ask(chat, click()));
}

fn listed(chat: u32, name: &str, program: &str) -> super::ChatBrowser {
    super::ChatBrowser {
        session: chat,
        name: name.into(),
        program: program.into(),
        unended: false,
    }
}

#[test]
fn every_browser_of_the_projects_chats_is_listed_under_its_chat_by_the_name_the_chat_knows() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    allowed_and_acting(&browsers, &hosted, 5);
    allowed_and_acting(&browsers, &hosted, 3);
    answered(&browsers, &hosted, &ask(3, Browsing::New));

    assert_eq!(
        browsers.listed(),
        [
            listed(3, "b1", "program-2"),
            listed(3, "b2", "program-3"),
            listed(5, "b1", "program-1"),
        ]
    );
}

#[test]
fn the_listing_moves_when_a_browser_starts_or_ends_and_not_on_an_act() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    let driver: Arc<dyn Driver> = hosted.clone();
    let at_first = browsers.listing();

    allowed_and_acting(&browsers, &hosted, 3);
    let started = browsers.listing();
    answered(&browsers, &hosted, &ask(3, click()));
    let acted = browsers.listing();
    answered(&browsers, &hosted, &ask(3, Browsing::New));
    let another = browsers.listing();
    answered(&browsers, &hosted, &ask(3, Browsing::Close));
    let closed = browsers.listing();
    browsers.narrowed(Some(&driver), 3);
    let narrowed = browsers.listing();
    allowed_and_acting(&browsers, &hosted, 4);
    let before_forget = browsers.listing();
    browsers.forget(Some(&driver), 4);

    assert_ne!(started, at_first);
    assert_eq!(acted, started, "an act starts and ends nothing");
    assert_ne!(another, acted);
    assert_ne!(closed, another);
    assert_ne!(narrowed, closed);
    assert_ne!(browsers.listing(), before_forget);
    assert!(browsers.listed().is_empty());
}

#[test]
fn a_browser_that_ended_by_itself_leaves_the_listing_at_its_chats_next_call() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    allowed_and_acting(&browsers, &hosted, 3);
    hosted.ended.lock().unwrap().push("program-1".into());
    let before = browsers.listing();

    answered(&browsers, &hosted, &ask(3, click()));

    assert_ne!(browsers.listing(), before);
    assert!(browsers.listed().is_empty());
}

#[test]
fn a_browser_not_ended_yet_stays_listed_under_its_chat_as_not_ended_until_it_ends() {
    let browsers = retrying_quickly();
    let hosted = Arc::new(Hosted::default());
    let driver: Arc<dyn Driver> = hosted.clone();
    allowed_and_acting(&browsers, &hosted, 3);
    *hosted.closes_fail.lock().unwrap() = u32::MAX;
    *hosted.stops_fail.lock().unwrap() = u32::MAX;

    browsers.narrowed(Some(&driver), 3);

    assert_eq!(
        browsers.listed(),
        [super::ChatBrowser {
            unended: true,
            ..listed(3, "b1", "program-1")
        }]
    );
    let before = browsers.listing();
    *hosted.closes_fail.lock().unwrap() = 0;
    // The retry tries it again, and it ends.
    until(|| browsers.unended().is_empty());
    assert_ne!(browsers.listing(), before, "its row moves");
    assert!(browsers.listed().is_empty());
}

#[test]
fn stop_all_takes_every_chats_browsers_off_the_listing_and_keeps_the_persons_answers() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    let driver: Arc<dyn Driver> = hosted.clone();
    allowed_and_acting(&browsers, &hosted, 3);
    allowed_and_acting(&browsers, &hosted, 4);
    let before = browsers.listing();

    browsers.all_stopped(Some(&driver));

    assert!(browsers.listed().is_empty());
    assert_ne!(browsers.listing(), before);
    assert_eq!(browsers.posture(3), Posture::Allowed { open_web: false });
    // The next call opens a new one.
    answered(&browsers, &hosted, &ask(3, click()));
    assert_eq!(browsers.listed(), [listed(3, "b2", "program-3")]);
}

#[test]
fn a_browser_a_chat_uses_is_known_as_one_and_one_it_no_longer_uses_is_not() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    allowed_and_acting(&browsers, &hosted, 3);

    assert!(browsers.is_in_use("program-1"));
    assert!(!browsers.is_in_use("program-9"));
    assert!(super::closing_refused([&browsers], "program-1").is_some());
    assert!(super::closing_refused([&browsers], "program-9").is_none());

    answered(&browsers, &hosted, &ask(3, Browsing::Close));
    assert!(!browsers.is_in_use("program-1"));
}

// ---- Reach Asks (#2018) ----

#[test]
fn every_act_tells_the_browser_what_its_chat_reaches_now() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    answered(&browsers, &hosted, &ask(3, click()));
    assert!(browsers.allow(3, true));
    answered(&browsers, &hosted, &ask(3, click()));
    answered(&browsers, &hosted, &ask(3, click()));
    let open_web = BrowserReach {
        open_web: true,
        ..reach()
    };
    assert_eq!(
        *hosted.reached.lock().unwrap(),
        [open_web.clone(), open_web],
        "the chat's reach as it is at each act, with what the person allowed"
    );
}

#[test]
fn an_act_that_asked_about_a_new_site_tells_the_chat_it_asked_and_not_to_retry() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    answered(&browsers, &hosted, &ask(3, click()));
    assert!(browsers.allow(3, false));
    *hosted.asks.lock().unwrap() = vec!["news.example.org:443".into()];
    let (answer, raised) = answered(&browsers, &hosted, &ask(3, click()));
    assert!(!raised, "the browser's Ask was answered already");
    match answer {
        Answer::Browsed { browsed, .. } => {
            assert_eq!(browsed.asked, ["news.example.org:443"]);
            let note = browsed.note.unwrap();
            assert!(note.contains("news.example.org:443"), "{note}");
            assert!(note.contains("asked the person"), "{note}");
            assert!(note.contains("Don't retry"), "{note}");
        }
        other => panic!("{other:?}"),
    }
}

fn site(written: &str) -> purlis_core::sandbox::browser_reach::Site {
    purlis_core::sandbox::browser_reach::Site::parse(written).unwrap()
}

#[test]
fn the_sites_a_browser_asked_about_wait_as_one_ask_and_an_allow_for_the_chat_reaches_its_next_act()
{
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    answered(&browsers, &hosted, &ask(3, click()));
    assert!(browsers.allow(3, false));
    *hosted.asks.lock().unwrap() =
        vec!["news.example.org:443".into(), "cdn.example.net:443".into()];
    answered(&browsers, &hosted, &ask(3, click()));
    hosted.asks.lock().unwrap().clear();

    let waits = browsers.sites_waiting();
    assert_eq!(waits.len(), 1, "one ask for the chat, every site on it");
    assert_eq!(
        waits[0].sites,
        [site("news.example.org:443"), site("cdn.example.net:443")]
    );
    assert!(
        browsers
            .sites_answered(3, &[site("elsewhere.example.org:443")], true)
            .is_err(),
        "an answer is to what was shown"
    );
    browsers
        .sites_answered(3, &[site("news.example.org:443")], true)
        .unwrap();
    assert_eq!(
        browsers.sites_waiting()[0].sites,
        [site("cdn.example.net:443")]
    );

    answered(&browsers, &hosted, &ask(3, click()));
    let reached = hosted.reached.lock().unwrap().last().cloned().unwrap();
    assert!(
        reached
            .listed
            .contains(&purlis_session_protocol::session::Listed {
                host: "news.example.org:443".into(),
                layer: purlis_session_protocol::session::Layer::Chat,
            }),
        "{reached:?}"
    );
    assert_eq!(browsers.chat_sites(), [(3, site("news.example.org:443"))]);
}

#[test]
fn a_chat_without_the_sandbox_is_asked_nothing_and_told_its_browser_reaches_only_what_it_was_given()
{
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    let unconfined = |browse: &BrowseAsk| {
        answered_confined(&browsers, &hosted, browse, &AsPersona::default(), false)
    };
    unconfined(&ask(3, click()));
    assert!(browsers.allow(3, false));
    *hosted.asks.lock().unwrap() = vec!["news.example.org:443".into()];
    let (answer, _) = unconfined(&ask(3, click()));
    assert!(browsers.sites_waiting().is_empty(), "nobody is asked");
    match answer {
        Answer::Browsed { browsed, .. } => {
            let note = browsed.note.unwrap();
            assert!(note.contains("not asked"), "{note}");
            assert!(note.contains("started without the sandbox"), "{note}");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn removing_a_chats_browser_site_ends_its_browsers() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    answered(&browsers, &hosted, &ask(3, click()));
    assert!(browsers.allow(3, false));
    *hosted.asks.lock().unwrap() = vec!["news.example.org:443".into()];
    answered(&browsers, &hosted, &ask(3, click()));
    browsers
        .sites_answered(3, &[site("news.example.org:443")], true)
        .unwrap();
    let driver: Arc<dyn Driver> = hosted.clone();
    let ended = browsers
        .chat_site_removed(Some(&driver), 3, &site("news.example.org:443"))
        .unwrap();
    assert_eq!(ended.ended, 1);
    assert!(browsers.chat_sites().is_empty());
}

#[test]
fn an_allow_takes_only_the_sites_their_ask_names_whole() {
    let long = site(&format!("{}.example.org:443", "a".repeat(150)));
    let short = site("news.example.org:443");
    let waiting = [short.clone(), long.clone()];
    assert!(super::named_whole(&waiting, std::slice::from_ref(&short)));
    assert!(!super::named_whole(&waiting, &[short, long.clone()]));
    assert!(!super::named_whole(
        std::slice::from_ref(&long),
        std::slice::from_ref(&long)
    ));
}

/// Browsers that retry quickly, so a test waits on the retry and not on its timer.
fn retrying_quickly() -> ChatBrowsers {
    ChatBrowsers::retrying(Duration::from_millis(5), Duration::from_millis(20))
}

/// Chat 3 allowed, with a browser `program-1` that could not be ended, and that purlis now
/// retries while the host does not answer.
fn one_stuck(browsers: &ChatBrowsers, hosted: &Arc<Hosted>) {
    let driver: Arc<dyn Driver> = hosted.clone();
    allowed_and_acting(browsers, hosted, 3);
    // Its first try fails (two closes, then a stop by its id); every close after waits.
    *hosted.closes_fail.lock().unwrap() = 2;
    *hosted.stops_fail.lock().unwrap() = 1;
    hosted.hold_closes_from(3);
    browsers.narrowed(Some(&driver), 3);
    assert_eq!(browsers.unended(), ["program-1"]);
    hosted.until_a_close_waits();
}

#[test]
fn a_browser_call_answers_at_once_while_a_browser_of_the_project_is_stuck() {
    let browsers = Arc::new(retrying_quickly());
    let hosted = Arc::new(Hosted::default());
    one_stuck(&browsers, &hosted);

    let (sent, answers) = std::sync::mpsc::channel();
    let calling = {
        let (browsers, hosted) = (Arc::clone(&browsers), Arc::clone(&hosted));
        std::thread::spawn(move || {
            let _ = sent.send(answered(&browsers, &hosted, &ask(3, click())).0);
        })
    };
    let answer = answers.recv_timeout(Duration::from_secs(5));

    hosted.answer_closes();
    calling.join().unwrap();
    let answer = answer.expect("the call answers while the stuck browser is retried");
    assert!(matches!(answer, Answer::Browsed { .. }), "{answer:?}");
}

#[test]
fn a_stuck_browser_being_retried_still_counts_toward_the_projects_three() {
    let browsers = Arc::new(retrying_quickly());
    let hosted = Arc::new(Hosted::default());
    one_stuck(&browsers, &hosted);
    // Two more run beside the stuck one: three in all.
    answered(&browsers, &hosted, &ask(3, click()));
    answered(&browsers, &hosted, &ask(3, Browsing::New));

    let (sent, answers) = std::sync::mpsc::channel();
    let calling = {
        let (browsers, hosted) = (Arc::clone(&browsers), Arc::clone(&hosted));
        std::thread::spawn(move || {
            let _ = sent.send(answered(&browsers, &hosted, &ask(3, Browsing::New)).0);
        })
    };
    let fourth = answers.recv_timeout(Duration::from_secs(5));

    hosted.answer_closes();
    calling.join().unwrap();
    let why = refusal(fourth.expect("the call answers"));
    assert!(why.contains("this project"), "{why}");
    assert_eq!(hosted.started.lock().unwrap().len(), 3);
}

#[test]
fn a_stuck_browser_is_ended_off_the_calls_path_once_the_host_answers_and_its_row_goes() {
    let told = Arc::new(Mutex::new(0u32));
    let browsers = {
        let told = Arc::clone(&told);
        retrying_quickly().telling(Arc::new(move || *told.lock().unwrap() += 1))
    };
    let hosted = Arc::new(Hosted::default());
    one_stuck(&browsers, &hosted);
    let before = browsers.listing();

    hosted.answer_closes();

    until(|| browsers.unended().is_empty());
    assert_eq!(*hosted.closed.lock().unwrap(), ["program-1"]);
    assert_ne!(browsers.listing(), before, "its row moves");
    assert!(browsers.listed().is_empty());
    until(|| *told.lock().unwrap() > 0);
}

/// [`answered`] at `now`, in seconds since 1970.
fn answered_at(
    browsers: &ChatBrowsers,
    hosted: &Arc<Hosted>,
    browse: &BrowseAsk,
    now: u64,
) -> Answer {
    let driver: Arc<dyn Driver> = hosted.clone();
    browsers
        .answer(
            Some(&driver),
            Path::new("/p"),
            browse,
            &|| reach(),
            true,
            &AsPersona::default,
            now,
        )
        .0
}

/// Chat 3's browser allowed, and one act of it carried out at 1000 (`program-1`).
fn used_at_1000(browsers: &ChatBrowsers, hosted: &Arc<Hosted>) -> Arc<dyn Driver> {
    allowed_and_acting(browsers, hosted, 3);
    hosted.clone()
}

#[test]
fn a_chats_browser_unused_for_ten_minutes_stops_and_its_next_call_is_told() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    let driver = used_at_1000(&browsers, &hosted);
    let before = browsers.listing();

    assert!(
        browsers.idle(Some(&driver), 1599, &[]).is_empty(),
        "not yet"
    );
    let stopped = browsers.idle(Some(&driver), 1600, &[]);

    assert_eq!(
        stopped,
        [(
            3,
            Ended {
                ended: 1,
                unended: 0
            }
        )]
    );
    assert_eq!(*hosted.closed.lock().unwrap(), ["program-1"]);
    assert!(browsers.listed().is_empty());
    assert_ne!(browsers.listing(), before, "its row goes");
    let answer = answered_at(&browsers, &hosted, &ask(3, click()), 1700);
    let Answer::Browsed { browser, browsed } = answer else {
        panic!("a new browser takes the act: {answer:?}");
    };
    assert_eq!(browser, "b2");
    let note = browsed.note.unwrap_or_default();
    assert!(note.contains("b1"), "{note}");
    assert!(note.contains("10 minutes"), "{note}");
    // Told once.
    let again = answered_at(&browsers, &hosted, &ask(3, click()), 1701);
    assert!(
        matches!(&again, Answer::Browsed { browsed, .. } if browsed.note.is_none()),
        "{again:?}"
    );
}

#[test]
fn a_chat_that_names_its_stopped_browser_is_told_why_it_has_none() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    let driver = used_at_1000(&browsers, &hosted);
    browsers.idle(Some(&driver), 1600, &[]);

    let why = refusal(answered_at(
        &browsers,
        &hosted,
        &BrowseAsk {
            browser: Some("b1".into()),
            ..ask(3, click())
        },
        1700,
    ));

    assert!(why.contains("10 minutes"), "{why}");
    assert!(why.contains("no browser b1"), "{why}");
}

#[test]
fn a_browser_the_chat_or_the_person_used_lately_does_not_stop() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    let driver = used_at_1000(&browsers, &hosted);
    answered_at(&browsers, &hosted, &ask(3, click()), 1300);
    assert!(
        browsers.idle(Some(&driver), 1600, &[]).is_empty(),
        "the chat acted at 1300"
    );

    browsers.touched("program-1", 1800);

    assert!(
        browsers.idle(Some(&driver), 2300, &[]).is_empty(),
        "the person used it at 1800"
    );
    assert_eq!(browsers.idle(Some(&driver), 2400, &[]).len(), 1);
}

#[test]
fn a_browser_the_person_drives_or_that_waits_on_them_never_stops_for_being_unused() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    let driver = used_at_1000(&browsers, &hosted);

    browsers.driven("program-1", true, 1000);
    assert!(
        browsers.idle(Some(&driver), 9000, &[]).is_empty(),
        "the person drives it"
    );
    browsers.driven("program-1", false, 9000);
    assert!(
        browsers
            .idle(Some(&driver), 99_000, &["program-1".to_owned()])
            .is_empty(),
        "an Ask about it waits on the person"
    );

    assert_eq!(browsers.idle(Some(&driver), 99_000, &[]).len(), 1);
}

#[test]
fn the_idle_time_is_the_one_its_browsers_are_held_to() {
    let browsers = ChatBrowsers::default().limited(purlis_core::browserlimits::Limits {
        idle_after: std::time::Duration::from_secs(120),
        ..purlis_core::browserlimits::Limits::for_memory(None)
    });
    let hosted = Arc::new(Hosted::default());
    let driver = used_at_1000(&browsers, &hosted);

    assert_eq!(browsers.idle(Some(&driver), 1120, &[]).len(), 1);
}

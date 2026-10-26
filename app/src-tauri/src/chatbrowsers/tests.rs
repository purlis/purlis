// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use purlis_core::hookwire::{Answer, BrowseAsk, Browsing};
use purlis_session_protocol::browsing::{Act, Browsed, BrowserReach, LogIn, Secret};

use purlis_core::sandbox::persona::Browser;

use super::{
    AsPersona, BrowserDriver, ChatBrowsers, Driver, Ended, HandOver, HeldLogin, NotStarted,
    Posture, Refused,
};

/// A driver that stands in for the session host: it starts numbered browsers, answers every
/// act settled, and keeps what it was asked.
#[derive(Default)]
struct Hosted {
    started: Mutex<Vec<BrowserReach>>,
    /// The pool each browser started on, in order, or none for this machine (#2050).
    on_pools: Mutex<Vec<Option<String>>>,
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
    /// The address of each login it performed, and its program (#2016).
    logins: Mutex<Vec<(String, String)>>,
    /// The kept login each browser started from, in order (#2021).
    kept: Mutex<Vec<Option<purlis_session_protocol::browsing::KeptLogin>>>,
    /// How many closes, and how many stops by id, fail from now.
    closes_fail: Mutex<u32>,
    stops_fail: Mutex<u32>,
    /// How many reads of a browser's pages, and how many logins, fail from now (#2022).
    pages_fail: Mutex<u32>,
    login_fails: Mutex<u32>,
    /// Where the page each act answers is, if anywhere (#2022).
    at: Mutex<Option<String>>,
    /// The device's budget, where the host says it is reached.
    full: Mutex<Option<usize>>,
    /// Where set, every start is refused so (#2207).
    refuses: Mutex<Option<NotStarted>>,
    /// The browsers the host lists with a driver (#2023), and whether its list fails.
    host_drives: Mutex<Vec<String>>,
    list_fails: Mutex<bool>,
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
    fn start(
        &self,
        _root: &Path,
        pool: Option<&str>,
        reach: BrowserReach,
        kept: Option<purlis_session_protocol::browsing::KeptLogin>,
    ) -> Result<String, NotStarted> {
        if let Some(most) = *self.full.lock().unwrap() {
            return Err(NotStarted::Full { most });
        }
        if let Some(not) = self.refuses.lock().unwrap().clone() {
            return Err(not);
        }
        self.kept.lock().unwrap().push(kept);
        self.on_pools.lock().unwrap().push(pool.map(str::to_owned));
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
        let pages = matches!(act, Act::Pages);
        self.acts
            .lock()
            .unwrap()
            .push((program.to_owned(), call_id.to_owned(), act));
        // The host takes the act's reach before it acts, as the session host does.
        self.reached.lock().unwrap().push(reach);
        if pages && fails(&self.pages_fail) {
            return Err(Refused::Said("the page did not answer in time".into()));
        }
        Ok(Browsed {
            settled: true,
            asked: self.asks.lock().unwrap().clone(),
            page: self.at.lock().unwrap().clone().map(|address| {
                purlis_session_protocol::browsing::PageSaid {
                    page: "p1".into(),
                    address,
                    title: "A page".into(),
                    current: true,
                }
            }),
            ..Browsed::default()
        })
    }

    fn log_in(&self, program: &str, login: LogIn) -> Result<Browsed, Refused> {
        if fails(&self.login_fails) {
            return Err(Refused::Said(
                "the password field did not go within the time allowed".into(),
            ));
        }
        self.logins
            .lock()
            .unwrap()
            .push((program.to_owned(), login.address.clone()));
        Ok(Browsed {
            settled: true,
            note: Some("the password field is gone from https://shop.example.test since purlis submitted the login".into()),
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

    fn driven(&self) -> Result<Vec<String>, String> {
        if *self.list_fails.lock().unwrap() {
            return Err("the session host did not answer in time".into());
        }
        Ok(self.host_drives.lock().unwrap().clone())
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
    answered_started(browsers, hosted, browse, as_persona, false)
}

/// [`answered_as`] for a chat the app started without the sandbox, or with it (#2242).
fn answered_started(
    browsers: &ChatBrowsers,
    hosted: &Arc<Hosted>,
    browse: &BrowseAsk,
    as_persona: &AsPersona,
    unsandboxed: bool,
) -> (Answer, bool) {
    let driver: Arc<dyn Driver> = hosted.clone();
    browsers.answer(
        Some(&driver),
        Path::new("/p"),
        None,
        browse,
        &|| reach(),
        unsandboxed,
        &|| as_persona.clone(),
        &logins,
        1000,
    )
}

/// The one browser login a test chat has: `shop-login`, its values a test's own (#2016).
fn logins(name: &str) -> super::ReadLogin {
    // `shop-login` and `other-login` have no kept store yet; `kept-login` has one to open
    // (#2021).
    if !["shop-login", "other-login", "kept-login"].contains(&name) {
        return Err(format!("the persona declares no browser login {name}"));
    }
    Ok(super::LoginRead {
        login: LogIn {
            address: "https://shop.example.test/login".into(),
            user: None,
            password: Secret::new(LOGIN_VALUE.into()),
            keep: Some(kept_as(name)),
        },
        opens: name == "kept-login",
        identity: IDENTITY.into(),
    })
}

/// The identity of each test login, as declared and allowed (#2699).
const IDENTITY: &str = "0123456789abcdef0123456789abcdef";

fn kept_as(name: &str) -> purlis_session_protocol::browsing::KeptLogin {
    purlis_session_protocol::browsing::KeptLogin {
        profile: "claude".into(),
        persona: "steward".into(),
        login: name.into(),
        identity: IDENTITY.into(),
        address: "https://shop.example.test/login".into(),
    }
}

/// What the test login fills: a value no answer to the chat may carry.
const LOGIN_VALUE: &str = "fixture-word";

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
            unsandboxed: false,
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

    assert!(
        why.contains("purlis runs at most 8 browsers at once on this device"),
        "{why}"
    );
    assert!(why.contains("browser_close"), "{why}");
    assert!(!why.contains("Try once more"), "{why}");
}

#[test]
fn a_browser_whose_engine_download_the_person_cancelled_is_refused_in_words_that_say_not_to_retry()
{
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    answered(&browsers, &hosted, &ask(3, click()));
    browsers.allow(3, false);
    *hosted.refuses.lock().unwrap() = Some(NotStarted::Cancelled);

    let why = refusal(answered(&browsers, &hosted, &ask(3, Browsing::New)).0);

    assert!(
        why.contains("The person cancelled the browser engine's download"),
        "{why}"
    );
    assert!(why.contains("Don't retry"), "{why}");
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
        None,
        &ask(3, click()),
        &|| narrower.clone(),
        false,
        &AsPersona::default,
        &logins,
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
    browsers.forget(Some(&driver), 3, "Chat 3");

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
                None,
                &ask(3, click()),
                &|| reach(),
                false,
                &AsPersona::default,
                &logins,
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
        ..super::ChatBrowser::default()
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
    browsers.forget(Some(&driver), 4, "Chat 4");

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

/// #2242: a chat started without the sandbox gets a browser like a sandboxed one, so a site
/// new to the project its browser held back waits on the person as any chat's does.
#[test]
fn a_chat_without_the_sandbox_is_asked_about_a_new_site_as_any_chat_is() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    let none = AsPersona::default();
    answered_started(&browsers, &hosted, &ask(3, click()), &none, true);
    assert!(browsers.waiting()[0].unsandboxed, "its Ask knows it");
    assert!(browsers.allow(3, false));
    *hosted.asks.lock().unwrap() = vec!["news.example.org:443".into()];

    let (answer, _) = answered_started(&browsers, &hosted, &ask(3, click()), &none, true);

    assert_eq!(
        browsers.sites_waiting()[0].sites,
        [site("news.example.org:443")]
    );
    match answer {
        Answer::Browsed { browsed, .. } => {
            let note = browsed.note.unwrap();
            assert!(note.contains("asked the person"), "{note}");
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

// ---- Logins purlis performs (#2016) ----

fn log_in(name: &str) -> Browsing {
    Browsing::LogIn { login: name.into() }
}

#[test]
fn a_browser_login_is_performed_in_the_chats_browser_and_no_value_reaches_the_chat() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    answered(&browsers, &hosted, &ask(3, log_in("shop-login")));
    browsers.allow(3, false);
    let (answer, _) = answered(&browsers, &hosted, &ask(3, log_in("shop-login")));
    let Answer::Browsed { browser, browsed } = answer else {
        panic!("{answer:?}");
    };
    assert_eq!(browser, "b1");
    let note = browsed.note.clone().unwrap_or_default();
    assert!(
        note.contains("since purlis submitted the login, with shop-login: take a snapshot"),
        "{note}"
    );
    assert!(note.contains("reaches that site alone"), "{note}");
    assert!(!format!("{browsed:?}").contains(LOGIN_VALUE));
    assert_eq!(
        *hosted.logins.lock().unwrap(),
        [(
            "program-1".to_owned(),
            "https://shop.example.test/login".to_owned()
        )]
    );
    assert!(
        hosted.acts.lock().unwrap().is_empty(),
        "a login is no act of the agent's"
    );
}

#[test]
fn a_login_the_chat_has_none_of_is_refused_and_nothing_is_filled() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    answered(&browsers, &hosted, &ask(3, log_in("mail-login")));
    browsers.allow(3, false);
    let why = refusal(answered(&browsers, &hosted, &ask(3, log_in("mail-login"))).0);
    assert!(
        why.contains("declares no browser login mail-login"),
        "{why}"
    );
    assert!(hosted.logins.lock().unwrap().is_empty());
}

#[test]
fn a_login_waits_for_the_chats_browser_ask_like_any_browser_call() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    let (first, raised) = answered(&browsers, &hosted, &ask(3, log_in("shop-login")));
    assert!(raised);
    assert!(refusal(first).contains("asked the person"));
    browsers.keep_off(3);
    let (off, _) = answered(&browsers, &hosted, &ask(3, log_in("shop-login")));
    assert!(matches!(off, Answer::No { .. }));
    assert!(hosted.logins.lock().unwrap().is_empty());
    assert!(hosted.started.lock().unwrap().is_empty());
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
            None,
            browse,
            &|| reach(),
            true,
            &AsPersona::default,
            &logins,
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
        idle_after: Some(std::time::Duration::from_secs(120)),
        ..purlis_core::browserlimits::Limits::for_memory(None)
    });
    let hosted = Arc::new(Hosted::default());
    let driver = used_at_1000(&browsers, &hosted);

    assert_eq!(browsers.idle(Some(&driver), 1120, &[]).len(), 1);
}

#[test]
fn a_browser_the_host_lists_with_a_driver_never_stops_for_being_unused() {
    // The host's own word: a take-over the window has not heard back from, or a login.
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    let driver = used_at_1000(&browsers, &hosted);
    hosted.host_drives.lock().unwrap().push("program-1".into());

    assert!(browsers.idle(Some(&driver), 99_000, &[]).is_empty());

    hosted.host_drives.lock().unwrap().clear();
    assert_eq!(browsers.idle(Some(&driver), 99_000, &[]).len(), 1);
}

#[test]
fn a_host_that_cannot_say_what_it_drives_has_nothing_stopped_for_being_unused() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    let driver = used_at_1000(&browsers, &hosted);
    *hosted.list_fails.lock().unwrap() = true;

    assert!(browsers.idle(Some(&driver), 99_000, &[]).is_empty());
    assert!(hosted.closed.lock().unwrap().is_empty());
}

#[test]
fn a_login_is_a_use_and_keeps_its_browser_from_stopping() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    let driver = used_at_1000(&browsers, &hosted);

    answered_at(&browsers, &hosted, &ask(3, log_in("shop-login")), 1500);

    assert!(
        browsers.idle(Some(&driver), 2000, &[]).is_empty(),
        "logged in at 1500"
    );
    assert_eq!(browsers.idle(Some(&driver), 2100, &[]).len(), 1);
}

#[test]
fn with_idle_minutes_set_to_never_no_browser_stops_for_being_unused() {
    let browsers = ChatBrowsers::default().limited(purlis_core::browserlimits::Limits {
        idle_after: None,
        ..purlis_core::browserlimits::Limits::for_memory(None)
    });
    let hosted = Arc::new(Hosted::default());
    let driver = used_at_1000(&browsers, &hosted);

    assert!(browsers.idle(Some(&driver), 999_999, &[]).is_empty());
}

#[test]
fn the_note_of_browsers_stopped_unused_names_each_and_the_setting() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    let driver = used_at_1000(&browsers, &hosted);
    answered(&browsers, &hosted, &ask(3, Browsing::New));
    browsers.idle(Some(&driver), 1600, &[]);

    let answer = answered_at(&browsers, &hosted, &ask(3, click()), 1700);

    let Answer::Browsed { browsed, .. } = answer else {
        panic!("{answer:?}");
    };
    let note = browsed.note.unwrap_or_default();
    assert!(note.contains("browsers b1, b2 were stopped"), "{note}");
    assert!(note.contains("what they held is gone"), "{note}");
    assert!(
        note.contains("Settings › This machine › Browsers"),
        "{note}"
    );
}

#[test]
fn the_projects_browser_sites_are_listed_for_its_browser_at_the_projects_layer() {
    use purlis_session_protocol::session::{Layer, Listed};
    let site = purlis_core::sandbox::hosts::Host::parse("news.example.org:443").unwrap();
    let reach = super::with_project_sites(BrowserReach::default(), &[site]);
    assert_eq!(
        reach.listed,
        [Listed {
            host: "news.example.org:443".into(),
            layer: Layer::Project,
        }]
    );
}

#[test]
fn a_pinned_browser_outlives_its_chat_and_ends_with_stop_all() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    let driver: Arc<dyn Driver> = hosted.clone();
    allowed_and_acting(&browsers, &hosted, 3);

    browsers.pin(3, "program-1", true).unwrap();
    browsers.forget(Some(&driver), 3, "Fix the login");

    assert!(hosted.closed.lock().unwrap().is_empty(), "it runs on");
    assert_eq!(
        browsers.listed(),
        [super::ChatBrowser {
            pinned: true,
            was: Some("Fix the login".into()),
            ..listed(3, "b1", "program-1")
        }]
    );
    assert!(
        browsers.is_kept("program-1"),
        "the window keeps it past a closed tab"
    );
    assert!(
        super::closing_refused([&browsers], "program-1").is_none(),
        "the person may stop it"
    );
    browsers.all_stopped(Some(&driver));
    assert_eq!(*hosted.closed.lock().unwrap(), ["program-1"]);
    assert!(browsers.listed().is_empty());
}

#[test]
fn a_pinned_browser_is_listed_as_pinned_while_its_chat_uses_it_and_unpinned_ends_with_it() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    let driver: Arc<dyn Driver> = hosted.clone();
    allowed_and_acting(&browsers, &hosted, 3);
    let before = browsers.listing();

    browsers.pin(3, "program-1", true).unwrap();
    assert_ne!(browsers.listing(), before, "its row moves");
    assert_eq!(
        browsers.listed(),
        [super::ChatBrowser {
            pinned: true,
            ..listed(3, "b1", "program-1")
        }]
    );
    browsers.pin(3, "program-1", false).unwrap();
    browsers.forget(Some(&driver), 3, "Fix the login");

    assert_eq!(*hosted.closed.lock().unwrap(), ["program-1"]);
    assert!(browsers.listed().is_empty());
}

#[test]
fn only_a_chats_own_browser_is_pinned_by_it() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    allowed_and_acting(&browsers, &hosted, 3);

    let why = browsers.pin(4, "program-1", true).unwrap_err();

    assert!(why.contains("no longer"), "{why}");
    assert!(browsers.listed().iter().all(|one| !one.pinned));
}

#[test]
fn a_pinned_browser_never_stops_for_being_unused() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    let driver: Arc<dyn Driver> = hosted.clone();
    allowed_and_acting(&browsers, &hosted, 3);
    browsers.pin(3, "program-1", true).unwrap();

    assert!(browsers.idle(Some(&driver), 99_000, &[]).is_empty());
    browsers.forget(Some(&driver), 3, "Fix the login");
    assert!(browsers.idle(Some(&driver), 99_000, &[]).is_empty());
    assert!(hosted.closed.lock().unwrap().is_empty());
}

#[test]
fn a_browser_with_no_chat_is_handed_to_a_chat_which_is_told_and_uses_it() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    allowed_and_acting(&browsers, &hosted, 4);
    let before = browsers.listing();

    let name = hand(&browsers, &hosted, "program-person", 4).unwrap();

    assert_eq!(name, "b2");
    assert_ne!(browsers.listing(), before);
    assert!(
        browsers.is_in_use("program-person"),
        "it ends with its chat now"
    );
    let answer = answered_at(&browsers, &hosted, &ask(4, click()), 2001);
    let Answer::Browsed { browser, browsed } = answer else {
        panic!("{answer:?}");
    };
    assert_eq!(browser, "b2", "the browser tools use it from now");
    let note = browsed.note.unwrap_or_default();
    assert!(note.contains("the person handed"), "{note}");
    assert_eq!(
        hosted.acts.lock().unwrap().last().unwrap().0,
        "program-person"
    );
}

#[test]
fn a_pinned_browser_whose_chat_closed_is_handed_to_another_chat() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    let driver: Arc<dyn Driver> = hosted.clone();
    allowed_and_acting(&browsers, &hosted, 3);
    allowed_and_acting(&browsers, &hosted, 4);
    browsers.pin(3, "program-1", true).unwrap();
    browsers.forget(Some(&driver), 3, "Fix the login");

    hand(&browsers, &hosted, "program-1", 4).unwrap();

    assert_eq!(
        browsers.listed(),
        [
            listed(4, "b1", "program-2"),
            super::ChatBrowser {
                pinned: true,
                ..listed(4, "b2", "program-1")
            },
        ]
    );
    assert!(!browsers.is_kept("program-1"));
    browsers.forget(Some(&driver), 4, "Other");
    assert_eq!(
        *hosted.closed.lock().unwrap(),
        ["program-2"],
        "still pinned"
    );
}

#[test]
fn a_browser_a_chat_uses_or_past_the_projects_limit_is_not_handed_over() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    allowed_and_acting(&browsers, &hosted, 3);
    allowed_and_acting(&browsers, &hosted, 4);

    let why = hand(&browsers, &hosted, "program-1", 4).unwrap_err();
    assert!(why.contains("another chat"), "{why}");

    answered(&browsers, &hosted, &ask(4, Browsing::New));
    let why = hand(&browsers, &hosted, "program-person", 4).unwrap_err();
    assert!(why.contains("3 browsers"), "{why}");
}

#[test]
fn a_pinned_browser_the_person_stopped_leaves_the_listing() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    let driver: Arc<dyn Driver> = hosted.clone();
    allowed_and_acting(&browsers, &hosted, 3);
    browsers.pin(3, "program-1", true).unwrap();
    browsers.forget(Some(&driver), 3, "Fix the login");
    let before = browsers.listing();

    assert!(browsers.stopped_by_the_person("program-1"));

    assert!(browsers.listed().is_empty());
    assert_ne!(browsers.listing(), before);
}

#[test]
fn the_addresses_of_the_open_browsers_are_what_their_last_acts_answered() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    let driver: Arc<dyn Driver> = hosted.clone();
    *hosted.at.lock().unwrap() = Some("https://staging.example.test/cart".into());
    allowed_and_acting(&browsers, &hosted, 3);
    *hosted.at.lock().unwrap() = Some("https://docs.example.test/".into());
    allowed_and_acting(&browsers, &hosted, 4);
    browsers.pin(4, "program-2", true).unwrap();
    browsers.forget(Some(&driver), 4, "Docs");

    assert_eq!(
        browsers.open_addresses(),
        [
            "https://staging.example.test/cart",
            "https://docs.example.test/"
        ]
    );
    browsers.narrowed(Some(&driver), 3);
    assert_eq!(browsers.open_addresses(), ["https://docs.example.test/"]);
}

#[test]
fn the_browsers_kept_at_the_last_quit_are_read_once_and_kept_until_the_person_puts_them_away() {
    let dir = tempfile::tempdir().expect("a project");
    let root = dir.path();
    purlis_core::browsersatquit::keep(root, &["https://shop.example.test/".to_owned()])
        .expect("kept at quit");
    let browsers = ChatBrowsers::default();

    assert_eq!(browsers.at_quit(root), ["https://shop.example.test/"]);
    assert_eq!(
        browsers.at_quit(root),
        ["https://shop.example.test/"],
        "a window drawn again sees them still"
    );
    assert!(
        purlis_core::browsersatquit::take(root).is_empty(),
        "offered once: the file went"
    );
    browsers.put_away_at_quit();
    assert!(browsers.at_quit(root).is_empty());
}

/// The reach a chat handed a browser is given: one host of its own, to tell it apart.
fn handed_reach() -> BrowserReach {
    BrowserReach {
        listed: vec![purlis_session_protocol::session::Listed {
            host: "chat-four.example.test".into(),
            layer: purlis_session_protocol::session::Layer::Chat,
        }],
        ..BrowserReach::default()
    }
}

/// Hands `program` to chat `session` at 2000, the chat reaching [`handed_reach`], and any login
/// one it may log in with.
fn hand(
    browsers: &ChatBrowsers,
    hosted: &Arc<Hosted>,
    program: &str,
    session: u32,
) -> Result<String, String> {
    hand_checked(browsers, hosted, program, session, &|_| Ok(()))
}

fn hand_checked(
    browsers: &ChatBrowsers,
    hosted: &Arc<Hosted>,
    program: &str,
    session: u32,
    may_log_in: &dyn Fn(&HeldLogin) -> Result<(), String>,
) -> Result<String, String> {
    let driver: Arc<dyn Driver> = hosted.clone();
    browsers.hand_over(
        Some(&driver),
        Path::new("/p"),
        HandOver {
            program,
            session,
            now: 2000,
            reach: handed_reach(),
        },
        may_log_in,
    )
}

/// Chat 3, as persona qa, signed in with shop-login, its browser pinned, and the chat closed.
fn signed_in_and_kept(browsers: &ChatBrowsers, hosted: &Arc<Hosted>) {
    let driver: Arc<dyn Driver> = hosted.clone();
    let qa = as_persona("qa", None, None);
    answered_as(browsers, hosted, &ask(3, click()), &qa);
    browsers.allow(3, false);
    answered_as(browsers, hosted, &ask(3, log_in("shop-login")), &qa);
    browsers.pin(3, "program-1", true).unwrap();
    browsers.forget(Some(&driver), 3, "Fix the login");
}

#[test]
fn a_signed_in_browser_is_listed_with_its_login() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());

    signed_in_and_kept(&browsers, &hosted);

    assert_eq!(browsers.listed()[0].login.as_deref(), Some("shop-login"));
}

#[test]
fn a_signed_in_browser_goes_only_to_a_chat_that_may_log_in_with_that_login_now() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    signed_in_and_kept(&browsers, &hosted);
    allowed_and_acting(&browsers, &hosted, 4);
    let asked: Mutex<Vec<HeldLogin>> = Mutex::default();

    let why = hand_checked(&browsers, &hosted, "program-1", 4, &|login| {
        asked.lock().unwrap().push(login.clone());
        Err("that chat's persona may not log in with it".into())
    })
    .unwrap_err();

    assert!(why.contains("shop-login"), "{why}");
    assert_eq!(
        *asked.lock().unwrap(),
        [HeldLogin {
            persona: Some("qa".into()),
            login: "shop-login".into(),
            identity: IDENTITY.into(),
        }]
    );
    assert!(browsers.is_kept("program-1"), "it stays with no chat");
    assert!(!browsers.is_in_use("program-1"));

    hand_checked(&browsers, &hosted, "program-1", 4, &|_| Ok(())).expect("allowed now");
    assert!(browsers.is_in_use("program-1"));
}

#[test]
fn a_handed_browser_reaches_what_its_new_chat_reaches_at_once() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    allowed_and_acting(&browsers, &hosted, 4);

    hand(&browsers, &hosted, "program-person", 4).expect("handed");

    let (program, _, act) = hosted.acts.lock().unwrap().last().cloned().expect("an act");
    assert_eq!(program, "program-person");
    assert!(matches!(act, Act::Pages), "{act:?}");
    let reached = hosted
        .reached
        .lock()
        .unwrap()
        .last()
        .cloned()
        .expect("a reach");
    assert!(
        reached
            .listed
            .iter()
            .any(|one| one.host == "chat-four.example.test"),
        "{reached:?}"
    );
}

#[test]
fn a_browser_that_cannot_be_given_its_new_chats_reach_is_not_handed_over() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    allowed_and_acting(&browsers, &hosted, 4);
    hosted.ended.lock().unwrap().push("program-person".into());

    let why = hand(&browsers, &hosted, "program-person", 4).unwrap_err();

    assert!(why.contains("not handed over"), "{why}");
    assert!(!browsers.is_in_use("program-person"));
}

#[test]
fn a_browser_purlis_is_still_ending_is_not_handed_over() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    let driver: Arc<dyn Driver> = hosted.clone();
    allowed_and_acting(&browsers, &hosted, 3);
    allowed_and_acting(&browsers, &hosted, 4);
    *hosted.closes_fail.lock().unwrap() = u32::MAX;
    *hosted.stops_fail.lock().unwrap() = u32::MAX;
    browsers.narrowed(Some(&driver), 3);

    let why = hand(&browsers, &hosted, "program-1", 4).unwrap_err();

    assert!(why.contains("still ending"), "{why}");
}

#[test]
fn no_address_is_kept_at_quit_of_a_browser_that_held_a_login_or_the_persons_typing() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    *hosted.at.lock().unwrap() = Some("https://shop.example.test/account".into());
    signed_in_and_kept(&browsers, &hosted);
    *hosted.at.lock().unwrap() = Some("https://docs.example.test/".into());
    allowed_and_acting(&browsers, &hosted, 4);
    *hosted.at.lock().unwrap() = Some("https://news.example.test/".into());
    allowed_and_acting(&browsers, &hosted, 5);

    browsers.driven("program-3", true, 3000);

    assert_eq!(browsers.open_addresses(), ["https://docs.example.test/"]);
}

#[test]
fn a_login_that_did_not_answer_ok_still_marks_its_browser_signed_in() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    let driver: Arc<dyn Driver> = hosted.clone();
    let qa = as_persona("qa", None, None);
    answered_as(&browsers, &hosted, &ask(3, click()), &qa);
    browsers.allow(3, false);
    *hosted.login_fails.lock().unwrap() = 1;

    let tried = answered_as(&browsers, &hosted, &ask(3, log_in("shop-login")), &qa);

    assert!(matches!(tried.0, Answer::No { .. }), "{tried:?}");
    browsers.pin(3, "program-1", true).unwrap();
    browsers.forget(Some(&driver), 3, "Fix the login");
    assert_eq!(browsers.listed()[0].login.as_deref(), Some("shop-login"));
    allowed_and_acting(&browsers, &hosted, 4);
    let why = hand_checked(&browsers, &hosted, "program-1", 4, &|login| {
        assert_eq!(login.persona.as_deref(), Some("qa"));
        Err("that chat runs as another persona".into())
    })
    .unwrap_err();
    assert!(why.contains("shop-login"), "{why}");
    assert!(browsers.is_kept("program-1"));
}

#[test]
fn a_failed_hand_over_gives_the_browser_back_the_reach_it_had() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    let driver: Arc<dyn Driver> = hosted.clone();
    allowed_and_acting(&browsers, &hosted, 3);
    browsers.pin(3, "program-1", true).unwrap();
    browsers.forget(Some(&driver), 3, "Fix the login");
    allowed_and_acting(&browsers, &hosted, 4);
    *hosted.pages_fail.lock().unwrap() = 1;

    let why = hand(&browsers, &hosted, "program-1", 4).unwrap_err();

    assert!(why.contains("not handed over"), "{why}");
    let last = hosted
        .reached
        .lock()
        .unwrap()
        .last()
        .cloned()
        .expect("a reach");
    assert_eq!(last, reach(), "the reach chat 3 last gave it");
    assert!(browsers.is_kept("program-1"));
}

#[test]
fn a_browser_whose_reach_cannot_be_given_back_is_ended() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    let driver: Arc<dyn Driver> = hosted.clone();
    allowed_and_acting(&browsers, &hosted, 3);
    browsers.pin(3, "program-1", true).unwrap();
    browsers.forget(Some(&driver), 3, "Fix the login");
    allowed_and_acting(&browsers, &hosted, 4);
    *hosted.pages_fail.lock().unwrap() = 2;

    let why = hand(&browsers, &hosted, "program-1", 4).unwrap_err();

    assert!(why.contains("ended it"), "{why}");
    assert!(!browsers.is_kept("program-1"));
    assert!(
        hosted
            .closed
            .lock()
            .unwrap()
            .contains(&"program-1".to_owned())
    );
}

#[test]
fn what_is_recorded_of_a_browser_goes_when_it_ends() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    let driver: Arc<dyn Driver> = hosted.clone();
    *hosted.at.lock().unwrap() = Some("https://docs.example.test/".into());
    let qa = as_persona("qa", None, None);
    answered_as(&browsers, &hosted, &ask(3, click()), &qa);
    browsers.allow(3, false);
    answered_as(&browsers, &hosted, &ask(3, log_in("shop-login")), &qa);
    browsers.driven("program-1", true, 1000);

    browsers.forget(Some(&driver), 3, "Fix the login");

    assert_eq!(browsers.records_of("program-1"), (false, false, false));
}

// ---- Kept logins (#2021) ----

#[test]
fn a_login_with_a_kept_store_opens_in_a_browser_of_its_own_from_it() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    answered(&browsers, &hosted, &ask(3, click()));
    browsers.allow(3, false);
    answered(&browsers, &hosted, &ask(3, click()));
    let (answer, _) = answered(&browsers, &hosted, &ask(3, log_in("kept-login")));
    let Answer::Browsed { browser, .. } = answer else {
        panic!("{answer:?}");
    };
    assert_eq!(browser, "b2", "a browser of its own");
    assert_eq!(
        *hosted.kept.lock().unwrap(),
        [None, Some(kept_as("kept-login"))]
    );
    assert_eq!(
        hosted
            .logins
            .lock()
            .unwrap()
            .last()
            .map(|(program, _)| program.clone()),
        Some("program-2".to_owned()),
        "the login goes into the browser opened from its store"
    );
}

#[test]
fn a_browser_opened_from_a_kept_store_is_handed_only_to_a_chat_with_that_stores_identity() {
    // #2699: it holds the store's login from its first moment, identity and all.
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    let driver: Arc<dyn Driver> = hosted.clone();
    let steward = as_persona("steward", None, None);
    answered_as(&browsers, &hosted, &ask(3, click()), &steward);
    browsers.allow(3, false);
    answered_as(&browsers, &hosted, &ask(3, click()), &steward);
    // Its own browser, from the store: program-2 beside the chat's program-1.
    answered_as(&browsers, &hosted, &ask(3, log_in("kept-login")), &steward);
    browsers.pin(3, "program-2", true).unwrap();
    browsers.forget(Some(&driver), 3, "Fix the login");
    allowed_and_acting(&browsers, &hosted, 4);
    let held: Mutex<Vec<HeldLogin>> = Mutex::default();
    let declared_otherwise = |login: &HeldLogin| {
        held.lock().unwrap().push(login.clone());
        login.may_go_to(Some("steward"), Ok("another-identity".into()))
    };

    let why = hand_checked(&browsers, &hosted, "program-2", 4, &declared_otherwise).unwrap_err();

    assert!(why.contains("declared otherwise"), "{why}");
    let kept = kept_as("kept-login");
    assert_eq!(
        *held.lock().unwrap(),
        [HeldLogin {
            persona: Some(kept.persona),
            login: kept.login,
            identity: kept.identity,
        }]
    );
    assert!(!browsers.is_in_use("program-2"), "it stays with no chat");
    hand_checked(&browsers, &hosted, "program-2", 4, &|login| {
        login.may_go_to(Some("steward"), Ok(IDENTITY.into()))
    })
    .expect("the same identity takes it");
}

#[test]
fn a_browser_signed_in_with_two_logins_goes_only_to_a_chat_allowed_both() {
    // #2699: the second login never takes the first one's place; its session may still be there.
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    let driver: Arc<dyn Driver> = hosted.clone();
    let qa = as_persona("qa", None, None);
    answered_as(&browsers, &hosted, &ask(3, click()), &qa);
    browsers.allow(3, false);
    answered_as(&browsers, &hosted, &ask(3, log_in("shop-login")), &qa);
    answered_as(&browsers, &hosted, &ask(3, log_in("other-login")), &qa);
    // A second try of the same login is held once.
    answered_as(&browsers, &hosted, &ask(3, log_in("shop-login")), &qa);
    browsers.pin(3, "program-1", true).unwrap();
    browsers.forget(Some(&driver), 3, "Fix the login");
    allowed_and_acting(&browsers, &hosted, 4);
    let asked: Mutex<Vec<String>> = Mutex::default();

    let why = hand_checked(&browsers, &hosted, "program-1", 4, &|login| {
        asked.lock().unwrap().push(login.login.clone());
        if login.login == "other-login" {
            Ok(())
        } else {
            Err("not allowed".into())
        }
    })
    .unwrap_err();

    assert!(why.contains("shop-login"), "{why}");
    assert!(!browsers.is_in_use("program-1"), "it stays with no chat");
    asked.lock().unwrap().clear();
    hand_checked(&browsers, &hosted, "program-1", 4, &|login| {
        asked.lock().unwrap().push(login.login.clone());
        Ok(())
    })
    .expect("allowed both");
    assert_eq!(*asked.lock().unwrap(), ["shop-login", "other-login"]);
}

#[test]
fn a_held_login_goes_only_to_the_same_persona_and_the_same_declaration_allowed_now() {
    // #2699: the identity is compared whole, not the login's name.
    let held = HeldLogin {
        persona: Some("qa".into()),
        login: "shop-login".into(),
        identity: IDENTITY.into(),
    };

    held.may_go_to(Some("qa"), Ok(IDENTITY.into()))
        .expect("the same login");
    let why = held
        .may_go_to(Some("qa"), Ok("fedcba9876543210fedcba9876543210".into()))
        .unwrap_err();
    assert!(why.contains("declared otherwise"), "{why}");
    let why = held
        .may_go_to(Some("steward"), Ok(IDENTITY.into()))
        .unwrap_err();
    assert!(why.contains("qa's login"), "{why}");
    // Why it is not allowed is never passed on: it may name the vault or the declaration.
    let why = held
        .may_go_to(Some("qa"), Err("vault 'shop-vault' is not allowed".into()))
        .unwrap_err();
    assert_eq!(why, "that chat may not log in with shop-login now");
}

#[test]
fn a_login_with_no_kept_store_keeps_what_the_chats_browser_holds() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    answered(&browsers, &hosted, &ask(3, click()));
    browsers.allow(3, false);
    let (answer, _) = answered(&browsers, &hosted, &ask(3, log_in("shop-login")));
    assert!(matches!(answer, Answer::Browsed { .. }), "{answer:?}");
    assert_eq!(
        *hosted.kept.lock().unwrap(),
        [None],
        "no browser from a store"
    );
}

#[test]
fn a_kept_store_is_never_opened_where_its_login_is_not_allowed_now() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    answered(&browsers, &hosted, &ask(3, click()));
    browsers.allow(3, false);
    let why = refusal(answered(&browsers, &hosted, &ask(3, log_in("mail-login"))).0);
    assert!(why.contains("declares no browser login"), "{why}");
    assert!(
        hosted.kept.lock().unwrap().iter().all(Option::is_none),
        "nothing opened from a store"
    );
}

// ---- a browser the person opens (#2176) ------------------------------------------------------

/// A project held by a window, whose `charter.toml` is `text`: it holds the project and its
/// hook socket, which a sandboxed run may not, so CI runs these first.
fn held_project(
    text: &str,
) -> (
    tempfile::TempDir,
    crate::planes::Planes,
    crate::planes::PlaneId,
) {
    let dir = tempfile::tempdir().expect("a directory");
    let root = dir.path().join("project");
    std::fs::create_dir_all(&root).expect("the project");
    std::fs::write(purlis_core::names::manifest(&root), text).expect("its manifest");
    let planes = crate::planes::Planes::telling(Arc::new(|_| {}), crate::Shipped::default(), None);
    let id = planes.open(&root);
    (dir, planes, id)
}

/// #2176: a browser the person opens reaches what the project's settings list for a chat on no
/// persona, and the browser sites allowed here, never a persona's hosts nor the floor's reach.
#[test]
fn a_browser_the_person_opens_reaches_the_projects_hosts_and_its_browser_sites() {
    let (_dir, planes, id) = held_project(
        "schema = 1\n[sandbox]\nmode = \"on\"\negress = []\nhosts = [\"api.example\"]\n\
         [sandbox.personas.devops]\nhosts = [\"ops.example\"]\n",
    );
    let held = planes.held(&id).expect("held");
    purlis_core::sandbox::local::grant_browser_site(held.root(), &site("docs.example:443"))
        .expect("kept");

    let reach = super::person_reach(&held);

    use purlis_session_protocol::session::Layer;
    let hosts: Vec<(&str, Layer)> = reach
        .listed
        .iter()
        .map(|one| (one.host.as_str(), one.layer))
        .collect();
    assert!(hosts.contains(&("api.example", Layer::Open)), "{hosts:?}");
    assert!(
        hosts.contains(&("docs.example:443", Layer::You)),
        "{hosts:?}"
    );
    assert!(
        !hosts.iter().any(|(host, _)| *host == "ops.example"),
        "{hosts:?}"
    );
    assert!(!reach.open_web);
}

/// #2176: the Allow on a person's browser's sites names only what it asked about, and keeps
/// each as a browser site of this machine's, audited, never a host of the chats' own commands.
#[test]
fn allow_on_a_browser_the_person_opened_keeps_only_the_sites_it_asked_about() {
    use purlis_core::sandbox::{self, grant::Level};
    let (_dir, planes, id) = held_project("schema = 1\n[sandbox]\nmode = \"on\"\negress = []\n");
    let held = planes.held(&id).expect("held");
    let audited: Mutex<Vec<String>> = Mutex::default();
    let audit = |chat: Option<u32>, one: &sandbox::grant::Audited<'_>| {
        assert_eq!(chat, None, "no chat's");
        audited.lock().unwrap().push(one.target.to_owned());
        Ok(())
    };
    let allow = |sites: &[&str], level| {
        let sites: Vec<String> = sites.iter().map(|one| (*one).to_owned()).collect();
        super::person_sites_allowed(&held, "B1", &sites, level, &audit, 7)
    };

    assert!(
        allow(&["example.com:443"], Level::You).is_err(),
        "nothing waits"
    );
    held.browsers()
        .theirs_asked("B1", &["example.com:443".to_owned()]);
    assert!(
        allow(&["other.example:443"], Level::You).is_err(),
        "not what it asked"
    );
    assert!(
        allow(&["example.com:443"], Level::Chat).is_err(),
        "no chat to keep it for"
    );
    allow(&["example.com:443"], Level::You).expect("allowed");

    assert_eq!(*audited.lock().unwrap(), ["example.com:443"]);
    assert_eq!(
        sandbox::local::browser_sites(held.root()),
        [site("example.com:443")]
    );
    assert!(held.browsers().theirs_waiting("B1").is_empty());
    assert!(
        allow(&["example.com:443"], Level::You).is_err(),
        "answered once"
    );

    held.browsers()
        .theirs_asked("B1", &["team.example:443".to_owned()]);
    allow(&["team.example:443"], Level::Project).expect("allowed for everyone");
    assert_eq!(
        sandbox::browser_sites::in_force(held.root(), &sandbox::policy::Locks::of(held.root())),
        [sandbox::hosts::Host::parse("team.example:443").unwrap()]
    );
}

/// #2176: where the project's sandbox is not on, a browser site for everyone in the project
/// reaches no browser (#2443), so the person's browser is not offered it, and it is refused.
#[test]
fn without_the_projects_sandbox_a_persons_browser_is_offered_this_machine_alone() {
    use purlis_core::sandbox::{self, grant::Level};
    let (_dir, planes, id) = held_project("schema = 1\n");
    let held = planes.held(&id).expect("held");
    let (named, answers) = super::theirs_asked(&held, "B1", &["example.com:443".to_owned()]);
    assert_eq!(named, ["example.com:443"]);
    let ids: Vec<&str> = answers.iter().map(|one| one.id.as_str()).collect();
    assert_eq!(ids, ["you", "keep"]);
    assert_eq!(
        answers[0].label,
        "Always allow in this project, on this machine"
    );
    let audit = |_: Option<u32>, _: &sandbox::grant::Audited<'_>| Ok(());
    let why = super::person_sites_allowed(
        &held,
        "B1",
        &["example.com:443".to_owned()],
        Level::Project,
        &audit,
        7,
    )
    .unwrap_err();
    assert!(why.contains("sandbox is on"), "{why}");
    assert!(
        sandbox::browser_sites::committed(held.root(), &sandbox::policy::Locks::of(held.root()))
            .is_empty()
    );
}

/// #2176: where it is on, both are offered, in the registry's words.
#[test]
fn with_the_projects_sandbox_a_persons_browser_is_offered_everyone_too() {
    let (_dir, planes, id) = held_project("schema = 1\n[sandbox]\nmode = \"on\"\negress = []\n");
    let held = planes.held(&id).expect("held");
    let (_, answers) = super::theirs_asked(&held, "B1", &["example.com:443".to_owned()]);
    let ids: Vec<&str> = answers.iter().map(|one| one.id.as_str()).collect();
    assert_eq!(ids, ["you", "project", "keep"]);
    assert_eq!(answers[1].label, "Allow for everyone in this project");
    assert_eq!(
        held.browsers().theirs_waiting("B1"),
        [site("example.com:443")]
    );
}

/// #2242: a chat started without the sandbox lists what a sandboxed chat of its persona would:
/// the project's hosts, the persona's as the person here allowed them, and the browser sites,
/// never the floor's reach; and nothing of another persona's.
#[test]
fn a_chat_without_the_sandbox_lists_what_a_sandboxed_chat_of_its_persona_would() {
    use purlis_session_protocol::session::Layer;
    let text = "schema = 1\n[sandbox]\nmode = \"on\"\negress = []\nhosts = [\"api.example\"]\n\
                [sandbox.personas.devops]\nhosts = [\"ops.example\"]\n\
                [sandbox.personas.qa]\nhosts = [\"qa.example\"]\n";
    let (_dir, planes, id) = held_project(text);
    let held = planes.held(&id).expect("held");
    let listed = |persona: Option<&str>| -> Vec<(String, Layer)> {
        super::listed_reach(&held, persona)
            .listed
            .into_iter()
            .map(|one| (one.host, one.layer))
            .collect()
    };
    assert!(
        !listed(Some("devops"))
            .iter()
            .any(|(host, _)| host == "ops.example"),
        "not until the person here allows the persona's hosts"
    );
    let hosts: Vec<purlis_core::sandbox::hosts::Host> =
        vec![purlis_core::sandbox::hosts::Host::parse("ops.example").unwrap()];
    purlis_core::sandbox::local::allow_persona_hosts(
        held.root(),
        "devops",
        &purlis_core::sandbox::persona::digest(&hosts, false),
    )
    .expect("allowed");

    let devops = listed(Some("devops"));

    assert!(
        devops.contains(&("api.example".to_owned(), Layer::Open)),
        "{devops:?}"
    );
    assert!(
        devops.contains(&("ops.example".to_owned(), Layer::Persona)),
        "{devops:?}"
    );
    assert!(
        !devops.iter().any(|(host, _)| host == "qa.example"),
        "{devops:?}"
    );
    assert!(!super::listed_reach(&held, Some("devops")).open_web);
}

/// #2242: a chat the app does not record as started without the sandbox, such as one held by a
/// sandbox whose compile was not recorded (D-2006-11), or one this window does not have open, is
/// given nothing listed: never the listing a chat without the sandbox gets.
#[test]
fn a_chat_not_recorded_as_started_without_the_sandbox_is_given_nothing_listed() {
    let (_dir, planes, id) = held_project(
        "schema = 1\n[sandbox]\nmode = \"on\"\negress = []\nhosts = [\"api.example\"]\n",
    );
    let held = planes.held(&id).expect("held");
    assert!(!held.chats().started_unsandboxed(9));

    let reach = super::reach_of(&held, 9);

    assert!(reach.listed.is_empty(), "{:?}", reach.listed);
    assert!(!reach.open_web);
}

/// #2443: a browser site for everyone in the project is refused, for a chat's Ask and for the
/// person's own browser alike, where the project's sandbox is not on: it would reach no browser.
#[test]
fn everyone_in_the_project_is_refused_where_the_projects_sandbox_is_not_on() {
    use purlis_core::sandbox::grant::Level;
    let off = tempfile::tempdir().expect("a project");
    std::fs::write(purlis_core::names::manifest(off.path()), "schema = 1\n").expect("written");
    let on = tempfile::tempdir().expect("a project");
    std::fs::write(
        purlis_core::names::manifest(on.path()),
        "schema = 1\n[sandbox]\nmode = \"on\"\negress = []\n",
    )
    .expect("written");
    let sites = [site("news.example.org:443")];

    let why = super::sites_grantable(off.path(), &sites, Level::Project).unwrap_err();

    assert!(why.contains("sandbox is"), "{why}");
    assert!(super::sites_grantable(off.path(), &sites, Level::You).is_ok());
    assert!(super::sites_grantable(on.path(), &sites, Level::Project).is_ok());
}

#[test]
fn the_window_says_why_it_ends_each_chats_browser() {
    let browser = super::ChatBrowser {
        session: 7,
        name: "b1".into(),
        program: "01K2".into(),
        ..super::ChatBrowser::default()
    };
    assert_eq!(
        super::ending_said(&browser, super::EndWhy::Unused { minutes: 15 }),
        "purlis: chat 7's browser b1 (01K2) is ended by the window: it went unused for 15 minutes"
    );
    for (why, said) in [
        (super::EndWhy::ClosedByItsChat, "its chat closed it"),
        (super::EndWhy::ChatEnded, "its chat ended"),
        (super::EndWhy::ReachNarrowed, "was narrowed"),
        (super::EndWhy::StopAll, "stopped every browser"),
        (super::EndWhy::NotHandedOver, "handed to"),
    ] {
        assert!(super::ending_said(&browser, why).contains(said), "{why:?}");
    }
}

// ---- Who drives, on a browser's row (#2300) ----

#[test]
fn a_browsers_row_says_the_person_drives_it_from_their_take_over_until_they_hand_it_back() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    used_at_1000(&browsers, &hosted);
    let driver = |browsers: &ChatBrowsers| browsers.listed()[0].driver;
    assert_eq!(driver(&browsers), None, "its agent drives it");

    let before = browsers.listing();
    browsers.driven("program-1", true, 1000);
    assert_ne!(browsers.listing(), before, "its row moves");
    assert_eq!(driver(&browsers), Some(BrowserDriver::Person));

    let taken = browsers.listing();
    browsers.driven("program-1", true, 1001);
    assert_eq!(
        browsers.listing(),
        taken,
        "taken over again, it does not move"
    );

    browsers.driven("program-1", false, 2000);
    assert_eq!(driver(&browsers), None);
}

#[test]
fn a_browsers_row_says_purlis_drives_it_while_it_logs_in_and_the_window_is_told_at_once() {
    let seen: Arc<Mutex<Vec<Option<BrowserDriver>>>> = Arc::default();
    let browsers = Arc::new_cyclic(|me: &std::sync::Weak<ChatBrowsers>| {
        let me = me.clone();
        let seen = Arc::clone(&seen);
        ChatBrowsers::default().telling(Arc::new(move || {
            if let Some(browsers) = me.upgrade() {
                seen.lock()
                    .unwrap()
                    .extend(browsers.listed().iter().map(|one| one.driver));
            }
        }))
    });
    let hosted = Arc::new(Hosted::default());
    answered(&browsers, &hosted, &ask(3, log_in("shop-login")));
    browsers.allow(3, false);
    answered(&browsers, &hosted, &ask(3, log_in("shop-login")));

    assert_eq!(
        *seen.lock().unwrap(),
        [Some(BrowserDriver::Purlis)],
        "told as the login starts"
    );
    assert_eq!(
        browsers.listed()[0].driver,
        None,
        "handed back once it answered"
    );
}

#[test]
fn two_logins_at_once_in_one_browser_hold_its_purlis_mark_until_both_have_answered() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    used_at_1000(&browsers, &hosted);
    let driver = |browsers: &ChatBrowsers| browsers.listed()[0].driver;

    browsers.purlis_drives("program-1", true);
    browsers.purlis_drives("program-1", true);
    browsers.purlis_drives("program-1", false);
    assert_eq!(
        driver(&browsers),
        Some(BrowserDriver::Purlis),
        "the other login still runs"
    );

    browsers.purlis_drives("program-1", false);
    assert_eq!(driver(&browsers), None);
}

// ---- Browsers on runners (#2050) ----

#[test]
fn a_chat_on_a_pools_runner_has_its_browser_started_on_that_runner() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    let driver: Arc<dyn Driver> = hosted.clone();
    let on_builds = |browse: &BrowseAsk| {
        browsers.answer(
            Some(&driver),
            Path::new("/p"),
            Some("builds"),
            browse,
            &|| reach(),
            false,
            &AsPersona::default,
            &logins,
            1000,
        )
    };
    on_builds(&ask(3, click()));
    browsers.allow(3, false);

    let (answer, _) = on_builds(&ask(3, click()));
    answered(&browsers, &hosted, &ask(4, click()));
    browsers.allow(4, false);
    answered(&browsers, &hosted, &ask(4, click()));

    assert!(matches!(answer, Answer::Browsed { .. }), "{answer:?}");
    assert_eq!(
        *hosted.on_pools.lock().unwrap(),
        [Some("builds".to_owned()), None],
        "chat 3's on its pool's runner, chat 4's on this machine"
    );
}

fn a_pool_as(image: &str) -> purlis_core::poollogins::Bound {
    purlis_core::poollogins::Bound {
        pool: "builds".into(),
        definition: format!("defined with {image}"),
        image: image.into(),
    }
}

#[test]
fn a_runner_chats_first_login_asks_once_whether_its_persona_may_log_in_on_the_pool() {
    let browsers = ChatBrowsers::default();
    let pool = a_pool_as("aaaa");

    let first = browsers.pool_login(3, "steward", &pool, "shop-login", 1000);
    let again = browsers.pool_login(3, "steward", &pool, "shop-login", 1001);

    let (why, raised) = first.expect_err("asked, and nothing filled");
    assert!(raised, "the first raises its Ask");
    assert!(why.contains("pool builds"), "{why}");
    assert!(why.contains("filled nothing"), "{why}");
    assert!(!again.expect_err("still asked").1, "and only the first");
    let waits = browsers.pool_logins_waiting();
    assert_eq!(waits.len(), 1);
    assert_eq!(
        (
            waits[0].session,
            waits[0].persona.as_str(),
            waits[0].changed
        ),
        (3, "steward", false)
    );
}

#[test]
fn once_the_person_allows_its_persona_on_the_pool_a_login_there_goes_ahead() {
    let browsers = ChatBrowsers::default();
    let pool = a_pool_as("aaaa");
    let _ = browsers.pool_login(3, "steward", &pool, "shop-login", 1000);
    let _ = browsers.pool_login(4, "steward", &pool, "other-login", 1000);
    let _ = browsers.pool_login(5, "reviewer", &pool, "shop-login", 1000);

    let answered = browsers.answer_pool_login(3, true);

    let sessions: Vec<u32> = answered.iter().map(|one| one.session).collect();
    assert_eq!(sessions, [3, 4], "every chat of that persona on that pool");
    assert!(
        browsers
            .pool_login(3, "steward", &pool, "shop-login", 1002)
            .is_ok()
    );
    assert!(
        browsers
            .pool_login(4, "steward", &pool, "shop-login", 1002)
            .is_ok()
    );
    assert!(
        browsers
            .pool_login(5, "reviewer", &pool, "shop-login", 1002)
            .is_err(),
        "another persona is asked apart"
    );
    assert!(
        browsers.answer_pool_login(3, true).is_empty(),
        "nothing waits"
    );
}

#[test]
fn a_changed_pool_image_asks_for_the_opt_in_again() {
    let browsers = ChatBrowsers::default();
    let _ = browsers.pool_login(3, "steward", &a_pool_as("aaaa"), "shop-login", 1000);
    browsers.answer_pool_login(3, true);

    let moved = browsers.pool_login(3, "steward", &a_pool_as("bbbb"), "shop-login", 2000);

    let (why, raised) = moved.expect_err("asked again");
    assert!(raised);
    assert!(why.contains("changed since the person let"), "{why}");
    assert!(browsers.pool_logins_waiting()[0].changed);
}

#[test]
fn keep_off_refuses_a_personas_logins_on_the_pool_until_it_changes() {
    let browsers = ChatBrowsers::default();
    let _ = browsers.pool_login(3, "steward", &a_pool_as("aaaa"), "shop-login", 1000);
    browsers.answer_pool_login(3, false);

    let (why, raised) = browsers
        .pool_login(3, "steward", &a_pool_as("aaaa"), "shop-login", 1001)
        .expect_err("kept off");
    let (_, changed) = browsers
        .pool_login(3, "steward", &a_pool_as("bbbb"), "shop-login", 1002)
        .expect_err("asked again once it changed");

    assert!(!raised, "no Ask while it stands");
    assert!(
        why.contains("keeps steward's browser logins off on pool builds"),
        "{why}"
    );
    assert!(changed);
}

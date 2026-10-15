// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;
use std::sync::{Arc, Mutex};

use purlis_core::hookwire::{Answer, BrowseAsk, Browsing};
use purlis_session_protocol::browsing::{Act, Browsed, BrowserReach};

use super::{ChatBrowsers, Driver, Posture, Refused};

/// A driver that stands in for the session host: it starts numbered browsers, answers every
/// act settled, and keeps what it was asked.
#[derive(Default)]
struct Hosted {
    started: Mutex<Vec<BrowserReach>>,
    acts: Mutex<Vec<(String, String, Act)>>,
    closed: Mutex<Vec<String>>,
    /// Programs that have ended in the host.
    ended: Mutex<Vec<String>>,
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

    fn close(&self, program: &str) {
        self.closed.lock().unwrap().push(program.to_owned());
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
        open_web: false,
    }
}

fn answered(browsers: &ChatBrowsers, hosted: &Arc<Hosted>, browse: &BrowseAsk) -> (Answer, bool) {
    let driver: Arc<dyn Driver> = hosted.clone();
    browsers.answer(Some(&driver), Path::new("/p"), browse, &|| reach(), 1000)
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
            since: 1000
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
fn a_chat_holds_at_most_three_browsers() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    answered(&browsers, &hosted, &ask(3, click()));
    browsers.allow(3, false);
    for _ in 0..3 {
        answered(&browsers, &hosted, &ask(3, Browsing::New));
    }

    let why = refusal(answered(&browsers, &hosted, &ask(3, Browsing::New)).0);

    assert!(why.contains("browser_close"), "{why}");
    assert_eq!(hosted.started.lock().unwrap().len(), 3);
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
            .answer(None, Path::new("/p"), &ask(3, click()), &|| reach(), 0)
            .0,
    );

    assert!(why.contains("session host"), "{why}");
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;
use std::sync::{Arc, Mutex};

use purlis_core::hookwire::{Answer, BrowseAsk, Browsing};
use purlis_core::state::State;
use purlis_session_protocol::browsing::{Act, Browsed, BrowserReach};

use super::{Ended, NeedsYou, Waiting};
use crate::chatbrowsers::{AsPersona, ChatBrowsers, Driver, Refused};
use crate::vaultroute::{Refusals, Told};

/// A session host that starts numbered browsers and answers every act settled.
#[derive(Default)]
struct Host {
    started: Mutex<u32>,
    acts: Mutex<Vec<(String, Act)>>,
    ended: Mutex<Vec<String>>,
}

impl Driver for Host {
    fn start(&self, _root: &Path, _reach: BrowserReach) -> Result<String, String> {
        let mut started = self.started.lock().unwrap();
        *started += 1;
        Ok(format!("program-{started}"))
    }

    fn act(&self, program: &str, _call_id: &str, act: Act) -> Result<Browsed, Refused> {
        if self.ended.lock().unwrap().iter().any(|one| one == program) {
            return Err(Refused::Gone);
        }
        self.acts.lock().unwrap().push((program.to_owned(), act));
        Ok(Browsed {
            settled: true,
            ..Browsed::default()
        })
    }

    fn close(&self, _program: &str) -> Result<(), String> {
        Ok(())
    }

    fn stop(&self, _program: &str) -> Result<(), String> {
        Ok(())
    }
}

/// A project whose chat 4 has browser `b1` open, on the fixture site's CAPTCHA page, and
/// what was told of every Ask of it that ended with its browser.
struct Project {
    host: Arc<Host>,
    driver: Arc<dyn Driver>,
    browsers: ChatBrowsers,
    needs: NeedsYou,
    ended: Arc<Mutex<Vec<Ended>>>,
}

const CHAT: u32 = 4;

fn browse(chat: u32, browser: Option<&str>, asked: Browsing) -> BrowseAsk {
    BrowseAsk {
        chat,
        browser: browser.map(str::to_owned),
        call_id: "c-1".into(),
        asked,
    }
}

fn open_captcha() -> Browsing {
    Browsing::Act(Act::Open {
        address: "http://fixture.test/captcha.html".into(),
    })
}

fn needs_you() -> Browsing {
    Browsing::NeedsYou {
        question: "The page asks for the letters in the picture".into(),
    }
}

impl Project {
    fn new() -> Self {
        let host = Arc::new(Host::default());
        let driver: Arc<dyn Driver> = host.clone();
        let ended: Arc<Mutex<Vec<Ended>>> = Arc::default();
        let told = Arc::clone(&ended);
        let project = Project {
            host,
            driver,
            browsers: ChatBrowsers::default(),
            needs: NeedsYou::new(Arc::new(move |one| told.lock().unwrap().push(one))),
            ended,
        };
        // The chat's first call asks the person what its browser may reach; they allow it.
        project.call(&browse(CHAT, None, open_captcha()));
        assert!(project.browsers.allow(CHAT, false));
        project.call(&browse(CHAT, None, open_captcha()));
        project
    }

    /// A browser call of the chat's, as the app answers it.
    fn call(&self, asked: &BrowseAsk) -> Answer {
        if let Some(answer) = super::answered_first(&self.browsers, &self.needs, asked, 1000) {
            return answer;
        }
        let answer = self
            .browsers
            .answer(
                Some(&self.driver),
                Path::new("/p"),
                asked,
                &BrowserReach::default,
                &AsPersona::default,
                1000,
            )
            .0;
        self.needs.settle(&self.browsers, 1001);
        answer
    }
}

fn note(answer: Answer) -> String {
    match answer {
        Answer::Browsed { browsed, .. } => browsed.note.expect("a note"),
        other => panic!("not an answer: {other:?}"),
    }
}

fn refusal(answer: Answer) -> String {
    match answer {
        Answer::No { why } => why,
        other => panic!("not a refusal: {other:?}"),
    }
}

#[test]
fn a_captcha_page_raises_an_ask_whose_source_is_the_browser_and_the_tool_returns_at_once() {
    let project = Project::new();

    let said = note(project.call(&browse(CHAT, Some("b1"), needs_you())));

    assert!(
        said.contains("asked the person to take browser b1 over"),
        "{said}"
    );
    assert!(said.contains("told when they hand it back"), "{said}");
    assert_eq!(
        project.needs.waiting(),
        [Waiting {
            session: CHAT,
            browser: "b1".into(),
            program: "program-1".into(),
            question: "The page asks for the letters in the picture".into(),
            since: 1000,
        }]
    );
}

#[test]
fn asking_again_about_a_browser_that_waits_raises_no_second_ask() {
    let project = Project::new();
    project.call(&browse(CHAT, Some("b1"), needs_you()));

    let again = note(project.call(&browse(CHAT, Some("b1"), needs_you())));

    assert!(again.contains("already"), "{again}");
    assert_eq!(project.needs.waiting().len(), 1);
}

#[test]
fn a_browser_the_chat_does_not_have_raises_nothing() {
    let project = Project::new();

    let why = refusal(project.call(&browse(CHAT, Some("b7"), needs_you())));
    let other = refusal(project.call(&browse(9, Some("b1"), needs_you())));

    assert!(why.contains("no browser b7"), "{why}");
    assert!(other.contains("no browser b1"), "{other}");
    assert!(project.needs.waiting().is_empty());
}

#[test]
fn while_the_person_is_asked_the_chats_acts_on_that_browser_are_refused_in_words() {
    let project = Project::new();
    project.call(&browse(CHAT, Some("b1"), needs_you()));
    let acts_before = project.host.acts.lock().unwrap().len();

    let why = refusal(project.call(&browse(CHAT, None, Browsing::Act(Act::Pages))));

    assert!(why.contains("the person"), "{why}");
    assert!(why.contains("don't use browser b1"), "{why}");
    assert_eq!(project.host.acts.lock().unwrap().len(), acts_before);
}

#[test]
fn the_hand_back_ends_the_ask_and_tells_the_chat_only_that_the_person_finished() {
    let project = Project::new();
    project.call(&browse(CHAT, Some("b1"), needs_you()));

    let back = project
        .needs
        .handed_back("program-1")
        .expect("an Ask waited");

    assert_eq!(back.browser, "b1");
    assert!(project.needs.waiting().is_empty());
    assert!(
        project.ended.lock().unwrap().is_empty(),
        "no Update: it was answered"
    );
    assert_eq!(
        super::finished(&back).line(),
        "⬢ purlis: your ask that the person take over browser `b1` — They finished and handed \
         it back — take a fresh snapshot before you act: references from before are void."
    );
    // And its browser is the chat's again.
    assert!(matches!(
        project.call(&browse(CHAT, None, Browsing::Act(Act::Pages))),
        Answer::Browsed { .. }
    ));
}

#[test]
fn a_hand_back_with_no_ask_waiting_ends_nothing() {
    let project = Project::new();
    assert!(project.needs.handed_back("program-1").is_none());
}

#[test]
fn a_waiting_chat_is_woken_with_the_line_and_a_busy_one_when_its_turn_ends() {
    let refusals = Refusals::default();
    let back = Waiting {
        session: CHAT,
        browser: "b1".into(),
        program: "program-1".into(),
        question: "q".into(),
        since: 1,
    };
    let sent: Mutex<Vec<String>> = Mutex::default();
    let next_turn: Mutex<Vec<String>> = Mutex::default();
    let tell = |state: State, asking: bool| {
        super::tell(
            &refusals,
            &back,
            || (state, asking),
            |_, text| {
                sent.lock().unwrap().push(text.to_owned());
                Ok(())
            },
            |outcome| next_turn.lock().unwrap().push(outcome.line()),
        )
    };

    assert_eq!(tell(State::Waiting, false), Told::Sent);
    assert_eq!(tell(State::Running, false), Told::Queued);
    assert_eq!(refusals.queued_for(CHAT), 1);
    // A chat that asks the person something is typed nothing: its next turn is told.
    assert_eq!(tell(State::Waiting, true), Told::NotSent);

    let line = super::finished(&back).line();
    assert_eq!(*sent.lock().unwrap(), std::slice::from_ref(&line));
    assert_eq!(*next_turn.lock().unwrap(), [line]);
}

#[test]
fn the_ask_ends_with_its_browser_and_leaves_an_update() {
    let project = Project::new();
    project.call(&browse(CHAT, Some("b1"), needs_you()));

    let closed = project.call(&browse(CHAT, Some("b1"), Browsing::Close));

    assert!(matches!(closed, Answer::Browsed { .. }), "{closed:?}");
    assert!(project.needs.waiting().is_empty());
    let ended = project.ended.lock().unwrap().clone();
    assert_eq!(ended.len(), 1);
    assert_eq!((ended[0].session, ended[0].browser.as_str()), (CHAT, "b1"));
    assert_eq!(ended[0].at, 1001);
    assert!(ended[0].says.contains("b1"), "{}", ended[0].says);
}

#[test]
fn a_chat_that_closes_and_stop_all_each_end_their_asks_with_an_update() {
    let project = Project::new();
    project.call(&browse(CHAT, Some("b1"), needs_you()));

    project.needs.chat_closed(CHAT, 1002);

    assert!(project.needs.waiting().is_empty());
    assert_eq!(project.ended.lock().unwrap().len(), 1);

    let other = Project::new();
    other.call(&browse(CHAT, Some("b1"), needs_you()));
    other.needs.all_ended(1003);
    assert!(other.needs.waiting().is_empty());
    assert_eq!(other.ended.lock().unwrap()[0].at, 1003);
}

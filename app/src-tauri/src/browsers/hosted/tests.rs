// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The window's browsers over an in-memory link to a real host whose browsers are stand-ins:
//! a browser opened in a project, a view of it drawn a whole frame at a time, and Stop all
//! ending every browser. The same over a real engine is `purlis-browser-engine`'s, on CI.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use purlis_host::standin::{StandInBrowsers, StandIns};
use purlis_host::{ComposesNothing, Hosting};

use super::WindowBrowsers;
use crate::browsers::{BrowserInput, BrowserViewRole, InputOutcome, KeyAct};

struct Window {
    browsers: WindowBrowsers,
    started: StandInBrowsers,
    root: std::path::PathBuf,
    _dirs: (tempfile::TempDir, tempfile::TempDir),
    _runtime: tokio::runtime::Runtime,
}

fn window() -> Window {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let store = tempfile::tempdir().unwrap();
    let started = StandInBrowsers::in_folder(store.path().join("browsers"));
    let hosting =
        Hosting::running_with(ComposesNothing, StandIns::default()).with_browsers(started.clone());
    let (connection, _window) = crate::hostedsessions::tests::in_memory(hosting, &runtime);
    let project = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(project.path()).unwrap();
    std::fs::write(root.join(purlis_core::plane::MANIFEST), "").unwrap();
    Window {
        browsers: WindowBrowsers::new(connection),
        started,
        root,
        _dirs: (store, project),
        _runtime: runtime,
    }
}

fn until(what: &str, done: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !done() {
        assert!(Instant::now() < deadline, "{what} never happened");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn a_browser_the_window_opens_is_drawn_a_whole_frame_at_a_time() {
    let window = window();
    let program = window.browsers.open(&window.root).unwrap();
    let screen = window.started.started()[0].clone();
    let drawn: Arc<Mutex<Vec<Vec<u8>>>> = Arc::default();
    let drawing = Arc::clone(&drawn);

    let watching = window
        .browsers
        .watch(
            &program,
            BrowserViewRole::Focused,
            Box::new(move |_aim, image| drawing.lock().unwrap().push(image.to_vec())),
        )
        .unwrap();
    assert_eq!((watching.width, watching.height), (1280, 800));
    let big = vec![7_u8; 100 * 1024];
    screen.makes(b"\xff\xd8 first");
    until("the first frame is drawn", || {
        drawn.lock().unwrap().len() == 1
    });
    screen.makes(&big);
    until("the second frame is drawn whole", || {
        drawn.lock().unwrap().len() == 2
    });
    assert_eq!(drawn.lock().unwrap()[1], big);

    window.browsers.unwatch(&program, watching.view).unwrap();
    until("frames stop with the last view", || !screen.shown().showing);
}

#[test]
fn two_clones_of_one_project_each_open_a_browser() {
    // #2178: one project id, two folders.
    let window = window();
    let id = "[project]\nid = \"01J9PRJCT00000000000000000\"\n";
    std::fs::write(window.root.join(purlis_core::plane::MANIFEST), id).unwrap();
    let clone = tempfile::tempdir().unwrap();
    let other = std::fs::canonicalize(clone.path()).unwrap();
    std::fs::write(other.join(purlis_core::plane::MANIFEST), id).unwrap();

    window.browsers.open(&window.root).unwrap();
    window.browsers.open(&other).unwrap();

    assert_eq!(window.started.started().len(), 2);
}

#[test]
fn stop_all_stops_every_browser_the_host_runs() {
    let window = window();
    window.browsers.open(&window.root).unwrap();
    window.browsers.open(&window.root).unwrap();

    let stopped = window.browsers.stop_every_browser();

    assert_eq!(stopped, 2);
    assert!(
        window
            .started
            .started()
            .iter()
            .all(|screen| screen.shown().stopped)
    );
}

#[test]
fn letting_go_of_a_project_ends_the_browsers_the_window_started_in_it_and_its_hold() {
    let window = window();
    let one = window.browsers.open(&window.root).unwrap();
    window.browsers.open(&window.root).unwrap();
    window.browsers.close(&one).unwrap();

    let ended = window.browsers.let_go(&window.root);

    assert_eq!(ended, 1, "the one still running");
    // A close answers once the host has forgotten the browser, and ends it after.
    until("every browser the window started has stopped", || {
        window
            .started
            .started()
            .iter()
            .all(|screen| screen.shown().stopped)
    });
    let link = window.browsers.link().unwrap();
    assert!(
        matches!(
            link.call(purlis_session_protocol::session::Command::List),
            Ok(purlis_session_protocol::session::Answer::Chats(listed)) if listed.is_empty()
        ),
        "nothing is left in the host"
    );
    let held_again = window.browsers.open(&window.root);
    assert!(
        held_again.is_ok(),
        "the project is held again when asked: {held_again:?}"
    );
}

/// A key the person pressed, aimed at what a frame said it showed.
fn a_key(aim: &str) -> BrowserInput {
    BrowserInput::Key {
        aim: aim.to_owned(),
        action: KeyAct::Down,
        key: "a".into(),
        physical: "KeyA".into(),
        key_code: 65,
        modifiers: 0,
    }
}

#[test]
fn while_the_person_drives_a_chats_act_is_refused_in_words_and_their_keys_reach_the_page() {
    use crate::chatbrowsers::{Driver, Refused};
    use purlis_session_protocol::browsing::Act;

    let window = window();
    let program = window.browsers.open(&window.root).unwrap();
    let screen = window.started.started()[0].clone();
    let aims: Arc<Mutex<Vec<String>>> = Arc::default();
    let seen = Arc::clone(&aims);
    window
        .browsers
        .watch(
            &program,
            BrowserViewRole::Focused,
            Box::new(move |aim, _image| seen.lock().unwrap().push(aim.unwrap_or_default())),
        )
        .unwrap();
    screen.shown().document = 5;
    screen.makes(b"\xff\xd8 a page");
    until("the frame says what it shows", || {
        !aims.lock().unwrap().is_empty()
    });
    let aim = aims.lock().unwrap()[0].clone();
    assert_eq!(aim, "p1:5");

    let early = window.browsers.input(&program, a_key(&aim)).unwrap();
    assert_eq!(
        early,
        InputOutcome::Refused {
            why: purlis_host::NOT_DRIVING.into()
        }
    );

    window.browsers.take_over(&program).unwrap();
    let refused = Driver::act(&window.browsers, &program, "c-1", Act::Pages);
    assert!(
        matches!(&refused, Err(Refused::Said(why)) if why.contains("the person has taken this browser over")),
        "{refused:?}"
    );
    assert_eq!(
        window.browsers.input(&program, a_key(&aim)).unwrap(),
        InputOutcome::Taken
    );
    assert_eq!(screen.shown().inputs.len(), 1);
    assert_eq!(
        window.browsers.input(&program, a_key("p1:4")).unwrap(),
        InputOutcome::Missed,
        "input aimed at a document the page has left is dropped"
    );

    window.browsers.hand_back(&program).unwrap();
    assert!(Driver::act(&window.browsers, &program, "c-2", Act::Pages).is_ok());
}

#[test]
fn a_view_opened_again_says_the_person_still_drives_the_browser() {
    let window = window();
    let program = window.browsers.open(&window.root).unwrap();
    let watching = window
        .browsers
        .watch(&program, BrowserViewRole::Focused, Box::new(|_, _| {}))
        .unwrap();
    assert!(!watching.person_drives);
    window.browsers.take_over(&program).unwrap();
    window.browsers.unwatch(&program, watching.view).unwrap();

    let again = window
        .browsers
        .watch(&program, BrowserViewRole::Focused, Box::new(|_, _| {}))
        .unwrap();

    assert!(again.person_drives);
}

#[test]
fn a_captcha_page_raises_an_ask_and_the_chat_is_told_only_that_the_person_finished() {
    // #2015, over a real host whose browser is a stand-in: the chat's browser opens the fixture
    // site's CAPTCHA page, the chat asks the person through `ask_operator` pointed at it, the
    // person takes it over, types, and hands it back with Done.
    use crate::browserneedsyou::{self, NeedsYou};
    use crate::chatbrowsers::{AsPersona, ChatBrowsers, Driver};
    use purlis_core::hookwire::{Answer, BrowseAsk, Browsing};
    use purlis_core::state::State;
    use purlis_session_protocol::browsing::{Act, BrowserReach};

    let window = window();
    let driver: Arc<dyn Driver> =
        Arc::new(WindowBrowsers::new(Arc::clone(&window.browsers.connection)));
    let chats = ChatBrowsers::default();
    let needs = NeedsYou::default();
    let calls = std::sync::atomic::AtomicU32::new(0);
    let call = |asked: Browsing| {
        let browse = BrowseAsk {
            chat: 4,
            browser: if matches!(asked, Browsing::Act(Act::Open { .. })) {
                None
            } else {
                Some("b1".into())
            },
            call_id: format!(
                "c-{}",
                calls.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ),
            asked,
        };
        if let Some(answer) = browserneedsyou::answered_first(&chats, &needs, &browse, 1000) {
            return answer;
        }
        chats
            .answer(
                Some(&driver),
                &window.root,
                &browse,
                &BrowserReach::default,
                &AsPersona::default,
                1000,
            )
            .0
    };
    let captcha = || {
        Browsing::Act(Act::Open {
            address: "http://fixture.test/captcha.html".into(),
        })
    };
    call(captcha());
    assert!(chats.allow(4, false));
    assert!(matches!(call(captcha()), Answer::Browsed { .. }));
    let (_, program) = chats.which(4, None).expect("the chat's browser");

    let asked = call(Browsing::NeedsYou {
        question: "The page asks for the letters in the picture".into(),
    });
    assert!(
        matches!(&asked, Answer::Browsed { browsed, .. } if browsed.note.as_deref().is_some_and(|note| note.contains("Needs you"))),
        "{asked:?}"
    );
    assert_eq!(
        needs.waiting().len(),
        1,
        "one Ask, whose source is the browser"
    );

    // The person takes it over from the Ask and types the code.
    window.browsers.take_over(&program).unwrap();
    let screen = window.started.started()[0].clone();
    window
        .browsers
        .watch(&program, BrowserViewRole::Focused, Box::new(|_, _| {}))
        .unwrap();
    screen.shown().document = 1;
    let typed = BrowserInput::Key {
        aim: "p1:1".to_owned(),
        action: KeyAct::Down,
        key: "Q".into(),
        physical: "KeyQ".into(),
        key_code: 81,
        modifiers: 0,
    };
    window.browsers.input(&program, typed).unwrap();
    window.browsers.hand_back(&program).unwrap();

    let back = needs.handed_back(&program).expect("the Ask waited");
    let sent: Mutex<Vec<String>> = Mutex::default();
    browserneedsyou::tell(
        &crate::vaultroute::Refusals::default(),
        &back,
        || (State::Waiting, false),
        |_, text| {
            sent.lock().unwrap().push(text.to_owned());
            Ok(())
        },
        |_| panic!("a waiting chat is told now"),
    );
    // Purlis's fixed words and the browser's name: nothing the person typed or saw.
    assert_eq!(
        *sent.lock().unwrap(),
        [
            "⬢ purlis: your ask that the person take over browser `b1` — They finished and handed \
          it back — take a fresh snapshot before you act: references from before are void."
        ]
    );
    assert!(!sent.lock().unwrap()[0].contains('Q'), "the key they typed");
    assert!(needs.waiting().is_empty());
    assert!(
        matches!(call(Browsing::Act(Act::Pages)), Answer::Browsed { .. }),
        "the browser is the chat's again"
    );
}

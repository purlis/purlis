// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The window's browsers over an in-memory link to a real host whose browsers are stand-ins:
//! a browser opened in a project, a view of it drawn a whole frame at a time, and Stop all
//! ending every browser. The same over a real engine is `purlis-browser-engine`'s, on CI.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use purlis_host::standin::{StandInBrowsers, StandIns};
use purlis_host::{ComposesNothing, Hosting};

use purlis_session_protocol::session::Pushed;

use super::WindowBrowsers;
use crate::browsers::{BrowserInput, BrowserViewRole, InputOutcome, KeyAct};
use crate::hostedsessions::Reconnect;
use crate::hostedsessions::tests::Between;
use crate::reconnecting::Reconnection;

struct Window {
    browsers: WindowBrowsers,
    started: StandInBrowsers,
    root: std::path::PathBuf,
    _dirs: (tempfile::TempDir, tempfile::TempDir),
    _runtime: tokio::runtime::Runtime,
}

fn window() -> Window {
    window_with(|hosting| hosting)
}

/// [`window`], its host behind a host that says, as a relaying host would, that the window's
/// link to a runner is down and back when the test pushes those words (#2542).
fn window_on_a_runner() -> (Window, tokio::sync::broadcast::Sender<Pushed>) {
    let mut pushes = None;
    let window = window_with(|hosting| {
        let between = Between::new(hosting, false);
        pushes = Some(between.pushes.clone());
        between
    });
    (window, pushes.unwrap())
}

/// A window whose host is `host` around one that runs stand-in browsers.
fn window_with<H: purlis_session_protocol::session::Host + Clone>(
    host: impl FnOnce(Hosting) -> H,
) -> Window {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let store = tempfile::tempdir().unwrap();
    let started = StandInBrowsers::in_folder(store.path().join("browsers"));
    let hosting =
        Hosting::running_with(ComposesNothing, StandIns::default()).with_browsers(started.clone());
    let (connection, _window) = crate::hostedsessions::tests::in_memory(host(hosting), &runtime);
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
            Arc::new(|_| {}),
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
fn an_engine_that_ends_by_itself_is_told_at_once_by_its_program() {
    // #2385: nobody calls the browser; the host's word of its end is what tells.
    let told: Arc<Mutex<Vec<String>>> = Arc::default();
    let telling = Arc::clone(&told);
    let window = window();
    let browsers = WindowBrowsers::new(Arc::clone(&window.browsers.connection)).telling_ended(
        Arc::new(move |program| telling.lock().unwrap().push(program)),
    );
    let program = browsers.open(&window.root).unwrap();
    let other = browsers.open(&window.root).unwrap();

    window.started.started()[0].ends(purlis_session_protocol::session::Exit::Signal(
        "SIGSEGV".into(),
    ));

    until("the window hears the engine ended", || {
        !told.lock().unwrap().is_empty()
    });
    assert_eq!(*told.lock().unwrap(), vec![program]);
    assert_ne!(other, told.lock().unwrap()[0], "the other runs on");
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

#[test]
fn past_the_devices_budget_the_next_browser_is_refused_in_words_and_one_closed_makes_room() {
    // #2023: the budget is the device's, whichever project or window started the others.
    let window = window();
    let browsers = WindowBrowsers::new(Arc::clone(&window.browsers.connection)).budgeted(2);
    let one = browsers.open(&window.root).unwrap();
    window.browsers.open(&window.root).unwrap();

    let refused = browsers.open(&window.root).unwrap_err();

    assert!(refused.contains("on this device"), "{refused}");
    assert!(refused.contains("2 browsers"), "{refused}");
    assert_eq!(window.started.started().len(), 2, "nothing more started");
    browsers.close(&one).unwrap();
    until("the closed one has left the host", || {
        browsers.open(&window.root).is_ok()
    });
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
    use purlis_session_protocol::browsing::{Act, BrowserReach};

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
            Arc::new(|_| {}),
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
    let refused = Driver::act(
        &window.browsers,
        &program,
        "c-1",
        Act::Pages,
        BrowserReach::default(),
    );
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
    assert!(
        Driver::act(
            &window.browsers,
            &program,
            "c-2",
            Act::Pages,
            BrowserReach::default()
        )
        .is_ok()
    );
}

#[test]
fn a_view_opened_again_says_the_person_still_drives_the_browser() {
    let window = window();
    let program = window.browsers.open(&window.root).unwrap();
    let watching = window
        .browsers
        .watch(
            &program,
            BrowserViewRole::Focused,
            Box::new(|_, _| {}),
            Arc::new(|_| {}),
        )
        .unwrap();
    assert!(!watching.person_drives);
    window.browsers.take_over(&program).unwrap();
    window.browsers.unwatch(&program, watching.view).unwrap();

    let again = window
        .browsers
        .watch(
            &program,
            BrowserViewRole::Focused,
            Box::new(|_, _| {}),
            Arc::new(|_| {}),
        )
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
                true,
                &AsPersona::default,
                &|_| Err("no login here".into()),
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
        .watch(
            &program,
            BrowserViewRole::Focused,
            Box::new(|_, _| {}),
            Arc::new(|_| {}),
        )
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

/// Where a browser view was said to stand, in the order it was said (#2542).
type Said = Arc<Mutex<Vec<Reconnection>>>;

/// A browser opened and watched, its first frame drawn: what its view was said, its view, and
/// its program.
fn a_watched_browser(window: &Window) -> (Said, u32, String) {
    let program = window.browsers.open(&window.root).unwrap();
    let screen = window.started.started()[0].clone();
    let drawn: Arc<Mutex<usize>> = Arc::default();
    let said: Said = Arc::default();
    let watching = window
        .browsers
        .watch(
            &program,
            BrowserViewRole::Focused,
            Box::new({
                let drawn = Arc::clone(&drawn);
                move |_, _| *drawn.lock().unwrap() += 1
            }),
            Arc::new({
                let said = Arc::clone(&said);
                move |now| said.lock().unwrap().push(now)
            }),
        )
        .unwrap();
    screen.makes(b"\xff\xd8 a page");
    until("the first frame is drawn", || *drawn.lock().unwrap() == 1);
    (said, watching.view, program)
}

#[test]
fn a_browser_view_on_a_runner_is_reconnecting_while_its_link_is_down_and_reconnected_after() {
    let (window, pushes) = window_on_a_runner();
    let (said, _, _) = a_watched_browser(&window);
    assert!(
        said.lock().unwrap().is_empty(),
        "nothing said while it is up"
    );

    pushes
        .send(Pushed::LinkDown {
            why: "the link to it ended".into(),
        })
        .unwrap();
    until("the view is said to be reconnecting", || {
        *said.lock().unwrap() == [Reconnection::Reconnecting]
    });
    pushes.send(Pushed::LinkBack {}).unwrap();

    until("the view is said to be reconnected", || {
        *said.lock().unwrap() == [Reconnection::Reconnecting, Reconnection::Reconnected]
    });
}

#[test]
fn a_browser_view_that_draws_again_from_a_snapshot_is_reconnected() {
    let (window, pushes) = window_on_a_runner();
    let (said, view, _) = a_watched_browser(&window);
    pushes
        .send(Pushed::LinkDown {
            why: "the link to it ended".into(),
        })
        .unwrap();
    until("the view is said to be reconnecting", || {
        *said.lock().unwrap() == [Reconnection::Reconnecting]
    });

    // Its link_back went missing; its stream drawn again, from a snapshot, says it is back.
    let link = window.browsers.link().unwrap();
    window.browsers.heard(link.id(), Reconnect::Drawn(view));

    assert_eq!(
        *said.lock().unwrap(),
        [Reconnection::Reconnecting, Reconnection::Reconnected]
    );
}

#[test]
fn a_browser_views_first_frame_says_it_drew_from_a_snapshot() {
    let window = window();
    let heard: Arc<Mutex<Vec<Reconnect>>> = Arc::default();
    window.browsers.link().unwrap().when_reconnecting({
        let heard = Arc::clone(&heard);
        Arc::new(move |one| heard.lock().unwrap().push(one))
    });

    let (_, view, _) = a_watched_browser(&window);

    until("its first frame is heard", || {
        *heard.lock().unwrap() == [Reconnect::Drawn(view)]
    });
}

#[test]
fn a_browser_view_closed_while_its_link_is_down_is_told_nothing_more() {
    let (window, pushes) = window_on_a_runner();
    let (said, view, program) = a_watched_browser(&window);
    pushes
        .send(Pushed::LinkDown {
            why: "the link to it ended".into(),
        })
        .unwrap();
    until("the view is said to be reconnecting", || {
        *said.lock().unwrap() == [Reconnection::Reconnecting]
    });

    window.browsers.unwatch(&program, view).unwrap();
    let link = window.browsers.link().unwrap();
    window.browsers.heard(link.id(), Reconnect::Over);

    assert_eq!(*said.lock().unwrap(), [Reconnection::Reconnecting]);
}

#[test]
fn the_views_of_a_window_link_that_was_replaced_are_followed_no_more() {
    let window = window();
    let (_said, view, _) = a_watched_browser(&window);
    let old = window.browsers.link().unwrap().id();
    let (_connection, replaced) = crate::hostedsessions::tests::in_memory(
        Hosting::running_with(ComposesNothing, StandIns::default()),
        &window._runtime,
    );

    window.browsers.follow(&replaced, view, Arc::new(|_| {}));

    let followed: Vec<(u64, u32)> = super::lock(&window.browsers.dimming)
        .views
        .keys()
        .copied()
        .collect();
    assert_eq!(followed, [(replaced.id(), view)]);
    assert_ne!(old, replaced.id());
}

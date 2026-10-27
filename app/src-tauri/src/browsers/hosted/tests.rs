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
    window_of(|started| started, host)
}

/// [`window_with`], its stand-in browsers as `browsers` makes them.
fn window_of<H: purlis_session_protocol::session::Host + Clone>(
    browsers: impl FnOnce(StandInBrowsers) -> StandInBrowsers,
    host: impl FnOnce(Hosting) -> H,
) -> Window {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let store = tempfile::tempdir().unwrap();
    let started = browsers(StandInBrowsers::in_folder(store.path().join("browsers")));
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
            Box::new(|_| {}),
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
fn a_first_browser_start_past_the_hosts_usual_answer_time_still_starts() {
    // The first start on a host downloads the engine inside it: it is waited for past the
    // 30 seconds any other answer gets, as long as the download may take.
    let window = window_of(StandInBrowsers::holding_starts, |hosting| hosting);
    let opened = std::thread::scope(|scope| {
        let opening = scope.spawn(|| window.browsers.open(&window.root));
        until("the start is under way", || window.started.waiting() == 1);
        std::thread::sleep(crate::hostedsessions::ANSWER_WITHIN + Duration::from_secs(2));
        window.started.let_starts_go();
        opening.join().unwrap()
    });
    assert!(opened.is_ok(), "{opened:?}");
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

#[test]
fn a_start_the_host_refuses_for_its_devices_budget_is_the_device_full_refusal_naming_the_setting() {
    // #2611: the host holds every client to the budget; the window's own count lets this
    // one through, and the host's refusal is still the person's device-full sentence.
    let window = window_of(|browsers| browsers.budgeted(1), |hosting| hosting);
    let browsers = WindowBrowsers::new(Arc::clone(&window.browsers.connection)).budgeted(1);
    let generous = WindowBrowsers::new(Arc::clone(&window.browsers.connection)).budgeted(8);
    browsers.open(&window.root).unwrap();

    let refused = generous.open(&window.root).unwrap_err();

    assert!(refused.contains("on this device"), "{refused}");
    assert!(
        refused.contains(purlis_core::browserlimits::SETTING),
        "{refused}"
    );
    assert_eq!(window.started.started().len(), 1, "nothing more started");
}

#[test]
fn a_start_refused_with_a_code_this_window_does_not_know_is_refused_in_the_hosts_words() {
    use crate::chatbrowsers::NotStarted;
    use purlis_session_protocol::session::{self, Refusal};

    let full = Refusal {
        code: session::DEVICE_FULL.into(),
        why: "the host's sentence".into(),
    };
    assert!(matches!(
        super::not_started(full, 3),
        NotStarted::Full { most: 3 }
    ));
    // A later minor's code, as `device_full` is to a window before 1.32: its `why` is shown.
    let later = Refusal {
        code: "a_later_minors_code".into(),
        why: "this device runs at most 3 browsers at once".into(),
    };
    match super::not_started(later, 3) {
        NotStarted::Failed(why) => assert_eq!(why, "this device runs at most 3 browsers at once"),
        other => panic!("a code it does not know is said in the host's words: {other:?}"),
    }
}

#[test]
fn a_start_the_host_refused_no_engine_sandbox_is_not_started_for_it_whatever_its_words() {
    // 1.36 (#2899): the code says it, not the words.
    use crate::chatbrowsers::NotStarted;
    use purlis_session_protocol::session;
    let refused = session::Refusal {
        code: session::NO_ENGINE_SANDBOX.into(),
        why: "worded in a way this window has never seen".into(),
    };
    assert_eq!(super::not_started(refused, 3), NotStarted::NoSandbox);
}

#[test]
fn a_start_the_host_refused_for_the_engines_sandbox_is_not_started_for_it() {
    // #2835: the host's words for it, whatever its code, are told apart from any failure.
    use crate::chatbrowsers::NotStarted;
    use purlis_session_protocol::session::Refusal;
    let refused = Refusal {
        code: "browser_failed".into(),
        why: format!(
            "the browser did not start: {}",
            purlis_core::browsers::sandboxing::NO_ENGINE_SANDBOX
        ),
    };
    assert_eq!(super::not_started(refused, 3), NotStarted::NoSandbox);
}

#[test]
fn a_start_whose_engine_download_the_person_cancelled_is_not_a_failure() {
    // #2207: the host's words for a cancel, in the core's sentence, are told apart from any
    // failure, so neither the chat nor the person is told to try again as if it broke.
    use crate::chatbrowsers::NotStarted;
    use purlis_session_protocol::session::Refusal;
    let refused = Refusal {
        code: "browser_failed".into(),
        why: purlis_core::browsers::download_cancelled_said(),
    };
    assert_eq!(super::not_started(refused, 3), NotStarted::Cancelled);
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
            Box::new(|_| {}),
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
            Box::new(|_| {}),
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
            Box::new(|_| {}),
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
            waited: false,
            asked,
        };
        if let Some(answer) = browserneedsyou::answered_first(&chats, &needs, &browse, 1000) {
            return answer;
        }
        chats
            .answer(
                Some(&driver),
                &window.root,
                None,
                &browse,
                &BrowserReach::default,
                false,
                &AsPersona::default,
                &|_| Err("no login here".to_owned()),
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
            Box::new(|_| {}),
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
            Box::new(|_| {}),
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
            key_changed: false,
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
            key_changed: false,
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
            key_changed: false,
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

// ---- the person sends their browser to an address (#2176) -----------------------------------

fn reaching(host: &str) -> purlis_session_protocol::browsing::BrowserReach {
    purlis_session_protocol::browsing::BrowserReach {
        listed: vec![purlis_session_protocol::session::Listed {
            host: host.into(),
            layer: purlis_session_protocol::session::Layer::Project,
        }],
        ..Default::default()
    }
}

fn a_page(page: &str, address: &str) -> purlis_session_protocol::browsing::PageSaid {
    purlis_session_protocol::browsing::PageSaid {
        page: page.into(),
        address: address.into(),
        title: String::new(),
        current: true,
    }
}

#[test]
fn a_browser_the_person_opened_goes_to_their_address_on_its_current_page_reaching_what_it_is_given()
{
    use purlis_session_protocol::browsing::{Act, Browsed};

    let window = window();
    let program = window
        .browsers
        .open_reaching(&window.root, Some(reaching("api.example")), None)
        .unwrap();
    let screen = window.started.started()[0].clone();
    assert_eq!(screen.shown().reach, Some(reaching("api.example")));
    screen.answers(Ok(Browsed {
        settled: true,
        pages: vec![a_page("p1", "about:blank")],
        ..Default::default()
    }));
    screen.answers(Ok(Browsed {
        settled: true,
        page: Some(a_page("p1", "https://api.example/")),
        ..Default::default()
    }));

    let went = window
        .browsers
        .go(&program, "https://api.example/", &reaching("api.example"))
        .unwrap();

    assert_eq!(
        went,
        crate::browsers::Went::Went {
            address: Some("https://api.example/".into())
        }
    );
    let acts: Vec<Act> = screen
        .shown()
        .acts
        .iter()
        .map(|(_, act)| act.clone())
        .collect();
    assert_eq!(
        acts,
        [
            Act::Pages,
            Act::Navigate {
                page: Some("p1".into()),
                address: "https://api.example/".into()
            }
        ]
    );
}

#[test]
fn a_browser_with_no_page_opens_one_at_the_address() {
    use purlis_session_protocol::browsing::Act;

    let window = window();
    let program = window
        .browsers
        .open_reaching(&window.root, Some(reaching("api.example")), None)
        .unwrap();
    let screen = window.started.started()[0].clone();

    window
        .browsers
        .go(&program, "https://api.example/", &reaching("api.example"))
        .unwrap();

    assert_eq!(
        screen.shown().acts.last().map(|(_, act)| act.clone()),
        Some(Act::Open {
            address: "https://api.example/".into()
        })
    );
}

#[test]
fn a_site_new_to_the_project_held_back_is_named_to_ask_about() {
    use purlis_session_protocol::browsing::Browsed;

    let window = window();
    let program = window
        .browsers
        .open_reaching(&window.root, Some(reaching("api.example")), None)
        .unwrap();
    let screen = window.started.started()[0].clone();
    screen.answers(Ok(Browsed::default()));
    screen.answers(Ok(Browsed {
        settled: true,
        asked: vec!["example.com:443".into()],
        ..Default::default()
    }));

    let went = window
        .browsers
        .go(&program, "https://example.com/", &reaching("api.example"))
        .unwrap();

    assert_eq!(
        went,
        crate::browsers::Went::Asks {
            sites: vec!["example.com:443".into()],
            answers: Vec::new(),
        }
    );
}

#[test]
fn a_browser_the_person_drives_is_handed_back_and_goes_and_each_act_has_an_id_of_its_own() {
    let window = window();
    let program = window
        .browsers
        .open_reaching(&window.root, Some(reaching("api.example")), None)
        .unwrap();
    let screen = window.started.started()[0].clone();
    window.browsers.take_over(&program).unwrap();

    window
        .browsers
        .go(&program, "https://api.example/a", &reaching("api.example"))
        .unwrap();
    window
        .browsers
        .go(&program, "https://api.example/b", &reaching("api.example"))
        .unwrap();

    assert!(!screen.shown().taken_over, "handed back to go");
    let ids: Vec<String> = screen
        .shown()
        .acts
        .iter()
        .map(|(id, _)| id.clone())
        .collect();
    assert_eq!(ids.len(), 4);
    let unique: std::collections::HashSet<&String> = ids.iter().collect();
    assert_eq!(unique.len(), 4, "{ids:?}");
}

#[test]
fn a_browser_that_has_ended_says_so() {
    let window = window();
    let program = window
        .browsers
        .open_reaching(&window.root, Some(reaching("api.example")), None)
        .unwrap();
    window.browsers.close(&program).unwrap();

    let gone = window
        .browsers
        .go(&program, "https://api.example/", &reaching("api.example"))
        .unwrap_err();

    assert!(gone.contains("has ended"), "{gone}");
}

#[test]
fn a_browsers_end_is_logged_with_how_the_host_said_it_ended() {
    use crate::hostedsessions::Ending;
    use purlis_session_protocol::session::Exit;
    assert_eq!(
        super::ended_said("01K2", &Ending::Exited(Exit::Signal("SIGKILL".into()))),
        "purlis: browser 01K2 ended, its engine's signal SIGKILL"
    );
    assert_eq!(
        super::ended_said("01K2", &Ending::Exited(Exit::Code(0))),
        "purlis: browser 01K2 ended, its engine's exit code 0"
    );
    assert!(super::ended_said("01K2", &Ending::WithTheHost).contains("session host"));
}

#[test]
fn the_event_log_says_who_drives_a_browser_and_that_someone_typed_never_what() {
    // #2293: a take-over, the person's keys, the hand-back, and a login purlis performs.
    use crate::chatbrowsers::Driver;
    use purlis_core::eventlog::browsing::{Driver as Drives, Said, Typist};
    use purlis_core::eventlog::{ArgsKey, Entry, Log, Recorder};
    use purlis_session_protocol::browsing::{LogIn, Secret};

    let Window {
        browsers,
        started,
        root,
        _dirs,
        _runtime,
    } = window();
    type SaidAt = (Option<std::path::PathBuf>, Said);
    let said: Arc<Mutex<Vec<SaidAt>>> = Arc::default();
    let saying = Arc::clone(&said);
    let browsers = browsers.saying(Arc::new(move |root, one| {
        saying
            .lock()
            .unwrap()
            .push((root.map(std::path::Path::to_path_buf), one));
    }));
    let program = browsers.open(&root).unwrap();
    let screen = started.started()[0].clone();
    browsers
        .watch(
            &program,
            BrowserViewRole::Focused,
            Box::new(|_, _| {}),
            Box::new(|_| {}),
            Arc::new(|_| {}),
        )
        .unwrap();
    screen.shown().document = 5;
    let key = |key: &str, physical: &str, action: KeyAct| BrowserInput::Key {
        aim: "p1:5".to_owned(),
        action,
        key: key.into(),
        physical: physical.into(),
        key_code: 81,
        modifiers: 0,
    };

    browsers.take_over(&program).unwrap();
    browsers.take_over(&program).unwrap();
    for typed in [
        key("Q", "KeyQ", KeyAct::Down),
        key("Q", "KeyQ", KeyAct::Up),
        key("Ж", "KeyZ", KeyAct::Down),
    ] {
        assert_eq!(
            browsers.input(&program, typed).unwrap(),
            InputOutcome::Taken
        );
    }
    browsers.hand_back(&program).unwrap();
    let login = LogIn {
        address: "https://shop.example.test/login?next=%2Fbasket".into(),
        user: Some(Secret::new("ada@shop.example.test".into())),
        password: Secret::new("correct horse battery staple".into()),
        keep: None,
        identity: None,
    };
    Driver::log_in(&browsers, &program, login).unwrap();

    let said = said.lock().unwrap().clone();
    assert!(
        said.iter()
            .all(|(at, _)| at.as_deref() == Some(root.as_path()))
    );
    let drives = |driver| Said::DriverChanged {
        browser: program.clone(),
        driver,
    };
    let typed = |by| Said::Typed {
        browser: program.clone(),
        by,
        origin: None,
    };
    assert_eq!(
        said.iter().map(|(_, one)| one.clone()).collect::<Vec<_>>(),
        [
            Said::Opened {
                browser: program.clone()
            },
            drives(Drives::Person),
            typed(Typist::Person),
            drives(Drives::Agent),
            drives(Drives::Purlis),
            typed(Typist::Purlis),
            drives(Drives::Agent),
        ],
        "each change once, and the first key of each stretch"
    );

    // Written and read back: nothing typed, filled or visited is in the log.
    let log = tempfile::tempdir().unwrap();
    let mut recorder = Recorder::new(
        Log::open(log.path(), "01J9ZZDEVICE0000000000000").unwrap(),
        ArgsKey::open(log.path()).unwrap(),
    );
    for (_, said) in said {
        recorder
            .apply(Entry::Browser {
                plane: root.clone(),
                number: None,
                said,
            })
            .unwrap();
    }
    let written = std::fs::read_to_string(log.path().join(purlis_core::eventlog::FILE)).unwrap();
    assert_eq!(written.lines().count(), 7);
    for never in [
        "KeyQ",
        "\"Q\"",
        "Ж",
        "KeyZ",
        "p1:5",
        "ada@",
        "correct horse",
        "shop.example.test",
        "basket",
    ] {
        assert!(
            !written.contains(never),
            "{never:?} is in the log: {written}"
        );
    }
}

#[test]
fn a_person_sending_their_browser_to_an_address_hands_it_back_in_the_log() {
    // #2293: an address the person types hands the browser back first, and says so.
    use purlis_core::eventlog::browsing::{Driver as Drives, Said};
    use purlis_session_protocol::browsing::BrowserReach;

    let Window {
        browsers,
        started: _started,
        root,
        _dirs,
        _runtime,
    } = window();
    let said: Arc<Mutex<Vec<Said>>> = Arc::default();
    let saying = Arc::clone(&said);
    let browsers = browsers.saying(Arc::new(move |_, one| saying.lock().unwrap().push(one)));
    let program = browsers.open(&root).unwrap();

    browsers.take_over(&program).unwrap();
    let _ = browsers.go(&program, "http://localhost:5173/", &BrowserReach::default());

    let driver = |driver| Said::DriverChanged {
        browser: program.clone(),
        driver,
    };
    assert_eq!(
        *said.lock().unwrap(),
        [
            Said::Opened {
                browser: program.clone()
            },
            driver(Drives::Person),
            driver(Drives::Agent)
        ]
    );
}

#[test]
fn a_window_started_again_says_nothing_of_a_login_the_host_refuses_as_the_person_drives() {
    // #2293: the person took it over under the window before; this one has watched nothing.
    use crate::chatbrowsers::{Driver, Refused};
    use purlis_core::eventlog::browsing::Said;
    use purlis_session_protocol::browsing::{LogIn, Secret};

    let window = window();
    let program = window.browsers.open(&window.root).unwrap();
    window.browsers.take_over(&program).unwrap();

    let said: Arc<Mutex<Vec<Said>>> = Arc::default();
    let saying = Arc::clone(&said);
    let again = WindowBrowsers::new(Arc::clone(&window.browsers.connection))
        .saying(Arc::new(move |_, one| saying.lock().unwrap().push(one)));
    let login = LogIn {
        address: "https://shop.example.test/login".into(),
        user: None,
        password: Secret::new("correct horse battery staple".into()),
        keep: None,
        identity: None,
    };
    let refused = Driver::log_in(&again, &program, login);

    assert!(
        matches!(&refused, Err(Refused::Said(why)) if why.contains("the person has taken this browser over")),
        "{refused:?}"
    );
    assert!(
        said.lock().unwrap().is_empty(),
        "the person drives it: no driver of purlis's or the agent's is said"
    );
}

#[test]
fn a_browser_view_is_told_its_pages_origin_alone_as_the_page_moves() {
    // #2300: what the view's chip names, its origin and nothing else of its address.
    let window = window();
    let program = window.browsers.open(&window.root).unwrap();
    let screen = window.started.started()[0].clone();
    let origins: Arc<Mutex<Vec<Option<String>>>> = Arc::default();
    let heard = Arc::clone(&origins);
    let drawn: Arc<Mutex<usize>> = Arc::default();
    let drawing = Arc::clone(&drawn);
    window
        .browsers
        .watch(
            &program,
            BrowserViewRole::Focused,
            Box::new(move |_, _| *drawing.lock().unwrap() += 1),
            Box::new(move |origin| heard.lock().unwrap().push(origin)),
            Arc::new(|_| {}),
        )
        .unwrap();

    screen.shown().address = "http://localhost:5173/login?next=%2Fadmin".into();
    screen.makes(b"\xff\xd8 one");
    until("the first frame", || *drawn.lock().unwrap() == 1);
    screen.shown().address = "https://docs.example.test/a?token=s3cret".into();
    screen.makes(b"\xff\xd8 two");
    until("the second frame", || *drawn.lock().unwrap() == 2);

    assert_eq!(
        *origins.lock().unwrap(),
        [
            Some("http://localhost:5173".to_owned()),
            Some("https://docs.example.test".to_owned())
        ]
    );
}

/// A window whose host runs stand-in browsers, and whose links to pool `builds`'s runner reach a
/// runner whose browsers are stand-ins of their own (#2050).
struct WithARunner {
    window: Window,
    on_the_runner: StandInBrowsers,
    _store: tempfile::TempDir,
}

fn a_window_with_a_runner() -> WithARunner {
    let store = tempfile::tempdir().unwrap();
    let on_the_runner = StandInBrowsers::in_folder(store.path().join("runner-browsers"));
    // A runner keeps its projects where it keeps them (#2046).
    let runner = Hosting::running_with(ComposesNothing, StandIns::default())
        .with_browsers(on_the_runner.clone())
        .keeping_projects_in(store.path().join("projects"));
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let here_store = tempfile::tempdir().unwrap();
    let started = StandInBrowsers::in_folder(here_store.path().join("browsers"));
    let hosting =
        Hosting::running_with(ComposesNothing, StandIns::default()).with_browsers(started.clone());
    let connection =
        crate::hostedsessions::tests::in_memory_with_a_runner(hosting, runner, &runtime);
    let project = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(project.path()).unwrap();
    std::fs::write(root.join(purlis_core::plane::MANIFEST), "").unwrap();
    WithARunner {
        window: Window {
            browsers: WindowBrowsers::new(connection),
            started,
            root,
            _dirs: (here_store, project),
            _runtime: runtime,
        },
        on_the_runner,
        _store: store,
    }
}

#[test]
fn a_runner_chats_browser_starts_on_its_pools_runner_and_is_driven_there() {
    use crate::chatbrowsers::Driver;
    let with = a_window_with_a_runner();
    let window = &with.window;

    let program = window
        .browsers
        .start(
            &window.root,
            Some("builds"),
            purlis_session_protocol::browsing::BrowserReach::default(),
            None,
        )
        .expect("it starts on the runner");
    let browsed = window
        .browsers
        .act(
            &program,
            "c-1",
            purlis_session_protocol::browsing::Act::Pages,
            purlis_session_protocol::browsing::BrowserReach::default(),
        )
        .expect("its act reaches it there");

    assert!(browsed.settled);
    assert_eq!(with.on_the_runner.started().len(), 1, "on the runner");
    assert!(
        window.started.started().is_empty(),
        "and not on this machine"
    );
    assert_eq!(with.on_the_runner.started()[0].shown().acts.len(), 1);
    assert_eq!(window.browsers.pool_of(&program).as_deref(), Some("builds"));
}

#[test]
fn a_runner_browsers_view_says_its_pool_and_the_persons_input_reaches_it_there() {
    use crate::chatbrowsers::Driver;
    let with = a_window_with_a_runner();
    let window = &with.window;
    let program = window
        .browsers
        .start(
            &window.root,
            Some("builds"),
            purlis_session_protocol::browsing::BrowserReach::default(),
            None,
        )
        .unwrap();
    let screen = with.on_the_runner.started()[0].clone();

    let watching = window
        .browsers
        .watch(
            &program,
            BrowserViewRole::Focused,
            Box::new(|_, _| {}),
            Box::new(|_| {}),
            Arc::new(|_| {}),
        )
        .expect("its view attaches on the runner");
    window.browsers.take_over(&program).unwrap();
    let typed = window
        .browsers
        .input(
            &program,
            BrowserInput::Key {
                aim: "p1:0".into(),
                action: KeyAct::Down,
                key: "a".into(),
                physical: "KeyA".into(),
                key_code: 65,
                modifiers: 0,
            },
        )
        .unwrap();

    assert_eq!(
        watching.pool.as_deref(),
        Some("builds"),
        "its chip names its pool"
    );
    assert_eq!(typed, InputOutcome::Taken);
    assert_eq!(screen.shown().inputs.len(), 1);
}

#[test]
fn a_runner_chats_browser_is_never_opened_from_a_kept_login() {
    use crate::chatbrowsers::Driver;
    let with = a_window_with_a_runner();
    let window = &with.window;

    window
        .browsers
        .start(
            &window.root,
            Some("builds"),
            purlis_session_protocol::browsing::BrowserReach::default(),
            Some(purlis_session_protocol::browsing::KeptLogin {
                profile: "claude".into(),
                persona: "steward".into(),
                login: "shop-login".into(),
                identity: "0123456789abcdef0123456789abcdef".into(),
                address: "https://shop.example.test/login".into(),
            }),
        )
        .expect("it starts, from no kept login");

    assert_eq!(with.on_the_runner.started()[0].shown().kept, None);
}

#[test]
fn typing_is_said_on_the_origin_of_the_page_and_the_log_holds_no_path_query_or_value() {
    // #2842: the person types on a page whose view was told where it is, and purlis logs in on
    // a page it answers with; the log names each page by its origin alone.
    use crate::chatbrowsers::Driver;
    use purlis_core::eventlog::browsing::{Said, Typist};
    use purlis_core::eventlog::{ArgsKey, Entry, Log, Recorder};
    use purlis_session_protocol::browsing::{Browsed, LogIn, PageSaid, Secret};

    let Window {
        browsers,
        started,
        root,
        _dirs,
        _runtime,
    } = window();
    let said: Arc<Mutex<Vec<Said>>> = Arc::default();
    let saying = Arc::clone(&said);
    let browsers = browsers.saying(Arc::new(move |_, one| saying.lock().unwrap().push(one)));
    let program = browsers.open(&root).unwrap();
    let screen = started.started()[0].clone();
    screen.shown().address = "https://shop.example.test/basket?item=42#pay".into();
    let origins: Arc<Mutex<Vec<Option<String>>>> = Arc::default();
    let heard = Arc::clone(&origins);
    browsers
        .watch(
            &program,
            BrowserViewRole::Focused,
            Box::new(|_, _| {}),
            Box::new(move |origin| heard.lock().unwrap().push(origin)),
            Arc::new(|_| {}),
        )
        .unwrap();
    until("the view hears where its page is", || {
        screen.makes(b"frame");
        !origins.lock().unwrap().is_empty()
    });
    let aim = format!("p1:{}", screen.shown().document);
    browsers.take_over(&program).unwrap();
    let typed = browsers
        .input(
            &program,
            BrowserInput::Key {
                aim,
                action: KeyAct::Down,
                key: "h".into(),
                physical: "KeyH".into(),
                key_code: 72,
                modifiers: 0,
            },
        )
        .unwrap();
    assert_eq!(typed, InputOutcome::Taken);
    browsers.hand_back(&program).unwrap();
    screen.answers(Ok(Browsed {
        settled: true,
        page: Some(PageSaid {
            page: "p1".into(),
            address: "https://accounts.example.test:8443/welcome?session=s3cret".into(),
            title: "Welcome, Ada".into(),
            current: true,
        }),
        ..Browsed::default()
    }));
    let login = LogIn {
        address: "https://accounts.example.test:8443/login?next=%2Fbasket".into(),
        user: Some(Secret::new("ada@shop.example.test".into())),
        password: Secret::new("correct horse battery staple".into()),
        keep: None,
        identity: None,
    };
    Driver::log_in(&browsers, &program, login).unwrap();

    let said = said.lock().unwrap().clone();
    let typing: Vec<&Said> = said
        .iter()
        .filter(|one| matches!(one, Said::Typed { .. }))
        .collect();
    assert_eq!(
        typing,
        [
            &Said::Typed {
                browser: program.clone(),
                by: Typist::Person,
                origin: Some("https://shop.example.test".into()),
            },
            &Said::Typed {
                browser: program.clone(),
                by: Typist::Purlis,
                origin: Some("https://accounts.example.test:8443".into()),
            },
        ]
    );

    // Written and read back: no path, query, fragment, title or value is in the log.
    let log = tempfile::tempdir().unwrap();
    let mut recorder = Recorder::new(
        Log::open(log.path(), "01J9ZZDEVICE0000000000000").unwrap(),
        ArgsKey::open(log.path()).unwrap(),
    );
    for said in said {
        recorder
            .apply(Entry::Browser {
                plane: root.clone(),
                number: None,
                said,
            })
            .unwrap();
    }
    let written = std::fs::read_to_string(log.path().join(purlis_core::eventlog::FILE)).unwrap();
    assert!(
        written.contains("\"origin\":\"https://shop.example.test\""),
        "{written}"
    );
    for never in [
        "/basket",
        "item=42",
        "#pay",
        "/welcome",
        "session=s3cret",
        "/login",
        "next=",
        "Welcome, Ada",
        "ada@",
        "correct horse",
        "KeyH",
        "\"h\"",
    ] {
        assert!(
            !written.contains(never),
            "{never:?} is in the log: {written}"
        );
    }
}

#[test]
fn a_browsers_end_the_window_did_not_ask_for_is_said_crashed_and_one_it_asked_for_is_not() {
    // #2842: a chat's close and the kill switch are ends the window asked for; an engine that
    // ends by itself crashed.
    use crate::chatbrowsers::Driver;
    use purlis_core::eventlog::browsing::{Closer, Said};
    use purlis_session_protocol::session::Exit;

    let Window {
        browsers,
        started,
        root,
        _dirs,
        _runtime,
    } = window();
    let said: Arc<Mutex<Vec<Said>>> = Arc::default();
    let saying = Arc::clone(&said);
    let browsers = browsers.saying(Arc::new(move |_, one| saying.lock().unwrap().push(one)));
    let closed = browsers.open(&root).unwrap();
    let crashing = browsers.open(&root).unwrap();
    let stopped = browsers.open(&root).unwrap();
    let screens = started.started();

    Driver::close(&browsers, &closed).unwrap();
    screens[1].ends(Exit::Signal("SIGSEGV".into()));
    until("the crash is said", || {
        said.lock()
            .unwrap()
            .iter()
            .any(|one| matches!(one, Said::Crashed { .. }))
    });
    browsers.stop_every_browser();
    // Long enough for an end the host tells of to be said, were it said.
    std::thread::sleep(std::time::Duration::from_millis(300));

    let ends: Vec<Said> = said
        .lock()
        .unwrap()
        .iter()
        .filter(|one| !matches!(one, Said::Opened { .. }))
        .cloned()
        .collect();
    assert_eq!(
        ends,
        [
            Said::Crashed { browser: crashing },
            Said::Closed {
                browser: stopped,
                by: Closer::KillSwitch
            },
        ],
        "a close the window asked for is said by whoever asked it, never as a crash"
    );
}

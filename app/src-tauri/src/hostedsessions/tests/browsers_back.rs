// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Browsers survive a crash of the window** (#2025), over in-memory links to a real host whose
//! programs and browsers are stand-ins: a window starts a chat's browser, the person takes it
//! over, and the window crashes; the window that comes back reattaches the chat and takes its
//! browser back with it, under the name the chat knew, the person still driving, its view drawn
//! from the next whole frame and taking the person's click. A browser nothing takes back ends.

use std::sync::{Arc, Mutex};

use purlis_core::hookwire::{BrowseAsk, Browsing};
use purlis_session_protocol::browsing::{Act, BrowserReach};

use super::*;
use crate::browsers::{
    BrowserInput, BrowserViewRole, InputOutcome, MouseAct, MouseButton, WindowBrowsers,
};
use crate::chatbrowsers::{AsPersona, BrowserDriver, ChatBrowser, ChatBrowsers, Driver};
use purlis_session_protocol::session::HeldProgram;

/// A window's browsers on its connection: the host's, and its chats', which note each there.
struct Browsing2 {
    window: Arc<WindowBrowsers>,
    driver: Arc<dyn Driver>,
    chats: ChatBrowsers,
}

/// Each chat's identity, as the app's record of it says.
fn id_of(chat: u32) -> String {
    format!("01JA0CHT{chat:018}")
}

fn browsing(window: &AWindow, root: &std::path::Path) -> Browsing2 {
    let browsers = Arc::new(WindowBrowsers::new(Arc::clone(&window.connection)));
    let driver: Arc<dyn Driver> = browsers.clone();
    Browsing2 {
        window: browsers,
        chats: ChatBrowsers::default().noting(Some(Arc::clone(&driver)), root),
        driver,
    }
}

/// What a window that comes back with chats `open` open does with the browsers `left`: every
/// chat may log in with what it holds, and has no Always.
fn take_back(again: &Browsing2, left: Vec<HeldProgram>, open: &[u32], root: &std::path::Path) {
    let opened = |session: u32| open.contains(&session).then(|| Some(id_of(session)));
    crate::chatbrowsers::take_back_from(
        left,
        &crate::chatbrowsers::Back {
            open: &opened,
            as_persona: &|_| AsPersona::default(),
            may_log_in: &|_, _| Ok(()),
        },
        &again.chats,
        Some(&again.driver),
        root,
    );
}

impl Browsing2 {
    /// One browser call of chat `chat`'s, as its tool server sends it.
    fn call(&self, root: &std::path::Path, chat: u32, asked: Browsing) {
        self.chats.identify(chat, Some(id_of(chat)));
        self.chats.answer(
            Some(&self.driver),
            root,
            None,
            &BrowseAsk {
                chat,
                browser: None,
                call_id: format!("c-{chat}"),
                waited: false,
                asked,
            },
            &BrowserReach::default,
            false,
            &AsPersona::default,
            &|_| Err("no logins here".into()),
            &|| Err("no folder here".to_owned()),
            1000,
        );
        // As every command of the window's ends: its browsers' notes follow.
        self.chats.renote();
    }

    /// Chat `chat`'s first browser, allowed by the person, and its program.
    fn a_browser_of(&self, root: &std::path::Path, chat: u32) -> String {
        self.call(root, chat, Browsing::Act(Act::Pages));
        self.chats.allow(chat, false);
        self.call(root, chat, Browsing::Act(Act::Pages));
        self.chats.which(chat, None).expect("its browser").1
    }
}

/// A frame a view drew: what it shows, and its image.
type Drawn = (Option<String>, Vec<u8>);

/// The note the host keeps of each browser of the project at `root`, as `window` reads it.
fn notes_kept(
    window: &AWindow,
    root: &std::path::Path,
) -> Vec<Option<purlis_session_protocol::session::ProgramNote>> {
    let project = purlis_core::scaffold::planefile::id_to_hold_by(root).unwrap();
    let link = window.connection.of_project(root, PATIENCE).unwrap();
    match link.call(Command::ProgramsOf { project }).unwrap() {
        Answer::Programs(programs) => programs
            .into_iter()
            .filter(|program| program.kind == ProgramKind::Browser)
            .map(|program| program.note)
            .collect(),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_chats_browser_comes_back_with_its_chat_its_lease_and_its_view_after_the_window_crashes() {
    let host = Comeback::new();
    let first = host.window();
    let browsers = browsing(&first, &host.root);
    for number in [3, 4] {
        first
            .sessions
            .open(Some(number), &a_shell("true"), &|_| {})
            .unwrap();
    }
    let kept = browsers.a_browser_of(&host.root, 3);
    let _left = browsers.a_browser_of(&host.root, 4);
    browsers.window.take_over(&kept).unwrap();
    until("the host keeps a note of each browser", || {
        notes_kept(&first, &host.root).iter().all(Option::is_some)
    });
    let screens = host.browsers.started();
    let (kept_screen, left_screen) = (screens[0].clone(), screens[1].clone());
    first.crashes();
    std::mem::forget(browsers);

    // The window comes back with chat 3 alone: chat 4's record was not put back.
    let back = host.window();
    back.sessions.reattach(3, &|_| {}).expect("chat 3 is back");
    back.sessions.close_what_was_not_reattached();
    let again = browsing(&back, &host.root);
    let left = back.sessions.browsers_left();
    assert_eq!(left.len(), 2, "{left:?}");
    take_back(&again, left, &[3], &host.root);

    assert_eq!(
        again.chats.listed(),
        [ChatBrowser {
            session: 3,
            name: "b1".into(),
            program: kept.clone(),
            driver: Some(BrowserDriver::Person),
            ..ChatBrowser::default()
        }],
        "back with its chat, under the name it knew, the person still driving"
    );
    until("the browser nothing took back is ended", || {
        left_screen.shown().stopped
    });
    assert!(!kept_screen.shown().stopped);

    let drawn: Arc<Mutex<Vec<Drawn>>> = Arc::default();
    let drawing = Arc::clone(&drawn);
    let watching = again
        .window
        .watch(
            &kept,
            BrowserViewRole::Focused,
            Box::new(move |aim, image| drawing.lock().unwrap().push((aim, image.to_vec()))),
            Box::new(|_| {}),
            Arc::new(|_| {}),
        )
        .expect("its view is opened again");
    assert!(watching.person_drives, "the lease is as it was");
    kept_screen.shown().document = 2;
    kept_screen.makes(b"\xff\xd8 after the crash");
    until("its view draws the next whole frame", || {
        drawn
            .lock()
            .unwrap()
            .iter()
            .any(|(_, image)| image == b"\xff\xd8 after the crash")
    });
    let aim = drawn.lock().unwrap().last().unwrap().0.clone().unwrap();

    let clicked = again
        .window
        .input(
            &kept,
            BrowserInput::Mouse {
                aim,
                action: MouseAct::Pressed,
                x: 20,
                y: 30,
                button: MouseButton::Left,
                clicks: 1,
                modifiers: 0,
                delta_x: 0,
                delta_y: 0,
            },
        )
        .unwrap();

    assert_eq!(clicked, InputOutcome::Taken);
    assert_eq!(
        kept_screen.shown().inputs.len(),
        1,
        "the click reached the page"
    );
    assert_eq!(
        host.browsers.started().len(),
        2,
        "nothing was started again"
    );
}

#[test]
fn a_browser_the_window_kept_no_note_of_is_ended_when_it_comes_back() {
    let host = Comeback::new();
    let first = host.window();
    let browsers = browsing(&first, &host.root);
    // Started without a note: a window that crashed before it noted it.
    let program = browsers.window.open(&host.root).unwrap();
    first.crashes();
    std::mem::forget(browsers);

    let back = host.window();
    let again = browsing(&back, &host.root);
    let left = back.sessions.browsers_left();
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].program, program);
    take_back(&again, left, &[], &host.root);

    let screen = host.browsers.started()[0].clone();
    until("it is ended", || screen.shown().stopped);
    assert!(again.chats.listed().is_empty());
}

#[test]
fn the_persons_own_browser_comes_back_for_a_view_tab_of_its_own() {
    let host = Comeback::new();
    let first = host.window();
    let browsers = browsing(&first, &host.root);
    let program = browsers
        .window
        .open_for_the_person(&host.root, None)
        .unwrap();
    browsers.chats.opened_by_the_person(&program);
    browsers.chats.renote();
    until("the host keeps its note", || {
        notes_kept(&first, &host.root).iter().all(Option::is_some)
    });
    first.crashes();
    std::mem::forget(browsers);

    let back = host.window();
    let again = browsing(&back, &host.root);
    let left = back.sessions.browsers_left();
    take_back(&again, left, &[], &host.root);

    assert_eq!(again.chats.take_theirs_back(), [program]);
    assert!(!host.browsers.started()[0].shown().stopped);
}

#[test]
fn a_browser_whose_engine_ended_while_no_window_was_there_is_let_go_of() {
    let host = Comeback::new();
    let first = host.window();
    let browsers = browsing(&first, &host.root);
    first
        .sessions
        .open(Some(3), &a_shell("true"), &|_| {})
        .unwrap();
    browsers.a_browser_of(&host.root, 3);
    until("the host keeps its note", || {
        notes_kept(&first, &host.root).iter().all(Option::is_some)
    });
    first.crashes();
    std::mem::forget(browsers);
    host.browsers.started()[0].ends(Exit::Code(1));

    let back = host.window();
    back.sessions.reattach(3, &|_| {}).expect("chat 3 is back");
    let again = browsing(&back, &host.root);
    let left = back.sessions.browsers_left();
    assert!(left[0].exit.is_some(), "the host says it ended");
    take_back(&again, left, &[3], &host.root);

    assert!(again.chats.listed().is_empty(), "nothing of it comes back");
}

#[test]
fn a_browser_whose_note_does_not_read_is_ended_when_the_window_comes_back() {
    let host = Comeback::new();
    let first = host.window();
    let browsers = browsing(&first, &host.root);
    let program = browsers.window.open(&host.root).unwrap();
    first
        .connection
        .of_project(&host.root, PATIENCE)
        .unwrap()
        .call(Command::NoteProgram {
            program: program.clone(),
            note: purlis_session_protocol::session::ProgramNote(
                serde_json::json!({"owner": "someone else's"}),
            ),
        })
        .expect("a note is kept, read or not");
    first.crashes();
    std::mem::forget(browsers);

    let back = host.window();
    let again = browsing(&back, &host.root);
    let left = back.sessions.browsers_left();
    take_back(&again, left, &[], &host.root);

    let screen = host.browsers.started()[0].clone();
    until("it is ended", || screen.shown().stopped);
}

#[test]
fn a_relink_never_hands_back_a_browser_this_window_started_since() {
    let host = Comeback::new();
    let first = host.window();
    let first_browsers = browsing(&first, &host.root);
    let before = first_browsers.window.open(&host.root).unwrap();
    first.crashes();
    std::mem::forget(first_browsers);

    let back = host.window();
    let again = browsing(&back, &host.root);
    back.sessions.hold().expect("held");
    // A browser this window starts, and a re-link that holds the project again.
    let since = again.window.open(&host.root).unwrap();
    for cut in back.cuts.lock().unwrap().iter() {
        cut.abort();
    }
    let left: Vec<String> = back
        .sessions
        .browsers_left()
        .into_iter()
        .map(|one| one.program)
        .collect();

    assert_eq!(left, [before], "only those of the first hold: not {since}");
}

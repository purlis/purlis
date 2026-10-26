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

/// A window's browsers on its connection: the host's, and its chats', which note each there.
struct Browsing2 {
    window: Arc<WindowBrowsers>,
    driver: Arc<dyn Driver>,
    chats: ChatBrowsers,
}

fn browsing(window: &AWindow) -> Browsing2 {
    let browsers = Arc::new(WindowBrowsers::new(Arc::clone(&window.connection)));
    let driver: Arc<dyn Driver> = browsers.clone();
    Browsing2 {
        window: browsers,
        chats: ChatBrowsers::default().noting(Some(Arc::clone(&driver))),
        driver,
    }
}

impl Browsing2 {
    /// One browser call of chat `chat`'s, as its tool server sends it.
    fn call(&self, root: &std::path::Path, chat: u32, asked: Browsing) {
        self.chats.answer(
            Some(&self.driver),
            root,
            &BrowseAsk {
                chat,
                browser: None,
                call_id: format!("c-{chat}"),
                asked,
            },
            &BrowserReach::default,
            false,
            &AsPersona::default,
            &|_| Err("no logins here".into()),
            1000,
        );
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
fn notes_kept(window: &AWindow, root: &std::path::Path) -> Vec<Option<serde_json::Value>> {
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
    let browsers = browsing(&first);
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
    let again = browsing(&back);
    let left = back.sessions.browsers_left();
    assert_eq!(left.len(), 2, "{left:?}");
    crate::chatbrowsers::take_back_from(left, &[3], &again.chats, Some(&again.driver), &host.root);

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
    let browsers = browsing(&first);
    // Started without a note: a window that crashed before it noted it.
    let program = browsers.window.open(&host.root).unwrap();
    first.crashes();
    std::mem::forget(browsers);

    let back = host.window();
    let again = browsing(&back);
    let left = back.sessions.browsers_left();
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].program, program);
    crate::chatbrowsers::take_back_from(left, &[], &again.chats, Some(&again.driver), &host.root);

    let screen = host.browsers.started()[0].clone();
    until("it is ended", || screen.shown().stopped);
    assert!(again.chats.listed().is_empty());
}

#[test]
fn the_persons_own_browser_comes_back_for_a_view_tab_of_its_own() {
    let host = Comeback::new();
    let first = host.window();
    let browsers = browsing(&first);
    let program = browsers
        .window
        .open_for_the_person(&host.root, None)
        .unwrap();
    until("the host keeps its note", || {
        notes_kept(&first, &host.root).iter().all(Option::is_some)
    });
    first.crashes();
    std::mem::forget(browsers);

    let back = host.window();
    let again = browsing(&back);
    let left = back.sessions.browsers_left();
    crate::chatbrowsers::take_back_from(left, &[], &again.chats, Some(&again.driver), &host.root);

    assert_eq!(again.chats.take_theirs_back(), [program]);
    assert!(!host.browsers.started()[0].shown().stopped);
}

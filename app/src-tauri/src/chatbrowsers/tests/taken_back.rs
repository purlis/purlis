// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A chat's browsers survive a crash of the window (#2025): each is noted in its session host as
//! what the window knew of it changes, and a window that comes back takes it back by that note.
//! The first window is one `ChatBrowsers`, the one that comes back another, as after a crash.

use super::*;
use crate::chatbrowsers::{Note, TakenBack};

/// The browsers of a window and its note-keeping host.
fn noted() -> (ChatBrowsers, Arc<Hosted>) {
    let hosted = Arc::new(Hosted::default());
    let driver: Arc<dyn Driver> = hosted.clone();
    (ChatBrowsers::default().noting(Some(driver)), hosted)
}

fn note_of(hosted: &Hosted, program: &str) -> Note {
    hosted
        .notes
        .lock()
        .unwrap()
        .get(program)
        .cloned()
        .unwrap_or_else(|| panic!("no note of {program}"))
}

/// What a window that comes back with chats `back` open does with `program`'s note.
fn back_in(
    browsers: &ChatBrowsers,
    program: &str,
    note: Option<&Note>,
    back: &[u32],
) -> TakenBack {
    browsers.taken_back(program, note, &|chat| back.contains(&chat), false, 3000)
}

#[test]
fn a_chats_browser_is_noted_in_its_host_as_it_starts_with_its_chat_and_name() {
    let (browsers, hosted) = noted();

    allowed_and_acting(&browsers, &hosted, 3);

    assert_eq!(
        note_of(&hosted, "program-1"),
        Note {
            chat: Some(3),
            name: Some("b1".into()),
            ..Note::default()
        }
    );
}

#[test]
fn pinning_a_browser_and_its_chat_closing_are_noted() {
    let (browsers, hosted) = noted();
    let driver: Arc<dyn Driver> = hosted.clone();
    allowed_and_acting(&browsers, &hosted, 3);

    browsers.pin(3, "program-1", true).unwrap();
    assert!(note_of(&hosted, "program-1").pinned);

    browsers.forget(Some(&driver), 3, "Fix the login");
    let note = note_of(&hosted, "program-1");
    assert!(note.pinned);
    assert_eq!(note.was.as_deref(), Some("Fix the login"));
}

#[test]
fn a_browser_noted_before_a_crash_goes_back_to_its_chat_under_the_name_it_knew() {
    let (first, hosted) = noted();
    allowed_and_acting(&first, &hosted, 3);
    let note = note_of(&hosted, "program-1");

    // The window crashes; the one that comes back has chat 3 open again.
    let again = ChatBrowsers::default();
    let back = back_in(&again, "program-1", Some(&note), &[3]);

    assert_eq!(back, TakenBack::WithItsChat);
    assert_eq!(again.listed(), [listed(3, "b1", "program-1")]);
    // Its chat's next call acts on it, with nothing asked again.
    let acts_before = hosted.acts.lock().unwrap().len();
    let (answer, raised) = answered(&again, &hosted, &ask(3, click()));
    assert!(!raised, "nothing is asked again");
    match answer {
        Answer::Browsed { browser, .. } => assert_eq!(browser, "b1"),
        other => panic!("not acted: {other:?}"),
    }
    let acts = hosted.acts.lock().unwrap();
    assert_eq!(acts.len(), acts_before + 1);
    assert_eq!(acts.last().unwrap().0, "program-1");
    drop(acts);
    // And a new one is named after it.
    match answered(&again, &hosted, &ask(3, Browsing::New)).0 {
        Answer::Browsed { browser, .. } => assert_eq!(browser, "b2"),
        other => panic!("not started: {other:?}"),
    }
}

#[test]
fn a_pinned_browser_whose_chat_did_not_come_back_is_kept_as_the_persons() {
    let (first, hosted) = noted();
    allowed_and_acting(&first, &hosted, 3);
    first.pin(3, "program-1", true).unwrap();
    let note = note_of(&hosted, "program-1");

    let again = ChatBrowsers::default();
    let back = back_in(&again, "program-1", Some(&note), &[]);

    assert_eq!(back, TakenBack::Kept);
    assert!(again.is_kept("program-1"));
    let kept = again.listed();
    assert_eq!(kept.len(), 1);
    assert!(kept[0].pinned);
    assert_eq!(kept[0].was.as_deref(), Some("chat 3"));
}

#[test]
fn a_browser_whose_chat_did_not_come_back_or_with_no_note_is_taken_back_by_nothing() {
    let (first, hosted) = noted();
    allowed_and_acting(&first, &hosted, 3);
    let note = note_of(&hosted, "program-1");
    let again = ChatBrowsers::default();

    assert_eq!(
        back_in(&again, "program-1", Some(&note), &[4]),
        TakenBack::NotTaken
    );
    assert_eq!(back_in(&again, "program-2", None, &[3]), TakenBack::NotTaken);
    assert!(again.listed().is_empty());
}

#[test]
fn the_persons_own_browser_is_taken_back_for_a_view_tab_once() {
    let again = ChatBrowsers::default();

    let back = back_in(&again, "program-9", Some(&Note::theirs()), &[3]);

    assert_eq!(back, TakenBack::Theirs);
    assert_eq!(again.take_theirs_back(), ["program-9"]);
    assert!(again.take_theirs_back().is_empty(), "each once");
    assert!(again.listed().is_empty(), "no chat's row");
}

#[test]
fn a_browser_taken_back_keeps_its_logins_so_a_hand_over_still_checks_each() {
    let (first, hosted) = noted();
    signed_in_and_kept(&first, &hosted);
    let note = note_of(&hosted, "program-1");
    assert_eq!(note.logins.len(), 1, "{note:?}");

    let again = ChatBrowsers::default();
    assert_eq!(
        back_in(&again, "program-1", Some(&note), &[]),
        TakenBack::Kept
    );
    allowed_and_acting(&again, &hosted, 4);
    let refused = hand_checked(&again, &hosted, "program-1", 4, &|_| {
        Err("that chat may not log in with shop-login now".into())
    })
    .unwrap_err();

    assert!(refused.contains("signed in with shop-login"), "{refused}");
    assert!(again.is_kept("program-1"), "it stays with no chat");
}

#[test]
fn a_browser_the_person_took_over_is_taken_back_marked_as_theirs_to_drive() {
    let (first, hosted) = noted();
    allowed_and_acting(&first, &hosted, 3);
    first.driven("program-1", true, 2000);
    let note = note_of(&hosted, "program-1");
    assert!(note.typed);

    let again = ChatBrowsers::default();
    again.taken_back("program-1", Some(&note), &|chat| chat == 3, true, 3000);

    let listed = again.listed();
    assert_eq!(listed[0].driver, Some(BrowserDriver::Person));
    assert!(again.records_of("program-1").1, "its take-over is recorded");
}

#[test]
fn a_note_reads_back_as_it_was_kept_and_one_that_does_not_read_is_none() {
    let note = Note {
        chat: Some(3),
        name: Some("b2".into()),
        open_web: true,
        pinned: true,
        ..Note::default()
    };
    let kept = serde_json::to_value(&note).unwrap();

    assert_eq!(Note::read(Some(&kept)), Some(note));
    assert_eq!(Note::read(Some(&serde_json::json!({"chat": "three"}))), None);
    assert_eq!(Note::read(None), None);
}

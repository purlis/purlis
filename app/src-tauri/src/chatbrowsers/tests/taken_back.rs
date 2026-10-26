// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A chat's browsers survive a crash of the window (#2025): each is noted in its session host as
//! what the window knew of it changes, and a window that comes back takes it back by that note.
//! The first window is one `ChatBrowsers`, the one that comes back another, as after a crash.

use super::*;
use crate::chatbrowsers::{Back, ChatBrowser, Left, Note, Owner, TakenBack};

const CHAT_3: &str = "01JA0CHT300000000000000000";
const CHAT_4: &str = "01JA0CHT400000000000000000";

/// The browsers of a window and its note-keeping host, chats 3 and 4 known by their identities.
fn noted() -> (ChatBrowsers, Arc<Hosted>) {
    let hosted = Arc::new(Hosted::default());
    let driver: Arc<dyn Driver> = hosted.clone();
    let browsers = ChatBrowsers::default().noting(Some(driver), Path::new("/p"));
    browsers.identify(3, Some(CHAT_3.into()));
    browsers.identify(4, Some(CHAT_4.into()));
    (browsers, hosted)
}

/// The note the host keeps of `program` once the window sent what changed.
fn note_of(browsers: &ChatBrowsers, hosted: &Hosted, program: &str) -> Note {
    browsers.renote();
    hosted
        .notes
        .lock()
        .unwrap()
        .get(program)
        .cloned()
        .unwrap_or_else(|| panic!("no note of {program}"))
}

/// The chats open in a window that comes back, by number and identity; whether each may log in
/// as it asks; and each chat's persona as it is now.
struct Again<'a> {
    open: &'a [(u32, &'a str)],
    may_log_in: bool,
    as_persona: AsPersona,
}

impl Again<'_> {
    fn chats(open: &[(u32, &'static str)]) -> Again<'static> {
        Again {
            open: Box::leak(open.to_vec().into_boxed_slice()),
            may_log_in: true,
            as_persona: AsPersona::default(),
        }
    }
}

/// What a window that comes back with `again` does with `program`, its note `note`, the host
/// saying whether it logged in and whether the person drives it.
fn back_in(
    browsers: &ChatBrowsers,
    hosted: &Arc<Hosted>,
    program: &str,
    note: Option<&Note>,
    again: &Again<'_>,
    logged_in: bool,
    person_drives: bool,
) -> TakenBack {
    let driver: Arc<dyn Driver> = hosted.clone();
    let open = |session: u32| {
        again
            .open
            .iter()
            .find(|(one, _)| *one == session)
            .map(|(_, id)| Some((*id).to_owned()))
    };
    let as_persona = |_| again.as_persona.clone();
    let may = |_, _: &HeldLogin| {
        if again.may_log_in {
            Ok(())
        } else {
            Err("that chat may not log in with shop-login now".to_owned())
        }
    };
    browsers.taken_back(
        Some(&driver),
        &Left {
            program,
            note,
            logged_in,
            person_drives,
        },
        &Back {
            open: &open,
            as_persona: &as_persona,
            may_log_in: &may,
        },
        3000,
    )
}

fn chat_owner(note: &Note) -> (u32, Option<String>, String) {
    match &note.owner {
        Owner::Chat { chat, id, name, .. } => (*chat, id.clone(), name.clone()),
        other => panic!("not a chat's: {other:?}"),
    }
}

#[test]
fn a_chats_browser_is_noted_with_its_chat_its_identity_and_its_name() {
    let (browsers, hosted) = noted();

    allowed_and_acting(&browsers, &hosted, 3);

    let note = note_of(&browsers, &hosted, "program-1");
    assert_eq!(chat_owner(&note), (3, Some(CHAT_3.into()), "b1".into()));
    assert!(note.logins.is_empty() && !note.taken_over && !note.viewed);
}

#[test]
fn a_note_is_sent_only_when_it_changed() {
    let (browsers, hosted) = noted();
    allowed_and_acting(&browsers, &hosted, 3);
    browsers.renote();
    let sent = *hosted.notes_sent.lock().unwrap();

    browsers.renote();
    assert_eq!(*hosted.notes_sent.lock().unwrap(), sent, "nothing changed");

    browsers.pin(3, "program-1", true).unwrap();
    browsers.renote();
    assert_eq!(*hosted.notes_sent.lock().unwrap(), sent + 1);
}

#[test]
fn pinning_a_browser_and_its_chat_closing_are_noted() {
    let (browsers, hosted) = noted();
    let driver: Arc<dyn Driver> = hosted.clone();
    allowed_and_acting(&browsers, &hosted, 3);

    browsers.pin(3, "program-1", true).unwrap();
    assert!(matches!(
        note_of(&browsers, &hosted, "program-1").owner,
        Owner::Chat { pinned: true, .. }
    ));

    browsers.forget(Some(&driver), 3, "Fix the login");
    assert_eq!(
        note_of(&browsers, &hosted, "program-1").owner,
        Owner::Kept {
            chat: 3,
            name: "b1".into(),
            was: "Fix the login".into()
        }
    );
}

#[test]
fn a_browser_being_ended_is_noted_so_at_once() {
    let (browsers, hosted) = noted();
    let driver: Arc<dyn Driver> = hosted.clone();
    allowed_and_acting(&browsers, &hosted, 3);
    browsers.renote();
    *hosted.closes_fail.lock().unwrap() = 1;
    *hosted.stops_fail.lock().unwrap() = 1;

    browsers.narrowed(Some(&driver), 3);

    // Before any later renote: the window may go at any moment from here.
    assert_eq!(
        hosted.notes.lock().unwrap().get("program-1").unwrap().owner,
        Owner::Ending
    );
}

#[test]
fn a_browser_noted_before_a_crash_goes_back_to_its_chat_under_the_name_it_knew() {
    let (first, hosted) = noted();
    allowed_and_acting(&first, &hosted, 3);
    let note = note_of(&first, &hosted, "program-1");

    // The window crashes; the one that comes back has chat 3 open again.
    let again = ChatBrowsers::default();
    let back = back_in(
        &again,
        &hosted,
        "program-1",
        Some(&note),
        &Again::chats(&[(3, CHAT_3)]),
        false,
        false,
    );

    assert_eq!(back, TakenBack::WithItsChat);
    assert_eq!(again.listed(), [listed(3, "b1", "program-1")]);
    // Its chat's next call acts on it, with nothing asked again.
    let (answer, raised) = answered(&again, &hosted, &ask(3, click()));
    assert!(!raised, "nothing is asked again");
    match answer {
        Answer::Browsed { browser, .. } => assert_eq!(browser, "b1"),
        other => panic!("not acted: {other:?}"),
    }
    assert_eq!(hosted.acts.lock().unwrap().last().unwrap().0, "program-1");
    // And a new one is named after it.
    match answered(&again, &hosted, &ask(3, Browsing::New)).0 {
        Answer::Browsed { browser, .. } => assert_eq!(browser, "b2"),
        other => panic!("not started: {other:?}"),
    }
}

#[test]
fn a_browser_goes_to_no_other_chat_that_has_its_chats_number_now() {
    let (first, hosted) = noted();
    allowed_and_acting(&first, &hosted, 3);
    let note = note_of(&first, &hosted, "program-1");

    let again = ChatBrowsers::default();
    let back = back_in(
        &again,
        &hosted,
        "program-1",
        Some(&note),
        &Again::chats(&[(3, "01JA0NTHRCHT00000000000000")]),
        false,
        false,
    );

    assert_eq!(back, TakenBack::NotTaken);
    assert!(again.listed().is_empty());
}

#[test]
fn a_pinned_browser_whose_chat_did_not_come_back_is_kept_as_the_persons() {
    let (first, hosted) = noted();
    allowed_and_acting(&first, &hosted, 3);
    first.pin(3, "program-1", true).unwrap();
    let note = note_of(&first, &hosted, "program-1");

    let again = ChatBrowsers::default();
    let back = back_in(
        &again,
        &hosted,
        "program-1",
        Some(&note),
        &Again::chats(&[]),
        false,
        false,
    );

    assert_eq!(back, TakenBack::Kept);
    let kept = again.listed();
    assert_eq!(kept.len(), 1);
    assert!(kept[0].pinned);
    assert_eq!(kept[0].was.as_deref(), Some("chat 3"));
}

#[test]
fn a_browser_with_no_note_being_ended_or_whose_chat_did_not_come_back_is_taken_back_by_nothing() {
    let (first, hosted) = noted();
    allowed_and_acting(&first, &hosted, 3);
    let note = note_of(&first, &hosted, "program-1");
    let again = ChatBrowsers::default();
    let open = Again::chats(&[(3, CHAT_3)]);

    let not_back = Again::chats(&[(4, CHAT_4)]);
    assert_eq!(
        back_in(
            &again,
            &hosted,
            "program-1",
            Some(&note),
            &not_back,
            false,
            false
        ),
        TakenBack::NotTaken
    );
    assert_eq!(
        back_in(&again, &hosted, "program-2", None, &open, false, false),
        TakenBack::NotTaken
    );
    let ending = Note::of(Owner::Ending);
    assert_eq!(
        back_in(
            &again,
            &hosted,
            "program-3",
            Some(&ending),
            &open,
            false,
            false
        ),
        TakenBack::NotTaken
    );
    assert!(again.listed().is_empty());
}

#[test]
fn a_browser_the_host_logged_in_whose_note_names_no_login_is_taken_back_by_nothing() {
    let (first, hosted) = noted();
    allowed_and_acting(&first, &hosted, 3);
    // A login the window had not noted when it went.
    let note = note_of(&first, &hosted, "program-1");

    let again = ChatBrowsers::default();
    let back = back_in(
        &again,
        &hosted,
        "program-1",
        Some(&note),
        &Again::chats(&[(3, CHAT_3)]),
        true,
        false,
    );

    assert_eq!(back, TakenBack::NotTaken);
}

#[test]
fn the_persons_own_browser_is_taken_back_for_a_view_tab_once() {
    let hosted = Arc::new(Hosted::default());
    let again = ChatBrowsers::default();

    let back = back_in(
        &again,
        &hosted,
        "program-9",
        Some(&Note::of(Owner::Theirs)),
        &Again::chats(&[]),
        false,
        false,
    );

    assert_eq!(back, TakenBack::Theirs);
    assert_eq!(again.take_theirs_back(), ["program-9"]);
    assert!(again.take_theirs_back().is_empty(), "each once");
    assert!(again.listed().is_empty(), "no chat's row");
}

#[test]
fn a_chats_browser_a_view_tab_of_its_own_showed_comes_back_in_one_too() {
    let (first, hosted) = noted();
    allowed_and_acting(&first, &hosted, 3);
    first.viewed("program-1", true);
    let note = note_of(&first, &hosted, "program-1");
    assert!(note.viewed);

    let again = ChatBrowsers::default();
    back_in(
        &again,
        &hosted,
        "program-1",
        Some(&note),
        &Again::chats(&[(3, CHAT_3)]),
        false,
        false,
    );

    assert_eq!(again.listed(), [listed(3, "b1", "program-1")]);
    assert_eq!(again.take_theirs_back(), ["program-1"]);
}

#[test]
fn a_browser_holding_a_login_goes_back_to_its_chat_only_where_it_may_log_in_with_it_now() {
    let (first, hosted) = noted();
    let qa = as_persona("qa", None, None);
    answered_as(&first, &hosted, &ask(3, click()), &qa);
    first.allow(3, false);
    answered_as(&first, &hosted, &ask(3, log_in("shop-login")), &qa);
    let note = note_of(&first, &hosted, "program-1");
    assert_eq!(note.logins.len(), 1, "{note:?}");

    let may_not = Again {
        may_log_in: false,
        ..Again::chats(&[(3, CHAT_3)])
    };
    let again = ChatBrowsers::default();
    let back = back_in(
        &again,
        &hosted,
        "program-1",
        Some(&note),
        &may_not,
        true,
        false,
    );

    assert_eq!(back, TakenBack::Kept, "the person's, not the chat's");
    assert!(again.is_kept("program-1"));
    // And still holding its login: a hand-over checks it.
    allowed_and_acting(&again, &hosted, 4);
    let refused = hand_checked(&again, &hosted, "program-1", 4, &|_| {
        Err("that chat may not log in with shop-login now".into())
    })
    .unwrap_err();
    assert!(refused.contains("signed in with shop-login"), "{refused}");

    let may = Again::chats(&[(3, CHAT_3)]);
    let other = ChatBrowsers::default();
    assert_eq!(
        back_in(&other, &hosted, "program-1", Some(&note), &may, true, false),
        TakenBack::WithItsChat
    );
}

#[test]
fn an_answer_resting_on_always_follows_it_as_narrowed_while_no_window_was_there() {
    let (first, hosted) = noted();
    let always_open = as_persona("qa", None, Some(true));
    answered_as(&first, &hosted, &ask(3, click()), &always_open);
    let note = note_of(&first, &hosted, "program-1");
    assert!(matches!(
        &note.owner,
        Owner::Chat { open_web: true, rests_on: Some(on), .. } if on == "qa"
    ));

    // While the window was gone, the person narrowed Always for qa to the project's sites.
    let narrowed = Again {
        as_persona: as_persona("qa", None, Some(false)),
        ..Again::chats(&[(3, CHAT_3)])
    };
    let again = ChatBrowsers::default();
    back_in(
        &again,
        &hosted,
        "program-1",
        Some(&note),
        &narrowed,
        false,
        false,
    );

    assert_eq!(again.posture(3), Posture::Allowed { open_web: false });
    assert!(
        hosted
            .closed
            .lock()
            .unwrap()
            .contains(&"program-1".to_owned()),
        "its browser, which reached the open web, is ended"
    );
    assert!(again.listed().is_empty());
}

#[test]
fn a_browser_this_window_holds_already_is_left_as_it_is_with_its_logins_merged() {
    let (browsers, hosted) = noted();
    allowed_and_acting(&browsers, &hosted, 3);
    let mut note = note_of(&browsers, &hosted, "program-1");
    note.logins.push(HeldLogin {
        persona: Some("qa".into()),
        login: "shop-login".into(),
        identity: "0123456789abcdef0123456789abcdef".into(),
    });

    let back = back_in(
        &browsers,
        &hosted,
        "program-1",
        Some(&note),
        &Again::chats(&[(3, CHAT_3)]),
        true,
        false,
    );

    assert_eq!(back, TakenBack::HeldHere);
    assert_eq!(
        browsers.listed(),
        [ChatBrowser {
            login: Some("shop-login".into()),
            ..listed(3, "b1", "program-1")
        }]
    );
}

#[test]
fn a_browser_the_person_took_over_is_taken_back_marked_as_theirs_to_drive() {
    let (first, hosted) = noted();
    allowed_and_acting(&first, &hosted, 3);
    first.driven("program-1", true, 2000);
    let note = note_of(&first, &hosted, "program-1");
    assert!(note.taken_over);

    let again = ChatBrowsers::default();
    back_in(
        &again,
        &hosted,
        "program-1",
        Some(&note),
        &Again::chats(&[(3, CHAT_3)]),
        false,
        true,
    );

    assert_eq!(again.listed()[0].driver, Some(BrowserDriver::Person));
    assert!(again.records_of("program-1").1, "its take-over is recorded");
}

#[test]
fn a_note_reads_back_as_it_was_kept_and_one_that_does_not_read_is_none() {
    let note = Note {
        logins: Vec::new(),
        taken_over: true,
        viewed: false,
        owner: Owner::Chat {
            chat: 3,
            id: Some(CHAT_3.into()),
            name: "b2".into(),
            open_web: true,
            rests_on: None,
            pinned: true,
        },
    };
    let kept = note.kept().unwrap();

    assert_eq!(Note::read(Some(&kept)), Some(note));
    let unreadable = purlis_session_protocol::session::ProgramNote(
        serde_json::json!({"owner": "chat", "chat": "three"}),
    );
    assert_eq!(Note::read(Some(&unreadable)), None);
    assert_eq!(Note::read(None), None);
}

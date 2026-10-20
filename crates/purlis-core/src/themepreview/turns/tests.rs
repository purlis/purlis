// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use super::*;

/// Long enough that a test never waits it out by accident.
const LONG: Duration = Duration::from_secs(30);

/// Waits until `held` says the painter is `who`'s, or fails.
fn until_held(turns: &Turns, who: Option<Who>) {
    let started = Instant::now();
    while turns.held() != who {
        assert!(started.elapsed() < LONG, "the painter never became {who:?}");
        thread::yield_now();
    }
}

#[test]
fn a_free_painter_is_the_first_askers_and_one_at_a_time() {
    let turns = Turns::new();
    let gallery = turns.gallery().expect("free");
    assert_eq!(gallery.who(), Who::Gallery);
    assert!(
        turns.gallery().is_none(),
        "a second picture is answered busy"
    );
    drop(gallery);
    let chat = turns.chat(LONG).expect("free again");
    assert_eq!(chat.who(), Who::Chat);
    assert!(
        turns.gallery().is_none(),
        "a picture is answered busy while a chat draws"
    );
    assert!(
        turns.chat(Duration::ZERO).is_none(),
        "a second chat is answered busy, as before"
    );
    drop(chat);
    assert_eq!(turns.held(), None);
}

#[test]
fn a_chat_asked_while_a_gallery_picture_draws_is_served_first() {
    let turns = Turns::new();
    let (page, drawn) = mpsc::channel::<()>();
    thread::scope(|scope| {
        let gallery = turns.gallery().expect("free");
        // The gallery's page draws for as long as it likes: only the chat's ask ends its wait.
        let painting = scope.spawn(move || {
            let waited = gallery.wait(&drawn, Instant::now() + LONG);
            drop(gallery);
            waited
        });
        let chat = scope.spawn(|| {
            let started = Instant::now();
            let turn = turns.chat(LONG).map(|turn| turn.who());
            (turn, started.elapsed())
        });
        let (turn, waited) = chat.join().unwrap();
        assert_eq!(turn, Some(Who::Chat));
        assert!(
            waited < Duration::from_secs(2),
            "the chat waited {waited:?} for the gallery's picture"
        );
        assert_eq!(painting.join().unwrap(), Err(Waited::GaveWay));
        drop(page);
    });
}

#[test]
fn no_gallery_picture_starts_while_a_chat_waits() {
    let turns = Turns::new();
    thread::scope(|scope| {
        let held = turns.gallery().expect("free");
        let chat = scope.spawn(|| turns.chat(LONG).map(|turn| turn.who()));
        // The chat is waiting: the gallery's next picture is answered busy.
        let started = Instant::now();
        while !held.gives_way() {
            assert!(started.elapsed() < LONG, "the chat never waited");
            thread::yield_now();
        }
        assert!(turns.gallery().is_none());
        drop(held);
        assert_eq!(chat.join().unwrap(), Some(Who::Chat));
    });
    until_held(&turns, None);
    assert!(
        turns.gallery().is_some(),
        "free again once the chat is done"
    );
}

#[test]
fn a_chat_is_answered_busy_when_the_picture_does_not_give_way_in_time() {
    let turns = Turns::new();
    let held = turns.gallery().expect("free");
    assert!(turns.chat(Duration::from_millis(20)).is_none());
    // Nothing is left waiting: the gallery's picture no longer gives way.
    assert!(!held.gives_way());
}

#[test]
fn a_chats_turn_never_gives_way() {
    let turns = Turns::new();
    let chat = turns.chat(LONG).expect("free");
    assert!(!chat.gives_way());
    let (sent, answer) = mpsc::channel();
    sent.send(7).unwrap();
    assert_eq!(chat.wait(&answer, Instant::now() + LONG), Ok(7));
}

#[test]
fn a_wait_ends_at_its_time_or_when_nothing_can_answer() {
    let turns = Turns::new();
    let turn = turns.gallery().expect("free");
    let (sent, answer) = mpsc::channel::<()>();
    assert_eq!(
        turn.wait(&answer, Instant::now() + Duration::from_millis(5)),
        Err(Waited::Over)
    );
    drop(sent);
    assert_eq!(turn.wait(&answer, Instant::now() + LONG), Err(Waited::Over));
}

#[test]
fn the_painter_is_free_again_however_a_turn_ends() {
    let turns = Turns::new();
    let ended = thread::scope(|scope| {
        scope
            .spawn(|| {
                let _turn = turns.gallery().expect("free");
                panic!("the page failed");
            })
            .join()
    });
    assert!(ended.is_err());
    assert_eq!(turns.held(), None);
    assert!(turns.chat(Duration::ZERO).is_some());
}

#[test]
fn after_a_chat_s_turn_the_gallery_asked_during_it_is_owed_one_picture_drawn_whole() {
    let turns = Turns::new();
    let chat = turns.chat(Duration::ZERO).expect("free");
    assert!(turns.gallery().is_none(), "busy while the chat draws");
    drop(chat);
    // Owed: every chat is answered busy until the gallery has drawn its picture.
    assert!(turns.chat(Duration::ZERO).is_none(), "the gallery is owed");
    assert!(turns.chat(Duration::ZERO).is_none(), "any chat, again");
    let owed = turns.gallery().expect("the gallery's turn");
    assert!(owed.owed());
    thread::scope(|scope| {
        let waiting = scope.spawn(|| turns.chat(LONG).map(|turn| turn.who()));
        // A chat waits for the owed picture, which does not give way to it.
        let started = Instant::now();
        while turns.state().chats_waiting == 0 {
            assert!(started.elapsed() < LONG, "the chat never waited");
            thread::yield_now();
        }
        assert!(!owed.gives_way());
        drop(owed);
        assert_eq!(waiting.join().unwrap(), Some(Who::Chat));
    });
    until_held(&turns, None);
    // The gallery did not ask during that chat's turn: nothing is owed after it.
    assert!(turns.chat(Duration::ZERO).is_some());
}

#[test]
fn a_chat_s_turn_the_gallery_did_not_ask_during_owes_it_nothing() {
    let turns = Turns::new();
    drop(turns.chat(Duration::ZERO).expect("free"));
    assert!(turns.chat(Duration::ZERO).is_some(), "nothing is owed");
    let gallery = turns.gallery().expect("free");
    assert!(!gallery.owed(), "a picture nobody owed gives way as before");
}

#[test]
fn a_picture_that_gave_way_to_a_chat_is_owed_after_that_chat_s_turn() {
    let turns = Turns::new();
    thread::scope(|scope| {
        let gallery = turns.gallery().expect("free");
        let painting = scope.spawn(move || {
            let (_page, drawn) = mpsc::channel::<()>();
            let waited = gallery.wait(&drawn, Instant::now() + LONG);
            drop(gallery);
            waited
        });
        let chat = turns.chat(LONG).expect("drawn once the picture gives way");
        assert_eq!(painting.join().unwrap(), Err(Waited::GaveWay));
        drop(chat);
    });
    assert!(turns.chat(Duration::ZERO).is_none(), "the gallery is owed");
    assert!(turns.gallery().expect("its turn").owed());
}

#[test]
fn what_the_gallery_is_owed_lapses_when_it_does_not_come_for_it() {
    let turns = Turns::owing_for(Duration::from_millis(20));
    let chat = turns.chat(Duration::ZERO).expect("free");
    assert!(turns.gallery().is_none());
    drop(chat);
    assert!(turns.chat(Duration::ZERO).is_none(), "owed");
    thread::sleep(Duration::from_millis(40));
    let chat = turns
        .chat(Duration::ZERO)
        .expect("lapsed: the chat is drawn");
    drop(chat);
    assert!(
        !turns.gallery().expect("free").owed(),
        "a lapsed debt is not paid later"
    );
}

#[test]
fn a_chat_whose_wait_timed_out_leaves_no_debt_for_a_later_chat_s_turn() {
    let turns = Turns::new();
    let gallery = turns.gallery().expect("free");
    // The picture does not give way in time: the chat is answered busy, and the picture drawn.
    assert!(turns.chat(Duration::from_millis(10)).is_none());
    drop(gallery);
    // A later chat's turn, the gallery asking nothing during it, owes the gallery nothing.
    drop(turns.chat(Duration::ZERO).expect("free"));
    assert!(
        turns.chat(Duration::ZERO).is_some(),
        "other chats are not held back"
    );
}

#[test]
fn a_gallery_ask_answered_busy_by_its_own_picture_leaves_no_debt_once_it_draws() {
    let turns = Turns::new();
    let gallery = turns.gallery().expect("free");
    assert!(turns.gallery().is_none(), "a second window's ask");
    drop(gallery);
    drop(turns.gallery().expect("drawn"));
    drop(turns.chat(Duration::ZERO).expect("free"));
    assert!(turns.chat(Duration::ZERO).is_some(), "nothing is owed");
}

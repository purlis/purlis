// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A launch's chats are registered at once and started behind the window (`restoring.rs`):
//! what [`Chats::begin_put_back`] and [`Chats::finish_put_back`] promise, on a host that runs
//! nothing and can be made slow.
//!
//! The first test is the measurement the change was made against: how long the window waits
//! for a record of 1, 10 and 40 chats, where each start takes [`A_START`].

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use purlis_core::engine::Size;
use purlis_core::reopen::{Chat, Record};

use super::Chats;
use crate::host::SessionHost;
use crate::host::pretend::Pretend;
use crate::restoring::Gate;

const SIZE: Size = Size {
    columns: 80,
    rows: 24,
};

/// What one start costs on the pretend host: far less than a harness, its sandbox and its
/// proxy coming up, and enough to be told from nothing.
const A_START: Duration = Duration::from_millis(40);

/// The most registering a record may take, however many chats it holds: a tenth of what
/// forty starts in turn take, and hundreds of times what registering them does.
const STARTS_NOTHING_WITHIN: Duration = Duration::from_millis(160);

/// How long a test waits for what a thread behind it is doing, before it calls it stuck.
const SOON: Duration = Duration::from_secs(20);

/// Chats on `host`, in a project of their own, starting at most `most` at once.
fn chats_on(host: &Pretend, most: usize) -> (Chats, tempfile::TempDir) {
    let project = tempfile::tempdir().expect("a project");
    let chats = Chats::on_host(
        Box::new(|_| {}),
        Box::new(host.clone()),
        project.path().to_path_buf(),
    );
    let chats = Chats {
        // A gate of its own: a start one test hangs holds no other test's place.
        gate: Arc::new(Gate::letting(most, Duration::from_secs(600))),
        ..chats
    };
    (chats, project)
}

/// A record of `count` shells, numbered from 1 in order, the one numbered `front` in front.
fn a_record_of(count: u32, front: u32) -> Record {
    Record {
        chats: (1..=count)
            .map(|number| Chat {
                program: "/bin/sh".to_owned(),
                name: format!("chat {number}"),
                number: Some(number),
                active: number == front,
                ..Default::default()
            })
            .collect(),
        dealt: count,
        ..Record::default()
    }
}

fn names(chats: &[Chat]) -> Vec<String> {
    chats.iter().map(|chat| chat.name.clone()).collect()
}

fn until(what: &str, done: impl Fn() -> bool) {
    let began = Instant::now();
    while !done() {
        assert!(began.elapsed() < SOON, "still waiting: {what}");
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// **The measurement.** The window can be used once a record is registered, and how long
/// that takes does not grow with the chats the record holds: forty chats whose starts take
/// forty milliseconds each used to be 1.6 s of the window waiting, and are none of it now.
#[test]
fn the_window_waits_no_longer_for_forty_chats_than_for_one() {
    for count in [1, 10, 40] {
        let host = Pretend::default();
        host.takes(A_START);
        let (chats, _project) = chats_on(&host, 4);

        let began = Instant::now();
        chats.begin_put_back(&a_record_of(count, 1), &|_| None);
        let usable = began.elapsed();

        assert!(
            host.asked().is_empty(),
            "registering a record started a chat"
        );
        assert_eq!(chats.drawn_now().len(), count as usize, "a tab each");
        // Forty starts in turn are 1.6 s. Registering them is well under a millisecond; the
        // line is far from both, so a busy machine does not move the answer.
        assert!(
            usable < STARTS_NOTHING_WITHIN,
            "registering {count} chats took {usable:?}: the window waited on them"
        );

        let opened = chats.finish_put_back(SIZE);
        let all = began.elapsed();
        assert_eq!(opened.len(), count as usize);
        println!("restore N={count}: usable after {usable:?}, every chat started after {all:?}");
    }
}

#[test]
fn every_chat_has_its_tab_in_its_place_before_any_has_started() {
    let host = Pretend::default();
    let (chats, _project) = chats_on(&host, 2);

    chats.begin_put_back(&a_record_of(5, 3), &|_| None);

    let drawn = chats.drawn_now();
    assert_eq!(
        drawn.iter().map(|(one, _)| one.session).collect::<Vec<_>>(),
        [1, 2, 3, 4, 5],
        "each under the number it will start as, in the record's order"
    );
    assert!(drawn.iter().all(|(_, restoring)| *restoring));
    assert_eq!(
        drawn
            .iter()
            .filter(|(one, _)| one.in_front)
            .map(|(one, _)| one.session)
            .collect::<Vec<_>>(),
        [3],
        "the chat that was in front is the one in front"
    );
    assert!(chats.open_now().is_empty(), "none is open yet");

    chats.finish_put_back(SIZE);

    let drawn = chats.drawn_now();
    assert_eq!(
        drawn.iter().map(|(one, _)| one.session).collect::<Vec<_>>(),
        [1, 2, 3, 4, 5],
        "and each is the chat, in the same place"
    );
    assert!(drawn.iter().all(|(_, restoring)| !*restoring));
}

#[test]
fn the_chat_in_front_starts_first_and_the_rest_in_the_records_order() {
    let host = Pretend::default();
    let (chats, _project) = chats_on(&host, 1);

    chats.begin_put_back(&a_record_of(5, 4), &|_| None);
    chats.finish_put_back(SIZE);

    let started: Vec<u32> = host.asked().into_iter().map(|(number, _)| number).collect();
    assert_eq!(started, [4, 1, 2, 3, 5]);
}

#[test]
fn no_more_chats_start_at_once_than_the_gate_lets() {
    let host = Pretend::default();
    host.takes(Duration::from_millis(10));
    let (chats, _project) = chats_on(&host, 2);

    chats.begin_put_back(&a_record_of(12, 1), &|_| None);
    let opened = chats.finish_put_back(SIZE);

    assert_eq!(opened.len(), 12);
    assert_eq!(host.most_at_once(), 2);
}

#[test]
fn a_start_that_hangs_holds_up_no_other_chat() {
    let host = Pretend::default();
    host.hangs_number(2);
    let (chats, _project) = chats_on(&host, 2);
    chats.begin_put_back(&a_record_of(8, 2), &|_| None);

    std::thread::scope(|scope| {
        let starting = scope.spawn(|| chats.finish_put_back(SIZE));

        until("every chat but the one that hangs", || {
            chats.open_now().len() == 7
        });
        let restoring: Vec<u32> = chats
            .drawn_now()
            .into_iter()
            .filter(|(_, restoring)| *restoring)
            .map(|(one, _)| one.session)
            .collect();
        assert_eq!(
            restoring,
            [2],
            "only the chat that hangs is still restoring"
        );

        host.lets_go(2);
        assert_eq!(starting.join().expect("it finishes").len(), 8);
    });
}

#[test]
fn a_chat_that_does_not_start_is_listed_so_in_its_place_and_the_rest_come_back() {
    let host = Pretend::default();
    host.takes(Duration::from_millis(3));
    // The later one is found out first: the earlier one's start is slower to be refused.
    host.refuses_number(2, "its program is gone");
    host.refuses_number(5, "its folder moved");
    let (chats, _project) = chats_on(&host, 3);

    chats.begin_put_back(&a_record_of(6, 5), &|_| None);
    let opened = chats.finish_put_back(SIZE);

    assert_eq!(
        opened.iter().map(|one| one.session).collect::<Vec<_>>(),
        [1, 3, 4, 6]
    );
    let would_not = chats.would_not_start();
    assert_eq!(
        would_not
            .iter()
            .map(|one| (one.name.as_str(), one.why.as_str()))
            .collect::<Vec<_>>(),
        [
            ("chat 2", "its program is gone"),
            ("chat 5", "its folder moved")
        ],
        "each with its reason, in the record's order"
    );
    // Still recorded, first, as every chat that did not start is.
    assert_eq!(
        names(&chats.record().chats),
        ["chat 2", "chat 5", "chat 1", "chat 3", "chat 4", "chat 6"]
    );
    assert!(chats.drawn_now().iter().all(|(_, restoring)| !*restoring));
}

#[test]
fn a_record_written_while_chats_come_back_holds_every_one_once_in_its_place() {
    let host = Pretend::default();
    host.hangs_number(1);
    host.hangs_number(4);
    let written: Arc<Mutex<Vec<Record>>> = Arc::default();
    let (chats, _project) = chats_on(&host, 4);
    let chats = Chats {
        record_it: {
            let written = Arc::clone(&written);
            Box::new(move |record| written.lock().unwrap().push(record.clone()))
        },
        ..chats
    };
    let all = ["chat 1", "chat 2", "chat 3", "chat 4", "chat 5"];
    chats.begin_put_back(&a_record_of(5, 4), &|_| None);
    assert_eq!(names(&chats.record().chats), all, "before any has started");

    std::thread::scope(|scope| {
        let starting = scope.spawn(|| chats.finish_put_back(SIZE));
        until("the three that do not hang", || chats.open_now().len() == 3);
        assert!(
            written.lock().unwrap().is_empty(),
            "a start of the launch wrote the record: it is written once, at the end"
        );

        // Anything else that changes meanwhile is written, with the two still to start.
        chats.bring_to_front(Some(3));
        let record = written.lock().unwrap().last().cloned().expect("it wrote");
        assert_eq!(names(&record.chats), all);
        let front: Vec<&str> = record
            .chats
            .iter()
            .filter(|chat| chat.active)
            .map(|chat| chat.name.as_str())
            .collect();
        assert_eq!(front, ["chat 3"]);
        // The ones that started have a run of this launch; the two that have not are as
        // they were recorded, with no program.
        assert!(record.chats[1].identity.run.is_some());
        assert!(record.chats[0].identity.run.is_none() && record.chats[0].pid.is_none());

        host.lets_go(1);
        host.lets_go(4);
        starting.join().expect("it finishes");
    });

    let last = written.lock().unwrap().last().cloned().expect("it wrote");
    assert_eq!(names(&last.chats), all);
    assert!(last.chats.iter().all(|chat| chat.identity.run.is_some()));
}

/// A chat found not to start is listed so before it leaves the ones still to start, so a
/// record built between the two reads it in both. It is written once: twice, the next launch
/// would take the second for another chat, mint it an id and start both.
#[test]
fn a_record_built_as_a_chat_is_found_not_to_start_holds_it_once() {
    let host = Pretend::default();
    let (chats, _project) = chats_on(&host, 2);
    chats.begin_put_back(&a_record_of(3, 1), &|_| None);
    // As `start_what_is_coming` leaves it between its two steps: listed as one that did not
    // start, and still among the ones to start.
    let second = super::lock(&chats.coming)[1].clone();
    chats.did_not_come(super::Waiting {
        approval: None,
        chat: second.chat,
        why: "its program is gone".to_owned(),
        told: None,
    });

    let record = chats.record();

    assert_eq!(names(&record.chats), ["chat 2", "chat 1", "chat 3"]);
    let mut ids: Vec<_> = record
        .chats
        .iter()
        .filter_map(|chat| chat.identity.id.clone())
        .collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), 3, "each chat under an id of its own");
}

#[test]
fn a_quit_while_chats_come_back_keeps_the_ones_not_started_and_starts_no_more() {
    let host = Pretend::default();
    host.hangs_number(1);
    let (chats, _project) = chats_on(&host, 1);
    chats.begin_put_back(&a_record_of(4, 1), &|_| None);

    std::thread::scope(|scope| {
        let starting = scope.spawn(|| chats.finish_put_back(SIZE));
        until("the first start is under way", || host.most_at_once() == 1);

        let mut last = None;
        chats.write_last(|record| last = Some(record.clone()));
        assert_eq!(
            names(&last.expect("the quit wrote").chats),
            ["chat 1", "chat 2", "chat 3", "chat 4"],
            "every chat is kept for the next launch"
        );
        chats.end_all();

        host.lets_go(1);
        starting.join().expect("it finishes");
    });

    assert_eq!(
        host.asked()
            .into_iter()
            .map(|(number, _)| number)
            .collect::<Vec<_>>(),
        [1],
        "no chat started after the quit began"
    );
    assert!(host.running().is_empty(), "and the one under way was ended");
}

/// #3114: a start that returns once the quit has begun is ended, and its chat is still one to
/// start in any record written after, in front as it was. Taken out of the ones still to start
/// and then ended, it was in no list a record reads: a quit's record built a moment later lost it
/// for good (`planes::restoring_tests::a_quit_while_chats_come_back_leaves_every_chat_in_the_record`
/// failed so once on CI, without chat 5).
#[test]
fn a_start_that_returns_into_a_quit_keeps_its_chat_in_every_record_after() {
    let host = Pretend::default();
    host.hangs_number(1);
    let (chats, _project) = chats_on(&host, 1);
    chats.begin_put_back(&a_record_of(2, 1), &|_| None);

    std::thread::scope(|scope| {
        let starting = scope.spawn(|| chats.finish_put_back(SIZE));
        until("the first start is under way", || host.most_at_once() == 1);
        // The quit begins: it writes its record, and nothing is written after it.
        chats.write_last(|_| {});
        host.lets_go(1);
        starting.join().expect("it finishes");
    });

    assert!(
        host.running().is_empty(),
        "the start that returned into the quit was ended"
    );
    let record = chats.record();
    assert_eq!(
        names(&record.chats),
        ["chat 1", "chat 2"],
        "a chat whose start returned into the quit is still kept for the next launch"
    );
    assert!(record.chats[0].active, "and it is still the one in front");
}

/// The one test that listens for what the window is told, so no other hears another's.
#[test]
fn the_window_is_told_as_each_chat_starts_or_does_not() {
    let host = Pretend::default();
    host.refuses_number(2, "its program is gone");
    let (chats, project) = chats_on(&host, 1);
    let told: Arc<Mutex<Vec<crate::restoring::Moved>>> = Arc::default();
    crate::restoring::telling({
        let (told, root) = (Arc::clone(&told), project.path().to_path_buf());
        Arc::new(move |moved| {
            if moved.root == root {
                told.lock().unwrap().push(moved);
            }
        })
    });

    chats.begin_put_back(&a_record_of(3, 3), &|_| None);
    chats.finish_put_back(SIZE);

    let told: Vec<(u32, Option<u32>, usize)> = told
        .lock()
        .unwrap()
        .iter()
        .map(|moved| (moved.number, moved.session, moved.left))
        .collect();
    assert_eq!(told, [(3, Some(3), 2), (1, Some(1), 1), (2, None, 0)]);
}

#[test]
fn a_chat_recorded_with_no_number_or_anothers_is_drawn_under_one_of_its_own() {
    let host = Pretend::default();
    let (chats, _project) = chats_on(&host, 2);
    let mut record = a_record_of(3, 1);
    record.chats[1].number = None;
    record.chats[2].number = Some(1);

    chats.begin_put_back(&record, &|_| None);
    let drawn: Vec<u32> = chats
        .drawn_now()
        .into_iter()
        .map(|(one, _)| one.session)
        .collect();
    assert_eq!(drawn, [1, 4, 5], "past every number the record holds");

    let opened = chats.finish_put_back(SIZE);
    assert_eq!(
        opened.iter().map(|one| one.session).collect::<Vec<_>>(),
        drawn,
        "and each starts as the number its tab was drawn under"
    );
}

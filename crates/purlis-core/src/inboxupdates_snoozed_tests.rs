// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Snooze all (#5647, ADR 0119): the record of what each chat held when it was snoozed, kept per
//! project beside the updates, and the rule that brings a chat back. Driven through [`Store`]
//! alone, on a data home of the test's own.

use super::*;

fn project() -> (tempfile::TempDir, std::path::PathBuf, Store) {
    let dir = tempfile::tempdir().expect("a directory");
    let root = dir.path().join("project");
    std::fs::create_dir_all(&root).expect("a project");
    let store = Store::in_data(&dir.path().join("data"));
    (dir, root, store)
}

/// Chat `session` holding the asks `asks`, by key.
fn chat(session: u32, asks: &[&str]) -> (u32, Vec<String>) {
    (session, asks.iter().map(|ask| (*ask).to_owned()).collect())
}

fn held(chats: &[(u32, Vec<String>)]) -> Held {
    chats.iter().cloned().collect()
}

fn sessions(snoozed: &Snoozed) -> Vec<u32> {
    snoozed.keys().copied().collect()
}

#[test]
fn snoozed_chats_are_kept_with_the_asks_they_held() {
    let (_dir, root, store) = project();
    assert!(
        store.snoozed(&root).is_empty(),
        "nothing is snoozed at first"
    );

    let answered = store
        .snooze(
            &root,
            held(&[chat(3, &["a1", "a2"]), chat(7, &["b1"])]),
            None,
        )
        .expect("snoozed");

    assert_eq!(answered, store.snoozed(&root));
    assert_eq!(
        answered.get(&3).map(|one| &one.asks),
        Some(&vec!["a1".to_owned(), "a2".to_owned()])
    );
    assert_eq!(
        answered.get(&7).map(|one| &one.asks),
        Some(&vec!["b1".to_owned()])
    );
}

#[test]
fn it_survives_a_new_store_on_the_same_data_home_as_a_relaunch_does() {
    let (dir, root, store) = project();
    store
        .snooze(&root, held(&[chat(3, &["a1"])]), None)
        .expect("snoozed");

    let again = Store::in_data(&dir.path().join("data"));
    assert_eq!(sessions(&again.snoozed(&root)), [3]);
}

#[test]
fn a_chat_returns_once_it_holds_an_ask_it_did_not_have_when_snoozed() {
    let (_dir, root, store) = project();
    store
        .snooze(
            &root,
            held(&[chat(3, &["a1", "a2"]), chat(7, &["b1"]), chat(9, &["c1"])]),
            None,
        )
        .expect("snoozed");

    // Chat 3 asks something new; chat 7 had one of its asks answered in its pane; chat 9 holds
    // what it held.
    let away = store
        .release(
            &root,
            &held(&[chat(3, &["a1", "a2", "a3"]), chat(7, &[]), chat(9, &["c1"])]),
            NOW,
        )
        .expect("released");

    assert_eq!(
        sessions(&away),
        [7, 9],
        "only the chat with a new ask came back"
    );
    assert_eq!(sessions(&store.snoozed(&root)), [7, 9], "and it stays back");
    // The ask it was snoozed with is no longer enough to put it away again.
    let away = store
        .release(&root, &held(&[chat(3, &["a1"])]), NOW)
        .expect("released");
    assert_eq!(sessions(&away), [7, 9]);
}

#[test]
fn a_snoozed_chat_that_holds_nothing_new_stays_away_however_long_it_waits() {
    let (_dir, root, store) = project();
    store
        .snooze(&root, held(&[chat(3, &["a1"])]), None)
        .expect("snoozed");

    // No clock: read again and again, the record is what it was.
    for _ in 0..3 {
        let away = store
            .release(&root, &held(&[chat(3, &["a1"])]), NOW)
            .expect("released");
        assert_eq!(sessions(&away), [3]);
    }
}

#[test]
fn waking_brings_back_the_chats_named_or_every_chat() {
    let (_dir, root, store) = project();
    store
        .snooze(
            &root,
            held(&[chat(3, &["a1"]), chat(7, &["b1"]), chat(9, &["c1"])]),
            None,
        )
        .expect("snoozed");

    let away = store.wake(&root, Some(&[3, 9])).expect("woken");
    assert_eq!(sessions(&away), [7]);

    let away = store.wake(&root, None).expect("woken");
    assert!(away.is_empty());
    assert!(store.snoozed(&root).is_empty());
}

#[test]
fn nothing_is_written_into_the_project_and_the_updates_are_left_as_they_were() {
    let (_dir, root, store) = project();
    store
        .note(
            &root,
            vec![Update {
                key: "done:4".to_owned(),
                kind: Kind::TaskDone,
                at: 1_800_000_000,
                session: Some(4),
                chain: Vec::new(),
                says: "done".to_owned(),
                read: false,
                dismissed: false,
                baselined: false,
            }],
            1_800_000_000,
        )
        .expect("noted");

    store
        .snooze(&root, held(&[chat(3, &["a1"])]), None)
        .expect("snoozed");
    store
        .release(&root, &held(&[chat(3, &["a1", "a2"])]), NOW)
        .expect("released");

    let in_project: Vec<_> = std::fs::read_dir(&root)
        .expect("the project")
        .flatten()
        .collect();
    assert!(in_project.is_empty(), "{in_project:?}");
    assert_eq!(store.read(&root, 1_800_000_000).len(), 1);
    assert_ne!(store.snoozed_file(&root), store.file(&root));
}

#[test]
fn at_most_so_many_chats_are_kept_the_lowest_numbers_going_first() {
    let (_dir, root, store) = project();
    let many: Vec<_> = (1..=(AT_MOST_SNOOZED as u32 + 5))
        .map(|session| chat(session, &["one"]))
        .collect();

    let away = store.snooze(&root, held(&many), None).expect("snoozed");

    assert_eq!(away.len(), AT_MOST_SNOOZED);
    assert_eq!(away.keys().next(), Some(&6));
}

#[test]
fn a_record_this_build_cannot_read_reads_as_nothing_snoozed_and_is_written_over() {
    let (_dir, root, store) = project();
    std::fs::create_dir_all(store.snoozed_file(&root).parent().expect("a folder")).expect("dir");
    std::fs::write(store.snoozed_file(&root), "not json").expect("written");

    assert!(store.snoozed(&root).is_empty());
    let away = store
        .snooze(&root, held(&[chat(3, &["a1"])]), None)
        .expect("snoozed");
    assert_eq!(sessions(&away), [3]);
}

// Snooze for a while (#6018, ADR 0121 decision 14): "for 1 hour" and "until tomorrow" beside
// "until it asks something new", whichever comes first.

const NOW: u64 = 1_800_000_000;
const HOUR: u64 = 3_600;

#[test]
fn a_chat_snoozed_for_a_while_stays_away_until_its_time_and_returns_at_it() {
    let (_dir, root, store) = project();
    store
        .snooze(&root, held(&[chat(3, &["a1"])]), Some(NOW + HOUR))
        .expect("snoozed");

    let away = store
        .release(&root, &held(&[chat(3, &["a1"])]), NOW + HOUR - 1)
        .expect("released");
    assert_eq!(
        sessions(&away),
        [3],
        "a second before its time it is still away"
    );

    let away = store
        .release(&root, &held(&[chat(3, &["a1"])]), NOW + HOUR)
        .expect("released");
    assert!(
        away.is_empty(),
        "at its time it is back, holding only what it held"
    );
    assert!(store.snoozed(&root).is_empty(), "and it stays back");
}

#[test]
fn a_chat_snoozed_for_a_while_still_returns_first_when_it_asks_something_new() {
    let (_dir, root, store) = project();
    store
        .snooze(&root, held(&[chat(3, &["a1"])]), Some(NOW + HOUR))
        .expect("snoozed");

    let away = store
        .release(&root, &held(&[chat(3, &["a1", "a2"])]), NOW)
        .expect("released");
    assert!(away.is_empty());
}

#[test]
fn a_snooze_whose_time_passed_while_purlis_was_closed_is_over_at_the_first_read() {
    let (dir, root, store) = project();
    store
        .snooze(
            &root,
            held(&[chat(3, &["a1"]), chat(7, &["b1"])]),
            Some(NOW + HOUR),
        )
        .expect("snoozed");
    store
        .snooze(&root, held(&[chat(9, &["c1"])]), None)
        .expect("snoozed");

    // A relaunch the next day: a new store on the same data home, and a chat that holds
    // nothing now is let go of too, its time being up.
    let again = Store::in_data(&dir.path().join("data"));
    let away = again
        .release(&root, &held(&[chat(3, &["a1"])]), NOW + 24 * HOUR)
        .expect("released");
    assert_eq!(
        sessions(&away),
        [9],
        "only the chat with no time stays away"
    );
}

#[test]
fn the_first_time_a_snoozed_chat_comes_back_by_itself_is_said() {
    let (_dir, root, store) = project();
    assert_eq!(first_back(&store.snoozed(&root)), None);
    store
        .snooze(&root, held(&[chat(9, &["c1"])]), None)
        .expect("snoozed");
    assert_eq!(
        first_back(&store.snoozed(&root)),
        None,
        "no time, no time said"
    );

    store
        .snooze(&root, held(&[chat(3, &["a1"])]), Some(NOW + 2 * HOUR))
        .expect("snoozed");
    let kept = store
        .snooze(&root, held(&[chat(7, &["b1"])]), Some(NOW + HOUR))
        .expect("snoozed");
    assert_eq!(first_back(&kept), Some(NOW + HOUR));
}

#[test]
fn a_record_written_before_snoozes_had_a_time_reads_as_snoozed_until_a_new_ask() {
    let (_dir, root, store) = project();
    std::fs::create_dir_all(store.snoozed_file(&root).parent().expect("a folder")).expect("dir");
    std::fs::write(store.snoozed_file(&root), r#"{"3":["a1","a2"]}"#).expect("written");

    let away = store
        .release(&root, &held(&[chat(3, &["a1"])]), NOW + 365 * 24 * HOUR)
        .expect("released");
    assert_eq!(sessions(&away), [3]);
    assert_eq!(
        away.get(&3).map(|one| one.asks.clone()),
        Some(vec!["a1".to_owned(), "a2".to_owned()])
    );
    assert_eq!(away.get(&3).and_then(|one| one.until), None);
}

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

fn held(chats: &[(u32, Vec<String>)]) -> Snoozed {
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
        .snooze(&root, held(&[chat(3, &["a1", "a2"]), chat(7, &["b1"])]))
        .expect("snoozed");

    assert_eq!(answered, store.snoozed(&root));
    assert_eq!(
        answered.get(&3),
        Some(&vec!["a1".to_owned(), "a2".to_owned()])
    );
    assert_eq!(answered.get(&7), Some(&vec!["b1".to_owned()]));
}

#[test]
fn it_survives_a_new_store_on_the_same_data_home_as_a_relaunch_does() {
    let (dir, root, store) = project();
    store
        .snooze(&root, held(&[chat(3, &["a1"])]))
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
        )
        .expect("snoozed");

    // Chat 3 asks something new; chat 7 had one of its asks answered in its pane; chat 9 holds
    // what it held.
    let away = store
        .release(
            &root,
            &held(&[chat(3, &["a1", "a2", "a3"]), chat(7, &[]), chat(9, &["c1"])]),
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
        .release(&root, &held(&[chat(3, &["a1"])]))
        .expect("released");
    assert_eq!(sessions(&away), [7, 9]);
}

#[test]
fn a_snoozed_chat_that_holds_nothing_new_stays_away_however_long_it_waits() {
    let (_dir, root, store) = project();
    store
        .snooze(&root, held(&[chat(3, &["a1"])]))
        .expect("snoozed");

    // No clock: read again and again, the record is what it was.
    for _ in 0..3 {
        let away = store
            .release(&root, &held(&[chat(3, &["a1"])]))
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
        .snooze(&root, held(&[chat(3, &["a1"])]))
        .expect("snoozed");
    store
        .release(&root, &held(&[chat(3, &["a1", "a2"])]))
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

    let away = store.snooze(&root, held(&many)).expect("snoozed");

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
        .snooze(&root, held(&[chat(3, &["a1"])]))
        .expect("snoozed");
    assert_eq!(sessions(&away), [3]);
}

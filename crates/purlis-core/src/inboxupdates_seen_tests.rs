// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **New** (#6002, ADR 0121 decisions 10 and 11): the record of which asks, Notices and updates
//! the person has seen, kept per project beside the updates. Driven through [`Store`] alone, on a
//! data home of the test's own.

use super::*;

fn project() -> (tempfile::TempDir, std::path::PathBuf, Store) {
    let dir = tempfile::tempdir().expect("a directory");
    let root = dir.path().join("project");
    std::fs::create_dir_all(&root).expect("a project");
    let store = Store::in_data(&dir.path().join("data"));
    (dir, root, store)
}

fn keys(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

fn names(seen: &Seen) -> Vec<&str> {
    seen.keys().map(String::as_str).collect()
}

#[test]
fn nothing_is_seen_until_the_window_says_it_was() {
    let (_dir, root, store) = project();
    assert!(store.seen(&root).is_empty(), "nothing is seen at first");

    let answered = store
        .see(&root, &keys(&["ask:a1", "notice:pin-gone:ide"]), 100)
        .expect("seen");

    assert_eq!(answered, store.seen(&root));
    assert_eq!(names(&answered), ["ask:a1", "notice:pin-gone:ide"]);
    assert_eq!(answered.get("ask:a1"), Some(&100));
}

#[test]
fn it_survives_a_new_store_on_the_same_data_home_as_a_relaunch_does() {
    let (dir, root, store) = project();
    store.see(&root, &keys(&["ask:a1"]), 100).expect("seen");

    let again = Store::in_data(&dir.path().join("data"));
    assert_eq!(names(&again.seen(&root)), ["ask:a1"]);
}

#[test]
fn seeing_again_keeps_when_it_was_first_seen_and_writes_nothing() {
    let (_dir, root, store) = project();
    store.see(&root, &keys(&["ask:a1"]), 100).expect("seen");
    let written = std::fs::metadata(store.seen_file(&root))
        .and_then(|meta| meta.modified())
        .expect("its file");

    let answered = store.see(&root, &keys(&["ask:a1"]), 200).expect("seen");

    assert_eq!(answered.get("ask:a1"), Some(&100));
    let after = std::fs::metadata(store.seen_file(&root))
        .and_then(|meta| meta.modified())
        .expect("its file");
    assert_eq!(
        written, after,
        "nothing new was seen, so nothing is written"
    );
}

#[test]
fn each_project_keeps_its_own() {
    let (dir, root, store) = project();
    let other = dir.path().join("other");
    std::fs::create_dir_all(&other).expect("another project");
    store.see(&root, &keys(&["ask:a1"]), 100).expect("seen");

    assert!(store.seen(&other).is_empty());
}

#[test]
fn it_keeps_at_most_its_bound_letting_go_of_what_was_seen_longest_ago() {
    let (_dir, root, store) = project();
    let first: Vec<String> = (0..AT_MOST_SEEN).map(|n| format!("ask:{n}")).collect();
    store.see(&root, &first, 100).expect("seen");

    let answered = store.see(&root, &keys(&["ask:last"]), 200).expect("seen");

    assert_eq!(answered.len(), AT_MOST_SEEN);
    assert!(answered.contains_key("ask:last"));
    assert_eq!(
        first
            .iter()
            .filter(|key| !answered.contains_key(*key))
            .count(),
        1,
        "one seen at the earliest time went"
    );
}

#[test]
fn a_key_is_held_to_its_bound() {
    let (_dir, root, store) = project();
    let long = "k".repeat(MOST_KEY * 2);

    let answered = store.see(&root, &[long], 100).expect("seen");

    assert_eq!(names(&answered)[0].chars().count(), MOST_KEY);
}

#[test]
fn a_file_this_build_cannot_read_reads_as_nothing_seen_and_is_written_over() {
    let (_dir, root, store) = project();
    std::fs::create_dir_all(store.seen_file(&root).parent().expect("its folder")).expect("folder");
    std::fs::write(store.seen_file(&root), b"not json").expect("written");

    assert!(store.seen(&root).is_empty());
    let answered = store.see(&root, &keys(&["ask:a1"]), 100).expect("seen");
    assert_eq!(names(&answered), ["ask:a1"]);
}

#[test]
fn an_ask_that_stops_standing_is_forgotten_so_the_same_ask_again_is_new() {
    let (_dir, root, store) = project();
    store
        .see(
            &root,
            &keys(&["ask:3:wait", "ask:4:p1", "notice:pin-gone:ide"]),
            100,
        )
        .expect("seen");

    let answered = store
        .forget(&root, "ask:", &keys(&["ask:4:p1"]))
        .expect("forgotten");

    assert_eq!(
        names(&answered),
        ["ask:4:p1", "notice:pin-gone:ide"],
        "only an ask no longer standing goes; what is not an ask stays"
    );
    assert_eq!(answered, store.seen(&root));
}

#[test]
fn forgetting_what_still_stands_writes_nothing() {
    let (_dir, root, store) = project();
    store.see(&root, &keys(&["ask:4:p1"]), 100).expect("seen");
    let written = std::fs::metadata(store.seen_file(&root))
        .and_then(|meta| meta.modified())
        .expect("its file");

    store
        .forget(&root, "ask:", &keys(&["ask:4:p1", "ask:5:p2"]))
        .expect("forgotten");

    let after = std::fs::metadata(store.seen_file(&root))
        .and_then(|meta| meta.modified())
        .expect("its file");
    assert_eq!(written, after);
}

/// Q258: **a chat keeps one look**. A new look at a chat's blocks (`blocks:<chat>:<seq>`)
/// replaces its earlier one in the same write, so however many looks are taken, the record
/// never fills with them and evicts the other New markers.
#[test]
fn many_looks_at_a_chat_s_blocks_keep_one_key_and_leave_the_other_new_markers() {
    let (_dir, root, store) = project();
    let others: Vec<String> = (0..AT_MOST_SEEN - 2)
        .map(|n| format!("update:u{n:04}"))
        .collect();
    store.see(&root, &others, 100).expect("seen");
    store
        .see(&root, &keys(&["blocks:01K6CHAT0000000000000000005:3"]), 101)
        .expect("seen");

    for seq in 0..(AT_MOST_SEEN as u64 * 2) {
        let look = format!("{BLOCKS}01K6CHAT0000000000000000004:{seq}");
        store.see(&root, &[look], 200 + seq).expect("seen");
    }

    let seen = store.seen(&root);
    let looks: Vec<&str> = names(&seen)
        .into_iter()
        .filter(|key| key.starts_with("blocks:01K6CHAT0000000000000000004:"))
        .collect();
    let last = AT_MOST_SEEN * 2 - 1;
    assert_eq!(
        looks,
        [format!("blocks:01K6CHAT0000000000000000004:{last}")]
    );
    assert!(
        others.iter().all(|key| seen.contains_key(key)),
        "every other New marker is still seen"
    );
    assert!(
        seen.contains_key("blocks:01K6CHAT0000000000000000005:3"),
        "another chat's look is its own"
    );
}

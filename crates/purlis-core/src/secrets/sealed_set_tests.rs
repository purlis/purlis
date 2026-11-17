// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

const STORE: &str = "test-answers";

fn fresh() -> Set {
    super::super::seal::forget_kept_for_this_test();
    super::super::seal::set_for_this_test(Some(vec![7; 32]));
    Set::default()
}

fn with(set: &mut Set, clone: &str, add: &[(&str, &str)]) {
    set.rewrite(STORE, clone, |_, _| true, add).expect("sealed");
}

#[test]
fn an_entry_the_app_sealed_counts_and_a_fresh_set_holds_nothing() {
    let mut set = fresh();
    assert_eq!(set.verify(STORE, "plane-a").standing, Standing::Fresh);
    with(&mut set, "plane-a", &[("mine", "steward -> devops")]);
    let verified = set.verify(STORE, "plane-a");
    assert_eq!(verified.standing, Standing::Holds);
    assert!(verified.holds("mine", "steward -> devops"));
    assert!(verified.dropped.is_empty());
}

#[test]
fn an_edited_entry_is_dropped_and_the_others_still_count() {
    let mut set = fresh();
    with(
        &mut set,
        "plane-a",
        &[("mine", "steward -> devops"), ("mine", "steward -> qa")],
    );
    set.entries[0].text = "steward -> root".to_owned();
    let verified = set.verify(STORE, "plane-a");
    assert!(!verified.holds("mine", "steward -> root"));
    assert!(verified.holds("mine", "steward -> qa"));
    assert_eq!(
        verified.dropped,
        vec![("mine".to_owned(), "steward -> root".to_owned())]
    );
    assert_eq!(verified.missing, 1);
}

#[test]
fn an_entry_with_a_kind_changed_is_dropped() {
    let mut set = fresh();
    with(&mut set, "plane-a", &[("seen", "steward -> devops")]);
    set.entries[0].kind = "mine".to_owned();
    assert!(!set.verify(STORE, "plane-a").holds("mine", "steward -> devops"));
}

#[test]
fn an_entry_copied_from_another_clone_is_dropped() {
    let mut theirs = fresh();
    with(&mut theirs, "plane-b", &[("mine", "steward -> devops")]);
    let mut ours = Set::default();
    with(&mut ours, "plane-a", &[("mine", "steward -> qa")]);
    ours.entries.push(theirs.entries[0].clone());
    ours.live.push(theirs.entries[0].mac.clone());
    let verified = ours.verify(STORE, "plane-a");
    // The live list was changed, so its anchor does not hold: nothing counts.
    assert_eq!(verified.standing, Standing::Broken);
    assert!(!verified.holds("mine", "steward -> devops"));
    // And put in without the live list, the entry is in no live list.
    ours.live.pop();
    let verified = ours.verify(STORE, "plane-a");
    assert!(!verified.holds("mine", "steward -> devops"));
    assert!(verified.holds("mine", "steward -> qa"));
}

#[test]
fn a_revoked_entry_put_back_from_a_copy_counts_as_nothing() {
    let mut set = fresh();
    with(
        &mut set,
        "plane-a",
        &[("mine", "steward -> devops"), ("mine", "steward -> qa")],
    );
    let before = set.clone();
    set.rewrite(STORE, "plane-a", |_, text| text != "steward -> devops", &[])
        .expect("revoked");
    assert!(!set.verify(STORE, "plane-a").holds("mine", "steward -> devops"));
    // The entry alone, put back: its seal holds, and it is in no live list.
    let mut entry_back = set.clone();
    entry_back.entries.push(before.entries[0].clone());
    let verified = entry_back.verify(STORE, "plane-a");
    assert!(!verified.holds("mine", "steward -> devops"));
    assert!(verified.holds("mine", "steward -> qa"));
    // The whole set, put back from before: its counter is behind the anchor.
    let verified = before.verify(STORE, "plane-a");
    assert_eq!(verified.standing, Standing::Broken);
    assert!(verified.holding.is_empty());
}

#[test]
fn a_rewrite_never_seals_what_it_did_not_verify() {
    let mut set = fresh();
    with(&mut set, "plane-a", &[("mine", "steward -> qa")]);
    // An entry written by something other than the app, with a seal that does not hold.
    set.entries.push(Entry {
        kind: "mine".to_owned(),
        text: "steward -> devops".to_owned(),
        generation: set.generation,
        mac: "00".repeat(32),
    });
    set.live.push("00".repeat(32));
    // A later write of the app's, for another answer.
    let mut after = set.clone();
    after
        .rewrite(STORE, "plane-a", |_, _| true, &[("mine", "steward -> ops")])
        .expect("sealed");
    let verified = after.verify(STORE, "plane-a");
    assert!(!verified.holds("mine", "steward -> devops"));
    assert!(verified.holds("mine", "steward -> ops"));
    assert!(after.entries.iter().all(|e| e.text != "steward -> devops"));
}

#[test]
fn a_set_whose_anchor_is_gone_counts_nothing() {
    let mut set = fresh();
    with(&mut set, "plane-a", &[("mine", "steward -> devops")]);
    super::super::seal::forget_kept_for_this_test();
    assert_eq!(set.verify(STORE, "plane-a").standing, Standing::Broken);
}

#[test]
fn a_process_that_cannot_seal_counts_nothing_and_changes_nothing() {
    let mut set = fresh();
    with(&mut set, "plane-a", &[("mine", "steward -> devops")]);
    super::super::seal::set_for_this_test(None);
    let verified = set.verify(STORE, "plane-a");
    assert_eq!(verified.standing, Standing::Unverifiable);
    assert!(verified.holding.is_empty());
    let before = set.clone();
    assert!(
        set.rewrite(STORE, "plane-a", |_, _| true, &[("mine", "a -> b")])
            .is_err()
    );
    assert_eq!(set, before);
    // The anchor is still there, for whoever asks without the key.
    assert!(Set::anchored(STORE, "plane-a"));
    super::super::seal::set_for_this_test(Some(vec![7; 32]));
}

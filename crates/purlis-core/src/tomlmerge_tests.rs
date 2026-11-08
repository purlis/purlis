// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Lists that are sets, written and merged (#5824 F5).

use super::*;

fn set_of(text: &str) -> String {
    let mut doc: toml_edit::DocumentMut = text.parse().unwrap();
    as_set(doc["sandbox"]["hosts"].as_array_mut().unwrap());
    doc.to_string()
}

#[test]
fn a_set_is_written_one_per_line_sorted_and_once_each() {
    assert_eq!(
        set_of("[sandbox]\nhosts = [\"b.example\", \"a.example\", \"b.example\"] # mine\n"),
        "[sandbox]\nhosts = [\n    \"a.example\",\n    \"b.example\",\n] # mine\n"
    );
    assert_eq!(set_of("[sandbox]\nhosts = []\n"), "[sandbox]\nhosts = []\n");
}

#[test]
fn a_comment_above_an_item_goes_with_it() {
    assert_eq!(
        set_of(
            "[sandbox]\nhosts = [\n  # the build cache\n  \"z.example\",\n  \"a.example\",\n]\n"
        ),
        "[sandbox]\nhosts = [\n    \"a.example\",\n    # the build cache\n    \"z.example\",\n]\n"
    );
}

#[test]
fn the_keys_that_are_sets_are_named_in_one_list() {
    assert!(is_set(&["sandbox", "hosts"]));
    assert!(is_set(&["sandbox", "browser-sites"]));
    assert!(is_set(&["sandbox", "personas", "devops", "hosts"]));
    assert!(is_set(&["dispatch", "grants", "steward"]));
    assert!(!is_set(&["sandbox", "egress"]));
    assert!(!is_set(&["sandbox", "personas", "devops", "tools"]));
}

#[test]
fn a_set_both_sides_added_to_keeps_both_and_a_removal_holds() {
    let base = "[dispatch.grants]\nsteward = [\"devops\", \"qa\"]\n";
    let ours = "[dispatch.grants]\nsteward = [\"devops\"]\n";
    let theirs = "[dispatch.grants]\nsteward = [\"devops\", \"qa\", \"writer\"]\n";
    assert_eq!(
        merged(base, ours, theirs).unwrap(),
        Merged::Clean(
            "[dispatch.grants]\nsteward = [\n    \"devops\",\n    \"writer\",\n]\n".to_owned()
        )
    );
}

#[test]
fn a_set_emptied_by_the_merge_takes_its_key_with_it() {
    let base = "[sandbox]\nmode = \"on\"\nhosts = [\"a.example\", \"b.example\"]\n";
    let ours = "[sandbox]\nmode = \"on\"\nhosts = [\"b.example\"]\n";
    let theirs = "[sandbox]\nmode = \"on\"\nhosts = [\"a.example\"]\n";
    assert_eq!(
        merged(base, ours, theirs).unwrap(),
        Merged::Clean("[sandbox]\nmode = \"on\"\n".to_owned())
    );
}

#[test]
fn a_side_that_is_not_toml_is_refused() {
    assert!(merged("", "[sandbox\n", "").is_err());
}

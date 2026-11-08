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

#[test]
fn a_key_the_remote_removed_goes_and_one_this_machine_added_stays() {
    let base = "# ours\n[plane]\nname = \"p\"\nautosave = true\n";
    let ours = "# ours\n[plane]\nname = \"p\"\nautosave = true\nmode = \"pr\"\n";
    let theirs = "[plane]\nname = \"p\"\n";
    assert_eq!(
        merged(base, ours, theirs).unwrap(),
        Merged::Clean("# ours\n[plane]\nname = \"p\"\nmode = \"pr\"\n".to_owned())
    );
}

#[test]
fn a_table_the_remote_added_comes_after_this_machines() {
    let base = "[plane]\nname = \"p\"\n";
    let ours = "[plane]\nname = \"p\"\n\n[theme]\nname = \"dawn\"\n";
    let theirs = "[extensions.stats]\nenabled = true\n\n[plane]\nname = \"p\"\n";
    let Merged::Clean(text) = merged(base, ours, theirs).unwrap() else {
        panic!("a conflict")
    };
    assert_eq!(
        text.parse::<toml::Table>().unwrap(),
        "[plane]\nname = \"p\"\n[theme]\nname = \"dawn\"\n[extensions.stats]\nenabled = true\n"
            .parse::<toml::Table>()
            .unwrap()
    );
    assert!(
        text.starts_with("[plane]\nname = \"p\"\n\n[theme]\nname = \"dawn\"\n"),
        "{text}"
    );
}

#[test]
fn spacing_alone_is_no_change_and_each_conflicting_key_is_named() {
    let base = "[sandbox]\nmode = \"on\"\n[plane]\nmode = \"pr\"\n[\"odd key\"]\nx = 1\n";
    let ours = "[sandbox]\nmode   =   \"off\"\n[plane]\nmode = \"push\"\n[\"odd key\"]\nx = 2\n";
    let theirs = "[sandbox]\nmode = \"ask\"\n[plane]\nmode=\"push\"\n[\"odd key\"]\nx = 3\n";
    assert_eq!(
        merged(base, ours, theirs).unwrap(),
        Merged::Conflict(vec!["\"odd key\".x".to_owned(), "sandbox.mode".to_owned()])
    );
}

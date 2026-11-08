// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

const RUNNER: &str = "01JB0RUNNER0000000000000000";
const PROJECT: &str = "01J9PRJCT00000000000000000";
const OTHER: &str = "01J9PRJCT00000000000000OTH";

#[test]
fn a_runners_cursor_is_kept_per_project_and_its_lowest_is_where_a_link_reads_from() {
    let home = tempfile::tempdir().unwrap();
    assert_eq!(logged_up_to(home.path(), RUNNER, PROJECT), None);
    assert_eq!(lowest_for(home.path(), RUNNER), None);

    keep(home.path(), RUNNER, PROJECT, 41).unwrap();
    keep(home.path(), RUNNER, PROJECT, 42).unwrap();
    keep(home.path(), RUNNER, OTHER, 7).unwrap();

    assert_eq!(logged_up_to(home.path(), RUNNER, PROJECT), Some(42));
    assert_eq!(logged_up_to(home.path(), RUNNER, OTHER), Some(7));
    assert_eq!(lowest_for(home.path(), RUNNER), Some(7));
    assert_eq!(lowest_for(home.path(), "01JB0OTHER00000000000000000"), None);
}

#[test]
fn an_id_that_is_no_plain_one_is_not_kept_and_a_file_this_build_cannot_read_is_none() {
    let home = tempfile::tempdir().unwrap();
    assert!(keep(home.path(), "../escape", PROJECT, 1).is_err());
    assert!(keep(home.path(), RUNNER, "../escape", 1).is_err());
    assert!(keep(home.path(), &"x".repeat(MOST_ID + 1), PROJECT, 1).is_err());

    keep(home.path(), RUNNER, PROJECT, 3).unwrap();
    let file = crate::machine::dir(home.path()).join(FILE);
    std::fs::write(&file, r#"{"version":2,"cursors":{}}"#).unwrap();
    assert_eq!(logged_up_to(home.path(), RUNNER, PROJECT), None);
    std::fs::write(&file, "not json").unwrap();
    assert_eq!(logged_up_to(home.path(), RUNNER, PROJECT), None);
}

#[test]
fn past_the_most_cursors_the_lowest_go_first_never_the_one_kept_now() {
    let home = tempfile::tempdir().unwrap();
    for n in 0..MOST_CURSORS {
        keep(home.path(), RUNNER, &format!("P{n:04}"), 100 + n as u64).unwrap();
    }

    keep(home.path(), "NEWEST", PROJECT, 0).unwrap();

    let kept = read(home.path());
    assert_eq!(
        kept.values().map(BTreeMap::len).sum::<usize>(),
        MOST_CURSORS
    );
    assert_eq!(logged_up_to(home.path(), "NEWEST", PROJECT), Some(0));
    assert_eq!(
        logged_up_to(home.path(), RUNNER, "P0000"),
        None,
        "the lowest went"
    );
    assert_eq!(logged_up_to(home.path(), RUNNER, "P0001"), Some(101));
}

/// A runner made again mints a new device (`machine::mint_device_id`), whose log starts its `seq`
/// at the bottom, and nothing removes the cursors of the one it replaced. Its seqs say nothing
/// of a gone device's: what goes first is the device kept least lately (#5657).
#[test]
fn past_the_most_cursors_a_device_kept_least_lately_goes_before_a_live_runners_low_cursors() {
    let home = tempfile::tempdir().unwrap();
    let gone = "01JB0GONE000000000000000000";
    let made_again = "01JB1MADEAGAIN0000000000000";
    for n in 0..MOST_CURSORS - 1 {
        keep(home.path(), gone, &format!("P{n:04}"), 5_000 + n as u64).unwrap();
    }

    keep(home.path(), made_again, PROJECT, 3).unwrap();
    keep(home.path(), made_again, OTHER, 4).unwrap();

    assert_eq!(
        logged_up_to(home.path(), made_again, PROJECT),
        Some(3),
        "the live runner's cursor stays"
    );
    assert_eq!(logged_up_to(home.path(), made_again, OTHER), Some(4));
    assert_eq!(
        logged_up_to(home.path(), gone, "P0000"),
        None,
        "the gone runner's went"
    );
    assert_eq!(
        read(home.path()).values().map(BTreeMap::len).sum::<usize>(),
        MOST_CURSORS
    );
}

#[test]
fn a_file_an_earlier_build_wrote_reads_and_its_runners_go_before_one_kept_since() {
    let home = tempfile::tempdir().unwrap();
    keep(home.path(), RUNNER, PROJECT, 1).unwrap();
    let file = crate::machine::dir(home.path()).join(FILE);
    let earlier: BTreeMap<String, u64> = (0..MOST_CURSORS)
        .map(|n| (format!("P{n:04}"), 9_000 + n as u64))
        .collect();
    let earlier =
        serde_json::json!({ "version": 1, "cursors": { "01JB0EARLIER000000000000000": earlier } });
    std::fs::write(&file, earlier.to_string()).unwrap();
    assert_eq!(
        logged_up_to(home.path(), "01JB0EARLIER000000000000000", "P0000"),
        Some(9_000)
    );

    keep(home.path(), RUNNER, PROJECT, 2).unwrap();
    keep(home.path(), RUNNER, OTHER, 1).unwrap();

    assert_eq!(logged_up_to(home.path(), RUNNER, PROJECT), Some(2));
    assert_eq!(logged_up_to(home.path(), RUNNER, OTHER), Some(1));
    assert_eq!(
        logged_up_to(home.path(), "01JB0EARLIER000000000000000", "P0001"),
        None
    );
}

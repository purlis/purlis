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

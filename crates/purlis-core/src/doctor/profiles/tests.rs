// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::doctor::Status;

fn wiring(state: State, detail: &str, fix: &str) -> wiring::Wiring {
    wiring::Wiring {
        state,
        detail: detail.to_owned(),
        fix: fix.to_owned(),
    }
}

#[test]
fn a_probe_that_could_not_tell_says_its_fix_or_that_it_was_not_checked() {
    let row = wiring_row(
        "profile x",
        &wiring(State::Unknown, "no hook", "charter wire x"),
    );
    assert_eq!(row.status, Status::Warn);
    assert_eq!(row.detail, "no hook");
    assert_eq!(row.hint, "charter wire x");

    let row = wiring_row("profile x", &wiring(State::Unknown, "could not look", ""));
    assert_eq!(row.status, Status::Warn);
    assert_eq!(row.hint, NOT_CHECKED_HINT);

    let row = wiring_row("profile x", &wiring(State::Wired, "wired", ""));
    assert_eq!(row.status, Status::Ok);
    assert_eq!(row.detail, "wired");
}

#[test]
fn a_long_fix_is_counted_rather_than_drawn_whole() {
    let fix = "f".repeat(DISPLAY_LIMIT + 3);
    let row = wiring_row("profile x", &wiring(State::Unknown, "d", &fix));
    assert_eq!(
        row.hint,
        format!("{}\u{2026} +3 not shown", "f".repeat(DISPLAY_LIMIT))
    );
}

#[test]
fn a_profile_whose_icon_fell_back_is_listed_with_why() {
    let dir = tempfile::tempdir().unwrap();
    let row = icon_row(
        &[
            ("work", "missing.svg"),
            ("fine", "spark"),
            ("odd", "no-such-glyph"),
        ],
        Some(dir.path()),
    )
    .expect("a row");
    assert_eq!(row.name, "profile icons");
    assert_eq!(row.status, Status::Warn);
    assert!(
        row.detail.contains("work: the icon file is not there"),
        "{}",
        row.detail
    );
    assert!(row.detail.contains("odd: "), "{}", row.detail);
    assert!(!row.detail.contains("fine"), "{}", row.detail);
    assert!(!row.hint.is_empty());
}

#[test]
fn no_row_where_no_icon_fell_back() {
    assert!(icon_row(&[("fine", "spark")], None).is_none());
    assert!(icon_row(&[], None).is_none());
}

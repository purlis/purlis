// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Prose shown as prose** (#5213): a question a task asks is several lines of text, so its
//! line ends and tabs reach the window as themselves, and every other character a reader would
//! not see is still marked by purlis's one rule.

use purlis_core::shown;

#[test]
fn a_line_end_and_a_tab_stay_as_themselves() {
    purlis_core::unsteered!();
    assert_eq!(
        shown::prose("Which branch?\n\n- **main**\n\t- or a fork"),
        "Which branch?\n\n- **main**\n\t- or a fork"
    );
}

#[test]
fn a_windows_line_end_is_one_line_end_and_not_a_mark() {
    purlis_core::unsteered!();
    assert_eq!(shown::prose("one\r\ntwo"), "one\ntwo");
}

#[test]
fn every_other_hidden_character_is_still_marked() {
    purlis_core::unsteered!();
    assert_eq!(
        shown::prose("a\u{202E}b\rc\u{200B}"),
        "a⟦U+202E⟧b⟦U+000D⟧c⟦U+200B⟧"
    );
}

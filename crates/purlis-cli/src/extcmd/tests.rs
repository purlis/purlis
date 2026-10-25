// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::unknown_word_note;

#[test]
fn an_unknown_word_says_where_else_purlis_looked() {
    assert_eq!(
        unknown_word_note("stauts", true),
        "No extension on this machine is called 'stauts' either, installed or built into the \
         app."
    );
    let outside = unknown_word_note("stauts", false);
    assert!(
        outside.starts_with("No extension installed on this machine is called 'stauts'")
            && outside.contains("not inside the app's bundle"),
        "{outside}"
    );
}

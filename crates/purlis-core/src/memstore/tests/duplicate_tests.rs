// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

// Oracles from `charter.memstore.wordset` / `.body` and `charter.todos._same_text`.

#[test]
fn only_words_longer_than_three_characters_count() {
    assert_eq!(
        wordset("Review the rollout plan")
            .into_iter()
            .collect::<Vec<_>>(),
        vec!["plan", "review", "rollout"],
        "`the` is three characters and is dropped"
    );
    assert_eq!(
        wordset("a ab abc abcd abcde")
            .into_iter()
            .collect::<Vec<_>>(),
        vec!["abcd", "abcde"]
    );
}

#[test]
fn a_word_is_unicode_so_an_accented_one_is_not_split_apart() {
    assert_eq!(
        wordset("café déjà-vu naïve")
            .into_iter()
            .collect::<Vec<_>>(),
        vec!["café", "déjà", "naïve"]
    );
}

#[test]
fn the_comparable_body_drops_the_heading_and_the_stamp_and_collapses_space() {
    assert_eq!(
        comparable_body("# Title here\n\n_2026-03-02 09:14 · persistent_\n\nSome   body\ntext\n"),
        "some body text"
    );
}

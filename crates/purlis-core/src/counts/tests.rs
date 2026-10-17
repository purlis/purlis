// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::{counted, plural};

#[test]
fn one_takes_the_singular_and_every_other_count_the_plural() {
    assert_eq!(counted(1usize, "file", "files"), "1 file");
    assert_eq!(counted(2usize, "file", "files"), "2 files");
    assert_eq!(counted(0usize, "file", "files"), "0 files");
}

#[test]
fn an_irregular_plural_is_written_out() {
    assert_eq!(counted(1u32, "copy", "copies"), "1 copy");
    assert_eq!(counted(3u32, "copy", "copies"), "3 copies");
    assert_eq!(counted(2u64, "harness", "harnesses"), "2 harnesses");
}

#[test]
fn any_integer_count_is_taken() {
    assert_eq!(counted(1i64, "commit", "commits"), "1 commit");
    assert_eq!(counted(-1i64, "commit", "commits"), "-1 commits");
    assert_eq!(counted(1u8, "item", "items"), "1 item");
}

#[test]
fn plural_picks_the_word_alone_for_a_verb_or_a_pronoun() {
    assert_eq!(plural(1usize, "is", "are"), "is");
    assert_eq!(plural(4usize, "is", "are"), "are");
    assert_eq!(plural(0u32, "it", "them"), "them");
}

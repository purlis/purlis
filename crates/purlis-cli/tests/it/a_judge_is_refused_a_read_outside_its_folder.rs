// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! SK-55 (#3235): the real binary, asked as a harness asks, refuses a judge a read outside its
//! folder; a binary that answers anything else does not pass.

#[test]
fn the_binary_refuses_a_judge_a_read_outside_its_folder() {
    let folder = tempfile::tempdir().expect("a directory");
    assert!(purlis_core::toolhooks::judge_guard_answers(
        std::path::Path::new(crate::support::purlis().get_program()),
        folder.path(),
    ));
    assert!(!purlis_core::toolhooks::judge_guard_answers(
        std::path::Path::new("/usr/bin/true"),
        folder.path(),
    ));
}

#[test]
fn the_binary_answers_for_an_outcome_chats_write_in_its_piece() {
    let folder = tempfile::tempdir().expect("a directory");
    assert!(purlis_core::toolhooks::outcome_guard_answers(
        std::path::Path::new(crate::support::purlis().get_program()),
        folder.path(),
    ));
    assert!(!purlis_core::toolhooks::outcome_guard_answers(
        std::path::Path::new("/usr/bin/true"),
        folder.path(),
    ));
}

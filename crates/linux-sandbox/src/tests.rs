// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn a_long_profile_is_carried_in_pieces_and_read_back_whole() {
    let profile: String = (0..20_000)
        .map(|n| format!("(deny file-write* (literal \"/x/{n}\"))\n"))
        .collect();
    let words = line(&profile, "codex".to_owned(), vec!["--a".to_owned()]);
    assert_eq!(words[0], COMMAND);
    assert!(words.iter().all(|word| word.len() <= MOST_PIECE));
    assert!(words.iter().filter(|word| *word == PROFILE_FLAG).count() > 1);
    let (read, program) = read_line(&words[1..]).expect("read back");
    assert_eq!(read, profile);
    assert_eq!(program, vec!["codex".to_owned(), "--a".to_owned()]);
}

#[test]
fn words_that_are_not_a_wrap_s_line_are_not_read() {
    let words = |list: &[&str]| list.iter().map(|it| (*it).to_owned()).collect::<Vec<_>>();
    assert!(read_line(&words(&["--profile", "(version 1)", "--"])).is_none());
    assert!(read_line(&words(&["--other", "x", "--", "sh"])).is_none());
    assert!(read_line(&words(&["--profile", "(version 1)"])).is_none());
}

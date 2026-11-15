// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

// Every value here is an obvious fake; none is or was a credential.

fn known(name: &str, value: &str) -> Vec<Known> {
    Known::of(name, value).expect("a key for the process")
}

#[test]
fn a_value_on_a_screen_is_shown_as_its_entry_s_named_mask() {
    let screen = "$ deploy\r\ntoken is FAKE-VAULT-VALUE-0001, done\r\n";

    let masked = masked(screen, &known("github-token", "FAKE-VAULT-VALUE-0001")).unwrap();

    assert_eq!(
        masked,
        "$ deploy\r\ntoken is ‹secret: github-token›, done\r\n"
    );
}

#[test]
fn a_value_a_colour_change_runs_through_is_masked_and_the_colour_kept() {
    // The snapshot writes a soft-wrapped line as one run of cells, and a pen change between
    // two of them as a sequence: the value is still one word on the screen.
    let screen = "key: FAKE-VAULT-\x1b[1mVALUE-0002\x1b[0m\r\n";

    let masked = masked(screen, &known("db-password", "FAKE-VAULT-VALUE-0002")).unwrap();

    assert_eq!(masked, "key: ‹secret: db-password›\x1b[1m\x1b[0m\r\n");
}

#[test]
fn a_value_broken_by_a_hard_line_break_is_two_words_and_is_not_masked() {
    let screen = "FAKE-VAULT-\r\nVALUE-0003\r\n";

    let masked = masked(screen, &known("api-key", "FAKE-VAULT-VALUE-0003")).unwrap();

    assert_eq!(masked, screen);
}

#[test]
fn every_occurrence_is_masked_and_text_around_is_untouched() {
    let screen = "a FAKE-VALUE-0004 b FAKE-VALUE-0004 c ‹é›";

    let masked = masked(screen, &known("k", "FAKE-VALUE-0004")).unwrap();

    assert_eq!(masked, "a ‹secret: k› b ‹secret: k› c ‹é›");
}

#[test]
fn two_values_that_overlap_are_masked_whole_each_named_in_the_order_they_start() {
    let mut both = known("first", "FAKE-AAAA-BBBB");
    both.extend(known("second", "BBBB-CCCC-FAKE"));
    let screen = "[FAKE-AAAA-BBBB-CCCC-FAKE]";

    let masked = masked(screen, &both).unwrap();

    assert_eq!(masked, "[‹secret: first›‹secret: second›]");
}

#[test]
fn a_value_read_from_a_file_with_a_trailing_newline_is_masked_as_printed() {
    let screen = "pw=FAKE-FILE-VALUE-0005\r\n";

    let masked = masked(screen, &known("pw", "FAKE-FILE-VALUE-0005\n")).unwrap();

    assert_eq!(masked, "pw=‹secret: pw›\r\n");
}

#[test]
fn a_value_shorter_than_four_bytes_is_not_masked() {
    assert!(known("pin", "abc").is_empty());
    assert_eq!(masked("abc abc", &known("pin", "abc")).unwrap(), "abc abc");
}

#[test]
fn a_value_inside_an_osc_title_sequence_is_masked_in_the_title_text_too() {
    // A window title is text the terminal shows, and a kept screen should not carry it.
    let screen = "\x1b]0;FAKE-TITLE-VALUE-06\x07ok";

    let masked = masked(screen, &known("t", "FAKE-TITLE-VALUE-06")).unwrap();

    assert!(!masked.contains("FAKE-TITLE-VALUE-06"), "{masked:?}");
}

#[test]
fn what_a_chat_was_handed_masks_its_screen_and_no_other_chat_s() {
    let root = Path::new("/projects/handed-to-a-chat");
    remember(root, 41, "github-token", "FAKE-HANDED-VALUE-0007");

    assert_eq!(
        masked_for(root, 41, "x FAKE-HANDED-VALUE-0007").unwrap(),
        "x ‹secret: github-token›"
    );
    assert_eq!(
        masked_for(root, 42, "x FAKE-HANDED-VALUE-0007").unwrap(),
        "x FAKE-HANDED-VALUE-0007"
    );
    assert_eq!(
        masked_for(
            Path::new("/projects/another"),
            41,
            "x FAKE-HANDED-VALUE-0007"
        )
        .unwrap(),
        "x FAKE-HANDED-VALUE-0007"
    );
}

#[test]
fn a_chat_forgotten_as_it_closes_masks_nothing_of_what_it_was_handed() {
    let root = Path::new("/projects/handed-then-forgotten");
    remember(root, 43, "k", "FAKE-HANDED-VALUE-0008");

    forget(root, 43);

    assert_eq!(
        masked_for(root, 43, "FAKE-HANDED-VALUE-0008").unwrap(),
        "FAKE-HANDED-VALUE-0008"
    );
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

/// SK-49: every bidi control turns a title's line around, the marks and the Arabic letter
/// mark too, as the generated Bidi_Control table names them; a joiner stays.
#[test]
fn every_bidi_control_breaks_a_title_s_line_and_a_joiner_does_not() {
    for c in [
        '\u{200E}', '\u{200F}', '\u{061C}', '\u{202A}', '\u{202E}', '\u{2066}', '\u{2069}',
        '\u{2028}', '\u{2029}', '\n', '\u{85}',
    ] {
        assert!(breaks_a_line(c), "U+{:04X}", u32::from(c));
    }
    for c in ['\u{200D}', '\u{200C}', 'a', ' '] {
        assert!(!breaks_a_line(c), "U+{:04X}", u32::from(c));
    }
}

// Every expectation here came from charter itself: `from charter.memstore import slug`.

#[test]
fn a_title_becomes_its_lowercase_words_joined_by_single_dashes() {
    assert_eq!(slug("Hello World"), "hello-world");
    assert_eq!(slug("MiXeD CaSe 123"), "mixed-case-123");
    assert_eq!(
        slug("The API returns 418 on Mondays"),
        "the-api-returns-418-on-mondays"
    );
}

#[test]
fn dashes_at_the_ends_are_trimmed_before_anything_else() {
    assert_eq!(slug("  --Trailing dashes--  "), "trailing-dashes");
}

#[test]
fn a_title_with_nothing_sluggable_in_it_is_called_note() {
    assert_eq!(slug("!!!"), "note");
    assert_eq!(slug(""), "note");
}

#[test]
fn every_non_ascii_letter_is_a_separator_not_a_letter() {
    // Python lowercases first, then replaces anything outside [a-z0-9]; the leading
    // run is trimmed, the inner ones are not.
    assert_eq!(slug("Ünïcödé fäncy"), "n-c-d-f-ncy");
}

#[test]
fn the_cut_to_48_happens_after_trimming_so_it_can_leave_a_trailing_dash() {
    assert_eq!(slug(&"a".repeat(60)), "a".repeat(48));
    assert_eq!(slug(&"x ".repeat(30)), "x-".repeat(24));
}

#[test]
fn a_title_whose_filename_would_be_a_device_gets_one_that_travels() {
    // charter-app#96. A persona memory carries no timestamp prefix, so `nul.md` is the
    // whole filename — and on Windows that is the null device: the write succeeds, and
    // the note the operator just wrote is gone. A declared difference from the frozen
    // Python, which writes `nul.md`.
    assert_eq!(slug("NUL"), "nul-note");
    assert_eq!(slug("con"), "con-note");
    assert_eq!(slug("COM1"), "com1-note");
    assert_eq!(slug("Aux"), "aux-note");
    assert_eq!(slug("lpt9"), "lpt9-note");
    // The alphabet already made `com1.txt` into `com1-txt`, whose stem is not a device,
    // and a name that merely begins like one was never one.
    assert_eq!(slug("com1.txt"), "com1-txt");
    assert_eq!(slug("console"), "console");
    assert_eq!(slug("com10"), "com10");
}

#[test]
fn every_filename_this_store_mints_travels() {
    // The pairing `worktree::name` keeps between its slug and its gate, kept here too:
    // the derivation is a convenience and the rule is the containment, and a test is
    // what stops the two drifting into a gap.
    for title in [
        "NUL",
        "con",
        "COM\u{00b9}",
        "Ship the widget",
        "!!!",
        "",
        "\u{00dc}n\u{00ef}c\u{00f6}d\u{00e9} f\u{00e4}ncy",
        &"a".repeat(60),
    ] {
        let name = format!("{}.md", slug(title));
        assert!(
            crate::contain::mintable(&name).is_ok(),
            "a memory titled {title:?} is filed as {name:?}, which does not travel"
        );
    }
}

mod duplicate_tests;
mod gate_tests;
mod one_line_tests;
mod restore_as_tests;
mod write_tests;

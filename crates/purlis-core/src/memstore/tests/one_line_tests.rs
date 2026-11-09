// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn every_character_that_breaks_or_hides_a_line_is_folded_to_a_space() {
    for (title, folded) in [
        ("Two\nlines", "Two lines"),
        ("Line\u{2028}separator", "Line separator"),
        ("Paragraph\u{2029}separator", "Paragraph separator"),
        ("Next\u{85}line", "Next line"),
        ("Turned\u{202E}around", "Turned around"),
        ("Isolated\u{2066}text\u{2069}", "Isolated text "),
    ] {
        assert_eq!(one_line(title), folded, "{title:?}");
        assert_eq!(
            index_line(title, "a.md"),
            format!("- [{folded}](a.md)"),
            "{title:?}"
        );
    }
}

#[test]
fn a_title_that_is_one_line_is_kept_as_it_is() {
    // A zero-width joiner is how a family emoji is written.
    for title in ["Plain", "Fam\u{1F468}\u{200D}\u{1F469}ily", "Caf\u{e9}"] {
        assert!(
            matches!(one_line(title), std::borrow::Cow::Borrowed(_)),
            "{title:?}"
        );
    }
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn a_name_or_email_that_spells_a_secret_through_its_escapes_is_refused() {
    let tail = &crate::secretshape::escaped::token()[1..];
    let spelled = format!("\\u0067{tail}");
    for why in [
        name_refused(&spelled),
        email_refused(&format!("{spelled}@x.dev")),
    ] {
        let why = why.expect("refused");
        assert!(why.contains("looks like a secret"), "{why}");
        assert!(!why.contains(tail), "{why}");
    }
}

/// #1301 (D-1301-1): a ZWJ family emoji and a Persian name written with a ZWNJ are names,
/// and each joiner anywhere it does not join is refused like any other invisible character.
#[test]
fn a_joiner_that_joins_is_a_name_and_one_that_does_not_is_refused() {
    for name in [
        // man, ZWJ, woman, ZWJ, girl
        "\u{1f468}\u{200d}\u{1f469}\u{200d}\u{1f467}",
        // rainbow flag: white flag, VS-16, ZWJ, rainbow
        "Kai \u{1f3f3}\u{fe0f}\u{200d}\u{1f308}",
        // woman, skin tone, ZWJ, laptop
        "Ada \u{1f469}\u{1f3fd}\u{200d}\u{1f4bb}",
        // Persian: mi + ZWNJ + khaham
        "\u{0645}\u{06cc}\u{200c}\u{062e}\u{0648}\u{0627}\u{0647}\u{0645}",
    ] {
        assert_eq!(name_refused(name), None, "{name:?}");
    }
    let invisible = "A name is one line of visible characters: no control, zero-width or \
                         direction-changing characters.";
    for name in [
        // A lone ZWJ at either end.
        "\u{200d}\u{1f468}",
        "\u{1f468}\u{200d}",
        // Two joiners in a row, and a ZWJ between letters.
        "\u{1f468}\u{200d}\u{200d}\u{1f469}",
        "Ad\u{200d}a",
        // A ZWNJ at either end, between Latin letters, or after an emoji.
        "\u{200c}\u{0645}\u{06cc}",
        "\u{0645}\u{06cc}\u{200c}",
        "Ad\u{200c}a",
        "\u{1f468}\u{200c}\u{1f469}",
        // Every other invisible character, joined or not.
        "\u{0645}\u{200b}\u{062e}",
        "\u{1f468}\u{202e}\u{1f469}",
    ] {
        assert_eq!(name_refused(name).as_deref(), Some(invisible), "{name:?}");
    }
}

/// #1301 (D-1301-3): the Brahmic scripts join with both joiners too: a ZWJ after a virama
/// asks for a half form (a Devanagari conjunct), a ZWNJ after one keeps the virama shown.
/// Each passes only between a letter or virama and a letter of those scripts.
#[test]
fn a_devanagari_conjunct_and_a_hindi_name_with_a_non_joiner_are_names() {
    for name in [
        // ksha with a half ka: ka, virama, ZWJ, ssa
        "\u{0915}\u{094d}\u{200d}\u{0937}",
        // Hindi: ka, virama, ZWNJ, ssa, then aa
        "Ra \u{0915}\u{094d}\u{200c}\u{0937}\u{093e}",
        // Malayalam chillu: na, virama, ZWJ, then ta
        "\u{0d28}\u{0d4d}\u{200d}\u{0d24}",
    ] {
        assert_eq!(name_refused(name), None, "{name:?}");
    }
    let invisible = "A name is one line of visible characters: no control, zero-width or \
                         direction-changing characters.";
    for name in [
        // A lone joiner after a virama at the end, or before a Latin letter.
        "\u{0915}\u{094d}\u{200d}",
        "\u{0915}\u{094d}\u{200c}a",
        // Between a Devanagari digit and a letter, and after a danda.
        "\u{0966}\u{200d}\u{0915}",
        "\u{0964}\u{200c}\u{0915}",
        // Every other invisible character between Devanagari letters.
        "\u{0915}\u{200b}\u{0937}",
    ] {
        assert_eq!(name_refused(name).as_deref(), Some(invisible), "{name:?}");
    }
}

/// #1301 (D-1301-4): the confirmation draws a joiner the name check let through as itself,
/// so a joined name echoes back as it was typed, and still escapes every other invisible
/// character.
#[test]
fn a_joined_name_is_shown_as_typed_and_any_other_invisible_character_escaped() {
    for name in [
        "\u{1f468}\u{200d}\u{1f469}\u{200d}\u{1f467}",
        "\u{0645}\u{06cc}\u{200c}\u{062e}\u{0648}\u{0627}\u{0647}\u{0645}",
        "\u{0915}\u{094d}\u{200d}\u{0937}",
    ] {
        assert_eq!(crate::shown::line(name), name);
    }
    assert_eq!(crate::shown::line("Ad\u{200d}a"), "Ad⟦U+200D⟧a");
    assert_eq!(crate::shown::line("a\u{202e}b"), "a⟦U+202E⟧b");
    assert_eq!(
        crate::shown::line(&"\u{0915}".repeat(300)).chars().count(),
        crate::shown::DISPLAY_LIMIT + 1
    );
}

/// #1301: the joiners are a name's only. An email keeps the shared rule.
#[test]
fn an_email_keeps_refusing_every_joiner() {
    assert!(email_refused("\u{0645}\u{06cc}\u{200c}\u{062e}@x.dev").is_some());
    assert!(email_refused("\u{1f468}\u{200d}\u{1f469}@x.dev").is_some());
}

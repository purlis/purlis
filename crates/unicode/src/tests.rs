// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

/// Every character the three hand lists named (the review's, the hook's and the repo
/// instructions'), and the ones the ticket found they missed, is invisible by the one table.
#[test]
fn the_one_table_holds_every_character_the_hand_lists_named_and_the_ones_they_missed() {
    let named = [
        // The review's and the hook's: controls, zero-width, bidi, fillers, selectors, tags.
        '\u{0}',
        '\u{1B}',
        '\u{7F}',
        '\u{9F}',
        '\u{AD}',
        '\u{034F}',
        '\u{061C}',
        '\u{115F}',
        '\u{1160}',
        '\u{17B4}',
        '\u{180B}',
        '\u{180E}',
        '\u{200B}',
        '\u{200C}',
        '\u{200D}',
        '\u{200E}',
        '\u{200F}',
        '\u{2028}',
        '\u{2029}',
        '\u{202A}',
        '\u{202E}',
        '\u{2060}',
        '\u{2064}',
        '\u{2065}',
        '\u{2066}',
        '\u{2069}',
        '\u{206F}',
        '\u{3164}',
        '\u{FE00}',
        '\u{FE0F}',
        '\u{FEFF}',
        '\u{FFA0}',
        '\u{FFF0}',
        '\u{FFF9}',
        '\u{FFFB}',
        '\u{1D173}',
        '\u{E0000}',
        '\u{E0001}',
        '\u{E0041}',
        '\u{E007F}',
        '\u{E0100}',
        '\u{E01EF}',
        '\u{E01F0}',
        '\u{E0FFF}',
        // What SK-49 found the hand lists missed: Cf they never named, and blank spaces.
        '\u{0600}',
        '\u{0605}',
        '\u{06DD}',
        '\u{070F}',
        '\u{08E2}',
        '\u{110BD}',
        '\u{13430}',
        '\u{1343F}',
        '\u{1BCA0}',
        '\u{1BCA3}',
        '\u{2000}',
        '\u{200A}',
        '\u{3000}',
        '\u{A0}',
    ];
    for c in named {
        assert!(invisible(c), "U+{:04X}", u32::from(c));
    }
}

#[test]
fn a_character_with_a_glyph_is_not_invisible() {
    for c in [
        'a',
        ' ',
        'é',
        '日',
        'م',
        'क',
        '👍',
        '⟦',
        '\u{2800}',
        '\u{E1000}',
    ] {
        assert!(!invisible(c), "U+{:04X}", u32::from(c));
    }
}

#[test]
fn a_hidden_character_is_written_as_its_one_mark() {
    assert_eq!(
        marked("a\u{202E}b\u{200B}c", Breaks::Marked),
        "a⟦U+202E⟧b⟦U+200B⟧c"
    );
    assert_eq!(marked("\u{1B}[8m", Breaks::Marked), "⟦U+001B⟧[8m");
    assert_eq!(marked("tag\u{E0049}", Breaks::Marked), "tag⟦U+E0049⟧");
    assert_eq!(marked("plain 日本語", Breaks::Marked), "plain 日本語");
}

/// The terminal layers mark a cell and then the line it is in, so marking twice is marking once.
#[test]
fn marking_what_is_marked_changes_nothing() {
    let once = marked("a\u{202E}b\n\u{1F468}\u{200D}\u{1F469}", Breaks::Marked);
    assert_eq!(marked(&once, Breaks::Marked), once);
}

#[test]
fn line_ends_and_tabs_are_marked_or_kept_as_the_caller_says() {
    assert_eq!(
        marked("a\nb\tc\r", Breaks::Marked),
        "a⟦U+000A⟧b⟦U+0009⟧c⟦U+000D⟧"
    );
    assert_eq!(marked("a\nb\tc\r", Breaks::Kept), "a\nb\tc⟦U+000D⟧");
    assert!(hides("a\nb", Breaks::Marked));
    assert!(!hides("a\nb\tc", Breaks::Kept));
    assert_eq!(
        hidden("a\u{200B}\nb\u{202E}", Breaks::Kept).collect::<Vec<_>>(),
        ['\u{200B}', '\u{202E}']
    );
}

/// D-1301: a joiner doing its one job inside a script or an emoji reads as text.
#[test]
fn a_joiner_inside_a_script_or_an_emoji_reads_as_text() {
    for text in [
        // Persian: the prefix `mi` kept apart from the verb by a ZWNJ (می‌خواهم).
        "\u{0645}\u{06CC}\u{200C}\u{062E}\u{0648}\u{0627}\u{0647}\u{0645}",
        // Devanagari: KA, VIRAMA, ZWJ, SSA asks for the half form of KA.
        "\u{0915}\u{094D}\u{200D}\u{0937}",
        // Devanagari: KA, VIRAMA, ZWNJ, SSA keeps the virama shown.
        "\u{0915}\u{094D}\u{200C}\u{0937}",
        // A family: man, ZWJ, woman, ZWJ, girl.
        "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}",
        // A rainbow flag: white flag, VS16, ZWJ, rainbow.
        "\u{1F3F3}\u{FE0F}\u{200D}\u{1F308}",
        // A coder with a skin tone: woman, medium skin tone, ZWJ, laptop.
        "\u{1F469}\u{1F3FD}\u{200D}\u{1F4BB}",
        // A red heart in emoji presentation.
        "\u{2764}\u{FE0F}",
    ] {
        assert_eq!(marked(text, Breaks::Marked), text);
        assert!(!hides(text, Breaks::Marked), "{text:?}");
    }
}

/// A joiner or a selector anywhere else is a hidden character like any other.
#[test]
fn a_joiner_or_a_selector_doing_no_job_is_marked() {
    assert_eq!(marked("a\u{200D}b", Breaks::Marked), "a⟦U+200D⟧b");
    assert_eq!(marked("a\u{200C}b", Breaks::Marked), "a⟦U+200C⟧b");
    assert_eq!(
        marked("\u{200D}\u{1F469}", Breaks::Marked),
        "⟦U+200D⟧\u{1F469}"
    );
    assert_eq!(
        marked("\u{1F468}\u{200D}", Breaks::Marked),
        "\u{1F468}⟦U+200D⟧"
    );
    assert_eq!(
        marked("\u{1F468}\u{200D}\u{200D}\u{1F469}", Breaks::Marked),
        "\u{1F468}⟦U+200D⟧⟦U+200D⟧\u{1F469}"
    );
    // A Devanagari digit or danda does not take a joiner.
    assert_eq!(
        marked("\u{0966}\u{200D}\u{0915}", Breaks::Marked),
        "\u{0966}⟦U+200D⟧\u{0915}"
    );
    // A ZWNJ between two emoji, and a ZWJ between two Arabic letters, do no job.
    assert_eq!(
        marked("\u{1F468}\u{200C}\u{1F469}", Breaks::Marked),
        "\u{1F468}⟦U+200C⟧\u{1F469}"
    );
    assert_eq!(
        marked("\u{0645}\u{200D}\u{06CC}", Breaks::Marked),
        "\u{0645}⟦U+200D⟧\u{06CC}"
    );
    // A selector after a letter, a second selector, and a supplement selector carry no
    // presentation: they are how text is smuggled behind an emoji.
    assert_eq!(marked("a\u{FE0F}", Breaks::Marked), "a⟦U+FE0F⟧");
    assert_eq!(
        marked("\u{1F600}\u{FE0F}\u{FE0F}", Breaks::Marked),
        "\u{1F600}\u{FE0F}⟦U+FE0F⟧"
    );
    assert_eq!(
        marked("\u{1F600}\u{E0100}", Breaks::Marked),
        "\u{1F600}⟦U+E0100⟧"
    );
}

#[test]
fn a_default_ignorable_character_is_known_from_the_generated_table() {
    for c in [
        '\u{200B}',
        '\u{200D}',
        '\u{FE0F}',
        '\u{3164}',
        '\u{E0041}',
        '\u{AD}',
    ] {
        assert!(default_ignorable(c), "U+{:04X}", u32::from(c));
    }
    // Invisible, but not ignorable: a control, a blank space, an Arabic number sign.
    for c in ['\u{1B}', '\u{2000}', '\u{0600}', 'a'] {
        assert!(!default_ignorable(c), "U+{:04X}", u32::from(c));
    }
}

#[test]
fn a_bidi_control_is_known_from_the_generated_table() {
    for c in ['\u{061C}', '\u{200E}', '\u{202E}', '\u{2066}', '\u{2069}'] {
        assert!(bidi_control(c), "U+{:04X}", u32::from(c));
    }
    for c in ['\u{200B}', '\u{200D}', 'a', '\u{E0041}'] {
        assert!(!bidi_control(c), "U+{:04X}", u32::from(c));
    }
}

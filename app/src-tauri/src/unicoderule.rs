// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The one rule, handed to the window** (SK-51, #3202): which characters a reader does not
//! see, and the emoji data that decides where a joiner or a selector reads as text, as
//! `purlis_unicode` holds them.
//!
//! The window marks what it draws itself (a block's folder, a repository's instruction file),
//! and it used to keep its own lists for that: a regex of zero-width characters for the
//! instruction preview, `\p{Cc}\p{Cf}\p{Zl}\p{Zp}` for a block's target. Each disagreed with the
//! core's generated table. Now the tables cross the seam the way every other shape does: as a
//! constant of `app/src/bindings.ts` ([`UNICODE_RULE`]), generated from the very statics the
//! core asks, so the window cannot hold a list of its own. `app/src/invisible.ts` applies them
//! with the core's joiner rule (D-1301), and `app/src/invisible.cases.json` ([`cases`]) holds
//! the core's own answers for the window's tests to meet.

use purlis_unicode::tables;

/// The constant's name in `bindings.ts`.
pub const UNICODE_RULE: &str = "UNICODE_RULE";

/// The tables `purlis_unicode::hidden` asks, as the window reads them: inclusive codepoint
/// ranges, and each emoji sequence as its codepoints.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct UnicodeRule {
    /// [`tables::INVISIBLE`]: every character a reader does not see on its own.
    pub invisible: Vec<(u32, u32)>,
    /// [`tables::EXTENDED_PICTOGRAPHIC`]: what a presentation selector or a ZWJ may follow.
    pub extended_pictographic: Vec<(u32, u32)>,
    /// [`tables::EMOJI_MODIFIER`]: the skin tones, which a ZWJ may follow.
    pub emoji_modifier: Vec<(u32, u32)>,
    /// [`tables::EMOJI_SEQUENCES`]: the keycaps and RGI tag flags, spelled with their
    /// selector or tags.
    pub emoji_sequences: Vec<Vec<u32>>,
    /// [`tables::BRAHMIC_LETTER`]: the letters of Devanagari to Malayalam a joiner may stand
    /// between, so the window does not ask its own engine's `\p{Alphabetic}` (#3470).
    pub brahmic_letter: Vec<(u32, u32)>,
    /// [`tables::ARABIC_LETTER`]: the Arabic letters a ZWNJ may stand between.
    pub arabic_letter: Vec<(u32, u32)>,
}

/// The one rule's tables, from `purlis_unicode`'s generated statics.
pub fn rule() -> UnicodeRule {
    UnicodeRule {
        invisible: tables::INVISIBLE.to_vec(),
        extended_pictographic: tables::EXTENDED_PICTOGRAPHIC.to_vec(),
        emoji_modifier: tables::EMOJI_MODIFIER.to_vec(),
        emoji_sequences: tables::EMOJI_SEQUENCES
            .iter()
            .map(|sequence| sequence.iter().map(|&ch| u32::from(ch)).collect())
            .collect(),
        brahmic_letter: tables::BRAHMIC_LETTER.to_vec(),
        arabic_letter: tables::ARABIC_LETTER.to_vec(),
    }
}

/// Where the core's answers for the window's tests live.
#[cfg(test)]
const CASES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/invisible.cases.json");

/// **The core's answers the window's `marked` must give** (`app/src/invisible.test.ts`): for
/// each text, `purlis_unicode::marked` with the line ends kept and with them marked.
///
/// The texts are the joiner rule's every edge (D-1301), and a sweep of the tables themselves:
/// each end of every invisible range alone and between letters, each end of every
/// pictographic range with one and two selectors and joined to another, every emoji sequence
/// whole and cut short, and each Brahmic block's virama with both joiners. One JSON per line,
/// so a difference reads as the texts it is about.
#[cfg(test)]
fn cases() -> String {
    let mut texts: Vec<String> = [
        "a\u{202E}b\u{200B}c",
        "\u{1B}[8m",
        "tag\u{E0049}",
        "plain 日本語",
        "a\nb\tc\r",
        "a\r\nb",
        "git\u{00A0}commit",
        "a\u{2013}b\u{2002}c\u{3000}d",
        "\u{115F}\u{1160}\u{3164}\u{FFA0}",
        "\u{0645}\u{06CC}\u{200C}\u{062E}\u{0648}\u{0627}\u{0647}\u{0645}",
        "\u{0645}\u{200D}\u{06CC}",
        "\u{0915}\u{094D}\u{200D}\u{0937}",
        "\u{0915}\u{094D}\u{200C}\u{0937}",
        "\u{0966}\u{200D}\u{0915}",
        "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}",
        "\u{1F3F3}\u{FE0F}\u{200D}\u{1F308}",
        "\u{1F469}\u{1F3FD}\u{200D}\u{1F4BB}",
        "\u{2764}\u{FE0F}",
        "\u{00A9}\u{FE0F}\u{FE0F}",
        "a\u{200D}b",
        "a\u{200C}b",
        "\u{200D}\u{1F469}",
        "\u{1F468}\u{200D}",
        "\u{1F468}\u{200D}\u{200D}\u{1F469}",
        "\u{1F468}\u{200C}\u{1F469}",
        "a\u{FE0F}",
        "\u{1F600}\u{FE0F}\u{FE0F}",
        "\u{1F600}\u{E0100}",
        "\u{1F600}\u{E0069}",
        "a\u{E0041}",
        "a\u{FE0F}\u{20E3}",
        "\u{2300}\u{FE0F}",
        "\u{2B00}\u{FE0F}",
        "\u{1F3F4}\u{E0069}\u{E0067}\u{E006E}\u{E006F}\u{E0072}\u{E0065}\u{E007F}",
        "\u{1F3F4}\u{E0067}\u{E0062}\u{E0073}\u{E0063}\u{E0074}",
        "⟦U+202E⟧ is a mark, \u{27E6} is not hidden",
        "\u{1F468}\u{200D}\u{1F469}\u{202E}\u{1F467}",
        "",
    ]
    .iter()
    .map(|text| (*text).to_owned())
    .collect();
    let at = |cp: u32| char::from_u32(cp);
    for &(lo, hi) in &tables::INVISIBLE {
        for ch in [lo, hi].into_iter().filter_map(at) {
            texts.push(ch.to_string());
            texts.push(format!("a{ch}b"));
            texts.push(format!("\u{1F600}{ch}\u{1F600}"));
        }
    }
    for &(lo, hi) in &tables::EXTENDED_PICTOGRAPHIC {
        for ch in [lo, hi].into_iter().filter_map(at) {
            texts.push(format!("{ch}\u{FE0F}"));
            texts.push(format!("{ch}\u{FE0E}\u{FE0F}"));
            texts.push(format!("{ch}\u{FE0F}\u{200D}\u{1F308}"));
            texts.push(format!("{ch}\u{1F3FB}\u{200D}{ch}"));
        }
    }
    for sequence in &tables::EMOJI_SEQUENCES {
        let whole: String = sequence.iter().collect();
        let short: String = sequence[..sequence.len() - 1].iter().collect();
        texts.push(format!("x{whole}{whole}y"));
        texts.push(short);
    }
    for block in (0x0900..=0x0D00).step_by(0x80) {
        let letter = at(block + 0x15).expect("a Brahmic consonant");
        for virama in [block + 0x4D, 0x0D3B, 0x0D3C].into_iter().filter_map(at) {
            texts.push(format!("{letter}{virama}\u{200D}{letter}"));
            texts.push(format!("{letter}{virama}\u{200C}{letter}"));
            texts.push(format!("{letter}{virama}\u{200D}a"));
        }
    }
    // Each end of every letter range, and the codepoint just past it, beside a joiner (#3470).
    let ends = |table: &[(u32, u32)]| -> Vec<char> {
        table
            .iter()
            .flat_map(|&(lo, hi)| [lo.wrapping_sub(1), lo, hi, hi + 1])
            .filter_map(at)
            .collect()
    };
    for ch in ends(&tables::BRAHMIC_LETTER) {
        texts.push(format!("\u{0915}\u{094D}\u{200D}{ch}"));
        texts.push(format!("{ch}\u{200C}\u{0915}"));
    }
    for ch in ends(&tables::ARABIC_LETTER) {
        texts.push(format!("{ch}\u{200C}\u{0645}"));
        texts.push(format!("\u{0645}\u{200C}{ch}"));
    }
    let mut seen = std::collections::HashSet::new();
    texts.retain(|text| seen.insert(text.clone()));
    let mut out = String::from("[\n");
    for (n, text) in texts.iter().enumerate() {
        let case = serde_json::json!({
            "text": text,
            "kept": purlis_unicode::marked(text, purlis_unicode::Breaks::Kept),
            "marked": purlis_unicode::marked(text, purlis_unicode::Breaks::Marked),
        });
        out.push_str("  ");
        out.push_str(&case.to_string());
        out.push_str(if n + 1 == texts.len() { "\n" } else { ",\n" });
    }
    out.push_str("]\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "writes the window's unicode cases instead of checking them"]
    fn regenerate_the_windows_unicode_cases() {
        std::fs::write(CASES, cases()).expect("the cases are written");
    }

    #[test]
    fn the_windows_unicode_cases_are_the_cores_answers() {
        assert_eq!(
            std::fs::read_to_string(CASES).unwrap_or_default(),
            cases(),
            "{CASES} is out of date: run \
             `cargo test -p purlis-app -- --ignored regenerate_the_windows_unicode_cases`"
        );
    }

    #[test]
    fn the_rule_the_window_reads_is_the_cores_tables() {
        let rule = rule();
        assert_eq!(rule.invisible, tables::INVISIBLE);
        assert_eq!(rule.extended_pictographic, tables::EXTENDED_PICTOGRAPHIC);
        assert_eq!(rule.emoji_modifier, tables::EMOJI_MODIFIER);
        assert_eq!(rule.emoji_sequences.len(), tables::EMOJI_SEQUENCES.len());
        assert_eq!(rule.brahmic_letter, tables::BRAHMIC_LETTER);
        assert_eq!(rule.arabic_letter, tables::ARABIC_LETTER);
    }
}

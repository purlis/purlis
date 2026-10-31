// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A notice card's secondary way stands apart from the card it is on** (#4210), at the
//! engine's interface: a theme in, what the check says out. Every built-in appearance holds it on
//! all three card grounds; a theme whose button fill lands on a card ground is flagged.

use serde_json::{Value, json};

use super::super::{Finding, ResolvedTheme, built_ins, check, load, purlis};
use super::{NOTICE_WAY_POINTED, NOTICE_WAY_REST};

/// Every built-in appearance: Purlis Classic's and Purlis's.
fn every_built_in() -> impl Iterator<Item = &'static ResolvedTheme> {
    built_ins().iter().chain(&purlis().appearances)
}

/// A dark theme over Purlis Classic with `tokens` and `parts`, as read.
fn over_classic(tokens: Value, parts: Value) -> ResolvedTheme {
    let loaded = load(&json!({
        "name": "Probe",
        "extends": "purlis-classic",
        "appearances": [{ "id": "dark", "tokens": tokens }],
        "parts": parts,
    }));
    assert!(!loaded.refused, "{:?}", loaded.complaints);
    loaded.theme.first().clone()
}

/// What the check said of a card's way, as `(variant, ground, pointed)`, in the theme as written.
fn not_apart(theme: &ResolvedTheme) -> Vec<(&'static str, &'static str, bool)> {
    check(theme)
        .findings
        .into_iter()
        .filter_map(|one| match one {
            Finding::NoticeWayNotApart {
                tint: None,
                variant,
                ground,
                pointed,
                ..
            } => Some((variant, ground, pointed)),
            _ => None,
        })
        .collect()
}

#[test]
fn every_built_in_appearance_stands_its_card_ways_apart() {
    for theme in every_built_in() {
        assert_eq!(not_apart(theme), Vec::new(), "{}", theme.name);
    }
}

#[test]
fn a_palette_whose_card_ground_meets_the_card_s_mix_is_flagged() {
    // No parts: the card draws its own mix of the button's fill and the words. A palette whose
    // overlay is that mix's very colour, at rest, reads it as words with no box.
    let base = "#202020";
    let words = "#202020";
    let theme = over_classic(
        json!({ "control.base": base, "text.primary": words, "surface.overlay": base }),
        json!({}),
    );
    let found = not_apart(&theme);
    assert!(
        found.contains(&("news", "surface.overlay", false)),
        "{found:?}"
    );
    assert!(
        found.contains(&("news", "surface.overlay", true)),
        "{found:?}"
    );
}

#[test]
fn the_check_mixes_what_the_stylesheet_draws() {
    let css = include_str!("../../../../app/src/ui/notice/notice.css");
    for share in [NOTICE_WAY_REST, NOTICE_WAY_POINTED] {
        let mix = format!(
            "color-mix(in oklab, var(--control-base), var(--text-primary) {}%)",
            (share * 100.0).round()
        );
        assert!(css.contains(&mix), "notice.css draws {mix}");
    }
}

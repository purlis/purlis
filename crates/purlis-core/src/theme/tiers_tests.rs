// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The grey tiers on every ground** (#2902, ruling #2803): four text tiers held to 4.5:1 and
//! faint held to 3:1, on every ground a part draws them on, in every built-in theme, at the
//! engine's interface: a theme in, the report out.

use serde_json::{Value, json};

use super::super::{Finding, Report, ResolvedTheme, built_ins, check, classic, load, purlis};
use super::{FAINT, PAIRS, Pair, TEXT, TEXT_TIERS, TIER_GROUNDS, contrast};

/// The floor a tier is held to: words at the window's text size, a mark at 3:1.
fn floor_of(tier: &str) -> f64 {
    if tier == FAINT { 3.0 } else { TEXT }
}

/// Every tier, the text tiers loudest first and faint last.
fn tiers() -> impl Iterator<Item = &'static str> {
    TEXT_TIERS.into_iter().chain([FAINT])
}

/// The plain pair `front` on `back`: drawn straight on the ground, not faded, not selected.
fn plain(front: &str, back: &str) -> Option<&'static Pair> {
    PAIRS.iter().find(|pair| {
        pair.front == front
            && pair.back == back
            && pair.under.is_none()
            && pair.fade.is_none()
            && !pair.selected
    })
}

/// Every built-in appearance: Purlis Classic's and Purlis's.
fn every_built_in() -> impl Iterator<Item = &'static ResolvedTheme> {
    built_ins().iter().chain(&purlis().appearances)
}

/// A format 2 theme of one dark appearance that sets `tokens`, as read.
fn dark(tokens: Value) -> ResolvedTheme {
    let resolved = load(&json!({
        "name": "Probe",
        "appearances": [{ "id": "dark", "appearance": "dark", "tokens": tokens }]
    }));
    assert!(!resolved.refused, "{:?}", resolved.complaints);
    resolved.theme.first().clone()
}

/// The ratio the check found `front` on `back` at, as the theme is.
fn found(report: &Report, front: &str, back: &str) -> Option<f64> {
    report.findings.iter().find_map(|one| match one {
        Finding::Contrast {
            tint: None,
            pair,
            ratio,
        } if pair.front == front && pair.back == back && pair.under.is_none() => Some(*ratio),
        _ => None,
    })
}

#[test]
fn every_tier_is_held_on_every_ground_at_its_floor() {
    for tier in tiers() {
        for ground in TIER_GROUNDS {
            let pair = plain(tier, ground)
                .unwrap_or_else(|| panic!("{tier} on {ground} is not one of the check's pairs"));
            assert_eq!(pair.floor, floor_of(tier), "{tier} on {ground}");
        }
    }
}

#[test]
fn no_pair_is_listed_twice() {
    for (at, pair) in PAIRS.iter().enumerate() {
        assert!(
            !PAIRS[..at].contains(pair),
            "{} on {} is listed twice",
            pair.front,
            pair.back
        );
    }
}

#[test]
fn every_built_in_clears_every_tier_on_every_ground() {
    for theme in every_built_in() {
        let report = check(theme);
        for tier in tiers() {
            for ground in TIER_GROUNDS {
                assert_eq!(
                    found(&report, tier, ground),
                    None,
                    "{} ({}): {tier} on {ground}",
                    theme.name,
                    theme.id
                );
            }
        }
    }
}

#[test]
fn the_tiers_step_down_from_primary_to_faint_in_every_built_in() {
    for theme in every_built_in() {
        let on_window = |tier: &str| contrast(theme.value(tier), theme.value("surface.base"));
        let ratios: Vec<f64> = tiers().map(on_window).collect();
        for step in ratios.windows(2) {
            assert!(
                step[0] >= step[1],
                "{} ({}): {ratios:?}",
                theme.name,
                theme.id
            );
        }
    }
}

#[test]
fn purlis_dark_draws_the_ruled_greys() {
    let dark = purlis().first();
    assert_eq!(dark.value("text.primary"), "#e6e4ec");
    assert_eq!(dark.value("text.secondary"), "#c9c7d2");
    assert_eq!(dark.value("text.muted"), "#a09eab");
    assert_eq!(dark.value("text.quiet"), "#91909d");
    assert_eq!(dark.value("text.faint"), "#74727f");
}

#[test]
fn a_placeholder_is_quiet_in_every_appearance_of_purlis() {
    // A placeholder is informational text: it takes quiet, never faint.
    for one in &purlis().appearances {
        assert_eq!(
            one.value("placeholder"),
            one.value("text.quiet"),
            "{}",
            one.id
        );
    }
}

#[test]
fn purlis_classic_keeps_today_s_look_its_quiet_and_faint_are_its_muted() {
    for one in &classic().appearances {
        assert_eq!(
            one.value("text.quiet"),
            one.value("text.muted"),
            "{}",
            one.id
        );
        assert_eq!(
            one.value("text.faint"),
            one.value("text.muted"),
            "{}",
            one.id
        );
    }
}

#[test]
fn a_quiet_tier_that_fades_on_the_row_under_the_pointer_is_warned() {
    // Figma's own quiet grey, before the ruling lifted it: 4.10 on the hover row.
    let theme = dark(json!({
        "surface.hover": "#2a2a31",
        "text.quiet": "#8a8896",
    }));
    assert_eq!(
        found(&check(&theme), "text.quiet", "surface.hover"),
        Some(4.1)
    );
}

#[test]
fn a_faint_mark_under_three_on_the_selected_row_is_warned() {
    // Figma's faintest text grey, on Purlis's selected row.
    let theme = dark(json!({
        "list.selected": "#2a2936",
        "text.faint": "#6f6d7a",
    }));
    let ratio = found(&check(&theme), "text.faint", "list.selected").expect("a finding");
    assert!(ratio < 3.0, "{ratio}");
}

#[test]
fn a_faint_mark_is_held_to_the_mark_floor_and_not_the_text_floor() {
    // Purlis's faint on its card is 3.27: a mark, not words, so no finding.
    let theme = dark(json!({
        "surface.overlay": "#24242a",
        "text.faint": "#74727f",
    }));
    assert_eq!(found(&check(&theme), "text.faint", "surface.overlay"), None);
}

// ---------------------------------------------------------------------------------------
// A theme that leaves the quiet tiers out
// ---------------------------------------------------------------------------------------

#[test]
fn a_theme_that_names_muted_and_no_quiet_tier_draws_both_in_its_muted() {
    // Written before the tiers: its quiet words and its marks stay as legible as its muted.
    let theme = dark(json!({ "text.muted": "#8f8f99" }));
    assert_eq!(theme.value("text.quiet"), "#8f8f99");
    assert_eq!(theme.value("text.faint"), "#8f8f99");
}

#[test]
fn a_theme_that_names_quiet_and_no_faint_draws_its_marks_in_its_quiet() {
    let theme = dark(json!({ "text.muted": "#a0a0a8", "text.quiet": "#909098" }));
    assert_eq!(theme.value("text.quiet"), "#909098");
    assert_eq!(theme.value("text.faint"), "#909098");
}

#[test]
fn a_theme_that_names_every_tier_draws_each_as_named() {
    let theme = dark(json!({
        "text.muted": "#a0a0a8",
        "text.quiet": "#909098",
        "text.faint": "#707078",
    }));
    assert_eq!(theme.value("text.faint"), "#707078");
}

#[test]
fn a_theme_that_names_no_grey_tier_draws_purlis_classic_s() {
    let theme = dark(json!({ "accent.base": "#2bb3a3" }));
    let classic = classic().first();
    for tier in ["text.muted", "text.quiet", "text.faint"] {
        assert_eq!(theme.value(tier), classic.value(tier), "{tier}");
    }
}

/// A child of `parent` whose dark appearance gives `tokens`, as read.
fn child_of(parent: Value, tokens: Value) -> ResolvedTheme {
    let child = json!({
        "name": "Child",
        "extends": "parent",
        "appearances": [{ "id": "dark", "tokens": tokens }]
    });
    let resolved = super::super::Library::new()
        .with("parent", parent)
        .load(None, &child);
    assert!(!resolved.refused, "{:?}", resolved.complaints);
    resolved.theme.first().clone()
}

/// A parent theme whose dark appearance names its own three greys.
fn parent_with_tiers() -> Value {
    json!({
        "name": "Parent",
        "appearances": [{ "id": "dark", "appearance": "dark", "tokens": {
            "text.muted": "#a0a0a8", "text.quiet": "#909098", "text.faint": "#707078"
        } }]
    })
}

#[test]
fn a_child_that_changes_only_muted_keeps_its_parent_s_quiet_and_faint() {
    // It holds only its differences (ADR 0097 §4): the parent's tiers are its own, not muted's.
    let dark = child_of(parent_with_tiers(), json!({ "text.muted": "#b0b0b8" }));
    assert_eq!(dark.value("text.muted"), "#b0b0b8");
    assert_eq!(dark.value("text.quiet"), "#909098");
    assert_eq!(dark.value("text.faint"), "#707078");
}

#[test]
fn a_child_of_purlis_that_changes_only_muted_keeps_the_ruled_greys() {
    let child = json!({
        "name": "Brighter",
        "extends": "purlis",
        "appearances": [{ "id": "dark", "tokens": { "text.muted": "#a4a2af" } }]
    });
    let resolved = load(&child);
    assert!(!resolved.refused, "{:?}", resolved.complaints);
    let dark = resolved.theme.first();
    assert_eq!(dark.value("text.muted"), "#a4a2af");
    assert_eq!(dark.value("text.quiet"), "#91909d");
    assert_eq!(dark.value("text.faint"), "#74727f");
}

#[test]
fn a_child_changes_quiet_and_faint_by_naming_them() {
    let dark = child_of(
        parent_with_tiers(),
        json!({ "text.quiet": "#989aa0", "text.faint": "#78787f" }),
    );
    assert_eq!(dark.value("text.quiet"), "#989aa0");
    assert_eq!(dark.value("text.faint"), "#78787f");
}

#[test]
fn a_child_of_a_theme_whose_tiers_follow_muted_follows_its_own_muted() {
    // The parent named only muted, so its tiers are following it; the child's muted leads them.
    let parent = json!({
        "name": "Parent",
        "appearances": [{ "id": "dark", "appearance": "dark", "tokens": {
            "text.muted": "#a0a0a8"
        } }]
    });
    let dark = child_of(parent, json!({ "text.muted": "#b0b0b8" }));
    assert_eq!(dark.value("text.quiet"), "#b0b0b8");
    assert_eq!(dark.value("text.faint"), "#b0b0b8");
}

#[test]
fn an_option_that_changes_only_muted_keeps_the_appearance_s_quiet_and_faint() {
    let resolved = load(&json!({
        "name": "Dusk",
        "appearances": [{ "id": "night", "appearance": "dark", "tokens": {
            "text.muted": "#a0a0a8", "text.quiet": "#909098", "text.faint": "#707078"
        } }],
        "options": [{
            "id": "soft", "kind": "toggle", "label": "Soft", "description": "Softer words.",
            "default": true,
            "on": { "tokens": { "text.muted": "#8f8f99" } }
        }]
    }));
    assert!(!resolved.refused, "{:?}", resolved.complaints);
    let night = resolved.theme.first();
    assert_eq!(night.value("text.muted"), "#8f8f99");
    assert_eq!(night.value("text.quiet"), "#909098");
    assert_eq!(night.value("text.faint"), "#707078");
}

#[test]
fn an_option_that_names_muted_draws_the_quiet_tiers_in_it() {
    let resolved = load(&json!({
        "name": "Dusk",
        "appearances": [{ "id": "night", "appearance": "dark" }],
        "options": [{
            "id": "soft", "kind": "toggle", "label": "Soft", "description": "Softer words.",
            "default": true,
            "on": { "tokens": { "text.muted": "#8f8f99" } }
        }]
    }));
    assert!(!resolved.refused, "{:?}", resolved.complaints);
    let night = resolved.theme.first();
    assert_eq!(night.value("text.quiet"), "#8f8f99");
    assert_eq!(night.value("text.faint"), "#8f8f99");
}

#[test]
fn a_format_1_theme_that_names_muted_draws_the_quiet_tiers_in_it() {
    let resolved = load(&json!({ "name": "Old", "tokens": { "text.muted": "#8f8f99" } }));
    assert!(!resolved.refused, "{:?}", resolved.complaints);
    let theme = resolved.theme.first();
    assert_eq!(theme.value("text.quiet"), "#8f8f99");
    assert_eq!(theme.value("text.faint"), "#8f8f99");
}

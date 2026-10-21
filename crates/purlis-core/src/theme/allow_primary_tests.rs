// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Allow is distinguishable from the primary by a theme's tokens alone** (#2948, ADR 0097 §6
//! as amended 2026-10-21), at the engine's interface: a theme in, what the window draws out.

use serde_json::{Value, json};

use super::super::{
    Appearance, Floor, ResolvedTheme, Theme, built_in_for, built_ins, check_every, draw, load,
    parts, purlis,
};
use super::{ALLOW_FROM_PRIMARY, hold};

/// A dark theme that sets `tokens` and is Purlis Classic's for the rest.
fn dark(tokens: Value) -> Theme {
    let resolved = load(&json!({
        "name": "Probe",
        "appearances": [{ "id": "dark", "appearance": "dark", "tokens": tokens }]
    }));
    assert!(!resolved.refused, "{:?}", resolved.complaints);
    resolved.theme
}

fn ours(token: &str) -> &'static str {
    built_in_for(Appearance::Dark).value(token)
}

/// What the window draws of `theme`'s first appearance, and the floors it held.
fn drawn_of(theme: &Theme) -> (ResolvedTheme, Vec<Floor>) {
    let drawn = draw(theme.first());
    (
        drawn.theme.expect("a theme is always drawn"),
        drawn.held.map(|held| held.floors).unwrap_or_default(),
    )
}

/// The nearest Allow comes to the primary in `theme`, in any state.
fn nearest(theme: &ResolvedTheme) -> f64 {
    parts::allow_from_primary(theme)
        .into_iter()
        .map(|look| look.apart)
        .fold(f64::INFINITY, f64::min)
}

// ---------------------------------------------------------------------------------------
// The built-in themes
// ---------------------------------------------------------------------------------------

#[test]
fn every_built_in_appearance_holds_allow_apart_from_the_primary_unchanged() {
    for theme in built_ins().iter().chain(&purlis().appearances) {
        assert!(
            nearest(theme) >= ALLOW_FROM_PRIMARY,
            "{} ({})",
            theme.name,
            theme.id
        );
        let (drawn, held) = hold(theme);
        assert!(
            held.is_none_or(|held| !held.floors.contains(&Floor::AllowNotPrimary)),
            "{} ({})",
            theme.name,
            theme.id
        );
        assert_eq!(drawn.values, theme.values, "{} ({})", theme.name, theme.id);
    }
}

#[test]
fn every_built_in_combination_draws_with_no_consent_floor_held() {
    for theme in [super::super::classic(), purlis()] {
        for one in check_every(theme) {
            assert!(
                one.held
                    .as_ref()
                    .is_none_or(|held| !held.floors.contains(&Floor::AllowNotPrimary)),
                "{}: {:?}",
                one.appearance,
                one.held
            );
        }
    }
}

// ---------------------------------------------------------------------------------------
// A theme whose tokens draw Allow as the primary
// ---------------------------------------------------------------------------------------

#[test]
fn a_primary_drawn_on_allow_s_fill_is_drawn_as_purlis_s_own() {
    let theme = dark(json!({ "accent.surface": ours("control.base") }));
    let (drawn, floors) = drawn_of(&theme);
    assert!(floors.contains(&Floor::AllowNotPrimary), "{floors:?}");
    assert_eq!(drawn.value("accent.surface"), ours("accent.surface"));
    // Allow stays as plain as Keep blocked: its fill is the theme's.
    assert_eq!(
        drawn.value("control.base"),
        theme.first().value("control.base")
    );
    assert!(nearest(&drawn) >= ALLOW_FROM_PRIMARY);
}

#[test]
fn allow_s_fill_drawn_as_the_primary_s_is_held_by_the_primary_s_fill() {
    let theme = dark(json!({ "control.base": ours("accent.surface") }));
    let (drawn, floors) = drawn_of(&theme);
    assert!(floors.contains(&Floor::AllowNotPrimary), "{floors:?}");
    assert!(nearest(&drawn) >= ALLOW_FROM_PRIMARY);
}

#[test]
fn the_primary_under_the_pointer_is_held_too() {
    let theme = dark(json!({ "tab.active": ours("control.hover") }));
    let (drawn, floors) = drawn_of(&theme);
    assert!(floors.contains(&Floor::AllowNotPrimary), "{floors:?}");
    assert_eq!(drawn.value("tab.active"), ours("tab.active"));
    assert!(nearest(&drawn) >= ALLOW_FROM_PRIMARY);
}

#[test]
fn looks_are_compared_by_the_colours_drawn_never_by_the_tokens_names() {
    // A hex of its own, the same colour as Allow's fill.
    let theme = dark(json!({ "accent.surface": ours("control.base").to_uppercase() }));
    let (_, floors) = drawn_of(&theme);
    assert!(floors.contains(&Floor::AllowNotPrimary), "{floors:?}");
}

#[test]
fn a_see_through_primary_is_measured_as_it_shows_on_its_grounds() {
    let theme = dark(json!({ "accent.surface": format!("{}ee", ours("control.base")) }));
    let (drawn, floors) = drawn_of(&theme);
    assert!(floors.contains(&Floor::AllowNotPrimary), "{floors:?}");
    assert!(nearest(&drawn) >= ALLOW_FROM_PRIMARY);
}

#[test]
fn a_theme_whose_tokens_keep_them_apart_draws_as_written() {
    let theme = dark(json!({ "accent.surface": "#0f4a3c", "tab.active": "#145444" }));
    let (drawn, floors) = drawn_of(&theme);
    assert!(!floors.contains(&Floor::AllowNotPrimary), "{floors:?}");
    assert_eq!(drawn.value("accent.surface"), "#0f4a3c");
}

#[test]
fn a_tint_that_brings_them_together_leaves_the_primary_untinted_there() {
    // Apart as written: a red control and a green primary of one lightness and chroma. A tint
    // turns the primary to the control's hue.
    let resolved = load(&json!({
        "name": "Probe",
        "appearances": [{ "id": "dark", "appearance": "dark", "tokens": {
            "control.base": "#2f2424",
            "accent.surface": "#1e2a2a"
        } }],
        "tint": { "tokens": ["accent.surface"] }
    }));
    assert!(!resolved.refused, "{:?}", resolved.complaints);
    let theme = resolved.theme.first();
    assert!(nearest(theme) >= ALLOW_FROM_PRIMARY);
    let drawn = draw(theme);
    let held = drawn.held.expect("held in a tint");
    assert!(held.floors.contains(&Floor::AllowNotPrimary), "{held:?}");
    assert!(
        held.untinted.contains(&"accent.surface") || held.tokens.contains(&"accent.surface"),
        "{held:?}"
    );
}

#[test]
fn the_notice_names_the_floor() {
    let theme = dark(json!({ "accent.surface": ours("control.base") }));
    let held = draw(theme.first()).held.expect("held");
    assert!(
        held.said()
            .contains("Allow is distinguishable from the primary"),
        "{}",
        held.said()
    );
}

#[test]
fn the_floor_holds_on_the_final_token_set_whatever_layer_set_it() {
    // A layer laid over a resolved theme after it was read (the person's own colours, say) is
    // drawn through the same floor: it measures the tokens the window draws, not a file.
    for appearance in [Appearance::Dark, Appearance::Light] {
        let classic = built_in_for(appearance);
        for (token, from) in [
            ("control.base", "accent.surface"),
            ("accent.surface", "control.base"),
            ("control.hover", "tab.active"),
        ] {
            let mut laid = classic.clone();
            laid.values.insert(token, classic.value(from).to_owned());
            let drawn = draw(&laid);
            let held = drawn.held.expect("held");
            assert!(
                held.floors.contains(&Floor::AllowNotPrimary),
                "{appearance:?} {token}: {held:?}"
            );
            assert!(
                nearest(&drawn.theme.expect("drawn")) >= ALLOW_FROM_PRIMARY,
                "{appearance:?} {token}"
            );
        }
    }
}

#[test]
fn a_hue_no_named_tint_takes_is_measured_too() {
    // Clears the floor as written and in every named tint, and falls under it at a hue a
    // workspace's own colour can take: the two fills are drawn untinted there.
    let theme = dark(json!({ "accent.surface": "#222b1f", "tab.active": "#222b1f" }));
    let one = theme.first();
    assert!(nearest(one) >= ALLOW_FROM_PRIMARY);
    for hue in &one.tint.hues {
        assert!(
            nearest(&super::super::tint::tinted(one, hue.name)) >= ALLOW_FROM_PRIMARY,
            "{}",
            hue.name
        );
    }
    let held = draw(one).held.expect("held at some hue");
    assert!(held.floors.contains(&Floor::AllowNotPrimary), "{held:?}");
    assert!(held.tokens.is_empty(), "{held:?}");
    assert!(!held.untinted.is_empty(), "{held:?}");
}

#[test]
fn every_built_in_clears_the_floor_at_every_whole_degree() {
    for theme in built_ins().iter().chain(&purlis().appearances) {
        if theme.tint.tabs().is_empty() {
            continue;
        }
        for degree in 0..360 {
            let tinted = super::super::tint::tinted_at(theme, f64::from(degree));
            assert!(
                nearest(&tinted) >= ALLOW_FROM_PRIMARY,
                "{} ({}) at {degree}",
                theme.name,
                theme.id
            );
        }
    }
}

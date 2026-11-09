// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Allow stays apart from the primary across states** (#2994, spec #2897): a person sees one
//! hovered or focused control beside others at rest, so Allow in any state it is drawn in is
//! measured against the primary in any state it is drawn in, by the token floor
//! ([`Floor::AllowNotPrimary`]) and by the parts check, each with its fallback.

use serde_json::{Value, json};

use super::parts::{self, State};
use super::*;

/// A dark theme over Purlis that sets `tokens` and `parts`.
fn over_purlis(tokens: Value, parts: Value) -> Theme {
    let resolved = load(&json!({
        "name": "Probe",
        "extends": "purlis",
        "parts": parts,
        "appearances": [{ "id": "dark", "appearance": "dark", "tokens": tokens }]
    }));
    assert!(!resolved.refused, "{:?}", resolved.complaints);
    resolved.theme
}

/// The nearest Allow comes to the primary in `theme`'s tokens, in any state of each.
fn nearest(theme: &ResolvedTheme) -> f64 {
    parts::allow_from_primary(theme)
        .into_iter()
        .map(|look| look.apart)
        .fold(f64::INFINITY, f64::min)
}

/// The nearest Allow comes to the primary in `theme`'s tokens with both in the same state: what
/// the floor measured before #2994.
fn nearest_alike(theme: &ResolvedTheme) -> f64 {
    parts::allow_from_primary(theme)
        .into_iter()
        .filter(|look| look.allow == look.primary)
        .map(|look| look.apart)
        .fold(f64::INFINITY, f64::min)
}

// ---------------------------------------------------------------------------------------
// The token floor
// ---------------------------------------------------------------------------------------

#[test]
fn every_allow_state_is_measured_against_every_primary_state() {
    let ours = purlis().first().clone();
    let measured = parts::allow_from_primary(&ours);
    for (allow, primary) in [
        (vec![State::Hover], vec![]),
        (vec![], vec![State::Hover]),
        (vec![State::Hover], vec![State::Hover]),
        (vec![], vec![]),
    ] {
        // A pair two others draw alike is measured once, so it is found by the looks it draws.
        let family = parts::family("button").expect("the button family");
        let look = |variant, states: &[State]| parts::default_look(family, variant, states);
        assert!(
            measured.iter().any(|one| {
                look("grants", &one.allow) == look("grants", &allow)
                    && look("primary", &one.primary) == look("primary", &primary)
            }),
            "Allow in {allow:?} against the primary in {primary:?}: {measured:?}"
        );
    }
}

#[test]
fn tokens_that_draw_allow_under_the_pointer_as_the_primary_at_rest_are_held_apart() {
    // Allow under the pointer drawn as the primary is at rest: the hovered control's ground the
    // primary's fill, and the primary's edge Allow's. In the same state each is apart.
    let ours = purlis().first().clone();
    let theme = over_purlis(
        json!({
            "control.hover": ours.value("accent.fill"),
            "accent.fill-edge": ours.value("accent.base"),
        }),
        json!({}),
    );
    assert!(
        nearest_alike(theme.first()) >= ALLOW_FROM_PRIMARY,
        "the same state alone is apart: {}",
        nearest_alike(theme.first())
    );
    assert!(nearest(theme.first()) < ALLOW_FROM_PRIMARY);
    let drawn = draw(theme.first());
    let floors = drawn.held.map(|held| held.floors).unwrap_or_default();
    assert!(floors.contains(&Floor::AllowNotPrimary), "{floors:?}");
    let drawn = drawn.theme.expect("a theme is always drawn");
    assert!(nearest(&drawn) >= ALLOW_FROM_PRIMARY, "{}", nearest(&drawn));
}

#[test]
fn every_built_in_appearance_holds_allow_apart_from_the_primary_across_states() {
    for theme in built_ins().iter().chain(&purlis().appearances) {
        assert!(
            nearest(theme) >= ALLOW_FROM_PRIMARY,
            "{} ({}): {:?}",
            theme.name,
            theme.id,
            parts::allow_from_primary(theme)
        );
    }
}

// ---------------------------------------------------------------------------------------
// The parts check
// ---------------------------------------------------------------------------------------

/// A theme over Purlis Classic, whose primary has Allow's accent edge, that sets `parts`.
fn over_classic(parts: Value) -> Theme {
    let resolved = load(&json!({ "name": "Probe", "extends": "purlis-classic", "parts": parts }));
    assert!(!resolved.refused, "{:?}", resolved.complaints);
    resolved.theme
}

/// Parts that draw Allow under the pointer as Purlis Classic draws the primary at rest (its fill
/// behind the same accent edge), with Keep blocked on Allow's fill, and the primary under the
/// pointer as Allow at rest: in any one state the two are as far apart as the defaults draw them.
fn allow_hovered_as_the_primary_at_rest() -> Value {
    json!({
        "button.root.secondary.hover.fill": "accent.fill",
        "button.root.grants.hover.fill": "accent.fill",
        "button.root.primary.hover.fill": "control.base",
    })
}

#[test]
fn parts_that_draw_allow_in_one_state_as_the_primary_in_another_fall_back() {
    let theme = over_classic(allow_hovered_as_the_primary_at_rest());
    assert!(
        parts::fallen_alike(theme.first()).is_empty(),
        "in any one state the two are apart"
    );
    let fallen = parts::fallen(theme.first());
    let across: Vec<_> = fallen
        .iter()
        .filter(|one| one.floor.starts_with("Allow never takes the primary look"))
        .collect();
    assert!(!across.is_empty(), "{fallen:?}");
    // The finding names the pair it measured.
    assert!(
        across
            .iter()
            .any(|one| one.floor.contains("under the pointer") && one.floor.contains("at rest")),
        "{across:?}"
    );
}

#[test]
fn what_the_parts_draw_after_the_fallback_holds_allow_apart_in_every_pair() {
    let theme = over_classic(allow_hovered_as_the_primary_at_rest());
    assert!(parts::held_again(theme.first()).is_empty());
    let near = parts::allow_near_primary_across(theme.first());
    assert!(near.is_empty(), "{near:?}");
}

#[test]
fn a_theme_whose_parts_keep_allow_apart_in_every_pair_draws_as_written() {
    // The primary rounded and Allow's edge in the waiting colour's place untouched: nothing nears.
    let theme = over_purlis(
        json!({}),
        json!({
            "button.root.corner-radius": "full",
            "button.root.primary.hover.fill": "accent.fill",
        }),
    );
    let fallen = parts::fallen(theme.first());
    assert!(
        !fallen
            .iter()
            .any(|one| one.floor.starts_with("Allow never takes the primary look")),
        "{fallen:?}"
    );
}

// ---------------------------------------------------------------------------------------
// Toggle states
// ---------------------------------------------------------------------------------------

#[test]
fn the_primary_and_the_consent_answers_are_never_drawn_pressed_as_a_toggle() {
    // A toggle's pressed look is the plain and the ghost button's alone (#3255): the primary,
    // Allow and danger keep their own looks in every state they are drawn in, so no pressed look
    // is shared between them.
    let family = parts::family("button").expect("the button family");
    for variant in ["primary", "grants", "danger"] {
        assert!(!family.draws(variant, &[State::Selected]), "{variant}");
    }
    for one in parts::allow_from_primary(purlis().first()) {
        assert!(
            !one.allow.contains(&State::Selected) && !one.primary.contains(&State::Selected),
            "{one:?}"
        );
    }
}

#[test]
fn the_defaults_keep_allow_danger_and_the_primary_apart_in_every_pair_of_states() {
    // The family's own looks, in each built-in appearance: Allow's against the primary's, and
    // danger's words against the primary's, in every pair of looks each is drawn in.
    for theme in built_ins().iter().chain(&purlis().appearances) {
        assert!(
            nearest(theme) >= ALLOW_FROM_PRIMARY,
            "{} ({})",
            theme.name,
            theme.id
        );
        let words = parts::danger_words_from_primary(theme);
        assert!(
            words >= DANGER_APART,
            "{} ({}): {words}",
            theme.name,
            theme.id
        );
        // And what the built-in's own parts draw holds them apart in every pair too.
        let near = parts::allow_near_primary_across(theme);
        assert!(near.is_empty(), "{} ({}): {near:?}", theme.name, theme.id);
    }
}


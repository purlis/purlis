// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The parts check holds only what the window draws** (#3384): a look [`super::Family::draws`]
//! leaves out (a danger button pressed, a danger row open) brings no finding and repaints no
//! surface, and a part's words are read on the grounds the part stands on, not on every ground
//! its family does.

use serde_json::{Value, json};

use super::super::{ResolvedTheme, load};
use super::{State, alike_as, broken_on, fallen, family, rules};

/// A dark theme over Purlis Classic that sets `tokens` and `parts`, untinted, as read.
fn shaped(tokens: Value, parts: Value) -> ResolvedTheme {
    let loaded = load(&json!({
        "name": "Shaped",
        "extends": "purlis-classic",
        "appearances": [{ "id": "dark", "tokens": tokens }],
        "parts": parts,
        "tint": { "on": false },
    }));
    assert!(!loaded.refused, "{:?}", loaded.complaints);
    loaded.theme.first().clone()
}

/// The looks that fell, each named.
fn fell(theme: &ResolvedTheme) -> Vec<String> {
    fallen(theme).into_iter().map(|one| one.part).collect()
}

#[test]
fn a_look_the_window_never_draws_brings_no_finding_and_no_fallback() {
    // Words the colour of their own fill: on a pressed danger button and an open danger row,
    // neither of which the window draws.
    let undrawn = shaped(
        json!({}),
        json!({
            "button.root.danger.selected.text": "accent.surface",
            "menu.root.danger.open.text": "tab.active",
        }),
    );
    assert_eq!(fell(&undrawn), Vec::<String>::new());
    // The same words on the looks the window does draw fall.
    let drawn = shaped(
        json!({}),
        json!({
            "button.root.secondary.selected.text": "accent.surface",
            "menu.root.default.open.text": "tab.active",
        }),
    );
    let drawn = fell(&drawn);
    for look in ["button.root.secondary.selected", "menu.root.default.open"] {
        assert!(drawn.contains(&look.to_owned()), "{look}: {drawn:?}");
    }
}

/// A theme whose panel is the accent's surface, and whose Allow, in `state`, has no fill of its
/// own and an edge lost on that panel.
fn allow_lost_on_its_panel(state: &str) -> ResolvedTheme {
    shaped(
        json!({}),
        json!({
            "panel.root.fill": "accent.surface",
            format!("button.root.grants.{state}.fill"): "transparent",
            format!("button.root.grants.{state}.border-colour"): "accent.fill",
        }),
    )
}

/// What the buttons on `theme`'s panel break there, read from its rules as written.
fn broken_on_the_panel(theme: &ResolvedTheme) -> Option<(String, Option<f64>)> {
    let button = family("button").expect("the button family");
    let panel = family("panel").expect("the panel family");
    let written = rules(&theme.parts);
    broken_on(
        theme,
        &[],
        std::slice::from_ref(theme),
        &written,
        button,
        panel,
        "accent.surface",
    )
}

#[test]
fn a_surface_keeps_a_fill_only_an_undrawn_look_breaks_on() {
    // Allow is never a toggle, so nobody sees its pressed edge: the panel keeps its fill.
    let undrawn = allow_lost_on_its_panel("selected");
    assert_eq!(broken_on_the_panel(&undrawn), None);
    assert_eq!(fell(&undrawn), Vec::<String>::new());
    // Under the pointer it is seen, and the same edge is held.
    let drawn = allow_lost_on_its_panel("hover");
    assert!(broken_on_the_panel(&drawn).is_some());
}

#[test]
fn a_field_s_error_is_read_only_where_a_field_that_can_refuse_stands() {
    let on_a_tab = json!({ "tab.active": "#a3a29a" });
    let field_fell = |theme: &ResolvedTheme| -> Vec<String> {
        fallen(theme)
            .into_iter()
            .filter(|one| one.part.starts_with("field."))
            .map(|one| one.floor)
            .collect()
    };
    // A chosen tab's ground is one a field stands on, but none that can refuse its value does.
    let error = shaped(on_a_tab.clone(), json!({ "field.error.text": "text.primary" }));
    assert_eq!(field_fell(&error), Vec::<String>::new());
    // Its label stands on all of them, and is read on all of them.
    let label = shaped(on_a_tab, json!({ "field.label.text": "text.secondary" }));
    let said = field_fell(&label);
    assert!(said.iter().any(|one| one.contains("label")), "{said:?}");
    // On a ground a refusing field stands on, its error is held.
    let lost = shaped(json!({}), json!({ "field.error.text": "surface.raised" }));
    let said = field_fell(&lost);
    assert!(said.iter().any(|one| one.contains("error")), "{said:?}");
}

#[test]
fn two_sets_of_states_are_alike_only_if_the_same_looks_are_drawn_in_them() {
    // Each draws a menu's rows alike, but a danger row is read in only one of them: the set read
    // first must not stand for the other.
    let menu = family("menu").expect("the menu family");
    assert_ne!(
        alike_as(&[], menu, &[State::Hover, State::Open]),
        alike_as(&[], menu, &[State::Hover, State::Selected]),
    );
}

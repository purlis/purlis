// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The parts check holds only what the window draws** (#3384): a look [`super::Family::draws`]
//! leaves out (a danger button pressed, a danger row open) brings no finding and repaints no
//! surface, and a part's words are read on the grounds the part stands on, not on every ground
//! its family does.

use serde_json::{Value, json};

use super::super::{ResolvedTheme, load};
use super::{STANDS_ON, State, alike_as, broken_on, fallen, family, rules};

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
    // A look's first broken floor is the one it reports, and its help is read before its error, on
    // every ground a field stands on and each with a wash over it: its quiet words miss 4.5:1 on
    // danger's wash over an active tab. The help here reads on all of them, so the error is read.
    let field_fell = |part: &str, colour: &str| -> Vec<String> {
        let parts = json!({ "field.help.text": "text.primary", part: colour });
        fallen(&shaped(json!({}), parts))
            .into_iter()
            .filter(|one| one.part.starts_with("field."))
            .map(|one| one.floor)
            .collect()
    };
    // Danger's colour clears 4.5:1 on all three grounds a field that can refuse stands on, and
    // misses it on a hairline's ground, which a field stands on but none that can refuse does.
    assert_eq!(
        field_fell("field.error.text", "danger.base"),
        Vec::<String>::new()
    );
    // Its label stands on all of them, and is read on all of them.
    let said = field_fell("field.label.text", "danger.base");
    assert!(said.iter().any(|one| one.contains("label")), "{said:?}");
    // On a ground a refusing field stands on, its error is held.
    let said = field_fell("field.error.text", "surface.raised");
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

#[test]
fn a_part_on_fewer_grounds_than_its_family_stands_on_no_surface_a_theme_fills() {
    // Such a part is read on its own list, which takes the place of a surface's fill: a family on
    // a filled surface would not be read on that fill.
    for (surface, families) in STANDS_ON {
        for one in families.iter().map(|name| family(name).expect("a family")) {
            let many: Vec<_> = one.under.iter().filter(|(_, on)| on.len() > 1).collect();
            assert!(many.is_empty(), "{} on a {surface}: {many:?}", one.name);
        }
    }
}

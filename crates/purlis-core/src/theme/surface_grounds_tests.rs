// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A surface's fill a theme sets is a ground of what is drawn on it** (#3099, from #3083): a
//! button on a panel or a popover whose fill a theme's parts change is measured on that fill,
//! the consent floors in every tint, and the surface draws its default where one breaks.

use serde_json::{Value, json};

use super::super::{ResolvedTheme, load};
use super::{Consented, consent_broken, fallen, family, surface_ground};

/// A dark theme over Purlis Classic that sets `tokens` and `parts`, as read.
fn shaped(tokens: Value, parts: Value) -> ResolvedTheme {
    let loaded = load(&json!({
        "name": "Shaped",
        "extends": "purlis-classic",
        "appearances": [{ "id": "dark", "tokens": tokens }],
        "parts": parts,
    }));
    assert!(!loaded.refused, "{:?}", loaded.complaints);
    loaded.theme.first().clone()
}

/// What fell, as `<part>: <floor>`.
fn fell_in(theme: &ResolvedTheme) -> Vec<String> {
    fallen(theme)
        .into_iter()
        .map(|one| format!("{}: {}", one.part, one.floor))
        .collect()
}

#[test]
fn a_popover_filled_in_the_words_of_the_buttons_in_it_draws_its_own_fill() {
    // The popover's own words read on its new fill; a ghost button's primary words in it do not.
    let theme = shaped(
        json!({}),
        json!({
            "popover.root.fill": "text.primary",
            "popover.root.text": "surface.base",
            "popover.footer.text": "surface.base",
            "popover.key.fill": "surface.base",
        }),
    );
    let fell = fell_in(&theme);
    assert!(
        fell.iter().any(
            |one| one.starts_with("popover.root.") && one.contains("what a button draws on it")
        ),
        "{fell:?}"
    );
    // Drawn as its default, the popover stands on its own surface again.
    assert!(
        super::stylesheet(&theme).contains("background-color: var(--surface-overlay)"),
        "the popover draws its default fill"
    );
}

#[test]
fn every_surface_a_family_stands_on_takes_a_fill_and_is_named_once() {
    // Who stands where is found from the components (`app/src/theme/grounds.test.ts`); here, that
    // the table names surfaces a theme can fill and families the manifest has.
    let mut seen = Vec::new();
    for (surface, families) in super::STANDS_ON {
        assert!(!seen.contains(surface), "{surface} is named twice");
        seen.push(surface);
        let one = family(surface).expect("a surface");
        assert!(
            one.parts[0].properties.contains(&super::Prop::Fill),
            "{surface}"
        );
        for name in *families {
            assert!(family(name).is_some(), "{name} on a {surface}");
            assert_ne!(name, surface, "a {surface} on itself");
        }
    }
}

#[test]
fn a_surface_fill_every_family_on_it_reads_on_is_drawn_as_written() {
    // A raised panel, as Purlis draws its own: every family drawn on the window reads on it.
    let theme = shaped(json!({}), json!({ "panel.root.fill": "surface.raised" }));
    assert_eq!(fell_in(&theme), Vec::<String>::new());
}

#[test]
fn allow_s_edge_is_held_on_a_surface_fill_a_theme_sets() {
    // A see-through control lets the surface under Allow show through it: on a surface the
    // accent's colour, its accent edge is lost.
    let theme = shaped(
        json!({ "control.base": "#2626230d" }),
        json!({ "panel.root.fill": "accent.base" }),
    );
    let button = family("button").expect("the button family");
    let panel = family("panel").expect("the panel family");
    let grants = button
        .variants
        .iter()
        .find(|one| one.name == "grants")
        .expect("an answer that grants");
    let on = surface_ground(&theme, panel, "accent.base");
    assert!(matches!(
        consent_broken(&theme, &on, &[], button, grants, &[]),
        Some(Consented::Own(floor, _)) if floor.contains("accent edge")
    ));
    // On the panel's own ground, the same button keeps its edge.
    let own = super::grounds(&theme, panel);
    assert!(consent_broken(&theme, &own, &[], button, grants, &[]).is_none());
    // And the panel falls back.
    let fell = fell_in(&theme);
    assert!(
        fell.iter().any(|one| one.starts_with("panel.root.")),
        "{fell:?}"
    );
}

#[test]
fn a_surface_under_a_consent_answer_is_measured_in_every_tint() {
    // The accent's surface is no ground a button names: a panel filled with it brings a new
    // ground under Allow, which is held in each of the theme's tints and at every whole degree.
    let tinted = load(&json!({
        "name": "Tinted", "extends": "purlis-classic",
        "tint": { "tokens": ["accent.base"] },
        "parts": { "panel.root.fill": "accent.surface" }
    }));
    assert!(!tinted.refused, "{:?}", tinted.complaints);
    let _ = fallen(tinted.theme.first());
    assert!(super::drawn_in() > 1, "measured in every tint");
    // A theme no workspace's colour tints is measured as written.
    let untinted = load(&json!({
        "name": "Plain", "extends": "purlis-classic",
        "tint": { "on": false },
        "parts": { "panel.root.fill": "accent.surface" }
    }));
    assert!(!untinted.refused, "{:?}", untinted.complaints);
    let _ = fallen(untinted.theme.first());
    assert_eq!(super::drawn_in(), 1);
}

/// What `name` breaks on `surface`'s fill `set` in place of its first look's default, read alone.
fn breaks(theme: &ResolvedTheme, name: &str, surface: &str, set: &str) -> Option<String> {
    let surface = family(surface).expect("a surface");
    let default = (surface.defaults)(surface.looks()[0], &[]).fill;
    let rules = super::rules(&theme.parts);
    super::broken_on(
        theme,
        &[],
        std::slice::from_ref(theme),
        &rules,
        family(name).expect("a family"),
        surface,
        default,
        set,
    )
    .map(|(floor, _)| floor)
}

/// A dark Classic whose `surface` is filled with the ground `token`, drawn `grey` (`#4a4a46`:
/// quiet words near 3:1 on it, the primary's near 6.5:1), with the popover's own words in the
/// strongest colour, so what falls is what is drawn on it.
fn filled_grey(surface: &str, token: &str, grey: &str) -> ResolvedTheme {
    shaped(
        json!({ token: grey }),
        json!({
            format!("{surface}.root.fill"): token,
            "popover.root.text": "text.primary",
            "popover.header.text": "text.primary",
            "popover.footer.text": "text.primary",
            "popover.key.text": "text.primary",
        }),
    )
}

#[test]
fn a_badge_on_a_panel_filled_where_its_words_vanish_brings_the_panel_back() {
    // A badge holds no answer, so before #3268 nothing measured it on a panel's fill.
    let theme = filled_grey("panel", "surface.sunken", "#303030");
    let fell = fell_in(&theme);
    assert!(
        fell.iter()
            .any(|one| one.starts_with("panel.root.") && one.contains("what a badge draws on it")),
        "{fell:?}"
    );
}

#[test]
fn a_field_s_help_on_a_popover_filled_where_it_vanishes_brings_the_popover_back() {
    // The field's own grounds name the sunken ground already: only its help, read on grounds of
    // its own (#4648), stands anew on the popover's fill (#3268).
    let theme = filled_grey("popover", "surface.sunken", "#4a4a46");
    let fell = fell_in(&theme);
    assert!(
        fell.iter().any(|one| one.starts_with("popover.root.")
            && one.contains("what a field draws on it: the help's words")),
        "{fell:?}"
    );
}

#[test]
fn a_field_s_help_is_read_on_a_dialog_s_fill_and_never_on_a_pane_s() {
    // A field's help is drawn in a dialog and on a raised ground, never on a pane's own: on a
    // dialog's fill it is read, on a panel's it is drawn nowhere.
    let dialog = filled_grey("dialog", "surface.sunken", "#4a4a46");
    let on_the_dialog = breaks(&dialog, "field", "dialog", "surface.sunken");
    assert!(
        on_the_dialog
            .as_deref()
            .is_some_and(|floor| floor.contains("the help's words")),
        "{on_the_dialog:?}"
    );
    // On a panel's, its error is read (a field that can refuse stands on a pane) and its help,
    // read before it, is not.
    let panel = filled_grey("panel", "surface.sunken", "#4a4a46");
    let on_the_panel = breaks(&panel, "field", "panel", "surface.sunken");
    assert!(
        on_the_panel
            .as_deref()
            .is_some_and(|floor| floor.contains("the error's words")),
        "{on_the_panel:?}"
    );
}

#[test]
fn a_list_s_row_in_a_dialog_is_read_on_the_dialog_s_fill() {
    // A list's rows stand in the palette's dialog (`Palette.tsx`): their quiet words on its fill,
    // a ground the list's own grounds do not name.
    let dialog = family("dialog").expect("the dialog family");
    assert!(
        super::drawn_on(dialog, "surface.overlay", "surface.hover").any(|one| one.name == "list"),
        "a list is drawn on a dialog"
    );
    let theme = filled_grey("dialog", "surface.hover", "#4a4a46");
    let broken = breaks(&theme, "list", "dialog", "surface.hover");
    assert!(broken.is_some(), "{broken:?}");
}

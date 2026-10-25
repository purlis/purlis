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
fn every_surface_a_consent_family_stands_on_is_one_of_its_grounds() {
    for (surface, families) in super::STANDS_ON {
        let surface = family(surface).expect("a surface");
        for variant in surface.looks() {
            let fill = (surface.defaults)(variant, &[]).fill;
            for name in *families {
                let one = family(name).expect("a family");
                assert!(
                    one.variants.iter().any(|variant| variant.consent.is_some()),
                    "{name} has a consent variant"
                );
                assert!(
                    one.grounds.contains(&fill),
                    "{name} on {}: {fill}",
                    surface.name
                );
            }
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

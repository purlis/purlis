// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A title-bar chip's waiting count, as a theme colours it** (#3442): the count is its amber
//! wash over an opaque ground of the bar's own (`titlebar.css`, D-3345-5), and a theme's
//! `titlebar.count.fill` sets the wash, the colour a person sees, on every chip and in every
//! state, while the ground stays the bar's underneath. The floors hold the count's digits on
//! that fill laid over that ground.

use serde_json::{Value, json};

use super::parts;
use super::*;

fn said(resolved: &Resolved) -> Vec<String> {
    resolved
        .complaints
        .iter()
        .map(|one| one.said.clone())
        .collect()
}

fn over(base: &str, parts: Value) -> Value {
    json!({ "name": "Shaped", "extends": base, "parts": parts })
}

const CSS: &str = include_str!("../../../../app/src/ui/titlebar/titlebar.css");

/// The body of the rule `selector` opens in `titlebar.css`.
fn rule(selector: &str) -> &'static str {
    let at = CSS
        .find(&format!("\n{selector} {{"))
        .unwrap_or_else(|| panic!("no rule {selector}"));
    let body = &CSS[at..];
    &body[..body.find('}').unwrap()]
}

#[test]
fn the_count_is_its_wash_over_a_ground_of_the_bar_s_own() {
    // The ground is a wrapper of its own, opaque, in the bar's colour; the wash is the count.
    let ground = rule("[data-ground=\"count\"][data-scope=\"titlebar\"]");
    assert!(
        ground.contains("background-color: var(--surface-deep);"),
        "{ground}"
    );
    let wash = rule("[data-part=\"count\"][data-scope=\"titlebar\"]");
    assert!(
        wash.contains("background-color: var(--wash-warning);"),
        "{wash}"
    );
    assert!(wash.contains("color: var(--state-waiting);"), "{wash}");
}

#[test]
fn a_theme_s_count_fill_sets_the_wash_on_every_chip_in_every_state() {
    let loaded = load(&over(
        "purlis",
        json!({ "titlebar.count.fill": "surface.raised" }),
    ));
    assert!(!loaded.refused, "{:?}", said(&loaded));
    let sheet = parts::stylesheet(loaded.theme.first());
    // Reached as the count inside the chip, whatever the chip's variant and state: the wash
    // itself, never the ground under it.
    assert!(
        sheet.contains(
            ":where([data-scope=\"titlebar\"][data-part=\"workspace\"] [data-scope=\"titlebar\"][data-part=\"count\"]) { background-color: var(--surface-raised); }"
        ),
        "{sheet}"
    );
    assert!(!sheet.contains("data-ground"), "{sheet}");
    assert!(
        parts::fallen(loaded.theme.first()).is_empty(),
        "{:?}",
        parts::fallen(loaded.theme.first())
    );
    // And a theme may set it in one variant and state alone.
    let loaded = load(&over(
        "purlis",
        json!({ "titlebar.count.pinned.selected.fill": "surface.raised" }),
    ));
    let sheet = parts::stylesheet(loaded.theme.first());
    assert!(
        sheet.contains(
            ":where([data-scope=\"titlebar\"][data-part=\"workspace\"][data-variant=\"pinned\"][data-selected] [data-scope=\"titlebar\"][data-part=\"count\"]) { background-color: var(--surface-raised); }"
        ),
        "{sheet}"
    );
}

#[test]
fn a_count_on_its_see_through_wash_is_held_over_the_bar_s_ground() {
    // The bar's own colour reads on the amber as a solid, but not on the amber's see-through wash
    // laid over the bar: measured over that ground, the digits fall and the wash is kept apart.
    for base in ["purlis", "purlis-classic"] {
        let loaded = load(&over(
            base,
            json!({ "titlebar.count.text": "surface.deep" }),
        ));
        assert!(!loaded.refused, "{:?}", said(&loaded));
        for appearance in &loaded.theme.appearances {
            let fell = parts::fallen(appearance);
            assert!(
                fell.iter().any(|one| one.part.starts_with("titlebar.workspace")
                    && one.floor.starts_with("the count's words")),
                "{base} {}: {fell:?}",
                appearance.id
            );
        }
    }
}

#[test]
fn a_faint_glow_the_digits_read_on_over_the_bar_holds() {
    // The opposite way round: amber digits on the amber's faint glow read, since what is under
    // them is mostly the bar, and the theme's fill is drawn.
    let loaded = load(&over(
        "purlis",
        json!({ "titlebar.count.fill": "state.waiting-glow" }),
    ));
    let theme = loaded.theme.first();
    assert!(parts::fallen(theme).is_empty(), "{:?}", parts::fallen(theme));
    assert!(
        parts::stylesheet(theme).contains("background-color: var(--state-waiting-glow);"),
        "{}",
        parts::stylesheet(theme)
    );
}

/// A theme whose count is a see-through wash laid on the bar, its digits only just reading there,
/// with the tint named by `tint`.
fn near_the_floor(tint: Value) -> Resolved {
    load(&json!({
        "name": "Moved", "extends": "purlis-classic",
        "appearances": [{ "id": "dark", "tokens": {
            "surface.deep": "#333333", "accent.wash": "#ff000040", "text.primary": "#a5a5a5"
        } }],
        "tint": tint,
        "parts": { "titlebar.count.fill": "accent.wash", "titlebar.count.text": "text.primary" },
    }))
}

fn count_fell(theme: &ResolvedTheme) -> bool {
    parts::fallen(theme)
        .iter()
        .any(|one| one.floor.starts_with("the count's words"))
}

fn dark(loaded: &Resolved) -> &ResolvedTheme {
    loaded
        .theme
        .appearances
        .iter()
        .find(|one| one.id == "dark")
        .unwrap()
}

#[test]
fn no_built_in_tint_moves_the_ground_under_a_count() {
    for base in ["purlis", "purlis-classic"] {
        let loaded = load(&json!({ "name": "Plain", "extends": base }));
        for appearance in &loaded.theme.appearances {
            assert!(
                !appearance.tint.tabs().contains(&"surface.deep"),
                "{base} {}",
                appearance.id
            );
            for degree in (0..360).step_by(15) {
                let tinted = tint::tinted_at(appearance, f64::from(degree));
                assert_eq!(
                    tinted.values.get("surface.deep"),
                    appearance.values.get("surface.deep"),
                    "{base} {} at {degree}",
                    appearance.id
                );
            }
        }
    }
}

#[test]
fn a_count_that_only_reads_untinted_is_held_in_each_tint_of_a_recipe_that_moves_its_ground() {
    // The digits read over the bar as written, and a tint is what breaks them. A wash keeps its
    // alpha, so the ground turning shows through it, and luminance kept on the ground alone is
    // not luminance kept under the digits.
    let untinted = near_the_floor(json!({ "on": false }));
    assert!(!untinted.refused, "{:?}", said(&untinted));
    assert!(!count_fell(dark(&untinted)));

    let moved = near_the_floor(json!({ "tokens": ["surface.deep"] }));
    assert!(!moved.refused, "{:?}", said(&moved));
    let theme = dark(&moved);
    assert!(
        (0..360).any(|degree| {
            // Measured on the tinted look alone: this is the tint the theme has to hold.
            count_fell(&tint::tinted_at(theme, f64::from(degree)))
        }),
        "no tint breaks the digits"
    );
    assert!(count_fell(theme), "{:?}", parts::fallen(theme));
    // And it falls to the wash the window draws by default, which the token floors hold.
    assert!(
        parts::stylesheet(theme).contains("background-color: var(--wash-warning);"),
        "{}",
        parts::stylesheet(theme)
    );
}

#[test]
fn a_count_on_an_opaque_fill_keeps_its_colours_when_the_ground_moves() {
    // Nothing shows through an opaque fill, so a tint of the bar takes nothing from its digits.
    let loaded = load(&json!({
        "name": "Solid", "extends": "purlis-classic",
        "appearances": [{ "id": "dark", "tokens": {
            "surface.deep": "#333333", "accent.wash": "#202020", "text.primary": "#ffffff"
        } }],
        "tint": { "tokens": ["surface.deep"] },
        "parts": { "titlebar.count.fill": "accent.wash", "titlebar.count.text": "text.primary" },
    }));
    assert!(!loaded.refused, "{:?}", said(&loaded));
    assert!(!count_fell(dark(&loaded)), "{:?}", parts::fallen(dark(&loaded)));
}

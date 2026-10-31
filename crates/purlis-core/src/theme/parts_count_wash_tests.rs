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

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What a see-through fill shows of a tint** (#4257): a tint keeps a token's luminance, but a
//! see-through fill lets the turned colour under it through, so words that read as a theme is
//! written can fall in a tint. These hold every part a theme can colour that way, and say what
//! the default-coloured title-bar count does between the named tints.

use serde_json::{Value, json};

use super::super::{load, tint};
use super::*;

/// The recipe: every ground a family stands on, so the fill has something turned to show.
const RECIPE: &[&str] = &[
    "surface.base",
    "surface.sunken",
    "surface.deep",
    "surface.raised",
    "surface.overlay",
    "surface.hover",
    "control.base",
    "control.hover",
    "tab.active",
    "list.selected",
];

const WASHES: &[&str] = &["#ff000040"];
const HUES: std::ops::RangeInclusive<i32> = 0..=11;

fn themed(wash: &str, grey: u8, parts: &Value) -> Option<ResolvedTheme> {
    let loaded = load(&json!({
        "name": "See-through", "extends": "purlis-classic",
        "appearances": [{ "id": "dark", "tokens": {
            "chart.4": wash, "chart.3": format!("#{grey:02x}{grey:02x}{grey:02x}")
        } }],
        "tint": { "tokens": RECIPE },
        "parts": parts,
    }));
    if loaded.refused {
        return None;
    }
    loaded
        .theme
        .appearances
        .into_iter()
        .find(|one| one.id == "dark")
}

/// `theme` measured as written alone, whatever its recipe.
fn untinted(theme: &ResolvedTheme) -> ResolvedTheme {
    let mut alone = theme.clone();
    alone.tint.on = false;
    alone
}

fn falls(theme: &ResolvedTheme, family: &str) -> bool {
    fallen(theme)
        .iter()
        .any(|one| one.part.starts_with(&format!("{family}.")))
}

/// The dimmest grey that reads on the wash as written, where a dimmer one falls.
fn just_reading(family: &str, wash: &str, parts: &Value) -> Option<u8> {
    let at = |grey| themed(wash, grey, parts).map(|one| falls(&untinted(&one), family));
    if at(0xff)? || !at(0)? {
        return None;
    }
    let (mut low, mut high) = (0_u8, 0xff_u8);
    while high - low > 1 {
        let middle = low + (high - low) / 2;
        if at(middle)? {
            low = middle;
        } else {
            high = middle;
        }
    }
    Some(high)
}

#[test]
fn a_part_that_only_reads_untinted_falls_in_every_family() {
    let mut caught = Vec::new();
    let mut missed = Vec::new();
    for family in MANIFEST {
        let root = family.parts[0];
        for part in family.parts {
            if !part.properties.contains(&Prop::Text) {
                continue;
            }
            let fill_at = if part.properties.contains(&Prop::Fill) {
                part.name
            } else if root.properties.contains(&Prop::Fill) {
                root.name
            } else {
                continue;
            };
            let parts = json!({
                format!("{}.{fill_at}.fill", family.name): "chart.4",
                format!("{}.{}.text", family.name, part.name): "chart.3",
            });
            for wash in WASHES {
                let Some(grey) = just_reading(family.name, wash, &parts) else {
                    continue;
                };
                let theme = themed(wash, grey, &parts).unwrap();
                let breaks = HUES.clone().any(|step| {
                    falls(
                        &untinted(&tint::tinted_at(&theme, f64::from(step * 30))),
                        family.name,
                    )
                });
                if breaks {
                    let name = format!("{}.{}", family.name, part.name);
                    if falls(&theme, family.name) {
                        caught.push(name);
                    } else {
                        missed.push(name);
                    }
                }
            }
        }
    }
    assert!(missed.is_empty(), "not held in a tint: {missed:?}");
    // The sweep found cases to hold: an empty one proves nothing.
    assert!(caught.len() > 20, "{caught:?}");
}

/// The lowest contrast of the count's default colours, over its ground: the pair the token check
/// holds (`check.rs`).
fn default_count(theme: &ResolvedTheme) -> f64 {
    lowest(
        theme,
        &laid(theme, &["surface.deep"]),
        "wash.warning",
        "state.waiting",
    )
    .unwrap()
}

#[test]
fn a_default_count_reads_at_every_degree_of_a_recipe_that_moves_its_ground() {
    for base in ["purlis", "purlis-classic"] {
        let loaded = load(&json!({
            "name": "Moved", "extends": base, "tint": { "tokens": ["surface.deep"] }
        }));
        for appearance in &loaded.theme.appearances {
            for degree in 0..360 {
                let tinted = tint::tinted_at(appearance, f64::from(degree));
                let ratio = default_count(&tinted);
                assert!(
                    ratio >= 4.5,
                    "{base} {} at {degree}: {ratio}",
                    appearance.id
                );
            }
        }
    }
}

#[test]
fn a_shipped_theme_and_an_opaque_count_build_no_tints_for_this() {
    // What pays for the tints is a look the theme changed that draws a see-through colour. Every
    // family asked of the rules as written, at rest.
    let singles = vec![Vec::new()];
    let shows = |theme: &ResolvedTheme| {
        let rules = rules(&theme.parts);
        // The rules' own families: `shows_through` knows a family by its address.
        rules
            .iter()
            .any(|rule| shows_through(theme, &rules, rule.family, &singles))
    };
    for base in ["purlis", "purlis-classic"] {
        let loaded = load(&json!({ "name": "Plain", "extends": base }));
        for appearance in &loaded.theme.appearances {
            assert!(!shows(appearance), "{base} {}", appearance.id);
        }
    }
    let count = |fill: &str| {
        themed(
            fill,
            0xff,
            &json!({ "titlebar.count.fill": "chart.4", "titlebar.count.text": "chart.3" }),
        )
        .unwrap()
    };
    assert!(!shows(&count("#202020")));
    let see_through = count("#ff000040");
    assert!(shows(&see_through), "{:?}", fallen(&see_through));
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What stands on a see-through surface, in a tint** (#4310): a surface's see-through fill lets
//! the turned colour under it through, so another family's words that read on it as written can
//! fall in a tint, before the surface's own do.

use serde_json::{Value, json};

use super::super::{ResolvedTheme, load, tint};
use super::{
    Fell, NEAR_A_FLOOR, coarsely_tinted, drawn, every_tinted, fallen, family, held, lowest,
    shows_a_tint, surface_ground, surfaces_set,
};

/// The recipe: every ground a surface stands on, so its fill has something turned to show.
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

const COLOURS: &[&str] = &[
    "#ff0000", "#00ff00", "#0000ff", "#ff00ff", "#ffaa00", "#00ffff", "#000000", "#ffffff",
    "#808080", "#ff8000",
];

fn themed(appearance: &str, tokens: Value, parts: Value) -> Option<ResolvedTheme> {
    let loaded = load(&json!({
        "name": "See-through surface", "extends": "purlis-classic",
        "appearances": [{ "id": appearance, "tokens": tokens }],
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
        .find(|one| one.id == appearance)
}

fn untinted(theme: &ResolvedTheme) -> ResolvedTheme {
    let mut alone = theme.clone();
    alone.tint.on = false;
    alone
}

/// A surface with no edge of its own to fall first, its own words and the switcher's count in
/// the strongest colour, so what falls is what the buttons draw.
fn on_a_surface(surface: &str, wash: &str) -> ResolvedTheme {
    let own: &[&str] = match surface {
        "popover" => &["root", "header", "footer", "key"],
        "dialog" => &["title", "subtitle", "description", "summary"],
        _ => &["title"],
    };
    let mut parts = json!({
        format!("{surface}.root.fill"): "chart.4",
        format!("{surface}.root.border-width"): 0,
        "titlebar.row-count.text": "text.primary",
    });
    for part in own {
        parts[format!("{surface}.{part}.text")] = json!("text.primary");
    }
    themed("light", json!({ "chart.4": wash }), parts).expect("the theme loads")
}

/// The washes found by a search that read as written, each with a degree they fall at.
const ONLY_IN_A_TINT: &[(&str, &str, i32)] = &[
    ("popover", "#ff000022", 150),
    ("panel", "#00ff008f", 0),
    ("dialog", "#ff000031", 180),
];

const BUTTON_WORDS: &str = "what a button draws on it: the root's words";

fn words_fell(fell: &[Fell]) -> bool {
    fell.iter().any(|one| one.floor.starts_with(BUTTON_WORDS))
}

#[test]
fn a_button_s_words_that_fall_only_in_a_tint_bring_the_surface_back() {
    for (surface, wash, degree) in ONLY_IN_A_TINT {
        let theme = on_a_surface(surface, wash);
        let written = fallen(&untinted(&theme));
        assert!(written.is_empty(), "{surface} {wash}: {written:?}");
        let tinted = fallen(&untinted(&tint::tinted_at(&theme, f64::from(*degree))));
        assert!(
            words_fell(&tinted),
            "{surface} {wash} at {degree}: {tinted:?}"
        );
        let held = fallen(&theme);
        assert!(
            words_fell(&held),
            "{surface} {wash} is read as written alone"
        );
    }
}

#[test]
fn a_surface_whose_words_fall_as_written_is_still_caught_on_the_same_colour() {
    // The control: the same colour, more of it, falls as written, so the washes above are near a
    // floor this reading can reach and not a fill it never reads.
    for (surface, _, _) in ONLY_IN_A_TINT {
        let falling = COLOURS
            .iter()
            .flat_map(|colour| {
                (0x10..0xff_u32)
                    .step_by(8)
                    .map(move |alpha| format!("{colour}{alpha:02x}"))
            })
            .map(|wash| on_a_surface(surface, &wash))
            .find(|theme| words_fell(&fallen(&untinted(theme))))
            .unwrap_or_else(|| panic!("no wash brings {surface}'s button words down"));
        assert!(words_fell(&fallen(&falling)), "{surface}");
    }
}

#[test]
fn the_switcher_s_count_that_falls_only_in_a_tint_brings_the_popover_back() {
    // What stands apart from its root on the popover: the fill, as found by a search.
    let theme = themed(
        "light",
        json!({ "chart.4": "#ff000014" }),
        json!({ "popover.root.fill": "chart.4", "popover.root.border-width": 0 }),
    )
    .expect("the theme loads");
    assert!(fallen(&untinted(&theme)).is_empty());
    let on_it = "what a titlebar draws on it";
    let tinted = fallen(&untinted(&tint::tinted_at(&theme, 120.0)));
    assert!(
        tinted.iter().any(|one| one.floor.starts_with(on_it)),
        "{tinted:?}"
    );
    let held = fallen(&theme);
    assert!(
        held.iter().any(|one| one.floor.starts_with(on_it)),
        "{held:?}"
    );
}

#[test]
fn an_opaque_surface_is_read_as_written_alone() {
    for fill in ["#ffffff", "#f0f0f0"] {
        let theme = on_a_surface("popover", fill);
        assert!(!shows_a_tint(&theme, "chart.4"));
        assert!(fallen(&theme).is_empty(), "{fill}: {:?}", fallen(&theme));
    }
    assert!(shows_a_tint(
        &on_a_surface("popover", "#ff000022"),
        "chart.4"
    ));
}

#[test]
fn a_shipped_theme_builds_no_tints_for_what_stands_on_its_surfaces() {
    for base in ["purlis", "purlis-classic"] {
        let loaded = load(&json!({ "name": "Plain", "extends": base }));
        for appearance in &loaded.theme.appearances {
            let rules = held(appearance).0;
            for (surface, _, _, set) in surfaces_set(&rules) {
                assert!(
                    !shows_a_tint(appearance, set),
                    "{base} {} {}",
                    appearance.id,
                    surface.name
                );
            }
        }
    }
}

#[test]
fn the_coarse_reading_comes_within_its_margin_of_every_degree() {
    // What lets a look clear of a floor at the coarse tints stay unread at the rest (#4257's
    // margin, read here on what stands on a surface).
    let button = family("button").unwrap();
    let mut widest: f64 = 0.0;
    let mut read = 0;
    for surface in ["popover"] {
        let surface_family = family(surface).unwrap();
        for colour in &COLOURS[..3] {
            for alpha in [0x14_u32, 0x60] {
                let theme = on_a_surface(surface, &format!("{colour}{alpha:02x}"));
                for variant in button.looks() {
                    let look = drawn(&[], button, variant, &[]);
                    let at = |one: &ResolvedTheme| {
                        let on = surface_ground(one, surface_family, "chart.4");
                        lowest(one, &on, look.grounds[0], look.words[0])
                    };
                    let least = |all: &[ResolvedTheme]| {
                        all.iter().filter_map(at).fold(f64::INFINITY, f64::min)
                    };
                    let (coarse, fine) = (coarsely_tinted(&theme), every_tinted(&theme));
                    let (coarse, fine) = (least(&coarse), least(&fine));
                    if coarse.is_finite() && fine.is_finite() {
                        widest = widest.max(coarse - fine);
                        read += 1;
                    }
                }
            }
        }
    }
    assert!(read > 20, "{read}");
    assert!(widest < NEAR_A_FLOOR, "{widest}");
}

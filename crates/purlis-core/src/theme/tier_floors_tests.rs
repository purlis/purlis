// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The tier floors** (#2940, ruling #2803·3) at the engine's interface: a theme in, what the
//! window draws and what the check says out.

use serde_json::{Value, json};

use super::super::check::{PAIRS, TIER_GROUNDS, contrast};
use super::super::{
    Finding, ResolvedTheme, built_ins, check, check_every, draw, load, purlis, tint, warnings,
};
use super::{Lifted, hold};

/// Every built-in appearance: Purlis Classic's and Purlis's.
fn every_built_in() -> impl Iterator<Item = &'static ResolvedTheme> {
    built_ins().iter().chain(&purlis().appearances)
}

/// A format 2 theme of one dark appearance that sets `tokens`, as read.
fn dark(tokens: Value) -> ResolvedTheme {
    let resolved = load(&json!({
        "name": "Probe",
        "appearances": [{ "id": "dark", "appearance": "dark", "tokens": tokens }]
    }));
    assert!(!resolved.refused, "{:?}", resolved.complaints);
    resolved.theme.first().clone()
}

/// What the window draws for `theme`.
fn drawn(theme: &ResolvedTheme) -> ResolvedTheme {
    draw(theme).theme.expect("a theme is drawn")
}

/// The least contrast `tier` has on any ground it is held on, in `theme` as it is.
fn least_on_grounds(theme: &ResolvedTheme, tier: &str) -> f64 {
    TIER_GROUNDS
        .iter()
        .map(|ground| contrast(theme.value(tier), theme.value(ground)))
        .fold(f64::INFINITY, f64::min)
}

/// What the check says it lifted.
fn lifted_in(theme: &ResolvedTheme) -> Vec<(&'static str, String, String)> {
    check(theme)
        .findings
        .into_iter()
        .filter_map(|one| match one {
            Finding::TierFallsBack {
                tier,
                written,
                drawn,
                ..
            } => Some((tier, written, drawn)),
            _ => None,
        })
        .collect()
}

// ---------------------------------------------------------------------------------------
// The built-in themes are unchanged
// ---------------------------------------------------------------------------------------

#[test]
fn every_built_in_appearance_is_drawn_with_its_own_tiers() {
    for theme in every_built_in() {
        let (held, lifted) = hold(theme);
        assert_eq!(
            lifted,
            Vec::<Lifted>::new(),
            "{} ({})",
            theme.name,
            theme.id
        );
        assert_eq!(held.values, theme.values, "{} ({})", theme.name, theme.id);
        for tier in [
            "text.primary",
            "text.secondary",
            "text.muted",
            "text.quiet",
            "text.faint",
        ] {
            assert_eq!(
                drawn(theme).value(tier),
                theme.value(tier),
                "{}: {tier}",
                theme.id
            );
        }
    }
}

#[test]
fn the_check_names_no_fallback_for_a_built_in_in_any_combination() {
    for theme in [super::super::classic(), purlis()] {
        for one in check_every(theme) {
            assert!(
                !one.report
                    .findings
                    .iter()
                    .any(|finding| matches!(finding, Finding::TierFallsBack { .. })),
                "{}: {:?}",
                one.appearance,
                one.report.findings
            );
        }
    }
}

// ---------------------------------------------------------------------------------------
// A third-party tier under its floor falls back
// ---------------------------------------------------------------------------------------

#[test]
fn a_quiet_tier_under_its_floor_is_drawn_as_the_nearest_grey_that_clears_it() {
    // Figma's quiet grey before the ruling lifted it: 4.10 on the row under the pointer.
    let theme = dark(json!({ "surface.hover": "#2a2a31", "text.quiet": "#8a8896" }));
    let window = drawn(&theme);
    let quiet = window.value("text.quiet");
    assert_ne!(quiet, "#8a8896");
    assert!(least_on_grounds(&window, "text.quiet") >= 4.5, "{quiet}");
    // Lifted toward the light, on a dark theme, and kept the grey's own hue.
    let (Some(was), Some(now)) = (tint::oklab_of("#8a8896"), tint::oklab_of(quiet)) else {
        panic!("colours");
    };
    assert!(now[0] > was[0], "{quiet}");
    let hue = |lab: [f64; 3]| lab[2].atan2(lab[1]);
    assert!((hue(now) - hue(was)).abs() < 0.1, "{quiet}");
    // The nearest: a step of lightness back toward the written grey no longer clears.
    let back = tint::at_lightness(quiet, now[0] - 0.004).expect("a colour");
    let mut nearer = window.clone();
    nearer.values.insert("text.quiet", back.clone());
    assert!(least_on_grounds(&nearer, "text.quiet") < 4.5, "{back}");
}

#[test]
fn the_rest_of_the_theme_draws_as_written() {
    let theme = dark(json!({ "surface.hover": "#2a2a31", "text.quiet": "#8a8896" }));
    let window = drawn(&theme);
    for (token, value) in &theme.values {
        if *token != "text.quiet" {
            assert_eq!(window.value(token), value, "{token}");
        }
    }
}

#[test]
fn a_faint_mark_is_held_to_the_mark_floor() {
    let theme = dark(json!({ "list.selected": "#2a2936", "text.faint": "#6f6d7a" }));
    let window = drawn(&theme);
    let faint = window.value("text.faint");
    assert!(least_on_grounds(&window, "text.faint") >= 3.0, "{faint}");
    // A mark, not words: lifted to its own floor and no further.
    assert!(least_on_grounds(&window, "text.faint") < 4.5, "{faint}");
}

#[test]
fn a_tier_that_clears_its_floor_is_never_moved() {
    let theme = dark(json!({
        "text.muted": "#a0a0a8",
        "text.quiet": "#989aa0",
        "text.faint": "#78787f",
    }));
    let (held, lifted) = hold(&theme);
    assert!(lifted.is_empty(), "{lifted:?}");
    assert_eq!(held.values, theme.values);
}

#[test]
fn a_dark_tier_on_a_light_theme_is_drawn_darker() {
    let resolved = load(&json!({
        "name": "Paper",
        "appearances": [{ "id": "light", "appearance": "light", "tokens": {
            "text.quiet": "#a8a8a8"
        } }]
    }));
    let theme = resolved.theme.first().clone();
    let window = drawn(&theme);
    let quiet = window.value("text.quiet");
    assert!(least_on_grounds(&window, "text.quiet") >= 4.5, "{quiet}");
    let (Some(was), Some(now)) = (tint::oklab_of("#a8a8a8"), tint::oklab_of(quiet)) else {
        panic!("colours");
    };
    assert!(now[0] < was[0], "{quiet}");
}

#[test]
fn every_text_tier_is_held_primary_included() {
    let theme = dark(json!({ "text.secondary": "#55555c", "text.primary": "#6a6a70" }));
    let window = drawn(&theme);
    for tier in ["text.primary", "text.secondary"] {
        assert!(
            least_on_grounds(&window, tier) >= 4.5,
            "{tier}: {}",
            window.value(tier)
        );
    }
}

#[test]
fn a_see_through_tier_keeps_its_alpha_when_that_is_enough() {
    let theme = dark(json!({ "text.quiet": "#909090e0" }));
    let window = drawn(&theme);
    let quiet = window.value("text.quiet");
    assert_eq!(quiet.len(), 9, "{quiet}");
    assert!(quiet.ends_with("e0"), "{quiet}");
    let least = TIER_GROUNDS
        .iter()
        .map(|ground| {
            let back = window.value(ground);
            contrast(&super::super::check::onto(quiet, back), back)
        })
        .fold(f64::INFINITY, f64::min);
    assert!(least >= 4.5, "{quiet}: {least}");
}

#[test]
fn the_colour_drawn_clears_the_floor_in_every_workspace_tint() {
    // A quiet grey that clears every ground as written, on a selected row a tint can move.
    let theme = dark(json!({ "text.quiet": "#8f8e9b" }));
    let window = drawn(&theme);
    for one in &window.tint.hues {
        let tinted = super::super::tint::tinted(&window, one.name);
        for pair in PAIRS
            .iter()
            .filter(|pair| pair.front == "text.quiet" && TIER_GROUNDS.contains(&pair.back))
        {
            let ratio = super::super::check::ratio(&tinted, pair).unwrap_or(f64::INFINITY);
            assert!(
                ratio >= pair.floor,
                "{} on {}: {ratio}",
                one.name,
                pair.back
            );
        }
    }
}

// ---------------------------------------------------------------------------------------
// The check names the fallback
// ---------------------------------------------------------------------------------------

#[test]
fn the_check_names_the_tier_it_lifts_and_the_colour_drawn() {
    let theme = dark(json!({ "surface.hover": "#2a2a31", "text.quiet": "#8a8896" }));
    let window = drawn(&theme);
    assert_eq!(
        lifted_in(&theme),
        vec![(
            "text.quiet",
            "#8a8896".to_owned(),
            window.value("text.quiet").to_owned()
        )]
    );
}

#[test]
fn the_check_says_the_fallback_in_words() {
    let theme = load(&json!({
        "name": "Probe",
        "appearances": [{ "id": "dark", "appearance": "dark", "tokens": {
            "surface.hover": "#2a2a31", "text.quiet": "#8a8896"
        } }]
    }))
    .theme;
    let lines = warnings::said(&theme, &check_every(&theme));
    let drawn = drawn(theme.first());
    let quiet = drawn.value("text.quiet");
    assert!(
        lines.iter().any(|line| line
            == &format!(
                "text.quiet is drawn as {quiet}, not #8a8896: 3.92 on control.hover, under 4.5, \
                 so purlis draws the nearest colour that clears it"
            )),
        "{lines:#?}"
    );
}

// ---------------------------------------------------------------------------------------
// Where no lightness clears every ground
// ---------------------------------------------------------------------------------------

/// Purlis Classic dark with a light code ground: no grey clears both it and the dark surfaces.
fn light_code_ground() -> ResolvedTheme {
    dark(json!({ "terminal.background": "#f4f4f0" }))
}

#[test]
fn a_tier_that_clears_the_window_s_own_surfaces_keeps_its_colour() {
    let theme = light_code_ground();
    let window = drawn(&theme);
    for tier in ["text.primary", "text.secondary", "text.muted", "text.quiet"] {
        assert_eq!(window.value(tier), theme.value(tier), "{tier}");
    }
    // A mark's floor has a grey that clears both sides, so faint is walked to it.
    assert!(least_on_grounds(&window, "text.faint") >= 3.0);
}

#[test]
fn the_tiers_stay_apart_on_a_theme_with_a_ground_on_the_other_side() {
    let window = drawn(&light_code_ground());
    let on_window = |tier: &str| contrast(window.value(tier), window.value("surface.base"));
    assert!(on_window("text.primary") > on_window("text.muted") + 2.0);
}

#[test]
fn a_tier_off_the_window_s_own_surfaces_is_walked_on_them_only() {
    // Quiet under its floor on the row under the pointer, and a light code ground no grey clears
    // with the dark surfaces: quiet is lifted on the dark surfaces, and the code ground is named.
    let theme = dark(json!({
        "terminal.background": "#f4f4f0",
        "surface.hover": "#2a2a31",
        "text.quiet": "#8a8896",
    }));
    let window = drawn(&theme);
    let quiet = window.value("text.quiet");
    assert_ne!(quiet, "#8a8896");
    let dark_grounds: Vec<&str> = TIER_GROUNDS
        .into_iter()
        .filter(|ground| *ground != "terminal.background")
        .collect();
    let least = dark_grounds
        .iter()
        .map(|ground| contrast(quiet, window.value(ground)))
        .fold(f64::INFINITY, f64::min);
    assert!(least >= 4.5, "{quiet}: {least}");
    // Never the mid grey that would read worse on the window.
    assert!(
        contrast(quiet, window.value("surface.base"))
            >= contrast("#8a8896", window.value("surface.base"))
    );
}

#[test]
fn the_check_names_the_ground_off_the_window_s_own_surfaces() {
    let theme = light_code_ground();
    let named: Vec<(&str, &str, bool)> = check(&theme)
        .findings
        .iter()
        .filter_map(|one| match one {
            Finding::TierFallsBack {
                tier,
                ground,
                clears,
                ..
            } => Some((*tier, *ground, *clears)),
            _ => None,
        })
        .collect();
    assert!(
        named.contains(&("text.quiet", "terminal.background", false)),
        "{named:?}"
    );
    let resolved = load(&json!({
        "name": "Probe",
        "appearances": [{ "id": "dark", "appearance": "dark", "tokens": {
            "terminal.background": "#f4f4f0"
        } }]
    }))
    .theme;
    let lines = warnings::said(&resolved, &check_every(&resolved));
    assert!(
        lines.iter().any(|line| line.starts_with(
            "text.quiet is drawn as written: no colour clears every ground, so purlis holds it \
             on the window's own surfaces; it is"
        ) && line.contains("on terminal.background")),
        "{lines:#?}"
    );
}

// ---------------------------------------------------------------------------------------
// The pane's header line, as drawn (#3637)
// ---------------------------------------------------------------------------------------

/// **A pane header's words keep a margin over the text floor** (#3637): the breadcrumb, where a
/// chat came from, its work item and its gauge are drawn in the secondary, muted and quiet tiers
/// on the terminal's fill, solid, so the quiet tier carries the quietness. At 0.92 opacity
/// Purlis light's quiet words come to 4.35:1 there (5.12:1 solid), under the floor; the
/// window's half of this, the stories' faded-words gate (`app/.storybook/faded.setup.ts`,
/// #3790), fails on any fade that takes words under it, wherever the opacity is set.
///
/// **The margin, a twentieth, is chosen headroom, not a measured tolerance**: nothing in the
/// engine produces it. It keeps every solid tier at least 4.725:1 on the pane's fill in every
/// built-in appearance, so retuning the quiet tier cannot drift it back onto the floor; the
/// tightest today are Purlis light's and Purlis dark's quiet words, at 5.12:1 and 5.15:1.
#[test]
fn a_pane_headers_words_keep_a_margin_over_the_text_floor_in_every_built_in() {
    let floor = PAIRS
        .iter()
        .find(|pair| pair.front == "text.quiet" && pair.back == "terminal.background")
        .expect("the quiet words are held on the terminal's fill")
        .floor;
    for theme in every_built_in() {
        let theme = drawn(theme);
        for tier in ["text.primary", "text.secondary", "text.muted", "text.quiet"] {
            let ratio = contrast(theme.value(tier), theme.value("terminal.background"));
            assert!(
                ratio >= floor * 1.05,
                "{} ({}): {tier} on terminal.background is {ratio:.2}:1, within a twentieth of \
                 {floor}:1",
                theme.name,
                theme.id
            );
        }
    }
}

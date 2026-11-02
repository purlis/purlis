// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The accent's grounds follow the accent** (#4666): a chosen item says its words in
//! `accent.text` on `accent.wash` or `accent.surface`, so where the words go the ground goes. An
//! accent given over a parent turns the parent's grounds to its hue, and a workspace's colour
//! turns the washes exactly where it turns the words, held apart from the waiting and danger
//! washes.

use serde_json::json;

use super::check::{contrast, onto};
use super::person::Person;
use super::tint::{self, oklab_of};
use super::vocabulary::{ACCENT_GROUNDS, ACCENT_WASHES, ACCENT_WORDS, TINTED};
use super::*;

/// A person's teal, the one #3100's tests give.
const TEAL: &str = "#2bb3a3";

/// A teal dark enough for a light window's words.
const DARK_TEAL: &str = "#0b6359";

/// The teal a person would give `theme`: one its window can read.
fn teal_for(theme: &ResolvedTheme) -> &'static str {
    let light = tint::luminance_of(theme.value("surface.base")).unwrap() > 0.5;
    if light { DARK_TEAL } else { TEAL }
}

/// Every appearance purlis ships: Purlis's three and Classic's two.
fn shipped() -> Vec<&'static ResolvedTheme> {
    purlis().appearances.iter().chain(built_ins()).collect()
}

/// A colour's OKLCH hue in degrees and its chroma.
fn hue_chroma(hex: &str) -> (f64, f64) {
    let [_, a, b] = oklab_of(hex).expect("a colour");
    (b.atan2(a).to_degrees().rem_euclid(360.0), a.hypot(b))
}

/// How far two hues are, the short way round.
fn degrees_apart(a: f64, b: f64) -> f64 {
    let d = (a - b).rem_euclid(360.0);
    d.min(360.0 - d)
}

/// The alpha a value is written with, `ff` for an opaque one.
fn alpha(hex: &str) -> &str {
    if hex.len() == 9 { &hex[7..] } else { "ff" }
}

/// `accent.text` on each ground it is drawn on, as the check measures it.
fn words_on_grounds(theme: &ResolvedTheme) -> Vec<(String, f64)> {
    let words = theme.value(ACCENT_WORDS);
    let mut out = vec![(
        "accent.surface".to_owned(),
        contrast(words, theme.value("accent.surface")),
    )];
    for wash in ACCENT_WASHES {
        for under in ["surface.base", "surface.deep"] {
            let ground = onto(theme.value(wash), theme.value(under));
            out.push((format!("{wash} over {under}"), contrast(words, &ground)));
        }
    }
    out
}

fn over_purlis(id: &str, tokens: serde_json::Value) -> ResolvedTheme {
    let loaded = load(&json!({
        "name": "Teal over Purlis", "extends": "purlis",
        "appearances": [{ "id": id, "tokens": tokens }],
    }));
    assert!(!loaded.refused, "{:?}", loaded.complaints);
    loaded
        .theme
        .appearances
        .into_iter()
        .find(|one| one.id == id)
        .expect("the appearance")
}

// ---------------------------------------------------------------------------------------
// An accent given over a parent
// ---------------------------------------------------------------------------------------

#[test]
fn an_accent_given_over_purlis_turns_its_washes_and_surface_to_its_hue() {
    let (teal, _) = hue_chroma(TEAL);
    let dark = purlis().first();
    let theme = over_purlis(&dark.id, json!({ "accent.base": TEAL }));
    assert_eq!(theme.value(ACCENT_WORDS), TEAL);
    for ground in ACCENT_GROUNDS {
        let (was, now) = (dark.value(ground), theme.value(ground));
        let (hue, _) = hue_chroma(now);
        assert!(degrees_apart(hue, teal) < 6.0, "{ground}: {now} at {hue}");
        // The ground keeps how light it was and how much it lets through.
        assert_eq!(alpha(now), alpha(was), "{ground}");
        let (l_was, l_now) = (
            tint::luminance_of(was).unwrap(),
            tint::luminance_of(now).unwrap(),
        );
        assert!((l_was - l_now).abs() < 0.005, "{ground}: {was} → {now}");
    }
}

#[test]
fn a_ground_the_theme_gives_is_its_own() {
    let dark = purlis().first();
    let theme = over_purlis(
        &dark.id,
        json!({ "accent.base": TEAL, "accent.wash": "#11223344" }),
    );
    assert_eq!(theme.value("accent.wash"), "#11223344");
    // The others still follow.
    assert_ne!(theme.value("accent.surface"), dark.value("accent.surface"));
}

#[test]
fn the_accent_as_its_parent_draws_it_leaves_the_grounds_as_they_are() {
    let dark = purlis().first();
    let theme = over_purlis(
        &dark.id,
        json!({ "accent.base": dark.value("accent.base") }),
    );
    for ground in ACCENT_GROUNDS {
        assert_eq!(theme.value(ground), dark.value(ground), "{ground}");
    }
}

#[test]
fn a_grey_accent_draws_grey_grounds() {
    let dark = purlis().first();
    let theme = over_purlis(&dark.id, json!({ "accent.base": "#9a9a9a" }));
    for ground in ACCENT_GROUNDS {
        let (_, chroma) = hue_chroma(theme.value(ground));
        assert!(chroma < 0.005, "{ground}: {}", theme.value(ground));
    }
}

#[test]
fn a_person_s_accent_turns_the_grounds_under_its_words_in_every_shipped_appearance() {
    for theme in shipped() {
        let given = teal_for(theme);
        let (person, said) = Person::read(Some(&json!({ "colours": { "accent.base": given } })));
        assert!(said.is_empty(), "{said:?}");
        let (teal, _) = hue_chroma(given);
        let (drawn, said) = person.layer().over(theme);
        let id = format!("{} {}", theme.name, theme.id);
        assert!(said.is_empty(), "{id}: {said:?}");
        assert_eq!(drawn.value(ACCENT_WORDS), given, "{id}");
        for ground in ACCENT_GROUNDS {
            let (hue, _) = hue_chroma(drawn.value(ground));
            assert!(degrees_apart(hue, teal) < 6.0, "{id} {ground}");
        }
    }
}

#[test]
fn a_person_s_accent_left_out_takes_its_grounds_with_it() {
    // An accent as dark as Purlis's ground: its words would vanish, so it is left out.
    let (person, _) = Person::read(Some(&json!({ "colours": { "accent.base": "#1a1a1f" } })));
    let dark = purlis().first();
    let (drawn, said) = person.layer().over(dark);
    assert!(!said.is_empty());
    assert!(
        said.iter().all(|one| !one.said.contains("accent.wash")),
        "{said:?}"
    );
    for ground in ACCENT_GROUNDS {
        assert_eq!(drawn.value(ground), dark.value(ground), "{ground}");
    }
}

// ---------------------------------------------------------------------------------------
// A workspace's colour
// ---------------------------------------------------------------------------------------

#[test]
fn a_workspace_s_colour_turns_the_washes_by_default() {
    for wash in ACCENT_WASHES {
        assert!(TINTED.contains(&wash), "{wash}");
    }
}

#[test]
fn the_washes_turn_exactly_where_the_words_do() {
    for theme in shipped() {
        for degree in 0..360 {
            let most = tint::most_at(theme, f64::from(degree));
            for wash in ACCENT_WASHES {
                assert_eq!(
                    tint::chroma_of(wash, most),
                    tint::chroma_of(ACCENT_WORDS, most),
                    "{} {} at {degree}: {wash}",
                    theme.name,
                    theme.id
                );
            }
        }
    }
}

#[test]
fn the_words_hold_four_and_a_half_to_one_on_their_grounds_at_every_hue() {
    for theme in shipped() {
        for degree in 0..360 {
            let tinted = tint::tinted_at(theme, f64::from(degree));
            for (ground, ratio) in words_on_grounds(&tinted) {
                assert!(
                    ratio >= 4.5,
                    "{} {} at {degree}: accent.text on {ground} is {ratio}",
                    theme.name,
                    theme.id
                );
            }
        }
    }
}

#[test]
fn a_turned_wash_stays_apart_from_the_waiting_and_danger_washes() {
    for theme in shipped() {
        for degree in 0..360 {
            let tinted = tint::tinted_at(theme, f64::from(degree));
            for wash in ACCENT_WASHES {
                if tinted.value(wash) == theme.value(wash) {
                    continue;
                }
                for from in ["wash.warning", "danger.wash"] {
                    let apart = oklab_distance(tinted.value(wash), tinted.value(from));
                    assert!(
                        apart >= 0.1,
                        "{} {} at {degree}: {wash} is {apart} from {from}",
                        theme.name,
                        theme.id
                    );
                }
            }
        }
    }
}

#[test]
fn a_theme_whose_wash_meets_the_waiting_wash_under_a_tint_keeps_wash_and_words_as_written() {
    // The wash drawn as the waiting wash turned half way round: apart as written, the waiting
    // wash itself under a workspace of the waiting wash's hue.
    let dark = purlis().first();
    let warning = dark.value("wash.warning");
    let (amber, _) = hue_chroma(warning);
    let wash = tint::tint_hex(warning, amber + 180.0);
    let theme = over_purlis(
        &dark.id,
        json!({ "accent.wash": wash, "accent.wash-hover": wash }),
    );
    assert!(oklab_distance(&wash, warning) >= 0.1);
    assert!(oklab_distance(&tint::tint_hex(&wash, amber), warning) < 0.1);
    let tinted = tint::tinted_at(&theme, amber);
    assert_eq!(tinted.value("accent.wash"), theme.value("accent.wash"));
    // The words stand on it, so they keep the theme's colour with it.
    assert_eq!(tinted.value(ACCENT_WORDS), theme.value(ACCENT_WORDS));
}

#[test]
fn a_recipe_that_turns_the_accent_turns_its_washes_and_leaving_the_accent_leaves_them() {
    let loaded = load(&json!({
        "name": "Accent only", "extends": "purlis",
        "tint": { "tokens": ["accent.base"] },
    }));
    assert!(!loaded.refused, "{:?}", loaded.complaints);
    let tabs = loaded.theme.first().tint.tabs();
    for token in [
        "accent.base",
        ACCENT_WORDS,
        "accent.wash",
        "accent.wash-hover",
    ] {
        assert!(tabs.contains(&token), "{token}: {tabs:?}");
    }
    let mut recipe = tint::Tint::default();
    let left = recipe.leave(&["accent.base"]);
    for token in [
        "accent.base",
        ACCENT_WORDS,
        "accent.wash",
        "accent.wash-hover",
    ] {
        assert!(left.contains(&token), "{token}: {left:?}");
        assert!(!recipe.tabs().contains(&token), "{token}");
    }
}

fn oklab_distance(a: &str, b: &str) -> f64 {
    let (a, b) = (oklab_of(a).unwrap(), oklab_of(b).unwrap());
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A pair that falls only between the named tints** (#4308): a workspace's own colour can be any
//! hue, so the check reads each pair a tint can move at every whole degree. A control falls as
//! written, so a sweep that reads nothing cannot pass.

use serde_json::{Value, json};

use super::super::parts::{COARSE_DEGREES, NEAR_A_FLOOR};
use super::super::vocabulary::TINTABLE;
use super::super::{
    ResolvedTheme, Theme, built_ins, check, check_every, load, tint, warnings_said,
};
use super::{Finding, PAIRS, Pair, floors, ratio};

/// The count's digits on the amber wash, over the title bar.
fn count() -> &'static Pair {
    pair("state.waiting", "wash.warning", Some("surface.deep"), 4.5)
}

fn pair(front: &str, back: &str, under: Option<&str>, floor: f64) -> &'static Pair {
    PAIRS
        .iter()
        .find(|one| {
            one.front == front && one.back == back && one.under == under && one.floor == floor
        })
        .unwrap()
}

/// `tokens` on Purlis Classic's dark, tinted by `recipe`.
fn whole(tokens: &Value, recipe: &[&str]) -> Theme {
    let loaded = load(&json!({
        "name": "Between", "extends": "purlis-classic",
        "appearances": [{ "id": "dark", "tokens": tokens }],
        "tint": { "tokens": recipe },
    }));
    assert!(!loaded.refused);
    loaded.theme
}

/// The dark appearance of [`whole`].
fn themed(tokens: &Value, recipe: &[&str]) -> ResolvedTheme {
    whole(tokens, recipe)
        .appearances
        .into_iter()
        .find(|one| one.id == "dark")
        .unwrap()
}

fn lowest_named(theme: &ResolvedTheme, pair: &Pair) -> f64 {
    theme
        .tint
        .hues
        .iter()
        .filter_map(|one| ratio(&tint::tinted(theme, one.name), pair))
        .fold(f64::INFINITY, f64::min)
}

fn lowest_at_any_degree(theme: &ResolvedTheme, pair: &Pair) -> f64 {
    (0..360)
        .filter_map(|degree| ratio(&tint::tinted_at(theme, f64::from(degree)), pair))
        .fold(f64::INFINITY, f64::min)
}

/// What the check says of `pair`: (found as written or at a named tint, found between them).
fn said_of(theme: &ResolvedTheme, pair: &Pair) -> (bool, Option<f64>) {
    let findings = check(theme).findings;
    let named = findings
        .iter()
        .any(|one| matches!(one, Finding::Contrast { pair: said, .. } if said == pair));
    let between = findings.iter().find_map(|one| match one {
        Finding::ContrastBetweenTints {
            pair: said, ratio, ..
        } if said == pair => Some(*ratio),
        _ => None,
    });
    (named, between)
}

/// The count in `grey`, on a wash found by a search: its pair reads as written and at each named
/// tint at grey `8e`, and first falls between them.
fn count_in(grey: &str) -> Value {
    json!({ "wash.warning": "#d2992224", "state.waiting": grey })
}

const RECIPE: [&str; 1] = ["surface.deep"];

#[test]
fn a_default_coloured_pair_that_falls_only_between_named_tints_is_found_at_its_lowest() {
    let theme = themed(&count_in("#8e8e8e"), &RECIPE);
    assert!(ratio(&theme, count()).unwrap() >= count().floor);
    assert!(lowest_named(&theme, count()) >= count().floor);
    let (named, between) = said_of(&theme, count());
    assert!(!named);
    assert_eq!(between, Some(lowest_at_any_degree(&theme, count())));
    assert!(between.unwrap() < count().floor);

    let written = whole(&count_in("#8e8e8e"), &RECIPE);
    let said = warnings_said(&written, &check_every(&written));
    assert!(
        said.iter().any(|line| line.contains(
            "state.waiting on wash.warning over surface.deep, in a workspace's own colour at"
        )),
        "{said:?}"
    );
}

#[test]
fn a_pair_that_falls_as_written_is_said_as_it_was_and_not_again_between_the_tints() {
    // The control: without it, a sweep that read nothing could pass the case above's negative.
    let theme = themed(&count_in("#808080"), &RECIPE);
    assert!(ratio(&theme, count()).unwrap() < count().floor);
    assert_eq!(said_of(&theme, count()), (true, None));
}

#[test]
fn a_pair_far_from_its_floor_at_every_degree_is_not_said() {
    let theme = themed(&count_in("#ffffff"), &RECIPE);
    assert_eq!(said_of(&theme, count()), (false, None));
}

/// Under the widest recipe, on both families.
fn widest() -> Vec<ResolvedTheme> {
    ["purlis-classic", "purlis"]
        .iter()
        .flat_map(|id| {
            load(&json!({ "name": "Widest", "extends": id, "tint": { "tokens": TINTABLE } }))
                .theme
                .appearances
        })
        .collect()
}

#[test]
fn a_pair_the_check_leaves_unread_does_not_fall_at_any_degree() {
    // What lets the sweep skip them.
    for theme in widest() {
        let skipped: Vec<&Pair> = PAIRS
            .iter()
            .filter(|pair| !floors::moves_with_hue(&theme, pair))
            .collect();
        assert!(!skipped.is_empty());
        let tints: Vec<ResolvedTheme> = (0..360)
            .map(|degree| tint::tinted_at(&theme, f64::from(degree)))
            .collect();
        for pair in skipped {
            let Some(written) = ratio(&theme, pair) else {
                continue;
            };
            for (degree, drawn) in tints.iter().enumerate() {
                let at = ratio(drawn, pair).unwrap();
                assert!(
                    at >= written.min(pair.floor),
                    "{} {pair:?} at {degree}: {at} from {written}",
                    theme.id
                );
            }
        }
    }
}

#[test]
fn a_coarse_reading_comes_within_the_margin_of_the_lowest_at_any_degree() {
    // What lets the sweep leave a pair at its coarse reading: the largest gap measured is 0.05.
    let mut largest: f64 = 0.0;
    for theme in widest() {
        let tints: Vec<ResolvedTheme> = (0..360)
            .map(|degree| tint::tinted_at(&theme, f64::from(degree)))
            .collect();
        for pair in PAIRS
            .iter()
            .filter(|pair| floors::moves_with_hue(&theme, pair))
        {
            let lowest = |step: usize| {
                tints
                    .iter()
                    .step_by(step)
                    .filter_map(|drawn| ratio(drawn, pair))
                    .fold(f64::INFINITY, f64::min)
            };
            if lowest(1).is_finite() {
                largest = largest.max(lowest(COARSE_DEGREES) - lowest(1));
            }
        }
    }
    assert!(largest > 0.0);
    assert!(largest < NEAR_A_FLOOR, "{largest}");
}

#[test]
fn no_built_in_falls_between_its_named_tints() {
    for theme in built_ins() {
        let between: Vec<Finding> = check(theme)
            .findings
            .into_iter()
            .filter(|one| matches!(one, Finding::ContrastBetweenTints { .. }))
            .collect();
        assert!(between.is_empty(), "{} {between:?}", theme.id);
    }
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A pair that falls only between the named tints** (#4308): a workspace's own colour can be any
//! hue, so the check reads each pair a tint can move at every whole degree. Each case carries a
//! control that falls as written, so a sweep that reads nothing cannot pass.

use serde_json::{Value, json};

use super::super::vocabulary::TINTABLE;
use super::super::{ResolvedTheme, Theme, built_ins, check, check_every, load, tint, warnings_said};
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

/// The dimmest of `token`'s greys that reads as written and at each named tint and falls at a
/// degree between them, and the grey dimmer that falls as written.
fn just_falling(token: &str, also: &Value, recipe: &[&str], pair: &Pair) -> (u8, u8) {
    let tokens = |grey: u8| {
        let mut tokens = also.clone();
        tokens[token] = json!(format!("#{grey:02x}{grey:02x}{grey:02x}"));
        tokens
    };
    let reads = |grey: u8| {
        let theme = themed(&tokens(grey), recipe);
        ratio(&theme, pair).unwrap() >= pair.floor && lowest_named(&theme, pair) >= pair.floor
    };
    let grey = (0x40..=0xff_u8)
        .filter(|grey| reads(*grey))
        .find(|grey| lowest_at_any_degree(&themed(&tokens(*grey), recipe), pair) < pair.floor)
        .expect("a grey in the band between the named tints and the worst degree");
    let falls = (0x40..grey)
        .rev()
        .find(|grey| ratio(&themed(&tokens(*grey), recipe), pair).unwrap() < pair.floor)
        .expect("a grey that falls as written");
    (grey, falls)
}

#[test]
fn a_default_coloured_pair_that_falls_only_between_named_tints_is_found_at_its_lowest() {
    let recipe = ["surface.deep"];
    let also = json!({});
    let (grey, control) = just_falling("state.waiting", &also, &recipe, count());
    let tokens = |grey: u8| {
        let mut tokens = also.clone();
        tokens["state.waiting"] = json!(format!("#{grey:02x}{grey:02x}{grey:02x}"));
        tokens
    };

    let theme = themed(&tokens(grey), &recipe);
    let (named, between) = said_of(&theme, count());
    assert!(!named);
    assert_eq!(between, Some(lowest_at_any_degree(&theme, count())));
    assert!(between.unwrap() < count().floor);
    let written = whole(&tokens(grey), &recipe);
    let said = warnings_said(&written, &check_every(&written));
    assert!(
        said.iter()
            .any(|line| line.contains("in a workspace's own colour at")),
        "{said:?}"
    );

    // The control: it falls as written, is said as it always was, and is not said twice.
    let (named, between) = said_of(&themed(&tokens(control), &recipe), count());
    assert!(named);
    assert_eq!(between, None);
}

#[test]
fn words_standing_in_for_a_platform_selection_are_read_between_the_named_tints() {
    let recipe = ["text.primary"];
    let selection = pair(
        "selection.text",
        "selection.background",
        Some("surface.base"),
        4.5,
    );
    let also = json!({ "selection.text": "platform", "selection.background": "#3a7bd580" });
    let (grey, _) = just_falling("text.primary", &also, &recipe, selection);
    let mut tokens = also;
    tokens["text.primary"] = json!(format!("#{grey:02x}{grey:02x}{grey:02x}"));
    let (named, between) = said_of(&themed(&tokens, &recipe), selection);
    assert!(!named);
    assert!(between.is_some());
}

#[test]
fn a_pair_the_check_leaves_unread_does_not_fall_at_any_degree() {
    // What lets the sweep skip them: read here at every degree, under the widest recipe.
    for id in ["purlis-classic", "purlis"] {
        let loaded = load(&json!({
            "name": "Widest", "extends": id, "tint": { "tokens": TINTABLE }
        }));
        for theme in &loaded.theme.appearances {
            let skipped: Vec<&Pair> = PAIRS
                .iter()
                .filter(|pair| !floors::moves_with_hue(theme, pair))
                .filter(|pair| pair.front != "selection.text")
                .collect();
            assert!(!skipped.is_empty());
            let tints: Vec<ResolvedTheme> = (0..360)
                .map(|degree| tint::tinted_at(theme, f64::from(degree)))
                .collect();
            for pair in skipped {
                let Some(written) = ratio(theme, pair) else {
                    continue;
                };
                for (degree, drawn) in tints.iter().enumerate() {
                    let at = ratio(drawn, pair).unwrap();
                    assert!(
                        at >= written.min(pair.floor),
                        "{id} {} {pair:?} at {degree}: {at} from {written}",
                        theme.id
                    );
                }
            }
        }
    }
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

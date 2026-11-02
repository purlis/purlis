// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A count's part rule near the hot count, in a tint** (#4384): a tint never turns the hot count,
//! so a fill a theme gives the plain count can come near it at some hue, though it is opaque and
//! far from it as written. An opaque colour moves no contrast, so only this distance asks for the
//! tints, and only where the recipe turns the rule's fill.

use serde_json::json;

use super::super::floors::{HOT_COUNT, HOT_COUNT_APART};
use super::super::{ResolvedTheme, load, tint};
use super::{COARSE_DEGREES, NEAR_THE_HOT_COUNT, apart, count_turns, fallen, family, rules};

const TOLD_APART: &str = "the plain count is told apart from the hot count";

/// A blue-violet hot count, and the rule's fill as light as it and far from it as written.
const HOT: &str = "#8f7dff";
const FILL: &str = "#e8605a";

/// The plain count's rule on `control.aimed`, black words on it at every hue.
fn on_a_rule(fill: &str, recipe: &[&str]) -> ResolvedTheme {
    let loaded = load(&json!({
        "name": "A count of its own", "extends": "purlis-classic",
        "appearances": [{ "id": "dark", "tokens": {
            "control.aimed": fill,
            "needs-you.count": HOT,
            "needs-you.count-text": "#000000",
        } }],
        "tint": { "tokens": recipe },
        "parts": {
            "activity.count.fill": "control.aimed",
            "activity.count.text": "needs-you.count-text",
        },
    }));
    assert!(!loaded.refused, "{:?}", loaded.complaints);
    loaded
        .theme
        .appearances
        .into_iter()
        .find(|one| one.id == "dark")
        .expect("the appearance")
}

fn from_hot(theme: &ResolvedTheme) -> f64 {
    apart(theme, "control.aimed", HOT_COUNT).expect("both are drawn")
}

fn told_apart_fell(theme: &ResolvedTheme) -> bool {
    fallen(theme).iter().any(|one| one.floor == TOLD_APART)
}

#[test]
fn a_rule_a_tint_brings_near_the_hot_count_draws_its_default() {
    let theme = on_a_rule(FILL, &["control.aimed"]);
    assert!(from_hot(&theme) >= HOT_COUNT_APART);
    assert!(
        (0..360).any(|degree| {
            from_hot(&tint::tinted_at(&theme, f64::from(degree))) < HOT_COUNT_APART
        }),
        "no hue brings the rule near"
    );
    assert!(told_apart_fell(&theme), "{:?}", fallen(&theme));
}

#[test]
fn a_rule_the_recipe_does_not_turn_is_read_as_written_alone() {
    let theme = on_a_rule(FILL, &["surface.hover"]);
    let activity = family("activity").unwrap();
    assert!(!count_turns(&theme, &rules(&theme.parts), activity));
    assert_eq!(fallen(&theme), []);
    // The control: the gate is the recipe, and it opens on the same colours.
    let turned = on_a_rule(FILL, &["control.aimed"]);
    assert!(count_turns(&turned, &rules(&turned.parts), activity));
}

#[test]
fn a_tint_s_step_moves_a_rule_less_than_the_margin_that_finds_it() {
    // What lets a coarse reading stand for the degrees between: a rule within the floor at any
    // degree is within the floor and the margin at the coarse degree nearest it.
    let step = u32::try_from(COARSE_DEGREES).unwrap();
    let mut widest: f64 = 0.0;
    for fill in [
        "#ff0000", "#00ff00", "#0000ff", "#ff00ff", "#ffff00", "#00ffff",
    ] {
        let theme = on_a_rule(fill, &["control.aimed"]);
        let at: Vec<_> = (0..360)
            .map(|degree| {
                let value = tint::tinted_at(&theme, f64::from(degree));
                tint::oklab_of(value.value("control.aimed")).expect("a colour")
            })
            .collect();
        for degree in 0..360_u32 {
            let nearest = (degree + step / 2) / step * step % 360;
            let (one, other) = (at[degree as usize], at[nearest as usize]);
            widest = widest.max(tint::distance(one, other));
        }
    }
    assert!(widest > 0.0, "the sweep turned nothing");
    assert!(widest < NEAR_THE_HOT_COUNT, "{widest}");
}

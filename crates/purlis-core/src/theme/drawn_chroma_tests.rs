// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Each side of a tint's rules is measured at the chroma it is drawn at** (#3388). The accent
//! gives way alone to the waiting amber while the sides of the shared rules keep the shared
//! chroma, so a rule between the two is drawn at two chromas; the search used to measure it at
//! one. A workspace's own `#rrggbb` can land on any hue, and the floors read the rules only at
//! the palette's, so nothing else stands behind a custom colour.
//!
//! The measure at one chroma stays as a limit: a chroma is offered only if the shared rules hold
//! at it too, so no built-in draws a colour it did not before.

use serde_json::{Value, json};

use super::floors::{ALLOW_APART, ALLOW_FROM_PRIMARY, DANGER_APART, WAITING_APART, hold};
use super::tint::{self, APART, Most, Rule};
use super::vocabulary::TINTED;
use super::{ResolvedTheme, built_ins, load, parts, purlis};

/// A workspace's own colour whose hue the shipped search got wrong: between the whole degrees.
const THE_BROKEN_HUE: &str = "#240c1b";

/// `tokens` on Purlis Classic's dark, its recipe turning `recipe`.
fn themed(tokens: &Value, recipe: &[&str]) -> ResolvedTheme {
    let loaded = load(&json!({
        "name": "Drawn", "extends": "purlis-classic",
        "appearances": [{ "id": "dark", "tokens": tokens }],
        "tint": { "tokens": recipe },
    }));
    assert!(!loaded.refused, "{:?}", loaded.complaints);
    loaded
        .theme
        .appearances
        .into_iter()
        .find(|one| one.id == "dark")
        .unwrap()
}

/// How far two theme colours are; `None` when one is not a colour, which has nothing to be near.
fn apart(one: &str, other: &str) -> Option<f64> {
    Some(tint::distance(tint::oklab_of(one)?, tint::oklab_of(other)?))
}

/// **What falls in `drawn`**: every rule of [`APART`] and Allow from the primary.
fn falls(drawn: &ResolvedTheme) -> Vec<String> {
    let rules = APART.iter().filter_map(|rule| {
        let near = apart(drawn.value(rule.token), drawn.value(rule.from))?;
        (near < rule.distance).then(|| format!("{} {} {near}", rule.token, rule.from))
    });
    let allow = parts::allow_from_primary(drawn)
        .into_iter()
        .filter(|look| look.apart < ALLOW_FROM_PRIMARY)
        .map(|look| format!("Allow {:?} {}", look.fills, look.apart));
    rules.chain(allow).collect()
}

/// **What the search answered before #3388**: every rule measured with every side at one chroma,
/// so a side drawn at the shared chroma was measured at the alone one.
fn at_one_chroma(theme: &ResolvedTheme, hue: f64) -> Most {
    Most {
        shared: tint::apart_chroma(theme, hue),
        alone: tint::apart_chroma_of(&tint::held_rules(theme), hue),
    }
}

/// `theme` turned as `most` says: what the window draws of its recipe at `hue`.
fn turned(theme: &ResolvedTheme, hue: f64, most: Most) -> ResolvedTheme {
    tint::turned_at(theme, hue, most, theme.tint.tabs())
}

/// A theme with the accent, Keep blocked's edge and the waiting amber as the broken hue found
/// them, and a danger colour nowhere near: only the accent's two rules can bind.
fn the_broken_theme() -> ResolvedTheme {
    themed(
        &json!({
            "accent.base": "#4d55df",
            "border.control": "#014c88",
            "state.waiting": "#ae2ca1",
            "danger.base": "#00c000",
        }),
        &["accent.base", "border.control"],
    )
}

#[test]
fn a_rule_between_the_two_chromas_holds_where_one_chroma_measured_it_wrong() {
    let theme = the_broken_theme();
    let hue = theme.tint.hue(THE_BROKEN_HUE).expect("a hue");
    let edges = |drawn: &ResolvedTheme| {
        apart(drawn.value("accent.base"), drawn.value("border.control")).unwrap()
    };
    // The control: measured at one chroma the rule holds, and drawn it does not.
    let was = at_one_chroma(&theme, hue);
    assert_eq!(was.shared, Some(f64::INFINITY));
    let edge_at_alone = tint::tint_hex_within(
        theme.value("border.control"),
        hue,
        was.alone.expect("a chroma the search said holds"),
    );
    let accent = turned(&theme, hue, was);
    assert!(apart(accent.value("accent.base"), &edge_at_alone).unwrap() >= ALLOW_APART);
    assert!(edges(&accent) < ALLOW_APART, "{}", edges(&accent));
    // Measured as it is drawn, it holds, and the waiting rule with it.
    let drawn = tint::tinted(&theme, THE_BROKEN_HUE);
    assert!(edges(&drawn) >= ALLOW_APART, "{}", edges(&drawn));
    assert_eq!(falls(&drawn), Vec::<String>::new());
}

#[test]
fn purlis_light_keeps_its_accent_where_only_a_grey_its_kin_could_not_take_would_hold() {
    // Between a yellow and a green workspace a chroma of 0.01 to 0.015 holds the accent's own
    // rules, and the link would fall near danger at it. The accent as written breaks nothing, so
    // it stays: a floor, not a search, is what may take a built-in's violet away.
    let light = purlis()
        .appearances
        .iter()
        .find(|one| one.id == "light")
        .expect("Purlis's light appearance");
    let accent = light.value("accent.base");
    for (from, floor) in [
        ("danger.base", DANGER_APART),
        ("state.waiting", WAITING_APART),
        ("border.control", ALLOW_APART),
    ] {
        assert!(apart(accent, light.value(from)).unwrap() >= floor, "{from}");
    }
    for degree in 98..118 {
        let drawn = tint::tinted_at(light, f64::from(degree));
        assert_eq!(drawn.value("accent.base"), accent, "{degree}");
        assert_eq!(falls(&drawn), Vec::<String>::new(), "{degree}");
        // The control: the link is what turns the grey down.
        let chroma = tint::tint_hex_within(light.value("markdown.link"), f64::from(degree), 0.01);
        assert!(apart(&chroma, light.value("danger.base")).unwrap() < DANGER_APART);
    }
}

/// A seeded generator: the sweep draws the same cases on every run.
struct Dice(u64);

impl Dice {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    /// A number under `most`.
    fn below(&mut self, most: u32) -> f64 {
        f64::from(u32::try_from(self.next() % u64::from(most)).unwrap())
    }

    fn colour(&mut self) -> String {
        format!("#{:06x}", self.next() & 0xff_ffff)
    }
}

/// `rules`' first two sides as `most` draws them, the accent held alone and the edge shared.
fn accent_and_edge(accent: &str, edge: &str, hue: f64, most: Most) -> [String; 2] {
    let at = |value: &str, chroma: Option<f64>| {
        chroma.map_or_else(
            || value.to_owned(),
            |chroma| tint::tint_hex_within(value, hue, chroma),
        )
    };
    [at(accent, most.alone), at(edge, most.shared)]
}

#[test]
fn the_accent_s_two_rules_hold_at_any_hue_for_any_colours_written_apart() {
    let mut dice = Dice(0x9e37_79b9_7f4a_7c15);
    let mut held = 0;
    while held < 150 {
        let accent = dice.colour();
        let Some(own) = tint::hue_of(&accent) else {
            continue;
        };
        let waiting = tint::tint_hex(&accent, (own + 20.0 + dice.below(60)) % 360.0);
        let edge = tint::tint_hex_within(
            &dice.colour(),
            (own + 330.0 + dice.below(60)) % 360.0,
            0.02 * (1.0 + dice.below(15)),
        );
        if apart(&accent, &edge).unwrap() < ALLOW_APART
            || apart(&accent, &waiting).unwrap() < WAITING_APART
        {
            continue;
        }
        held += 1;
        let hue = (own + dice.below(36_000) / 100.0) % 360.0;
        let all = [
            Rule {
                token: &accent,
                token_tinted: true,
                token_alone: true,
                from: &edge,
                from_tinted: true,
                from_alone: false,
                distance: ALLOW_APART,
            },
            Rule {
                token: &accent,
                token_tinted: true,
                token_alone: true,
                from: &waiting,
                from_tinted: false,
                from_alone: true,
                distance: WAITING_APART,
            },
        ];
        let most = tint::most_of(&all, &all[..1], hue);
        let [drawn, side] = accent_and_edge(&accent, &edge, hue, most);
        let case = format!("{accent} {edge} {waiting} at {hue}: {most:?}");
        assert!(apart(&drawn, &side).unwrap() >= ALLOW_APART, "{case}");
        assert!(apart(&drawn, &waiting).unwrap() >= WAITING_APART, "{case}");
    }
}

/// A `#rrggbb` a person could give a workspace, every five degrees and between the whole ones.
fn custom_colours() -> Vec<String> {
    (0..72)
        .map(|step| tint::tint_hex("#c0392b", f64::from(step) * 5.0 + 0.37))
        .collect()
}

/// A theme of Purlis Classic's that also turns Keep blocked's edge, its colours from `dice`.
fn thrown(dice: &mut Dice) -> ResolvedTheme {
    let recipe: Vec<&str> = TINTED.iter().copied().chain(["border.control"]).collect();
    themed(
        &json!({ "accent.base": dice.colour(), "border.control": dice.colour() }),
        &recipe,
    )
}

#[test]
fn every_rule_and_allow_hold_under_every_custom_colour() {
    let mut dice = Dice(0x2545_f491_4f6c_dd1d);
    let themes: Vec<ResolvedTheme> = built_ins()
        .iter()
        .chain(&purlis().appearances)
        .cloned()
        .chain((0..4).map(|_| thrown(&mut dice)))
        .chain([the_broken_theme()])
        .collect();
    let colours = custom_colours();
    for theme in &themes {
        let (drawn, _) = hold(theme);
        for colour in &colours {
            let tint = tint::tinted(&drawn, colour);
            assert_eq!(falls(&tint), Vec::<String>::new(), "{} {colour}", theme.id);
        }
    }
}

#[test]
fn the_sweep_reads_what_it_says_it_holds() {
    // The control: the same reading of the same colours fails as written when a tint is given
    // whole, with no search at all, so a reading that saw nothing could not pass above.
    let ours = themed(&json!({}), TINTED);
    let whole = Most {
        shared: Some(f64::INFINITY),
        alone: Some(f64::INFINITY),
    };
    let close = themed(
        &json!({ "danger.base": tint::tint_hex(ours.value("accent.base"), 25.0) }),
        TINTED,
    );
    let fell = falls(&turned(&close, 25.0, whole));
    assert!(
        fell.iter()
            .any(|one| one.starts_with("accent.base danger.base")),
        "{fell:?}"
    );
    assert_eq!(
        falls(&tint::tinted_at(&hold(&close).0, 25.0)),
        Vec::<String>::new()
    );
}

#[test]
fn no_built_in_draws_a_colour_it_did_not_before() {
    let mut moved = Vec::new();
    let customs: Vec<f64> = custom_colours()
        .iter()
        .filter_map(|colour| tint::hue_of(colour))
        .collect();
    for theme in built_ins().iter().chain(&purlis().appearances) {
        let tabs = theme.tint.tabs();
        if tabs.is_empty() {
            continue;
        }
        let named = theme.tint.hues.iter().map(|one| one.hue);
        let whole = (0..360).map(f64::from);
        for hue in whole.chain(named).chain(customs.iter().copied()) {
            let (now, was) = (tint::most_at(theme, hue), at_one_chroma(theme, hue));
            if now == was {
                continue;
            }
            let (now, was) = (turned(theme, hue, now), turned(theme, hue, was));
            for token in tabs {
                if now.value(token) != was.value(token) {
                    moved.push(format!(
                        "{} {hue} {token}: {} to {}",
                        theme.id,
                        was.value(token),
                        now.value(token)
                    ));
                }
            }
        }
    }
    assert_eq!(moved, Vec::<String>::new());
}

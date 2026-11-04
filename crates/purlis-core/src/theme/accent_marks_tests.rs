// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What is drawn in the accent follows a given accent** (#4918): an accent given over Purlis
//! turns the primary's fill (at rest and under the pointer), the focus ring and the selected row's
//! edge to its hue, as it turns the washes (#4666): luminance and alpha kept, chroma no more than
//! the accent's. Each keeps every contrast Purlis gave it, and stays as apart from waiting and
//! danger as the rules ask; where turning would bring it nearer, it stays Purlis's. The active tab
//! is a grey ground in Purlis, not drawn in the accent, and stays Purlis's.

use serde_json::json;

use super::check::{PAIRS, contrast, ratio};
use super::person::Person;
use super::tint::{self, APART, oklab_of};
use super::vocabulary::DRAWN_IN_THE_ACCENT;
use super::*;

/// A person's teal, the one #3100's tests give.
const TEAL: &str = "#2bb3a3";

/// A teal dark enough for a light window's words.
const DARK_TEAL: &str = "#0b6359";

fn light(theme: &ResolvedTheme) -> bool {
    tint::luminance_of(theme.value("surface.base")).unwrap() > 0.5
}

/// A colour's OKLCH hue in degrees.
fn hue(hex: &str) -> f64 {
    let [_, a, b] = oklab_of(hex).expect("a colour");
    b.atan2(a).to_degrees().rem_euclid(360.0)
}

fn degrees_apart(a: f64, b: f64) -> f64 {
    let d = (a - b).rem_euclid(360.0);
    d.min(360.0 - d)
}

fn apart(a: &str, b: &str) -> f64 {
    tint::distance(oklab_of(a).unwrap(), oklab_of(b).unwrap())
}

/// `theme` with a person's `accent` laid over it.
fn with_accent(theme: &ResolvedTheme, accent: &str) -> ResolvedTheme {
    let (person, said) = Person::read(Some(&json!({ "colours": { "accent.base": accent } })));
    assert!(said.is_empty(), "{said:?}");
    person.layer().over(theme).0
}

#[test]
fn a_given_teal_turns_the_fill_the_ring_and_the_edge_in_every_purlis_appearance() {
    for theme in &purlis().appearances {
        let given = if light(theme) { DARK_TEAL } else { TEAL };
        let drawn = with_accent(theme, given);
        assert_eq!(drawn.value("accent.base"), given, "{}", theme.id);
        for token in DRAWN_IN_THE_ACCENT {
            let (was, now) = (theme.value(token), drawn.value(token));
            assert_ne!(now, was, "{} {token} stayed Purlis's", theme.id);
            // What Purlis draws as its accent is the accent given (#5157).
            if was == theme.value("accent.base") {
                assert_eq!(now, given, "{} {token}", theme.id);
                continue;
            }
            assert!(
                degrees_apart(hue(now), hue(given)) < 6.0,
                "{} {token}: {now}",
                theme.id
            );
            let (l_was, l_now) = (
                tint::luminance_of(was).unwrap(),
                tint::luminance_of(now).unwrap(),
            );
            assert!(
                (l_was - l_now).abs() < 0.005,
                "{} {token}: {was} → {now}",
                theme.id
            );
        }
        // The active tab is a grey ground in Purlis: nothing of the accent to turn.
        assert_eq!(
            drawn.value("tab.active"),
            theme.value("tab.active"),
            "{}",
            theme.id
        );
    }
}

#[test]
fn what_follows_the_accent_keeps_its_contrasts_and_its_distance_from_waiting_and_danger() {
    // At every tenth degree, an accent of Purlis's own lightness and colour at that hue.
    for theme in purlis().appearances.iter().chain(built_ins()) {
        for degree in (0..360).step_by(10) {
            let given = tint::tint_hex(theme.value("accent.base"), f64::from(degree));
            let drawn = with_accent(theme, &given);
            let id = format!("{} {} at {degree} ({given})", theme.name, theme.id);
            for pair in PAIRS
                .iter()
                .filter(|pair| DRAWN_IN_THE_ACCENT.contains(&pair.front))
            {
                let (Some(was), Some(is)) = (ratio(theme, pair), ratio(&drawn, pair)) else {
                    continue;
                };
                assert!(
                    is >= was.min(pair.floor),
                    "{id}: {} on {} is {is}, {was} as written",
                    pair.front,
                    pair.back
                );
            }
            for fill in ["accent.fill", "accent.fill-hover"] {
                let (was, is) = (
                    contrast(theme.value("accent.fill-text"), theme.value(fill)),
                    contrast(drawn.value("accent.fill-text"), drawn.value(fill)),
                );
                assert!(
                    is >= was.min(4.5),
                    "{id}: the primary's words on {fill} are {is}"
                );
            }
            for rule in APART
                .iter()
                .filter(|rule| DRAWN_IN_THE_ACCENT.contains(&rule.token))
            {
                let (was, is) = (
                    apart(theme.value(rule.token), theme.value(rule.from)),
                    apart(drawn.value(rule.token), drawn.value(rule.from)),
                );
                assert!(
                    is >= was.min(rule.distance),
                    "{id}: {} {} is {is} from {} {}",
                    rule.token,
                    drawn.value(rule.token),
                    rule.from,
                    drawn.value(rule.from)
                );
            }
        }
    }
}

#[test]
fn an_orange_accent_s_ring_never_reads_as_waiting() {
    // An orange the colour of Purlis's waiting amber: the ring turned to it would be the waiting
    // edge; it stays Purlis's lavender.
    let dark = purlis().first();
    let drawn = with_accent(dark, dark.value("state.waiting"));
    for token in ["focus.ring", "list.selected-edge"] {
        assert!(
            apart(drawn.value(token), drawn.value("state.waiting")) >= 0.1,
            "{token} {}",
            drawn.value(token)
        );
    }
}

#[test]
fn a_ring_the_theme_gives_is_its_own() {
    let dark = purlis().first();
    let loaded = load(&json!({
        "name": "Teal over Purlis", "extends": "purlis",
        "appearances": [{ "id": dark.id, "tokens": { "accent.base": TEAL, "focus.ring": "#ffffff" } }],
    }));
    assert!(!loaded.refused, "{:?}", loaded.complaints);
    let theme = loaded
        .theme
        .appearances
        .into_iter()
        .find(|one| one.id == dark.id)
        .unwrap();
    assert_eq!(theme.value("focus.ring"), "#ffffff");
    assert_ne!(
        theme.value("list.selected-edge"),
        dark.value("list.selected-edge")
    );
}

#[test]
fn a_fill_the_parent_draws_as_its_surface_follows_the_surface_not_the_accent() {
    // Purlis Classic's primary is filled with its surface (#3177's KIN): an accent given over it
    // alone leaves the fill as Classic draws it, and the ring and the edge turn.
    for classic in built_ins() {
        let loaded = load(&json!({
            "name": "Teal over Classic",
            "appearance": if light(classic) { "light" } else { "dark" },
            "tokens": { "accent.base": if light(classic) { DARK_TEAL } else { TEAL } },
        }));
        assert!(!loaded.refused, "{:?}", loaded.complaints);
        let theme = loaded.theme.first();
        for fill in ["accent.fill", "accent.fill-hover"] {
            assert_eq!(
                theme.value(fill),
                classic.value(fill),
                "{} {fill}",
                classic.id
            );
        }
        for edge in ["focus.ring", "list.selected-edge"] {
            assert_ne!(
                theme.value(edge),
                classic.value(edge),
                "{} {edge}",
                classic.id
            );
        }
    }
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What is drawn in the accent follows a given accent** (#4918): an accent given over Purlis
//! turns the primary's fill (at rest and under the pointer), the focus ring and the selected row's
//! edge to its hue, as it turns the washes (#4666): luminance and alpha kept, chroma no more than
//! the accent's. Each keeps every contrast Purlis gave it, and stays as apart from waiting and
//! danger as the rules ask; where turning would bring it nearer, it stays Purlis's. The active tab
//! is a grey ground in Purlis, not drawn in the accent, and stays Purlis's. What the parent draws
//! as its accent is the accent given, and the link and the first chart colour follow it too, each
//! kept from the state colours (#5157).

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
            let follows = |token: &str| {
                DRAWN_IN_THE_ACCENT.contains(&token) || tint::SIGNAL_MARKS.contains(&token)
            };
            for pair in PAIRS.iter().filter(|pair| follows(pair.front)) {
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
            for rule in APART.iter().filter(|rule| follows(rule.token)) {
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
fn a_fill_the_parent_draws_as_its_surface_follows_the_surface_as_turned_not_as_written() {
    // Purlis Classic's primary is filled with its surface (#3177's KIN): an accent given over it
    // alone turns the surface (#4666), and the fill is that turned surface, sharing its hue
    // (#5157), not Classic's teal beside a red surface. The fill under the pointer is Classic's
    // active tab, which no accent turns; the ring and the edge turn.
    for classic in built_ins() {
        for accent in [
            if light(classic) { DARK_TEAL } else { TEAL },
            if light(classic) { "#5b3f9e" } else { "#9d7cd8" },
        ] {
            let loaded = load(&json!({
                "name": "An accent over Classic",
                "appearance": if light(classic) { "light" } else { "dark" },
                "tokens": { "accent.base": accent },
            }));
            assert!(!loaded.refused, "{:?}", loaded.complaints);
            let theme = loaded.theme.first();
            let id = format!("{} {accent}", classic.id);
            assert_ne!(
                theme.value("accent.surface"),
                classic.value("accent.surface"),
                "{id}"
            );
            assert_eq!(
                theme.value("accent.fill"),
                theme.value("accent.surface"),
                "{id}"
            );
            assert_eq!(
                theme.value("accent.fill-hover"),
                classic.value("accent.fill-hover"),
                "{id}"
            );
            for edge in ["focus.ring", "list.selected-edge"] {
                assert_eq!(theme.value(edge), accent, "{id} {edge}");
            }
        }
    }
}

#[test]
fn a_given_blue_turns_the_link_and_the_first_chart_colour_and_keeps_them_from_the_states() {
    // Purlis draws its first chart colour as its accent and its link in its accent's family
    // (#5157): a blue given over it, at Purlis's own lightness, draws both blue, never within 0.1
    // of a state colour.
    for theme in &purlis().appearances {
        let given = tint::tint_hex(theme.value("accent.base"), 235.0);
        let drawn = with_accent(theme, &given);
        for mark in tint::SIGNAL_MARKS {
            let (was, now) = (theme.value(mark), drawn.value(mark));
            let id = format!("{} {mark}: {was} → {now}", theme.id);
            if was == theme.value("accent.base") {
                assert_eq!(now, given, "{id}");
            }
            assert!(degrees_apart(hue(now), hue(&given)) < 6.0, "{id}");
            for state in tint::SIGNALS {
                assert!(
                    apart(now, drawn.value(state)) >= 0.1,
                    "{id} is {} from {state}",
                    apart(now, drawn.value(state))
                );
            }
        }
    }
}

#[test]
fn a_given_teal_draws_a_teal_link_and_first_chart_colour_clear_of_the_states() {
    // Purlis's link turned to teal at Purlis's lightness would read as success, so the link takes
    // the teal itself, which clears it (#5157, as a tinted link takes the accent's own, #4921).
    for theme in &purlis().appearances {
        // A teal the person's contrast keeps as the accent in each appearance, clear of success.
        let given = match theme.id.as_str() {
            "light" => "#026d6d",
            "light-high-contrast" => DARK_TEAL,
            _ => TEAL,
        };
        let drawn = with_accent(theme, given);
        assert_eq!(drawn.value("accent.base"), given, "{}", theme.id);
        for mark in tint::SIGNAL_MARKS {
            let now = drawn.value(mark);
            let id = format!("{} {mark}: {now}", theme.id);
            assert_eq!(now, given, "{id}");
            for state in tint::SIGNALS {
                assert!(apart(now, drawn.value(state)) >= 0.1, "{id} from {state}");
            }
        }
    }
}

#[test]
fn a_teal_itself_too_near_success_keeps_the_link_and_first_chart_colour_as_they_were() {
    // In Purlis light this teal is the accent, but within 0.1 of success as a link: both marks
    // stay Purlis's, so a link is never read as a state (#4921), and the accent is still the teal.
    let theme = purlis()
        .appearances
        .iter()
        .find(|one| one.id == "light")
        .expect("Purlis light");
    assert!(apart(DARK_TEAL, theme.value("state.success")) < 0.1);
    let drawn = with_accent(theme, DARK_TEAL);
    assert_eq!(drawn.value("accent.base"), DARK_TEAL);
    for mark in tint::SIGNAL_MARKS {
        assert_eq!(drawn.value(mark), theme.value(mark), "{mark}");
    }
}

#[test]
fn an_accent_the_colour_of_success_leaves_the_link_and_first_chart_colour_as_they_were() {
    // Neither the link turned nor the accent itself can stand apart from success: both marks stay
    // the theme's, so a link is never read as a state (#4921).
    for theme in &purlis().appearances {
        let drawn = with_accent(theme, theme.value("state.success"));
        for mark in tint::SIGNAL_MARKS {
            assert_eq!(drawn.value(mark), theme.value(mark), "{} {mark}", theme.id);
        }
    }
}

#[test]
fn a_theme_s_lavender_over_classic_light_leaves_its_ring_turned_and_the_theme_drawn() {
    // A theme's own accent has no person layer to leave a colour out: the ring drawn as this
    // lavender would be under its 3:1 on a light surface, a hard floor, and refuse the whole
    // theme. It turns to the lavender's hue at Classic's lightness instead (#5157).
    let classic = built_ins()
        .iter()
        .find(|one| light(one))
        .expect("Classic light");
    let lavender = "#b4a7f5";
    let loaded = load(&json!({
        "name": "Lavender over Classic", "appearance": "light",
        "tokens": { "accent.base": lavender },
    }));
    assert!(!loaded.refused, "{:?}", loaded.complaints);
    let theme = loaded.theme.first();
    for edge in ["focus.ring", "list.selected-edge"] {
        let now = theme.value(edge);
        assert_ne!(now, lavender, "{edge}");
        assert_ne!(now, classic.value(edge), "{edge}");
        assert!(
            degrees_apart(hue(now), hue(lavender)) < 6.0,
            "{edge}: {now}"
        );
    }
}

#[test]
fn what_follows_any_accent_keeps_every_contrast_it_had_or_its_floor() {
    let steps = [0x10, 0x50, 0x90, 0xd0];
    for theme in purlis().appearances.iter().chain(built_ins()) {
        for r in steps {
            for g in steps {
                for b in steps {
                    let accent = format!("#{r:02x}{g:02x}{b:02x}");
                    let mut given = std::collections::BTreeMap::new();
                    given.insert("accent.base", accent.clone());
                    super::vocabulary::follow_kin(&mut given, &theme.values);
                    let mut drawn = theme.clone();
                    drawn.values.extend(given);
                    let follows = |token: &str| {
                        DRAWN_IN_THE_ACCENT.contains(&token) || tint::SIGNAL_MARKS.contains(&token)
                    };
                    for pair in PAIRS.iter().filter(|pair| pair.tokens().any(follows)) {
                        let (Some(was), Some(is)) = (ratio(theme, pair), ratio(&drawn, pair))
                        else {
                            continue;
                        };
                        assert!(
                            is >= was.min(pair.floor),
                            "{} {} under {accent}: {} on {} is {is}, {was} as written",
                            theme.name,
                            theme.id,
                            pair.front,
                            pair.back
                        );
                    }
                }
            }
        }
    }
}

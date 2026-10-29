// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A side view's card row stands apart from its panel** (#3774), at the engine's interface: a
//! theme in, what the check says out. The card's fill steps from the side panel's, or the card
//! draws an edge a hairline from it, in every built-in appearance and in every tint.

use serde_json::{Value, json};

use super::super::{Finding, ResolvedTheme, built_ins, check, load, purlis};
use super::{CARD_EDGE, CARD_STEP, PAIRS, contrast};

/// Every built-in appearance: Purlis Classic's and Purlis's.
fn every_built_in() -> impl Iterator<Item = &'static ResolvedTheme> {
    built_ins().iter().chain(&purlis().appearances)
}

/// A dark theme over Purlis Classic that sets `parts`, as read.
fn over_classic(parts: Value) -> ResolvedTheme {
    let loaded = load(&json!({
        "name": "Probe",
        "extends": "purlis-classic",
        "appearances": [{ "id": "dark", "tokens": {} }],
        "parts": parts,
    }));
    assert!(!loaded.refused, "{:?}", loaded.complaints);
    loaded.theme.first().clone()
}

/// What the check said of the card row, as `(tint, card, ground, ratio)`.
fn not_apart(
    theme: &ResolvedTheme,
) -> Vec<(Option<&'static str>, &'static str, &'static str, f64)> {
    check(theme)
        .findings
        .into_iter()
        .filter_map(|one| match one {
            Finding::CardNotApart {
                tint,
                card,
                ground,
                ratio,
                floor,
            } => {
                assert_eq!(floor, CARD_STEP);
                Some((tint, card, ground, ratio))
            }
            _ => None,
        })
        .collect()
}

#[test]
fn every_built_in_appearance_draws_its_side_view_s_cards_a_step_from_the_panel() {
    for theme in every_built_in() {
        assert_eq!(not_apart(theme), [], "{} {}", theme.name, theme.id);
        let on = super::super::parts::card_on_side(theme).expect("both choose their cards");
        let step = contrast(theme.value(on.card), theme.value(on.side));
        assert!(
            step >= CARD_STEP,
            "{} {}: {} on {} steps {step:.3}",
            theme.name,
            theme.id,
            on.card,
            on.side
        );
    }
}

#[test]
fn a_card_row_drawn_in_its_panel_s_own_ground_is_reported_as_written_and_in_every_tint() {
    let theme = over_classic(json!({ "list.root.card.fill": "surface.base" }));
    let found = not_apart(&theme);
    assert!(
        found.contains(&(None, "surface.base", "surface.base", 1.0)),
        "{found:?}"
    );
    // A workspace's colour keeps the two the same: each tint says so too.
    assert_eq!(found.len(), 1 + theme.tint.hues.len(), "{found:?}");
}

#[test]
fn a_panel_drawn_in_the_card_s_ground_is_reported_too() {
    // Purlis's panel ground under the family's own card ground: the card vanishes into it.
    let theme = over_classic(json!({
        "list.root.card.fill": "surface.raised",
        "panel.root.fill": "surface.raised",
    }));
    assert!(
        not_apart(&theme).contains(&(None, "surface.raised", "surface.raised", 1.0)),
        "{:?}",
        not_apart(&theme)
    );
}

#[test]
fn a_card_on_its_panel_s_ground_inside_an_edge_stands_apart() {
    // Its edge, muted words' colour, clears the hairline floor on the panel.
    let edged = over_classic(json!({
        "list.root.card.fill": "surface.base",
        "list.root.card.border-colour": "text.muted",
        "list.root.card.border-width": 1,
    }));
    assert!(contrast(edged.value("text.muted"), edged.value("surface.base")) >= CARD_EDGE);
    assert_eq!(not_apart(&edged), []);
    // An edge with no width is not drawn.
    let unedged = over_classic(json!({
        "list.root.card.fill": "surface.base",
        "list.root.card.border-colour": "text.muted",
        "list.root.card.border-width": 0,
    }));
    assert_eq!(not_apart(&unedged).len(), 1 + unedged.tint.hues.len());
}

#[test]
fn the_card_s_step_is_only_reported_never_a_pair_the_floors_or_a_person_s_colours_act_on() {
    // #3735: a pair of `PAIRS` also decides the consent floors, an import's tier walk and which of
    // the person's own colours a theme takes, so a ground the parts choose is never one.
    assert!(
        !PAIRS.iter().any(|pair| {
            let grounds = ["surface.base", "surface.raised", "control.base"];
            grounds.contains(&pair.front) && grounds.contains(&pair.back)
        }),
        "a card's ground is not held as a pair"
    );
}

#[test]
fn a_palette_alone_is_not_yet_held() {
    // A theme with no parts draws its cards on the family's raised ground, wherever its palette
    // puts it: like a meter's fill on its track, that waits for a pair the check only reports
    // (#3735).
    let loaded = load(&json!({
        "name": "Flat",
        "appearances": [{ "id": "dark", "appearance": "dark", "tokens": {
            "surface.base": "#202020",
            "surface.raised": "#202020",
        } }],
    }));
    assert!(!loaded.refused, "{:?}", loaded.complaints);
    assert_eq!(not_apart(loaded.theme.first()), []);
}

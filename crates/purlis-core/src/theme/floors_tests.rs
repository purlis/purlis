// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The consent floors, at the engine's interface: a theme in, what the window draws out.

use std::sync::Arc;

use serde_json::{Value, json};

use super::vocabulary;
use super::*;
use crate::extension::project::theme::Pick;

/// A dark theme named `name` that sets `tokens` and is Purlis Classic's for the rest.
fn theme(name: &str, tokens: Value) -> Theme {
    let resolved = load(&json!({ "name": name, "appearance": "dark", "tokens": tokens }));
    assert!(!resolved.refused, "{:?}", resolved.complaints);
    resolved.theme
}

fn ours(token: &str) -> &'static str {
    built_in_for(Appearance::Dark).value(token)
}

/// What the window draws of `theme`'s first appearance.
fn drawn_of(theme: &Theme) -> (ResolvedTheme, Option<Held>) {
    let drawn = draw(theme.first());
    (drawn.theme.expect("a theme is always drawn"), drawn.held)
}

/// Every consent pair the drawn theme fails, as it is and under every workspace tint.
fn consent_failures(drawn: &ResolvedTheme) -> Vec<Finding> {
    let consent: Vec<Pair> = floors::pairs().map(|(_, pair)| *pair).collect();
    check(drawn)
        .findings
        .into_iter()
        .filter(|one| matches!(one, Finding::Contrast { pair, .. } if consent.contains(pair)))
        .collect()
}

#[test]
fn the_built_in_themes_clear_every_consent_floor_and_are_drawn_unchanged() {
    for built in built_ins() {
        let drawn = draw(built);
        assert_eq!(drawn.held, None, "{}", built.name);
        assert_eq!(drawn.theme.as_ref(), Some(built));
        assert!(floors::apart(built) >= DANGER_APART, "{}", built.name);
        assert!(floors::allow_apart(built) >= ALLOW_APART, "{}", built.name);
    }
    for pick in ["purlis-dark", "purlis-light", "system"] {
        let pick = Pick::parse(pick).expect("a pick");
        let it = resolve(
            &Inputs {
                pick: Some(&pick),
                theirs: None,
                on: On::Every,
                system: Appearance::Light,
                contrast: Contrast::Standard,
            },
            || unreachable!("no survey for a built-in"),
        );
        assert_eq!(it.held, None);
    }
}

#[test]
fn every_ask_pair_is_one_the_check_measures() {
    let asked: std::collections::BTreeSet<(&str, &str)> = floors::pairs()
        .filter(|(floor, _)| *floor == Floor::Ask)
        .map(|(_, pair)| (pair.front, pair.back))
        .collect();
    assert_eq!(
        asked.into_iter().collect::<Vec<_>>(),
        [
            ("needs-you.text", "needs-you.base"),
            // Selected text a theme colours, on its selection over a dialog (#2706).
            ("selection.text", "selection.background"),
            ("text.primary", "control.base"),
            ("text.primary", "control.hover"),
            ("text.primary", "danger.wash"),
            ("text.primary", "surface.overlay"),
        ]
    );
}

#[test]
fn danger_words_that_vanish_into_the_danger_surface_are_drawn_as_purlis_s_and_the_rest_as_written()
{
    let mine = theme(
        "Ember",
        json!({
            "danger.text": "#301010",
            "danger.surface": "#301010",
            "text.primary": "#eeeeee",
            "accent.base": "#4fa3e0",
        }),
    );
    let (drawn, held) = drawn_of(&mine);
    assert_eq!(drawn.value("danger.text"), ours("danger.text"));
    assert_eq!(drawn.value("danger.surface"), ours("danger.surface"));
    assert_eq!(drawn.value("text.primary"), "#eeeeee");
    assert_eq!(drawn.value("accent.base"), "#4fa3e0");
    let held = held.expect("the floor was held");
    assert_eq!(held.theme, "Ember");
    // Its blue accent also brings the primary's fill near Allow's at a hue a workspace's own
    // colour can take, which is drawn untinted there (#2948).
    assert_eq!(held.floors, [Floor::Danger, Floor::AllowNotPrimary]);
    assert_eq!(held.tokens, ["danger.surface", "danger.text"]);
    assert_eq!(consent_failures(&drawn), []);
}

#[test]
fn a_state_mark_lost_on_the_window_is_purlis_s_and_the_window_stays_the_theme_s() {
    let mine = theme(
        "Flat",
        json!({ "state.failed": "#202020", "surface.base": "#202020" }),
    );
    let (drawn, held) = drawn_of(&mine);
    assert_eq!(drawn.value("state.failed"), ours("state.failed"));
    assert_eq!(drawn.value("surface.base"), "#202020");
    let held = held.expect("the floor was held");
    assert_eq!(held.floors, [Floor::StateMarks]);
    assert_eq!(held.tokens, ["state.failed"]);
    assert_eq!(consent_failures(&drawn), []);
}

#[test]
fn danger_drawn_in_the_accent_s_colour_is_drawn_in_purlis_s_danger() {
    let mine = theme(
        "Teal",
        json!({ "danger.base": "#52c4a6", "accent.base": "#52c4a6" }),
    );
    let (drawn, held) = drawn_of(&mine);
    assert_eq!(drawn.value("danger.base"), ours("danger.base"));
    assert_eq!(drawn.value("accent.base"), "#52c4a6");
    let held = held.expect("the floor was held");
    assert_eq!(held.floors, [Floor::DangerApart]);
    assert_eq!(held.tokens, ["danger.base"]);
}

#[test]
fn an_accent_in_purlis_s_danger_colour_is_drawn_as_purlis_s_accent() {
    // Purlis's own danger is no further from this accent than the theme's was, so the ground of
    // the floor, the accent, is drawn as purlis's too.
    let red = ours("danger.base");
    let mine = theme("Rose", json!({ "danger.base": red, "accent.base": red }));
    let (drawn, held) = drawn_of(&mine);
    assert_eq!(drawn.value("danger.base"), red);
    assert_eq!(drawn.value("accent.base"), ours("accent.base"));
    // The primary's edge follows the accent a theme over Classic names (#3177), so it is drawn as
    // purlis's too.
    assert_eq!(
        held.expect("held").tokens,
        ["accent.base", "accent.fill-edge"]
    );
}

#[test]
fn an_ask_s_words_lost_on_its_dialog_are_purlis_s() {
    let mine = theme("Dim", json!({ "text.primary": "#2a2a2a" }));
    let (drawn, held) = drawn_of(&mine);
    assert_eq!(drawn.value("text.primary"), ours("text.primary"));
    let held = held.expect("the floor was held");
    assert_eq!(held.floors, [Floor::Ask]);
    assert_eq!(held.tokens, ["text.primary"]);
    assert_eq!(consent_failures(&drawn), []);
}

#[test]
fn the_notice_names_the_theme_and_each_floor_it_breaks_once() {
    let mine = theme(
        "Muddle",
        json!({
            "danger.text": "#301010",
            "danger.surface": "#301010",
            "state.waiting": "#191917",
            "state.running": "#191917",
        }),
    );
    let held = drawn_of(&mine).1.expect("held");
    assert_eq!(held.floors, [Floor::Danger, Floor::StateMarks]);
    let said = held.said();
    assert!(
        said.starts_with("Muddle breaks the consent floors"),
        "{said}"
    );
    assert!(said.contains(Floor::Danger.says()), "{said}");
    assert!(said.contains(Floor::StateMarks.says()), "{said}");
    assert!(said.contains("state.running, state.waiting"), "{said}");
}

#[test]
fn a_theme_that_clears_the_floors_is_drawn_exactly_as_written() {
    let mine = theme(
        "Calm",
        json!({ "accent.base": "#4fa3e0", "danger.base": "#e88080", "state.running": "#58a6ff" }),
    );
    let (drawn, held) = drawn_of(&mine);
    assert_eq!(held, None);
    assert_eq!(&drawn, mine.first());
}

#[test]
fn every_source_is_drawn_with_the_floors_held() {
    let breaks = json!({ "danger.text": "#301010", "danger.surface": "#301010" });
    let mine = theme("Mine", breaks.clone());
    // The person's own theme, with no pick.
    let it = resolve(
        &Inputs {
            pick: None,
            theirs: Some(&mine),
            on: On::Every,
            system: Appearance::Dark,
            contrast: Contrast::Standard,
        },
        || unreachable!("no survey when the person has a theme"),
    );
    assert_eq!(it.held.expect("theirs is held").theme, "Mine");
    // Following the system, a theme of one appearance.
    assert!(
        follow(&mine, Appearance::Light, Contrast::Standard)
            .held
            .is_some()
    );
    // An extension's, picked and with no pick.
    let text = json!({ "name": "Night", "appearance": "dark", "tokens": breaks }).to_string();
    let offered =
        || -> Arc<[Offered]> { Arc::from(vec![Offered::of("acme".into(), "Night".into(), &text)]) };
    for pick in [Some("acme/Night"), None] {
        let pick = pick.map(|one| Pick::parse(one).expect("a pick"));
        let it = resolve(
            &Inputs {
                pick: pick.as_ref(),
                theirs: None,
                on: On::Every,
                system: Appearance::Dark,
                contrast: Contrast::Standard,
            },
            offered,
        );
        let drawn = it.theme.expect("drawn");
        assert_eq!(drawn.value("danger.text"), ours("danger.text"));
        assert_eq!(it.held.expect("an extension's is held").theme, "Night");
    }
}

#[test]
fn a_theme_folder_s_appearances_are_each_held_against_purlis_s_of_their_own_tag() {
    let document = json!({
        "name": "Duo",
        "appearances": [
            { "id": "night", "appearance": "dark", "tokens": { "state.failed": "#191917" } },
            { "id": "day", "appearance": "light", "tokens": { "state.failed": "#fcfbf8" } }
        ]
    });
    let duo = load(&document);
    assert!(!duo.refused, "{:?}", duo.complaints);
    for (system, tag) in [
        (Appearance::Dark, Appearance::Dark),
        (Appearance::Light, Appearance::Light),
    ] {
        let it = follow(&duo.theme, system, Contrast::Standard);
        let drawn = it.theme.expect("drawn");
        assert_eq!(
            drawn.value("state.failed"),
            built_in_for(tag).value("state.failed")
        );
        let held = it.held.expect("held");
        assert_eq!(held.tokens, ["state.failed"]);
        assert!(held.theme.starts_with("Duo ("), "{}", held.theme);
    }
}

#[test]
fn a_theme_of_one_colour_everywhere_is_drawn_clearing_every_consent_floor() {
    for (appearance, grey) in [
        ("dark", "#808080"),
        ("light", "#808080"),
        ("dark", "#000000"),
    ] {
        let tokens: serde_json::Map<String, Value> = vocabulary::TOKENS
            .iter()
            .map(|token| ((*token).to_owned(), json!(grey)))
            .collect();
        let resolved = load(&json!({ "name": "Grey", "appearance": appearance, "tokens": tokens }));
        assert!(!resolved.refused, "{:?}", resolved.complaints);
        let drawn = draw(resolved.theme.first());
        let held = drawn.held.expect("a one-colour theme breaks the floors");
        assert_eq!(
            held.floors,
            [
                Floor::Ask,
                Floor::AllowApart,
                Floor::Danger,
                Floor::DangerApart,
                Floor::StateMarks,
                Floor::AllowNotPrimary
            ]
        );
        let drawn = drawn.theme.expect("drawn");
        assert_eq!(consent_failures(&drawn), [], "{appearance} {grey}");
        assert_eq!(draw(&drawn).held, None, "{appearance} {grey}");
    }
}

#[test]
fn a_source_s_floors_notice_is_gone_once_what_it_draws_clears_them() {
    let drawn_from = |mine: &Theme| {
        resolve(
            &Inputs {
                pick: None,
                theirs: Some(mine),
                on: On::Every,
                system: Appearance::Dark,
                contrast: Contrast::Standard,
            },
            || unreachable!("no survey when the person has a theme"),
        )
    };
    let broken = theme("Mine", json!({ "state.failed": "#191917" }));
    assert!(drawn_from(&broken).held.is_some());
    let mended = theme("Mine", json!({ "state.failed": "#ff6b6b" }));
    let it = drawn_from(&mended);
    assert_eq!(it.held, None);
    assert_eq!(it.theme.expect("drawn").value("state.failed"), "#ff6b6b");
}

#[test]
fn faded_words_are_measured_at_the_opacity_they_are_drawn() {
    // Words that clear the floor solid and not at four-fifths opacity are held.
    let mine = theme("Faint", json!({ "text.primary": "#9a9a95" }));
    let faded = PAIRS
        .iter()
        .find(|pair| pair.fade.is_some())
        .expect("a faded pair");
    assert_eq!(
        (faded.front, faded.back),
        ("text.primary", "surface.overlay")
    );
    let (drawn, held) = drawn_of(&mine);
    assert_eq!(drawn.value("text.primary"), ours("text.primary"));
    assert_eq!(held.expect("held").floors, [Floor::Ask]);
}

#[test]
fn allow_and_keep_blocked_are_held_apart_by_the_allow_s_edge_and_its_ground() {
    // Allow and Keep blocked are equal in weight on `control.base`: Allow behind an
    // `accent.base` edge, Keep blocked behind a `border.control` one.
    let held: Vec<(&str, &str)> = floors::pairs()
        .filter(|(floor, _)| *floor == Floor::AllowApart)
        .map(|(_, pair)| (pair.front, pair.back))
        .collect();
    assert_eq!(
        held,
        [
            ("accent.base", "control.base"),
            ("accent.base", "control.hover")
        ]
    );
    for appearance in [Appearance::Dark, Appearance::Light] {
        assert!(floors::allow_apart(built_in_for(appearance)) >= ALLOW_APART);
    }
}

#[test]
fn an_allow_edged_in_keep_blocked_s_colour_is_drawn_with_purlis_s_accent() {
    let mine = theme(
        "Slate",
        json!({ "accent.base": ours("border.control"), "text.primary": "#eeeeee" }),
    );
    let (drawn, held) = drawn_of(&mine);
    assert_eq!(drawn.value("accent.base"), ours("accent.base"));
    assert_eq!(drawn.value("text.primary"), "#eeeeee");
    let held = held.expect("the floor was held");
    assert_eq!(held.floors, [Floor::AllowApart]);
    assert_eq!(held.tokens, ["accent.base"]);
    assert!(
        held.said()
            .contains("Allow is distinguishable from Keep blocked")
    );
    assert!(floors::allow_apart(&drawn) >= ALLOW_APART);
    assert_eq!(draw(&drawn).held, None);
}

#[test]
fn an_allow_whose_edge_vanishes_on_the_control_s_ground_is_drawn_with_purlis_s_accent() {
    // The accent drawn in the control's own colour: Allow would look as plain as Keep blocked.
    let mine = theme("Hidden", json!({ "accent.base": ours("control.base") }));
    let (drawn, held) = drawn_of(&mine);
    assert_eq!(drawn.value("accent.base"), ours("accent.base"));
    assert_eq!(drawn.value("control.base"), ours("control.base"));
    let held = held.expect("the floor was held");
    assert_eq!(held.floors, [Floor::AllowApart]);
    assert_eq!(held.tokens, ["accent.base"]);
    assert_eq!(consent_failures(&drawn), []);
}

#[test]
fn an_answer_s_words_lost_on_its_hovered_button_are_purlis_s() {
    let mine = theme("Hover", json!({ "control.hover": ours("text.primary") }));
    let (drawn, held) = drawn_of(&mine);
    assert_eq!(drawn.value("control.hover"), ours("control.hover"));
    let held = held.expect("the floor was held");
    assert!(held.floors.contains(&Floor::Ask), "{:?}", held.floors);
    assert_eq!(consent_failures(&drawn), []);
}

#[test]
fn the_needs_you_mark_is_held_on_a_hovered_control() {
    // The title bar's needs-you hand on its button under the pointer.
    let pair = PAIRS
        .iter()
        .find(|pair| (pair.front, pair.back) == ("needs-you.base", "control.hover"))
        .expect("the pair is measured");
    assert_eq!(pair.floor, 3.0);
    assert!(
        floors::pairs().any(|(floor, one)| floor == Floor::StateMarks && one == pair),
        "held by the consent floors"
    );
    for appearance in [Appearance::Dark, Appearance::Light] {
        let classic = built_in_for(appearance);
        assert!(
            check(classic)
                .findings
                .iter()
                .all(|one| !matches!(one, Finding::Contrast { pair: found, .. } if found == pair)),
            "{appearance:?}"
        );
    }
    // Classic's dark mark before it was lifted for this ground: 2.79:1 on its hovered control.
    let mine = theme("Before", json!({ "needs-you.base": "#b85050" }));
    let (drawn, held) = drawn_of(&mine);
    assert_eq!(drawn.value("needs-you.base"), ours("needs-you.base"));
    let held = held.expect("the floor was held");
    assert_eq!(held.floors, [Floor::StateMarks]);
    assert_eq!(held.tokens, ["needs-you.base"]);
}

#[test]
fn a_see_through_ask_on_a_tinted_control_is_measured_at_every_hue() {
    // An ask's words written see-through on a dark control the recipe turns: the screen mixes the
    // two, so their contrast moves with the hue, and the floors sweep the pair.
    let resolved = load(&json!({
        "name": "Veiled",
        "tint": { "tokens": ["control.base"] },
        "appearances": [{
            "id": "dark", "appearance": "dark",
            "tokens": { "text.primary": "#ffffffcc", "control.base": "#262623" }
        }]
    }));
    assert!(!resolved.refused, "{:?}", resolved.complaints);
    let theme = resolved.theme.first();
    let pair = check::PAIRS
        .iter()
        .find(|pair| {
            (pair.front, pair.back) == ("text.primary", "control.base")
                && pair.under.is_none()
                && pair.fade.is_none()
        })
        .expect("an ask's words are held on its answers' buttons");
    assert!(floors::moves_with_hue(theme, pair));
}

#[test]
fn a_selection_over_the_wash_moves_with_a_tint_that_turns_only_the_dialog_under_both() {
    // A see-through selection over a see-through wash, over a dialog the recipe turns: the dialog
    // is the bottom layer (#2724), so the pair's contrast moves with the hue though neither layer
    // above it is turned, and the floors sweep it.
    let resolved = load(&json!({
        "name": "Deep",
        "tint": { "tokens": ["surface.overlay"] },
        "appearances": [{
            "id": "dark", "appearance": "dark",
            "tokens": {
                "selection.background": "#a99cf055",
                "selection.text": "#ffffff",
                "danger.wash": "#f0a9a81f",
            }
        }]
    }));
    assert!(!resolved.refused, "{:?}", resolved.complaints);
    let theme = resolved.theme.first();
    assert_eq!(theme.tint.tabs(), ["surface.overlay"]);
    let pair = check::PAIRS
        .iter()
        .find(|pair| pair.under == Some("danger.wash") && pair.beneath == Some("surface.overlay"))
        .expect("the selection on the danger wash over the dialog is held");
    assert!(floors::moves_with_hue(theme, pair));
}

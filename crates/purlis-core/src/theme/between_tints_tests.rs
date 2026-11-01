// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A pair that falls only between the named tints** (#4308): a workspace's own colour can be any
//! hue, so the check reads each pair a tint can move at every whole degree. A control falls as
//! written, so a sweep that reads nothing cannot pass.

use serde_json::{Value, json};

use super::super::parts::{self, COARSE_DEGREES, NEAR_A_FLOOR};
use super::super::vocabulary::TINTABLE;
use super::super::{
    ResolvedTheme, Theme, built_ins, check, check_every, load, tint, warnings_said,
};
use super::{
    CARD_STEP, Finding, NEAR_A_STEP, NEAR_IN_OKLAB, PAIRS, Pair, card_step, floors, ratio, standing,
};

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
            .filter(|one| {
                matches!(
                    one,
                    Finding::ContrastBetweenTints { .. }
                        | Finding::SelectedBetweenTints { .. }
                        | Finding::CardBetweenTints { .. }
                        | Finding::NoticeWayBetweenTints { .. }
                )
            })
            .collect();
        assert!(between.is_empty(), "{} {between:?}", theme.id);
    }
}

// ---------------------------------------------------------------------------------------
// What must stand apart and is not a pair (#4406)
// ---------------------------------------------------------------------------------------

/// `tokens` and `parts` on Purlis Classic's dark, tinted by `recipe`.
fn with_parts(tokens: &Value, parts: &Value, recipe: &[&str]) -> ResolvedTheme {
    let loaded = load(&json!({
        "name": "Between", "extends": "purlis-classic",
        "appearances": [{ "id": "dark", "tokens": tokens }],
        "parts": parts,
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

/// Halfway across the widest gap between two named hues, teal's and blue's.
const FAR_FROM_A_NAME: f64 = 222.0;

/// `token` as a recipe that turns it draws it at [`FAR_FROM_A_NAME`], given `written`.
fn drawn_far_from_a_name(token: &str, written: &str, recipe: &[&str]) -> String {
    let theme = with_parts(&json!({ token: written }), &json!({}), recipe);
    tint::tinted_at(&theme, FAR_FROM_A_NAME)
        .value(token)
        .to_owned()
}

const ROW: &str = "#2a2a2a";

/// The selected row in [`ROW`], turned by the recipe, with `surface.hover` written as given.
fn selected_beside(hover: &str) -> ResolvedTheme {
    with_parts(
        &json!({ "list.selected": ROW, "surface.hover": hover }),
        &json!({}),
        &["list.selected"],
    )
}

/// What the check says of the row and the pointer's: (found as written or named, degree between).
fn selected_said(theme: &ResolvedTheme) -> (bool, Option<u16>) {
    let findings = check(theme).findings;
    let named = findings.iter().any(
        |one| matches!(one, Finding::SelectedNotApart { other, .. } if *other == "surface.hover"),
    );
    let between = findings.iter().find_map(|one| match one {
        Finding::SelectedBetweenTints { degree, other } if *other == "surface.hover" => {
            Some(*degree)
        }
        _ => None,
    });
    (named, between)
}

#[test]
fn a_selected_row_can_meet_the_pointer_s_only_between_the_named_tints() {
    let hover = drawn_far_from_a_name("list.selected", ROW, &["list.selected"]);
    let theme = selected_beside(&hover);
    let (named, between) = selected_said(&theme);
    assert!(!named, "{hover}");
    let degree = between.unwrap_or_else(|| panic!("not said, at {hover}"));
    assert!(
        f64::from(degree).max(FAR_FROM_A_NAME) - f64::from(degree).min(FAR_FROM_A_NAME) <= 5.0,
        "{degree}"
    );

    let loaded = load(&json!({
        "name": "Between", "extends": "purlis-classic",
        "appearances": [{ "id": "dark", "tokens": { "list.selected": ROW, "surface.hover": hover } }],
        "tint": { "tokens": ["list.selected"] },
    }));
    let said = warnings_said(&loaded.theme, &check_every(&loaded.theme));
    assert!(
        said.iter().any(|line| line.contains(
            "list.selected is drawn exactly as surface.hover, in a workspace's own colour at"
        )),
        "{said:?}"
    );
}

#[test]
fn a_selected_row_equal_as_written_is_said_as_it_was_and_not_again_between_the_tints() {
    // The control: it falls as written, so a sweep that read nothing cannot pass the case above's
    // negatives. The tints turn the row away from it, so only the theme as written says it.
    assert_eq!(selected_said(&selected_beside(ROW)), (true, None));
}

#[test]
fn a_selected_row_far_from_the_pointer_s_is_not_said() {
    assert_eq!(selected_said(&selected_beside("#ff0000")), (false, None));
}

/// A vivid news card whose ground the recipe turns. It keeps its chroma, so a person sees it move
/// between the named hues.
const VIVID: &str = "#c04040";

/// A theme whose secondary button is filled with `fill`, on the [`VIVID`] card the recipe turns.
fn way_beside(fill: &str) -> ResolvedTheme {
    with_parts(
        &json!({ "surface.overlay": VIVID, "surface.hover": fill }),
        &json!({ "button.root.secondary.fill": "surface.hover" }),
        &["surface.overlay"],
    )
}

/// What the check says of a news card's way at rest: (found as written or named, between).
fn way_said(theme: &ResolvedTheme) -> (bool, Option<(u16, f64)>) {
    let findings = check(theme).findings;
    let named = findings.iter().any(|one| {
        matches!(
            one,
            Finding::NoticeWayNotApart {
                variant: "news",
                pointed: false,
                ..
            }
        )
    });
    let between = findings.iter().find_map(|one| match one {
        Finding::NoticeWayBetweenTints {
            variant: "news",
            pointed: false,
            degree,
            distance,
            ..
        } => Some((*degree, *distance)),
        _ => None,
    });
    (named, between)
}

#[test]
fn a_notice_way_can_meet_its_card_only_between_the_named_tints() {
    let fill = drawn_far_from_a_name("surface.overlay", VIVID, &["surface.overlay"]);
    let (named, between) = way_said(&way_beside(&fill));
    assert!(!named, "{fill}");
    let (degree, distance) = between.unwrap_or_else(|| panic!("not said, at {fill}"));
    assert!(
        f64::from(degree).max(FAR_FROM_A_NAME) - f64::from(degree).min(FAR_FROM_A_NAME) <= 5.0,
        "{degree}"
    );
    assert!(distance < parts::SEEN, "{distance}");
}

#[test]
fn a_notice_way_on_its_card_as_written_is_said_as_it_was_and_not_again_between_the_tints() {
    // The control: a way that falls as written.
    assert_eq!(way_said(&way_beside(VIVID)).1, None);
    assert!(way_said(&way_beside(VIVID)).0);
}

#[test]
fn a_notice_way_far_from_its_card_is_not_said() {
    assert_eq!(way_said(&way_beside("#10e0e0")), (false, None));
}

/// A card row in grey `card`, turned by the recipe, on a panel in grey `panel`.
fn card_on(panel: u8, card: u8) -> ResolvedTheme {
    let grey = |one: u8| format!("#{one:02x}{one:02x}{one:02x}");
    with_parts(
        &json!({ "surface.raised": grey(card), "surface.sunken": grey(panel) }),
        &json!({ "list.root.card.fill": "surface.raised", "panel.root.fill": "surface.sunken" }),
        &["surface.raised"],
    )
}

fn card_step_at(theme: &ResolvedTheme, drawn: &ResolvedTheme) -> f64 {
    card_step(drawn, parts::card_on_side(theme).unwrap()).0
}

#[test]
fn a_card_row_s_step_can_fall_only_between_the_named_tints_and_is_said_when_it_does() {
    // The tint keeps a shade's luminance, so all that moves the step is the rounding of a turned
    // shade: a card a hair over the floor. Each panel's nearest such card is read against what the
    // check says, and at least one of them falls.
    let mut fell = 0;
    for panel in (0x20..=0x50_u8).step_by(3) {
        let card = (panel..=0xff)
            .find(|card| card_step_at(&card_on(panel, *card), &card_on(panel, *card)) >= CARD_STEP)
            .unwrap();
        let theme = card_on(panel, card);
        let named = theme
            .tint
            .hues
            .iter()
            .map(|one| card_step_at(&theme, &tint::tinted(&theme, one.name)))
            .fold(f64::INFINITY, f64::min);
        let lowest = (0..360)
            .map(|degree| card_step_at(&theme, &tint::tinted_at(&theme, f64::from(degree))))
            .fold(f64::INFINITY, f64::min);
        let said = check(&theme)
            .findings
            .into_iter()
            .find_map(|one| match one {
                Finding::CardBetweenTints { ratio, .. } => Some(ratio),
                _ => None,
            });
        let falls_between = named >= CARD_STEP && lowest < CARD_STEP;
        assert_eq!(
            said,
            falls_between.then_some(lowest),
            "panel {panel:x} card {card:x}: named {named}, lowest {lowest}"
        );
        fell += usize::from(falls_between);
    }
    assert!(fell > 0, "no card row fell between the named tints");
}

#[test]
fn a_card_row_on_its_panel_as_written_is_said_as_it_was_and_not_again_between_the_tints() {
    // The control: a step that falls as written.
    let found = check(&card_on(0x30, 0x30)).findings;
    assert!(
        found
            .iter()
            .any(|one| matches!(one, Finding::CardNotApart { tint: None, .. })),
        "{found:?}"
    );
    assert!(
        !found
            .iter()
            .any(|one| matches!(one, Finding::CardBetweenTints { .. })),
        "{found:?}"
    );
}

#[test]
fn what_stands_apart_has_a_coarse_reading_within_its_margin_of_the_lowest_at_any_degree() {
    // What lets the sweep leave one at its coarse reading, on the widest recipe and a vivid card.
    let vivid = with_parts(
        &json!({ "surface.overlay": VIVID, "surface.hover": "#40c040" }),
        &json!({ "button.root.secondary.fill": "surface.hover" }),
        &["surface.overlay"],
    );
    let (mut steps, mut oklab): (f64, f64) = (0.0, 0.0);
    for theme in widest().into_iter().chain([vivid]) {
        let card = parts::card_on_side(&theme);
        let ways = parts::notice_ways(&theme);
        let reads: Vec<_> = (0..360)
            .map(|degree| standing(&tint::tinted_at(&theme, f64::from(degree)), card, ways))
            .collect();
        for at in 0..reads[0].len() {
            let lowest = |step: usize| {
                reads
                    .iter()
                    .step_by(step)
                    .map(|one| one[at].value)
                    .fold(f64::INFINITY, f64::min)
            };
            let gap = lowest(COARSE_DEGREES) - lowest(1);
            if format!("{:?}", reads[0][at].what).starts_with("Card") {
                steps = steps.max(gap);
            } else {
                oklab = oklab.max(gap);
            }
        }
    }
    assert!(steps > 0.0 && oklab > 0.0, "{steps} {oklab}");
    assert!(steps < NEAR_A_STEP, "{steps}");
    assert!(oklab < NEAR_IN_OKLAB, "{oklab}");
}

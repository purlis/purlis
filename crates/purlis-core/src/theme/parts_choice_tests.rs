// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The choice families at the engine's interface (#2907): the checkbox, the radio, the switch, the
//! segmented control and its segment, the slider and the option card in the manifest, their keys
//! and their stylesheet, a choice reached by its chosen state, no height and no state a wrapper
//! never marks, and their colours held to `app/src/ui/choice/choice.css`.

use serde_json::{Value, json};

use super::parts::{self, State};
use super::*;

/// A theme over Purlis Classic that sets `parts`.
fn with_parts(parts: Value) -> Value {
    json!({ "name": "Shaped", "extends": "purlis-classic", "parts": parts })
}

fn said(resolved: &Resolved) -> Vec<String> {
    resolved
        .complaints
        .iter()
        .map(|one| one.said.clone())
        .collect()
}

/// The stylesheet of a theme's first appearance, which must be drawn.
fn sheet_of(parts: Value) -> String {
    let loaded = load(&with_parts(parts));
    assert!(!loaded.refused, "{:?}", said(&loaded));
    parts::stylesheet(loaded.theme.first())
}

/// The families this ticket adds, with their parts and their variants, in the manifest's order.
const CHOICES: &[(&str, &[&str], &[&str])] = &[
    (
        "checkbox",
        &["root", "box", "label", "description"],
        &["box", "chip"],
    ),
    ("radio", &["root", "mark", "label", "description"], &[]),
    ("switch", &["root", "track", "thumb", "label"], &[]),
    ("segmented-control", &["root"], &[]),
    ("segment", &["root", "label", "count"], &[]),
    ("slider", &["root", "track", "fill", "thumb", "output"], &[]),
    (
        "option-card",
        &["root", "icon", "title", "detail", "check"],
        &["card", "tile"],
    ),
];

// ---------------------------------------------------------------------------------------
// The manifest
// ---------------------------------------------------------------------------------------

#[test]
fn the_choice_families_are_in_the_manifest_since_next() {
    let manifest = parts::manifest_json();
    let families = manifest["families"].as_array().unwrap();
    for (name, parts_named, variants) in CHOICES {
        let family = families
            .iter()
            .find(|one| one["name"] == *name)
            .unwrap_or_else(|| panic!("{name} is in the manifest"));
        assert_eq!(family["since"], parts::NEXT, "{name}");
        let names = |list: &str| -> Vec<String> {
            family[list]
                .as_array()
                .unwrap()
                .iter()
                .map(|one| one["name"].as_str().unwrap().to_owned())
                .collect()
        };
        assert_eq!(names("parts"), *parts_named, "{name}");
        assert_eq!(names("variants"), *variants, "{name}");
        // No choice draws an answer: none carries a consent.
        for variant in family["variants"].as_array().unwrap() {
            assert_eq!(variant["consent"], false, "{name}");
        }
    }
}

// ---------------------------------------------------------------------------------------
// Keys and the stylesheet
// ---------------------------------------------------------------------------------------

#[test]
fn a_choice_is_reached_by_its_chosen_state_and_its_mark_inside_its_row() {
    let sheet = sheet_of(json!({
        "checkbox.box.selected.fill": "accent.surface",
        "switch.track.selected.fill": "accent.surface",
        "segment.root.selected.fill": "surface.raised",
        "option-card.root.card.selected.border-colour": "accent.base",
    }));
    for reached in [
        ":where([data-scope=\"checkbox\"][data-part=\"root\"][data-selected] [data-scope=\"checkbox\"][data-part=\"box\"]) { background-color: var(--accent-surface); }",
        ":where([data-scope=\"switch\"][data-part=\"root\"][data-selected] [data-scope=\"switch\"][data-part=\"track\"]) { background-color: var(--accent-surface); }",
        ":where([data-scope=\"segment\"][data-part=\"root\"][data-selected]) { background-color: var(--surface-raised); }",
        ":where([data-scope=\"option-card\"][data-part=\"root\"][data-variant=\"card\"][data-selected]) { border-color: var(--accent-base); }",
    ] {
        assert!(sheet.contains(reached), "{reached}\n{sheet}");
    }
}

#[test]
fn a_mark_s_corner_is_clamped_to_half_its_own_box() {
    // A box, a track and a thumb are as tall as the family draws them: `full` is half of each.
    let sheet = sheet_of(json!({
        "checkbox.box.corner-radius": "full",
        "switch.thumb.corner-radius": 2,
    }));
    assert!(
        sheet.contains(
            ":where([data-scope=\"checkbox\"][data-part=\"root\"] [data-scope=\"checkbox\"][data-part=\"box\"]) { border-radius: 50%; }"
        ),
        "{sheet}"
    );
    assert!(
        sheet.contains(
            ":where([data-scope=\"switch\"][data-part=\"root\"] [data-scope=\"switch\"][data-part=\"thumb\"]) { border-radius: min(2px, 50%); }"
        ),
        "{sheet}"
    );
}

#[test]
fn a_choice_takes_no_height_and_no_state_its_wrapper_never_marks() {
    let loaded = load(&with_parts(json!({
        "switch.root.height": 32,
        "radio.root.pressed.text": "accent.base",
        "segment.root.open.fill": "accent.surface",
        "slider.root.hover.text": "accent.base",
        "segmented-control.root.selected.fill": "accent.surface",
        "checkbox.box.chip.selected.fill": "accent.surface",
    })));
    assert!(!loaded.refused, "{:?}", said(&loaded));
    for key in [
        "switch.root.height",
        "radio.root.pressed.text",
        "segment.root.open.fill",
        "slider.root.hover.text",
        "segmented-control.root.selected.fill",
    ] {
        assert!(
            said(&loaded)
                .iter()
                .any(|one| one.contains(&format!("parts.{key} is not"))),
            "{key}: {:?}",
            said(&loaded)
        );
    }
    // The one key a choice has is drawn; the theme it extends sets no choice of its own.
    let choices: Vec<_> = loaded
        .theme
        .first()
        .parts
        .keys()
        .filter(|key| CHOICES.iter().any(|(name, _, _)| *name == key.family))
        .collect();
    assert_eq!(choices.len(), 1, "{choices:?}");
}

#[test]
fn a_tick_never_changes_a_size() {
    // A state sets colours only: a box ticked, a segment chosen, a card picked keep their size.
    for key in [
        "checkbox.box.selected.border-width",
        "segment.root.selected.corner-radius",
        "option-card.root.selected.padding",
        "option-card.title.selected.font-weight",
    ] {
        let loaded = load(&with_parts(json!({ key: 2 })));
        assert!(loaded.refused, "{key} was drawn");
        assert!(
            said(&loaded)
                .iter()
                .any(|one| one.contains("a state never changes")),
            "{key}: {:?}",
            said(&loaded)
        );
    }
}

#[test]
fn a_card_s_words_that_break_their_floor_draw_the_family_default() {
    let loaded = load(&with_parts(
        json!({ "option-card.title.card.text": "surface.base" }),
    ));
    assert!(!loaded.refused, "{:?}", said(&loaded));
    let fell = parts::fallen(loaded.theme.first());
    assert!(
        fell.iter()
            .any(|one| one.part.starts_with("option-card.root.card")),
        "{fell:?}"
    );
}

// ---------------------------------------------------------------------------------------
// The defaults, held to choice.css
// ---------------------------------------------------------------------------------------

/// The stylesheet with prettier's wrapping of a long selector read back onto one line, and no
/// comments.
fn css() -> String {
    let text = include_str!("../../../../app/src/ui/choice/choice.css");
    let mut out = String::new();
    let mut rest = text;
    while let Some(at) = rest.find("/*") {
        out.push_str(&rest[..at]);
        rest = rest[at..]
            .find("*/")
            .map_or("", |end| &rest[at + end + 2..]);
    }
    out.push_str(rest);
    out.replace("(\n    ", "(").replace("\n  ) {", ") {")
}

/// The declarations of the rule `selector` opens in `css`.
fn rule<'a>(css: &'a str, selector: &str) -> &'a str {
    let at = css
        .rfind(&format!("\n{selector} {{"))
        .unwrap_or_else(|| panic!("choice.css has no rule {selector}"));
    let body = &css[at..];
    &body[..body.find('}').unwrap()]
}

fn declared<'a>(body: &'a str, property: &str) -> Option<&'a str> {
    body.lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix(&format!("{property}: ")))
        .map(|value| value.trim_end_matches(';'))
}

/// The token a declared value names (`var(--surface-base)` is `surface.base`), and `transparent`
/// for a value that says it or for none.
fn token(value: Option<&str>) -> String {
    let Some(value) = value.map(|it| it.rsplit(' ').next().unwrap_or(it)) else {
        return "transparent".to_owned();
    };
    if value == "transparent" {
        return value.to_owned();
    }
    let name = value
        .strip_prefix("var(--")
        .and_then(|it| it.strip_suffix(')'))
        .unwrap_or_else(|| panic!("{value} names no token"));
    vocabulary::TOKENS
        .iter()
        .find(|token| token.replace('.', "-") == name)
        .unwrap_or_else(|| panic!("--{name} is not a token"))
        .to_string()
}

/// A rule's colours: fill, words and edge, each as the family's look names them.
fn drawn(css: &str, selector: &str) -> [String; 3] {
    let body = rule(css, selector);
    [
        token(declared(body, "background-color")),
        token(declared(body, "color")),
        token(declared(body, "border").or_else(|| declared(body, "border-color"))),
    ]
}

fn held(family: &str, variant: &str, states: &[State]) -> [String; 3] {
    parts::look_of(family, variant, states).map(str::to_owned)
}

#[test]
fn the_choice_families_defaults_are_what_choice_css_draws() {
    let css = css();
    let root = |scope: &str| format!("[data-scope=\"{scope}\"][data-part=\"root\"]");
    // Each family's root at rest.
    for (family, variant, selector) in [
        ("checkbox", "box", root("checkbox")),
        (
            "checkbox",
            "chip",
            format!("{}[data-variant=\"chip\"]", root("checkbox")),
        ),
        ("radio", "", root("radio")),
        ("switch", "", root("switch")),
        ("segmented-control", "", root("segmented-control")),
        ("segment", "", root("segment")),
        ("option-card", "card", root("option-card")),
    ] {
        assert_eq!(
            held(family, variant, &[]),
            drawn(&css, &selector),
            "{family} {variant}"
        );
    }
    // A slider's row has no ground of its own and its value is in the secondary words.
    let slider = rule(&css, &root("slider"));
    assert_eq!(held("slider", "", &[])[1], token(declared(slider, "color")));
    assert_eq!(declared(slider, "background-color"), None);
    // A tile is a card with no ground of its own, in the muted words.
    let tile = rule(
        &css,
        &format!("{}[data-variant=\"tile\"]", root("option-card")),
    );
    assert_eq!(
        held("option-card", "tile", &[])[..2],
        [
            token(declared(tile, "background-color")),
            token(declared(tile, "color"))
        ]
    );
    // Chosen: a chip on the accent's wash, a segment on the window's ground, a card on the wash
    // behind the accent's edge, each in the primary words.
    let chosen = [State::Selected];
    let chip = rule(
        &css,
        &format!("{}[data-variant=\"chip\"][data-selected]", root("checkbox")),
    );
    assert_eq!(
        held("checkbox", "chip", &chosen)[..2],
        [
            token(declared(chip, "background-color")),
            token(declared(chip, "color"))
        ]
    );
    assert_eq!(
        held("segment", "", &chosen)[..2],
        drawn(&css, &format!("{}[data-selected]", root("segment")))[..2]
    );
    assert_eq!(
        held("option-card", "card", &chosen),
        drawn(&css, &format!("{}[data-selected]", root("option-card")))
    );
    // Under the pointer: a segment on the control's hover in the primary words.
    let hover = rule(
        &css,
        &format!("{}[data-hovered]:not([data-disabled])", root("segment")),
    );
    assert_eq!(
        held("segment", "", &[State::Hover])[..2],
        [
            token(declared(hover, "background-color")),
            token(declared(hover, "color"))
        ]
    );
}

#[test]
fn a_choice_s_quieter_words_are_the_tiers_choice_css_draws() {
    let css = css();
    for (family, part) in [
        ("checkbox", "description"),
        ("radio", "description"),
        ("option-card", "detail"),
        ("option-card", "check"),
    ] {
        let drawn = rule(
            &css,
            &format!("[data-scope=\"{family}\"][data-part=\"{part}\"]"),
        );
        let family_of = parts::family(family).expect("a choice family");
        let at = family_of
            .parts
            .iter()
            .position(|one| one.name == part)
            .unwrap();
        assert_eq!(
            family_of.own_words_of(at).map(str::to_owned),
            Some(token(declared(drawn, "color"))),
            "{family}.{part}"
        );
    }
}

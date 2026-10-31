// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A card pressed whole at the engine's interface (#3885): in the manifest with its parts, its
//! variants and its states, a theme's keys reaching its box, its edge, its title and each state,
//! a card's title never reaching its title, a part it does not have said, the defaults held to
//! `card.css`, its pairs held, and both built-ins drawing the default.

use serde_json::{Value, json};

use super::parts;
use super::*;

fn over(base: &str, parts: Value) -> Value {
    json!({ "name": "Shaped", "extends": base, "parts": parts })
}

fn said(resolved: &Resolved) -> Vec<String> {
    resolved
        .complaints
        .iter()
        .map(|one| one.said.clone())
        .collect()
}

/// The stylesheet a theme over `base` that sets `parts` draws, which must be taken.
fn sheet(base: &str, parts: Value) -> String {
    let loaded = load(&over(base, parts));
    assert!(!loaded.refused, "{:?}", said(&loaded));
    parts::stylesheet(loaded.theme.first())
}

const ROOT: &str = "[data-scope=\"card-button\"][data-part=\"root\"]";
const TITLE: &str = "[data-scope=\"card-button\"][data-part=\"title\"]";

#[test]
fn a_pressed_card_comes_in_the_next_release_s_manifest() {
    let family = parts::manifest_json()["families"]
        .as_array()
        .unwrap()
        .iter()
        .find(|one| one["name"] == "card-button")
        .expect("a pressed card is in the manifest")
        .clone();
    assert_eq!(family["since"], parts::NEXT);
    let names = |list: &str| -> Vec<String> {
        family[list]
            .as_array()
            .unwrap()
            .iter()
            .map(|one| one["name"].as_str().unwrap().to_owned())
            .collect()
    };
    assert_eq!(names("parts"), ["root", "mark", "title", "line"]);
    assert_eq!(names("variants"), ["door", "pick"]);
    for part in family["parts"].as_array().unwrap() {
        assert_eq!(part["since"], parts::NEXT, "{part}");
    }
    // At rest, under the pointer, pressed, with the keyboard's focus and unavailable; never
    // chosen.
    assert_eq!(
        family["states"],
        json!(["hover", "pressed", "focused", "disabled"])
    );
}

#[test]
fn a_theme_restyles_its_ground_its_edge_and_its_title() {
    let sheet = sheet(
        PURLIS,
        json!({
            "card-button.root.door.fill": "surface.raised",
            "card-button.root.door.border-colour": "border.strong",
            "card-button.title.font-weight": 600,
        }),
    );
    assert!(
        sheet.contains(&format!(
            ":where({ROOT}[data-variant=\"door\"]) {{ background-color: var(--surface-raised); border-color: var(--border-strong); }}"
        )),
        "{sheet}"
    );
    assert!(
        sheet.contains(&format!(":where({ROOT} {TITLE}) {{ font-weight: 600; }}")),
        "{sheet}"
    );
}

#[test]
fn each_state_reaches_the_box_its_button_says_it_of() {
    for (state, marked) in [
        ("hover", "[data-hovered]:not([data-disabled])"),
        ("pressed", "[data-pressed]:not([data-disabled])"),
        ("focused", "[data-focus-visible]"),
        ("disabled", "[data-disabled]"),
    ] {
        let sheet = sheet(
            PURLIS,
            json!({ format!("card-button.root.door.{state}.border-colour"): "border.strong" }),
        );
        assert!(
            sheet.contains(&format!(
                ":where({ROOT}[data-variant=\"door\"]{marked}) {{ border-color: var(--border-strong); }}"
            )),
            "{state}: {sheet}"
        );
    }
}

#[test]
fn a_card_s_title_never_reaches_a_pressed_card_s_title() {
    // #3834's review: inside a `<Card>`, a theme's `card.title` would have won over a pressed
    // card's own title. They are two families, so each title is reached by its own scope alone,
    // and neither rule reaches the other's element wherever it stands.
    let sheet = sheet(
        PURLIS,
        json!({
            "card.title.text": "text.secondary",
            "card-button.title.text": "text.primary",
        }),
    );
    let card =
        "[data-scope=\"card\"][data-part=\"root\"] [data-scope=\"card\"][data-part=\"title\"]";
    assert!(
        sheet.contains(&format!(
            ":where({card}) {{ color: var(--text-secondary); }}"
        )),
        "{sheet}"
    );
    assert!(
        sheet.contains(&format!(
            ":where({ROOT} {TITLE}) {{ color: var(--text-primary); }}"
        )),
        "{sheet}"
    );
    for line in sheet
        .lines()
        .filter(|line| line.contains("--text-secondary"))
    {
        assert!(!line.contains("card-button"), "{line}");
    }
}

#[test]
fn a_state_a_variant_or_a_part_it_does_not_have_is_said() {
    for key in [
        "card-button.root.selected.fill",
        "card-button.root.tile.fill",
        "card-button.more.text",
        "card-button.root.hover.padding",
        "card-button.mark.fill",
    ] {
        let loaded = load(&over(PURLIS, json!({ key: "surface.raised" })));
        assert!(!said(&loaded).is_empty(), "{key} was taken");
    }
}

#[test]
fn a_pick_whose_words_cannot_be_read_draws_the_default() {
    let loaded = load(&over(
        PURLIS,
        json!({ "card-button.root.pick.text": "surface.base" }),
    ));
    assert!(!loaded.refused, "{:?}", said(&loaded));
    let fell = parts::fallen(loaded.theme.first());
    assert!(
        fell.iter().any(|one| one.part.starts_with("card-button")),
        "{fell:?}"
    );
}

/// The token a rule `selector` opens in `css` gives `property`.
fn token(css: &str, selector: &str, property: &str) -> String {
    let at = css
        .find(&format!("\n{selector} {{"))
        .unwrap_or_else(|| panic!("no rule {selector}"));
    let body = &css[at..];
    let body = &body[..body.find('}').unwrap()];
    let value = body
        .lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix(&format!("{property}: ")))
        .unwrap_or_else(|| panic!("{selector} sets no {property}"))
        .trim_end_matches(';');
    let name = value
        .rsplit("var(--")
        .next()
        .and_then(|it| it.strip_suffix(')'))
        .unwrap_or_else(|| panic!("{value} names no token"));
    vocabulary::TOKENS
        .iter()
        .find(|token| token.replace('.', "-") == name)
        .unwrap_or_else(|| panic!("--{name} is not a token"))
        .to_string()
}

#[test]
fn the_defaults_are_what_card_css_draws() {
    let css = include_str!("../../../../app/src/ui/card/card.css");
    let door = format!("{ROOT}[data-variant=\"door\"]");
    let pick = format!("{ROOT}[data-variant=\"pick\"]");
    let look = |variant: &str| parts::look_of("card-button", variant, &[]);
    // A door: the control's ground, the primary words, the subtle hairline.
    assert_eq!(look("door")[0], token(css, &door, "background-color"));
    assert_eq!(look("door")[1], token(css, ROOT, "color"));
    assert_eq!(look("door")[2], token(css, ROOT, "border"));
    // A pick: no ground and no edge of its own, the accent's words.
    assert_eq!(look("pick")[0], "transparent");
    assert_eq!(look("pick")[1], token(css, &pick, "color"));
    assert_eq!(look("pick")[2], "transparent");
    // Unavailable, faint.
    assert_eq!(
        parts::look_of("card-button", "door", &[parts::State::Disabled])[1],
        token(css, &format!("{ROOT}[data-disabled]"), "color")
    );
    // A door's mark in the accent, and its line in the secondary tier.
    let family = parts::MANIFEST
        .iter()
        .find(|one| one.name == "card-button")
        .unwrap();
    for (part, selector) in [
        ("mark", "[data-scope=\"card-button\"][data-part=\"mark\"]"),
        ("line", "[data-scope=\"card-button\"][data-part=\"line\"]"),
    ] {
        let at = family
            .parts
            .iter()
            .position(|one| one.name == part)
            .unwrap();
        assert_eq!(
            family.own_words_of(at).map(str::to_owned),
            Some(token(css, selector, "color")),
            "{part}"
        );
    }
}

#[test]
fn the_pairs_a_pressed_card_draws_are_held() {
    // What `card.css` draws it in, at rest and under the pointer, on each ground it stands on:
    // each a pair the check holds every built-in to
    // (`check_tests::every_built_in_still_clears_the_check`), words at 4.5:1 and marks at 3:1.
    let words = 4.5;
    let mark = 3.0;
    let mut drawn = vec![
        // A door: its title in the primary words and its line in the secondary tier, on the
        // control's ground and its hover; its mark in the accent on both.
        ("text.primary", "control.base", words),
        ("text.primary", "control.hover", words),
        ("text.secondary", "control.base", words),
        ("text.secondary", "control.hover", words),
        ("accent.base", "control.base", mark),
        ("accent.base", "control.hover", mark),
        // A door unavailable: faint, at a mark's floor, on its own ground.
        ("text.faint", "control.base", mark),
        // A pick under the pointer: the accent's words on the hover surface.
        ("accent.base", "surface.hover", words),
    ];
    for ground in ["surface.base", "surface.raised", "surface.overlay"] {
        // A pick at rest: the accent's words and its line in the secondary tier.
        drawn.push(("accent.base", ground, words));
        drawn.push(("text.secondary", ground, words));
    }
    for (front, back, floor) in drawn {
        assert!(
            check::PAIRS.iter().any(|pair| pair.front == front
                && pair.back == back
                && pair.under.is_none()
                && pair.floor >= floor),
            "{front} on {back} at {floor}:1 is held"
        );
    }
}

#[test]
fn both_built_ins_draw_the_family_default_cleanly() {
    for theme in [purlis(), classic()] {
        assert!(
            !theme
                .first()
                .parts
                .keys()
                .any(|key| key.family == "card-button"),
            "a built-in draws a pressed card's default"
        );
        for appearance in &theme.appearances {
            assert!(parts::fallen(appearance).is_empty(), "{}", appearance.id);
        }
    }
}

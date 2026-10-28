// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The view frame and its page navigation at the engine's interface (#3666): in the manifest with
//! their parts, keyed on the wrapper's attributes, a size refused inside a state, a page's
//! colours held to `panel.css`, its floors held, and both built-ins drawing the family default.

use serde_json::{Value, json};

use super::parts;
use super::*;

/// A theme over `base` that sets `parts`.
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

/// A family of the manifest, by name.
fn family_json(name: &str) -> Value {
    parts::manifest_json()["families"]
        .as_array()
        .unwrap()
        .iter()
        .find(|one| one["name"] == name)
        .unwrap_or_else(|| panic!("{name} is in the manifest"))
        .clone()
}

fn names(family: &Value, list: &str) -> Vec<String> {
    family[list]
        .as_array()
        .unwrap()
        .iter()
        .map(|one| one["name"].as_str().unwrap().to_owned())
        .collect()
}

const NAV_ITEM: &str = "[data-scope=\"view-nav\"][data-part=\"item\"]";

// ---------------------------------------------------------------------------------------
// The manifest
// ---------------------------------------------------------------------------------------

#[test]
fn the_view_frame_comes_in_the_next_release_s_manifest() {
    let frame = family_json("view-frame");
    assert_eq!(frame["since"], parts::NEXT);
    assert_eq!(
        names(&frame, "parts"),
        [
            "root",
            "nav",
            "list",
            "item",
            "head",
            "header",
            "name",
            "lead",
            "bar",
            "history",
            "path",
            "crumb",
            "crumb-separator"
        ]
    );
    for part in frame["parts"].as_array().unwrap() {
        assert_eq!(part["since"], parts::NEXT, "{part}");
    }
    assert_eq!(frame["variants"], json!([]));
    assert_eq!(frame["states"], json!([]));
}

#[test]
fn a_page_in_the_navigation_comes_in_the_next_release_s_manifest() {
    let nav = family_json("view-nav");
    assert_eq!(nav["since"], parts::NEXT);
    assert_eq!(names(&nav, "parts"), ["item", "label", "indicator"]);
    assert_eq!(
        nav["states"],
        json!(["hover", "pressed", "selected", "focused"])
    );
}

#[test]
fn a_size_inside_a_state_or_a_part_the_frame_does_not_have_is_said() {
    let loaded = load(&over(
        PURLIS,
        json!({ "view-nav.item.selected.corner-radius": 4 }),
    ));
    assert!(
        said(&loaded)
            .iter()
            .any(|one| one.contains(parts::NO_SIZE_IN_A_STATE)),
        "{:?}",
        said(&loaded)
    );
    for key in ["view-frame.nav.hover.fill", "view-frame.pane.fill"] {
        let loaded = load(&over(PURLIS, json!({ key: "surface.base" })));
        assert!(
            said(&loaded)
                .iter()
                .any(|one| one.contains(&format!("parts.{key} is not a component part"))),
            "{key}: {:?}",
            said(&loaded)
        );
    }
}

// ---------------------------------------------------------------------------------------
// The stylesheet
// ---------------------------------------------------------------------------------------

#[test]
fn a_theme_reaches_the_current_page_and_its_mark() {
    let loaded = load(&over(
        PURLIS,
        json!({
            "view-nav.item.selected.fill": "accent.surface",
            "view-nav.indicator.selected.fill": "accent.text",
            "view-frame.header.padding": "space.5",
        }),
    ));
    assert!(!loaded.refused, "{:?}", said(&loaded));
    let sheet = parts::stylesheet(loaded.theme.first());
    assert!(
        sheet.contains(&format!(
            ":where({NAV_ITEM}[data-selected]) {{ background-color: var(--accent-surface); }}"
        )),
        "{sheet}"
    );
    assert!(
        sheet.contains(&format!(
            ":where({NAV_ITEM}[data-selected] [data-scope=\"view-nav\"][data-part=\"indicator\"]) {{ background-color: var(--accent-text); }}"
        )),
        "{sheet}"
    );
    assert!(
        sheet.contains("[data-scope=\"view-frame\"][data-part=\"header\"]"),
        "{sheet}"
    );
}

#[test]
fn a_current_page_whose_words_cannot_be_read_draws_the_default() {
    let loaded = load(&over(
        PURLIS,
        json!({
            "view-nav.item.selected.fill": "accent.surface",
            "view-nav.item.selected.text": "accent.surface",
        }),
    ));
    assert!(!loaded.refused, "{:?}", said(&loaded));
    let fell = parts::fallen(loaded.theme.first());
    assert!(
        fell.iter().any(|one| one.part.starts_with("view-nav.item")),
        "{fell:?}"
    );
}

// ---------------------------------------------------------------------------------------
// The defaults, held to the stylesheet
// ---------------------------------------------------------------------------------------

/// A stylesheet with its comments taken out.
fn css_of(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(at) = rest.find("/*") {
        out.push_str(&rest[..at]);
        rest = rest[at..]
            .find("*/")
            .map_or("", |end| &rest[at + end + 2..]);
    }
    out.push_str(rest);
    out
}

/// The declarations of the last rule `selector` opens in `css`.
fn rule<'a>(css: &'a str, selector: &str) -> &'a str {
    let at = css
        .rfind(&format!("\n{selector} {{"))
        .unwrap_or_else(|| panic!("no rule {selector}"));
    let body = &css[at..];
    &body[..body.find('}').unwrap()]
}

/// The token `property` names in `body`: `var(--surface-base)` is `surface.base`.
fn token(body: &str, property: &str) -> String {
    let value = body
        .lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix(&format!("{property}: ")))
        .unwrap_or_else(|| panic!("{body} sets no {property}"))
        .trim_end_matches(';');
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

#[test]
fn a_page_s_defaults_are_what_panel_css_draws() {
    let css = css_of(include_str!("../../../../app/src/ui/panel/panel.css"));
    let at_rest = rule(&css, NAV_ITEM);
    let hover = rule(&css, &format!("{NAV_ITEM}[data-hovered]"));
    let chosen = rule(&css, &format!("{NAV_ITEM}[data-selected]"));
    let look = |states: &[parts::State]| parts::look_of("view-nav", "", states);
    assert_eq!(look(&[])[0], token(at_rest, "background-color"));
    assert_eq!(look(&[])[1], token(at_rest, "color"));
    assert_eq!(
        look(&[parts::State::Hover])[0],
        token(hover, "background-color")
    );
    assert_eq!(look(&[parts::State::Hover])[1], token(hover, "color"));
    assert_eq!(
        look(&[parts::State::Selected])[0],
        token(chosen, "background-color")
    );
    assert_eq!(look(&[parts::State::Selected])[1], token(chosen, "color"));
    // Under the pointer the current page keeps its look: its rule comes after the pointer's.
    assert_eq!(
        look(&[parts::State::Hover, parts::State::Selected]),
        look(&[parts::State::Selected])
    );
    assert!(
        css.rfind(&format!("\n{NAV_ITEM}[data-selected] {{"))
            > css.rfind(&format!("\n{NAV_ITEM}[data-hovered] {{"))
    );
    // The lead line's words.
    let lead = rule(&css, "[data-scope=\"view-frame\"][data-part=\"lead\"]");
    assert_eq!(token(lead, "color"), "text.muted");
}

#[test]
fn a_state_of_a_page_changes_no_size() {
    // Each rule of a page in a state sets colours only, so choosing a page moves no other.
    let css = css_of(include_str!("../../../../app/src/ui/panel/panel.css"));
    for state in ["[data-hovered]", "[data-selected]", "[data-focus-visible]"] {
        let body = rule(&css, &format!("{NAV_ITEM}{state}"));
        for line in body.lines().skip(1).map(str::trim).filter(|one| !one.is_empty()) {
            let property = line.split(':').next().unwrap();
            assert!(
                [
                    "background-color",
                    "color",
                    "outline",
                    "outline-offset"
                ]
                .contains(&property),
                "{state}: {line}"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------
// The built-ins
// ---------------------------------------------------------------------------------------

#[test]
fn both_built_ins_draw_the_family_default_cleanly() {
    for theme in [purlis(), classic()] {
        assert!(
            !theme
                .first()
                .parts
                .keys()
                .any(|key| key.family == "view-frame" || key.family == "view-nav"),
            "a built-in draws the view frame's default"
        );
        for appearance in &theme.appearances {
            assert!(parts::fallen(appearance).is_empty(), "{}", appearance.id);
        }
    }
}

#[test]
fn the_path_s_words_are_held_to_their_floor() {
    // The path's words in the hairline's colour cannot be read: the default is drawn.
    let loaded = load(&over(
        PURLIS,
        json!({ "view-frame.crumb.text": "border.subtle" }),
    ));
    assert!(!loaded.refused, "{:?}", said(&loaded));
    let fell = parts::fallen(loaded.theme.first());
    assert!(
        fell.iter().any(|one| one.part.starts_with("view-frame")),
        "{fell:?}"
    );
    let css = css_of(include_str!("../../../../app/src/ui/panel/panel.css"));
    let crumb = rule(&css, "[data-scope=\"view-frame\"][data-part=\"crumb\"]");
    assert_eq!(token(crumb, "color"), "text.primary");
    let path = rule(&css, "[data-scope=\"view-frame\"][data-part=\"path\"]");
    assert_eq!(token(path, "color"), "text.muted");
}

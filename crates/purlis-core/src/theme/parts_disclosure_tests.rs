// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The disclosure at the engine's interface (#3675): in the manifest with its parts and no state,
//! a theme's keys reaching its panel and its twist, a part it does not have said, the twist's
//! floor held, the defaults held to `disclosure.css`, and both built-ins drawing the default.

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

const ROOT: &str = "[data-scope=\"disclosure\"][data-part=\"root\"]";
const PANEL: &str = "[data-scope=\"disclosure\"][data-part=\"panel\"]";
const TWIST: &str = "[data-scope=\"disclosure\"][data-part=\"twist\"]";

#[test]
fn the_disclosure_comes_in_the_next_release_s_manifest() {
    let family = parts::manifest_json()["families"]
        .as_array()
        .unwrap()
        .iter()
        .find(|one| one["name"] == "disclosure")
        .expect("the disclosure is in the manifest")
        .clone();
    assert_eq!(family["since"], parts::NEXT);
    let parts: Vec<&str> = family["parts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|one| one["name"].as_str().unwrap())
        .collect();
    assert_eq!(parts, ["root", "trigger", "twist", "panel"]);
    for part in family["parts"].as_array().unwrap() {
        assert_eq!(part["since"], parts::NEXT, "{part}");
    }
    // Opening it is no state: the twist turns, and nothing in it changes size or colour.
    assert_eq!(family["states"], json!([]));
}

#[test]
fn a_theme_reaches_the_panel_and_the_twist() {
    let loaded = load(&over(
        PURLIS,
        json!({
            "disclosure.panel.fill": "surface.sunken",
            "disclosure.panel.corner-radius": 8,
            "disclosure.twist.text": "text.muted",
        }),
    ));
    assert!(!loaded.refused, "{:?}", said(&loaded));
    let sheet = parts::stylesheet(loaded.theme.first());
    assert!(
        sheet.contains(&format!(
            ":where({ROOT} {PANEL}) {{ background-color: var(--surface-sunken); border-radius: min(8px, 50%); }}"
        )),
        "{sheet}"
    );
    assert!(
        sheet.contains(&format!(
            ":where({ROOT} {TWIST}) {{ color: var(--text-muted); }}"
        )),
        "{sheet}"
    );
}

#[test]
fn a_state_or_a_part_it_does_not_have_is_said() {
    for key in [
        "disclosure.panel.open.fill",
        "disclosure.chevron.text",
        "disclosure.twist.fill",
    ] {
        let loaded = load(&over(PURLIS, json!({ key: "accent.text" })));
        assert!(!said(&loaded).is_empty(), "{key} was taken");
    }
}

#[test]
fn a_twist_that_cannot_be_seen_draws_the_default() {
    let loaded = load(&over(
        PURLIS,
        json!({ "disclosure.twist.text": "border.subtle" }),
    ));
    assert!(!loaded.refused, "{:?}", said(&loaded));
    let fell = parts::fallen(loaded.theme.first());
    assert!(
        fell.iter().any(|one| one.part.starts_with("disclosure")),
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
fn the_defaults_are_what_disclosure_css_draws() {
    let css = include_str!("../../../../app/src/ui/disclosure/disclosure.css");
    assert_eq!(token(css, TWIST, "color"), "text.quiet");
    let family = parts::MANIFEST
        .iter()
        .find(|one| one.name == "disclosure")
        .unwrap();
    let twist = family
        .parts
        .iter()
        .position(|one| one.name == "twist")
        .unwrap();
    assert_eq!(family.own_words_of(twist), Some("text.quiet"));
}

#[test]
fn both_built_ins_draw_the_family_default_cleanly() {
    for theme in [purlis(), classic()] {
        assert!(
            !theme
                .first()
                .parts
                .keys()
                .any(|key| key.family == "disclosure"),
            "a built-in draws the disclosure's default"
        );
        for appearance in &theme.appearances {
            assert!(parts::fallen(appearance).is_empty(), "{}", appearance.id);
        }
    }
}

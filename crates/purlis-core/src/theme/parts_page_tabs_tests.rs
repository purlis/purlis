// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Page tabs at the engine's interface (#3668): in the manifest with their parts, a theme's keys
//! reaching the chosen tab and its indicator, no size in a state, the words' floors held, the
//! defaults held to `tabs.css`, and both built-ins drawing the family default.

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

const TAB: &str = "[data-scope=\"page-tabs\"][data-part=\"tab\"]";

#[test]
fn page_tabs_come_in_the_next_release_s_manifest() {
    let family = parts::manifest_json()["families"]
        .as_array()
        .unwrap()
        .iter()
        .find(|one| one["name"] == "page-tabs")
        .expect("page tabs are in the manifest")
        .clone();
    assert_eq!(family["since"], parts::NEXT);
    let parts: Vec<&str> = family["parts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|one| one["name"].as_str().unwrap())
        .collect();
    assert_eq!(parts, ["tab", "label", "count", "indicator", "list", "panel"]);
    assert_eq!(family["states"], json!(["hover", "selected", "focused"]));
}

#[test]
fn a_theme_reaches_the_chosen_tab_and_its_indicator() {
    let loaded = load(&over(
        PURLIS,
        json!({
            "page-tabs.tab.selected.text": "accent.text",
            "page-tabs.indicator.selected.fill": "accent.text",
            "page-tabs.indicator.height": 3,
        }),
    ));
    assert!(!loaded.refused, "{:?}", said(&loaded));
    let sheet = parts::stylesheet(loaded.theme.first());
    assert!(
        sheet.contains(&format!(
            ":where({TAB}[data-selected]) {{ color: var(--accent-text); }}"
        )),
        "{sheet}"
    );
    assert!(
        sheet.contains(&format!(
            ":where({TAB}[data-selected] [data-scope=\"page-tabs\"][data-part=\"indicator\"]) {{ background-color: var(--accent-text); }}"
        )),
        "{sheet}"
    );
}

#[test]
fn a_size_in_a_state_is_refused_so_no_tab_moves() {
    for key in [
        "page-tabs.label.selected.font-weight",
        "page-tabs.indicator.selected.height",
    ] {
        let value = if key.ends_with("height") {
            json!(3)
        } else {
            json!(600)
        };
        let loaded = load(&over(PURLIS, json!({ key: value })));
        assert!(
            said(&loaded)
                .iter()
                .any(|one| one.contains(parts::NO_SIZE_IN_A_STATE)),
            "{key}: {:?}",
            said(&loaded)
        );
    }
}

#[test]
fn chosen_words_that_cannot_be_read_draw_the_default() {
    let loaded = load(&over(
        PURLIS,
        json!({ "page-tabs.tab.selected.text": "border.subtle" }),
    ));
    assert!(!loaded.refused, "{:?}", said(&loaded));
    let fell = parts::fallen(loaded.theme.first());
    assert!(
        fell.iter().any(|one| one.part.starts_with("page-tabs.tab")),
        "{fell:?}"
    );
}

/// The token a rule `selector` opens in `css` gives `property`.
fn token(css: &str, selector: &str, property: &str) -> String {
    let at = css
        .rfind(&format!("\n{selector} {{"))
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
fn a_tab_s_defaults_are_what_tabs_css_draws() {
    let css = include_str!("../../../../app/src/ui/tabs/tabs.css");
    let look = |states: &[parts::State]| parts::look_of("page-tabs", "", states);
    assert_eq!(look(&[])[1], token(css, TAB, "color"));
    assert_eq!(
        look(&[parts::State::Hover])[1],
        token(css, &format!("{TAB}[data-hovered]"), "color")
    );
    assert_eq!(
        look(&[parts::State::Selected])[1],
        token(css, &format!("{TAB}[data-selected]"), "color")
    );
    assert_eq!(
        token(
            css,
            "[data-scope=\"page-tabs\"][data-part=\"count\"]",
            "color"
        ),
        "text.quiet"
    );
}

#[test]
fn both_built_ins_draw_the_family_default_cleanly() {
    for theme in [purlis(), classic()] {
        assert!(
            !theme
                .first()
                .parts
                .keys()
                .any(|key| key.family == "page-tabs"),
            "a built-in draws page tabs' default"
        );
        for appearance in &theme.appearances {
            assert!(parts::fallen(appearance).is_empty(), "{}", appearance.id);
        }
    }
}

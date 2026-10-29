// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The review page at the engine's interface (#3672): in the manifest with its parts and no
//! state, a theme's keys reaching its bar and its summary, the bar's edge kept from a theme, the
//! summary's floor held, the defaults held to `review-page.css`, and both built-ins drawing the
//! family default.

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

const ROOT: &str = "[data-scope=\"review-page\"][data-part=\"root\"]";
const BAR: &str = "[data-scope=\"review-page\"][data-part=\"bar\"]";
const SUMMARY: &str = "[data-scope=\"review-page\"][data-part=\"summary\"]";

#[test]
fn the_review_page_comes_in_the_next_release_s_manifest() {
    let family = parts::manifest_json()["families"]
        .as_array()
        .unwrap()
        .iter()
        .find(|one| one["name"] == "review-page")
        .expect("the review page is in the manifest")
        .clone();
    assert_eq!(family["since"], parts::NEXT);
    let parts: Vec<&str> = family["parts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|one| one["name"].as_str().unwrap())
        .collect();
    assert_eq!(parts, ["root", "body", "bar", "summary", "answers"]);
    for part in family["parts"].as_array().unwrap() {
        assert_eq!(part["since"], parts::NEXT, "{part}");
    }
    assert_eq!(family["states"], json!([]));
}

#[test]
fn a_theme_reaches_the_bar_and_its_summary() {
    let loaded = load(&over(
        PURLIS,
        json!({
            "review-page.bar.fill": "surface.raised",
            "review-page.summary.text": "text.primary",
        }),
    ));
    assert!(!loaded.refused, "{:?}", said(&loaded));
    let sheet = parts::stylesheet(loaded.theme.first());
    assert!(
        sheet.contains(&format!(
            ":where({ROOT} {BAR}) {{ background-color: var(--surface-raised); }}"
        )),
        "{sheet}"
    );
    assert!(
        sheet.contains(&format!(":where({ROOT} {SUMMARY}) {{ color: var(--text-primary); }}")),
        "{sheet}"
    );
}

#[test]
fn the_bar_s_edge_is_never_a_theme_s() {
    // The bar is a consent surface where it holds a grant: the line over it is the window's.
    for key in [
        "review-page.bar.border-width",
        "review-page.bar.border-colour",
    ] {
        let loaded = load(&over(PURLIS, json!({ key: 0 })));
        assert!(!said(&loaded).is_empty(), "{key} was taken");
    }
}

#[test]
fn a_summary_that_cannot_be_read_draws_the_default() {
    let loaded = load(&over(
        PURLIS,
        json!({ "review-page.summary.text": "border.subtle" }),
    ));
    assert!(!loaded.refused, "{:?}", said(&loaded));
    let fell = parts::fallen(loaded.theme.first());
    assert!(
        fell.iter().any(|one| one.part.starts_with("review-page")),
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
fn the_defaults_are_what_review_page_css_draws() {
    let css = include_str!("../../../../app/src/ui/review-page/review-page.css");
    assert_eq!(token(css, SUMMARY, "color"), "text.muted");
    assert_eq!(token(css, BAR, "border-block-start-color"), "border.subtle");
    let look = parts::look_of("review-page", "", &[]);
    assert_eq!(look[1], "text.primary");
}

#[test]
fn both_built_ins_draw_the_family_default_cleanly() {
    for theme in [purlis(), classic()] {
        assert!(
            !theme
                .first()
                .parts
                .keys()
                .any(|key| key.family == "review-page"),
            "a built-in draws the review page's default"
        );
        for appearance in &theme.appearances {
            assert!(parts::fallen(appearance).is_empty(), "{}", appearance.id);
        }
    }
}

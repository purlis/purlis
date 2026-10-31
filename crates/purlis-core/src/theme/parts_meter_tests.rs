// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The meter family at the engine's interface (#3674): in the manifest since the next version,
//! its colours held to `meter.css`, each fill measured on its track at 3:1 in every built-in
//! theme, and a theme whose fill or track would lose the bar drawing the variant's default back.

use serde_json::{Value, json};

use super::parts;
use super::*;

/// A theme over `base` that sets `parts`.
fn over(base: &str, parts: Value) -> Value {
    json!({ "name": "Shaped", "extends": base, "parts": parts })
}

/// What fell back in every appearance of a theme over `base` that sets `parts`.
fn fell_in(base: &str, parts: Value) -> Vec<Vec<String>> {
    let loaded = load(&over(base, parts));
    assert!(!loaded.refused, "{:?}", loaded.complaints);
    loaded
        .theme
        .appearances
        .iter()
        .map(|theme| {
            parts::fallen(theme)
                .iter()
                .map(|one| one.part.clone())
                .collect()
        })
        .collect()
}

/// The built-in themes, by the name a theme extends them by.
const BUILT_IN: &[&str] = &["purlis", "purlis-classic"];

#[test]
fn the_meter_is_in_the_manifest_since_next() {
    let manifest = parts::manifest_json();
    let meter = manifest["families"]
        .as_array()
        .unwrap()
        .iter()
        .find(|one| one["name"] == "meter")
        .expect("the meter family is in the manifest");
    assert_eq!(meter["since"], parts::NEXT);
    let names = |key: &str| -> Vec<String> {
        meter[key]
            .as_array()
            .unwrap()
            .iter()
            .map(|one| one["name"].as_str().unwrap().to_owned())
            .collect()
    };
    assert_eq!(
        names("parts"),
        ["root", "label", "track", "fill", "value"]
    );
    assert_eq!(names("variants"), ["subject", "baseline", "previous"]);
}

/// A stylesheet with no comments.
fn uncommented(text: &str) -> String {
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

/// The token the rule `selector` in `css` gives `property`: `var(--accent-base)` is
/// `accent.base`.
fn drawn(css: &str, selector: &str, property: &str) -> String {
    let at = css
        .find(&format!("\n{selector} {{"))
        .unwrap_or_else(|| panic!("no rule {selector}"));
    let body = &css[at..];
    let body = &body[..body.find('}').expect("the rule ends")];
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

/// **What `meter.css` draws, as the engine holds it** (`parts_meter.rs`): the track in the
/// subtle hairline's colour, the subject's fill in the accent, the baseline's in the waiting
/// colour, the previous version's in the muted tier. The engine's side of each is what its
/// fallbacks write back (below).
const DRAWN: &[(&str, &str)] = &[
    ("subject", "accent.base"),
    ("baseline", "state.waiting"),
    ("previous", "text.muted"),
];

#[test]
fn the_meter_s_defaults_are_what_its_stylesheet_draws() {
    let css = uncommented(include_str!("../../../../app/src/ui/meter/meter.css"));
    let family = parts::family("meter").expect("the meter family");
    let root = "[data-scope=\"meter\"][data-part=\"root\"]";
    assert_eq!(
        drawn(&css, root, "color"),
        parts::look_of("meter", "subject", &[])[1]
    );
    assert_eq!(
        drawn(&css, "[data-scope=\"meter\"][data-part=\"label\"]", "color"),
        family.own_words_of(1).expect("the label's words")
    );
    assert_eq!(
        drawn(
            &css,
            "[data-scope=\"meter\"][data-part=\"track\"]",
            "background-color"
        ),
        "border.subtle"
    );
    for (variant, fill) in DRAWN {
        let selector = if *variant == "subject" {
            "[data-scope=\"meter\"][data-part=\"fill\"]".to_owned()
        } else {
            format!("{root}[data-variant=\"{variant}\"] [data-part=\"fill\"]")
        };
        assert_eq!(drawn(&css, &selector, "background-color"), *fill, "{variant}");
    }
}

#[test]
fn a_fallen_meter_draws_its_variant_s_fill_and_track_back() {
    // A track in the fill's own colour loses every variant's bar.
    let loaded = load(&over(
        "purlis-classic",
        json!({
            "meter.track.fill": "text.primary",
            "meter.fill.fill": "text.primary"
        }),
    ));
    assert!(!loaded.refused, "{:?}", loaded.complaints);
    let sheet = parts::stylesheet(loaded.theme.first());
    for (variant, fill) in DRAWN {
        let token = format!("var(--{})", fill.replace('.', "-"));
        let reached = format!(
            "[data-variant=\"{variant}\"] [data-scope=\"meter\"][data-part=\"fill\"]"
        );
        assert!(
            sheet.lines().any(|line| line.contains(&reached)
                && line.contains(&format!("background-color: {token}"))),
            "{variant}: {sheet}"
        );
    }
}

#[test]
fn every_built_in_theme_draws_each_meter_s_fill_at_3_to_1_on_its_track() {
    for base in BUILT_IN {
        for fell in fell_in(base, json!({})) {
            assert!(
                fell.iter().all(|one| !one.starts_with("meter.")),
                "{base}: {fell:?}"
            );
        }
    }
}

#[test]
fn a_fill_lost_on_its_track_draws_its_variant_s_default() {
    for base in BUILT_IN {
        let fell = fell_in(base, json!({ "meter.fill.baseline.fill": "border.subtle" }));
        assert!(
            fell.iter()
                .all(|one| one.iter().any(|part| part == "meter.root.baseline")),
            "{base}: {fell:?}"
        );
        // The other variants keep what they draw.
        assert!(
            fell.iter()
                .all(|one| !one.iter().any(|part| part == "meter.root.subject")),
            "{base}: {fell:?}"
        );
    }
}

#[test]
fn a_track_that_hides_the_fill_draws_the_track_back_too() {
    let fell = fell_in(
        "purlis-classic",
        json!({ "meter.track.fill": "accent.base" }),
    );
    assert!(
        fell.iter()
            .all(|one| one.iter().any(|part| part == "meter.root.subject")),
        "{fell:?}"
    );
    let loaded = load(&over(
        "purlis-classic",
        json!({ "meter.track.fill": "accent.base" }),
    ));
    let sheet = parts::stylesheet(loaded.theme.first());
    // The fallback draws the subject's track and fill back, after the theme's rule.
    let track = "[data-variant=\"subject\"] [data-scope=\"meter\"][data-part=\"track\"]";
    let fill = "[data-variant=\"subject\"] [data-scope=\"meter\"][data-part=\"fill\"]";
    assert!(
        sheet.lines().any(|line| line.contains(track)
            && line.contains("background-color: var(--border-subtle)")),
        "{sheet}"
    );
    assert!(
        sheet.lines().any(|line| line.contains(fill)
            && line.contains("background-color: var(--accent-base)")),
        "{sheet}"
    );
}

#[test]
fn a_fill_that_keeps_its_contrast_is_drawn_as_the_theme_says() {
    let loaded = load(&over(
        "purlis-classic",
        json!({ "meter.fill.previous.fill": "text.secondary" }),
    ));
    assert!(!loaded.refused, "{:?}", loaded.complaints);
    for theme in &loaded.theme.appearances {
        assert!(
            parts::fallen(theme)
                .iter()
                .all(|one| one.part != "meter.root.previous"),
            "{:?}",
            parts::fallen(theme)
        );
    }
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The activity item family at the engine's interface (#2917, R116–R120, N17): in the manifest
//! with its parts and its two corners' variants, its keys in the stylesheet, its colours held to
//! `activity.css`, Purlis drawing the family default and Purlis Classic its accent edge through
//! parts; and the shell's motion tokens (R150) in both built-ins.

use serde_json::{Value, json};

use super::parts;
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

#[test]
fn the_activity_family_is_in_the_manifest_at_version_10() {
    let manifest = parts::manifest_json();
    let family = manifest["families"]
        .as_array()
        .unwrap()
        .iter()
        .find(|one| one["name"] == "activity")
        .expect("the activity family is in the manifest")
        .clone();
    assert_eq!(family["since"], 10);
    let names = |list: &str| -> Vec<String> {
        family[list]
            .as_array()
            .unwrap()
            .iter()
            .map(|one| one["name"].as_str().unwrap().to_owned())
            .collect()
    };
    assert_eq!(names("parts"), ["item", "icon", "count", "indicator"]);
    assert_eq!(names("variants"), ["tab", "bar"]);
    for variant in family["variants"].as_array().unwrap() {
        assert_eq!(
            variant["consent"], false,
            "an activity item answers nothing"
        );
    }
}

#[test]
fn the_reference_keys_reach_the_item_and_its_chosen_state() {
    // R139's Purlis keys, as a theme writes them.
    let loaded = load(&with_parts(json!({
        "activity.item.corner-radius": 9,
        "activity.item.selected.fill": "accent.surface",
        "activity.item.bar.corner-radius": "full",
    })));
    assert!(!loaded.refused, "{:?}", said(&loaded));
    let sheet = parts::stylesheet(loaded.theme.first());
    assert!(
        sheet.contains(
            ":where([data-scope=\"activity\"][data-part=\"item\"]) { border-radius: min(9px,"
        ),
        "{sheet}"
    );
    assert!(
        sheet.contains(
            "[data-scope=\"activity\"][data-part=\"item\"][data-selected]) { background-color: var(--accent-surface);"
        ),
        "{sheet}"
    );
    assert!(
        sheet.contains("[data-scope=\"activity\"][data-part=\"item\"][data-variant=\"bar\"])"),
        "{sheet}"
    );
}

#[test]
fn a_size_is_no_key_of_the_item_and_a_state_never_changes_one() {
    let loaded = load(&with_parts(json!({ "activity.item.height": 40 })));
    assert!(
        said(&loaded)
            .iter()
            .any(|one| one.contains("activity.item.height")),
        "{:?}",
        said(&loaded)
    );
    let loaded = load(&with_parts(
        json!({ "activity.item.hover.corner-radius": 4 }),
    ));
    assert!(
        said(&loaded)
            .iter()
            .any(|one| one.contains(parts::NO_SIZE_IN_A_STATE)),
        "{:?}",
        said(&loaded)
    );
}

#[test]
fn the_indicator_is_reached_inside_a_chosen_item() {
    let loaded = load(&with_parts(
        json!({ "activity.indicator.selected.fill": "accent.base" }),
    ));
    assert!(!loaded.refused, "{:?}", said(&loaded));
    let sheet = parts::stylesheet(loaded.theme.first());
    assert!(
        sheet.contains(
            ":where([data-scope=\"activity\"][data-part=\"item\"][data-selected] [data-scope=\"activity\"][data-part=\"indicator\"]) { background-color: var(--accent-base); }"
        ),
        "{sheet}"
    );
}

#[test]
fn chosen_words_that_break_their_floor_draw_the_family_default() {
    // The accent's wash under words in the wash's own colour: unreadable, so the default holds.
    let loaded = load(&with_parts(json!({
        "activity.item.selected.fill": "accent.surface",
        "activity.item.selected.text": "accent.surface",
    })));
    assert!(!loaded.refused, "{:?}", said(&loaded));
    let fell = parts::fallen(loaded.theme.first());
    assert!(
        fell.iter().any(|one| one.part.starts_with("activity.item")),
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
    out.replace("(\n    ", "(").replace("\n  ) {", ") {")
}

/// The declarations of the last rule `selector` opens in `css`.
fn rule<'a>(css: &'a str, selector: &str) -> &'a str {
    let at = css
        .rfind(&format!("\n{selector} {{"))
        .unwrap_or_else(|| panic!("no rule {selector}"));
    let body = &css[at..];
    &body[..body.find('}').unwrap()]
}

fn declared<'a>(body: &'a str, property: &str) -> Option<&'a str> {
    body.lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix(&format!("{property}: ")))
        .map(|value| value.trim_end_matches(';'))
}

/// The token a declared value names: `var(--surface-base)` is `surface.base`.
fn token(value: &str) -> String {
    if value == "transparent" {
        return value.to_owned();
    }
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
fn the_item_s_defaults_are_what_activity_css_draws() {
    let css = css_of(include_str!("../../../../app/src/ui/activity/activity.css"));
    let root = "[data-scope=\"activity\"][data-part=\"item\"]";
    let rest = rule(&css, root);
    let hover = rule(&css, &format!("{root}[data-hovered]"));
    let chosen = rule(&css, &format!("{root}[data-selected]"));
    for variant in ["tab", "bar"] {
        let held = parts::look_json("activity", variant);
        assert_eq!(
            held["fill"],
            token(declared(rest, "background-color").unwrap())
        );
        assert_eq!(held["text"], token(declared(rest, "color").unwrap()));
        assert_eq!(
            held["hover"],
            token(declared(hover, "background-color").unwrap())
        );
    }
    let selected = parts::activity_selected_json();
    assert_eq!(
        selected["fill"],
        token(declared(chosen, "background-color").unwrap())
    );
    assert_eq!(selected["text"], token(declared(chosen, "color").unwrap()));
}

// ---------------------------------------------------------------------------------------
// The built-ins
// ---------------------------------------------------------------------------------------

#[test]
fn purlis_draws_the_family_default_and_classic_its_accent_edge() {
    let purlis = purlis();
    assert!(
        !purlis
            .first()
            .parts
            .keys()
            .any(|key| key.to_string().starts_with("activity.")),
        "Purlis draws the activity family's default"
    );
    let sheet = parts::stylesheet(classic().first());
    assert!(
        sheet.contains(
            "[data-scope=\"activity\"][data-part=\"item\"][data-selected] [data-scope=\"activity\"][data-part=\"indicator\"]) { background-color: var(--accent-base); }"
        ),
        "{sheet}"
    );
    assert!(sheet.contains("border-radius:"), "{sheet}");
    // Nothing Classic sets falls back: its look is clean in both appearances.
    for appearance in &classic().appearances {
        assert!(parts::fallen(appearance).is_empty(), "{}", appearance.id);
    }
}

#[test]
fn the_shell_s_motions_are_tokens_in_both_built_ins() {
    // R150: the right panel, a bottom half and a side view, each named for the change it times.
    // A bottom half rises over the rail's `reveal` (#2915), the same in both built-ins.
    for (id, open, reveal, swap) in [(PURLIS, 240, 220, 180), (CLASSIC, 160, 220, 0)] {
        let built = built_in(id).expect("a built-in");
        let durations = &built.motion.durations;
        assert_eq!(durations["duration.open"], open, "{id}");
        assert_eq!(durations["duration.reveal"], reveal, "{id}");
        assert_eq!(durations["duration.swap"], swap, "{id}");
    }
    let purlis = purlis();
    for appearance in &purlis.appearances {
        assert_eq!(appearance.motion.durations["duration.open"], 240);
    }
}

#[test]
fn a_theme_that_names_no_shell_motion_keeps_its_parent_s() {
    let loaded = load(&json!({
        "name": "Calm",
        "extends": "purlis",
        "appearances": [{ "id": "dark", "motion": { "duration.swap": 0 } }]
    }));
    assert!(!loaded.refused, "{:?}", said(&loaded));
    let first = loaded.theme.first();
    assert_eq!(first.motion.durations["duration.swap"], 0);
    assert_eq!(first.motion.durations["duration.open"], 240);
}

// ---------------------------------------------------------------------------------------
// The count's own pill, and where the consent floors are tinted
// ---------------------------------------------------------------------------------------

#[test]
fn a_count_s_words_are_held_on_its_own_pill() {
    // Words in the pill's own colour: unreadable on the pill, whatever the item's fill is.
    let loaded = load(&with_parts(json!({
        "activity.count.fill": "accent.base",
        "activity.count.text": "accent.base",
    })));
    assert!(!loaded.refused, "{:?}", said(&loaded));
    let fell = parts::fallen(loaded.theme.first());
    assert!(
        fell.iter()
            .any(|one| one.part.starts_with("activity.item")
                && one.floor.contains("the count's words")),
        "{fell:?}"
    );
    // Its fallback draws the count's own pill and words again.
    let sheet = parts::stylesheet(loaded.theme.first());
    assert!(
        sheet.contains("background-color: var(--surface-hover)"),
        "{sheet}"
    );
    assert!(sheet.contains("color: var(--text-secondary)"), "{sheet}");
}

#[test]
fn a_theme_s_count_colours_never_reach_the_needs_you_count() {
    let loaded = load(&with_parts(
        json!({ "activity.count.fill": "surface.overlay" }),
    ));
    assert!(!loaded.refused, "{:?}", said(&loaded));
    let sheet = parts::stylesheet(loaded.theme.first());
    assert!(
        sheet.contains("[data-scope=\"activity\"][data-part=\"count\"]:not([data-tone])"),
        "{sheet}"
    );
}

/// A theme whose secondary button's edge meets Allow's only in a workspace's tint (as
/// `parts_tests::the_consent_floors_hold_in_every_workspace_tint` builds it), with `extra` parts.
fn tinted_with(extra: Value) -> Value {
    let accent = classic().first().value("accent.base").to_owned();
    let opposite = tint_hex(&accent, (hue_of(&accent).unwrap() + 180.0) % 360.0);
    let mut parts = json!({ "button.root.secondary.border-colour": "tab.active" });
    for (key, value) in extra.as_object().unwrap() {
        parts[key] = value.clone();
    }
    json!({
        "name": "Tinted", "extends": "purlis-classic",
        "appearances": [{ "id": "dark", "tokens": { "tab.active": opposite } }],
        "tint": { "tokens": ["accent.base", "tab.active"] },
        "parts": parts
    })
}

#[test]
fn an_allow_edge_broken_in_one_tint_still_falls_back_beside_activity_parts() {
    let loaded = load(&tinted_with(
        json!({ "activity.item.selected.fill": "accent.surface" }),
    ));
    assert!(!loaded.refused, "{:?}", said(&loaded));
    let fell = parts::fallen(loaded.theme.first());
    assert!(
        fell.iter().any(|one| one.part == "button.root.secondary"
            && one.floor == "Allow is distinguishable from Keep blocked by its edge"),
        "{fell:?}"
    );
    assert!(parts::drawn_in() > 1, "measured in every tint");
}

#[test]
fn a_theme_of_activity_parts_alone_is_measured_once_as_written() {
    let loaded = load(&json!({
        "name": "Tabs", "extends": "purlis-classic",
        "tint": { "tokens": ["accent.base"] },
        "parts": { "activity.item.selected.fill": "accent.surface" }
    }));
    assert!(!loaded.refused, "{:?}", said(&loaded));
    assert!(parts::fallen(loaded.theme.first()).is_empty());
    assert_eq!(parts::drawn_in(), 1);
}

// ---------------------------------------------------------------------------------------
// The hot count's own role (#3098)
// ---------------------------------------------------------------------------------------

#[test]
fn the_hot_count_draws_from_its_own_role_and_no_other() {
    let css = include_str!("../../../../app/src/ui/activity/activity.css");
    let rule = css
        .split("[data-tone=\"needs-you\"] {")
        .nth(1)
        .and_then(|rest| rest.split('}').next())
        .expect("the hot count's rule");
    assert!(
        rule.contains("background-color: var(--needs-you-count);"),
        "{rule}"
    );
    assert!(
        rule.contains("color: var(--needs-you-count-text);"),
        "{rule}"
    );
}

#[test]
fn a_plain_count_rule_that_comes_near_the_hot_count_draws_its_default() {
    let loaded = load(&with_parts(json!({
        "activity.count.fill": "needs-you.count",
        "activity.count.text": "needs-you.count-text",
    })));
    assert!(!loaded.refused, "{:?}", said(&loaded));
    let fell = parts::fallen(loaded.theme.first());
    assert!(
        fell.iter().any(|one| one.part.starts_with("activity.item")
            && one.floor == "the plain count is told apart from the hot count"),
        "{fell:?}"
    );
}

#[test]
fn a_plain_count_rule_apart_from_the_hot_count_stays_as_written() {
    // A role only the hot count has, as the rule's fill: white, far from the hot colour. Where the
    // role does not exist the rule names no colour, and it is said.
    let loaded = load(&with_parts(json!({
        "activity.count.fill": "needs-you.count-text",
        "activity.count.text": "surface.deep",
    })));
    assert!(!loaded.refused, "{:?}", said(&loaded));
    assert_eq!(said(&loaded), Vec::<String>::new());
    assert_eq!(parts::fallen(loaded.theme.first()), []);
}

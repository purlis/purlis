// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The panel separator family at the engine's interface (#3143): in the manifest with its handle
//! and its grip, keyed on the panel library's own state marks, what it draws in a state held at
//! 3:1 as a non-text mark, its defaults held to `panel.css`, Purlis drawing the family default
//! (a quiet grip, never the accent) and Purlis Classic its accent line through parts.

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

/// The stylesheet of a theme over Purlis that sets `parts`, which must be drawn, and what fell.
fn drawn_over_purlis(parts: Value) -> (String, Vec<parts::Fell>) {
    let loaded = load(&over(PURLIS, parts));
    assert!(!loaded.refused, "{:?}", said(&loaded));
    let first = loaded.theme.first();
    (parts::stylesheet(first), parts::fallen(first))
}

const ROOT: &str = "[data-scope=\"panel-separator\"][data-part=\"root\"]";
const GRIP: &str = "[data-scope=\"panel-separator\"][data-part=\"grip\"]";
const HOVER: &str = ":is([data-separator=\"hover\"], [data-separator=\"focus\"]:hover)";

// ---------------------------------------------------------------------------------------
// The manifest
// ---------------------------------------------------------------------------------------

#[test]
fn the_panel_separator_is_in_the_manifest_at_version_13() {
    let manifest = parts::manifest_json();
    assert!(manifest["version"].as_u64().unwrap() >= 13);
    let family = manifest["families"]
        .as_array()
        .unwrap()
        .iter()
        .find(|one| one["name"] == "panel-separator")
        .expect("the panel separator is in the manifest")
        .clone();
    assert_eq!(family["since"], 13);
    let parts: Vec<&str> = family["parts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|one| one["name"].as_str().unwrap())
        .collect();
    assert_eq!(parts, ["root", "grip"]);
    for part in family["parts"].as_array().unwrap() {
        assert_eq!(part["properties"], json!(["fill"]), "{part}");
    }
    assert_eq!(family["variants"], json!([]));
    assert_eq!(family["states"], json!(["hover", "pressed", "focused"]));
    assert_eq!(family["marks"], true);
    assert_eq!(
        family["attributes"],
        json!({
            "hover": ":is([data-separator=\"hover\"], [data-separator=\"focus\"]:hover)",
            "pressed": "[data-separator=\"active\"]",
            "focused": ":focus-visible",
        })
    );
    // Only this family says so: every other one is drawn in words, on React Aria's marks.
    let others = manifest["families"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|one| one["name"] != "panel-separator");
    for one in others {
        assert!(one.get("marks").is_none(), "{}", one["name"]);
        assert!(one.get("attributes").is_none(), "{}", one["name"]);
    }
}

#[test]
fn a_size_or_a_state_the_handle_never_takes_is_said_and_left_out() {
    for key in [
        "panel-separator.grip.height",
        "panel-separator.root.border-width",
        "panel-separator.grip.text",
        "panel-separator.root.open.fill",
        "panel-separator.grip.selected.fill",
    ] {
        let loaded = load(&over(PURLIS, json!({ key: "accent.base" })));
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
fn a_state_is_keyed_on_the_panel_library_s_own_marks() {
    let (sheet, fell) = drawn_over_purlis(json!({
        "panel-separator.root.hover.fill": "text.muted",
        "panel-separator.grip.hover.fill": "surface.deep",
        "panel-separator.grip.pressed.fill": "text.primary",
    }));
    assert!(fell.is_empty(), "{fell:?}");
    assert!(
        sheet.contains(&format!(
            ":where({ROOT}{HOVER}) {{ background-color: var(--text-muted); }}"
        )),
        "{sheet}"
    );
    assert!(
        sheet.contains(&format!(
            ":where({ROOT}{HOVER} {GRIP}) {{ background-color: var(--surface-deep); }}"
        )),
        "{sheet}"
    );
    assert!(
        sheet.contains(&format!(
            ":where({ROOT}[data-separator=\"active\"] {GRIP}) {{ background-color: var(--text-primary); }}"
        )),
        "{sheet}"
    );
    assert!(!sheet.contains("[data-hovered]"), "{sheet}");
    assert!(!sheet.contains("[data-pressed]"), "{sheet}");
}

// ---------------------------------------------------------------------------------------
// The floors: a mark, at 3:1
// ---------------------------------------------------------------------------------------

#[test]
fn a_grip_too_faint_to_be_told_apart_draws_the_family_default() {
    // The window's hairline is no mark: 3:1 is what a person needs to see the grip.
    let (sheet, fell) = drawn_over_purlis(json!({
        "panel-separator.grip.hover.fill": "border.subtle",
    }));
    let one = fell
        .iter()
        .find(|one| one.part == "panel-separator.root.hover")
        .unwrap_or_else(|| panic!("{fell:?}"));
    assert_eq!(
        one.floor,
        "the grip's mark contrasts 3:1 with what it is drawn on"
    );
    assert!(one.ratio.is_some_and(|ratio| ratio < 3.0), "{one:?}");
    // The default comes back after the theme's rule, so the faint tier is what is drawn.
    let mine = sheet
        .find("background-color: var(--border-subtle)")
        .expect("the theme's rule");
    let back = sheet
        .rfind(&format!(
            ":where({ROOT}{HOVER} {GRIP}) {{ background-color: var(--text-faint); }}"
        ))
        .unwrap_or_else(|| panic!("{sheet}"));
    assert!(back > mine, "{sheet}");
}

#[test]
fn the_handle_s_own_fill_in_a_state_is_held_as_a_mark_too() {
    let (_, fell) = drawn_over_purlis(json!({
        "panel-separator.root.pressed.fill": "border.subtle",
    }));
    let one = fell
        .iter()
        .find(|one| one.part == "panel-separator.root.pressed")
        .unwrap_or_else(|| panic!("{fell:?}"));
    assert_eq!(
        one.floor,
        "the root's mark contrasts 3:1 with what it is drawn on"
    );
}

#[test]
fn a_grip_on_the_handle_s_own_fill_is_read_on_that_fill() {
    // The accent's wash under a grip in the same wash: nothing to tell apart.
    let (_, fell) = drawn_over_purlis(json!({
        "panel-separator.root.hover.fill": "accent.surface",
        "panel-separator.grip.hover.fill": "accent.surface",
    }));
    assert!(
        fell.iter()
            .any(|one| one.part == "panel-separator.root.hover"),
        "{fell:?}"
    );
}

#[test]
fn the_line_at_rest_is_the_window_s_hairline_and_no_mark() {
    // At rest the handle is the hairline between two regions, which the token floors hold.
    let (sheet, fell) = drawn_over_purlis(json!({
        "panel-separator.root.fill": "border.subtle",
    }));
    assert!(fell.is_empty(), "{fell:?}");
    assert!(
        sheet.contains(&format!(
            ":where({ROOT}) {{ background-color: var(--border-subtle); }}"
        )),
        "{sheet}"
    );
}

#[test]
fn a_clear_mark_holds_in_every_appearance_of_both_built_ins() {
    for base in [PURLIS, CLASSIC] {
        let loaded = load(&over(
            base,
            json!({
                "panel-separator.grip.hover.fill": "text.faint",
                "panel-separator.grip.pressed.fill": "text.muted",
                "panel-separator.root.hover.fill": "transparent",
                "panel-separator.root.pressed.fill": "transparent",
            }),
        ));
        assert!(!loaded.refused, "{:?}", said(&loaded));
        for appearance in &loaded.theme.appearances {
            let fell = parts::fallen(appearance);
            assert!(fell.is_empty(), "{base} {}: {fell:?}", appearance.id);
        }
    }
}

// ---------------------------------------------------------------------------------------
// The defaults, held to the stylesheet
// ---------------------------------------------------------------------------------------

/// A stylesheet with its comments taken out and prettier's wrapping of a long selector read back
/// onto one line.
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
    out.replace("(\n    ", "(")
        .replace(",\n    ", ", ")
        .replace("\n  )", ")")
        .replace("\n  > ", " > ")
}

/// The token the `background-color` of the rule `selector` opens names, or `transparent`.
fn fill_of(css: &str, selector: &str) -> String {
    let at = css
        .find(&format!("\n{selector} {{"))
        .unwrap_or_else(|| panic!("no rule {selector}"));
    let body = &css[at..];
    let body = &body[..body.find('}').unwrap()];
    let value = body
        .lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix("background-color: "))
        .unwrap_or_else(|| panic!("{selector} sets no fill"))
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
fn the_grip_s_defaults_are_what_panel_css_draws() {
    let css = css_of(include_str!("../../../../app/src/ui/panel/panel.css"));
    let grip = |state: &str| format!("{ROOT}{state} > {GRIP}");
    assert_eq!(fill_of(&css, GRIP), parts::separator_fill_of(1, &[]));
    // The two marks the hover state is keyed on, each a rule of its own. The library marks a
    // handle with the keyboard on it `focus`, so only the second can hold with the keyboard, and
    // it gives way to it.
    for hover in [
        "[data-separator=\"hover\"]",
        "[data-separator=\"focus\"]:hover:not(:focus-visible)",
    ] {
        assert_eq!(
            fill_of(&css, &grip(hover)),
            parts::separator_fill_of(1, &[parts::State::Hover]),
            "{hover}"
        );
    }
    assert_eq!(
        fill_of(&css, &grip("[data-separator=\"active\"]")),
        parts::separator_fill_of(1, &[parts::State::Pressed])
    );
    assert_eq!(
        fill_of(&css, &grip(":focus-visible")),
        parts::separator_fill_of(1, &[parts::State::Focused])
    );
    // Under the pointer with the keyboard on it: the focus rule, which no hover rule outranks.
    assert_eq!(
        fill_of(&css, &grip(":focus-visible")),
        parts::separator_fill_of(1, &[parts::State::Hover, parts::State::Focused])
    );
    assert!(
        !css.contains(&format!("{ROOT}[data-separator=\"focus\"]:hover > {GRIP}")),
        "{css}"
    );
    // The handle draws no fill of its own: its line at rest is the regions' (`App.css`).
    for state in [HOVER, "[data-separator=\"active\"]", ":focus-visible"] {
        let Some(at) = css.find(&format!("\n{ROOT}{state} {{")) else {
            continue;
        };
        let body = &css[at..];
        let body = &body[..body.find('}').unwrap()];
        assert!(!body.contains("background"), "{state}: {body}");
    }
    assert_eq!(parts::look_of("panel-separator", "", &[])[0], "transparent");
}

// ---------------------------------------------------------------------------------------
// The built-ins
// ---------------------------------------------------------------------------------------

#[test]
fn purlis_draws_a_quiet_grip_and_never_the_accent() {
    let purlis = purlis();
    assert!(
        !purlis
            .first()
            .parts
            .keys()
            .any(|key| key.family == "panel-separator"),
        "Purlis draws the panel separator's default"
    );
    for states in [&[parts::State::Hover][..], &[parts::State::Pressed]] {
        // Under the pointer and dragged, never the accent.
        let fill = parts::separator_fill_of(1, states);
        assert!(fill.starts_with("text."), "{states:?}: {fill}");
        assert_ne!(parts::separator_fill_of(0, states), "accent.base");
    }
    // With the keyboard on it, the grip in the window's focus colour: a short mark, not a ring
    // the handle's whole length.
    assert_eq!(
        parts::separator_fill_of(1, &[parts::State::Focused]),
        "focus.ring"
    );
    assert_eq!(
        parts::separator_fill_of(1, &[parts::State::Hover, parts::State::Focused]),
        "focus.ring"
    );
    // Quiet under the pointer, clear while dragged: two tiers, the faint one first.
    assert_eq!(
        parts::separator_fill_of(1, &[parts::State::Hover]),
        check::FAINT
    );
    assert_eq!(
        parts::separator_fill_of(1, &[parts::State::Pressed]),
        "text.muted"
    );
}

#[test]
fn classic_keeps_its_accent_line_and_draws_no_grip() {
    let sheet = parts::stylesheet(classic().first());
    for state in [HOVER, "[data-separator=\"active\"]", ":focus-visible"] {
        assert!(
            sheet.contains(&format!(
                ":where({ROOT}{state}) {{ background-color: var(--accent-base); }}"
            )),
            "{state}: {sheet}"
        );
        assert!(
            sheet.contains(&format!(
                ":where({ROOT}{state} {GRIP}) {{ background-color: transparent; }}"
            )),
            "{state}: {sheet}"
        );
    }
    for appearance in &classic().appearances {
        assert!(parts::fallen(appearance).is_empty(), "{}", appearance.id);
    }
}

#[test]
fn a_handle_with_the_keyboard_on_it_that_draws_nothing_draws_its_default() {
    // No grip and no line of its own: nothing says where the keyboard is (WCAG 2.4.7).
    let (sheet, fell) = drawn_over_purlis(json!({
        "panel-separator.grip.focused.fill": "transparent",
    }));
    let one = fell
        .iter()
        .find(|one| one.part == "panel-separator.root.focused")
        .unwrap_or_else(|| panic!("{fell:?}"));
    assert!(one.floor.contains("keyboard"), "{one:?}");
    assert!(
        sheet.contains(&format!(
            ":where({ROOT}:focus-visible {GRIP}) {{ background-color: var(--focus-ring); }}"
        )),
        "{sheet}"
    );
    // Its line in the accent says it as well as a grip does.
    let (_, fell) = drawn_over_purlis(json!({
        "panel-separator.root.focused.fill": "accent.base",
        "panel-separator.grip.focused.fill": "transparent",
    }));
    assert!(fell.is_empty(), "{fell:?}");
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The refused state of the boxes a person gives a value to, at the engine's interface (#3110,
//! #3323, #3333): `invalid` in the manifest since the next release, only on the text input, the
//! textarea, the select and the combobox, keyed on `[data-invalid]`, a colour and never a size,
//! its danger edge what `app/src/ui/field/field.css` and `app/src/ui/select/select.css` draw, and
//! that edge held to 3:1 on the box's fill whatever a theme writes.

use serde_json::{Value, json};

use super::parts::{self, State};
use super::*;

/// A theme over Purlis Classic that sets `parts`.
fn with_parts(parts: Value) -> Value {
    json!({ "name": "Shaped", "extends": "purlis-classic", "parts": parts })
}

/// A theme's parts that set one key.
fn one(key: String, value: Value) -> Value {
    Value::Object([(key, value)].into_iter().collect())
}

/// The sentences a refused theme says.
fn refusal(parts: Value) -> Vec<String> {
    let loaded = load(&with_parts(parts));
    assert!(loaded.refused, "the theme was drawn");
    said(&loaded)
}

fn said(resolved: &Resolved) -> Vec<String> {
    resolved
        .complaints
        .iter()
        .map(|one| one.said.clone())
        .collect()
}

/// The states a family names in the manifest.
fn states_of(manifest: &Value, name: &str) -> Vec<String> {
    manifest["families"]
        .as_array()
        .unwrap()
        .iter()
        .find(|one| one["name"] == name)
        .unwrap_or_else(|| panic!("{name} is in the manifest"))["states"]
        .as_array()
        .unwrap()
        .iter()
        .map(|one| one.as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn invalid_is_a_state_since_next_keyed_on_data_invalid() {
    assert_eq!(State::Invalid.since(), parts::NEXT);
    let manifest = parts::manifest_json();
    let invalid = manifest["states"]
        .as_array()
        .unwrap()
        .iter()
        .find(|one| one["name"] == "invalid")
        .expect("the manifest lists invalid");
    assert_eq!(invalid["attribute"], "[data-invalid]");
    assert_eq!(invalid["since"], parts::NEXT);
    // The release the last version shipped knew no such state.
    let released = parts::manifest_as_of(parts::VERSION);
    assert!(
        !released["states"]
            .as_array()
            .unwrap()
            .iter()
            .any(|one| one["name"] == "invalid")
    );
}

/// The families whose root a person gives a value to, and so can be refused.
const BOXES: [&str; 4] = ["text-input", "textarea", "select", "combobox"];

#[test]
fn only_the_boxes_a_value_is_given_to_are_refused() {
    let manifest = parts::manifest_json();
    for family in BOXES {
        assert!(
            states_of(&manifest, family).contains(&"invalid".to_owned()),
            "{family}"
        );
    }
    for family in parts::MANIFEST
        .iter()
        .map(|one| one.name)
        .filter(|name| !BOXES.contains(name))
    {
        assert!(
            !states_of(&manifest, family).contains(&"invalid".to_owned()),
            "{family} names invalid"
        );
    }
    // A key that names it on a family that is never refused is said and left out.
    let loaded = load(&with_parts(
        json!({ "button.root.invalid.fill": "danger.surface" }),
    ));
    assert!(
        said(&loaded)
            .iter()
            .any(|one| one.contains("parts.button.root.invalid.fill is not")),
        "{:?}",
        said(&loaded)
    );
}

#[test]
fn a_refused_box_is_reached_by_data_invalid_and_never_sized_there() {
    let loaded = load(&with_parts(Value::Object(
        BOXES
            .iter()
            .map(|family| {
                (
                    format!("{family}.root.invalid.border-colour"),
                    json!("danger.text"),
                )
            })
            .collect(),
    )));
    assert!(!loaded.refused, "{:?}", said(&loaded));
    let sheet = parts::stylesheet(loaded.theme.first());
    for family in BOXES {
        let reached = format!(
            ":where([data-scope=\"{family}\"][data-part=\"root\"][data-invalid]) {{ border-color: var(--danger-text); }}"
        );
        assert!(sheet.contains(&reached), "{reached}\n{sheet}");
    }
    // Colour only: a refusal moves nothing around it.
    for family in BOXES {
        let loaded = load(&with_parts(one(
            format!("{family}.root.invalid.border-width"),
            json!(2),
        )));
        assert!(loaded.refused, "{family}: a width inside invalid was drawn");
    }
}

#[test]
fn the_refused_edge_is_what_select_css_draws() {
    let css = include_str!("../../../../app/src/ui/select/select.css");
    let selector = "[data-scope=\"select\"][data-part=\"root\"][data-invalid],\n[data-scope=\"combobox\"][data-part=\"root\"][data-invalid] {";
    let at = css
        .find(selector)
        .unwrap_or_else(|| panic!("select.css draws no refused box"));
    let body = &css[at..];
    let body = &body[..body.find('}').unwrap()];
    assert!(body.contains("border-color: var(--danger-base);"), "{body}");
    // Colour only, in the stylesheet too.
    assert_eq!(body.matches(':').count(), 1, "{body}");
    for family in BOXES {
        let [fill, text, border] = parts::look_of(family, "", &[State::Invalid]);
        assert_eq!(
            [fill, text, border],
            ["surface.sunken", "text.primary", "danger.base"],
            "{family}"
        );
    }
}

#[test]
fn the_refused_edge_is_what_field_css_draws() {
    let css = include_str!("../../../../app/src/ui/field/field.css");
    let selector = ":is([data-scope=\"text-input\"], [data-scope=\"textarea\"])[data-part=\"root\"][data-invalid] {";
    let at = css
        .find(selector)
        .unwrap_or_else(|| panic!("field.css draws no refused box"));
    let body = &css[at..];
    let body = &body[..body.find('}').unwrap()];
    assert!(body.contains("border-color: var(--danger-base);"), "{body}");
    // Colour only, in the stylesheet too.
    assert_eq!(body.matches(':').count(), 2, "{body}");
}

#[test]
fn a_refused_edge_that_cannot_be_seen_on_its_box_falls_back() {
    // Its edge is all that says a box is refused at a glance: 3:1 on its fill, as any edge that
    // is a mark, even on a fill no ground of the box's is (where an edge at rest is not held).
    for family in BOXES {
        let loaded = load(&with_parts(json!({
            format!("{family}.root.invalid.fill"): "control.base",
            format!("{family}.root.invalid.border-colour"): "control.base",
        })));
        assert!(!loaded.refused, "{family}: {:?}", said(&loaded));
        let fallen = parts::fallen(loaded.theme.first());
        assert!(
            fallen
                .iter()
                .any(|one| one.part.starts_with(&format!("{family}.root"))
                    && one.floor == "the root's border contrasts 3:1 with its fill"),
            "{family}: {fallen:?}"
        );
    }
}

// ---------------------------------------------------------------------------------------
// No theme erases the refusal (#3322's review, B1)
// ---------------------------------------------------------------------------------------

/// Where each family's refused root falls back, said by its look's name.
const REFUSED: &[(&str, &str)] = &[
    ("text-input", "text-input.root"),
    ("textarea", "textarea.root"),
    ("select", "select.root"),
    ("combobox", "combobox.root"),
];

/// The floor a refused box breaks when its edge is gone or drawn as its unrefused edge.
const HIDDEN: &str = "a refused box keeps its own edge, told apart from the edge it has when it \
                      is not refused";

/// What fell for `family`'s root in a state set holding `invalid`, as `floor` says.
fn refused_fell(parts: Value, family: &str) -> Vec<String> {
    let loaded = load(&with_parts(parts));
    assert!(!loaded.refused, "{:?}", said(&loaded));
    parts::fallen(loaded.theme.first())
        .into_iter()
        .filter(|one| {
            one.part.starts_with(&format!("{family}.root."))
                && one.part.contains("invalid")
                && one.floor == HIDDEN
        })
        .map(|one| one.part)
        .collect()
}

#[test]
fn a_transparent_refused_edge_falls_back() {
    for (family, _) in REFUSED {
        let fell = refused_fell(
            one(
                format!("{family}.root.invalid.border-colour"),
                json!("transparent"),
            ),
            family,
        );
        assert!(
            fell.contains(&format!("{family}.root.invalid")),
            "{family}: {fell:?}"
        );
    }
}

#[test]
fn a_refused_box_keeps_its_edge_s_width() {
    for (family, root) in REFUSED {
        let refused = refusal(one(format!("{root}.border-width"), json!(0)));
        assert!(
            refused.iter().any(|one| one.starts_with(&format!(
                "parts.{family}.root.border-width is refused: a box that can be refused shows it by \
                 its edge, so its edge keeps a width; for no edge at rest, set \
                 {family}.root.border-colour to \"transparent\""
            ))),
            "{family}: {refused:?}"
        );
        // Any other width is the theme's.
        let loaded = load(&with_parts(one(format!("{root}.border-width"), json!(2))));
        assert!(!loaded.refused, "{family}: {:?}", said(&loaded));
    }
}

#[test]
fn an_edge_at_rest_that_reaches_the_refused_box_falls_back() {
    for (family, _) in REFUSED {
        // The rest edge reaches every state, so refused would look as valid.
        let fell = refused_fell(
            one(
                format!("{family}.root.border-colour"),
                json!("border.strong"),
            ),
            family,
        );
        assert!(
            fell.contains(&format!("{family}.root.invalid")),
            "{family}: {fell:?}"
        );
        // And so does a hover's or a focus's, with the state it holds at once.
        let fell = refused_fell(
            one(
                format!("{family}.root.hover.border-colour"),
                json!("border.strong"),
            ),
            family,
        );
        assert!(
            fell.iter()
                .any(|one| one.contains("hover") && one.contains("invalid")),
            "{family}: {fell:?}"
        );
        // A focus edge too (#3382's review, N3).
        let fell = refused_fell(
            one(
                format!("{family}.root.focused.border-colour"),
                json!("border.strong"),
            ),
            family,
        );
        assert!(
            fell.iter()
                .any(|one| one.contains("focused") && one.contains("invalid")),
            "{family}: {fell:?}"
        );
    }
    // What falls back draws the danger edge in the refused state, after the theme's rules.
    let loaded = load(&with_parts(
        json!({ "select.root.border-colour": "border.strong" }),
    ));
    let sheet = parts::stylesheet(loaded.theme.first());
    let rest = sheet
        .find(":where([data-scope=\"select\"][data-part=\"root\"]) {")
        .expect("the theme's rest rule");
    let refused = sheet
        .rfind("[data-scope=\"select\"][data-part=\"root\"][data-invalid]) {")
        .expect("the refused fallback");
    assert!(refused > rest, "{sheet}");
    assert!(
        sheet[refused..]
            .lines()
            .next()
            .unwrap()
            .contains("var(--danger-base)"),
        "{sheet}"
    );
}

#[test]
fn a_refused_edge_drawn_apart_is_drawn_as_written() {
    let loaded = load(&with_parts(json!({
        "select.root.border-colour": "border.strong",
        "select.root.invalid.border-colour": "danger.base",
    })));
    assert!(!loaded.refused, "{:?}", said(&loaded));
    let fell = parts::fallen(loaded.theme.first());
    assert!(
        !fell.iter().any(|one| one.part == "select.root.invalid"),
        "{fell:?}"
    );
}

#[test]
fn an_edge_at_rest_in_the_danger_colour_still_leaves_the_refusal_told_apart() {
    // The refused default is the danger edge: a theme that draws it at rest, or one alike it,
    // would leave a refused box looking valid, so the edge at rest falls back too (#3382's
    // review, B1).
    for (family, _) in REFUSED {
        for colour in ["danger.base", "danger.text"] {
            for state in ["", "hover.", "focused."] {
                let loaded = load(&with_parts(one(
                    format!("{family}.root.{state}border-colour"),
                    json!(colour),
                )));
                assert!(!loaded.refused, "{family}: {:?}", said(&loaded));
                let theme = loaded.theme.first();
                let held: Vec<State> = match state {
                    "hover." => vec![State::Hover, State::Invalid],
                    "focused." => vec![State::Focused, State::Invalid],
                    _ => vec![State::Invalid],
                };
                assert!(
                    parts::refusal_shown(theme, family, &held),
                    "{family}.root.{state}border-colour {colour}: {:?}",
                    parts::fallen(theme)
                );
                assert!(
                    parts::held_again(theme).is_empty(),
                    "{family} {state}{colour}"
                );
            }
        }
    }
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The select and combobox families' refused state at the engine's interface (#3110): `invalid`
//! in the manifest since the next release, only on the two boxes, keyed on `[data-invalid]`, a
//! colour and never a size, its danger edge what `app/src/ui/select/select.css` draws, and that
//! edge held to 3:1 on the box's fill whatever a theme writes.

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

#[test]
fn only_the_select_and_the_combobox_are_refused() {
    let manifest = parts::manifest_json();
    for family in ["select", "combobox"] {
        assert!(
            states_of(&manifest, family).contains(&"invalid".to_owned()),
            "{family}"
        );
    }
    for family in parts::MANIFEST
        .iter()
        .map(|one| one.name)
        .filter(|name| !matches!(*name, "select" | "combobox"))
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
    let loaded = load(&with_parts(json!({
        "select.root.invalid.border-colour": "danger.text",
        "combobox.root.invalid.border-colour": "danger.text",
    })));
    assert!(!loaded.refused, "{:?}", said(&loaded));
    let sheet = parts::stylesheet(loaded.theme.first());
    for family in ["select", "combobox"] {
        let reached = format!(
            ":where([data-scope=\"{family}\"][data-part=\"root\"][data-invalid]) {{ border-color: var(--danger-text); }}"
        );
        assert!(sheet.contains(&reached), "{reached}\n{sheet}");
    }
    // Colour only: a refusal moves nothing around it.
    let loaded = load(&with_parts(
        json!({ "select.root.invalid.border-width": 2 }),
    ));
    assert!(loaded.refused, "a width inside invalid was drawn");
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
    for family in ["select", "combobox"] {
        let [fill, text, border] = parts::look_of(family, "", &[State::Invalid]);
        assert_eq!(
            [fill, text, border],
            ["surface.sunken", "text.primary", "danger.base"],
            "{family}"
        );
    }
}

#[test]
fn a_refused_edge_that_cannot_be_seen_on_its_box_falls_back() {
    // Its edge is all that says a box is refused at a glance: 3:1 on its fill, as any edge that
    // is a mark, even on a fill no ground of the box's is (where an edge at rest is not held).
    let loaded = load(&with_parts(json!({
        "select.root.invalid.fill": "control.base",
        "select.root.invalid.border-colour": "control.base",
    })));
    assert!(!loaded.refused, "{:?}", said(&loaded));
    let fallen = parts::fallen(loaded.theme.first());
    assert!(
        fallen.iter().any(|one| one.part.starts_with("select.root")
            && one.floor == "the root's border contrasts 3:1 with its fill"),
        "{fallen:?}"
    );
}

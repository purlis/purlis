// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The grip family at the engine's interface (#5290): in the manifest with its root and its dots,
//! keyed on its own states, with no ground a theme can give its root, and its defaults held to
//! `grip.css`.

use serde_json::json;

use super::parts;
use super::*;

const ROOT: &str = "[data-scope=\"grip\"][data-part=\"root\"]";
const DOT: &str = "[data-scope=\"grip\"][data-part=\"dot\"]";
const HOVER: &str = ":hover";
const FOCUSED: &str = ":is(:focus-visible, :focus-visible *)";

#[test]
fn the_grip_is_in_the_manifest_drawn_in_marks_on_its_own_states() {
    let manifest = parts::manifest_json();
    let family = manifest["families"]
        .as_array()
        .unwrap()
        .iter()
        .find(|one| one["name"] == "grip")
        .expect("the grip is in the manifest")
        .clone();
    assert_eq!(family["since"], 14);
    let names: Vec<&str> = family["parts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|one| one["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["root", "mark", "dots", "dot"]);
    // The root has nothing a theme can set: no ground, no edge.
    assert_eq!(family["parts"][0]["properties"], json!([]));
    assert_eq!(family["parts"][3]["properties"], json!(["fill"]));
    assert_eq!(family["states"], json!(["hover", "focused"]));
    assert_eq!(family["marks"], true);
    assert_eq!(
        family["attributes"],
        json!({ "hover": HOVER, "focused": FOCUSED })
    );
}

#[test]
fn a_theme_can_give_the_root_no_ground() {
    for key in [
        "grip.root.fill",
        "grip.root.hover.fill",
        "grip.root.focused.border",
    ] {
        let loaded = load(&json!({
            "name": "Shaped",
            "extends": PURLIS,
            "parts": { key: "accent.base" },
        }));
        assert!(
            loaded
                .complaints
                .iter()
                .any(|one| one.said.contains(&format!("parts.{key} is not a component part"))),
            "{key}: {:?}",
            loaded.complaints
        );
    }
}

#[test]
fn a_theme_reaches_the_dots_in_each_state() {
    let loaded = load(&json!({
        "name": "Shaped",
        "extends": PURLIS,
        "parts": { "grip.dot.hover.fill": "text.primary" },
    }));
    assert!(!loaded.refused, "{:?}", loaded.complaints);
    let sheet = parts::stylesheet(loaded.theme.first());
    assert!(
        sheet.contains(&format!(
            ":where({ROOT}{HOVER} {DOT}) {{ background-color: var(--text-primary); }}"
        )),
        "{sheet}"
    );
}

#[test]
fn the_dots_defaults_are_what_grip_css_draws() {
    // A selector the formatter wrapped is read on one line.
    let css = include_str!("../../../../app/src/ui/grip/grip.css").replace("\n  [", " [");
    let css = css.as_str();
    let rule = |selector: &str| -> String {
        let at = css
            .find(&format!("{selector} {{"))
            .unwrap_or_else(|| panic!("no rule {selector}\n{css}"));
        let body = &css[at..];
        body[..body.find('}').unwrap()].to_owned()
    };
    // At rest, no fill: the mark is drawn in their place.
    assert_eq!(parts::grip_fill_of(3, &[]), "transparent");
    assert!(!rule(&format!("{ROOT} {DOT}")).contains("background-color: var("));
    for (state, selector) in [(parts::State::Hover, HOVER), (parts::State::Focused, FOCUSED)] {
        let token = parts::grip_fill_of(3, &[state]).replace('.', "-");
        assert!(
            rule(&format!("{ROOT}{selector} {DOT}"))
                .contains(&format!("background-color: var(--{token});")),
            "{state:?}"
        );
        // The root draws no ground in any state.
        assert_eq!(parts::grip_fill_of(0, &[state]), "transparent");
    }
    // Nowhere in the family's sheet does the root take a ground, an edge or a shadow.
    for line in css
        .lines()
        .filter(|line| line.starts_with(ROOT) && line.ends_with(" {") && !line.contains(DOT))
    {
        let body = rule(line.trim_end_matches(" {"));
        // A ground or an edge taken away (the one a browser gives a button) is none.
        for declaration in body.lines().map(str::trim) {
            let Some((property, value)) = declaration.split_once(": ") else {
                continue;
            };
            let value = value.trim_end_matches(';');
            if ["background", "border", "box-shadow"]
                .iter()
                .any(|one| property.starts_with(one))
            {
                assert!(
                    ["none", "transparent", "0"].contains(&value),
                    "{line}: {declaration}"
                );
            }
        }
    }
}

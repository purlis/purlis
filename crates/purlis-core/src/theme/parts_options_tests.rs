// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A theme option can carry component parts** (#2946, spec #2897): an option's value is a
//! layer of parts as well as of tokens, with the same keys and bounds as the theme's own `parts`,
//! laid over them in the options' order, and held by the same checks and floors in every
//! combination of appearance and option values the check measures.

use serde_json::{Value, json};

use super::parts;
use super::*;

fn said(resolved: &Resolved) -> Vec<String> {
    resolved
        .complaints
        .iter()
        .map(|one| one.said.clone())
        .collect()
}

fn values(pairs: &[(&str, &str)]) -> Values {
    pairs
        .iter()
        .map(|(id, value)| ((*id).to_owned(), (*value).to_owned()))
        .collect()
}

/// A theme over Purlis Classic with its own `parts` and `options`.
fn shaped(parts: Value, options: Value) -> Value {
    json!({
        "name": "Shaped",
        "extends": "purlis-classic",
        "parts": parts,
        "options": options,
    })
}

fn loaded(document: &Value) -> Theme {
    let loaded = load(document);
    assert!(!loaded.refused, "refused: {:?}", said(&loaded));
    loaded.theme
}

/// A toggle whose on layer is `parts`.
fn pills(parts: Value) -> Value {
    json!({
        "id": "pills", "kind": "toggle", "label": "Pills", "default": false,
        "on": { "parts": parts }
    })
}

/// The stylesheet `theme` draws in its first appearance with `chosen`.
fn sheet(theme: &Theme, chosen: &[(&str, &str)]) -> String {
    let (drawn, complaints) = theme.with(&values(chosen));
    assert!(complaints.is_empty(), "{complaints:?}");
    parts::stylesheet(drawn.first())
}

/// The stylesheet of a theme over Purlis Classic with `parts` and no options.
fn sheet_of(parts: Value) -> String {
    parts::stylesheet(loaded(&shaped(parts, json!([]))).first())
}

#[test]
fn an_option_s_parts_resolve_over_the_theme_s() {
    let theme = loaded(&shaped(
        json!({ "button.root.corner-radius": 4, "button.label.font-weight": 500 }),
        json!([pills(json!({ "button.root.corner-radius": "full" }))]),
    ));
    // Off, the theme's own parts; on, the option's over them, key by key.
    assert_eq!(
        sheet(&theme, &[("pills", "off")]),
        sheet_of(json!({ "button.root.corner-radius": 4, "button.label.font-weight": 500 }))
    );
    assert_eq!(
        sheet(&theme, &[("pills", "on")]),
        sheet_of(json!({ "button.root.corner-radius": "full", "button.label.font-weight": 500 }))
    );
}

#[test]
fn a_choice_s_values_each_carry_parts_and_later_options_win() {
    let theme = loaded(&shaped(
        json!({}),
        json!([
            {
                "id": "corners", "kind": "choice", "label": "Corners", "default": "soft",
                "values": [
                    { "id": "soft", "label": "Soft", "parts": { "button.root.corner-radius": 6 } },
                    { "id": "round", "label": "Round", "parts": { "button.root.corner-radius": "full" } }
                ]
            },
            pills(json!({ "button.root.corner-radius": 0 }))
        ]),
    ));
    assert_eq!(
        sheet(&theme, &[("corners", "round")]),
        sheet_of(json!({ "button.root.corner-radius": "full" }))
    );
    assert_eq!(
        sheet(&theme, &[("corners", "round"), ("pills", "on")]),
        sheet_of(json!({ "button.root.corner-radius": 0 }))
    );
    // The theme as drawn with no choice made is its defaults'.
    assert_eq!(
        parts::stylesheet(theme.first()),
        sheet_of(json!({ "button.root.corner-radius": 6 }))
    );
}

#[test]
fn a_key_purlis_does_not_have_in_an_option_is_said_and_left_out() {
    let loaded = load(&shaped(
        json!({}),
        json!([pills(json!({
            "button.root.sparkle": 2,
            "button.root.corner-radius": "full"
        }))]),
    ));
    assert!(!loaded.refused, "{:?}", said(&loaded));
    let notes = said(&loaded);
    assert!(
        notes.iter().any(|one| one.contains("option pills")
            && one.contains("button.root.sparkle")
            && one.contains("left out")),
        "{notes:?}"
    );
    assert_eq!(
        sheet(&loaded.theme, &[("pills", "on")]),
        sheet_of(json!({ "button.root.corner-radius": "full" }))
    );
}

#[test]
fn a_value_outside_its_bound_in_an_option_refuses_the_theme_naming_the_option() {
    for parts in [
        json!({ "button.root.corner-radius": -1 }),
        json!({ "button.root.hover.padding": 4 }),
        json!({ "button.root.grants.border-width": 0 }),
    ] {
        let loaded = load(&shaped(json!({}), json!([pills(parts.clone())])));
        assert!(loaded.refused, "{parts}");
        assert!(
            said(&loaded)
                .iter()
                .any(|one| one.contains("option pills") && one.contains("parts.button.root")),
            "{parts}: {:?}",
            said(&loaded)
        );
    }
}

#[test]
fn parts_that_do_not_fit_in_some_combination_refuse_the_theme_naming_it() {
    // The theme's buttons are tall; each option fits over them alone, and together the button is
    // too short for its words.
    let loaded = load(&shaped(
        json!({ "button.root.height": 32 }),
        json!([
            {
                "id": "short", "kind": "toggle", "label": "Short", "default": false,
                "on": { "parts": { "button.root.height": 24 } }
            },
            {
                "id": "roomy", "kind": "toggle", "label": "Roomy", "default": false,
                "on": { "parts": { "button.root.padding": [6, 12] } }
            }
        ]),
    ));
    assert!(loaded.refused, "{:?}", said(&loaded));
    assert!(
        said(&loaded)
            .iter()
            .any(|one| one.contains("short on") && one.contains("roomy on")),
        "{:?}",
        said(&loaded)
    );
    for alone in ["short", "roomy"] {
        let one = load(&json!({
            "name": "Shaped",
            "extends": "purlis-classic",
            "parts": { "button.root.height": 32 },
            "options": [match alone {
                "short" => json!({
                    "id": "short", "kind": "toggle", "label": "Short", "default": false,
                    "on": { "parts": { "button.root.height": 24 } }
                }),
                _ => json!({
                    "id": "roomy", "kind": "toggle", "label": "Roomy", "default": false,
                    "on": { "parts": { "button.root.padding": [6, 12] } }
                }),
            }],
        }));
        assert!(!one.refused, "{alone}: {:?}", said(&one));
    }
}

#[test]
fn the_consent_floors_hold_in_each_combination_the_check_measures() {
    // On, the option draws Allow on the primary's ground; the check of that combination says
    // the part falls back, and the window draws Allow's own look there. Off, nothing falls.
    let theme = loaded(&shaped(
        json!({}),
        json!([pills(json!({ "button.root.grants.fill": "accent.surface" }))]),
    ));
    let checked = check_every(&theme);
    assert_eq!(checked.len(), 2 * theme.bases().len());
    for one in &checked {
        let falls = one.report.findings.iter().any(|finding| {
            matches!(finding, check::Finding::PartFallsBack { part, .. } if part.starts_with("button.root.grants"))
        });
        assert_eq!(falls, one.options["pills"] == "on", "{one:?}");
    }
    let on = sheet(&theme, &[("pills", "on")]);
    let allow = on
        .lines()
        .rfind(|line| line.contains("[data-variant=\"grants\"])"))
        .expect("Allow's rule");
    assert!(allow.contains("var(--control-base)"), "{allow}");
}

#[test]
fn an_option_s_parts_are_in_the_schema() {
    let schema = schema();
    let layer = &schema["$defs"]["layer"]["properties"];
    assert_eq!(layer["parts"]["$ref"], "#/properties/parts");
    let value = &schema["$defs"]["option"]["oneOf"][1]["properties"]["values"]["items"];
    assert_eq!(value["properties"]["parts"]["$ref"], "#/properties/parts");
    // The block they name is the theme's own, with every key the manifest has.
    assert!(
        schema["properties"]["parts"]["properties"]
            .get("button.root.corner-radius")
            .is_some()
    );
}

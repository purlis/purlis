// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Theme options at the engine's interface (#2325): a theme document and option values in; the
//! theme drawn, its complaints and the check of every appearance × option combination out.

use serde_json::{Value, json};

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

/// A theme with a dark and a light appearance and `options`.
fn with_options(options: Value) -> Value {
    json!({
        "name": "Dusk",
        "palette": { "rule": "#5a5a66" },
        "appearances": [
            { "id": "night", "appearance": "dark" },
            { "id": "day", "appearance": "light" }
        ],
        "options": options
    })
}

fn borders(default: bool) -> Value {
    json!({
        "id": "borders", "kind": "toggle", "label": "Borders",
        "description": "Draws a rule between panes.", "default": default,
        "on": { "tokens": { "border.subtle": "rule" } }
    })
}

fn accent() -> Value {
    json!({
        "id": "accent", "kind": "choice", "label": "Accent", "default": "lavender",
        "values": [
            { "id": "lavender", "label": "Lavender", "tokens": { "accent.base": "#b4a7f5" } },
            {
                "id": "teal", "label": "Teal", "tokens": { "accent.base": "#2bb3a3" },
                "appearances": { "day": { "tokens": { "accent.base": "#0f6e64" } } }
            }
        ]
    })
}

fn loaded(document: &Value) -> Theme {
    let loaded = load(document);
    assert!(!loaded.refused, "refused: {:?}", said(&loaded));
    loaded.theme
}

fn night(theme: &Theme) -> &ResolvedTheme {
    &theme.appearances[0]
}

fn day(theme: &Theme) -> &ResolvedTheme {
    &theme.appearances[1]
}

// ---------------------------------------------------------------------------------------
// Toggles and choices are token layers
// ---------------------------------------------------------------------------------------

#[test]
fn a_toggle_off_by_default_draws_nothing_until_it_is_turned_on() {
    let theme = loaded(&with_options(json!([borders(false)])));
    let classic_rule = built_in_for(Appearance::Dark).value("border.subtle");
    assert_eq!(night(&theme).value("border.subtle"), classic_rule);

    let (on, complaints) = theme.with(&values(&[("borders", "on")]));
    assert!(complaints.is_empty(), "{complaints:?}");
    assert_eq!(night(&on).value("border.subtle"), "#5a5a66");
    assert_eq!(day(&on).value("border.subtle"), "#5a5a66");
}

#[test]
fn a_toggle_on_by_default_is_drawn_on_when_nothing_chose() {
    let theme = loaded(&with_options(json!([borders(true)])));
    assert_eq!(night(&theme).value("border.subtle"), "#5a5a66");
    let (off, _) = theme.with(&values(&[("borders", "off")]));
    assert_eq!(
        night(&off).value("border.subtle"),
        built_in_for(Appearance::Dark).value("border.subtle")
    );
}

#[test]
fn a_choice_draws_its_default_and_each_value_layers_its_tokens_per_appearance() {
    let theme = loaded(&with_options(json!([accent()])));
    assert_eq!(night(&theme).value("accent.base"), "#b4a7f5");
    assert_eq!(day(&theme).value("accent.base"), "#b4a7f5");

    let (teal, _) = theme.with(&values(&[("accent", "teal")]));
    assert_eq!(night(&teal).value("accent.base"), "#2bb3a3");
    // The value's layer for one appearance goes over its layer for every appearance.
    assert_eq!(day(&teal).value("accent.base"), "#0f6e64");
}

#[test]
fn each_appearance_says_the_option_values_it_was_drawn_with_and_whether_they_were_chosen() {
    let theme = loaded(&with_options(json!([borders(false), accent()])));
    let drawn: Vec<(&str, &str, bool)> = night(&theme)
        .options
        .iter()
        .map(|one| (one.option.id.as_str(), one.value.as_str(), one.chosen))
        .collect();
    assert_eq!(
        drawn,
        [("borders", "off", false), ("accent", "lavender", false)]
    );

    let (chosen, _) = theme.with(&values(&[("accent", "teal")]));
    let drawn: Vec<(&str, &str, bool)> = day(&chosen)
        .options
        .iter()
        .map(|one| (one.option.id.as_str(), one.value.as_str(), one.chosen))
        .collect();
    assert_eq!(drawn, [("borders", "off", false), ("accent", "teal", true)]);
}

#[test]
fn a_value_the_option_does_not_have_draws_its_default_and_says_so() {
    let theme = loaded(&with_options(json!([accent()])));
    let (drawn, complaints) = theme.with(&values(&[("accent", "mauve"), ("glow", "on")]));
    assert_eq!(night(&drawn).value("accent.base"), "#b4a7f5");
    let complaints: Vec<&str> = complaints.iter().map(|one| one.said.as_str()).collect();
    assert_eq!(
        complaints,
        [
            "the option accent of Dusk has no value \"mauve\", so its default, lavender, is drawn",
            "Dusk has no option \"glow\", so it is left out",
        ]
    );
}

#[test]
fn a_theme_with_no_options_draws_as_it_did() {
    let theme = loaded(&json!({
        "name": "Plain",
        "appearances": [{ "id": "dark", "appearance": "dark" }]
    }));
    assert!(theme.options.is_empty());
    assert!(night(&theme).options.is_empty());
    assert_eq!(theme.combinations(), 1);
    assert_eq!(night(&theme).values, built_in_for(Appearance::Dark).values);
}

// ---------------------------------------------------------------------------------------
// The grammar: toggles and choices only, refused whole when broken
// ---------------------------------------------------------------------------------------

#[test]
fn an_option_that_is_not_a_toggle_or_a_choice_refuses_the_theme_there_are_no_free_numbers() {
    let refused = load(&with_options(json!([{
        "id": "radius", "kind": "number", "label": "Corner radius", "default": 4,
        "min": 0, "max": 12
    }])));
    assert!(refused.refused);
    assert_eq!(
        said(&refused),
        [
            "option radius is of kind \"number\"; a theme option is a toggle or a choice, and \
          there are no free-number options"
        ]
    );
}

#[test]
fn a_broken_option_refuses_the_theme_and_every_fault_is_said() {
    let refused = load(&with_options(json!([
        { "id": "borders", "kind": "toggle", "label": "", "default": "yes", "on": {} },
        {
            "id": "accent", "kind": "choice", "label": "Accent", "default": "rose",
            "values": [
                { "id": "lavender", "label": "Lavender", "tokens": { "accent.base": "lilac" } },
                { "id": "lavender", "label": "Again" }
            ]
        },
        { "id": "Glow", "kind": "toggle" }
    ])));
    assert!(refused.refused);
    assert_eq!(
        said(&refused),
        [
            "the label of option borders is \"\"; it is one line of text, not empty, at most 64 \
             characters",
            "the default of option borders is \"yes\"; a toggle's default is true or false",
            "accent.base in value lavender of option accent is \"lilac\", which is neither a hex \
             colour nor a name in this theme's palette",
            "two values of option accent are called lavender",
            "the default of option accent is \"rose\", which is not one of its values",
            "option id \"Glow\" is not lower-case letters, digits and dashes starting with a \
             letter, at most 32 long",
        ]
    );
}

#[test]
fn a_layer_for_an_appearance_the_theme_does_not_have_refuses_it() {
    let refused = load(&with_options(json!([{
        "id": "accent", "kind": "choice", "label": "Accent", "default": "a",
        "values": [
            { "id": "a", "label": "A" },
            { "id": "b", "label": "B", "appearances": { "dusk": { "tokens": {} } } }
        ]
    }])));
    assert!(refused.refused);
    assert_eq!(
        said(&refused),
        ["option accent value b names appearance dusk, which the theme does not have"]
    );
}

#[test]
fn a_token_name_purlis_does_not_have_in_an_option_is_left_out_and_said() {
    let loaded = load(&with_options(json!([{
        "id": "borders", "kind": "toggle", "label": "Borders", "default": true,
        "on": { "tokens": { "border.glow": "#ffffff", "border.subtle": "#222222" } }
    }])));
    assert!(!loaded.refused);
    assert_eq!(night(&loaded.theme).value("border.subtle"), "#222222");
    assert_eq!(
        said(&loaded),
        ["border.glow in the on layer of option borders is not a purlis token, so it is left out"]
    );
}

// ---------------------------------------------------------------------------------------
// The cap: every appearance × option combination is checkable
// ---------------------------------------------------------------------------------------

fn choice(id: &str, count: usize) -> Value {
    let values: Vec<Value> = (0..count)
        .map(|at| json!({ "id": format!("v{at}"), "label": format!("V{at}") }))
        .collect();
    json!({ "id": id, "kind": "choice", "label": id, "default": "v0", "values": values })
}

#[test]
fn a_theme_at_the_cap_is_drawn_and_one_past_it_is_refused_with_its_count() {
    // Two appearances × 2 × 4 × 4 = 64.
    let at_cap = load(&with_options(json!([
        borders(false),
        choice("a", 4),
        choice("b", 4)
    ])));
    assert!(!at_cap.refused, "{:?}", said(&at_cap));
    assert_eq!(at_cap.theme.combinations(), MOST_COMBINATIONS);

    let past = load(&with_options(json!([
        borders(false),
        choice("a", 4),
        choice("b", 5)
    ])));
    assert!(past.refused);
    assert_eq!(
        said(&past),
        [
            "Dusk has 80 combinations of appearance and option values (2 appearances x 2 x 4 x 5), \
          and the theme check covers at most 64"
        ]
    );
}

#[test]
fn the_cap_counts_the_options_a_theme_has_from_the_one_it_extends() {
    let library = Library::new().with("base", with_options(json!([choice("a", 4)])));
    let child = library.load(
        None,
        &json!({
            "name": "Child", "extends": "base",
            "appearances": [{ "id": "hc", "appearance": "dark", "high-contrast": true }],
            "options": [choice("b", 6)]
        }),
    );
    assert!(child.refused);
    assert_eq!(
        said(&child),
        [
            "Child has 72 combinations of appearance and option values (3 appearances x 4 x 6), \
          and the theme check covers at most 64"
        ]
    );
}

// ---------------------------------------------------------------------------------------
// extends
// ---------------------------------------------------------------------------------------

#[test]
fn a_child_has_its_parent_s_options_and_one_of_its_own_takes_that_one_s_place() {
    let library = Library::new().with("base", with_options(json!([borders(false), accent()])));
    let child = library.load(
        None,
        &json!({
            "name": "Child", "extends": "base",
            "appearances": [{ "id": "night", "tokens": { "surface.base": "#000000" } }],
            "options": [
                {
                    "id": "borders", "kind": "toggle", "label": "Rules", "default": true,
                    "on": { "tokens": { "border.subtle": "#777777" } }
                },
                choice("density-tint", 2)
            ]
        }),
    );
    assert!(!child.refused, "{:?}", said(&child));
    let theme = child.theme;
    let ids: Vec<&str> = theme.options.iter().map(|one| one.id.as_str()).collect();
    assert_eq!(ids, ["borders", "accent", "density-tint"]);
    // The child's own toggle, on by default, over its own appearance's tokens.
    assert_eq!(night(&theme).value("border.subtle"), "#777777");
    assert_eq!(night(&theme).value("surface.base"), "#000000");
    // The parent's choice still layers over the child's appearances.
    let (teal, _) = theme.with(&values(&[("accent", "teal")]));
    assert_eq!(day(&teal).value("accent.base"), "#0f6e64");
}

// ---------------------------------------------------------------------------------------
// The theme check covers every combination
// ---------------------------------------------------------------------------------------

#[test]
fn the_check_covers_every_appearance_and_option_combination_and_names_each() {
    let theme = loaded(&with_options(json!([borders(false), accent()])));
    let covered: Vec<(String, Vec<(String, String)>)> = check_every(&theme)
        .into_iter()
        .map(|one| (one.appearance, one.options.into_iter().collect()))
        .collect();
    let pair = |b: &str, a: &str| {
        vec![
            ("accent".to_owned(), a.to_owned()),
            ("borders".to_owned(), b.to_owned()),
        ]
    };
    assert_eq!(
        covered,
        [
            ("night".to_owned(), pair("off", "lavender")),
            ("night".to_owned(), pair("off", "teal")),
            ("night".to_owned(), pair("on", "lavender")),
            ("night".to_owned(), pair("on", "teal")),
            ("day".to_owned(), pair("off", "lavender")),
            ("day".to_owned(), pair("off", "teal")),
            ("day".to_owned(), pair("on", "lavender")),
            ("day".to_owned(), pair("on", "teal")),
        ]
    );
}

#[test]
fn a_contrast_failure_only_one_option_value_makes_is_found_in_that_combination_alone() {
    let theme = loaded(&json!({
        "name": "Faint",
        "appearances": [{ "id": "dark", "appearance": "dark" }],
        "options": [{
            "id": "quiet", "kind": "toggle", "label": "Quiet text", "default": false,
            "on": { "tokens": { "text.primary": "#2a2a2a" } }
        }]
    }));
    let failing: Vec<String> = check_every(&theme)
        .into_iter()
        .filter(|one| !one.report.passes())
        .map(|one| format!("{} quiet={}", one.appearance, one.options["quiet"]))
        .collect();
    assert_eq!(failing, ["dark quiet=on"]);
}

#[test]
fn a_theme_with_no_options_is_checked_once_per_appearance() {
    let covered: Vec<String> = check_every(classic())
        .into_iter()
        .map(|one| {
            assert!(one.options.is_empty());
            one.appearance
        })
        .collect();
    assert_eq!(covered, ["dark", "light"]);
}

// ---------------------------------------------------------------------------------------
// The schema
// ---------------------------------------------------------------------------------------

#[test]
fn the_schema_describes_toggles_and_choices_and_nothing_else() {
    let schema = schema();
    let option = &schema["$defs"]["option"];
    let kinds: Vec<&str> = option["oneOf"]
        .as_array()
        .expect("an option is one of two shapes")
        .iter()
        .map(|one| one["properties"]["kind"]["const"].as_str().expect("a kind"))
        .collect();
    assert_eq!(kinds, ["toggle", "choice"]);
    assert_eq!(
        schema["properties"]["options"]["items"]["$ref"],
        "#/$defs/option"
    );
}

// ---------------------------------------------------------------------------------------
// A layer holds whatever an appearance's tokens may: geometry within its bounds too
// ---------------------------------------------------------------------------------------

fn corners(value: Value) -> Value {
    json!([{
        "id": "corners", "kind": "choice", "label": "Corners", "default": "square",
        "values": [
            { "id": "square", "label": "Square" },
            {
                "id": "soft", "label": "Soft", "tokens": { "radius.md": value },
                "appearances": { "day": { "tokens": { "accent.base": "#0f6e64" } } }
            }
        ]
    }])
}

#[test]
fn an_option_value_sets_geometry_within_its_bound_and_compact_follows() {
    let theme = loaded(&with_options(corners(json!(8))));
    let classic_md = built_in_for(Appearance::Dark).geometry["radius.md"];
    assert_eq!(night(&theme).geometry["radius.md"], classic_md);

    let (soft, complaints) = theme.with(&values(&[("corners", "soft")]));
    assert!(complaints.is_empty(), "{complaints:?}");
    assert_eq!(night(&soft).geometry["radius.md"], 8.0);
    assert_eq!(day(&soft).geometry["radius.md"], 8.0);
    // What compact draws is worked out from the geometry the option drew.
    assert_eq!(night(&soft).compact["radius.md"], 8.0);
    assert_eq!(day(&soft).value("accent.base"), "#0f6e64");
}

#[test]
fn an_option_value_s_geometry_outside_its_bound_refuses_the_theme_and_names_the_bound() {
    let refused = load(&with_options(corners(json!(40))));
    assert!(refused.refused);
    let said = said(&refused);
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(
        said[0].starts_with("radius.md in value soft of option corners is 40, outside its bound"),
        "{said:?}"
    );
}

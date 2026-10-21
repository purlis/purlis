// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Component parts at the engine's interface (#2903): the manifest, the key grammar, the bounds
//! and the corner clamp, the state-size rule, the fallback to the family default, the floors per
//! part and state with the consent parts, and the stylesheet the window draws.

use serde_json::{Value, json};

use super::parts::{self, Prop, STATES};
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

/// The stylesheet of a theme's first appearance, which must be drawn.
fn sheet_of(parts: Value) -> String {
    let loaded = load(&with_parts(parts));
    assert!(!loaded.refused, "{:?}", said(&loaded));
    parts::stylesheet(loaded.theme.first())
}

/// The sentences a refused theme says.
fn refusal(parts: Value) -> Vec<String> {
    let loaded = load(&with_parts(parts));
    assert!(loaded.refused, "the theme was drawn");
    said(&loaded)
}

// ---------------------------------------------------------------------------------------
// The tracer: a theme rounds the button's root to a pill
// ---------------------------------------------------------------------------------------

#[test]
fn a_theme_rounds_the_buttons_root_to_a_pill() {
    let sheet = sheet_of(json!({ "button.root.corner-radius": "full" }));
    assert_eq!(
        sheet,
        "@layer parts {\n  :where(.ui-button-sm) { --purlis-part-height: var(--size-chrome); }\n  \
         :where([data-scope=\"button\"][data-part=\"root\"], .ui-button) { border-radius: \
         9999px; }\n}\n"
    );
}

#[test]
fn a_theme_that_names_no_parts_draws_exactly_as_today() {
    for theme in built_ins() {
        assert!(theme.parts.is_empty(), "{} names parts", theme.name);
        assert_eq!(parts::stylesheet(theme), "");
        assert!(parts::fallen(theme).is_empty());
    }
    let loaded = load(&json!({ "name": "Plain", "extends": "purlis-classic" }));
    assert_eq!(parts::stylesheet(loaded.theme.first()), "");
    let empty = load(&with_parts(json!({})));
    assert!(!empty.refused);
    assert_eq!(parts::stylesheet(empty.theme.first()), "");
}

// ---------------------------------------------------------------------------------------
// The key grammar and the values
// ---------------------------------------------------------------------------------------

#[test]
fn a_key_names_a_variant_and_a_state_and_is_keyed_on_their_attributes() {
    let sheet = sheet_of(json!({ "button.root.primary.hover.fill": "accent.base" }));
    assert!(sheet.contains(
        "[data-scope=\"button\"][data-part=\"root\"][data-variant=\"primary\"][data-hovered]"
    ));
    assert!(
        sheet.contains(
            ".ui-button.ui-button-primary:not(.ui-button-danger, .ui-button-grants):hover:not(:disabled, [aria-disabled=\"true\"])"
        )
    );
    assert!(sheet.contains("background-color: var(--accent-base);"));
}

#[test]
fn the_plain_variant_is_reached_on_todays_class_without_another_variant() {
    let sheet = sheet_of(json!({ "button.root.secondary.fill": "control.hover" }));
    assert!(sheet.contains(
        ".ui-button:not(.ui-button-primary, .ui-button-ghost, .ui-button-danger, .ui-button-grants)"
    ));
}

#[test]
fn a_part_other_than_the_root_is_reached_inside_its_root() {
    let sheet = sheet_of(json!({ "button.label.ghost.text": "text.secondary" }));
    assert!(sheet.contains(
        "[data-scope=\"button\"][data-part=\"root\"][data-variant=\"ghost\"] \
         [data-scope=\"button\"][data-part=\"label\"]"
    ));
    assert!(sheet.contains("color: var(--text-secondary);"));
}

#[test]
fn a_key_naming_nothing_purlis_has_is_said_and_left_out() {
    let loaded = load(&with_parts(json!({
        "button.root.corner-radius": 6,
        "spinner.root.fill": "accent.base",
        "button.knob.fill": "accent.base",
        "button.root.opacity": 0.5,
        "button.icon.fill": "accent.base",
        "button.root.primary.rest.fill": "accent.base",
    })));
    assert!(!loaded.refused, "{:?}", said(&loaded));
    let notes = said(&loaded);
    for key in [
        "spinner.root.fill",
        "button.knob.fill",
        "button.root.opacity",
        "button.icon.fill",
        "button.root.primary.rest.fill",
    ] {
        assert!(
            notes
                .iter()
                .any(|one| one.contains(&format!("parts.{key} is not"))),
            "{key}: {notes:?}"
        );
    }
    assert_eq!(loaded.theme.first().parts.len(), 1);
}

#[test]
fn a_colour_names_a_semantic_token_never_a_hex_or_a_palette_name() {
    let said = refusal(json!({ "button.root.fill": "#ff0000" }));
    assert_eq!(
        said,
        [
            "parts.button.root.fill is \"#ff0000\": a part's colour names a semantic colour token \
          (accent.base) or transparent, never a hex or a palette name, so palettes, appearances \
          and tints keep working"
        ]
    );
    let loaded = load(&json!({
        "name": "Named", "extends": "purlis-classic",
        "palette": { "rose": "#ff00aa" },
        "parts": { "button.root.fill": "rose" }
    }));
    assert!(loaded.refused);
    let clear = load(&with_parts(
        json!({ "button.root.ghost.fill": "transparent" }),
    ));
    assert!(!clear.refused);
    assert!(!refusal(json!({ "button.root.text": "transparent" })).is_empty());
}

#[test]
fn a_size_is_a_number_in_its_bound_or_a_geometry_token() {
    let sheet = sheet_of(json!({
        "button.root.padding": [3, "space.5"],
        "button.root.gap": "space.1",
        "button.root.font-weight": 600,
        "button.root.border-width": 2,
        "button.root.shadow": "raised",
    }));
    assert!(sheet.contains("padding-block: calc(3rem / 14); padding-inline: var(--space-5);"));
    assert!(sheet.contains("gap: var(--space-1);"));
    assert!(sheet.contains("font-weight: 600;"));
    assert!(sheet.contains("border-width: 2px; border-style: solid;"));
    assert!(sheet.contains("box-shadow: var(--elevation-raised);"));
    for (key, value, bound) in [
        ("button.root.gap", json!(40), "from 0 to 16"),
        ("button.root.border-width", json!(9), "from 0 to 4"),
        ("button.root.font-size", json!(30), "from 10 to 18"),
        ("button.root.padding", json!([20, 4]), "from 0 to 12"),
        ("button.root.font-weight", json!(950), "from 300 to 700"),
        ("button.root.corner-radius", json!(-1), "from 0 to 9999"),
    ] {
        let said = refusal(json!({ key: value }));
        assert!(
            said.iter().any(|one| one.contains(bound)),
            "{key}: {said:?}"
        );
    }
    let said = refusal(json!({ "button.root.gap": "radius.md" }));
    assert!(said[0].contains("a space.* token"), "{said:?}");
}

#[test]
fn a_geometry_token_is_held_to_the_parts_bound_at_the_value_it_draws() {
    // space.8 is 32 comfortable and 24 compact, past a gap's 16.
    let refused = refusal(json!({ "button.root.gap": "space.8" }));
    assert_eq!(
        refused,
        [
            "parts.button.root.gap is space.8, 32 in comfortable, outside its bound: from 0 to 16",
            "parts.button.root.gap is space.8, 24 in compact, outside its bound: from 0 to 16",
        ]
    );
    for (key, token) in [
        ("button.root.font-size", "font-size.xl"),
        ("button.root.padding", "space.6"),
    ] {
        let refused = refusal(json!({ key: token }));
        assert!(
            refused.iter().any(|one| one.contains("outside its bound")),
            "{key}: {refused:?}"
        );
    }
    // The value a theme gives the token is the one held: Classic's strong is 600, inside.
    let strong = load(&with_parts(
        json!({ "button.root.font-weight": "font-weight.strong" }),
    ));
    assert!(!strong.refused, "{:?}", said(&strong));
    let heavy = load(&json!({
        "name": "Heavy", "extends": "purlis-classic",
        "appearances": [{ "id": "dark", "tokens": { "font-weight.strong": 800 } }],
        "parts": { "button.root.font-weight": "font-weight.strong" }
    }));
    assert!(heavy.refused);
    assert!(
        said(&heavy)[0].contains("800 in comfortable"),
        "{:?}",
        said(&heavy)
    );
}

#[test]
fn a_size_never_reaches_todays_small_or_wrapping_buttons() {
    let sheet = sheet_of(json!({
        "button.root.height": 32,
        "button.root.fill": "control.hover",
    }));
    let sized = sheet
        .lines()
        .find(|line| line.contains("block-size"))
        .expect("the height's rule");
    assert!(
        sized.contains(".ui-button:not(.ui-button-sm, .ui-button-row > .ui-button)"),
        "{sized}"
    );
    assert!(!sized.contains("background-color"), "{sized}");
    let coloured = sheet
        .lines()
        .find(|line| line.contains("background-color: var(--control-hover)"))
        .expect("the fill's rule");
    assert!(!coloured.contains(":not(.ui-button-sm"), "{coloured}");
}

// ---------------------------------------------------------------------------------------
// Corners, heights and the room for words
// ---------------------------------------------------------------------------------------

#[test]
fn a_corner_is_clamped_to_half_the_parts_height_as_it_is_drawn() {
    let half = "calc(var(--purlis-part-height, var(--size-control)) / 2)";
    let sheet = sheet_of(json!({ "button.root.corner-radius": 12 }));
    assert!(
        sheet.contains(&format!("border-radius: min(12px, {half});")),
        "{sheet}"
    );
    // Today's small button is clamped to its own height.
    assert!(sheet.contains(":where(.ui-button-sm) { --purlis-part-height: var(--size-chrome); }"));
    let four = sheet_of(json!({ "button.root.corner-radius": ["full", 0, "radius.md", 4] }));
    assert!(four.contains(&format!(
        "border-radius: 9999px min(0px, {half}) min(var(--radius-md), {half}) min(4px, {half});"
    )));
    // A variant's own height is the one a corner set for every variant is clamped to: the rule
    // that sets the height sets the property the clamp reads.
    let tall = sheet_of(json!({
        "button.root.primary.height": 32,
        "button.root.corner-radius": 20,
    }));
    assert!(tall.contains(&format!("border-radius: min(20px, {half});")));
    assert!(tall.contains("block-size: calc(32rem / 14); --purlis-part-height: calc(32rem / 14);"));
    // `full` is the largest corner, a pill at any height.
    let pill = sheet_of(json!({
        "button.root.primary.height": 32,
        "button.root.corner-radius": "full",
    }));
    assert!(pill.contains("border-radius: 9999px;"));
    assert!(
        refusal(json!({ "button.root.corner-radius": [1, 2] }))[0].contains("one value or four")
    );
}

#[test]
fn a_control_stays_a_target_and_its_words_fit_in_both_densities() {
    let said = refusal(json!({ "button.root.height": 20 }));
    assert!(said[0].contains("from 24 to 48"), "{said:?}");
    // size.chrome is 21 comfortable and 18 compact: below a target, in both.
    let said = refusal(json!({ "button.root.height": "size.chrome" }));
    for density in [("21", "comfortable"), ("18", "compact")] {
        assert!(
            said.contains(&format!(
                "parts: button.root.secondary is {}px tall in {}, below the 24px a control \
                 keeps so it can be reached",
                density.0, density.1
            )),
            "{said:?}"
        );
    }
    // 14px words, 6px of padding above and below and a 1px border take 28px: more than 24.
    let said = refusal(json!({ "button.root.height": 24, "button.root.padding": [6, 8] }));
    assert!(
        said.contains(
            &"parts: button.root.secondary's words, padding and border take 28px in comfortable, \
          more than its 24px height, so its words would not fit"
                .to_owned()
        )
    );
    // In compact the control is 24px: room for 14px words and 4px of padding, not 6.
    let fits = load(&with_parts(json!({ "button.root.padding": [4, 8] })));
    assert!(!fits.refused, "{:?}", super::parts_tests::said(&fits));
    let compact = refusal(json!({ "button.root.padding": [6, 8] }));
    assert!(
        compact.iter().all(|one| one.contains("in compact")),
        "{compact:?}"
    );
}

// ---------------------------------------------------------------------------------------
// A state never changes size
// ---------------------------------------------------------------------------------------

#[test]
fn a_size_inside_a_state_is_refused_with_a_sentence_that_says_why() {
    for property in parts::PROPERTIES.iter().filter(|one| !one.in_state()) {
        for state in STATES {
            let key = format!("button.root.{}.{}", state.name(), property.name());
            let value = match property {
                Prop::Padding => json!(4),
                Prop::FontWeight => json!(500),
                Prop::FontSize => json!(12),
                Prop::Height => json!(28),
                _ => json!(2),
            };
            let said = refusal(json!({ key.clone(): value }));
            assert_eq!(said.len(), 1, "{key}: {said:?}");
            assert!(
                said[0].starts_with(&format!(
                    "parts.{key} is refused: {}",
                    parts::NO_SIZE_IN_A_STATE
                )),
                "{said:?}"
            );
        }
    }
    let said = refusal(json!({ "button.root.primary.selected.font-weight": 600 }));
    assert_eq!(
        said,
        [
            "parts.button.root.primary.selected.font-weight is refused: a state never changes a \
          part's size, so nothing moves when the pointer, a press, the keyboard or a choice \
          reaches it; inside a state only fill, text, border-colour and shadow may be set. Give \
          font-weight to button.root.primary at rest"
        ]
    );
}

// ---------------------------------------------------------------------------------------
// Unset properties are the family default
// ---------------------------------------------------------------------------------------

#[test]
fn a_property_left_unset_is_the_family_default() {
    // Only what the theme names is written: everything else is the window's own stylesheet.
    let sheet = sheet_of(json!({ "button.root.ghost.corner-radius": 4 }));
    assert_eq!(sheet.lines().count(), 4, "{sheet}");
    assert!(!sheet.contains("background-color") && !sheet.contains("color:"));
}

#[test]
fn a_child_theme_layers_its_parts_over_its_parents() {
    let library = Library::new().with(
        "round",
        with_parts(json!({ "button.root.corner-radius": "full", "button.root.gap": 6 })),
    );
    let child = library.load(
        None,
        &json!({ "name": "Child", "extends": "round", "parts": { "button.root.gap": 2 } }),
    );
    assert!(!child.refused, "{:?}", said(&child));
    let sheet = parts::stylesheet(child.theme.first());
    assert!(sheet.contains("border-radius: 9999px;"));
    assert!(sheet.contains("gap: calc(2rem / 14);"));
}

#[test]
fn a_theme_folders_zip_gives_its_parts_back() {
    let document = with_parts(json!({ "button.root.corner-radius": "full" }));
    let files = Files::from([(
        THEME_JSON.to_owned(),
        serde_json::to_vec_pretty(&document).unwrap(),
    )]);
    let (name, back) = unpack(&pack("shaped", &files).unwrap()).unwrap();
    assert_eq!(name, "shaped");
    let read: Value = serde_json::from_slice(&back[THEME_JSON]).unwrap();
    assert_eq!(
        load(&read).theme.first().parts,
        load(&document).theme.first().parts
    );
}

// ---------------------------------------------------------------------------------------
// Floors per part and state, and the consent parts
// ---------------------------------------------------------------------------------------

#[test]
fn a_part_whose_words_break_their_floor_draws_its_family_default_there() {
    let loaded = load(&with_parts(
        json!({ "button.root.hover.fill": "text.primary" }),
    ));
    let theme = loaded.theme.first();
    let fallen = parts::fallen(theme);
    let names: Vec<&str> = fallen.iter().map(|one| one.part.as_str()).collect();
    // Every variant whose words are text.primary breaks under the pointer; danger's words are
    // its own colour, which still reads on a light fill.
    assert!(names.contains(&"button.root.secondary.hover"), "{names:?}");
    assert!(names.contains(&"button.root.primary.hover"), "{names:?}");
    assert!(fallen[0].floor.contains("4.5:1"), "{fallen:?}");
    assert!(fallen[0].ratio.is_some_and(|ratio| ratio < 4.5));
    let sheet = parts::stylesheet(theme);
    assert!(sheet.contains("[data-variant=\"secondary\"][data-hovered]"));
    assert!(sheet.contains("background-color: var(--control-hover);"));
    // The theme check says each one.
    let report = check(theme);
    assert!(report.findings.iter().any(|one| matches!(
        one,
        check::Finding::PartFallsBack { part, .. } if part == "button.root.secondary.hover"
    )));
}

#[test]
fn a_part_that_clears_its_floors_is_drawn_as_written() {
    let loaded = load(&with_parts(json!({
        "button.root.primary.fill": "accent.surface",
        "button.root.ghost.hover.fill": "control.hover",
    })));
    let fallen = parts::fallen(loaded.theme.first());
    assert!(fallen.is_empty(), "{fallen:?}");
}

#[test]
fn allows_edge_cannot_be_turned_off() {
    let said = refusal(json!({ "button.root.grants.border-width": 0 }));
    assert_eq!(
        said,
        [
            "parts.button.root.grants.border-width is refused: an answer that grants keeps its \
          accent edge and its lock, so a theme can never disguise a consent decision"
        ]
    );
    // A theme with no borders anywhere still draws Allow's.
    let sheet = sheet_of(json!({ "button.root.border-width": 0 }));
    let allow = sheet
        .lines()
        .find(|line| line.contains("[data-variant=\"grants\"]"))
        .expect("Allow's edge is drawn back");
    assert!(allow.contains("border-width: 1px;"), "{allow}");
    assert!(
        sheet.find("border-width: 0px").unwrap() < sheet.find("border-width: 1px").unwrap(),
        "Allow's edge is written after the theme's, so it wins"
    );
    // Its edge's colour is held too: Keep blocked's colour is no edge.
    let loaded = load(&with_parts(
        json!({ "button.root.grants.border-colour": "border.strong" }),
    ));
    let fallen = parts::fallen(loaded.theme.first());
    assert!(
        fallen.iter().any(|one| one.part == "button.root.grants"),
        "{fallen:?}"
    );
}

#[test]
fn allow_never_takes_the_primary_look() {
    let loaded = load(&with_parts(
        json!({ "button.root.grants.fill": "accent.surface" }),
    ));
    let theme = loaded.theme.first();
    let fallen = parts::fallen(theme);
    assert!(
        fallen.iter().any(|one| one.part == "button.root.grants"),
        "{fallen:?}"
    );
    let sheet = parts::stylesheet(theme);
    let last_grants = sheet
        .lines()
        .rfind(|line| line.contains("[data-variant=\"grants\"], "))
        .unwrap();
    assert!(
        last_grants.contains("background-color: var(--control-base);"),
        "{last_grants}"
    );
    // Two looks are compared as they are drawn, not by the tokens' names: a token that draws
    // the accent's colour is the accent.
    let aliased = load(&with_parts(json!({
        "button.root.primary.fill": "control.base",
        "button.root.primary.border-colour": "focus.ring",
        "button.root.primary.hover.fill": "control.hover",
    })));
    let fallen = parts::fallen(aliased.theme.first());
    assert!(
        fallen.iter().any(|one| one.part == "button.root.primary"
            && one.floor == "Allow never takes the primary look"),
        "{fallen:?}"
    );
}

#[test]
fn keep_blocked_never_takes_allows_edge() {
    for parts in [
        json!({ "button.root.border-colour": "accent.base" }),
        json!({ "button.root.secondary.border-colour": "accent.base" }),
        json!({ "button.root.fill": "control.base", "button.root.border-colour": "accent.base" }),
    ] {
        let loaded = load(&with_parts(parts.clone()));
        let theme = loaded.theme.first();
        let fallen = parts::fallen(theme);
        assert!(
            fallen.iter().any(|one| one.part == "button.root.secondary"
                && one.floor == "Allow is distinguishable from Keep blocked by its edge"),
            "{parts}: {fallen:?}"
        );
        // Keep blocked draws its own edge again, last, so it wins.
        let sheet = parts::stylesheet(theme);
        let keep_blocked = sheet
            .lines()
            .rfind(|line| line.contains("[data-variant=\"secondary\"], "))
            .expect("Keep blocked's rule");
        assert!(
            keep_blocked.contains("border-color: var(--border-strong);"),
            "{keep_blocked}"
        );
    }
}

#[test]
fn allows_edge_is_drawn_whatever_else_the_theme_sets_on_it() {
    let sheet = sheet_of(json!({
        "button.root.border-width": 0,
        "button.root.grants.fill": "control.hover",
    }));
    let edge = sheet
        .lines()
        .rfind(|line| line.contains("[data-variant=\"grants\"], ") && line.contains("border-width"))
        .expect("Allow's edge is drawn back");
    assert!(edge.contains("border-width: 1px;"), "{edge}");
    assert!(sheet.find("border-width: 0px").unwrap() < sheet.find("border-width: 1px").unwrap());
}

#[test]
fn danger_never_takes_the_primary_look() {
    let loaded = load(&with_parts(json!({
        "button.root.danger.fill": "accent.surface",
        "button.root.danger.border-colour": "accent.base",
        "button.root.danger.text": "text.primary",
    })));
    let theme = loaded.theme.first();
    let fallen = parts::fallen(theme);
    assert!(
        fallen.iter().any(|one| one.part == "button.root.danger"),
        "{fallen:?}"
    );
    let sheet = parts::stylesheet(theme);
    let danger = sheet
        .lines()
        .rfind(|line| line.contains("[data-variant=\"danger\"], "))
        .expect("danger's rule");
    assert!(danger.contains("color: var(--danger-base);"), "{danger}");
    // Its words are held apart from the accent too.
    let accented = load(&with_parts(
        json!({ "button.root.danger.text": "accent.base" }),
    ));
    let fallen = parts::fallen(accented.theme.first());
    assert!(
        fallen.iter().any(|one| one.part == "button.root.danger"),
        "{fallen:?}"
    );
}

#[test]
fn the_consent_floors_hold_in_every_workspace_tint() {
    // Keep blocked's edge is the accent at the opposite hue: apart from Allow's as written, and
    // drawn alike in a workspace whose colour turns both to its own hue.
    let accent = built_in("purlis-dark")
        .unwrap()
        .value("accent.base")
        .to_owned();
    let opposite = tint_hex(&accent, (hue_of(&accent).unwrap() + 180.0) % 360.0);
    let loaded = load(&json!({
        "name": "Tinted", "extends": "purlis-classic",
        "appearances": [{ "id": "dark", "tokens": { "tab.active": opposite } }],
        "tint": { "tokens": ["accent.base", "tab.active"] },
        "parts": { "button.root.secondary.border-colour": "tab.active" }
    }));
    assert!(!loaded.refused, "{:?}", said(&loaded));
    let theme = loaded.theme.first();
    let as_written = tint::distance(
        tint::oklab_of(theme.value("accent.base")).unwrap(),
        tint::oklab_of(theme.value("tab.active")).unwrap(),
    );
    assert!(as_written >= ALLOW_APART, "{as_written}");
    let fallen = parts::fallen(theme);
    assert!(
        fallen.iter().any(|one| one.part == "button.root.secondary"
            && one.floor == "Allow is distinguishable from Keep blocked by its edge"),
        "{fallen:?}"
    );
}

#[test]
fn a_button_with_a_consent_variants_class_draws_as_that_variant() {
    let sheet = sheet_of(json!({ "button.root.primary.text": "text.secondary" }));
    assert!(
        sheet.contains(".ui-button.ui-button-primary:not(.ui-button-danger, .ui-button-grants)"),
        "{sheet}"
    );
}

#[test]
fn a_border_that_is_a_parts_only_edge_contrasts_3_to_1() {
    let loaded = load(&with_parts(json!({
        "button.root.ghost.border-colour": "surface.raised",
    })));
    let fallen = parts::fallen(loaded.theme.first());
    assert!(
        fallen.iter().any(|one| one.part == "button.root.ghost"
            && one.floor == "the root's border contrasts 3:1 with its fill"),
        "{fallen:?}"
    );
}

#[test]
fn a_disabled_control_is_not_held_to_the_floors() {
    let loaded = load(&with_parts(
        json!({ "button.root.disabled.text": "surface.base" }),
    ));
    assert!(parts::fallen(loaded.theme.first()).is_empty());
}

// ---------------------------------------------------------------------------------------
// The family default is the window's own stylesheet
// ---------------------------------------------------------------------------------------

/// The declarations of the rule `selector` opens in `css`.
fn rule<'a>(css: &'a str, selector: &str) -> &'a str {
    let at = css
        .find(&format!("\n{selector} {{"))
        .unwrap_or_else(|| panic!("controls.css has no rule {selector}"));
    let body = &css[at..];
    &body[..body.find('}').unwrap()]
}

fn declared<'a>(body: &'a str, property: &str) -> Option<&'a str> {
    body.lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix(&format!("{property}: ")))
        .map(|value| value.trim_end_matches(';'))
}

#[test]
fn the_buttons_family_default_is_what_controls_css_draws() {
    let css = include_str!("../../../../app/src/primitives/controls.css");
    let token = |var: &str| -> Value {
        if var == "transparent" {
            return Value::from(var);
        }
        let name = var
            .strip_prefix("var(--")
            .and_then(|it| it.strip_suffix(')'))
            .unwrap_or_else(|| panic!("{var} is not one token"));
        Value::from(
            vocabulary::TOKENS
                .iter()
                .find(|token| token.replace('.', "-") == name)
                .unwrap_or_else(|| panic!("--{name} is not a token"))
                .to_string(),
        )
    };
    let hover = ":hover:not(:disabled, [aria-disabled=\"true\"])";
    let base = rule(css, ".ui-button");
    let base_border = declared(base, "border")
        .and_then(|it| it.split_whitespace().last())
        .expect("the button's border");
    let base_hover = rule(css, &format!(".ui-button{hover}"));
    for (variant, class) in [
        ("secondary", None),
        ("primary", Some(".ui-button-primary")),
        ("ghost", Some(".ui-button-ghost")),
        ("danger", Some(".ui-button-danger")),
        ("grants", Some(".ui-button-grants")),
    ] {
        let own = class.map_or("", |class| rule(css, class));
        let own_hover = class
            .filter(|class| css.contains(&format!("\n{class}{hover} {{")))
            .map_or("", |class| rule(css, &format!("{class}{hover}")));
        fn first<'a>(property: &str, layers: &[&'a str]) -> &'a str {
            layers
                .iter()
                .find_map(|layer| declared(layer, property))
                .unwrap_or_else(|| panic!("no {property}"))
        }
        let drawn = json!({
            "fill": token(first("background-color", &[own, base])),
            "text": token(first("color", &[own, base])),
            "border": token(declared(own, "border-color").unwrap_or(base_border)),
            "hover": token(first("background-color", &[own_hover, base_hover])),
        });
        let held = parts::defaults_json(variant);
        for key in ["fill", "text", "border", "hover"] {
            assert_eq!(held[key], drawn[key], "{variant}'s {key}");
        }
    }
    let pressed = rule(css, ".ui-button[aria-pressed=\"true\"]");
    assert_eq!(
        parts::defaults_json("secondary")["selected"],
        json!({
            "fill": token(declared(pressed, "background-color").unwrap()),
            "border": token(declared(pressed, "border-color").unwrap()),
        })
    );
}

// ---------------------------------------------------------------------------------------
// The manifest and the schema
// ---------------------------------------------------------------------------------------

#[test]
fn the_manifest_lists_every_family_part_state_and_property_with_its_bounds() {
    let manifest = parts::manifest_json();
    assert_eq!(manifest["version"], 1);
    for list in ["states", "properties"] {
        for one in manifest[list].as_array().unwrap() {
            assert_eq!(one["since"], 1, "{one}");
        }
    }
    assert_eq!(
        manifest["grammar"],
        "<family>.<part>[.<variant>][.<state>].<property>"
    );
    let states: Vec<&str> = manifest["states"]
        .as_array()
        .unwrap()
        .iter()
        .map(|one| one["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        states,
        [
            "hover", "pressed", "selected", "focused", "disabled", "open"
        ]
    );
    let properties: Vec<(&str, bool)> = manifest["properties"]
        .as_array()
        .unwrap()
        .iter()
        .map(|one| {
            (
                one["name"].as_str().unwrap(),
                one["in_state"].as_bool().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        properties,
        [
            ("fill", true),
            ("text", true),
            ("border-colour", true),
            ("shadow", true),
            ("border-width", false),
            ("corner-radius", false),
            ("padding", false),
            ("gap", false),
            ("font-size", false),
            ("font-weight", false),
            ("height", false),
        ]
    );
    let height = &manifest["properties"][10]["bounds"];
    assert_eq!(
        (height["least"].as_i64(), height["most"].as_i64()),
        (Some(24), Some(48))
    );
    let button = &manifest["families"][0];
    assert_eq!(button["name"], "button");
    assert_eq!(button["since"], 1);
    let parts: Vec<&str> = button["parts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|one| one["name"].as_str().unwrap())
        .collect();
    assert_eq!(parts, ["root", "label", "icon"]);
    assert!(
        button["variants"]
            .as_array()
            .unwrap()
            .iter()
            .any(|one| one["name"] == "grants" && one["consent"] == true)
    );
}

#[test]
fn the_schema_completes_every_part_key_and_no_size_inside_a_state() {
    let schema = schema();
    let keys = schema["properties"]["parts"]["properties"]
        .as_object()
        .expect("the part keys");
    assert_eq!(schema["properties"]["parts"]["additionalProperties"], false);
    for key in [
        "button.root.corner-radius",
        "button.root.primary.hover.fill",
        "button.label.font-weight",
        "button.icon.grants.focused.text",
    ] {
        assert!(keys.contains_key(key), "{key}");
    }
    for key in [
        "button.root.hover.height",
        "button.root.selected.font-weight",
        "button.icon.fill",
    ] {
        assert!(!keys.contains_key(key), "{key}");
    }
    // Every key it completes is one the engine reads, with a value the engine accepts.
    for key in keys.keys() {
        let value =
            if key.ends_with("fill") || key.ends_with("border-colour") || key.ends_with(".text") {
                json!("text.secondary")
            } else if key.ends_with("shadow") {
                json!("none")
            } else if key.ends_with("font-weight") {
                json!(500)
            } else if key.ends_with("font-size") {
                json!(12)
            } else if key.ends_with("height") {
                json!(28)
            } else {
                json!(2)
            };
        let loaded = load(&with_parts(json!({ key.clone(): value })));
        assert!(
            !said(&loaded)
                .iter()
                .any(|one| one.contains("is not a component part")),
            "{key}"
        );
    }
}

/// The digest of `purlis theme parts`' manifest at each version: a change to the manifest is a
/// new version, with its digest added here.
const MANIFEST_DIGESTS: &[(u32, &str)] = &[(
    1,
    "3c186a1690df39a4e8c20ad43f3713cb7e0149ab681e00970c0c21ac4daa8769",
)];

#[test]
fn the_manifest_version_changes_with_the_manifest() {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(parts::manifest_json().to_string().as_bytes());
    let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    let pinned = MANIFEST_DIGESTS
        .iter()
        .find(|(version, _)| *version == parts::VERSION)
        .map(|(_, digest)| *digest);
    assert_eq!(
        pinned,
        Some(hex.as_str()),
        "the manifest changed: raise parts::VERSION, give what is new `since` that version, and \
         add the new digest to MANIFEST_DIGESTS"
    );
}

#[test]
fn a_css_spelling_of_a_property_is_told_its_name() {
    let loaded = load(&with_parts(
        json!({ "button.root.border-color": "accent.base" }),
    ));
    assert!(
        said(&loaded)[0].ends_with("so it was left out; did you mean `border-colour`?"),
        "{:?}",
        said(&loaded)
    );
}

/// A theme over Purlis Classic whose dark appearance sets `tokens` and that sets `parts`.
fn with_tokens_and_parts(tokens: Value, parts: Value) -> Value {
    json!({
        "name": "Shaped", "extends": "purlis-classic",
        "appearances": [{ "id": "dark", "tokens": tokens }],
        "parts": parts
    })
}

fn fallen_parts(document: &Value) -> Vec<String> {
    let loaded = load(document);
    assert!(!loaded.refused, "{:?}", said(&loaded));
    parts::fallen(loaded.theme.first())
        .into_iter()
        .map(|one| one.part)
        .collect()
}

#[test]
fn allow_is_drawn_on_keep_blockeds_fill() {
    for near in ["#1d3630", "#1f3832"] {
        let fallen = fallen_parts(&with_tokens_and_parts(
            json!({ "chart.2": near }),
            json!({ "button.root.grants.fill": "chart.2" }),
        ));
        assert!(
            fallen.contains(&"button.root.grants".to_owned()),
            "{near}: {fallen:?}"
        );
    }
}

#[test]
fn the_primary_is_never_drawn_nearer_allow_than_the_family_draws_it() {
    for near in ["#2c2c29", "#2e2e2b"] {
        let fallen = fallen_parts(&with_tokens_and_parts(
            json!({ "chart.4": near }),
            json!({
                "button.root.primary.fill": "chart.4",
                "button.root.primary.border-colour": "focus.ring",
                "button.root.primary.hover.fill": "control.hover",
            }),
        ));
        assert!(
            fallen.contains(&"button.root.primary".to_owned()),
            "{near}: {fallen:?}"
        );
    }
}

#[test]
fn dangers_words_are_apart_from_the_primarys() {
    for near in ["#e4e3dd", "#e7e6e0"] {
        let fallen = fallen_parts(&with_tokens_and_parts(
            json!({ "chart.3": near }),
            json!({
                "button.root.danger.fill": "accent.surface",
                "button.root.danger.border-colour": "accent.base",
                "button.root.danger.text": "chart.3",
            }),
        ));
        assert!(
            fallen.contains(&"button.root.danger".to_owned()),
            "{near}: {fallen:?}"
        );
    }
}

#[test]
fn a_border_width_never_reaches_todays_small_buttons() {
    let sheet = sheet_of(json!({ "button.root.border-width": 2 }));
    let line = sheet
        .lines()
        .find(|line| line.contains("border-width: 2px"))
        .expect("the width's rule");
    assert!(
        line.contains(":not(.ui-button-sm, .ui-button-row > .ui-button)"),
        "{line}"
    );
}

#[test]
fn the_primarys_look_is_its_fill_and_an_edge_that_stands_out() {
    for near in ["#5ed0b2", "#63d5b7"] {
        let fallen = fallen_parts(&with_tokens_and_parts(
            json!({ "chart.5": near }),
            json!({
                "button.root.primary.fill": "control.base",
                "button.root.primary.border-colour": "chart.5",
                "button.root.primary.hover.fill": "control.hover",
            }),
        ));
        assert!(
            fallen.contains(&"button.root.primary".to_owned()),
            "{near}: {fallen:?}"
        );
    }
}

#[test]
fn a_see_through_fill_is_measured_as_it_shows_on_each_ground() {
    for tokens in [json!({}), json!({ "surface.raised": "#262623" })] {
        let fallen = fallen_parts(&with_tokens_and_parts(
            tokens.clone(),
            json!({
                "button.root.primary.fill": "transparent",
                "button.root.primary.border-colour": "focus.ring",
                "button.root.primary.hover.fill": "control.hover",
            }),
        ));
        assert!(
            fallen.contains(&"button.root.primary".to_owned()),
            "{tokens}: {fallen:?}"
        );
    }
}

// ---------------------------------------------------------------------------------------
// What an apply costs
// ---------------------------------------------------------------------------------------

/// A parts theme with states, over a theme a workspace's colour tints, named apart by `radius`
/// so no other test has drawn it.
fn costed(radius: u32) -> ResolvedTheme {
    let loaded = load(&with_parts(json!({
        "button.root.corner-radius": radius,
        "button.root.primary.hover.fill": "tab.active",
        "button.root.ghost.text": "text.secondary",
        "button.root.grants.focused.border-colour": "accent.base",
    })));
    assert!(!loaded.refused, "{:?}", said(&loaded));
    loaded.theme.first().clone()
}

#[test]
fn a_parts_theme_is_measured_once_for_its_stylesheet_and_what_fell() {
    let theme = costed(40);
    let before = parts::passes();
    let sheet = parts::stylesheet(&theme);
    let fallen = parts::fallen(&theme);
    assert_eq!(parts::passes() - before, 1);
    // An equal theme is the same theme; the measure is the one a fresh pass gives.
    let again = theme.clone();
    assert_eq!(parts::stylesheet(&again), sheet);
    assert_eq!(parts::fallen(&again), fallen);
    assert_eq!(parts::passes() - before, 1);
}

#[test]
fn a_gallery_of_parts_themes_is_measured_once_per_theme_however_often_it_is_drawn() {
    let themes: Vec<ResolvedTheme> = (60..68).map(costed).collect();
    let before = parts::passes();
    for _ in 0..3 {
        for theme in &themes {
            parts::stylesheet(theme);
            parts::fallen(theme);
        }
    }
    assert_eq!(parts::passes() - before, themes.len());
}

#[test]
fn a_theme_whose_values_differ_is_measured_again() {
    let theme = costed(41);
    let mut tinted = theme.clone();
    tinted.values.insert("accent.base", "#c97474".to_owned());
    let before = parts::passes();
    parts::fallen(&theme);
    parts::fallen(&tinted);
    assert_eq!(parts::passes() - before, 2);
}

#[test]
#[ignore = "a bench: `cargo test --release -p purlis-core --lib parts_apply_bench -- --ignored --nocapture`"]
fn parts_apply_bench() {
    const RUNS: u32 = 20;
    let themes: Vec<ResolvedTheme> = (0..RUNS).map(|at| costed(100 + at)).collect();
    let started = std::time::Instant::now();
    let drawn: Vec<(String, Vec<parts::Fell>)> = themes
        .iter()
        .map(|theme| (parts::stylesheet(theme), parts::fallen(theme)))
        .collect();
    let each = started.elapsed() / RUNS;
    // What was drawn, folded to one number, so two builds can be told to draw the same.
    let digest = format!("{drawn:?}")
        .bytes()
        .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
        });
    println!(
        "parts apply (stylesheet and fallen of one theme): {each:?} per theme, drawn {digest:016x}"
    );
}

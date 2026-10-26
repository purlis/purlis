// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Component parts at the engine's interface (#2903): the manifest, the key grammar, the bounds
//! and the corner clamp, the state-size rule, the fallback to the family default, the floors per
//! part and state with the consent parts, and the stylesheet the window draws.

use serde_json::{Value, json};

use super::parts::{self, Prop};
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

/// The stylesheet of a theme's first appearance, which must be drawn, less the rules of Purlis
/// Classic's own parts: what the theme's parts add over its parent's.
fn sheet_of(parts: Value) -> String {
    let loaded = load(&with_parts(parts));
    assert!(!loaded.refused, "{:?}", said(&loaded));
    over_classic(&parts::stylesheet(loaded.theme.first()))
}

/// A stylesheet less the rules Purlis Classic's parts write, which are its tabs' strip look
/// (#2914) and its activity items' look (#2917): the family defaults draw Purlis, and Classic
/// keeps its own looks through parts. Empty when nothing else is written.
fn over_classic(sheet: &str) -> String {
    let classic = parts::stylesheet(classic().first());
    let own: Vec<&str> = sheet
        .lines()
        .filter(|line| {
            !((line.starts_with("  :where(") || line.contains("[data-scope=\"tabs\"]"))
                && classic.lines().any(|one| one == *line))
        })
        .collect();
    if own.len() <= 2 {
        return String::new();
    }
    own.join("\n") + "\n"
}

/// Whether a key is one of Purlis Classic's own parts.
fn classics(key: &parts::Key) -> bool {
    classic().first().parts.contains_key(key)
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
        concat!(
            "@layer parts {\n",
            "  :where([data-scope=\"button\"][data-part=\"root\"][data-size=\"sm\"]) { --purlis-part-height: max(24px, var(--size-chrome), var(--space-7)); }\n",
            "  :where([data-scope=\"button\"][data-part=\"root\"][data-size=\"lg\"]) { --purlis-part-height: calc(var(--size-control) + var(--space-2)); }\n",
            "  :where([data-scope=\"button\"][data-part=\"root\"]) { border-radius: 9999px; }\n}\n",
        )
    );
}

#[test]
fn a_theme_that_names_no_parts_draws_exactly_as_today() {
    // Purlis Classic names only its tabs' strip look (#2914), its activity items' (#2917), and
    // what keeps it as it drew when the family defaults took Purlis's (#3177): its rail's 9px
    // corners, its title bar's 5px chips and its update pill's primary words. Nothing it draws
    // falls back.
    for theme in built_ins() {
        assert!(
            theme.parts.keys().all(
                |key| ["tabs", "activity", "rail", "titlebar", "status-item"].contains(&key.family)
            ),
            "{} names parts beyond what keeps it as it drew",
            theme.name
        );
        assert!(parts::fallen(theme).is_empty());
    }
    // Purlis names its panels' fill (#2913), the one its docked tabs join by default (#2914),
    // and the edges it draws none of (#3177): its tooltip's, dialog's and popover's. Its tabs and
    // its buttons are the family defaults and cost nothing to measure: a button part would have
    // its consent floors measured at every hue a workspace's colour can take, at every launch.
    let purlis = built_in("purlis").expect("Purlis");
    assert!(
        purlis
            .parts
            .keys()
            .all(|key| ["panel", "tooltip", "dialog", "popover"].contains(&key.family)),
        "{:?}",
        purlis.parts.keys().collect::<Vec<_>>()
    );
    assert!(parts::fallen(purlis).is_empty());
    let loaded = load(&json!({ "name": "Plain", "extends": "purlis-classic" }));
    assert_eq!(
        parts::stylesheet(loaded.theme.first()),
        parts::stylesheet(classic().first())
    );
    let empty = load(&with_parts(json!({})));
    assert!(!empty.refused);
    assert_eq!(over_classic(&parts::stylesheet(empty.theme.first())), "");
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
    // Only the wrapper's attributes: no class reaches a part (#2904).
    assert!(!sheet.contains(".ui-button"), "{sheet}");
    assert!(sheet.contains("background-color: var(--accent-base);"));
}

#[test]
fn an_unavailable_button_is_never_drawn_under_the_pointer_or_pressed() {
    // The wrapper marks a button that keeps the focus while unavailable (`aria-disabled`) as
    // `data-disabled` too, so a theme's disabled parts reach it and its hover and pressed never
    // do (#2904).
    let sheet = sheet_of(json!({
        "button.root.hover.fill": "control.hover",
        "button.root.pressed.fill": "control.hover",
        "button.root.disabled.text": "text.faint",
    }));
    assert!(
        sheet.contains("[data-hovered]:not([data-disabled])"),
        "{sheet}"
    );
    assert!(
        sheet.contains("[data-pressed]:not([data-disabled])"),
        "{sheet}"
    );
    assert!(
        sheet.contains("[data-scope=\"button\"][data-part=\"root\"][data-disabled]"),
        "{sheet}"
    );
}

#[test]
fn the_plain_variant_is_reached_by_its_own_attribute() {
    let sheet = sheet_of(json!({ "button.root.secondary.fill": "control.hover" }));
    assert!(sheet.contains(
        ":where([data-scope=\"button\"][data-part=\"root\"][data-variant=\"secondary\"])"
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
    assert_eq!(
        loaded
            .theme
            .first()
            .parts
            .keys()
            .filter(|key| !classics(key))
            .count(),
        1
    );
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
fn a_size_never_reaches_another_size_or_a_wrapping_row() {
    let sheet = sheet_of(json!({
        "button.root.height": 32,
        "button.root.fill": "control.hover",
    }));
    let sized = sheet
        .lines()
        .find(|line| line.contains("block-size"))
        .expect("the height's rule");
    assert!(
        sized.contains("[data-scope=\"button\"][data-part=\"root\"]:not([data-size=\"sm\"], [data-size=\"lg\"], [data-scope=\"button-row\"][data-wraps] > *)"),
        "{sized}"
    );
    assert!(!sized.contains("background-color"), "{sized}");
    let coloured = sheet
        .lines()
        .find(|line| line.contains("background-color: var(--control-hover)"))
        .expect("the fill's rule");
    assert!(!coloured.contains(":not([data-size"), "{coloured}");
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
    // The small and the large button are clamped to their own height.
    assert!(sheet.contains(":where([data-scope=\"button\"][data-part=\"root\"][data-size=\"sm\"]) { --purlis-part-height: max(24px, var(--size-chrome), var(--space-7)); }"));
    assert!(sheet.contains(":where([data-scope=\"button\"][data-part=\"root\"][data-size=\"lg\"]) { --purlis-part-height: calc(var(--size-control) + var(--space-2)); }"));
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
    let root = parts::family("button").unwrap().parts[0].properties;
    for property in root.iter().filter(|one| !one.in_state()) {
        for state in parts::family("button").unwrap().states {
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
          reaches it; inside a state only fill, text, border-colour and shadow may be set, and \
          a tab's flare and hover inset, which are drawn out of its flow and size nothing. Give \
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
    // The layer, the sm and lg clamps a corner reads, the one rule, and the layer's end.
    assert_eq!(sheet.lines().count(), 5, "{sheet}");
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
        .rfind(|line| line.contains("[data-variant=\"grants\"])"))
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
            .rfind(|line| line.contains("[data-variant=\"secondary\"])"))
            .expect("Keep blocked's rule");
        assert!(
            keep_blocked.contains("border-color: var(--border-control);"),
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
        .rfind(|line| {
            line.contains("[data-variant=\"grants\"]:not(") && line.contains("border-width")
        })
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
        .rfind(|line| line.contains("[data-variant=\"danger\"])"))
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
    let accent = classic().first().value("accent.base").to_owned();
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
fn a_variants_rule_reaches_only_a_button_of_that_variant() {
    // A button names one variant (`data-variant`), so a rule for the primary never reaches an
    // Allow or a danger (ADR 0097 §6).
    let sheet = sheet_of(json!({ "button.root.primary.text": "text.secondary" }));
    assert!(
        sheet.contains(
            ":where([data-scope=\"button\"][data-part=\"root\"][data-variant=\"primary\"])"
        ),
        "{sheet}"
    );
    assert!(!sheet.contains(".ui-button"), "{sheet}");
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
        .unwrap_or_else(|| panic!("button.css has no rule {selector}"));
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
fn the_buttons_family_default_is_what_button_css_draws() {
    // A selector prettier wraps (a long variant's `:not(…)`) read back onto one line.
    let css = &include_str!("../../../../app/src/ui/button/button.css")
        .replace("(\n    ", "(")
        .replace("\n  ) {", ") {");
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
    let root = "[data-scope=\"button\"][data-part=\"root\"]";
    let base = rule(css, root);
    let base_border = declared(base, "border")
        .and_then(|it| it.split_whitespace().last())
        .expect("the button's border");
    let base_hover = rule(css, &format!("{root}[data-hovered]:not([data-disabled])"));
    for variant in parts::MANIFEST[0].variants.iter().map(|one| one.name) {
        let own_selector = format!("{root}[data-variant=\"{variant}\"]");
        let own = if css.contains(&format!("\n{own_selector} {{")) {
            rule(css, &own_selector)
        } else {
            ""
        };
        let hovered = format!("{own_selector}[data-hovered]:not([data-disabled])");
        let own_hover = if css.contains(&format!("\n{hovered} {{")) {
            rule(css, &hovered)
        } else {
            ""
        };
        fn first<'a>(property: &str, layers: &[&'a str]) -> &'a str {
            layers
                .iter()
                .find_map(|layer| declared(layer, property))
                .unwrap_or_else(|| panic!("no {property}"))
        }
        // A variant's own rule comes after the base's hover in the file, at the same weight,
        // so a fill it sets is its fill under the pointer too unless it has a hover of its own.
        let drawn = json!({
            "fill": token(first("background-color", &[own, base])),
            "text": token(first("color", &[own, base])),
            "border": token(declared(own, "border-color").unwrap_or(base_border)),
            "hover": token(first("background-color", &[own_hover, own, base_hover])),
        });
        let held = parts::defaults_json(variant);
        for key in ["fill", "text", "border", "hover"] {
            assert_eq!(held[key], drawn[key], "{variant}'s {key}");
        }
    }
    let pressed = rule(css, &format!("{root}[data-selected]"));
    assert_eq!(
        parts::defaults_json("secondary")["selected"],
        json!({
            "fill": token(declared(pressed, "background-color").unwrap()),
            "border": token(declared(pressed, "border-color").unwrap()),
        })
    );
    // A toggle that is on, under the pointer: its own rule's fill, over every variant's.
    let pressed_hover = rule(
        css,
        &format!("{root}[data-selected][data-hovered]:not([data-disabled])"),
    );
    for variant in ["secondary", "primary", "ghost", "danger", "grants"] {
        assert_eq!(
            parts::defaults_json(variant)["selected_hover"],
            json!({
                "fill": token(declared(pressed_hover, "background-color").unwrap()),
                "border": token(declared(pressed, "border-color").unwrap()),
            }),
            "{variant}"
        );
    }
}

#[test]
fn the_tooltips_family_default_is_what_tooltip_css_draws() {
    let css = include_str!("../../../../app/src/ui/tooltip/tooltip.css");
    let root = rule(css, "[data-scope=\"tooltip\"][data-part=\"root\"]");
    let token = |var: &str| {
        var.trim_start_matches("var(--")
            .trim_end_matches(')')
            .replace('-', ".")
    };
    let border = declared(root, "border")
        .and_then(|it| it.split_whitespace().last())
        .expect("the tooltip's border");
    let fill = token(declared(root, "background-color").expect("its fill"));
    // Nothing on it is pressed: under the pointer it keeps its fill.
    let drawn = json!({
        "fill": fill,
        "text": token(declared(root, "color").expect("its words")),
        "border": token(border),
        "hover": fill,
    });
    assert_eq!(parts::look_json("tooltip", ""), drawn);
}

#[test]
fn a_theme_rounds_the_tooltip_and_turns_its_edge_off() {
    let sheet = sheet_of(json!({
        "tooltip.root.corner-radius": 8,
        "tooltip.root.border-width": 0,
    }));
    assert!(
        sheet.contains("[data-scope=\"tooltip\"][data-part=\"root\"]"),
        "{sheet}"
    );
    assert!(sheet.contains("border-width: 0px"), "{sheet}");
    // Its size is its words': a theme sets none of it, and a key that tries is said and left out.
    let loaded = load(&with_parts(json!({ "tooltip.root.height": 30 })));
    assert!(
        said(&loaded)
            .iter()
            .any(|one| one.contains("parts.tooltip.root.height is not a component part")),
        "{:?}",
        said(&loaded)
    );
    assert!(sheet_of(json!({ "tooltip.root.height": 30 })).is_empty());
}

/// A stylesheet's `var(--x-y)` as the token `x.y`, or `transparent`.
fn token_of(var: &str) -> String {
    if var == "transparent" {
        return var.to_owned();
    }
    let name = var
        .strip_prefix("var(--")
        .and_then(|it| it.strip_suffix(')'))
        .unwrap_or_else(|| panic!("{var} is not one token"));
    vocabulary::TOKENS
        .iter()
        .find(|token| token.replace('.', "-") == name)
        .unwrap_or_else(|| panic!("--{name} is not a token"))
        .to_string()
}

#[test]
fn the_popover_and_menu_family_defaults_are_what_their_stylesheets_draw() {
    use parts::State::{Disabled, Focused, Hover, Open};
    let popover = include_str!("../../../../app/src/ui/popover/popover.css");
    let surface = rule(popover, "[data-scope=\"popover\"][data-part=\"root\"]");
    let border = declared(surface, "border")
        .and_then(|it| it.split_whitespace().last())
        .expect("the surface's border");
    for variant in ["popover", "menu"] {
        for state in [&[][..], &[Hover][..]] {
            assert_eq!(
                parts::look_of("popover", variant, state).map(str::to_owned),
                [
                    token_of(declared(surface, "background-color").unwrap()),
                    token_of(declared(surface, "color").unwrap()),
                    token_of(border),
                ],
                "the popover's {variant} surface"
            );
        }
    }

    let menu = &include_str!("../../../../app/src/ui/menu/menu.css")
        .replace("(\n    ", "(")
        .replace("\n  ) {", ") {");
    let row = "[data-scope=\"menu\"][data-part=\"root\"]";
    let at_rest = rule(menu, row);
    assert_eq!(
        declared(at_rest, "background-color"),
        None,
        "a row at rest has no ground"
    );
    assert_eq!(
        parts::look_of("menu", "default", &[]),
        ["transparent", "text.primary", "transparent"]
    );
    assert_eq!(
        token_of(declared(at_rest, "color").unwrap()),
        "text.primary"
    );
    let highlight =
        token_of(declared(rule(menu, &format!("{row}[data-open]")), "background-color").unwrap());
    for state in [Hover, Focused, Open] {
        assert_eq!(
            parts::look_of("menu", "default", &[state])[0],
            highlight,
            "{state:?}"
        );
    }
    let danger = format!("{row}[data-variant=\"danger\"]");
    assert_eq!(
        parts::look_of("menu", "danger", &[])[1],
        token_of(declared(rule(menu, &danger), "color").unwrap())
    );
    let danger_hover = token_of(
        declared(
            rule(
                menu,
                &format!("{danger}[data-hovered]:not([data-disabled])"),
            ),
            "background-color",
        )
        .unwrap(),
    );
    for state in [Hover, Focused] {
        assert_eq!(
            parts::look_of("menu", "danger", &[state])[0],
            danger_hover,
            "{state:?}"
        );
    }
    assert_eq!(
        parts::look_of("menu", "default", &[Disabled])[1],
        token_of(declared(rule(menu, &format!("{row}[data-unavailable]")), "color").unwrap())
    );
}

#[test]
fn the_list_family_default_is_what_its_stylesheet_draws() {
    use parts::State::{Disabled, Focused, Hover, Selected};
    let css = include_str!("../../../../app/src/ui/list/list.css");
    let row = "[data-scope=\"list\"][data-part=\"root\"]";
    let at_rest = rule(css, row);
    assert_eq!(
        declared(at_rest, "background-color"),
        None,
        "a row at rest has no ground"
    );
    assert_eq!(
        parts::look_of("list", "", &[]),
        [
            "transparent",
            token_of(declared(at_rest, "color").unwrap()).as_str(),
            "transparent"
        ]
    );
    // The pointer's pill is drawn on a row that is not chosen: a chosen row keeps its wash.
    let hover = rule(
        css,
        &format!("{row}[data-hovered]:not([data-disabled]):not([data-selected])"),
    );
    assert_eq!(
        parts::look_of("list", "", &[Hover])[0],
        token_of(declared(hover, "background-color").unwrap())
    );
    // The keyboard's ring changes no colour of the row.
    assert_eq!(
        parts::look_of("list", "", &[Focused]),
        parts::look_of("list", "", &[])
    );
    let chosen = rule(css, &format!("{row}[data-selected]"));
    for states in [&[Selected][..], &[Selected, Hover][..]] {
        let look = parts::look_of("list", "", states);
        assert_eq!(
            look[0],
            token_of(declared(chosen, "background-color").unwrap()),
            "{states:?}"
        );
        assert_eq!(
            look[1],
            token_of(declared(chosen, "color").unwrap()),
            "{states:?}"
        );
    }
    let off = rule(css, &format!("{row}[data-unavailable]"));
    assert_eq!(
        parts::look_of("list", "", &[Disabled])[1],
        token_of(declared(off, "color").unwrap())
    );
}

// ---------------------------------------------------------------------------------------
// The manifest and the schema
// ---------------------------------------------------------------------------------------

#[test]
fn the_manifest_lists_every_family_part_state_and_property_with_its_bounds() {
    let manifest = parts::manifest_json();
    assert_eq!(manifest["version"], parts::VERSION);
    // A refused box (#3110) is the one state added since the first version.
    for one in manifest["states"].as_array().unwrap() {
        let since = if one["name"] == "invalid" {
            parts::NEXT
        } else {
            1
        };
        assert_eq!(one["since"], since, "{one}");
    }
    // The tab family's layout options came in version 8 (#2914).
    for one in manifest["properties"].as_array().unwrap() {
        let since = if one["name"] == "max-width"
            || one["name"] == "flare"
            || one["name"] == "inset"
            || one["name"] == "attach"
            || one["name"] == "sizing"
            || one["name"] == "dividers"
            || one["name"] == "position"
        {
            8
        } else {
            1
        };
        assert_eq!(one["since"], since, "{one}");
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
            "hover", "pressed", "selected", "focused", "disabled", "open", "invalid"
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
            ("max-width", false),
            ("flare", true),
            ("inset", true),
            ("attach", false),
            ("sizing", false),
            ("dividers", false),
            ("position", false),
        ]
    );
    let height = &manifest["properties"][10]["bounds"];
    assert_eq!(
        (height["least"].as_i64(), height["most"].as_i64()),
        (Some(24), Some(48))
    );
    let dialog = manifest["families"]
        .as_array()
        .unwrap()
        .iter()
        .find(|one| one["name"] == "dialog")
        .expect("the dialog family");
    assert_eq!(dialog["since"], 7);
    let parts: Vec<&str> = dialog["parts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|one| one["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        parts,
        [
            "root",
            "title",
            "subtitle",
            "description",
            "separator",
            "summary"
        ]
    );
    let variants: Vec<&str> = dialog["variants"]
        .as_array()
        .unwrap()
        .iter()
        .map(|one| one["name"].as_str().unwrap())
        .collect();
    assert_eq!(variants, ["dialog", "alert"]);
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
    assert_eq!(button["focused"], "[data-focus-visible]");
    // The field families (#2905): a field and the two boxes it holds, none with a variant.
    let family = |name: &str| {
        manifest["families"]
            .as_array()
            .unwrap()
            .iter()
            .find(|one| one["name"] == name)
            .unwrap_or_else(|| panic!("no {name}"))
            .clone()
    };
    for (name, wanted) in [
        ("field", &["root", "label", "help", "error"][..]),
        ("text-input", &["root", "icon"][..]),
        ("textarea", &["root"][..]),
    ] {
        let one = family(name);
        assert_eq!(one["since"], 3, "{name}");
        assert_eq!(one["variants"], json!([]), "{name}");
        assert_eq!(one["focused"], "[data-focus-within]", "{name}");
        let states: Vec<&str> = one["states"]
            .as_array()
            .unwrap()
            .iter()
            .map(|state| state.as_str().unwrap())
            .collect();
        let wanted_states: &[&str] = if name == "field" {
            &["disabled"]
        } else {
            &["hover", "focused", "disabled"]
        };
        assert_eq!(states, wanted_states, "{name}");
        let parts: Vec<&str> = one["parts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|part| part["name"].as_str().unwrap())
            .collect();
        assert_eq!(parts, wanted, "{name}");
    }
    // The popover and menu families (#2909): the surface and its anatomy, and a menu's row.
    let names = |one: &Value, list: &str| -> Vec<String> {
        one[list]
            .as_array()
            .unwrap()
            .iter()
            .map(|it| it["name"].as_str().unwrap().to_owned())
            .collect()
    };
    let popover = family("popover");
    assert_eq!(popover["since"], 6);
    assert_eq!(
        names(&popover, "parts"),
        ["root", "filter", "rule", "list", "header", "footer", "key"]
    );
    assert_eq!(names(&popover, "variants"), ["popover", "menu"]);
    let menu = family("menu");
    assert_eq!(menu["since"], 6);
    assert_eq!(
        names(&menu, "parts"),
        ["root", "label", "description", "shortcut", "indicator"]
    );
    assert!(
        menu["variants"]
            .as_array()
            .unwrap()
            .iter()
            .any(|one| one["name"] == "danger" && one["consent"] == true)
    );
    // The list family (#2911): a row, the list around it, its slots and a group's name.
    let list = family("list");
    assert_eq!(list["since"], 11);
    assert_eq!(list["variants"], json!([]));
    assert_eq!(
        names(&list, "parts"),
        [
            "root",
            "body",
            "lead",
            "label",
            "description",
            "trail",
            "header"
        ]
    );
    // A textarea is as tall as its lines: no height to set.
    assert!(
        !family("textarea")["parts"][0]["properties"]
            .as_array()
            .unwrap()
            .contains(&json!("height"))
    );
    // The tooltip (#2908): one look, its surface and an icon button's second line.
    let tooltip = family("tooltip");
    assert_eq!(tooltip["since"], 5);
    assert_eq!(tooltip["variants"], json!([]));
    let parts: Vec<&str> = tooltip["parts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|one| one["name"].as_str().unwrap())
        .collect();
    assert_eq!(parts, ["root", "detail"]);
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

/// **The digest of the contract each release shipped**: [`contract`] of
/// [`parts::manifest_as_of`] that version, so what a released theme was written against never
/// changes, while its prose may be reworded. Only a release adds a line here, and the release step
/// writes it (`node tools/theme-parts-release.mjs`); a pull request that adds to the manifest
/// gives what it adds `since: NEXT` and leaves this alone. Versions 1 to 11 were pinned over the
/// whole text, descriptions and all, before the contract was the thing pinned; 12 is pinned again
/// over its contract (D-manifest-6), and only the last release's line is checked.
const MANIFEST_DIGESTS: &[(u32, &str)] = &[
    (
        1,
        "3c186a1690df39a4e8c20ad43f3713cb7e0149ab681e00970c0c21ac4daa8769",
    ),
    (
        2,
        "0d939a9f8ad769584b644227563214dcf5e8dfbeca21e3c5a27ba545bba1d46d",
    ),
    (
        3,
        "a59f1d43e8ea81fd5d4379743664d436979ec1cf53324f1346c2712b805dbe55",
    ),
    (
        4,
        "dc6e2997214e822fdd97166f037b2d83091813f11701ce0bbfe06fa1403041f3",
    ),
    (
        5,
        "815d4a7cc64ef67b80a8a2b88423945870fc6a46d72355c43a38ac385f5713de",
    ),
    (
        6,
        "51e9dfc27ab31296830631003a7071db655e6aa69b7142c99fbce61550b3482c",
    ),
    (
        7,
        "67bde082299e45b7255ba4c5b05c0463e95c0262fa4430653b47b13d3b930d06",
    ),
    (
        8,
        "3a5db3c2ce7fc328da8f0c6a22a4af730aec9e51b45d02141051884af667ecc0",
    ),
    (
        9,
        "f9ce10aed0e38be894b68e8d340ec365a805608c9914086c62cc7958926db105",
    ),
    (
        10,
        "c939e7d602f3e4d800d4bc7ae3ce8a55abe8d0724287f76ecc058ec2a40c9efe",
    ),
    (
        11,
        "be280ba42efe49bf56ddae1aa28df6e1ee50ce46e77f4a3458af198b01077c39",
    ),
    (
        12,
        "8a4b6d0a12377b03f8da2ff51f063c3066a96f6c2083c61fd9247b5359a845f6",
    ),
];

/// The prose of the manifest, which a theme is never written against: every `description`, and
/// the sentences that say the grammar's rest and why a state keeps a size. [`contract`] drops a
/// key of one of these names **at every depth**, so a contract key ever named `description`,
/// `rest` or `no_size_in_a_state` (a map keyed by a state called "rest", say) would be dropped
/// unseen: name such a key otherwise, or narrow this filter to the paths it means.
const PROSE: [&str; 3] = ["description", "rest", "no_size_in_a_state"];

/// **The manifest's contract**: what a theme is written against (the families, parts, variants,
/// states and properties, their bounds, attributes and `since`), with its prose left out, so a
/// reworded description changes no pin and any other change to a released item does.
fn contract(manifest: &Value) -> Value {
    match manifest {
        Value::Object(map) => Value::Object(
            map.iter()
                .filter(|(key, _)| !PROSE.contains(&key.as_str()))
                .map(|(key, value)| (key.clone(), contract(value)))
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(contract).collect()),
        other => other.clone(),
    }
}

fn digest_of(manifest: &Value) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(contract(manifest).to_string().as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[test]
fn a_reworded_description_keeps_the_pin_and_a_changed_bound_breaks_it() {
    let released = parts::manifest_as_of(parts::VERSION);
    let pinned = digest_of(&released);

    let mut reworded = released.clone();
    reworded["families"][0]["description"] = json!("The same button, said another way.");
    reworded["families"][0]["parts"][0]["description"] = json!("Its root, reworded.");
    reworded["properties"][0]["description"] = json!("A fill, reworded.");
    reworded["states"][0]["description"] = json!("Hovered, reworded.");
    reworded["no_size_in_a_state"] = json!("Reworded.");
    assert_ne!(reworded, released);
    assert_eq!(digest_of(&reworded), pinned);

    let corner = released["properties"]
        .as_array()
        .unwrap()
        .iter()
        .position(|one| one["name"] == "corner-radius")
        .expect("corner-radius");
    let mut bound = released.clone();
    bound["properties"][corner]["bounds"]["or"] = json!([]);
    assert_ne!(digest_of(&bound), pinned, "a bound is the contract");

    for (path, value) in [
        ("/families/0/since", json!(99)),
        ("/families/0/parts/0/name", json!("renamed")),
        ("/families/0/variants/0/consent", json!(true)),
        ("/properties/0/in_state", json!(false)),
        ("/states/0/attribute", json!("[data-other]")),
        ("/grammar", json!("<family>.<property>")),
    ] {
        let mut changed = released.clone();
        *changed.pointer_mut(path).expect(path) = value;
        assert_ne!(digest_of(&changed), pinned, "{path} is the contract");
    }
}

/// The pin the release step writes: `PURLIS_THEME_PARTS_PIN=1` adds the released version's digest
/// to [`MANIFEST_DIGESTS`] when it has none.
fn pin(version: u32, hex: &str) {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/src/theme/parts_tests.rs");
    let text = std::fs::read_to_string(path).expect("parts_tests.rs is read");
    let start = text
        .find("const MANIFEST_DIGESTS: &[(u32, &str)] = &[")
        .expect("the digest table");
    let end = start + text[start..].find("\n];\n").expect("the table's end");
    let line = format!("\n    (\n        {version},\n        \"{hex}\",\n    ),");
    let written = format!("{}{line}{}", &text[..end], &text[end..]);
    std::fs::write(path, written).expect("parts_tests.rs is written");
}

#[test]
fn every_released_manifest_keeps_its_digest() {
    let hex = digest_of(&parts::manifest_as_of(parts::VERSION));
    let pinned = MANIFEST_DIGESTS
        .iter()
        .find(|(version, _)| *version == parts::VERSION)
        .map(|(_, digest)| *digest);
    if pinned.is_none() && std::env::var_os("PURLIS_THEME_PARTS_PIN").is_some_and(|v| v == "1") {
        pin(parts::VERSION, &hex);
        return;
    }
    assert_eq!(
        pinned,
        Some(hex.as_str()),
        "the manifest version {} shipped changed. A pull request never changes what a release \
         shipped: give what it adds `since: NEXT` (`super::NEXT` in a family's own file) and leave \
         parts::VERSION and MANIFEST_DIGESTS alone; only the release step \
         (`node tools/theme-parts-release.mjs`) raises the version and pins its digest",
        parts::VERSION
    );
    // No version is pinned past the last release, and none twice.
    let mut versions: Vec<u32> = MANIFEST_DIGESTS.iter().map(|(one, _)| *one).collect();
    assert!(
        versions.iter().all(|one| *one <= parts::VERSION),
        "{versions:?}"
    );
    versions.dedup();
    assert_eq!(versions.len(), MANIFEST_DIGESTS.len(), "{versions:?}");
}

/// Every `since` the manifest says, with what says it.
fn every_since() -> Vec<(String, u32, u32)> {
    let mut said = Vec::new();
    for state in parts::STATES {
        said.push((format!("state {}", state.name()), state.since(), 1));
    }
    for property in parts::PROPERTIES {
        said.push((format!("property {}", property.name()), property.since(), 1));
    }
    for family in parts::MANIFEST {
        said.push((family.name.to_owned(), family.since, 1));
        for part in family.parts {
            let name = format!("{}.{}", family.name, part.name);
            said.push((name, part.since, family.since));
        }
        for variant in family.variants {
            let name = format!("{}.{}", family.name, variant.name);
            said.push((name, variant.since, family.since));
        }
    }
    said
}

#[test]
fn what_is_unreleased_is_since_next_and_nothing_is_older_than_what_holds_it() {
    // `VERSION + 1` here, not the marker, which the release step replaces where it is written.
    let next = parts::VERSION + 1;
    for (name, since, least) in every_since() {
        assert!(
            since <= next,
            "{name} is since {since}: what no release has shipped is since NEXT ({next})"
        );
        assert!(
            since >= least,
            "{name} is since {since}, before what holds it ({least})"
        );
    }
    // The manifest as of NEXT is the whole of it, and as of the last release it has no more than
    // the whole.
    assert_eq!(parts::manifest_as_of(next), parts::manifest_json());
    let unreleased = every_since()
        .iter()
        .any(|(_, since, _)| *since > parts::VERSION);
    assert_eq!(parts::has_unreleased(), unreleased);
    if !unreleased {
        assert_eq!(
            parts::manifest_as_of(parts::VERSION),
            parts::manifest_json()
        );
    }
}

#[test]
fn an_unreleased_addition_is_listed_but_left_out_of_what_the_release_shipped() {
    let released = parts::manifest_as_of(parts::VERSION);
    let all = parts::manifest_json();
    assert_eq!(released["version"], parts::VERSION);
    assert_eq!(all["version"], parts::VERSION);
    // The manifest at an earlier version leaves out what came after it: the button alone at 1,
    // with none of version 2's variants.
    let first = parts::manifest_as_of(1);
    let families = first["families"].as_array().unwrap();
    assert_eq!(families.len(), 1, "{families:?}");
    assert_eq!(families[0]["name"], "button");
    assert!(
        families[0]["variants"]
            .as_array()
            .unwrap()
            .iter()
            .all(|one| one["since"] == 1)
    );
    assert!(
        first["properties"]
            .as_array()
            .unwrap()
            .iter()
            .all(|one| one["since"] == 1)
    );
}

#[test]
fn the_schema_says_the_last_release_and_whether_there_is_more() {
    let said = schema()["properties"]["parts"]["description"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let version = format!("manifest version {}", parts::VERSION);
    if parts::has_unreleased() {
        let more = format!(
            "{version}, plus unreleased additions marked since {}",
            parts::VERSION + 1
        );
        assert!(said.contains(&more), "{said}");
    } else {
        assert!(said.contains(&format!("({version})")), "{said}");
    }
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
fn a_border_width_never_reaches_another_size() {
    let sheet = sheet_of(json!({ "button.root.border-width": 2 }));
    let line = sheet
        .lines()
        .find(|line| line.contains("border-width: 2px"))
        .expect("the width's rule");
    assert!(
        line.contains("[data-scope=\"button\"][data-part=\"root\"]:not([data-size=\"sm\"], [data-size=\"lg\"], [data-scope=\"button-row\"][data-wraps] > *)"),
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

#[test]
fn a_parts_words_are_held_on_every_ground_its_family_is_drawn_on() {
    // A ghost button is see-through, so its words are read on whatever it is drawn on: a
    // Notice's trouble line puts buttons on the danger surface, and a wash is read laid over the
    // surface under it.
    let words = json!({ "button.root.ghost.text": "text.secondary" });
    let as_given = fallen_parts(&with_tokens_and_parts(json!({}), words.clone()));
    assert!(as_given.is_empty(), "{as_given:?}");
    for (ground, hex) in [("danger.surface", "#8a8980"), ("danger.wash", "#c9c8c0c0")] {
        let fallen = fallen_parts(&with_tokens_and_parts(
            json!({ ground: hex }),
            words.clone(),
        ));
        assert!(
            fallen.contains(&"button.root.ghost".to_owned()),
            "{ground}: {fallen:?}"
        );
    }
}

#[test]
fn a_grounds_alpha_is_read_as_drawn() {
    // Every ground written with an alpha that draws it opaque is the same ground, so a part is
    // held on it exactly as on the ground written without one.
    let mut grounds = serde_json::Map::new();
    for (token, hex) in [
        ("surface.base", "#191917"),
        ("surface.raised", "#1b1b19"),
        ("surface.overlay", "#1f1f1c"),
        ("surface.sunken", "#111110"),
        ("surface.deep", "#141413"),
        ("border.subtle", "#2f2f2b"),
        ("tab.active", "#1f3530"),
        ("danger.surface", "#2a1a1a"),
        ("layer.project", "#141413"),
        ("layer.workspace", "#191917"),
        ("layer.chat", "#1d1d1b"),
        ("layer.selected", "#2a2a26"),
    ] {
        grounds.insert(token.to_owned(), Value::from(format!("{hex}ff")));
    }
    for parts in [
        json!({
            "button.root.primary.fill": "transparent",
            "button.root.primary.border-colour": "focus.ring",
            "button.root.primary.hover.fill": "control.hover",
        }),
        json!({ "button.root.ghost.text": "surface.overlay" }),
    ] {
        let plain = fallen_parts(&with_tokens_and_parts(json!({}), parts.clone()));
        let written = fallen_parts(&with_tokens_and_parts(
            Value::Object(grounds.clone()),
            parts,
        ));
        assert!(!plain.is_empty(), "{plain:?}");
        assert_eq!(written, plain);
    }
}

// ---------------------------------------------------------------------------------------
// The field families (#2905)
// ---------------------------------------------------------------------------------------

#[test]
fn a_family_with_no_variants_is_reached_by_its_part_alone() {
    let sheet = sheet_of(json!({
        "text-input.root.corner-radius": 8,
        "text-input.root.hover.fill": "control.hover",
        "field.help.text": "text.muted",
    }));
    assert!(
        sheet.contains(":where([data-scope=\"text-input\"][data-part=\"root\"]) { border-radius: min(8px, calc(var(--purlis-part-height, var(--size-control)) / 2)); }"),
        "{sheet}"
    );
    assert!(
        sheet.contains(
            "[data-scope=\"text-input\"][data-part=\"root\"][data-hovered]:not([data-disabled])"
        ),
        "{sheet}"
    );
    assert!(sheet.contains(
        ":where([data-scope=\"field\"][data-part=\"root\"] [data-scope=\"field\"][data-part=\"help\"]) { color: var(--text-muted); }"
    ));
    assert!(!sheet.contains("data-variant"), "{sheet}");
    // Only the families whose corners the theme sets are clamped to their own sizes.
    assert!(sheet.contains(
        ":where([data-scope=\"text-input\"][data-part=\"root\"][data-size=\"sm\"]) { --purlis-part-height: max(var(--size-chrome), var(--space-7)); }"
    ));
    assert!(!sheet.contains("[data-scope=\"button\"]"), "{sheet}");
}

#[test]
fn a_field_has_the_focus_whenever_it_holds_the_caret() {
    let sheet = sheet_of(json!({
        "text-input.root.focused.border-colour": "accent.base",
        "textarea.root.focused.border-colour": "accent.base",
        "button.root.focused.border-colour": "accent.base",
    }));
    assert!(sheet.contains("[data-scope=\"text-input\"][data-part=\"root\"][data-focus-within]"));
    assert!(sheet.contains("[data-scope=\"textarea\"][data-part=\"root\"][data-focus-within]"));
    assert!(sheet.contains("[data-scope=\"button\"][data-part=\"root\"][data-focus-visible]"));
}

#[test]
fn a_text_inputs_size_never_reaches_its_small_or_large_box() {
    let sheet = sheet_of(json!({ "text-input.root.height": 30 }));
    assert!(
        sheet.contains("[data-scope=\"text-input\"][data-part=\"root\"]:not([data-size=\"sm\"], [data-size=\"lg\"])"),
        "{sheet}"
    );
    let short = refusal(json!({ "text-input.root.height": "size.chrome" }));
    assert!(
        short
            .iter()
            .any(|one| one.starts_with("parts: text-input.root is 21px tall in comfortable")),
        "{short:?}"
    );
    // A textarea is as tall as its lines; a height for it is no part purlis has.
    let loaded = load(&with_parts(json!({ "textarea.root.height": 40 })));
    assert!(!loaded.refused);
    assert!(
        said(&loaded)
            .iter()
            .any(|one| one.contains("parts.textarea.root.height is not a component part")),
        "{:?}",
        said(&loaded)
    );
}

#[test]
fn a_fields_help_that_breaks_its_floor_is_drawn_in_its_own_default() {
    let loaded = load(&with_parts(json!({ "field.help.text": "surface.base" })));
    let theme = loaded.theme.first();
    let fallen = parts::fallen(theme);
    assert_eq!(fallen.len(), 1, "{fallen:?}");
    assert_eq!(fallen[0].part, "field.root");
    assert!(fallen[0].floor.contains("the help's words"), "{fallen:?}");
    let sheet = over_classic(&parts::stylesheet(theme));
    // Its help goes back to the quiet tier and its error to the danger colour, and the field's
    // root, which draws no colour of its own, is given none.
    assert!(sheet.contains("color: var(--text-quiet);"), "{sheet}");
    assert!(sheet.contains("color: var(--danger-text);"), "{sheet}");
    assert!(!sheet.contains("background-color"), "{sheet}");
}

#[test]
fn a_boxs_words_on_its_fill_are_held_to_their_floor() {
    let loaded = load(&with_parts(
        json!({ "text-input.root.fill": "text.primary" }),
    ));
    let fallen = parts::fallen(loaded.theme.first());
    assert_eq!(
        fallen
            .iter()
            .map(|one| one.part.as_str())
            .collect::<Vec<_>>(),
        ["text-input.root"]
    );
    // The prototype's look (R54): borderless on the window's fill. A border that is the box's
    // only edge on its own ground must hold 3:1; with none, its words are what is measured.
    let edged = load(&with_parts(
        json!({ "text-input.root.fill": "surface.base" }),
    ));
    assert!(
        parts::fallen(edged.theme.first())[0]
            .floor
            .contains("border contrasts 3:1")
    );
    let clear = load(&with_parts(json!({
        "text-input.root.fill": "surface.base",
        "text-input.root.border-width": 0,
        "textarea.root.border-width": 0,
    })));
    let fell = parts::fallen(clear.theme.first());
    assert!(fell.is_empty(), "{fell:?}");
}

#[test]
fn the_field_families_default_is_what_field_css_draws() {
    let css = include_str!("../../../../app/src/ui/field/field.css");
    let token = |var: &str| -> String {
        var.strip_prefix("var(--")
            .and_then(|it| it.strip_suffix(')'))
            .map(|name| {
                vocabulary::TOKENS
                    .iter()
                    .find(|token| token.replace('.', "-") == name)
                    .unwrap_or_else(|| panic!("--{name} is not a token"))
                    .to_string()
            })
            .unwrap_or_else(|| var.to_owned())
    };
    let boxes = rule(
        css,
        "[data-scope=\"text-input\"][data-part=\"root\"],\n[data-scope=\"textarea\"][data-part=\"root\"]",
    );
    let held = parts::field_defaults_json();
    assert_eq!(
        held["box"]["fill"],
        token(declared(boxes, "background-color").unwrap())
    );
    assert_eq!(
        held["box"]["text"],
        token(declared(boxes, "color").unwrap())
    );
    let border = declared(boxes, "border").and_then(|it| it.split_whitespace().last());
    assert_eq!(held["box"]["border"], token(border.unwrap()));
    for part in ["label", "help", "error"] {
        let drawn = rule(
            css,
            &format!("[data-scope=\"field\"][data-part=\"{part}\"]"),
        );
        assert_eq!(
            held[part],
            token(declared(drawn, "color").unwrap()),
            "the field's {part}"
        );
    }
}

#[test]
fn the_select_families_default_is_what_select_css_draws() {
    let css = include_str!("../../../../app/src/ui/select/select.css");
    let token = |var: &str| -> String {
        var.strip_prefix("var(--")
            .and_then(|it| it.strip_suffix(')'))
            .map(|name| {
                vocabulary::TOKENS
                    .iter()
                    .find(|token| token.replace('.', "-") == name)
                    .unwrap_or_else(|| panic!("--{name} is not a token"))
                    .to_string()
            })
            .unwrap_or_else(|| var.to_owned())
    };
    let boxes = rule(
        css,
        "[data-scope=\"select\"][data-part=\"root\"],\n[data-scope=\"combobox\"][data-part=\"root\"]",
    );
    for family in ["select", "combobox"] {
        let held = parts::look_json(family, "");
        assert_eq!(
            held["fill"],
            token(declared(boxes, "background-color").unwrap()),
            "{family}"
        );
        assert_eq!(
            held["text"],
            token(declared(boxes, "color").unwrap()),
            "{family}"
        );
        let border = declared(boxes, "border").and_then(|it| it.split_whitespace().last());
        assert_eq!(held["border"], token(border.unwrap()), "{family}");
        // The box a select draws is the text input's (`field.css`), so the two are one look.
        assert_eq!(held, parts::look_json("text-input", ""), "{family}");
    }
    // R64: the chevron and the placeholder in the quiet tier, as the engine holds them.
    let chevrons = rule(
        css,
        "[data-scope=\"select\"][data-part=\"chevron\"],\n[data-scope=\"combobox\"][data-part=\"chevron\"]",
    );
    assert_eq!(token(declared(chevrons, "color").unwrap()), "text.quiet");
    let placeholder = rule(css, "[data-scope=\"select\"][data-part=\"placeholder\"]");
    assert_eq!(token(declared(placeholder, "color").unwrap()), "text.quiet");
    let manifest = parts::manifest_json();
    let words = |name: &str| {
        manifest["families"]
            .as_array()
            .unwrap()
            .iter()
            .find(|one| one["name"] == name)
            .unwrap_or_else(|| panic!("no {name}"))
            .clone()
    };
    let select = words("select");
    assert_eq!(select["since"], parts::NEXT);
    assert_eq!(select["focused"], "[data-focus-visible]");
    assert_eq!(words("combobox")["focused"], "[data-focus-within]");
}

#[test]
fn a_select_and_a_combobox_take_the_text_inputs_keys_and_hold_their_floors() {
    // A width but 0: a box that can be refused keeps its edge (#3110).
    let loaded = load(&with_parts(json!({
        "select.root.border-width": 2,
        "select.root.fill": "surface.raised",
        "select.chevron.text": "text.muted",
        "select.placeholder.text": "text.secondary",
        "combobox.root.corner-radius": "full",
        "select.root.open.border-colour": "accent.base",
    })));
    assert!(!loaded.refused, "{:?}", said(&loaded));
    assert!(said(&loaded).is_empty(), "{:?}", said(&loaded));
    let drawn = parts::stylesheet(loaded.theme.first());
    assert!(
        drawn.contains("[data-scope=\"select\"][data-part=\"root\"][data-open]"),
        "{drawn}"
    );
    assert!(
        drawn.contains("[data-scope=\"select\"][data-part=\"root\"] [data-scope=\"select\"][data-part=\"chevron\"]"),
        "{drawn}"
    );
    // A state a select's button never takes is no part.
    let pressed = load(&with_parts(
        json!({ "select.root.pressed.fill": "accent.surface" }),
    ));
    assert!(
        said(&pressed)
            .iter()
            .any(|one| one.contains("parts.select.root.pressed.fill is not a component part")),
        "{:?}",
        said(&pressed)
    );
}

#[test]
fn a_state_a_fields_wrapper_never_marks_is_no_part() {
    let loaded = load(&with_parts(json!({
        "field.label.focused.text": "accent.base",
        "field.help.hover.text": "accent.base",
        "text-input.root.pressed.fill": "accent.surface",
        "textarea.root.open.fill": "accent.surface",
        "text-input.root.hover.border-colour": "accent.base",
    })));
    assert!(!loaded.refused, "{:?}", said(&loaded));
    for key in [
        "field.label.focused.text",
        "field.help.hover.text",
        "text-input.root.pressed.fill",
        "textarea.root.open.fill",
    ] {
        assert!(
            said(&loaded)
                .iter()
                .any(|one| one.contains(&format!("parts.{key} is not a component part"))),
            "{key}: {:?}",
            said(&loaded)
        );
    }
    assert_eq!(
        loaded
            .theme
            .first()
            .parts
            .keys()
            .filter(|key| !classics(key))
            .count(),
        1
    );
    let keys = schema()["properties"]["parts"]["properties"]
        .as_object()
        .cloned()
        .unwrap_or_default();
    assert!(keys.contains_key("text-input.root.hover.border-colour"));
    assert!(keys.contains_key("field.label.disabled.text"));
    assert!(!keys.contains_key("field.label.focused.text"));
    assert!(!keys.contains_key("textarea.root.selected.fill"));
}

#[test]
fn a_field_is_measured_on_every_ground_a_control_stands_on() {
    let button = parts::family("button").expect("the button family");
    for name in ["field", "text-input", "textarea"] {
        let family = parts::family(name).expect("a field family");
        assert_eq!(family.grounds, button.grounds, "{name}");
    }
}

// ---------------------------------------------------------------------------------------
// States that hold at once
// ---------------------------------------------------------------------------------------

#[test]
fn the_primary_never_takes_allows_look_in_two_states_at_once() {
    // The look two states draw at once, as the stylesheet resolves it, is held like one state's.
    for later in ["pressed", "focused", "open"] {
        let fallen = fallen_parts(&with_parts(json!({
            "button.root.primary.hover.fill": "control.hover",
            "button.root.primary.hover.border-colour": "text.secondary",
            format!("button.root.primary.{later}.border-colour"): "accent.base",
        })));
        assert!(
            fallen.contains(&format!("button.root.primary.hover+{later}")),
            "{later}: {fallen:?}"
        );
    }
}

#[test]
fn a_parts_words_are_held_in_every_set_of_states_that_hold_at_once() {
    // A toggle that is on draws its own fill under the pointer; the words a theme gives the plain
    // button are held on it there too, and the finding names both states.
    let fallen = fallen_parts(&with_tokens_and_parts(
        json!({ "tab.active": "#a3a29a" }),
        json!({ "button.root.secondary.text": "text.secondary" }),
    ));
    assert!(
        fallen.contains(&"button.root.secondary.hover+selected".to_owned()),
        "{fallen:?}"
    );
    assert!(
        !fallen.contains(&"button.root.secondary".to_owned()),
        "{fallen:?}"
    );
}

#[test]
fn a_set_of_states_that_falls_back_draws_a_default_its_floors_hold() {
    let loaded = load(&with_parts(json!({
        "button.root.primary.hover.fill": "control.hover",
        "button.root.primary.hover.border-colour": "text.secondary",
        "button.root.primary.focused.border-colour": "accent.base",
    })));
    let theme = loaded.theme.first();
    let sheet = parts::stylesheet(theme);
    // The fallback is written once, for the state the stylesheet writes last.
    let focused = "[data-variant=\"primary\"][data-focus-visible])";
    let written: Vec<&str> = sheet
        .lines()
        .filter(|line| line.contains(focused))
        .collect();
    assert_eq!(written.len(), 2, "{sheet}");
    assert!(written[1].contains("var(--accent-fill)"), "{sheet}");
    assert!(
        written[1].contains("border-color: var(--accent-fill-edge)"),
        "{sheet}"
    );
}

#[test]
fn states_whose_mix_is_the_familys_own_look_are_not_held_back() {
    let fallen = fallen_parts(&with_parts(json!({
        "button.root.primary.hover.fill": "tab.active",
        "button.root.primary.focused.border-colour": "accent.base",
        "button.root.corner-radius": "full",
    })));
    assert!(fallen.is_empty(), "{fallen:?}");
}

#[test]
fn the_floors_are_held_until_holding_them_again_writes_no_fallback() {
    // A fallback changes how a state draws, so the hold runs again until a round writes none:
    // holding the result once more finds nothing that falls.
    let themes = [
        with_tokens_and_parts(
            json!({ "control.hover": "#9a9a90" }),
            json!({
                "button.root.secondary.focused.fill": "accent.surface",
                "button.root.grants.focused.fill": "accent.surface",
                "button.root.primary.focused.fill": "control.base",
                "button.root.primary.focused.border-colour": "accent.base",
                "button.root.primary.hover.fill": "control.hover",
                "button.root.primary.hover.text": "surface.base",
            }),
        ),
        with_parts(json!({
            "button.root.primary.hover.fill": "control.hover",
            "button.root.primary.hover.border-colour": "text.secondary",
            "button.root.primary.open.border-colour": "accent.base",
            "button.root.secondary.open.fill": "accent.surface",
            "button.root.grants.open.fill": "accent.surface",
            "button.root.primary.open.fill": "control.base",
        })),
        with_tokens_and_parts(
            json!({ "tab.active": "#a3a29a" }),
            json!({ "button.root.secondary.text": "text.secondary" }),
        ),
        with_tokens_and_parts(
            json!({ "tab.active": "#a3a29a" }),
            json!({
                "button.root.primary.hover.fill": "tab.active",
                "button.root.primary.hover.text": "surface.base",
                "button.root.primary.focused.fill": "control.base",
                "button.root.primary.focused.border-colour": "accent.base",
                "button.root.secondary.focused.fill": "accent.surface",
                "button.root.grants.focused.fill": "accent.surface",
            }),
        ),
    ];
    for document in &themes {
        let loaded = load(document);
        assert!(!loaded.refused, "{:?}", said(&loaded));
        let theme = loaded.theme.first();
        assert_eq!(parts::held_again(theme), [], "{document}");
    }
}

#[test]
fn a_field_familys_own_states_are_held_at_once_too() {
    // A text box is never pressed, chosen or open: its own states (hovered and focused) are the
    // sets it is measured in, and each falls back with the set named.
    let fallen = fallen_parts(&with_parts(json!({
        "text-input.root.hover.fill": "text.primary",
        "text-input.root.hover.text": "surface.base",
        "text-input.root.focused.fill": "surface.sunken",
    })));
    assert!(
        fallen.contains(&"text-input.root.hover+focused".to_owned()),
        "{fallen:?}"
    );
    assert!(
        !fallen.contains(&"text-input.root.hover".to_owned()),
        "{fallen:?}"
    );
    assert!(
        !fallen.contains(&"text-input.root.focused".to_owned()),
        "{fallen:?}"
    );
    assert!(
        !fallen
            .iter()
            .any(|one| one.contains("pressed") || one.contains("open")),
        "{fallen:?}"
    );
}

// ---------------------------------------------------------------------------------------
// The dialog family (#2910)
// ---------------------------------------------------------------------------------------

#[test]
fn a_theme_rounds_a_dialog_and_its_corner_is_not_a_controls() {
    let sheet = sheet_of(json!({ "dialog.root.corner-radius": 12 }));
    // A dialog is a surface, not a target: its corner is not clamped by a height, only kept
    // within the inset of what it holds.
    assert!(
        sheet.contains(
            ":where([data-scope=\"dialog\"][data-part=\"root\"]) { border-radius: min(12px, var(--space-6)); }"
        ),
        "{sheet}"
    );
    let alert = sheet_of(json!({ "dialog.title.alert.font-weight": 700 }));
    assert!(
        alert.contains(
            ":where([data-scope=\"dialog\"][data-part=\"root\"][data-variant=\"alert\"] [data-scope=\"dialog\"][data-part=\"title\"])"
        ),
        "{alert}"
    );
}

#[test]
fn a_dialogs_corner_never_cuts_off_what_it_holds() {
    // `full` on a surface is the inset of what it holds, never a pill: Cancel, bottom-leading,
    // and the title's first word stay whole whatever a theme rounds.
    let full = sheet_of(json!({ "dialog.root.corner-radius": "full" }));
    assert!(
        full.contains(
            ":where([data-scope=\"dialog\"][data-part=\"root\"]) { border-radius: var(--space-6); }"
        ),
        "{full}"
    );
    assert!(!full.contains("9999px"), "{full}");
    let four = sheet_of(json!({ "dialog.root.corner-radius": ["full", 0, "radius.lg", 400] }));
    assert!(
        four.contains(
            "border-radius: var(--space-6) min(0px, var(--space-6)) min(var(--radius-lg), var(--space-6)) min(400px, var(--space-6));"
        ),
        "{four}"
    );
    // A control's corner is still its own, a pill at any height.
    assert!(sheet_of(json!({ "button.root.corner-radius": "full" })).contains("9999px"));
}

#[test]
fn a_dialogs_quieter_words_are_measured_in_the_tier_they_are_drawn_in() {
    // The fill clears the primary words but not the quiet summary, the muted subtitle or the
    // secondary question: the dialog's fill falls to the family default.
    for tier in ["text.quiet", "text.muted", "text.secondary"] {
        let fell = fallen_parts(&with_tokens_and_parts(
            json!({ "surface.raised": "#1f1f1c", tier: "#2a2a26" }),
            json!({ "dialog.root.fill": "surface.raised" }),
        ));
        assert!(
            fell.iter().any(|one| one.starts_with("dialog.root.")),
            "{tier}: {fell:?}"
        );
    }
    // The same fill with every tier readable is drawn.
    assert!(
        fallen_parts(&with_tokens_and_parts(
            json!({ "surface.raised": "#1f1f1c" }),
            json!({ "dialog.root.fill": "surface.raised" }),
        ))
        .is_empty()
    );
}

#[test]
fn the_dialogs_tiers_are_what_dialog_css_draws() {
    let css = include_str!("../../../../app/src/ui/dialog/dialog.css");
    for (part, tier) in parts::DIALOG_TIERS {
        let drawn = rule(
            css,
            &format!("[data-scope=\"dialog\"][data-part=\"{part}\"]"),
        );
        let colour = declared(drawn, "color").expect("its words");
        assert_eq!(
            colour,
            format!("var(--{})", tier.replace('.', "-")),
            "{part}"
        );
    }
}

#[test]
fn a_dialog_takes_no_height_and_no_size_inside_a_state() {
    // Its height is the dialog rules', never a theme's: the key is said and left out.
    let loaded = load(&with_parts(json!({ "dialog.root.height": 400 })));
    assert!(!loaded.refused, "{:?}", said(&loaded));
    assert!(
        said(&loaded)
            .iter()
            .any(|one| one.contains("parts.dialog.root.height is not")),
        "{:?}",
        said(&loaded)
    );
    let refused = refusal(json!({ "dialog.title.hover.font-size": 18 }));
    assert!(
        refused
            .iter()
            .any(|one| one.contains("a state never changes")),
        "{refused:?}"
    );
}

#[test]
fn a_dialogs_words_that_break_their_floor_draw_the_family_default() {
    let loaded = load(&with_parts(
        json!({ "dialog.title.text": "surface.overlay" }),
    ));
    assert!(!loaded.refused, "{:?}", said(&loaded));
    let fell = parts::fallen(loaded.theme.first());
    assert!(
        fell.iter().any(|one| one.part.starts_with("dialog.root.")),
        "{fell:?}"
    );
}

#[test]
fn the_dialogs_family_default_is_what_dialog_css_draws() {
    let css = include_str!("../../../../app/src/ui/dialog/dialog.css");
    let root = rule(css, "[data-scope=\"dialog\"][data-part=\"root\"]");
    let token = |var: &str| {
        var.trim_start_matches("var(--")
            .trim_end_matches(')')
            .replacen('-', ".", 1)
    };
    let border = declared(root, "border")
        .and_then(|it| it.split_whitespace().last())
        .expect("the dialog's border");
    let drawn = json!({
        "fill": token(declared(root, "background-color").expect("its fill")),
        "text": token(declared(root, "color").expect("its words")),
        "border": token(border),
    });
    for variant in ["dialog", "alert"] {
        let held = parts::dialog_defaults_json(variant);
        assert_eq!(held, drawn, "{variant}");
    }
}

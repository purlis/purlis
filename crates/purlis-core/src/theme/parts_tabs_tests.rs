// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The session tabs' parts at the engine's interface (#2914): the layout options a theme sets
//! (`tabs.layout.attach`, sizing, dividers, gap), the chosen tab's flare and the hover pill's
//! inset, a selection that never changes a tab's size, the parts drawn around a tab, the editor
//! toolbar a file tab joins, and Purlis Classic's strip look kept through parts.

use serde_json::{Value, json};

use super::parts::{self, State};
use super::*;

/// A theme of its own, extending nothing, that sets `parts`: what the family defaults draw
/// under, with nothing of Classic's.
fn own(parts: Value) -> Value {
    json!({
        "name": "Docked",
        "appearances": [{ "id": "dark", "appearance": "dark", "tokens": {} }],
        "parts": parts,
    })
}

fn said(resolved: &Resolved) -> Vec<String> {
    resolved
        .complaints
        .iter()
        .map(|one| one.said.clone())
        .collect()
}

fn sheet_of(parts: Value) -> String {
    let loaded = load(&own(parts));
    assert!(!loaded.refused, "{:?}", said(&loaded));
    parts::stylesheet(loaded.theme.first())
}

fn refusal(parts: Value) -> Vec<String> {
    let loaded = load(&own(parts));
    assert!(loaded.refused, "the theme was drawn");
    said(&loaded)
}

/// The declarations of the rule `selector` opens, on its one line.
fn rule<'a>(sheet: &'a str, selector: &str) -> &'a str {
    let opened = format!("{selector} {{ ");
    let at = sheet
        .find(&opened)
        .unwrap_or_else(|| panic!("no rule for {selector} in\n{sheet}"));
    let rest = &sheet[at + opened.len()..];
    &rest[..rest.find(" }").expect("the rule closes")]
}

const LAYOUT: &str = ":where([data-scope=\"tabs\"][data-part=\"layout\"])";
const STRIP: &str = ":where([data-scope=\"tabs\"][data-part=\"strip\"])";
const PANEL: &str = ":where([data-scope=\"panel\"][data-part=\"root\"][data-variant=\"content\"])";
const TAB: &str = "[data-scope=\"tabs\"][data-part=\"tab\"]";
/// A chosen tab and its flares, a file's tab left to its toolbar.
const CHOSEN: &str = ":where([data-scope=\"tabs\"][data-part=\"tab\"][data-selected]:not([data-variant=\"file\"])), :where([data-scope=\"tabs\"][data-part=\"tab\"][data-selected]:not([data-variant=\"file\"]))::before, :where([data-scope=\"tabs\"][data-part=\"tab\"][data-selected]:not([data-variant=\"file\"]))::after";

#[test]
fn the_tabs_attach_to_the_panel_by_default_and_a_theme_puts_them_on_a_strip() {
    // Purlis draws docked tabs from the family defaults: its folder names no tab part, and the
    // chosen tabs it is drawn with join the panel's fill it names.
    let purlis = built_in("purlis").expect("Purlis");
    assert!(
        !purlis
            .parts
            .keys()
            .any(|key| key.family == "tabs" && key.part == "layout"),
        "Purlis's docked tabs are the family default"
    );
    let css = include_str!("../../../../app/src/ui/tabs/tabs.css");
    assert!(
        css.contains("  --tabs-joined: 1;\n"),
        "Purlis's default is docked"
    );
    // On a strip of their own: the chat strip's shade, a chosen tab a step lighter and nothing
    // joined, written first so a fill a theme names wins.
    let strip = sheet_of(json!({
        "tabs.layout.attach": "strip",
        "tabs.strip.fill": "surface.deep",
    }));
    let first: Vec<String> = strip.lines().skip(1).take(3).map(str::to_owned).collect();
    assert_eq!(
        first,
        [
            format!("  {LAYOUT} {{ --tabs-joined: 0; }}"),
            format!("  {STRIP} {{ background-color: var(--tabs-strip-shade); }}"),
            format!("  {CHOSEN} {{ background-color: var(--tabs-chosen-shade); }}"),
        ]
    );
    assert!(
        strip.contains(&format!(
            "  {STRIP} {{ background-color: var(--surface-deep); }}"
        )),
        "{strip}"
    );
    // Back to the panel, as a child of a strip theme says it.
    let panel = sheet_of(json!({ "tabs.layout.attach": "panel" }));
    assert!(
        panel.contains(&format!("  {LAYOUT} {{ --tabs-joined: 1; }}")),
        "{panel}"
    );
    // Docked, each chosen tab takes the family default of what it joins.
    assert!(!panel.contains(CHOSEN), "{panel}");
}

#[test]
fn a_layout_word_is_one_purlis_reads_or_the_theme_is_refused() {
    assert_eq!(
        refusal(json!({ "tabs.layout.attach": "floating" })),
        ["parts.tabs.layout.attach is \"floating\": it is one of panel or strip"]
    );
    assert_eq!(
        refusal(json!({ "tabs.layout.sizing": 3 })),
        ["parts.tabs.layout.sizing is 3: it is one of content or equal"]
    );
    let sheet = sheet_of(json!({
        "tabs.layout.sizing": "equal",
        "tabs.layout.dividers": "on",
        "tabs.layout.gap": 24,
    }));
    // The words in one rule, and the room between tabs, a size, in the rule of sizes after it.
    let written = rule(&sheet, LAYOUT);
    for one in ["--tabs-grow: 1; --tabs-basis: 0px;", "--tabs-divider: 1px;"] {
        assert!(written.contains(one), "{one} in {written}");
    }
    assert!(
        sheet.contains(&format!("{LAYOUT} {{ --tabs-gap: calc(24rem / 14); }}")),
        "{sheet}"
    );
}

#[test]
fn the_chosen_tabs_flare_and_the_hover_pills_inset_are_set_in_their_own_state() {
    let sheet = sheet_of(json!({
        "tabs.tab.selected.flare": 8,
        "tabs.tab.hover.inset": [3, 0, 5],
    }));
    assert_eq!(
        rule(&sheet, &format!(":where({TAB}[data-selected])")),
        "--tabs-flare: 8px;"
    );
    assert_eq!(
        rule(
            &sheet,
            &format!(":where({TAB}[data-hovered]:not([data-disabled]))")
        ),
        "--tabs-hover-inset: 3px 0px 5px;"
    );
    // One value is every side; a pill that fills its cell takes no corners of its own.
    let sheet = sheet_of(json!({ "tabs.tab.hover.inset": 0 }));
    assert!(
        sheet.contains("--tabs-hover-inset: 0px 0px 0px; --tabs-hover-corner: 0px;"),
        "{sheet}"
    );
    // Anywhere else they are refused, with where they go.
    assert_eq!(
        refusal(json!({ "tabs.tab.flare": 8 })),
        [
            "parts.tabs.tab.flare is refused: a tab's flare is drawn only while it is chosen, so \
             it is set as tabs.tab.selected.flare"
        ]
    );
    assert_eq!(
        refusal(json!({ "tabs.tab.view.selected.inset": 2 })),
        [
            "parts.tabs.tab.view.selected.inset is refused: a tab's inset is drawn only while it \
             is under the pointer, so it is set as tabs.tab.view.hover.inset"
        ]
    );
    assert_eq!(
        refusal(json!({ "tabs.tab.selected.flare": 17 })),
        [
            "parts.tabs.tab.selected.flare is 17, outside its bound: it is a whole number from 0 \
             to 16 px"
        ]
    );
}

#[test]
fn choosing_a_tab_never_changes_its_size() {
    // R149: a heavier chosen tab is refused, as every size inside a state is (#2800).
    for (key, value) in [
        ("tabs.tab.selected.font-weight", json!(600)),
        ("tabs.tab.selected.padding", json!(4)),
        ("tabs.tab.selected.height", json!(31)),
        ("tabs.tab.selected.max-width", json!(200)),
        ("tabs.label.selected.font-weight", json!(500)),
        ("tabs.tab.hover.corner-radius", json!(7)),
    ] {
        let said = refusal(json!({ key: value }));
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

#[test]
fn a_tabs_size_is_its_own_and_its_widest_is_bounded() {
    let sheet = sheet_of(json!({
        "tabs.tab.max-width": 240,
        "tabs.tab.corner-radius": [8, 8, 0, 0],
        "tabs.tab.padding": [0, 12],
    }));
    let written = rule(&sheet, &format!(":where({TAB})"));
    assert!(
        written.contains("max-inline-size: calc(240rem / 14);"),
        "{written}"
    );
    assert!(
        sheet.contains("padding-block: 0; padding-inline: calc(12rem / 14);"),
        "{sheet}"
    );
    assert!(sheet.contains("border-radius: min(8px, "), "{sheet}");
    assert_eq!(
        refusal(json!({ "tabs.tab.max-width": 20 })),
        [
            "parts.tabs.tab.max-width is 20, outside its bound: it is a whole number from 48 to \
             480 px at the default text size"
        ]
    );
}

#[test]
fn a_part_drawn_around_the_tabs_takes_no_variant_and_no_state() {
    let loaded = load(&own(json!({
        "tabs.strip.hover.fill": "surface.overlay",
        "tabs.layout.file.attach": "strip",
    })));
    assert!(!loaded.refused);
    let notes: Vec<String> = said(&loaded)
        .into_iter()
        .filter(|one| one.starts_with("parts."))
        .collect();
    assert_eq!(
        notes,
        [
            "parts.tabs.strip.hover.fill is not a component part purlis has: tabs.strip is drawn \
             around every tab and takes no variant or state, so it was left out",
            "parts.tabs.layout.file.attach is not a component part purlis has: tabs.layout is \
             drawn around every tab and takes no variant or state, so it was left out",
        ]
    );
    // The strip is reached by its own attributes, not inside a tab, and its line is under it.
    let sheet = sheet_of(json!({
        "tabs.strip.fill": "surface.deep",
        "tabs.strip.border-colour": "border.subtle",
        "tabs.strip.border-width": 1,
    }));
    let strip = ":where([data-scope=\"tabs\"][data-part=\"strip\"])";
    assert_eq!(
        rule(&sheet, strip),
        "background-color: var(--surface-deep); border-block-end-color: var(--border-subtle);"
    );
    assert!(
        sheet.contains(&format!(
            "{strip} {{ border-block-end-width: 1px; border-block-end-style: solid; }}"
        )),
        "{sheet}"
    );
}

#[test]
fn an_indicator_is_a_mark_a_few_px_tall_on_the_edge_a_theme_names() {
    let sheet = sheet_of(json!({
        "tabs.indicator.height": 2,
        "tabs.indicator.position": "bottom",
        "tabs.indicator.fill": "accent.base",
    }));
    let written = rule(
        &sheet,
        &format!(":where({TAB} [data-scope=\"tabs\"][data-part=\"indicator\"])"),
    );
    assert!(sheet.contains("block-size: 2px;"), "{sheet}");
    assert!(
        written.contains("inset-block-start: auto; inset-block-end: 0;"),
        "{written}"
    );
    assert_eq!(
        refusal(json!({ "tabs.indicator.height": 7 })),
        [
            "parts.tabs.indicator.height is 7, outside its bound: it is a whole number from 0 to 6 px"
        ]
    );
}

#[test]
fn what_a_chosen_tab_joins_it_takes_the_fill_of() {
    // The panel's fill is the chosen tab's, and a file's toolbar's is the chosen file tab's,
    // each drawn on the tab and its flares; a hover fill is the pill an unchosen tab draws.
    let sheet = sheet_of(json!({
        "panel.root.content.fill": "surface.overlay",
        "editor.toolbar.fill": "surface.hover",
        "tabs.tab.hover.fill": "surface.hover",
    }));
    let chosen = format!(":where({TAB}[data-selected])");
    let file = format!(":where({TAB}[data-variant=\"file\"][data-selected])");
    for one in [
        format!("  {PANEL} {{ background-color: var(--surface-overlay); }}"),
        "  :where([data-scope=\"editor\"][data-part=\"toolbar\"]) { background-color: \
         var(--surface-hover); }"
            .to_owned(),
        format!("  {chosen} {{ background-color: var(--surface-overlay); }}"),
        format!("  {chosen}::before {{ background-color: var(--surface-overlay); }}"),
        format!("  {chosen}::after {{ background-color: var(--surface-overlay); }}"),
        format!("  {file} {{ background-color: var(--surface-hover); }}"),
        format!("  {file}::after {{ background-color: var(--surface-hover); }}"),
        format!(
            "  :where({TAB}[data-hovered]:not([data-disabled]):not([data-selected]))::before \
             {{ background-color: var(--surface-hover); }}"
        ),
    ] {
        assert!(sheet.contains(&one), "{one} in\n{sheet}");
    }
    // A theme that gives every chosen tab a fill leaves a file's joined to its toolbar.
    let sheet = sheet_of(json!({ "tabs.tab.selected.fill": "transparent" }));
    assert!(
        sheet.contains(&format!(
            "  {file} {{ background-color: var(--terminal-background); }}"
        )),
        "{sheet}"
    );
}

#[test]
fn a_tabs_words_are_held_to_their_floor_on_every_ground_the_strip_is_drawn_on() {
    // A tab's words in the panel's own colour cannot be read on it: the tab draws its default.
    let loaded = load(&own(json!({ "tabs.tab.text": "surface.raised" })));
    assert!(!loaded.refused, "{:?}", said(&loaded));
    let fell = parts::fallen(loaded.theme.first());
    assert!(
        fell.iter()
            .any(|one| one.part == "tabs.tab.chat" && one.floor.contains("words")),
        "{fell:?}"
    );
}

#[test]
fn purlis_classic_keeps_a_strip_look_through_parts() {
    let classic = classic().first();
    assert!(
        parts::fallen(classic).is_empty(),
        "{:?}",
        parts::fallen(classic)
    );
    let sheet = parts::stylesheet(classic);
    for one in [
        format!("{LAYOUT} {{ --tabs-joined: 0; }}"),
        format!("{LAYOUT} {{ --tabs-grow: 1; --tabs-basis: 0px; --tabs-divider: 1px; }}"),
        format!("{LAYOUT} {{ --tabs-gap: 0; }}"),
        format!("{STRIP} {{ background-color: var(--tabs-strip-shade); }}"),
        format!(":where({TAB}[data-selected]) {{ background-color: var(--layer-selected);"),
        "--tabs-flare: 0px;".to_owned(),
        "--tabs-hover-inset: 0px 0px 0px; --tabs-hover-corner: 0px;".to_owned(),
    ] {
        assert!(sheet.contains(&one), "{one} in\n{sheet}");
    }
    // Its light appearance holds the floors too.
    for one in &classic_theme().appearances {
        assert!(parts::fallen(one).is_empty(), "{}", one.id);
    }
}

fn classic_theme() -> &'static Theme {
    super::classic()
}

#[test]
fn the_manifest_names_the_tabs_and_the_editor_toolbar() {
    let manifest = parts::manifest_json();
    let families: Vec<&str> = manifest["families"]
        .as_array()
        .unwrap()
        .iter()
        .map(|one| one["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        families,
        [
            "button",
            "field",
            "text-input",
            "textarea",
            "tabs",
            "editor",
            "panel",
            "panel-section",
            "empty-state",
            "status-item",
            "tooltip",
            "popover",
            "menu",
            "dialog",
            "rail",
        ]
    );
    let tabs = &manifest["families"][4];
    let parts: Vec<&str> = tabs["parts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|one| one["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        parts,
        [
            "tab",
            "label",
            "closer",
            "count",
            "indicator",
            "strip",
            "layout"
        ]
    );
    let around: Vec<&str> = tabs["parts"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|one| one["around"] == true)
        .map(|one| one["name"].as_str().unwrap())
        .collect();
    assert_eq!(around, ["strip", "layout"]);
    let attach = manifest["properties"]
        .as_array()
        .unwrap()
        .iter()
        .find(|one| one["name"] == "attach")
        .unwrap();
    assert_eq!(attach["bounds"], json!({ "enum": ["panel", "strip"] }));
    let flare = manifest["properties"]
        .as_array()
        .unwrap()
        .iter()
        .find(|one| one["name"] == "flare")
        .unwrap();
    assert_eq!(flare["only_in"], "selected");
}

#[test]
fn the_schema_completes_the_tab_keys_and_none_that_would_move_a_tab() {
    let schema = schema();
    let keys = schema["properties"]["parts"]["properties"]
        .as_object()
        .expect("the part keys");
    for key in [
        "tabs.layout.attach",
        "tabs.layout.sizing",
        "tabs.layout.dividers",
        "tabs.layout.gap",
        "tabs.tab.selected.flare",
        "tabs.tab.hover.inset",
        "tabs.tab.hover.fill",
        "tabs.tab.max-width",
        "tabs.tab.file.selected.fill",
        "tabs.indicator.position",
        "tabs.indicator.height",
        "tabs.count.corner-radius",
        "tabs.closer.corner-radius",
        "tabs.strip.border-width",
        "editor.toolbar.fill",
    ] {
        assert!(keys.contains_key(key), "{key}");
    }
    for key in [
        "tabs.tab.flare",
        "tabs.tab.selected.inset",
        "tabs.tab.selected.font-weight",
        "tabs.strip.hover.fill",
        "tabs.layout.chat.attach",
        "button.root.attach",
    ] {
        assert!(!keys.contains_key(key), "{key}");
    }
    assert_eq!(
        keys["tabs.indicator.height"],
        json!({ "$ref": "#/$defs/part-mark-height" })
    );
}

/// The token a declaration draws in: `transparent`, or the one `var()` names.
fn drawn_in(declared: &str) -> String {
    if declared.trim() == "transparent" {
        return "transparent".to_owned();
    }
    let name = declared
        .trim()
        .strip_prefix("var(--")
        .and_then(|it| it.strip_suffix(')'))
        .unwrap_or_else(|| panic!("{declared} is not one token"));
    vocabulary::TOKENS
        .iter()
        .find(|token| token.replace('.', "-") == name)
        .unwrap_or_else(|| panic!("--{name} is not a token"))
        .to_string()
}

/// What the rule `selector` opens in `css` declares for `property`.
fn declared_in(css: &str, selector: &str, property: &str) -> String {
    // The selector alone, or one of a list, on a line of its own.
    let at = [" {", ","]
        .iter()
        .find_map(|after| css.find(&format!("\n{selector}{after}\n").replace(" {\n", " {")))
        .unwrap_or_else(|| panic!("tabs.css has no rule {selector}"));
    let rest = &css[at..];
    let body = &rest[rest.find('{').expect("the rule opens") + 1..];
    let body = &body[..body.find("\n}").expect("the rule closes")];
    body.lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix(&format!("{property}: ")))
        .map(|value| value.trim_end_matches(';').to_owned())
        .unwrap_or_else(|| panic!("{selector} declares no {property}"))
}

#[test]
fn the_tabs_family_default_is_what_tabs_css_draws() {
    let css = include_str!("../../../../app/src/ui/tabs/tabs.css");
    let tab = "[data-scope=\"tabs\"][data-part=\"tab\"]";
    let look = |variant, state| parts::look_pair_of("tabs", variant, state);
    let drawn = |selector: &str, property| drawn_in(&declared_in(css, selector, property));
    for variant in ["chat", "view", "file", "background"] {
        assert_eq!(
            look(variant, None),
            (drawn(tab, "background-color"), drawn(tab, "color")),
            "{variant} at rest"
        );
        assert_eq!(
            look(variant, Some(State::Hover)),
            (
                drawn(
                    &format!("{tab}[data-hovered]:not([data-selected])::before"),
                    "background-color"
                ),
                drawn(
                    &format!("{tab}[data-hovered]:not([data-disabled])"),
                    "color"
                )
            ),
            "{variant} under the pointer"
        );
        let chosen = if variant == "file" || variant == "chat" {
            drawn(
                &format!("{tab}[data-variant=\"{variant}\"][data-selected]"),
                "background-color",
            )
        } else {
            drawn(&format!("{tab}[data-selected]"), "background-color")
        };
        assert_eq!(
            look(variant, Some(State::Selected)),
            (chosen, drawn(&format!("{tab}[data-selected]"), "color")),
            "{variant} chosen"
        );
    }
    let toolbar = "[data-scope=\"editor\"][data-part=\"toolbar\"]";
    assert_eq!(
        parts::look_pair_of("editor", "default", None),
        (drawn(toolbar, "background-color"), drawn(toolbar, "color"))
    );
}

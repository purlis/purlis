// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The window's own families and its geometry at the engine's interface (#2913): the region
//! panel, a panel's section, an empty state and a status item in the manifest, their keys and
//! their stylesheet, the least target held only where a family is a control, their colours held
//! to their stylesheets, and `window.gutter`, `panel.corner-radius` and `panel.border-width`,
//! where Purlis Classic draws flush and Purlis floats its panels (#2805).

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

/// The stylesheet of a theme's first appearance, which must be drawn.
fn sheet_of(parts: Value) -> String {
    let loaded = load(&with_parts(parts));
    assert!(!loaded.refused, "{:?}", said(&loaded));
    parts::stylesheet(loaded.theme.first())
}

// ---------------------------------------------------------------------------------------
// The manifest
// ---------------------------------------------------------------------------------------

#[test]
fn the_window_s_families_are_in_the_manifest_at_version_3() {
    let manifest = parts::manifest_json();
    let families = manifest["families"].as_array().unwrap();
    let of = |name: &str| -> (Vec<String>, Vec<String>) {
        let family = families
            .iter()
            .find(|one| one["name"] == name)
            .unwrap_or_else(|| panic!("{name} is in the manifest"));
        assert_eq!(family["since"], 4, "{name}");
        let names = |list: &str| {
            family[list]
                .as_array()
                .unwrap()
                .iter()
                .map(|one| one["name"].as_str().unwrap().to_owned())
                .collect()
        };
        (names("parts"), names("variants"))
    };
    assert_eq!(
        of("panel"),
        (
            vec!["root".into(), "title".into()],
            vec!["side".into(), "content".into(), "bottom".into()]
        )
    );
    assert_eq!(
        of("panel-section"),
        (
            vec!["root".into(), "head".into(), "title".into(), "top".into()],
            vec!["default".into()]
        )
    );
    assert_eq!(
        of("empty-state"),
        (
            vec![
                "root".into(),
                "icon".into(),
                "headline".into(),
                "body".into(),
                "action".into()
            ],
            vec!["page".into(), "panel".into()]
        )
    );
    assert_eq!(
        of("status-item"),
        (
            vec!["root".into(), "icon".into(), "label".into(), "count".into()],
            vec![
                "bar".into(),
                "pill".into(),
                "waiting".into(),
                "accent".into()
            ]
        )
    );
    // None of them carries a consent: no answer is drawn by them.
    for family in families.iter().filter(|one| one["since"] == 3) {
        for variant in family["variants"].as_array().unwrap() {
            assert_eq!(variant["consent"], false, "{family}");
        }
    }
}

// ---------------------------------------------------------------------------------------
// Keys and the stylesheet
// ---------------------------------------------------------------------------------------

#[test]
fn a_theme_rounds_one_panel_variant_and_the_browser_keeps_its_corners() {
    let sheet = sheet_of(json!({
        "panel.root.content.corner-radius": 16,
        "panel.root.fill": "surface.raised",
    }));
    assert!(
        sheet.contains(
            ":where([data-scope=\"panel\"][data-part=\"root\"]) { background-color: var(--surface-raised); --panel-fill: var(--surface-raised); }"
        ),
        "{sheet}"
    );
    // A panel is as tall as what it holds: its corner is clamped to half its own box, never to
    // a control's height.
    assert!(
        sheet.contains(
            ":where([data-scope=\"panel\"][data-part=\"root\"][data-variant=\"content\"]) { border-radius: min(16px, 50%); }"
        ),
        "{sheet}"
    );
}

#[test]
fn a_box_s_corners_are_each_clamped_to_half_its_box() {
    let sheet = sheet_of(json!({ "panel.root.corner-radius": [24, 0, "full", 0] }));
    assert!(
        sheet.contains(
            ":where([data-scope=\"panel\"][data-part=\"root\"]) { border-radius: min(24px, 50%) min(0px, 50%) 50% min(0px, 50%); }"
        ),
        "{sheet}"
    );
}

#[test]
fn a_status_pill_s_corner_is_clamped_to_its_24px() {
    let sheet = sheet_of(json!({ "status-item.root.pill.corner-radius": 20 }));
    assert!(
        sheet.contains(
            ":where([data-scope=\"status-item\"][data-part=\"root\"][data-variant=\"pill\"]) { border-radius: min(20px, calc(var(--purlis-part-height, 24px) / 2)); }"
        ),
        "{sheet}"
    );
}

#[test]
fn a_section_s_title_and_an_empty_state_s_headline_are_reached_inside_their_roots() {
    let sheet = sheet_of(json!({
        "panel-section.title.text": "text.primary",
        "empty-state.headline.font-weight": 500,
    }));
    assert!(
        sheet.contains(
            ":where([data-scope=\"panel-section\"][data-part=\"root\"] [data-scope=\"panel-section\"][data-part=\"title\"]) { color: var(--text-primary); }"
        ),
        "{sheet}"
    );
    assert!(
        sheet.contains(
            ":where([data-scope=\"empty-state\"][data-part=\"root\"] [data-scope=\"empty-state\"][data-part=\"headline\"]) { font-weight: 500; }"
        ),
        "{sheet}"
    );
}

#[test]
fn a_key_a_family_does_not_have_is_said_and_left_out() {
    let loaded = load(&with_parts(json!({
        "panel.root.height": 30,
        "panel.handle.text": "text.primary",
        "status-item.count.font-size": 12,
    })));
    assert!(!loaded.refused, "{:?}", said(&loaded));
    let notes = said(&loaded);
    for key in [
        "panel.root.height",
        "panel.handle.text",
        "status-item.count.font-size",
    ] {
        assert!(
            notes
                .iter()
                .any(|one| one.contains(&format!("parts.{key} is not"))),
            "{key}: {notes:?}"
        );
    }
}

// ---------------------------------------------------------------------------------------
// The least target is a control's
// ---------------------------------------------------------------------------------------

#[test]
fn a_box_is_no_target_and_is_not_held_to_one() {
    // A section's room and an empty state's are bounded, and nothing else: a box grows to hold
    // its words.
    let loaded = load(&with_parts(json!({
        "panel-section.root.padding": 12,
        "panel-section.head.gap": 2,
        "empty-state.root.padding": [12, 32],
    })));
    assert!(!loaded.refused, "{:?}", said(&loaded));
}

#[test]
fn a_status_item_keeps_its_words_in_its_24px() {
    let loaded = load(&with_parts(json!({ "status-item.root.padding": [8, 8] })));
    assert!(loaded.refused, "the item was drawn");
    assert!(
        said(&loaded)
            .iter()
            .any(|one| one.contains("status-item.root.bar")
                && one.contains("more than its 24px height")),
        "{:?}",
        said(&loaded)
    );
    let loaded = load(&with_parts(json!({ "status-item.root.padding": [2, 12] })));
    assert!(!loaded.refused, "{:?}", said(&loaded));
}

#[test]
fn a_size_inside_a_state_is_refused_on_these_families_too() {
    let loaded = load(&with_parts(
        json!({ "status-item.root.pill.hover.padding": 2 }),
    ));
    assert!(loaded.refused);
    assert!(
        said(&loaded)
            .iter()
            .any(|one| one.contains(parts::NO_SIZE_IN_A_STATE)),
        "{:?}",
        said(&loaded)
    );
}

#[test]
fn a_status_pill_whose_words_break_their_floor_draws_its_default() {
    let loaded = load(&with_parts(json!({
        "status-item.root.pill.fill": "text.primary",
        "status-item.root.pill.text": "text.primary",
    })));
    assert!(!loaded.refused, "{:?}", said(&loaded));
    let fell = parts::fallen(loaded.theme.first());
    assert!(
        fell.iter().any(|one| one.part == "status-item.root.pill"),
        "{fell:?}"
    );
}

// ---------------------------------------------------------------------------------------
// The family default is the window's own stylesheet
// ---------------------------------------------------------------------------------------

/// A stylesheet with prettier's wrapping of a long selector read back onto one line, and no
/// comments.
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

/// The declarations of the last rule `selector` opens in `css`: a variant's own, after a rule it
/// shares with its siblings.
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
fn the_panel_s_and_the_section_s_defaults_are_what_panel_css_draws() {
    let css = css_of(include_str!("../../../../app/src/ui/panel/panel.css"));
    let root = rule(&css, "[data-scope=\"panel\"][data-part=\"root\"]");
    // The fill is said again as `--panel-fill`, which a view's top is drawn on (#3340).
    assert_eq!(
        declared(root, "--panel-fill"),
        declared(root, "background-color")
    );
    for variant in ["side", "content", "bottom"] {
        let held = parts::look_json("panel", variant);
        assert_eq!(
            held["fill"],
            token(declared(root, "background-color").unwrap()),
            "{variant}"
        );
        assert_eq!(
            held["border"],
            token(declared(root, "border").unwrap()),
            "{variant}"
        );
    }
    // A view's title, the panel's own part (#3340), in the panel's words' colour.
    let view_title = rule(&css, "[data-scope=\"panel\"][data-part=\"title\"]");
    assert_eq!(
        parts::look_json("panel", "side")["text"],
        token(declared(view_title, "color").unwrap())
    );
    let title = rule(&css, "[data-scope=\"panel-section\"][data-part=\"title\"]");
    let held = parts::look_json("panel-section", "default");
    assert_eq!(held["text"], token(declared(title, "color").unwrap()));
    assert_eq!(held["fill"], "transparent");
}

#[test]
fn the_empty_state_s_default_is_what_empty_state_css_draws() {
    let css = css_of(include_str!(
        "../../../../app/src/ui/empty-state/empty-state.css"
    ));
    let root = rule(&css, "[data-scope=\"empty-state\"][data-part=\"root\"]");
    for variant in ["page", "panel"] {
        let held = parts::look_json("empty-state", variant);
        assert_eq!(
            held["text"],
            token(declared(root, "color").unwrap()),
            "{variant}"
        );
        assert_eq!(held["fill"], "transparent", "{variant}");
    }
}

#[test]
fn the_status_item_s_defaults_are_what_status_css_draws() {
    let css = css_of(include_str!("../../../../app/src/ui/status/status.css"));
    let root = "[data-scope=\"status-item\"][data-part=\"root\"]";
    let base = rule(&css, root);
    let base_hover = rule(&css, &format!("{root}[data-hovered]:not([data-disabled])"));
    for variant in ["bar", "pill", "waiting", "accent"] {
        let own_selector = format!("{root}[data-variant=\"{variant}\"]");
        let own = if css.contains(&format!("\n{own_selector} {{")) {
            rule(&css, &own_selector)
        } else {
            ""
        };
        let hovered = format!("{own_selector}[data-hovered]:not([data-disabled])");
        let own_hover = if css.contains(&format!("\n{hovered} {{")) {
            rule(&css, &hovered)
        } else {
            ""
        };
        let first = |property: &str, layers: &[&str]| -> String {
            token(
                layers
                    .iter()
                    .find_map(|layer| declared(layer, property))
                    .unwrap_or_else(|| panic!("no {property}")),
            )
        };
        let held = parts::look_json("status-item", variant);
        assert_eq!(
            held["fill"],
            first("background-color", &[own, base]),
            "{variant}"
        );
        assert_eq!(held["text"], first("color", &[own, base]), "{variant}");
        assert_eq!(
            held["hover"],
            first("background-color", &[own_hover, base_hover]),
            "{variant}"
        );
    }
}

#[test]
fn the_title_bar_chip_s_defaults_are_what_titlebar_css_draws() {
    let css = css_of(include_str!("../../../../app/src/ui/titlebar/titlebar.css"));
    let root = "[data-part=\"workspace\"][data-scope=\"titlebar\"]";
    let base = rule(&css, root);
    let hovered = rule(&css, &format!("{root}[data-hovered]"));
    let chosen = rule(&css, &format!("{root}[data-selected]"));
    let chosen_hovered = rule(&css, &format!("{root}[data-selected][data-hovered]"));
    let unpinned = rule(&css, &format!("{root}[data-variant=\"unpinned\"]"));
    // A pin: quiet words, no box, until the pointer reaches it or it is current (R101).
    assert_eq!(
        parts::look_of("titlebar", "pinned", &[]),
        [
            "transparent",
            token(declared(base, "color").unwrap()).as_str(),
            "transparent"
        ]
    );
    // Under the pointer the card's fill; chosen the accent's wash, and the stronger wash under
    // the pointer, each with the primary words (R101 as amended by #3345).
    for (states, fill, text) in [
        (&[parts::State::Hover][..], hovered, hovered),
        (&[parts::State::Selected][..], chosen, chosen),
        (
            &[parts::State::Hover, parts::State::Selected][..],
            chosen_hovered,
            chosen,
        ),
    ] {
        assert_eq!(
            parts::look_of("titlebar", "pinned", states),
            [
                token(declared(fill, "background-color").unwrap()).as_str(),
                token(declared(text, "color").unwrap()).as_str(),
                "transparent"
            ],
            "{states:?}"
        );
    }
    assert_ne!(
        parts::look_of("titlebar", "pinned", &[parts::State::Hover])[0],
        parts::look_of("titlebar", "pinned", &[parts::State::Selected])[0],
        "the chosen chip never looks hovered"
    );
    // The dashed chip: muted words, the subtle line (R105).
    let [_, text, border] = parts::look_of("titlebar", "unpinned", &[]);
    assert_eq!(text, token(declared(unpinned, "color").unwrap()));
    assert_eq!(border, token(declared(unpinned, "border-color").unwrap()));
    // It is always current and takes no fill for it, as the prototype draws it; the pointer
    // lights it like any chip.
    let [fill, ..] = parts::look_of("titlebar", "unpinned", &[parts::State::Selected]);
    assert_eq!(fill, token(declared(unpinned, "background-color").unwrap()));
    let unpinned_lit = rule(
        &css,
        &format!("{root}[data-variant=\"unpinned\"][data-hovered]"),
    );
    let [fill, text, _] = parts::look_of("titlebar", "unpinned", &[parts::State::Hover]);
    assert_eq!(
        fill,
        token(declared(unpinned_lit, "background-color").unwrap())
    );
    assert_eq!(text, token(declared(unpinned_lit, "color").unwrap()));
}

#[test]
fn the_plane_root_s_chip_is_the_workspace_chip_s_root_variant_since_next() {
    // A theme keys it as a chip (`titlebar.workspace.root.*`, #3654), new in the next release.
    let manifest = parts::manifest_json();
    let titlebar = manifest["families"]
        .as_array()
        .unwrap()
        .iter()
        .find(|one| one["name"] == "titlebar")
        .unwrap();
    let root = titlebar["variants"]
        .as_array()
        .unwrap()
        .iter()
        .find(|one| one["name"] == "root")
        .expect("the root chip is a variant of the title bar's chip");
    assert_eq!(root["since"], parts::NEXT);
    assert_eq!(root["consent"], false);
    // Drawn as a pin is (titlebar.css): quiet at rest, the card's fill under the pointer, the
    // accent's wash while the plane root is in front, and never looking merely hovered then.
    let css = css_of(include_str!("../../../../app/src/ui/titlebar/titlebar.css"));
    let chip = "[data-part=\"workspace\"][data-scope=\"titlebar\"]";
    for states in [
        &[][..],
        &[parts::State::Hover][..],
        &[parts::State::Selected][..],
        &[parts::State::Hover, parts::State::Selected][..],
    ] {
        assert_eq!(
            parts::look_of("titlebar", "root", states),
            parts::look_of("titlebar", "pinned", states),
            "{states:?}"
        );
    }
    let own = rule(&css, &format!("{chip}[data-variant=\"root\"]"));
    assert!(declared(own, "background-color").is_none());
    assert!(declared(own, "color").is_none());
    // A theme's key for it is read and drawn on the root chip alone.
    let sheet = sheet_of(json!({ "titlebar.workspace.root.selected.fill": "accent.surface" }));
    assert!(
        sheet.contains("[data-variant=\"root\"][data-selected]"),
        "{sheet}"
    );
}

#[test]
fn a_dormant_pin_s_chip_is_the_workspace_chip_s_dormant_variant_since_next() {
    // A theme keys it as a chip (`titlebar.workspace.dormant.*`, #3651), new in the next release.
    let manifest = parts::manifest_json();
    let titlebar = manifest["families"]
        .as_array()
        .unwrap()
        .iter()
        .find(|one| one["name"] == "titlebar")
        .unwrap();
    let dormant = titlebar["variants"]
        .as_array()
        .unwrap()
        .iter()
        .find(|one| one["name"] == "dormant")
        .expect("a dormant pin's chip is a variant of the title bar's chip");
    assert_eq!(dormant["since"], parts::NEXT);
    assert_eq!(dormant["consent"], false);
    // Drawn as titlebar.css draws it: muted words and the subtle line at rest, never the chosen
    // wash (it is never current), the card's fill and the primary words under the pointer.
    let css = css_of(include_str!("../../../../app/src/ui/titlebar/titlebar.css"));
    let chip = "[data-part=\"workspace\"][data-scope=\"titlebar\"]";
    let own = rule(&css, &format!("{chip}[data-variant=\"dormant\"]"));
    let [fill, text, border] = parts::look_of("titlebar", "dormant", &[]);
    assert_eq!(fill, "transparent");
    assert_eq!(text, token(declared(own, "color").unwrap()));
    assert_eq!(border, token(declared(own, "border-color").unwrap()));
    assert_eq!(declared(own, "border-style"), Some("dotted"));
    assert_eq!(
        parts::look_of("titlebar", "dormant", &[parts::State::Selected]),
        parts::look_of("titlebar", "dormant", &[]),
        "never current, so never the chosen wash"
    );
    let lit = rule(&css, &format!("{chip}[data-variant=\"dormant\"][data-hovered]"));
    let [fill, text, _] = parts::look_of("titlebar", "dormant", &[parts::State::Hover]);
    assert_eq!(fill, token(declared(lit, "background-color").unwrap()));
    assert_eq!(text, token(declared(lit, "color").unwrap()));
}

// ---------------------------------------------------------------------------------------
// The window's geometry (#2805)
// ---------------------------------------------------------------------------------------

#[test]
fn purlis_classic_draws_flush_and_purlis_floats_its_panels() {
    for theme in built_ins() {
        for geometry in [&theme.geometry, &theme.compact] {
            assert_eq!(geometry["window.gutter"], 0.0, "Classic {}", theme.id);
            assert_eq!(geometry["panel.corner-radius"], 0.0, "Classic {}", theme.id);
            assert_eq!(geometry["panel.border-width"], 1.0, "Classic {}", theme.id);
        }
    }
    for theme in &purlis().appearances {
        for geometry in [&theme.geometry, &theme.compact] {
            assert_eq!(geometry["window.gutter"], 8.0, "Purlis {}", theme.id);
            assert_eq!(geometry["panel.corner-radius"], 12.0, "Purlis {}", theme.id);
            assert_eq!(geometry["panel.border-width"], 0.0, "Purlis {}", theme.id);
        }
    }
}

#[test]
fn purlis_draws_its_panels_one_step_up_and_classic_as_it_always_did() {
    for theme in &purlis().appearances {
        let sheet = parts::stylesheet(theme);
        assert!(
            sheet.contains(
                ":where([data-scope=\"panel\"][data-part=\"root\"]) { background-color: var(--surface-raised); --panel-fill: var(--surface-raised); }"
            ),
            "Purlis {}: {sheet}",
            theme.id
        );
        assert_eq!(parts::fallen(theme), Vec::new(), "Purlis {}", theme.id);
    }
    // Classic names no panel part: its parts are its tabs' strip look (#2914), its activity
    // items' (#2917) and its resize handles' (#3143).
    for theme in built_ins() {
        let sheet = parts::stylesheet(theme);
        assert!(
            !sheet.contains("data-scope=\"panel\"")
                && !sheet.contains("data-scope=\"panel-section"),
            "Classic {}",
            theme.id
        );
    }
}

#[test]
fn every_built_in_appearance_has_the_amber_wash_its_words_read_on() {
    for theme in built_ins().iter().chain(&purlis().appearances) {
        let wash = theme.value("wash.warning");
        assert!(
            wash.len() == 9 && wash.starts_with('#'),
            "{}: {wash} is a wash",
            theme.id
        );
        let alpha = u8::from_str_radix(&wash[7..9], 16).unwrap();
        assert!(
            (0x19..=0x24).contains(&alpha),
            "{}: the wash is amber at .10 to .14, not {alpha}",
            theme.id
        );
    }
}

#[test]
fn the_window_s_geometry_is_written_in_px_and_held_to_its_bounds() {
    let written = geometry::properties(&geometry::classic());
    assert_eq!(written["--window-gutter"], "0px");
    assert_eq!(written["--panel-corner-radius"], "0px");
    assert_eq!(written["--panel-border-width"], "1px");
    let loaded = load(&json!({
        "name": "Wide",
        "extends": "purlis-classic",
        "appearances": [{
            "id": "dark",
            "appearance": "dark",
            "tokens": { "window.gutter": 17, "panel.corner-radius": 24, "panel.border-width": 0 },
        }],
    }));
    assert!(loaded.refused);
    assert!(
        said(&loaded)
            .iter()
            .any(|one| one.contains("window.gutter") && one.contains("from 0 to 16 px")),
        "{:?}",
        said(&loaded)
    );
}

// ---------------------------------------------------------------------------------------
// A chip's notice dot and a notice card's point (#3648): parts drawn beside their roots
// ---------------------------------------------------------------------------------------

#[test]
fn a_chip_s_notice_dot_is_reached_by_its_own_attributes_not_inside_a_chip() {
    // The dot is the chip's sibling in its slot (`ChipSlot`), so a rule inside a chip would reach
    // nothing: it is drawn around the chip, in no variant and no state of it.
    let sheet = sheet_of(json!({ "titlebar.dot.fill": "accent.base" }));
    assert!(
        sheet.contains(
            ":where([data-scope=\"titlebar\"][data-part=\"dot\"]) { background-color: var(--accent-base); }"
        ),
        "{sheet}"
    );
    let loaded = load(&with_parts(json!({ "titlebar.dot.pinned.hover.fill": "accent.base" })));
    assert!(
        said(&loaded)
            .iter()
            .any(|one| one.contains("parts.titlebar.dot.pinned.hover.fill is not")),
        "{:?}",
        said(&loaded)
    );
}

#[test]
fn each_kind_of_notice_dot_is_a_part_of_its_own() {
    // A warning's dot (#3649) is keyed apart from news's, so a theme colouring one never colours
    // the other the same, and it too is drawn around the chip, in no variant and no state.
    let sheet = sheet_of(json!({
        "titlebar.dot.fill": "accent.base",
        "titlebar.dot-warning.fill": "state.waiting",
    }));
    assert!(
        sheet.contains(
            ":where([data-scope=\"titlebar\"][data-part=\"dot-warning\"]) { background-color: var(--state-waiting); }"
        ),
        "{sheet}"
    );
    assert!(
        sheet.contains(
            ":where([data-scope=\"titlebar\"][data-part=\"dot\"]) { background-color: var(--accent-base); }"
        ),
        "{sheet}"
    );
    let loaded = load(&with_parts(json!({ "titlebar.dot-warning.pinned.fill": "state.waiting" })));
    assert!(
        said(&loaded)
            .iter()
            .any(|one| one.contains("parts.titlebar.dot-warning.pinned.fill is not")),
        "{:?}",
        said(&loaded)
    );
}

#[test]
fn a_notice_card_s_point_is_its_own_part_and_leaves_every_other_popover_s_arrow_alone() {
    // The point is the notice card's (`HungPopover`); the arrow a held popover draws is an SVG of
    // the surface's own ground, no part a theme keys.
    let sheet = sheet_of(json!({ "popover.point.fill": "surface.raised" }));
    assert!(
        sheet.contains(
            ":where([data-scope=\"popover\"][data-part=\"point\"]) { background-color: var(--surface-raised); }"
        ),
        "{sheet}"
    );
    let loaded = load(&with_parts(json!({
        "popover.arrow.fill": "surface.raised",
        "popover.point.popover.fill": "surface.raised",
    })));
    let notes = said(&loaded);
    for key in ["popover.arrow.fill", "popover.point.popover.fill"] {
        assert!(
            notes.iter().any(|one| one.contains(&format!("parts.{key} is not"))),
            "{key}: {notes:?}"
        );
    }
}

// ---------------------------------------------------------------------------------------
// The crumb, the ⌄, and a switcher's rows and pins (#3656): parts drawn apart from every chip
// ---------------------------------------------------------------------------------------

/// The title bar's parts drawn apart from every chip, each with the states its own element is
/// marked in (`ProjectCrumb`, `AllWorkspaces`, `SwitcherPin` and `SwitcherRow`).
const APART: &[(&str, &[&str])] = &[
    ("crumb", &["hover", "selected", "focused"]),
    ("all", &["hover", "selected", "focused"]),
    ("pin", &["hover", "selected", "focused"]),
    ("row", &["hover", "selected"]),
];

/// The attribute the window marks a state by, on the part's own element.
fn marked(state: &str) -> &'static str {
    match state {
        "hover" => "[data-hovered]:not([data-disabled])",
        "selected" => "[data-selected]",
        "focused" => "[data-focus-visible]",
        _ => unreachable!("{state}"),
    }
}

#[test]
fn the_crumb_the_all_button_a_row_and_a_pin_are_reached_by_their_own_attributes() {
    // None is drawn inside a workspace chip, so a rule inside one would reach nothing.
    for (part, _) in APART {
        let sheet = sheet_of(json!({ format!("titlebar.{part}.fill"): "surface.raised" }));
        assert!(
            sheet.contains(&format!(
                ":where([data-scope=\"titlebar\"][data-part=\"{part}\"]) {{ background-color: var(--surface-raised); }}"
            )),
            "{part}: {sheet}"
        );
        assert!(
            !sheet.contains(&format!(
                "[data-part=\"workspace\"] [data-scope=\"titlebar\"][data-part=\"{part}\"]"
            )),
            "{part} is never reached inside a chip: {sheet}"
        );
    }
}

#[test]
fn the_crumb_s_words_reach_the_crumb() {
    // The ticket's own case (#3656): `titlebar.crumb.fill` and its words draw the crumb.
    let sheet = sheet_of(json!({
        "titlebar.crumb.fill": "surface.raised",
        "titlebar.crumb.text": "text.primary",
    }));
    assert!(
        sheet.contains(
            ":where([data-scope=\"titlebar\"][data-part=\"crumb\"]) { background-color: var(--surface-raised); color: var(--text-primary); }"
        ),
        "{sheet}"
    );
}

#[test]
fn each_apart_part_s_state_reaches_the_part_marked_in_it() {
    for (part, states) in APART {
        for state in *states {
            let sheet = sheet_of(json!({
                format!("titlebar.{part}.{state}.fill"): "surface.raised",
            }));
            assert!(
                sheet.contains(&format!(
                    ":where([data-scope=\"titlebar\"][data-part=\"{part}\"]{}) {{ background-color: var(--surface-raised); }}",
                    marked(state)
                )),
                "{part} {state}: {sheet}"
            );
        }
    }
}

#[test]
fn an_apart_part_in_a_chip_s_variant_or_a_state_it_is_never_in_is_said_and_left_out() {
    // No release has published format 2 (v0.4.1 has no theme schema), so the schema never
    // completes these keys, and a theme that writes one is told it reaches nothing.
    let keys = parts::schema()["properties"].clone();
    for key in [
        "titlebar.crumb.pinned.fill",
        "titlebar.all.unpinned.hover.text",
        "titlebar.pin.root.fill",
        "titlebar.row.dormant.corner-radius",
        "titlebar.row.focused.fill",
    ] {
        let loaded = load(&with_parts(json!({ key: "surface.raised" })));
        assert!(!loaded.refused, "{key}: {:?}", said(&loaded));
        assert!(
            said(&loaded)
                .iter()
                .any(|one| one.contains(&format!("parts.{key} reaches nothing"))
                    && one.contains("apart")),
            "{key}: {:?}",
            said(&loaded)
        );
        assert!(
            !parts::stylesheet(loaded.theme.first()).contains("var(--surface-raised)"),
            "{key} draws nothing"
        );
        assert!(keys.get(key).is_none(), "{key} is not completed");
    }
    // Its own keys are.
    for key in [
        "titlebar.crumb.fill",
        "titlebar.crumb.hover.fill",
        "titlebar.row.selected.text",
        "titlebar.pin.focused.fill",
    ] {
        assert!(keys.get(key).is_some(), "{key}");
    }
}

#[test]
fn a_chip_that_falls_leaves_the_crumb_s_colours_as_the_theme_sets_them() {
    // #3946's review: in Purlis dark a faint unpinned chip breaks its floor and draws its
    // default back; the crumb, which broke nothing, keeps the primary words the theme gave it.
    let loaded = load(&json!({
        "name": "Shaped",
        "extends": "purlis",
        "parts": {
            "titlebar.crumb.text": "text.primary",
            "titlebar.workspace.unpinned.text": "text.faint",
        },
    }));
    assert!(!loaded.refused, "{:?}", said(&loaded));
    let dark = loaded
        .theme
        .appearances
        .iter()
        .find(|one| one.id == "dark")
        .expect("Purlis dark");
    let fell = parts::fallen(dark);
    assert!(
        fell.iter()
            .any(|one| one.part == "titlebar.workspace.unpinned"),
        "the chip falls: {fell:?}"
    );
    let sheet = parts::stylesheet(dark);
    let crumb = "[data-scope=\"titlebar\"][data-part=\"crumb\"]";
    assert!(
        sheet.contains(&format!(
            ":where({crumb}) {{ color: var(--text-primary); }}"
        )),
        "{sheet}"
    );
    for line in sheet.lines().filter(|line| line.contains(crumb)) {
        assert!(
            !line.contains("--text-secondary"),
            "the crumb is drawn back: {line}"
        );
    }
}

#[test]
fn a_chip_s_own_parts_are_still_reached_inside_it() {
    // The count is drawn in the chip, in its variants and states.
    let sheet = sheet_of(json!({ "titlebar.count.pinned.hover.fill": "surface.raised" }));
    assert!(
        sheet.contains(
            ":where([data-scope=\"titlebar\"][data-part=\"workspace\"][data-variant=\"pinned\"][data-hovered]:not([data-disabled]) [data-scope=\"titlebar\"][data-part=\"count\"])"
        ),
        "{sheet}"
    );
}

#[test]
fn the_pairs_the_crumb_the_all_button_a_row_and_a_pin_draw_are_held() {
    // What `titlebar.css` draws them in, at rest and in each state, on the title bar and on the
    // switcher's popover: each is a pair the check holds every built-in to
    // (`check_tests::every_built_in_still_clears_the_check`), words at 4.5:1 and marks at 3:1.
    let words = 4.5;
    let mark = 3.0;
    for (front, back, floor) in [
        // The crumb: secondary words on the bar, and on the card's fill under the pointer and
        // while its switcher is open.
        ("text.secondary", "surface.deep", words),
        ("text.secondary", "surface.overlay", words),
        // The ⌄, a mark: quiet on the bar, primary on the card's fill.
        ("text.quiet", "surface.deep", mark),
        ("text.primary", "surface.overlay", mark),
        // A pin, a mark on the popover: faint at rest, primary under the pointer, the accent
        // while pinned.
        ("text.faint", "surface.overlay", mark),
        ("accent.base", "surface.overlay", mark),
        // A row: secondary on the popover, primary on the selected row's fill, faint while it
        // cannot run.
        ("text.secondary", "surface.overlay", words),
        ("text.primary", "list.selected", words),
        ("text.faint", "surface.overlay", mark),
    ] {
        assert!(
            check::PAIRS.iter().any(|pair| pair.front == front
                && pair.back == back
                && pair.under.is_none()
                && pair.floor >= floor),
            "{front} on {back} at {floor}:1 is held"
        );
    }
}

#[test]
fn the_manifest_says_which_parts_are_drawn_apart_from_the_next_version_on() {
    // What version 12 shipped keeps its contract; the next version says it, and the release
    // step carries it into the version it gives the next release.
    let part = |version: u32, name: &str| -> Value {
        parts::manifest_as_of(version)["families"]
            .as_array()
            .unwrap()
            .iter()
            .find(|one| one["name"] == "titlebar")
            .unwrap()["parts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|one| one["name"] == name)
            .unwrap()
            .clone()
    };
    assert_eq!(
        part(parts::VERSION + 1, "crumb")["apart"],
        json!(["hover", "selected", "focused"])
    );
    assert_eq!(part(parts::VERSION + 1, "row")["apart"], json!(["hover", "selected"]));
    assert!(part(parts::VERSION, "crumb").get("apart").is_none());
    assert!(part(parts::VERSION + 1, "count").get("apart").is_none());
    assert!(parts::has_unreleased());
}

/// #3999: the unpinned chip and a dormant one draw no fill of their own at rest, and the
/// unpinned one none while it is current, so their dashed and dotted line is what draws them
/// apart from the bar and says which they are. It is a non-text boundary, held to 3:1 (WCAG
/// 1.4.11) on the bar's ground outside it and on its own fill inside it, in every state, in every
/// built-in appearance. The window's hairline (`border.subtle`) measured 1.38:1 there in Purlis
/// dark; the line is drawn in the chip's muted words, held to 4.5 on both grounds.
#[test]
fn an_edged_chip_s_line_holds_3_to_1_in_every_state_in_every_built_in() {
    use parts::State::{Focused, Hover, Selected};
    let states: [&[parts::State]; 5] = [&[], &[Hover], &[Selected], &[Selected, Hover], &[Focused]];
    for theme in built_ins().iter().chain(&purlis().appearances) {
        for variant in ["unpinned", "dormant"] {
            for held in states {
                let [fill, _, border] = parts::look_of("titlebar", variant, held);
                for ground in ["surface.deep", fill] {
                    if ground == "transparent" {
                        continue;
                    }
                    let ratio = check::contrast(theme.value(border), theme.value(ground));
                    assert!(
                        ratio >= 3.0,
                        "{} {}: the {variant} chip's {border} on {ground} {held:?} is {ratio:.2}",
                        theme.name,
                        theme.id
                    );
                }
            }
        }
    }
}

/// #3999 at the engine's interface: a theme that colours the chip itself has the unpinned and
/// dormant chips' default looks measured (D-3949-3). They hold, so neither draws its default back.
#[test]
fn a_theme_that_colours_the_chip_leaves_the_edged_chips_standing() {
    for base in ["purlis", "purlis-classic"] {
        let loaded = load(&json!({
            "name": "Shaped",
            "extends": base,
            "parts": { "titlebar.workspace.text": "text.secondary" },
        }));
        assert!(!loaded.refused, "{:?}", said(&loaded));
        for appearance in &loaded.theme.appearances {
            let fell: Vec<_> = parts::fallen(appearance)
                .into_iter()
                .filter(|one| one.part.starts_with("titlebar.workspace"))
                .collect();
            assert!(fell.is_empty(), "{base} {}: {fell:?}", appearance.id);
        }
    }
}

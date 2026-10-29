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

#[test]
fn the_notices_item_s_dot_is_reached_by_its_own_attributes_not_inside_an_item() {
    // The status bar's dot (#3653) is the Notices item's sibling over its corner, as a chip's is
    // in its slot: drawn around the item, in no variant and no state of it.
    let sheet = sheet_of(json!({ "status-item.dot.fill": "accent.base" }));
    assert!(
        sheet.contains(
            ":where([data-scope=\"status-item\"][data-part=\"dot\"]) { background-color: var(--accent-base); }"
        ),
        "{sheet}"
    );
    let loaded = load(&with_parts(json!({ "status-item.dot.bar.hover.fill": "accent.base" })));
    assert!(
        said(&loaded)
            .iter()
            .any(|one| one.contains("parts.status-item.dot.bar.hover.fill is not")),
        "{:?}",
        said(&loaded)
    );
}

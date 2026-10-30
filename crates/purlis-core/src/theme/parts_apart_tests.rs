// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The title bar's parts drawn apart from every chip at the floors (#3949), and the switcher
//! row's count as a part of its own (#3947): each measured on the ground it is drawn on, in each
//! of its states, and one that breaks a floor drawing its own default back while the chip keeps
//! what the theme gave it.

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

/// `base`'s appearance `id` with `parts` set, untinted, as read.
fn shaped(base: &str, id: &str, parts: Value) -> ResolvedTheme {
    let loaded = load(&json!({
        "name": "Shaped",
        "extends": base,
        "parts": parts,
        "tint": { "on": false },
    }));
    assert!(!loaded.refused, "{:?}", said(&loaded));
    loaded
        .theme
        .appearances
        .iter()
        .find(|one| one.id == id)
        .unwrap_or_else(|| panic!("{base} {id}"))
        .clone()
}

/// Purlis dark with `parts` set.
fn purlis_dark(parts: Value) -> ResolvedTheme {
    shaped("purlis", "dark", parts)
}

/// Purlis Classic light with `parts` set: its selected row and its popover, and its bar and its
/// popover, are far enough apart that words read on one and not the other.
fn classic_light(parts: Value) -> ResolvedTheme {
    shaped("purlis-classic", "light", parts)
}

fn fell(theme: &ResolvedTheme) -> Vec<String> {
    parts::fallen(theme)
        .into_iter()
        .map(|one| one.part)
        .collect()
}

const CRUMB: &str = "[data-scope=\"titlebar\"][data-part=\"crumb\"]";
const ROW: &str = "[data-scope=\"titlebar\"][data-part=\"row\"]";
/// What a row's keys never reach (#4000): an unavailable row.
const AVAIL: &str = ":not([data-unavailable])";

#[test]
fn a_crumb_whose_words_vanish_under_the_pointer_falls_alone() {
    // Under the pointer the crumb is drawn on the card's fill: words in that fill vanish there.
    let theme = purlis_dark(json!({ "titlebar.crumb.hover.text": "surface.overlay" }));
    let fell = fell(&theme);
    assert_eq!(fell, ["titlebar.crumb.hover"], "only the crumb falls");
    let sheet = parts::stylesheet(&theme);
    assert!(
        sheet.contains(&format!(
            ":where({CRUMB}[data-hovered]:not([data-disabled])) {{ background-color: var(--surface-overlay); color: var(--text-secondary); }}"
        )),
        "{sheet}"
    );
    assert!(
        !sheet.contains("[data-part=\"workspace\"]"),
        "the chip is left as it is: {sheet}"
    );
}

#[test]
fn a_selected_row_is_measured_on_the_selected_fill_it_is_drawn_on() {
    // In Purlis Classic light, `syntax.keyword` reads on the popover (5:1) and not on the
    // selected row's fill (4.1:1): measured on the popover, it would hold.
    let theme = classic_light(json!({ "titlebar.row.selected.text": "syntax.keyword" }));
    assert_eq!(fell(&theme), ["titlebar.row.selected"]);
    // Under the pointer, on the popover, the same words hold, but not on the selected row
    // under the pointer, which they reach too (#4000).
    let theme = classic_light(json!({ "titlebar.row.hover.text": "syntax.keyword" }));
    assert_eq!(fell(&theme), ["titlebar.row.hover+selected"]);
    // And words that read on the selected fill hold there.
    let theme = purlis_dark(json!({ "titlebar.row.selected.text": "text.primary" }));
    assert!(fell(&theme).is_empty(), "{:?}", fell(&theme));
    let sheet = parts::stylesheet(&theme);
    assert!(
        sheet.contains(&format!(
            ":where({ROW}[data-selected]{AVAIL}) {{ color: var(--text-primary); }}"
        )),
        "{sheet}"
    );
}

#[test]
fn an_open_crumb_is_measured_on_the_card_s_fill_not_the_chip_s() {
    // In Purlis Classic light, `icon.red` reads on the card's fill (5:1) and not on the bar
    // (4:1): open, the crumb is drawn on the card's fill, so it holds there; at rest, on the
    // bar, it falls, and only at rest.
    let theme = classic_light(json!({ "titlebar.crumb.selected.text": "icon.red" }));
    assert!(fell(&theme).is_empty(), "{:?}", fell(&theme));
    let theme = classic_light(json!({ "titlebar.crumb.text": "icon.red" }));
    assert_eq!(fell(&theme), ["titlebar.crumb"]);
    assert_eq!(
        parts::apart_look("titlebar", "pin", Some(parts::State::Selected)).words,
        "accent.base"
    );
}

#[test]
fn a_row_that_falls_at_rest_keeps_the_selected_row_s_fill() {
    // #3997's review: a row's words too faint at rest draw the row's default back, and the row
    // Enter would go to keeps its selected fill and words, so the keyboard sees where it is.
    let theme = purlis_dark(json!({ "titlebar.row.text": "text.faint" }));
    assert!(
        fell(&theme).contains(&"titlebar.row".to_owned()),
        "{:?}",
        fell(&theme)
    );
    let sheet = parts::stylesheet(&theme);
    assert!(
        sheet.contains(&format!(
            ":where({ROW}[data-selected]{AVAIL}) {{ background-color: var(--list-selected); color: var(--text-primary); }}"
        )),
        "{sheet}"
    );
    assert!(
        sheet.contains(&format!(
            ":where({ROW}[data-hovered]:not([data-disabled]){AVAIL}) {{ background-color: var(--surface-overlay); color: var(--text-secondary); }}"
        )),
        "{sheet}"
    );
}

#[test]
fn a_failing_chip_and_a_failing_crumb_each_fall_on_their_own() {
    let theme = purlis_dark(json!({
        "titlebar.workspace.unpinned.text": "text.faint",
        "titlebar.crumb.text": "surface.deep",
    }));
    let fell = fell(&theme);
    // The crumb falls at rest, alone of the apart parts; the chip in its own looks.
    assert!(fell.contains(&"titlebar.crumb".to_owned()), "{fell:?}");
    assert!(
        fell.contains(&"titlebar.workspace.unpinned".to_owned()),
        "{fell:?}"
    );
    assert!(
        fell.iter()
            .all(|one| one == "titlebar.crumb" || one.starts_with("titlebar.workspace.")),
        "{fell:?}"
    );
    let sheet = parts::stylesheet(&theme);
    assert!(
        sheet.contains(&format!(
            ":where({CRUMB}) {{ background-color: transparent; color: var(--text-secondary); }}"
        )),
        "{sheet}"
    );
    // Its states draw their own defaults back too: the rest rule's none would cover them.
    for state in ["[data-hovered]:not([data-disabled])", "[data-selected]"] {
        assert!(
            sheet.contains(&format!(
                ":where({CRUMB}{state}) {{ background-color: var(--surface-overlay); color: var(--text-secondary); }}"
            )),
            "{state}: {sheet}"
        );
    }
}

#[test]
fn every_apart_part_s_default_holds_in_every_built_in() {
    // What `titlebar.css` draws them in, measured as a theme's own setting is: a theme that
    // sets each to its default never falls.
    let mut set = serde_json::Map::new();
    for (part, states) in [
        ("crumb", &["", "hover", "selected"][..]),
        ("all", &["", "hover", "selected"][..]),
        ("pin", &["", "hover", "selected"][..]),
        ("row", &["", "hover", "selected"][..]),
        ("row-count", &[""][..]),
    ] {
        for state in states {
            let at = match *state {
                "hover" => Some(parts::State::Hover),
                "selected" => Some(parts::State::Selected),
                _ => None,
            };
            let look = parts::apart_look("titlebar", part, at);
            let key = if state.is_empty() {
                format!("titlebar.{part}.text")
            } else {
                format!("titlebar.{part}.{state}.text")
            };
            set.insert(key, json!(look.words));
        }
    }
    for base in ["purlis", "purlis-classic"] {
        let loaded = load(&json!({
            "name": "Shaped",
            "extends": base,
            "parts": Value::Object(set.clone()),
        }));
        assert!(!loaded.refused, "{:?}", said(&loaded));
        for appearance in &loaded.theme.appearances {
            assert!(
                parts::fallen(appearance).is_empty(),
                "{base} {}: {:?}",
                appearance.id,
                parts::fallen(appearance)
            );
        }
    }
}

// ---------------------------------------------------------------------------------------
// The switcher row's count (#3947)
// ---------------------------------------------------------------------------------------

#[test]
fn a_row_s_count_is_its_own_part_reached_by_its_own_attributes() {
    let theme = purlis_dark(json!({ "titlebar.row-count.fill": "accent.surface", "titlebar.row-count.text": "accent.text" }));
    let sheet = parts::stylesheet(&theme);
    assert!(
        sheet.contains(
            ":where([data-scope=\"titlebar\"][data-part=\"row-count\"]) { background-color: var(--accent-surface); color: var(--accent-text); }"
        ),
        "{sheet}"
    );
    // The chip's count is untouched.
    assert!(!sheet.contains("[data-part=\"count\"]"), "{sheet}");
    // It is in no variant and no state, and comes in the next release.
    let keys = parts::schema()["properties"].clone();
    assert!(keys.get("titlebar.row-count.fill").is_some());
    assert!(keys.get("titlebar.row-count.hover.fill").is_none());
    assert!(keys.get("titlebar.row-count.pinned.text").is_none());
    let family = parts::manifest_json()["families"]
        .as_array()
        .unwrap()
        .iter()
        .find(|one| one["name"] == "titlebar")
        .unwrap()
        .clone();
    let part = family["parts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|one| one["name"] == "row-count")
        .expect("row-count is in the manifest")
        .clone();
    assert_eq!(part["since"], parts::NEXT);
    assert_eq!(part["apart"], json!([]));
}

#[test]
fn a_row_s_count_whose_digits_vanish_on_its_wash_falls_alone() {
    let theme = purlis_dark(json!({ "titlebar.row-count.text": "wash.warning" }));
    assert_eq!(fell(&theme), ["titlebar.row-count"]);
}

#[test]
fn a_row_s_count_is_held_on_its_wash_over_the_popover_and_the_selected_row() {
    let look = parts::apart_look("titlebar", "row-count", None);
    assert_eq!(look.fill, "wash.warning");
    assert_eq!(look.words, "state.waiting");
    assert_eq!(look.under, ["surface.overlay", "list.selected"]);
    for under in look.under {
        assert!(
            check::PAIRS.iter().any(|pair| pair.front == "state.waiting"
                && pair.back == "wash.warning"
                && pair.under == Some(*under)
                && pair.floor >= 4.5),
            "state.waiting on wash.warning over {under} is held"
        );
    }
}

#[test]
fn a_part_that_falls_at_rest_keeps_what_the_theme_sets_in_a_state() {
    // The open crumb's own words the theme gives it read on the card's fill, and stay; only
    // its fill is drawn back with the rest.
    let theme = purlis_dark(json!({
        "titlebar.crumb.text": "surface.deep",
        "titlebar.crumb.selected.text": "text.primary",
    }));
    assert_eq!(fell(&theme), ["titlebar.crumb"]);
    let sheet = parts::stylesheet(&theme);
    assert!(
        sheet.contains(&format!(
            ":where({CRUMB}[data-selected]) {{ background-color: var(--surface-overlay); }}"
        )),
        "{sheet}"
    );
    assert!(
        !sheet.contains(&format!(
            ":where({CRUMB}[data-selected]) {{ background-color: var(--surface-overlay); color:"
        )),
        "{sheet}"
    );
}

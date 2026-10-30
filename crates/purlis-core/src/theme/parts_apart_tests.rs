// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The title bar's parts drawn apart from every chip at the floors (#3949): each measured on the ground it is drawn on, in each
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

/// Purlis dark with `parts` set, as read.
fn purlis_dark(parts: Value) -> ResolvedTheme {
    let loaded = load(&json!({
        "name": "Shaped",
        "extends": "purlis",
        "parts": parts,
        "tint": { "on": false },
    }));
    assert!(!loaded.refused, "{:?}", said(&loaded));
    loaded
        .theme
        .appearances
        .iter()
        .find(|one| one.id == "dark")
        .expect("Purlis dark")
        .clone()
}

fn fell(theme: &ResolvedTheme) -> Vec<String> {
    parts::fallen(theme)
        .into_iter()
        .map(|one| one.part)
        .collect()
}

const CRUMB: &str = "[data-scope=\"titlebar\"][data-part=\"crumb\"]";
const ROW: &str = "[data-scope=\"titlebar\"][data-part=\"row\"]";

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
    // The row Enter would go to is drawn on `list.selected`, not the popover's surface.
    let theme = purlis_dark(json!({ "titlebar.row.selected.text": "list.selected" }));
    assert_eq!(fell(&theme), ["titlebar.row.selected"]);
    // And words that read there hold.
    let theme = purlis_dark(json!({ "titlebar.row.selected.text": "text.primary" }));
    assert!(fell(&theme).is_empty(), "{:?}", fell(&theme));
    let sheet = parts::stylesheet(&theme);
    assert!(
        sheet.contains(&format!(
            ":where({ROW}[data-selected]) {{ color: var(--text-primary); }}"
        )),
        "{sheet}"
    );
}

#[test]
fn an_open_crumb_is_measured_on_the_card_s_fill_not_the_chip_s() {
    // Open, the crumb is drawn on `surface.overlay`; its words on that ground hold, whatever
    // the chosen chip's accent wash would make of them.
    let ground = |state: Option<parts::State>| parts::apart_look("titlebar", "crumb", state);
    assert_eq!(ground(None).under, ["surface.deep"]);
    assert_eq!(ground(Some(parts::State::Selected)).fill, "surface.overlay");
    assert_eq!(ground(Some(parts::State::Hover)).fill, "surface.overlay");
    assert_eq!(
        parts::apart_look("titlebar", "row", Some(parts::State::Selected)).fill,
        "list.selected"
    );
    assert_eq!(
        parts::apart_look("titlebar", "pin", Some(parts::State::Selected)).words,
        "accent.base"
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

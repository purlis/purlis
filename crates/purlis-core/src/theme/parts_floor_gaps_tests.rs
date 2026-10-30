// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The title bar's floor gaps (#4000), found by #3997's review: the pin and the row measured in
//! the states they are in at once, a row's count held on a row's fill a theme sets, an
//! unavailable row kept in its faint tier, the switcher's rows and pins held on a popover fill a
//! theme sets, and a chip that falls at rest keeping its states' own fills.

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

fn purlis_dark(parts: Value) -> ResolvedTheme {
    shaped("purlis", "dark", parts)
}

fn fell(theme: &ResolvedTheme) -> Vec<String> {
    parts::fallen(theme)
        .into_iter()
        .map(|one| one.part)
        .collect()
}

const PIN: &str = "[data-scope=\"titlebar\"][data-part=\"pin\"]";
const ROW: &str = "[data-scope=\"titlebar\"][data-part=\"row\"]";
const CHIP: &str = "[data-scope=\"titlebar\"][data-part=\"workspace\"]";
const HOVER: &str = "[data-hovered]:not([data-disabled])";

// ---------------------------------------------------------------------------------------
// 1. States that hold at once
// ---------------------------------------------------------------------------------------

#[test]
fn a_pinned_pin_under_the_pointer_is_measured_on_the_hover_fill() {
    // `accent.fill` under the pointer holds the primary mark (4.4:1), but a pinned pin under
    // the pointer draws the accent on it (2.3:1), which no state alone draws.
    let theme = purlis_dark(json!({ "titlebar.pin.hover.fill": "accent.fill" }));
    assert_eq!(fell(&theme), ["titlebar.pin.hover+selected"]);
    let sheet = parts::stylesheet(&theme);
    // Each state of the pair draws its own default back, after the theme's.
    assert!(
        sheet.contains(&format!(
            ":where({PIN}{HOVER}) {{ background-color: var(--surface-overlay); color: var(--text-primary); }}"
        )),
        "{sheet}"
    );
}

#[test]
fn a_selected_row_under_the_pointer_is_measured_on_what_both_draw() {
    // The row's hover fill the theme sets is under the selected row's words while both hold.
    let theme = purlis_dark(json!({
        "titlebar.row.hover.fill": "border.strong",
        "titlebar.row.selected.text": "text.muted",
    }));
    assert_eq!(fell(&theme), ["titlebar.row.hover+selected"]);
}

// ---------------------------------------------------------------------------------------
// 2. A row's count on a row's fill
// ---------------------------------------------------------------------------------------

#[test]
fn a_row_s_count_is_held_on_the_row_fill_a_theme_sets() {
    // An amber row: its own words read on it, but the count's amber on its wash does not, and
    // neither does the count's default, so the row draws its own fill back.
    let theme = purlis_dark(json!({
        "titlebar.row.fill": "state.waiting",
        "titlebar.row.text": "surface.base",
    }));
    assert_eq!(fell(&theme), ["titlebar.row"]);
    let sheet = parts::stylesheet(&theme);
    assert!(
        sheet.contains(&format!(
            ":where({ROW}:not([data-unavailable])) {{ background-color: transparent; color: var(--text-secondary); }}"
        )),
        "{sheet}"
    );
}

#[test]
fn a_row_fill_the_count_reads_on_holds() {
    let theme = purlis_dark(json!({ "titlebar.row.hover.fill": "surface.raised" }));
    assert!(fell(&theme).is_empty(), "{:?}", fell(&theme));
}

// ---------------------------------------------------------------------------------------
// 3. An unavailable row
// ---------------------------------------------------------------------------------------

#[test]
fn a_theme_s_row_keys_never_reach_an_unavailable_row() {
    // It stays in the faint tier `titlebar.css` draws it in, which the check holds as a mark on
    // the popover (`text.faint` on `surface.overlay`, 3:1), whatever the rows' colours.
    let theme = purlis_dark(json!({
        "titlebar.row.text": "text.primary",
        "titlebar.row.hover.fill": "surface.raised",
    }));
    let sheet = parts::stylesheet(&theme);
    assert!(
        sheet.contains(&format!(
            ":where({ROW}:not([data-unavailable])) {{ color: var(--text-primary); }}"
        )),
        "{sheet}"
    );
    assert!(
        sheet.contains(&format!(
            ":where({ROW}{HOVER}:not([data-unavailable])) {{ background-color: var(--surface-raised); }}"
        )),
        "{sheet}"
    );
    assert!(check::PAIRS.iter().any(|pair| pair.front == "text.faint"
        && pair.back == "surface.overlay"
        && pair.under.is_none()
        && pair.floor >= 3.0));
}

// ---------------------------------------------------------------------------------------
// 4. A popover's fill a theme sets, under the switcher's rows and pins
// ---------------------------------------------------------------------------------------

#[test]
fn a_popover_fill_the_rows_cannot_be_read_on_draws_the_popover_back() {
    // In Purlis Classic dark, a row's count, amber on its wash, cannot be read over
    // `border.subtle` (4.3:1): a popover filled with it is what a title bar's part breaks on.
    let theme = shaped("purlis-classic", "dark", json!({}));
    let (family, floor, ratio) = parts::apart_breaks_on(&theme, "surface.overlay", "border.subtle")
        .expect("a row's count breaks on it");
    assert_eq!(family, "titlebar");
    assert!(floor.starts_with("the row-count's words"), "{floor}");
    assert!(ratio < 4.5, "{ratio}");
    // The popover's own default fill holds them all.
    assert!(
        parts::apart_breaks_on(&theme, "surface.overlay", "surface.overlay").is_none()
    );
    // And a theme that sets it draws the popover back.
    let theme = shaped(
        "purlis-classic",
        "dark",
        json!({ "popover.root.fill": "border.subtle" }),
    );
    assert!(
        fell(&theme).iter().any(|one| one.starts_with("popover.root")),
        "{:?}",
        fell(&theme)
    );
}

// ---------------------------------------------------------------------------------------
// 5. A chip that falls at rest
// ---------------------------------------------------------------------------------------

#[test]
fn a_chip_that_falls_at_rest_keeps_its_states_fills() {
    // In Purlis Classic light, `icon.red` reads on the card's fill (5:1) and not on the bar
    // (4:1): a pinned chip's words in it fall at rest, and the chip under the pointer keeps the
    // card's fill `titlebar.css` gives it, rather than the rest's none.
    let theme = shaped(
        "purlis-classic",
        "light",
        json!({ "titlebar.workspace.pinned.text": "icon.red" }),
    );
    assert!(
        fell(&theme).contains(&"titlebar.workspace.pinned".to_owned()),
        "{:?}",
        fell(&theme)
    );
    let sheet = parts::stylesheet(&theme);
    assert!(
        sheet.contains(&format!(
            ":where({CHIP}[data-variant=\"pinned\"]{HOVER}) {{ background-color: var(--surface-overlay);"
        )),
        "{sheet}"
    );
    assert!(
        sheet.contains(&format!(
            ":where({CHIP}[data-variant=\"pinned\"][data-selected]) {{ background-color: var(--accent-wash);"
        )),
        "{sheet}"
    );
}

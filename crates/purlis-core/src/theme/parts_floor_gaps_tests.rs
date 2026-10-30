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
    // The pair draws its own look back, keyed on both states: a pinned pin under the pointer
    // keeps the card's fill with the accent, as `titlebar.css` draws it.
    assert!(
        sheet.contains(&format!(
            ":where({PIN}{HOVER}[data-selected]) {{ background-color: var(--surface-overlay); color: var(--accent-base); }}"
        )),
        "{sheet}"
    );
    // And nothing it writes for one state alone covers the other.
    assert!(
        !sheet.contains(&format!(":where({PIN}[data-selected]) {{ background-color")),
        "{sheet}"
    );
}

#[test]
fn a_selected_row_under_the_pointer_is_measured_on_what_both_draw() {
    // The row's hover fill the theme sets is under the selected row's words while both hold:
    // the window's ground under the pointer, the primary words on the selected row, each reads
    // alone, but not the primary words on the accent.
    let theme = purlis_dark(json!({
        "titlebar.row.hover.fill": "accent.base",
        "titlebar.row.hover.text": "surface.base",
        "titlebar.row.selected.text": "text.primary",
    }));
    let fallen = fell(&theme);
    assert!(
        fallen.contains(&"titlebar.row.hover+selected".to_owned()),
        "{fallen:?}"
    );
    // The selected row alone, and the row under the pointer alone, hold their words; the
    // accent under the pointer is too bright for a row's amber count, which the row draws back
    // its fill for.
    assert!(
        !fallen.contains(&"titlebar.row.selected".to_owned()),
        "{fallen:?}"
    );
    assert!(fallen.iter().all(|one| one.starts_with("titlebar.row.hover")), "{fallen:?}");
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
    let (family, floor, ratio) =
        parts::apart_breaks_on(&theme, "popover", "popover", "border.subtle")
            .expect("a row's count breaks on it");
    assert_eq!(family, "titlebar");
    assert!(floor.starts_with("the row-count's words"), "{floor}");
    assert!(ratio < 4.5, "{ratio}");
    // The popover's own default fill holds them all, and no other surface holds the switcher.
    assert!(parts::apart_breaks_on(&theme, "popover", "popover", "surface.overlay").is_none());
    assert!(parts::apart_breaks_on(&theme, "tooltip", "", "border.subtle").is_none());
    assert!(parts::apart_breaks_on(&theme, "popover", "menu", "border.subtle").is_none());
    // A theme that sets it draws the popover back for what the title bar draws on it alone: in
    // Purlis Classic light, over `layer.divider`, the popover's own words, its menus' and its
    // buttons' read, and a row's count does not.
    let theme = shaped(
        "purlis-classic",
        "light",
        json!({ "popover.root.popover.fill": "layer.divider" }),
    );
    let fell = parts::fallen(&theme);
    assert!(
        fell.iter().any(|one| one.part == "popover.root.popover"
            && one.floor.starts_with("what a titlebar draws on it")),
        "{fell:?}"
    );
}

#[test]
fn a_surface_the_switcher_is_never_drawn_on_never_falls_for_it() {
    // A tooltip's fill is no ground of a switcher's row (#4039's review).
    let theme = shaped(
        "purlis-classic",
        "dark",
        json!({ "tooltip.root.fill": "border.subtle" }),
    );
    let fell = parts::fallen(&theme);
    assert!(
        !fell
            .iter()
            .any(|one| one.floor.starts_with("what a titlebar draws on it")),
        "{fell:?}"
    );
}

#[test]
fn a_see_through_popover_fill_is_measured_as_it_is_laid() {
    // `wash.warning` is a wash: laid over the popover's grounds, a pin's faint mark reads on it
    // (3.4:1 and over), where on the amber alone it would not (2.3:1).
    let theme = shaped(
        "purlis",
        "light",
        json!({ "popover.root.popover.fill": "wash.warning" }),
    );
    assert!(
        parts::apart_breaks_on(&theme, "popover", "popover", "wash.warning").is_none(),
        "{:?}",
        parts::apart_breaks_on(&theme, "popover", "popover", "wash.warning")
    );
    let fell = parts::fallen(&theme);
    assert!(
        !fell
            .iter()
            .any(|one| one.floor.starts_with("what a titlebar draws on it")),
        "{fell:?}"
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
    // The current chip under the pointer takes the stronger wash, keyed on both states, and a
    // chip reached from the keyboard is drawn as it is otherwise: its ring changes no colour.
    assert!(
        sheet.contains(&format!(
            ":where({CHIP}[data-variant=\"pinned\"]{HOVER}[data-selected]) {{ background-color: var(--accent-wash-hover); }}"
        )),
        "{sheet}"
    );
    assert!(
        !sheet.contains(&format!("{CHIP}[data-variant=\"pinned\"][data-focus-visible]")),
        "{sheet}"
    );
}

#[test]
fn a_state_that_falls_after_its_rest_did_draws_its_whole_default_back() {
    // #4039's review, B1: the pin falls at rest, its hover kept the theme's amber fill, and
    // the hover's primary mark on that amber falls in turn: its fill is drawn back too.
    let theme = purlis_dark(json!({
        "titlebar.pin.text": "surface.base",
        "titlebar.pin.hover.fill": "state.waiting",
    }));
    let fallen = fell(&theme);
    assert!(fallen.contains(&"titlebar.pin.hover".to_owned()), "{fallen:?}");
    let sheet = parts::stylesheet(&theme);
    assert!(
        sheet.contains(&format!(
            ":where({PIN}{HOVER}) {{ background-color: var(--surface-overlay);"
        )),
        "{sheet}"
    );
    // And a chip the same.
    let theme = purlis_dark(json!({
        "titlebar.workspace.text": "surface.base",
        "titlebar.workspace.hover.fill": "state.waiting",
    }));
    let fallen = fell(&theme);
    assert!(
        fallen
            .iter()
            .any(|one| one.starts_with("titlebar.workspace.") && one.ends_with(".hover")),
        "{fallen:?}"
    );
    let sheet = parts::stylesheet(&theme);
    assert!(
        sheet.contains(&format!(
            ":where({CHIP}[data-variant=\"pinned\"]{HOVER}) {{ background-color: var(--surface-overlay);"
        )),
        "{sheet}"
    );
}



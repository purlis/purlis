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
    // The pair falls, and the accent under the pointer is too bright for a row's amber count,
    // which the row draws its fill back for under the pointer.
    assert_eq!(
        fell(&theme),
        ["titlebar.row.hover+selected", "titlebar.row.hover"]
    );
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



// ---------------------------------------------------------------------------------------
// 6. What a pair of states draws after one of them falls (#4039's second review)
// ---------------------------------------------------------------------------------------

/// The colours the parts layer draws on an element of the title bar: the last declaration of
/// each property among the rules whose selector its attributes match, as the stylesheet orders
/// them. `None` where no rule of the layer sets it, so `titlebar.css` draws it.
fn drawn_on(sheet: &str, part: &str, attributes: &[&str]) -> std::collections::BTreeMap<String, String> {
    let mut drawn = std::collections::BTreeMap::new();
    let has = |token: &str| -> bool {
        let name = token.split('=').next().unwrap_or(token);
        match token.split_once('=') {
            Some(_) => attributes.contains(&token),
            None => attributes.iter().any(|one| one.split('=').next() == Some(name)),
        }
    };
    for line in sheet.lines().map(str::trim) {
        let Some(rest) = line.strip_prefix(":where(") else {
            continue;
        };
        let Some((selector, body)) = rest.split_once(") {") else {
            continue;
        };
        if selector.contains(' ') || selector.contains(',') {
            continue;
        }
        if !selector.starts_with(&format!("[data-scope=\"titlebar\"][data-part=\"{part}\"]")) {
            continue;
        }
        let mut matched = true;
        let mut at = 0;
        while let Some(open) = selector[at..].find('[') {
            let start = at + open;
            let end = start + selector[start..].find(']').unwrap();
            let token = &selector[start + 1..end];
            let negated = selector[..start].ends_with(":not(");
            if !token.starts_with("data-scope") && !token.starts_with("data-part=") {
                let wanted = token.replace('"', "");
                if has(&wanted) == negated {
                    matched = false;
                }
            }
            at = end + 1;
        }
        if !matched {
            continue;
        }
        for declaration in body.trim_end_matches('}').split(';') {
            if let Some((property, value)) = declaration.trim().split_once(": ") {
                drawn.insert(property.to_owned(), value.to_owned());
            }
        }
    }
    drawn
}

fn fill_of(drawn: &std::collections::BTreeMap<String, String>) -> Option<&str> {
    drawn.get("background-color").map(String::as_str)
}

fn words_of(drawn: &std::collections::BTreeMap<String, String>) -> Option<&str> {
    drawn.get("color").map(String::as_str)
}

const PIN_PAIR: &[&str] = &["data-hovered", "data-selected"];

#[test]
fn a_pinned_pin_under_the_pointer_keeps_its_accent_after_its_hover_falls() {
    for parts in [
        json!({ "titlebar.pin.hover.fill": "state.waiting" }),
        json!({ "titlebar.pin.text": "surface.base", "titlebar.pin.hover.fill": "state.waiting" }),
    ] {
        let theme = purlis_dark(parts.clone());
        let sheet = parts::stylesheet(&theme);
        let drawn = drawn_on(&sheet, "pin", PIN_PAIR);
        assert_eq!(fill_of(&drawn), Some("var(--surface-overlay)"), "{parts}: {sheet}");
        assert_eq!(words_of(&drawn), Some("var(--accent-base)"), "{parts}: {sheet}");
    }
}

#[test]
fn a_current_chip_under_the_pointer_keeps_the_stronger_wash_after_its_hover_falls() {
    let theme = purlis_dark(json!({ "titlebar.workspace.hover.fill": "state.waiting" }));
    let sheet = parts::stylesheet(&theme);
    let drawn = drawn_on(
        &sheet,
        "workspace",
        &["data-variant=pinned", "data-hovered", "data-selected"],
    );
    assert_eq!(fill_of(&drawn), Some("var(--accent-wash-hover)"), "{sheet}");
    // Its words are `titlebar.css`'s primary: the hover's fall writes back only what the rules
    // drew otherwise there, and the theme set no words (#4196's review).
    assert_eq!(words_of(&drawn), None, "{sheet}");
}

#[test]
fn the_row_enter_would_go_to_keeps_its_fill_after_the_row_s_hover_falls() {
    let theme = purlis_dark(json!({ "titlebar.row.hover.fill": "text.primary" }));
    assert!(
        fell(&theme).contains(&"titlebar.row.hover".to_owned()),
        "{:?}",
        fell(&theme)
    );
    let sheet = parts::stylesheet(&theme);
    let drawn = drawn_on(&sheet, "row", &["data-hovered", "data-selected"]);
    assert_eq!(fill_of(&drawn), Some("var(--list-selected)"), "{sheet}");
    assert_eq!(words_of(&drawn), Some("var(--text-primary)"), "{sheet}");
}

#[test]
fn a_current_chip_reached_from_the_keyboard_keeps_its_wash_after_its_focus_falls() {
    let theme = purlis_dark(json!({ "titlebar.workspace.focused.text": "surface.deep" }));
    assert!(
        fell(&theme).iter().any(|one| one.ends_with(".focused")),
        "{:?}",
        fell(&theme)
    );
    let sheet = parts::stylesheet(&theme);
    let current = drawn_on(
        &sheet,
        "workspace",
        &["data-variant=pinned", "data-focus-visible", "data-selected"],
    );
    // Its fill is the family's own (`titlebar.css`'s wash): a focus that falls writes back only
    // what it changes from the rest, which is no fill (#4174).
    assert_eq!(fill_of(&current), None, "{sheet}");
    assert_eq!(words_of(&current), Some("var(--text-primary)"), "{sheet}");
    // A chip at rest reached from the keyboard draws no fill of its own.
    let other = drawn_on(&sheet, "workspace", &["data-variant=pinned", "data-focus-visible"]);
    assert!(
        fill_of(&other).is_none_or(|fill| fill == "transparent"),
        "{sheet}"
    );
}

// ---------------------------------------------------------------------------------------
// 7. A failing pair falls only where both states hold (#4049)
// ---------------------------------------------------------------------------------------

#[test]
fn a_pin_s_failing_pair_leaves_a_plain_hovered_pin_the_theme_s_fill() {
    // `accent.fill` under the pointer holds the primary mark, not the accent of a pinned pin.
    let theme = purlis_dark(json!({ "titlebar.pin.hover.fill": "accent.fill" }));
    assert_eq!(fell(&theme), ["titlebar.pin.hover+selected"]);
    let sheet = parts::stylesheet(&theme);
    let hovered = drawn_on(&sheet, "pin", &["data-hovered"]);
    assert_eq!(fill_of(&hovered), Some("var(--accent-fill)"), "{sheet}");
    let both = drawn_on(&sheet, "pin", PIN_PAIR);
    assert_eq!(fill_of(&both), Some("var(--surface-overlay)"), "{sheet}");
    assert_eq!(words_of(&both), Some("var(--accent-base)"), "{sheet}");
}

#[test]
fn a_chip_s_failing_pair_leaves_a_plain_hovered_chip_the_theme_s_fill() {
    // Under the pointer `border.strong` holds the primary words, and a current chip's accent
    // words hold on its wash, but not the accent on `border.strong`, which a current chip
    // under the pointer draws.
    let theme = purlis_dark(json!({
        "titlebar.workspace.pinned.hover.fill": "border.strong",
        "titlebar.workspace.pinned.selected.text": "accent.base",
    }));
    assert!(
        fell(&theme).contains(&"titlebar.workspace.pinned.hover+selected".to_owned()),
        "{:?}",
        fell(&theme)
    );
    let sheet = parts::stylesheet(&theme);
    let hovered = drawn_on(&sheet, "workspace", &["data-variant=pinned", "data-hovered"]);
    assert_eq!(fill_of(&hovered), Some("var(--border-strong)"), "{sheet}");
    let current = drawn_on(&sheet, "workspace", &["data-variant=pinned", "data-selected"]);
    assert_eq!(words_of(&current), Some("var(--accent-base)"), "{sheet}");
    let both = drawn_on(
        &sheet,
        "workspace",
        &["data-variant=pinned", "data-hovered", "data-selected"],
    );
    assert_eq!(fill_of(&both), Some("var(--accent-wash-hover)"), "{sheet}");
    assert_eq!(words_of(&both), Some("var(--text-primary)"), "{sheet}");
}

// ---------------------------------------------------------------------------------------
// 8. A see-through row fill over a popover fill a theme sets (#4064)
// ---------------------------------------------------------------------------------------

#[test]
fn a_see_through_row_fill_is_measured_over_the_popover_fill_a_theme_sets() {
    // In Purlis Classic light, `icon.pink` reads on `diff.inserted` laid over the popover's own
    // surface (5.1:1), and not laid over `tab.active` (4:1), which the popover holds as its fill.
    let rows = json!({
        "titlebar.row.fill": "diff.inserted",
        "titlebar.row.text": "icon.pink",
    });
    let theme = shaped("purlis-classic", "light", rows.clone());
    assert!(
        !fell(&theme).iter().any(|one| one.starts_with("titlebar.")),
        "{:?}",
        fell(&theme)
    );
    let mut both = rows;
    both["popover.root.popover.fill"] = json!("tab.active");
    let theme = shaped("purlis-classic", "light", both);
    let fallen = fell(&theme);
    assert!(fallen.contains(&"titlebar.row".to_owned()), "{fallen:?}");
    assert!(
        !fallen.iter().any(|one| one.starts_with("popover.")),
        "{fallen:?}"
    );
}

/// [`drawn_on`] for a part drawn inside the chip: the rules that reach `part` inside a
/// `workspace` chip whose attributes are `chip`.
fn drawn_in_chip(
    sheet: &str,
    chip: &[&str],
    part: &str,
) -> std::collections::BTreeMap<String, String> {
    let mut drawn = std::collections::BTreeMap::new();
    let inner = format!("[data-scope=\"titlebar\"][data-part=\"{part}\"]");
    for line in sheet.lines().map(str::trim) {
        let Some(rest) = line.strip_prefix(":where(") else {
            continue;
        };
        let Some((selector, body)) = rest.split_once(") {") else {
            continue;
        };
        let Some((outer, tail)) = selector.split_once(' ') else {
            continue;
        };
        if !tail.starts_with(&inner) || selector.contains(',') {
            continue;
        }
        // The chip's own attributes, matched as [`drawn_on`] matches an element's.
        let line = format!(":where({outer}) {{{body}");
        if drawn_on(&line, "workspace", chip).is_empty() {
            continue;
        }
        for declaration in body.trim_end_matches('}').split(';') {
            if let Some((property, value)) = declaration.trim().split_once(": ") {
                drawn.insert(property.to_owned(), value.to_owned());
            }
        }
    }
    drawn
}

#[test]
fn a_chip_s_failing_pair_draws_its_count_back_where_both_hold() {
    // In Purlis light the amber reads on `border.subtle` under the pointer, and `accent.fill`
    // reads on the count's wash while current, but not `accent.fill` on `border.subtle`, which
    // the count of a current chip under the pointer draws (#4084's review).
    let theme = shaped(
        "purlis",
        "light",
        json!({
            "titlebar.count.pinned.hover.fill": "border.subtle",
            "titlebar.count.pinned.selected.text": "accent.fill",
        }),
    );
    assert!(
        fell(&theme).contains(&"titlebar.workspace.pinned.hover+selected".to_owned()),
        "{:?}",
        fell(&theme)
    );
    let sheet = parts::stylesheet(&theme);
    let both = drawn_in_chip(
        &sheet,
        &["data-variant=pinned", "data-hovered", "data-selected"],
        "count",
    );
    assert_eq!(fill_of(&both), Some("var(--wash-warning)"), "{sheet}");
    assert_eq!(words_of(&both), Some("var(--state-waiting)"), "{sheet}");
    // Each state alone keeps the theme's.
    let hovered = drawn_in_chip(&sheet, &["data-variant=pinned", "data-hovered"], "count");
    assert_eq!(fill_of(&hovered), Some("var(--border-subtle)"), "{sheet}");
    let current = drawn_in_chip(&sheet, &["data-variant=pinned", "data-selected"], "count");
    assert_eq!(words_of(&current), Some("var(--accent-fill)"), "{sheet}");
}

#[test]
fn a_row_s_count_is_held_over_the_popover_fill_a_theme_sets() {
    // In Purlis Classic light, a row's count on `diff.inserted` reads over the popover's own
    // surface (5.1:1) and not over `tab.active` (4:1); the row's own words read on both, so
    // the row draws its fill back for its count alone.
    let rows = json!({ "titlebar.row.fill": "diff.inserted" });
    let theme = shaped("purlis-classic", "light", rows.clone());
    assert!(
        !fell(&theme).iter().any(|one| one.starts_with("titlebar.")),
        "{:?}",
        fell(&theme)
    );
    let mut both = rows;
    both["popover.root.popover.fill"] = json!("tab.active");
    let theme = shaped("purlis-classic", "light", both);
    assert_eq!(fell(&theme), ["titlebar.row"]);
}

#[test]
fn a_row_s_failing_pair_leaves_a_plain_hovered_row_the_theme_s_colours() {
    // #4049's own case: under the pointer the row's own colours read (the window's ground on a
    // light grey), and the row Enter would go to its primary words on the selected fill; but
    // not those words on the grey, which that row under the pointer draws. The count stands on
    // a ground of its own, so it reads on the grey too.
    let theme = purlis_dark(json!({
        "titlebar.row.hover.fill": "text.secondary",
        "titlebar.row.hover.text": "surface.base",
        "titlebar.row.selected.text": "text.primary",
        "titlebar.row-count.fill": "surface.base",
        "titlebar.row-count.text": "text.primary",
    }));
    assert_eq!(fell(&theme), ["titlebar.row.hover+selected"]);
    let sheet = parts::stylesheet(&theme);
    let hovered = drawn_on(&sheet, "row", &["data-hovered"]);
    assert_eq!(fill_of(&hovered), Some("var(--text-secondary)"), "{sheet}");
    assert_eq!(words_of(&hovered), Some("var(--surface-base)"), "{sheet}");
    let both = drawn_on(&sheet, "row", &["data-hovered", "data-selected"]);
    assert_eq!(fill_of(&both), Some("var(--list-selected)"), "{sheet}");
}

// ---------------------------------------------------------------------------------------
// 9. Three states at once (#4086)
// ---------------------------------------------------------------------------------------

#[test]
fn a_current_hovered_focused_chip_falls_only_where_all_three_hold() {
    // In Purlis light, `text.muted` reads on the bar, on the card's fill and on the accent's
    // wash, so on a chip reached from the keyboard alone, under the pointer or current; but
    // not on the stronger wash a current chip under the pointer takes (4.3:1).
    let theme = shaped(
        "purlis",
        "light",
        json!({ "titlebar.workspace.pinned.focused.text": "text.muted" }),
    );
    assert_eq!(
        fell(&theme),
        ["titlebar.workspace.pinned.hover+selected+focused"]
    );
    let sheet = parts::stylesheet(&theme);
    let chip = |states: &[&str]| {
        let mut attributes = vec!["data-variant=pinned"];
        attributes.extend_from_slice(states);
        drawn_on(&sheet, "workspace", &attributes)
    };
    // A plain focused chip, and a focused one current or under the pointer, keep the theme's.
    for states in [
        &["data-focus-visible"][..],
        &["data-focus-visible", "data-selected"][..],
        &["data-focus-visible", "data-hovered"][..],
    ] {
        assert_eq!(
            words_of(&chip(states)),
            Some("var(--text-muted)"),
            "{states:?}: {sheet}"
        );
    }
    // All three draw the pair's own look: the stronger wash, the primary words.
    let all = chip(&["data-focus-visible", "data-hovered", "data-selected"]);
    assert_eq!(fill_of(&all), Some("var(--accent-wash-hover)"), "{sheet}");
    assert_eq!(words_of(&all), Some("var(--text-primary)"), "{sheet}");
}

// A see-through part fill is laid over its root's own fill, then its grounds (#3303).

/// The rail's button filled light on the dark bar, its words dark, with its badge as `badge`.
fn on_a_light_button(badge: Value) -> ResolvedTheme {
    let mut parts = json!({
        "rail.button.fill": "text.primary",
        "rail.button.text": "surface.deep",
    });
    parts
        .as_object_mut()
        .unwrap()
        .extend(badge.as_object().unwrap().clone());
    purlis_dark(parts)
}

fn badge_fell(theme: &ResolvedTheme) -> bool {
    parts::fallen(theme)
        .iter()
        .any(|one| one.floor.starts_with("the badge's words"))
}

#[test]
fn a_see_through_part_fill_is_measured_over_the_root_s_own_fill() {
    // The light words read on the wash over the dark bar, and not on it over the light button
    // the badge is drawn in.
    let theme = on_a_light_button(json!({
        "rail.badge.fill": "accent.wash",
        "rail.badge.text": "text.primary",
    }));
    assert!(badge_fell(&theme), "{:?}", parts::fallen(&theme));
}

#[test]
fn words_that_only_read_over_the_root_s_own_fill_hold() {
    // The opposite way round: dark words on the wash over the light button read where they are
    // drawn, though they would not over the bar.
    let theme = on_a_light_button(json!({
        "rail.badge.fill": "accent.wash",
        "rail.badge.text": "surface.deep",
    }));
    assert!(!badge_fell(&theme), "{:?}", parts::fallen(&theme));
}

#[test]
fn a_chip_s_count_stands_on_the_bar_s_ground_whatever_the_chip_s_fill() {
    // `titlebar.css` holds the wash on an opaque ground of the bar's own, so a chip's fill is not
    // under it (D-3345-3), and light digits on the wash read on a light chip.
    let theme = purlis_dark(json!({
        "titlebar.workspace.fill": "text.primary",
        "titlebar.workspace.text": "surface.deep",
        "titlebar.count.fill": "accent.wash",
        "titlebar.count.text": "text.primary",
    }));
    assert!(
        !parts::fallen(&theme)
            .iter()
            .any(|one| one.floor.starts_with("the count's words")),
        "{:?}",
        parts::fallen(&theme)
    );
}

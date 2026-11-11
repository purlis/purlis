// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A surface's fill a theme sets is a ground of what is drawn on it** (#3099, from #3083): a
//! button on a panel or a popover whose fill a theme's parts change is measured on that fill,
//! the consent floors in every tint, and the surface draws its default where one breaks.

use serde_json::{Value, json};

use super::super::{ResolvedTheme, load};
use super::{Consented, State, consent_broken, fallen, family, surface_ground};

/// A dark theme over Purlis Classic that sets `tokens` and `parts`, as read.
fn shaped(tokens: Value, parts: Value) -> ResolvedTheme {
    let loaded = load(&json!({
        "name": "Shaped",
        "extends": "purlis-classic",
        "appearances": [{ "id": "dark", "tokens": tokens }],
        "parts": parts,
    }));
    assert!(!loaded.refused, "{:?}", loaded.complaints);
    loaded.theme.first().clone()
}

/// What fell, as `<part>: <floor>`.
fn fell_in(theme: &ResolvedTheme) -> Vec<String> {
    fallen(theme)
        .into_iter()
        .map(|one| format!("{}: {}", one.part, one.floor))
        .collect()
}

#[test]
fn a_popover_filled_in_the_words_of_the_buttons_in_it_draws_its_own_fill() {
    // The popover's own words read on its new fill; a ghost button's primary words in it do not.
    let theme = shaped(
        json!({}),
        json!({
            "popover.root.fill": "text.primary",
            "popover.root.text": "surface.base",
            "popover.footer.text": "surface.base",
            "popover.key.fill": "surface.base",
        }),
    );
    let fell = fell_in(&theme);
    assert!(
        fell.iter().any(
            |one| one.starts_with("popover.root.") && one.contains("what a button draws on it")
        ),
        "{fell:?}"
    );
    // Drawn as its default, the popover stands on its own surface again.
    assert!(
        super::stylesheet(&theme).contains("background-color: var(--surface-overlay)"),
        "the popover draws its default fill"
    );
}

#[test]
fn every_surface_a_family_stands_on_takes_a_fill_and_is_named_once() {
    // Who stands where is found from the components (`app/src/theme/grounds.test.ts`); here, that
    // the table names surfaces a theme can fill and families the manifest has.
    let mut seen = Vec::new();
    for (key, families) in super::STANDS_ON {
        assert!(!seen.contains(key), "{key} is named twice");
        seen.push(key);
        // A surface alone, or one look of it (`list.row`, #6149).
        let (surface, look) = super::surface_and_look(key);
        let one = family(surface).expect("a surface");
        assert!(
            one.parts[0].properties.contains(&super::Prop::Fill),
            "{key}"
        );
        if let Some(look) = look {
            assert!(
                one.looks().contains(&look),
                "{key}: a look of the {surface}"
            );
        }
        for name in *families {
            assert!(family(name).is_some(), "{name} on a {key}");
            assert_ne!(*name, surface, "a {surface} on itself");
            // Named once for a look: on the surface alone, it is on every look already.
            if look.is_some() {
                let everywhere = super::STANDS_ON
                    .iter()
                    .any(|(other, on)| *other == surface && on.contains(name));
                assert!(!everywhere, "{name} on a {key} and on every {surface}");
            }
        }
    }
}

#[test]
fn a_badge_is_read_on_a_list_s_plain_row_fill_and_never_on_its_card_s() {
    // The window draws a list's badges on its plain rows (`DevicesView.tsx`,
    // `SkillLibraryGrade.tsx`) and none on a card row, the one list fill a shipped theme sets
    // (Classic's `list.root.card.fill`, #6149).
    let list = family("list").expect("the list family");
    let badge_on = |variant| {
        super::drawn_on(list, variant, "transparent", "surface.sunken")
            .any(|one| one.name == "badge")
    };
    assert!(badge_on("row"), "a badge on a list's row");
    assert!(!badge_on("card"), "a badge on a list's card");
}

#[test]
fn what_a_row_opens_over_the_window_is_never_read_on_the_row_s_fill() {
    // A popover or a tooltip a list's row holds is portaled to the window's body: it is drawn
    // over the window, on its own grounds, never on the row's fill, chosen (#6156) or at rest
    // (#6164). What is drawn in the row itself is read on it.
    let list = family("list").expect("the list family");
    let drawn: Vec<&str> = super::drawn_on(list, "row", "list.selected", "surface.sunken")
        .map(|one| one.name)
        .collect();
    assert!(
        drawn.contains(&"badge") && drawn.contains(&"chat-mark"),
        "{drawn:?}"
    );
    assert!(
        !drawn.contains(&"popover") && !drawn.contains(&"tooltip"),
        "{drawn:?}"
    );
}

#[test]
fn a_chosen_row_filled_where_a_badge_in_it_vanishes_brings_the_chosen_fill_back() {
    // Before #6156 a surface's fill was read at rest alone: a list's chosen row could be given
    // a fill its badges' words vanish on, and the theme passed its check.
    let theme = shaped(
        json!({ "surface.sunken": "#303030" }),
        json!({ "list.root.row.selected.fill": "surface.sunken" }),
    );
    let fell = fell_in(&theme);
    let chosen: Vec<_> = fell
        .iter()
        .filter(|one| one.starts_with("list.root.row.selected"))
        .collect();
    assert!(
        chosen
            .iter()
            .any(|one| one.contains("what a badge draws on it")),
        "{fell:?}"
    );
    // The row at rest is drawn as written: only its chosen fill falls.
    assert!(
        fell.iter()
            .all(|one| !one.starts_with("list.root.row:") && !one.starts_with("list.root:")),
        "{fell:?}"
    );
}

#[test]
fn a_chosen_row_is_read_on_its_own_fill_after_its_rest_falls() {
    // A fall at rest keeps what the theme writes for a chosen row itself, so that fill is read
    // once the hold is asked again (#6156's review).
    let theme = shaped(
        json!({ "surface.sunken": "#303030", "accent.surface": "#303030" }),
        json!({
            "list.root.row.fill": "surface.sunken",
            "list.root.row.selected.fill": "accent.surface",
        }),
    );
    let fell = fell_in(&theme);
    assert!(
        fell.iter().any(|one| one.starts_with("list.root.row:")),
        "{fell:?}"
    );
    assert!(
        fell.iter()
            .any(|one| one.starts_with("list.root.row.selected")),
        "{fell:?}"
    );
}

#[test]
fn a_surface_fill_every_family_on_it_reads_on_is_drawn_as_written() {
    // A raised panel, as Purlis draws its own: every family drawn on the window reads on it.
    let theme = shaped(json!({}), json!({ "panel.root.fill": "surface.raised" }));
    assert_eq!(fell_in(&theme), Vec::<String>::new());
}

#[test]
fn allow_s_edge_is_held_on_a_surface_fill_a_theme_sets() {
    // A see-through control lets the surface under Allow show through it: on a surface the
    // accent's colour, its accent edge is lost.
    let theme = shaped(
        json!({ "control.base": "#2626230d" }),
        json!({ "panel.root.fill": "accent.base" }),
    );
    let button = family("button").expect("the button family");
    let panel = family("panel").expect("the panel family");
    let grants = button
        .variants
        .iter()
        .find(|one| one.name == "grants")
        .expect("an answer that grants");
    let on = surface_ground(&theme, panel, "accent.base");
    assert!(matches!(
        consent_broken(&theme, &on, &[], button, grants, &[]),
        Some(Consented::Own(floor, _)) if floor.contains("accent edge")
    ));
    // On the panel's own ground, the same button keeps its edge.
    let own = super::grounds(&theme, panel);
    assert!(consent_broken(&theme, &own, &[], button, grants, &[]).is_none());
    // And the panel falls back.
    let fell = fell_in(&theme);
    assert!(
        fell.iter().any(|one| one.starts_with("panel.root.")),
        "{fell:?}"
    );
}

#[test]
fn a_surface_under_a_consent_answer_is_measured_in_every_tint() {
    // The accent's surface is no ground a button names: a panel filled with it brings a new
    // ground under Allow, which is held in each of the theme's tints and at every whole degree.
    let tinted = load(&json!({
        "name": "Tinted", "extends": "purlis-classic",
        "tint": { "tokens": ["accent.base"] },
        "parts": { "panel.root.fill": "accent.surface" }
    }));
    assert!(!tinted.refused, "{:?}", tinted.complaints);
    let _ = fallen(tinted.theme.first());
    assert!(super::drawn_in() > 1, "measured in every tint");
    // A theme no workspace's colour tints is measured as written.
    let untinted = load(&json!({
        "name": "Plain", "extends": "purlis-classic",
        "tint": { "on": false },
        "parts": { "panel.root.fill": "accent.surface" }
    }));
    assert!(!untinted.refused, "{:?}", untinted.complaints);
    let _ = fallen(untinted.theme.first());
    assert_eq!(super::drawn_in(), 1);
}

/// What `name` breaks on `surface`'s fill `set` in place of its first look's default, read alone.
fn breaks(theme: &ResolvedTheme, name: &str, surface: &str, set: &str) -> Option<String> {
    let surface = family(surface).expect("a surface");
    let default = (surface.defaults)(surface.looks()[0], &[]).fill;
    let rules = super::rules(&theme.parts);
    super::broken_on(
        theme,
        &[],
        std::slice::from_ref(theme),
        &rules,
        family(name).expect("a family"),
        surface,
        default,
        set,
    )
    .map(|(floor, _)| floor)
}

/// A dark Classic whose `surface` is filled with the ground `token`, drawn `grey` (`#4a4a46`:
/// quiet words near 3:1 on it, the primary's near 6.5:1), with the popover's own words in the
/// strongest colour, so what falls is what is drawn on it.
fn filled_grey(surface: &str, token: &str, grey: &str) -> ResolvedTheme {
    shaped(
        json!({ token: grey }),
        json!({
            format!("{surface}.root.fill"): token,
            "popover.root.text": "text.primary",
            "popover.header.text": "text.primary",
            "popover.footer.text": "text.primary",
            "popover.key.text": "text.primary",
        }),
    )
}

#[test]
fn a_badge_on_a_panel_filled_where_its_words_vanish_brings_the_panel_back() {
    // A badge holds no answer, so before #3268 nothing measured it on a panel's fill.
    let theme = filled_grey("panel", "surface.sunken", "#303030");
    let fell = fell_in(&theme);
    assert!(
        fell.iter()
            .any(|one| one.starts_with("panel.root.") && one.contains("what a badge draws on it")),
        "{fell:?}"
    );
}

#[test]
fn a_field_s_help_on_a_popover_filled_where_it_vanishes_brings_the_popover_back() {
    // The field's own grounds name the sunken ground already: only its help, read on grounds of
    // its own (#4648), stands anew on the popover's fill (#3268).
    let theme = filled_grey("popover", "surface.sunken", "#4a4a46");
    let fell = fell_in(&theme);
    assert!(
        fell.iter().any(|one| one.starts_with("popover.root.")
            && one.contains("what a field draws on it: the help's words")),
        "{fell:?}"
    );
}

#[test]
fn a_field_s_help_is_read_on_a_dialog_s_fill_and_never_on_a_pane_s() {
    // A field's help is drawn in a dialog and on a raised ground, never on a pane's own: on a
    // dialog's fill it is read, on a panel's it is drawn nowhere.
    let dialog = filled_grey("dialog", "surface.sunken", "#4a4a46");
    let on_the_dialog = breaks(&dialog, "field", "dialog", "surface.sunken");
    assert!(
        on_the_dialog
            .as_deref()
            .is_some_and(|floor| floor.contains("the help's words")),
        "{on_the_dialog:?}"
    );
    // On a panel's, its error is read (a field that can refuse stands on a pane) and its help,
    // read before it, is not.
    let panel = filled_grey("panel", "surface.sunken", "#4a4a46");
    let on_the_panel = breaks(&panel, "field", "panel", "surface.sunken");
    assert!(
        on_the_panel
            .as_deref()
            .is_some_and(|floor| floor.contains("the error's words")),
        "{on_the_panel:?}"
    );
}

#[test]
fn a_list_s_row_in_a_dialog_is_read_on_the_dialog_s_fill() {
    // A list's rows stand in the palette's dialog (`Palette.tsx`): their quiet words on its fill,
    // a ground the list's own grounds do not name.
    let dialog = family("dialog").expect("the dialog family");
    assert!(
        super::drawn_on(
            dialog,
            dialog.looks()[0],
            "surface.overlay",
            "surface.hover"
        )
        .any(|one| one.name == "list"),
        "a list is drawn on a dialog"
    );
    let theme = filled_grey("dialog", "surface.hover", "#4a4a46");
    let broken = breaks(&theme, "list", "dialog", "surface.hover");
    assert!(broken.is_some(), "{broken:?}");
}

#[test]
fn a_tree_filled_where_a_badge_in_its_rows_vanishes_brings_the_tree_back() {
    // A tree's row draws no fill of its own: before #5863 nothing was measured on a fill a theme
    // gives it, so its badges could vanish there and pass.
    let theme = filled_grey("tree", "surface.sunken", "#303030");
    let fell = fell_in(&theme);
    assert!(
        fell.iter()
            .any(|one| one.starts_with("tree.root:") && one.contains("what a badge draws on it")),
        "{fell:?}"
    );
    // Read on each ground under it, the tree falls once.
    assert_eq!(
        fell.iter()
            .filter(|one| one.starts_with("tree.root:"))
            .count(),
        1,
        "{fell:?}"
    );
    // Drawn as its default, the row stands on its parent's ground again.
    // The fallback is written after the rule it takes the place of, so the last fill the row's
    // rules set is the one drawn.
    let sheet = super::stylesheet(&theme);
    let last = sheet.lines().map(str::trim_start).rfind(|line| {
        line.starts_with(":where([data-scope=\"tree\"][data-part=\"root\"]) {")
            && line.contains("background-color")
    });
    assert!(
        last.is_some_and(|line| line.contains("background-color: transparent")),
        "the tree draws no fill of its own: {last:?}"
    );
}

#[test]
fn a_table_filled_where_a_badge_in_its_rows_vanishes_brings_the_table_back() {
    let theme = filled_grey("table", "surface.sunken", "#303030");
    let fell = fell_in(&theme);
    assert!(
        fell.iter()
            .any(|one| one.starts_with("table.root:") && one.contains("what a badge draws on it")),
        "{fell:?}"
    );
}

#[test]
fn a_tree_filled_where_every_family_in_it_reads_is_drawn_as_written() {
    // The window's raised ground under a tree's rows: what stands on them reads there.
    let theme = shaped(json!({}), json!({ "tree.root.fill": "surface.raised" }));
    assert_eq!(fell_in(&theme), Vec::<String>::new());
    assert!(
        super::stylesheet(&theme).lines().any(|line| line
            .trim_start()
            .starts_with(":where([data-scope=\"tree\"][data-part=\"root\"])")
            && line.contains("background-color: var(--surface-raised)")),
        "the tree draws the fill it was given"
    );
}

#[test]
fn a_surface_with_no_fill_stands_on_its_parent_s_grounds_until_a_theme_fills_it() {
    // Unfilled, a tree is measured nowhere; filled, its default is the no fill it draws, which
    // stands for each ground the tree is drawn on.
    let unfilled = super::rules(&shaped(json!({}), json!({})).parts);
    assert!(
        super::surfaces_set(&unfilled)
            .iter()
            .all(|(surface, ..)| surface.name != "tree")
    );
    let filled =
        super::rules(&shaped(json!({}), json!({ "tree.root.fill": "accent.surface" })).parts);
    let on: Vec<_> = super::surfaces_set(&filled)
        .into_iter()
        .filter(|(surface, ..)| surface.name == "tree")
        .map(|(_, _, states, default, set)| (states, default, set))
        .collect();
    // A fill at rest is drawn chosen and under the pointer too, in place of the defaults there
    // (#6156).
    assert_eq!(
        on,
        [
            (&[][..], "transparent", "accent.surface"),
            (&[State::Selected][..], "list.selected", "accent.surface"),
            (&[State::Hover][..], "surface.hover", "accent.surface"),
        ]
    );
    let tree = family("tree").expect("the tree family");
    assert_eq!(
        super::defaults_under(tree, &"transparent"),
        ["surface.base", "surface.raised", "surface.sunken"]
    );
    assert_eq!(
        super::defaults_under(tree, &"surface.overlay"),
        ["surface.overlay"]
    );
}

#[test]
fn a_field_s_error_is_read_on_a_table_s_fill_and_its_help_never_is() {
    // A table's rows stand on the window's own ground, which a field's error is read on and its
    // help is not: on a table's fill, the error stands anew and the help is drawn nowhere.
    let table = family("table").expect("the table family");
    let field = family("field").expect("the field family");
    assert!(
        super::drawn_on(table, table.looks()[0], "transparent", "surface.sunken")
            .any(|one| one.name == "field"),
        "a field is drawn on a table's fill"
    );
    let theme = filled_grey("table", "surface.sunken", "#4a4a46");
    let help = field
        .parts
        .iter()
        .position(|part| part.name == "help")
        .expect("a field's help");
    let variant = field.looks()[0];
    let look = super::drawn(&[], field, variant, &[]);
    let on = surface_ground(&theme, table, "surface.sunken");
    assert_eq!(
        super::under_part(
            &theme,
            &on,
            Some(super::defaults_under(table, &"transparent")),
            field,
            help,
            variant,
            &look
        ),
        Vec::<String>::new(),
        "a field's help is drawn nowhere on a table"
    );
    let rules = super::rules(&theme.parts);
    let broken = super::broken_on(
        &theme,
        &[],
        std::slice::from_ref(&theme),
        &rules,
        field,
        table,
        "transparent",
        "surface.sunken",
    )
    .map(|(floor, _)| floor);
    assert!(
        broken
            .as_deref()
            .is_some_and(|floor| floor.contains("the error's words")),
        "{broken:?}"
    );
}

#[test]
fn a_row_filled_like_a_tooltip_is_drawn_as_written_since_the_tooltip_opens_over_the_window() {
    // A list's row filled in the tooltip's own colour, #1f1f1c: on the row a tooltip's edge would
    // vanish, but the tooltip is portaled to the window's body, never drawn on the row, so the
    // row draws as written (#6164).
    let theme = shaped(
        json!({ "surface.sunken": "#1f1f1c" }),
        json!({ "list.root.row.fill": "surface.sunken" }),
    );
    let fell = fell_in(&theme);
    assert!(
        !fell.iter().any(|one| one.starts_with("list.root.row")),
        "{fell:?}"
    );
}

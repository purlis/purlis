// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The popover and menu families** (#2909, spec #2897): the surface a popover and a menu stand
//! on, with R165's anatomy, and a menu's row, each declared once here and once in its wrapper
//! under `app/src/ui/`, whose stylesheet draws the defaults [`super::Look`] holds a copy of.
//!
//! A surface is as tall as what it holds, but its root sets no padding or height, so it is held
//! like a control of the overlay's text size; a menu's row is a control a control tall.

use super::{ALL, Consent, Family, Length, Look, Part, Prop, State, TRANSPARENT, Variant};

pub(super) const POPOVER: Family = Family {
    name: "popover",
    description: "A popover, and the surface a menu stands on: a card beside a row, a refusal \
                  under a field, a list with its filter, a menu.",
    since: 6,
    parts: &[
        Part {
            name: "root",
            since: 6,
            description: "The surface: its fill, words, border, corners and lift.",
            properties: &[
                Prop::Fill,
                Prop::Text,
                Prop::BorderColour,
                Prop::BorderWidth,
                Prop::CornerRadius,
                Prop::Shadow,
            ],
        },
        Part {
            name: "filter",
            since: 6,
            description: "The filter row at its top, the field that narrows its list.",
            properties: &[Prop::Fill, Prop::Padding],
        },
        Part {
            name: "rule",
            since: 6,
            description: "The rule under the filter row and between a menu's groups.",
            properties: &[Prop::Fill],
        },
        Part {
            name: "list",
            since: 6,
            description: "The list that scrolls inside it: its rows' room and the gap between them.",
            properties: &[Prop::Padding, Prop::Gap],
        },
        Part {
            name: "header",
            since: 6,
            description: "A group's name over its rows.",
            properties: &[Prop::Text, Prop::FontSize, Prop::FontWeight],
        },
        Part {
            name: "footer",
            since: 6,
            description: "The footer that says the keys its list answers to.",
            properties: &[Prop::Fill, Prop::Text, Prop::Padding, Prop::FontSize],
        },
        Part {
            name: "key",
            since: 6,
            description: "A key the footer names, in its cap.",
            properties: &[Prop::Fill, Prop::Text, Prop::CornerRadius],
        },
    ],
    variants: &[
        Variant {
            name: "popover",
            since: 6,
            description: "A popover that holds words and controls.",
            consent: None,
        },
        Variant {
            name: "menu",
            since: 6,
            description: "The surface a menu stands on.",
            consent: None,
        },
    ],
    grounds: &["surface.base", "surface.raised"],
    keeps_size: &[],
    sized_apart: &[],
    height: Some(Length::Token("size.control")),
    font_size: "font-size.md",
    line_height: 1.0,
    border_width: 1.0,
    defaults: popover_look,
    // A surface is drawn by what it holds: no state of its own.
    states: &[],
    focused: "[data-focus-visible]",
    // The footer's keys in the quiet tier, and each key in its cap on the raised surface
    // (`popover.css`), read on that cap (#3099).
    words: &[("footer", "text.quiet"), ("key", "text.secondary")],
    corner_most: None,
    around: &[],
    fills: &[("key", "surface.raised")],
    under: &[],
};

pub(super) const MENU: Family = Family {
    name: "menu",
    description: "A menu's row: a dropdown's, a context menu's, a submenu's.",
    since: 6,
    parts: &[
        Part {
            name: "root",
            since: 6,
            description: "The row itself: its fill, words, corners and size.",
            properties: ALL,
        },
        Part {
            name: "label",
            since: 6,
            description: "Its words.",
            properties: &[Prop::Text, Prop::FontSize, Prop::FontWeight],
        },
        Part {
            name: "description",
            since: 6,
            description: "The one line under its words: what it costs, or why it cannot run.",
            properties: &[Prop::Text, Prop::FontSize],
        },
        Part {
            name: "shortcut",
            since: 6,
            description: "The key that does the same, at its end.",
            properties: &[Prop::Text, Prop::FontSize],
        },
        Part {
            name: "indicator",
            since: 6,
            description: "A toggle's tick or a submenu's arrow, at its end.",
            properties: &[Prop::Text],
        },
    ],
    variants: &[
        Variant {
            name: "default",
            since: 6,
            description: "A row.",
            consent: None,
        },
        Variant {
            name: "danger",
            since: 6,
            description: "A row that cannot be undone.",
            consent: Some(Consent::Danger),
        },
    ],
    grounds: &["surface.overlay"],
    keeps_size: &[],
    sized_apart: &[],
    height: Some(Length::Token("size.control")),
    font_size: "font-size.md",
    line_height: 1.0,
    border_width: 0.0,
    defaults: menu_look,
    // What its wrapper marks: under the pointer, the keyboard's focus, a ticked toggle, a
    // submenu open from it, and a row that cannot run.
    states: &[
        State::Hover,
        State::Focused,
        State::Selected,
        State::Disabled,
        State::Open,
    ],
    focused: "[data-focus-visible]",
    // Its words not in the row's colour: the line under them, its key and its tick.
    words: &[
        ("description", "text.secondary"),
        ("shortcut", "text.quiet"),
        ("indicator", "text.muted"),
    ],
    corner_most: None,
    around: &[],
    fills: &[],
    under: &[],
};

/// **The popover's colours as `app/src/ui/popover/popover.css` draws them**: one surface for a
/// popover and a menu, the overlay surface (#1845), in every state.
fn popover_look(_variant: &str, _states: &[State]) -> Look {
    Look {
        fill: "surface.overlay",
        text: "text.primary",
        border: "border.subtle",
    }
}

/// **A menu row's colours as `app/src/ui/menu/menu.css` draws them**, which a test holds to that
/// file: at rest on the surface's own ground; under the pointer or the keyboard, or with its
/// submenu open, on the highlight (the danger colour's own ground for what cannot be undone);
/// and a row that cannot run in the faint tier.
fn menu_look(variant: &str, states: &[State]) -> Look {
    let danger = variant == "danger";
    let mut look = Look {
        fill: TRANSPARENT,
        text: if danger {
            "danger.base"
        } else {
            "text.primary"
        },
        border: TRANSPARENT,
    };
    if states.contains(&State::Disabled) {
        look.text = "text.faint";
        return look;
    }
    let under = states.contains(&State::Hover) || states.contains(&State::Focused);
    if danger && under {
        look.fill = "danger.surface";
    } else if under || states.contains(&State::Open) {
        look.fill = "tab.active";
    }
    look
}

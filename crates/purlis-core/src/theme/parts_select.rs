// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The select and combobox families** (#2906, spec #2897): the box a person picks one of many
//! from, and the box they type in with suggestions under it, each declared once here and once in
//! its wrapper under `app/src/ui/select/`, whose stylesheet (`select.css`) draws the defaults
//! [`super::Look`] holds a copy of.
//!
//! Both boxes are drawn as the text input's is (`box_look`), so a select and a text field in one
//! form are one look, and a theme that reaches one box reaches the other by the same key. Their
//! lists are no family of their own: they stand on the popover family's `menu` surface and their
//! options are the menu family's rows, so a theme that sets a menu's row sets an option's too.

use super::{Family, Length, Part, Prop, State, box_look};

/// What a theme may set on a box with words in it: everything but the words' weight.
const BOX: &[Prop] = &[
    Prop::Fill,
    Prop::Text,
    Prop::BorderColour,
    Prop::Shadow,
    Prop::BorderWidth,
    Prop::CornerRadius,
    Prop::Padding,
    Prop::Gap,
    Prop::FontSize,
    Prop::Height,
];

/// Every surface a control is drawn on (the button's, which `grounds.test.ts` finds from the
/// components): a select stands where a field does, a filter on a list's layer.
const GROUNDS: &[&str] = &[
    "surface.base",
    "surface.raised",
    "surface.overlay",
    "surface.sunken",
    "surface.deep",
    "border.subtle",
    "tab.active",
    "danger.surface",
    "danger.wash",
    "layer.project",
    "layer.workspace",
    "layer.chat",
    "layer.selected",
    "terminal.background",
];

/// The words of a box that are not in its own colour: what it shows while nothing is picked and
/// its chevron, both in the quiet tier (R64).
const QUIET: &[(&str, &str)] = &[("placeholder", "text.quiet"), ("chevron", "text.quiet")];

/// **A select** (#2906): the button that shows what is picked, and opens the list to pick from.
pub(super) const SELECT: Family = Family {
    name: "select",
    description: "A select: one of many, picked from a list it opens.",
    since: super::NEXT,
    parts: &[
        Part {
            name: "root",
            since: super::NEXT,
            description: "The box itself: its fill, border, corners and size, and the words of \
                          what is picked.",
            properties: BOX,
        },
        Part {
            name: "placeholder",
            since: super::NEXT,
            description: "What it shows while nothing is picked.",
            properties: &[Prop::Text],
        },
        Part {
            name: "chevron",
            since: super::NEXT,
            description: "The chevron at its end, which says it opens a list.",
            properties: &[Prop::Text],
        },
    ],
    variants: &[],
    grounds: GROUNDS,
    keeps_size: &["[data-size=\"sm\"]", "[data-size=\"lg\"]"],
    sized_apart: &[
        (
            "[data-scope=\"select\"][data-part=\"root\"][data-size=\"sm\"]",
            "max(var(--size-chrome), var(--space-7))",
        ),
        (
            "[data-scope=\"select\"][data-part=\"root\"][data-size=\"lg\"]",
            "calc(var(--size-control) + var(--space-2))",
        ),
    ],
    height: Some(Length::Token("size.control")),
    font_size: "font-size.base",
    line_height: 1.0,
    border_width: 1.0,
    defaults: box_look,
    // A button: under the pointer, the keyboard on it, its list open, and unavailable.
    states: &[State::Hover, State::Focused, State::Disabled, State::Open],
    focused: "[data-focus-visible]",
    words: QUIET,
    corner_most: None,
    around: &[],
    fills: &[],
    under: &[],
};

/// **A combobox** (#2906): a box to type in, with the suggestions that match under it.
pub(super) const COMBOBOX: Family = Family {
    name: "combobox",
    description: "A combobox: a box to type in, with the suggestions that match in a list under it.",
    since: super::NEXT,
    parts: &[
        Part {
            name: "root",
            since: super::NEXT,
            description: "The box itself: its fill, border, corners and size, and the words typed \
                          in it.",
            properties: BOX,
        },
        Part {
            name: "chevron",
            since: super::NEXT,
            description: "The chevron at its end, which opens every suggestion.",
            properties: &[Prop::Text],
        },
    ],
    variants: &[],
    grounds: GROUNDS,
    keeps_size: &["[data-size=\"sm\"]", "[data-size=\"lg\"]"],
    sized_apart: &[
        (
            "[data-scope=\"combobox\"][data-part=\"root\"][data-size=\"sm\"]",
            "max(var(--size-chrome), var(--space-7))",
        ),
        (
            "[data-scope=\"combobox\"][data-part=\"root\"][data-size=\"lg\"]",
            "calc(var(--size-control) + var(--space-2))",
        ),
    ],
    height: Some(Length::Token("size.control")),
    font_size: "font-size.base",
    line_height: 1.0,
    border_width: 1.0,
    defaults: box_look,
    // A box typed in: under the pointer, holding the caret, its list open, and unavailable.
    states: &[State::Hover, State::Focused, State::Disabled, State::Open],
    focused: "[data-focus-within]",
    words: &[("chevron", "text.quiet")],
    corner_most: None,
    around: &[],
    fills: &[],
    under: &[],
};

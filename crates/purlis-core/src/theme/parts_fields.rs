// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The field families** (#2905): a field (its label, its control, its help and its error, as
//! one), the text input and the textarea, declared once here and once in their wrappers,
//! `app/src/ui/field/`.

use super::{CONTROL_GROUNDS, Family, Length, Part, Prop, State, box_look, field_look};

pub(super) const FIELD: Family = Family {
    name: "field",
    description: "A field: its label, its control, its help and its error, as one.",
    since: 3,
    parts: &[
        Part {
            name: "root",
            since: 3,
            description: "The field itself, which stacks its label, its control, its help and \
                      its error: the room between them.",
            properties: &[Prop::Gap],
        },
        Part {
            name: "label",
            since: 3,
            description: "Its name, over its control.",
            properties: &[Prop::Text, Prop::FontSize, Prop::FontWeight],
        },
        Part {
            name: "help",
            since: 3,
            description: "Its line of help, under its control.",
            properties: &[Prop::Text, Prop::FontSize],
        },
        Part {
            name: "error",
            since: 3,
            description: "Why its value is refused, under its control.",
            properties: &[Prop::Text, Prop::FontSize],
        },
    ],
    variants: &[],
    // A field stands where a button does, a filter on a list's layer.
    grounds: CONTROL_GROUNDS,
    keeps_size: &[],
    sized_apart: &[],
    height: Some(Length::Token("size.control")),
    font_size: "font-size.base",
    line_height: 1.0,
    border_width: 0.0,
    defaults: field_look,
    states: &[State::Disabled],
    focused: "[data-focus-within]",
    words: &[("help", "text.quiet"), ("error", "danger.text")],
    corner_most: None,
    around: &[],
    fills: &[],
};

pub(super) const TEXT_INPUT: Family = Family {
    name: "text-input",
    description: "A box for one line of text: a name, a token, a filter.",
    since: 3,
    parts: &[
        Part {
            name: "root",
            since: 3,
            description: "The box itself: its fill, border, corners and size, and the words \
                      typed in it.",
            properties: &[
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
            ],
        },
        Part {
            name: "icon",
            since: 3,
            description: "The mark at its start: a filter's or a search's glass.",
            properties: &[Prop::Text],
        },
    ],
    variants: &[],
    // A field stands where a button does, a filter on a list's layer.
    grounds: CONTROL_GROUNDS,
    keeps_size: &["[data-size=\"sm\"]", "[data-size=\"lg\"]"],
    sized_apart: &[
        (
            "[data-scope=\"text-input\"][data-part=\"root\"][data-size=\"sm\"]",
            "max(var(--size-chrome), var(--space-7))",
        ),
        (
            "[data-scope=\"text-input\"][data-part=\"root\"][data-size=\"lg\"]",
            "calc(var(--size-control) + var(--space-2))",
        ),
    ],
    height: Some(Length::Token("size.control")),
    font_size: "font-size.base",
    line_height: 1.0,
    border_width: 1.0,
    defaults: box_look,
    states: &[State::Hover, State::Focused, State::Disabled],
    focused: "[data-focus-within]",
    words: &[],
    corner_most: None,
    around: &[],
    fills: &[],
};

pub(super) const TEXTAREA: Family = Family {
    name: "textarea",
    description: "A box for lines of text: an answer, a list of entries.",
    since: 3,
    parts: &[Part {
        name: "root",
        since: 3,
        description: "The box itself: its fill, border, corners and room, and the words typed \
                  in it. As tall as its lines.",
        properties: &[
            Prop::Fill,
            Prop::Text,
            Prop::BorderColour,
            Prop::Shadow,
            Prop::BorderWidth,
            Prop::CornerRadius,
            Prop::Padding,
            Prop::FontSize,
        ],
    }],
    variants: &[],
    // A field stands where a button does, a filter on a list's layer.
    grounds: CONTROL_GROUNDS,
    keeps_size: &[],
    sized_apart: &[],
    height: Some(Length::Token("size.control")),
    font_size: "font-size.base",
    line_height: 1.0,
    border_width: 1.0,
    defaults: box_look,
    states: &[State::Hover, State::Focused, State::Disabled],
    focused: "[data-focus-within]",
    words: &[],
    corner_most: None,
    around: &[],
    fills: &[],
};

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The collection families** (#2911, spec #2897): a list's row and a tree's, each declared
//! once here and once in its wrapper under `app/src/ui/list/` and `app/src/ui/tree/`, whose
//! stylesheets draw the defaults [`super::Look`] holds a copy of.
//!
//! A row is the family's root, and carries its state; the list around it, a group's name over
//! its rows and the slots inside a row (its marks, its words, the line about it, its end) are
//! its other parts. A row is a control a control tall at the default size; a dense list's rows
//! and a dialog's are drawn at the family's other sizes, which a theme's size never reaches.

use super::{ALL, Family, Length, Look, Part, Prop, State, TRANSPARENT};

/// What a theme may set on a row's words: their colour, size and weight.
const WORDS: &[Prop] = &[Prop::Text, Prop::FontSize, Prop::FontWeight];

/// What a theme may set on a row's marks and its end: their colour and the room between them.
const MARKS: &[Prop] = &[Prop::Text, Prop::Gap];

pub(super) const LIST: Family = Family {
    name: "list",
    description: "A list's row: an option to choose, a row that acts, a card of a grid.",
    since: 11,
    parts: &[
        Part {
            name: "root",
            since: 11,
            description: "The row itself: its fill, words, corners and size.",
            properties: ALL,
        },
        Part {
            name: "body",
            since: 11,
            description: "The list that holds the rows: the room it keeps around them, so a row's \
                          pill never touches the panel it is in.",
            properties: &[Prop::Padding, Prop::Gap],
        },
        Part {
            name: "lead",
            since: 11,
            description: "Its marks before its words: a state, an icon, a persona's tile.",
            properties: MARKS,
        },
        Part {
            name: "label",
            since: 11,
            description: "Its words.",
            properties: WORDS,
        },
        Part {
            name: "description",
            since: 11,
            description: "The one line about it, beside its words or under them.",
            properties: &[Prop::Text, Prop::FontSize],
        },
        Part {
            name: "trail",
            since: 11,
            description: "Its end: a count, a state, the row's own actions.",
            properties: MARKS,
        },
        Part {
            name: "header",
            since: 11,
            description: "A group's name over its rows.",
            properties: WORDS,
        },
    ],
    variants: &[],
    // Drawn in a side view, a tab's page, Settings and a dialog.
    grounds: &[
        "surface.base",
        "surface.raised",
        "surface.overlay",
        "surface.sunken",
    ],
    keeps_size: &["[data-size=\"sm\"]", "[data-size=\"lg\"]"],
    sized_apart: &[
        (
            "[data-scope=\"list\"][data-part=\"root\"][data-size=\"sm\"]",
            "var(--size-chrome)",
        ),
        (
            "[data-scope=\"list\"][data-part=\"root\"][data-size=\"lg\"]",
            "calc(var(--size-control) + var(--space-2))",
        ),
    ],
    height: Some(Length::Token("size.control")),
    font_size: "font-size.base",
    line_height: 1.0,
    border_width: 0.0,
    defaults: list_look,
    // What React Aria marks on a row: under the pointer, the keyboard's focus, chosen, and a row
    // that cannot be used.
    states: &[
        State::Hover,
        State::Focused,
        State::Selected,
        State::Disabled,
    ],
    focused: "[data-focus-visible]",
    // Its words not in the row's colour: the line about it, and a group's name.
    words: &[("description", "text.quiet"), ("header", "text.quiet")],
    corner_most: None,
    // The list that holds the rows, and a group's name over them, stand around a row.
    around: &["body", "header"],
    fills: &[],
};

/// What a theme may set on a tree's row: all of a row's properties but its padding, which is its
/// indentation (each level one step in) and stays the family's.
const TREE_ROW: &[Prop] = &[
    Prop::Fill,
    Prop::Text,
    Prop::BorderColour,
    Prop::Shadow,
    Prop::BorderWidth,
    Prop::CornerRadius,
    Prop::Gap,
    Prop::FontSize,
    Prop::FontWeight,
    Prop::Height,
];

pub(super) const TREE: Family = Family {
    name: "tree",
    description: "A tree's row: a folder, a file, a repo and what is true of it, a chat and \
                  what runs under it.",
    since: super::NEXT,
    parts: &[
        Part {
            name: "root",
            since: super::NEXT,
            description: "The row itself: its fill, words, corners and size. Its indentation \
                          is the family's.",
            properties: TREE_ROW,
        },
        Part {
            name: "body",
            since: super::NEXT,
            description: "The tree that holds the rows: the room it keeps around them, so a \
                          row's pill never touches the panel it is in.",
            properties: &[Prop::Padding, Prop::Gap],
        },
        Part {
            name: "twist",
            since: super::NEXT,
            description: "The chevron that opens and closes a row's rows: a mark.",
            properties: &[Prop::Text],
        },
        Part {
            name: "lead",
            since: super::NEXT,
            description: "Its marks before its words: a file's icon, a state.",
            properties: MARKS,
        },
        Part {
            name: "label",
            since: super::NEXT,
            description: "Its words.",
            properties: WORDS,
        },
        Part {
            name: "description",
            since: super::NEXT,
            description: "The one line about it, beside its words.",
            properties: &[Prop::Text, Prop::FontSize],
        },
        Part {
            name: "trail",
            since: super::NEXT,
            description: "Its end: a count, a change's mark, the row's own actions.",
            properties: MARKS,
        },
        Part {
            name: "header",
            since: super::NEXT,
            description: "A group's name over its rows.",
            properties: WORDS,
        },
    ],
    variants: &[],
    // Drawn in a side view and a tab's page.
    grounds: &["surface.base", "surface.raised", "surface.sunken"],
    keeps_size: &["[data-size=\"sm\"]", "[data-size=\"lg\"]"],
    sized_apart: &[
        (
            "[data-scope=\"tree\"][data-part=\"root\"][data-size=\"sm\"]",
            "var(--size-chrome)",
        ),
        (
            "[data-scope=\"tree\"][data-part=\"root\"][data-size=\"lg\"]",
            "calc(var(--size-control) + var(--space-2))",
        ),
    ],
    height: Some(Length::Token("size.control")),
    font_size: "font-size.base",
    line_height: 1.0,
    border_width: 0.0,
    // A tree's row is a list's row (`list_look`), in the tree's own scope.
    defaults: list_look,
    states: &[
        State::Hover,
        State::Focused,
        State::Selected,
        State::Disabled,
    ],
    focused: "[data-focus-visible]",
    // Its words not in the row's colour: the line about it, a group's name, and the twist, a
    // mark in the quiet tier (R129, the Explorer's twisties).
    words: &[
        ("description", "text.quiet"),
        ("header", "text.quiet"),
        ("twist", "text.quiet"),
    ],
    corner_most: None,
    around: &["body", "header"],
    fills: &[],
};

/// **A row's colours as `app/src/ui/list/list.css` draws them**, which a test holds to that file:
/// at rest in the secondary tier on no ground of its own (R19); under the pointer, the pill;
/// chosen, the wash with its words in the primary tier (R21), under the pointer too; and a row
/// that cannot be used in the faint tier.
pub(super) fn list_look(_variant: &str, states: &[State]) -> Look {
    if states.contains(&State::Disabled) {
        return Look {
            fill: TRANSPARENT,
            text: "text.faint",
            border: TRANSPARENT,
        };
    }
    if states.contains(&State::Selected) {
        return Look {
            fill: "list.selected",
            text: "text.primary",
            border: TRANSPARENT,
        };
    }
    Look {
        fill: if states.contains(&State::Hover) {
            "surface.hover"
        } else {
            TRANSPARENT
        },
        text: "text.secondary",
        border: TRANSPARENT,
    }
}

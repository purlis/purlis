// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The tree family** (#2911, spec #2897): a tree's row, declared once here and once in its
//! wrapper under `app/src/ui/tree/`, whose stylesheet draws the defaults [`super::Look`] holds a
//! copy of. A tree's row is a list's row, a level a step in, with its twist: it is drawn with the
//! list's colours ([`super::collections::list_look`]).

use super::collections::list_look;
use super::{Family, Length, Part, Prop, State};

/// What a theme may set on a row's words: their colour, size and weight.
const WORDS: &[Prop] = &[Prop::Text, Prop::FontSize, Prop::FontWeight];

/// What a theme may set on a row's marks before and after its words: their colour and the room
/// between them.
const MARKS: &[Prop] = &[Prop::Text, Prop::Gap];

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

/// **A tree's row** (#2911): the list's row, a level a step in, with its twist.
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


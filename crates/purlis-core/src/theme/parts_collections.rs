// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The collection families** (#2911, spec #2897): a list's row and a table's, each declared once
//! here and once in its wrapper under `app/src/ui/list/` and `app/src/ui/table/`, whose
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

/// **A table** (#2911 part 3): its row, the family's root and the part a state reaches (the
/// pointer on a row that opens or is chosen, the keyboard on it, chosen, a row that cannot be
/// used); the card that holds the table (`frame`), a column's name (`head`), a cell's room
/// (`cell`), and the bar over a table that chooses (`bar`). A table of data and a table of
/// controls are drawn alike, so it has no variants. A row is as tall as what its cells hold: no
/// height to set, and no corner (a row of a table is a band across it). The rule between rows is
/// a part of its own (`rule`), which the family draws as none (#2804) and a theme may fill.
pub(super) const TABLE: Family = Family {
    name: "table",
    description: "A table: its rows, the card that holds them, a column's name and a cell.",
    since: super::NEXT,
    parts: &[
        Part {
            name: "root",
            since: super::NEXT,
            description: "A row: its fill and its words, at rest and in each state.",
            properties: &[Prop::Fill, Prop::Text],
        },
        Part {
            name: "frame",
            since: super::NEXT,
            description: "The card that holds the table and scrolls it sideways: its fill, edge, \
                          corners and lift.",
            properties: &[
                Prop::Fill,
                Prop::BorderColour,
                Prop::BorderWidth,
                Prop::CornerRadius,
                Prop::Shadow,
            ],
        },
        Part {
            name: "head",
            since: super::NEXT,
            description: "A column's name, over its cells.",
            properties: WORDS,
        },
        Part {
            name: "cell",
            since: super::NEXT,
            description: "A cell: the room around what it holds.",
            properties: &[Prop::Padding],
        },
        Part {
            name: "rule",
            since: super::NEXT,
            description: "The rule between rows and under the columns' names: none unless a \
                          theme fills it.",
            properties: &[Prop::Fill],
        },
        Part {
            name: "bar",
            since: super::NEXT,
            description: "The bar over a table that chooses: its filter, and what the chosen rows \
                          can be done to.",
            properties: &[Prop::Gap],
        },
    ],
    variants: &[],
    // Its rows stand on its frame, the window's own ground.
    grounds: &["surface.base"],
    keeps_size: &[],
    sized_apart: &[],
    height: None,
    font_size: "font-size.base",
    line_height: 1.4,
    border_width: 0.0,
    defaults: table_look,
    // What React Aria marks on a data table's row: under the pointer, the keyboard's focus,
    // chosen, and a row that cannot be used.
    states: &[
        State::Hover,
        State::Focused,
        State::Selected,
        State::Disabled,
    ],
    focused: "[data-focus-visible]",
    // A column's name is in the label role, in the muted tier (R74).
    words: &[("head", "text.muted")],
    corner_most: None,
    // The card, a column's name and the bar stand around a row.
    around: &["frame", "head", "bar"],
    fills: &[],
};

/// **A table's row as `app/src/ui/table/table.css` draws it**, which a test holds to that file:
/// at rest on no ground of its own, its words in the primary tier (a table's words are data);
/// under the pointer, the pointer's ground; chosen, the wash with its words in the primary tier,
/// under the pointer too; and a row that cannot be used in the faint tier.
pub(super) fn table_look(_variant: &str, states: &[State]) -> Look {
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
        text: "text.primary",
        border: TRANSPARENT,
    }
}

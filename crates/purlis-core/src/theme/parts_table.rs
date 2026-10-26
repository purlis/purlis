// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The table family** (#2911, spec #2897): a table's row, declared once here and once in its
//! wrapper under `app/src/ui/table/`, whose stylesheet draws the defaults [`super::Look`] holds a
//! copy of.

use super::{Family, Look, Part, Prop, State, TRANSPARENT};

/// What a column's name takes: its words.
const WORDS: &[Prop] = &[Prop::Text, Prop::FontSize, Prop::FontWeight];

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
    under: &[],
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

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What a drag draws over the panes** (#5291, spec #5288): a pane about to be, the size a
//! terminal will take, the mark on a refused place and on a tab about to come forward (#5294), and what is held and what letting go would do
//! by the pointer. Declared once here and once in its wrapper,
//! `app/src/ui/pane-preview/PanePreview.tsx`, whose stylesheet (`pane-preview.css`) draws the
//! defaults [`super::Look`] holds a copy of.
//!
//! **None of it is read by a reader, or takes the pointer.** A reader hears the drag from the
//! window's status line. Its words are held at 4.5:1 on the ground each is drawn on, and the mark on
//! a refused place at 3:1 as an edge that is a part's only edge.

use super::{Family, Look, Part, Prop, State, TRANSPARENT};

/// What a label by the pointer takes: its ground, its edge, its words and its room.
const LABEL: &[Prop] = &[
    Prop::Fill,
    Prop::Text,
    Prop::BorderColour,
    Prop::BorderWidth,
    Prop::CornerRadius,
    Prop::Padding,
    Prop::FontSize,
];

/// **What a drag draws over the panes** (#5291).
pub(super) const PANE_PREVIEW: Family = Family {
    name: "pane-preview",
    description: "What a drag draws over the panes: a pane about to be, the size a terminal will \
                  take, the mark on a refused place, and the label and the words by the pointer.",
    since: super::NEXT,
    parts: &[
        Part {
            name: "arriving",
            since: super::NEXT,
            description: "A pane the drop would make, or a side that would fold: its fill and its \
                          edge.",
            properties: &[
                Prop::Fill,
                Prop::Text,
                Prop::BorderColour,
                Prop::BorderWidth,
                Prop::CornerRadius,
            ],
        },
        Part {
            name: "name",
            since: super::NEXT,
            description: "The name on a pane about to be.",
            properties: &[Prop::Text, Prop::FontSize, Prop::Padding],
        },
        Part {
            name: "line",
            since: super::NEXT,
            description: "On the strip, the line where a tab would go.",
            properties: &[Prop::Fill],
        },
        Part {
            name: "opening",
            since: super::NEXT,
            description: "The mark along a tab a drag rests on, until it comes forward.",
            properties: &[Prop::Fill],
        },
        Part {
            name: "size",
            since: super::NEXT,
            description: "The columns and rows a terminal will take, on the pane that will change.",
            properties: &[
                Prop::Fill,
                Prop::Text,
                Prop::CornerRadius,
                Prop::Padding,
                Prop::FontSize,
            ],
        },
        Part {
            name: "refused",
            since: super::NEXT,
            description: "The mark on the pane a refused place means: its edge alone.",
            properties: &[Prop::BorderColour, Prop::BorderWidth, Prop::CornerRadius],
        },
        Part {
            name: "held",
            since: super::NEXT,
            description: "What is held, by the pointer.",
            properties: LABEL,
        },
        Part {
            name: "words",
            since: super::NEXT,
            description: "What letting go would do, or why it can't, by the pointer.",
            properties: LABEL,
        },
    ],
    variants: &[],
    grounds: &["terminal.background", "surface.overlay"],
    keeps_size: &[],
    sized_apart: &[],
    height: None,
    font_size: "font-size.sm",
    line_height: 1.0,
    border_width: 0.0,
    defaults: pane_preview_look,
    states: &[],
    focused: "[data-focus-visible]",
    words: &[
        ("size", "text.primary"),
        ("held", "text.primary"),
        ("words", "text.primary"),
    ],
    corner_most: None,
    around: &[],
    fills: &[
        ("size", "surface.overlay"),
        ("held", "surface.overlay"),
        ("words", "surface.overlay"),
    ],
    under: &[],
};

/// **Its colours as `pane-preview.css` draws them**: no ground of the family's own, words in the
/// primary tier.
pub(super) fn pane_preview_look(_variant: &str, _states: &[State]) -> Look {
    Look {
        fill: TRANSPARENT,
        text: "text.primary",
        border: TRANSPARENT,
    }
}

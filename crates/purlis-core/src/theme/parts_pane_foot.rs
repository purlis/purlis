// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The pane foot** (#5202, spec #5191, look A): the quiet line under a chat's terminal that
//! says where the chat runs and how full its context is, declared once here and once in its
//! wrapper, `app/src/ui/pane-foot/PaneFoot.tsx`, whose stylesheet (`pane-foot.css`) draws the
//! defaults [`super::Look`] holds a copy of.
//!
//! Its parts are joined by a `·` and drawn in muted words on the terminal's own fill: harness
//! · workspace at its start, and at its end `ctx`, its number with its fill as a thin bar under
//! it, then `cache`. It is the tab strip's height at the density in force, which its stylesheet
//! writes as the tab's own sum (a control's height and 3/14 rem), so it is no size token
//! (`height: None`) and a compact appearance makes it compact.
//!
//! **Its bar is not the only way its value is told**: the number above it says it in figures,
//! so the bar is a mark a reader is not given (`aria-hidden`), drawn in the core's tone for the
//! value. Its words are held at 4.5:1 on the terminal's fill as every part's words are.

use super::{Family, Look, Part, Prop, State, TRANSPARENT};

/// What the foot's words take: their colour and size.
const WORDS: &[Prop] = &[Prop::Text, Prop::FontSize];

/// **The pane foot** (#5202): where a chat runs, and its gauge.
pub(super) const PANE_FOOT: Family = Family {
    name: "pane-foot",
    description: "The quiet line under a chat's terminal: the harness and the workspace it runs \
                  in, and its context and cache gauge.",
    since: super::NEXT,
    parts: &[
        Part {
            name: "root",
            since: super::NEXT,
            description: "The line: its fill, any line a theme draws on it, its words, the room \
                          around and between its groups.",
            properties: &[
                Prop::Fill,
                Prop::Text,
                Prop::BorderColour,
                Prop::BorderWidth,
                Prop::Padding,
                Prop::Gap,
                Prop::FontSize,
            ],
        },
        Part {
            name: "start",
            since: super::NEXT,
            description: "Where the chat runs, at the line's start: the room between its parts.",
            properties: &[Prop::Gap],
        },
        Part {
            name: "end",
            since: super::NEXT,
            description: "The gauge, at the line's end: the room between its parts.",
            properties: &[Prop::Gap],
        },
        Part {
            name: "separator",
            since: super::NEXT,
            description: "The `·` between two parts of the line.",
            properties: &[Prop::Text],
        },
        Part {
            name: "gauge",
            since: super::NEXT,
            description: "One gauge: its label and its value, and for `ctx` the bar under them: \
                          the room between them.",
            properties: &[Prop::Gap],
        },
        Part {
            name: "label",
            since: super::NEXT,
            description: "A gauge's name: `ctx`, `cache`.",
            properties: WORDS,
        },
        Part {
            name: "value",
            since: super::NEXT,
            description: "A gauge's value in figures of one width.",
            properties: WORDS,
        },
        Part {
            name: "track",
            since: super::NEXT,
            description: "The thin bar under `ctx`'s number: its fill and its corners.",
            properties: &[Prop::Fill, Prop::CornerRadius],
        },
        Part {
            name: "fill",
            since: super::NEXT,
            description: "How full the context is, on the track, in the core's tone for it.",
            properties: &[Prop::Fill],
        },
    ],
    variants: &[],
    // The terminal's own fill, which a chat's pane is from edge to edge.
    grounds: &["terminal.background"],
    keeps_size: &[],
    sized_apart: &[],
    height: None,
    font_size: "font-size.xs",
    line_height: 1.0,
    border_width: 0.0,
    defaults: pane_foot_look,
    states: &[],
    focused: "[data-focus-visible]",
    // The line's words in the muted tier; a gauge's value a step up, in the secondary one.
    words: &[
        ("separator", "text.muted"),
        ("label", "text.muted"),
        ("value", "text.secondary"),
    ],
    corner_most: None,
    around: &[],
    // The track's own fill, the subtle hairline's colour.
    fills: &[("track", "border.subtle")],
    under: &[],
};

/// **The foot's colours as `pane-foot.css` draws them**: no ground of its own (the terminal's
/// shows), its words in the muted tier, and no line above it.
pub(super) fn pane_foot_look(_variant: &str, _states: &[State]) -> Look {
    Look {
        fill: TRANSPARENT,
        text: "text.muted",
        border: TRANSPARENT,
    }
}

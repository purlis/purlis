// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The panel separator family** (#3143, spec #2897): the handle between two panels that a
//! person drags to resize them, declared once here and once in its wrapper,
//! `app/src/ui/panel/Separator.tsx`, whose stylesheet (`panel.css`) draws the defaults
//! [`super::Look`] and [`separator_fill`] hold a copy of.
//!
//! Every resize handle of the window is one: a side's edge, the line under the work, the line
//! between the bottom panel's halves, a split between two panes and the file tree's edge. Its
//! parts are the handle itself (the root, which carries the state and is as big as the place a
//! person can grab) and its grip, a short mark at its middle drawn over what is either side of
//! it, so a wider look never changes where the handle is or what it takes up.
//!
//! **A separator is drawn in marks, not words**: what it draws in a state is held at 3:1 on what
//! it is drawn on (WCAG 1.4.11), and it has no words to hold ([`super::MARKED`]). Its line at
//! rest is the window's hairline between two regions, which the token floors hold as every
//! hairline is.
//!
//! **Its states are the panel library's**, not React Aria's: the library marks the handle under
//! the pointer and the one being dragged on its own `data-separator` ([`OWN_STATES`]). The
//! keyboard's ring is the window's own, on every separator.

use super::{Family, Look, Part, Prop, State, TRANSPARENT};

/// **The panel separator** (#3143): the handle between two panels.
pub(super) const PANEL_SEPARATOR: Family = Family {
    name: "panel-separator",
    description: "The handle between two panels that resizes them: a side's edge, the line under \
                  the work, the line between the bottom panel's halves, a split between panes.",
    since: 12,
    parts: &[
        Part {
            name: "root",
            since: 12,
            description: "The handle: its line, as long as the panels it parts.",
            properties: &[Prop::Fill],
        },
        Part {
            name: "grip",
            since: 12,
            description: "The short mark at the handle's middle, drawn under the pointer and \
                          while it is dragged.",
            properties: &[Prop::Fill],
        },
    ],
    variants: &[],
    // The window's frame a floating window's gutter shows, and a panel's surface: the base one,
    // and the raised one a theme floats its panels on (Purlis, #2913).
    grounds: &["surface.deep", "surface.base", "surface.raised"],
    keeps_size: &[],
    sized_apart: &[],
    height: None,
    font_size: "font-size.base",
    line_height: 1.0,
    border_width: 0.0,
    defaults: separator_look,
    // Under the pointer and dragged: what the panel library marks (`OWN_STATES`).
    states: &[State::Hover, State::Pressed],
    focused: "[data-focus-visible]",
    words: &[],
    corner_most: None,
    around: &[],
    fills: &[],
};

/// **What the panel library marks a handle with in each state** (#3143): under the pointer, and
/// dragged. The library's own hit area is wider than the handle, so it, and not `:hover`, says
/// when the pointer is on it; but a handle a press has given the keyboard is marked `focus`
/// from then on, so under the pointer it is that handle's own box that says so.
pub(super) const OWN_STATES: &[(State, &str)] = &[
    (
        State::Hover,
        ":is([data-separator=\"hover\"], [data-separator=\"focus\"]:hover)",
    ),
    (State::Pressed, "[data-separator=\"active\"]"),
];

/// **The handle's colours as `panel.css` draws them**: no fill of its own in any state (its line
/// at rest is the window's hairline, `App.css`), and no words.
pub(super) fn separator_look(_variant: &str, _states: &[State]) -> Look {
    Look {
        fill: TRANSPARENT,
        text: "text.primary",
        border: TRANSPARENT,
    }
}

/// **Each part's fill as `panel.css` draws it while `states` hold** (#3143): the handle draws
/// none; the grip is none at rest, the faint tier under the pointer, and the muted tier while it
/// is dragged: a quiet mark, then a clear one, and never the accent.
pub(super) fn separator_fill(part: usize, states: &[State]) -> &'static str {
    if part == 0 {
        return TRANSPARENT;
    }
    if states.contains(&State::Pressed) {
        "text.muted"
    } else if states.contains(&State::Hover) {
        "text.faint"
    } else {
        TRANSPARENT
    }
}

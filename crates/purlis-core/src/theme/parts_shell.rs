// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The window's own families** (#2913, spec #2897): the region panel, a panel's section, an
//! empty state and a status item, each declared once here and once in its wrapper under
//! `app/src/ui/`, whose stylesheet draws the defaults [`super::Look`] holds a copy of.
//!
//! A panel, a section and an empty state are boxes as tall as what they hold (`height: None`):
//! no target of their own, so [`super::fits`] holds only their bounds, and each of their corners is
//! clamped to half their own box. A status item is a control, drawn 24px tall in either density
//! (R115), and holds the least target like a button.

use super::{ALL, Family, Length, Look, Part, Prop, State, TRANSPARENT, Variant};

/// The properties a box (a panel, a section's root, an empty state's root) may set: its fill,
/// edge, corners and shadow, and the room inside it.
const PANEL_ROOT: &[Prop] = &[
    Prop::Fill,
    Prop::BorderColour,
    Prop::BorderWidth,
    Prop::CornerRadius,
    Prop::Shadow,
];

/// What a box that only lays out what it holds may set: its room and its gaps.
const ROOM: &[Prop] = &[Prop::Padding, Prop::Gap];

/// A part's words: their colour, size and weight.
const WORDS: &[Prop] = &[Prop::Text, Prop::FontSize, Prop::FontWeight];

/// **A region panel** (#2805, #2913): a side panel, the content, a half of the bottom panel. The
/// window's gutter and the panels' corners are the theme's `window.gutter` and
/// `panel.corner-radius`; a part sets one variant's on top.
pub(super) const PANEL: Family = Family {
    name: "panel",
    description: "A region of the window: a side panel, the content, a half of the bottom panel.",
    since: 4,
    parts: &[Part {
        name: "root",
        since: 4,
        description: "The panel's surface: its fill, edge, corners and shadow.",
        properties: PANEL_ROOT,
    }],
    variants: &[
        Variant {
            name: "side",
            since: 4,
            description: "A side panel, left or right.",
            consent: None,
        },
        Variant {
            name: "content",
            since: 4,
            description: "The content: the chats and the views open in the middle.",
            consent: None,
        },
        Variant {
            name: "bottom",
            since: 4,
            description: "A half of the bottom panel.",
            consent: None,
        },
    ],
    // Drawn on the window's frame, which its gutter shows.
    grounds: &["surface.deep"],
    keeps_size: &[],
    sized_apart: &[],
    height: None,
    font_size: "font-size.base",
    line_height: 1.4,
    border_width: 0.0,
    defaults: panel_look,
    states: &[],
    focused: "[data-focus-visible]",
    words: &[],
    corner_most: None,
};

/// **The panel's colours as `app/src/ui/panel/panel.css` draws them**: the base surface, the
/// window's words, the subtle hairline.
fn panel_look(_variant: &str, _states: &[State]) -> Look {
    Look {
        fill: "surface.base",
        text: "text.primary",
        border: "border.subtle",
    }
}

/// **A section of a panel**: its heading line and what it holds under it. Its title is the
/// section's name in the label's capitals; the line between two sections is a separator, not a
/// part of it.
pub(super) const PANEL_SECTION: Family = Family {
    name: "panel-section",
    description: "A section of a panel: its heading line, its title and what it holds.",
    since: 4,
    parts: &[
        Part {
            name: "root",
            since: 4,
            description: "The section: the room around what it holds.",
            properties: ROOM,
        },
        Part {
            name: "head",
            since: 4,
            description: "Its heading line: the title and the section's actions.",
            properties: ROOM,
        },
        Part {
            name: "title",
            since: 4,
            description: "Its title.",
            properties: WORDS,
        },
    ],
    variants: &[Variant {
        name: "default",
        since: 4,
        description: "Every section.",
        consent: None,
    }],
    // The base ground, and the raised one a theme puts its panels on (Purlis, #2913).
    grounds: &["surface.base", "surface.raised"],
    keeps_size: &[],
    sized_apart: &[],
    height: None,
    font_size: "font-size.xs",
    line_height: 1.4,
    border_width: 0.0,
    defaults: section_look,
    states: &[],
    focused: "[data-focus-visible]",
    words: &[],
    corner_most: None,
};

/// **A section's colours as `panel.css` draws them**: no ground of its own, its title in the
/// secondary tier.
fn section_look(_variant: &str, _states: &[State]) -> Look {
    Look {
        fill: TRANSPARENT,
        text: "text.secondary",
        border: TRANSPARENT,
    }
}

/// **An empty state**: what a surface says when there is nothing in it, and the way out.
pub(super) const EMPTY_STATE: Family = Family {
    name: "empty-state",
    description: "What a surface says when it holds nothing, and what to do about it.",
    since: 4,
    parts: &[
        Part {
            name: "root",
            since: 4,
            description: "The empty state: the room around it and between its lines.",
            properties: ROOM,
        },
        Part {
            name: "icon",
            since: 4,
            description: "The mark above it.",
            properties: &[Prop::Text],
        },
        Part {
            name: "headline",
            since: 4,
            description: "Its one line: what is true.",
            properties: WORDS,
        },
        Part {
            name: "body",
            since: 4,
            description: "The sentence under it: what goes here.",
            properties: &[Prop::Text, Prop::FontSize],
        },
        Part {
            name: "action",
            since: 4,
            description: "Its way out: the buttons under it, and the room between them.",
            properties: &[Prop::Gap],
        },
    ],
    variants: &[
        Variant {
            name: "page",
            since: 4,
            description: "In the middle of the content.",
            consent: None,
        },
        Variant {
            name: "panel",
            since: 4,
            description: "At a side panel's width.",
            consent: None,
        },
    ],
    grounds: &["surface.base", "surface.raised"],
    keeps_size: &[],
    sized_apart: &[],
    height: None,
    font_size: "font-size.md",
    line_height: 1.4,
    border_width: 0.0,
    defaults: empty_look,
    states: &[],
    focused: "[data-focus-visible]",
    words: &[],
    corner_most: None,
};

/// **An empty state's colours as `empty-state.css` draws them**: no ground, its sentence and its
/// mark in the muted tier (its headline a tier up, which the floors measure as the lower).
fn empty_look(_variant: &str, _states: &[State]) -> Look {
    Look {
        fill: TRANSPARENT,
        text: "text.muted",
        border: TRANSPARENT,
    }
}

/// **A status item** (N28, R115): a cell of the status bar, or a pill at the title bar's right.
/// What can be pressed runs a command; what cannot is its words. 24px tall in either density.
pub(super) const STATUS_ITEM: Family = Family {
    name: "status-item",
    description: "A status bar item, or a status pill at the title bar's right.",
    since: 4,
    parts: &[
        Part {
            name: "root",
            since: 4,
            description: "The item itself: its fill, border, corners and size.",
            properties: ALL,
        },
        Part {
            name: "icon",
            since: 4,
            description: "Its icon.",
            properties: &[Prop::Text],
        },
        Part {
            name: "label",
            since: 4,
            description: "Its words.",
            properties: WORDS,
        },
        Part {
            name: "count",
            since: 4,
            description: "Its count.",
            properties: &[Prop::Text],
        },
    ],
    variants: &[
        Variant {
            name: "bar",
            since: 4,
            description: "A cell of the status bar: no box until the pointer reaches it.",
            consent: None,
        },
        Variant {
            name: "pill",
            since: 4,
            description: "A status pill at the title bar's right: full corners, quiet words.",
            consent: None,
        },
        Variant {
            name: "waiting",
            since: 4,
            description: "The pill that says what waits on the person.",
            consent: None,
        },
        Variant {
            name: "accent",
            since: 4,
            description: "The pill that is news: an update on offer.",
            consent: None,
        },
    ],
    grounds: &["surface.deep"],
    keeps_size: &[],
    sized_apart: &[],
    height: Some(Length::Number(24.0)),
    font_size: "font-size.sm",
    line_height: 1.0,
    border_width: 0.0,
    defaults: status_look,
    // What its wrapper marks: under the pointer, pressed, the keyboard's focus, unavailable.
    states: &[
        State::Hover,
        State::Pressed,
        State::Focused,
        State::Disabled,
    ],
    focused: "[data-focus-visible]",
    words: &[],
    corner_most: None,
};

/// **A status item's colours as `app/src/ui/status/status.css` draws them**, at rest and under
/// the pointer.
fn status_look(variant: &str, states: &[State]) -> Look {
    let mut look = match variant {
        "pill" => Look {
            fill: TRANSPARENT,
            text: "text.muted",
            border: TRANSPARENT,
        },
        "accent" => Look {
            fill: "accent.surface",
            text: "text.primary",
            border: TRANSPARENT,
        },
        "waiting" => Look {
            fill: "wash.warning",
            text: "text.primary",
            border: TRANSPARENT,
        },
        _ => Look {
            fill: TRANSPARENT,
            text: "text.primary",
            border: TRANSPARENT,
        },
    };
    // Under the pointer, whatever else holds at once (pressed, the keyboard's focus): a status
    // item draws no look of its own for those.
    if states.contains(&State::Hover) {
        look.fill = if variant == "accent" {
            "list.selected"
        } else {
            "surface.hover"
        };
    }
    look
}

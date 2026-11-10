// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The view frame and its page navigation** (#3666, spec #2897, the Skill studio frame's
//! variant A): a view tab laid out as pages, declared once here and once in its wrapper,
//! `app/src/ui/panel/ViewFrame.tsx`, whose stylesheet (`panel.css`) draws the defaults
//! [`super::Look`] holds a copy of.
//!
//! Two families, as the activity bar and its items are: the frame (`view-frame`), which lays out
//! the panes and draws no state, and a page in its navigation (`view-nav`), which is a control
//! and carries the states. A frame is a box as tall as the tab (`height: None`); a page in the
//! navigation is a control as tall as the control size, and holds the least target.

use super::{Family, Length, Look, Part, Prop, State, TRANSPARENT};

/// What a pane may set: its fill and the line that parts it from the next.
const PANE: &[Prop] = &[Prop::Fill, Prop::BorderColour, Prop::BorderWidth];

/// What a box that only lays out what it holds may set: its room and its gaps.
const ROOM: &[Prop] = &[Prop::Padding, Prop::Gap];

/// **The view frame** (#3666): the navigation, the page's list and the item beside it.
pub(super) const VIEW_FRAME: Family = Family {
    name: "view-frame",
    description: "A view tab laid out as pages: the page navigation, the page's list, and the \
                  item chosen in it beside it.",
    since: 13,
    parts: &[
        Part {
            name: "root",
            since: 13,
            description: "The frame: the panes side by side, or one at a time when it is narrow.",
            properties: &[Prop::Fill],
        },
        Part {
            name: "nav",
            since: 13,
            description: "The page navigation's column: its fill, the line after it and the \
                          room around its pages.",
            properties: &[
                Prop::Fill,
                Prop::BorderColour,
                Prop::BorderWidth,
                Prop::Padding,
            ],
        },
        Part {
            name: "list",
            since: 13,
            description: "The page's list pane: its fill and the line between it and the item.",
            properties: PANE,
        },
        Part {
            name: "item",
            since: 13,
            description: "The pane of the item chosen in the list.",
            properties: &[Prop::Fill],
        },
        Part {
            name: "head",
            since: 13,
            description: "The heading over a page's list and its lead line: the room around them.",
            properties: ROOM,
        },
        Part {
            name: "heading",
            since: 14,
            description: "The line of a page's heading and the one action beside it (#5332): \
                          the Library's Select.",
            properties: &[Prop::Gap],
        },
        Part {
            name: "header",
            since: 13,
            description: "The item's page header, held in view over its body: its name, tag, \
                          mark, description and actions.",
            properties: &[
                Prop::Fill,
                Prop::BorderColour,
                Prop::BorderWidth,
                Prop::Padding,
                Prop::Gap,
            ],
        },
        Part {
            name: "name",
            since: 13,
            description: "The item's name in its page header.",
            properties: &[Prop::Text, Prop::FontSize, Prop::FontWeight],
        },
        Part {
            name: "lead",
            since: 13,
            description: "The one line under a page's heading or an item's name that says what \
                          it is.",
            properties: &[Prop::Text, Prop::FontSize],
        },
        Part {
            name: "bar",
            since: 13,
            description: "The view's top bar over its panes (#3667): back and forward, the path \
                          and the view's own controls, over a line.",
            properties: &[
                Prop::Fill,
                Prop::BorderColour,
                Prop::BorderWidth,
                Prop::Padding,
                Prop::Gap,
            ],
        },
        Part {
            name: "history",
            since: 13,
            description: "The back and forward buttons at the bar's start: the room between them.",
            properties: &[Prop::Gap],
        },
        Part {
            name: "path",
            since: 13,
            description: "The breadcrumb of the view's pages: its words' colour and size.",
            properties: &[Prop::Text, Prop::FontSize],
        },
        Part {
            name: "crumb",
            since: 13,
            description: "The last name on the path: where the person is.",
            properties: &[Prop::Text, Prop::FontWeight],
        },
        Part {
            name: "crumb-separator",
            since: 13,
            description: "The mark between two names on the path.",
            properties: &[Prop::Text],
        },
    ],
    variants: &[],
    // The panel it is drawn in: the base surface, and the raised one a theme floats its panels on.
    grounds: &["surface.base", "surface.raised"],
    keeps_size: &[],
    sized_apart: &[],
    height: None,
    font_size: "font-size.md",
    line_height: 1.4,
    border_width: 0.0,
    defaults: view_frame_look,
    states: &[],
    focused: "[data-focus-visible]",
    // The lead line in the muted tier; the item's name in the primary one.
    words: &[
        ("lead", "text.muted"),
        ("name", "text.primary"),
        ("path", "text.muted"),
        ("crumb", "text.primary"),
        ("crumb-separator", "text.muted"),
    ],
    corner_most: None,
    around: &[],
    fills: &[],
    under: &[],
    later: &[],
};

/// **The frame's colours as `panel.css` draws them**: no ground of its own (the panel's shows),
/// the window's words, and the subtle hairline between its panes.
pub(super) fn view_frame_look(_variant: &str, _states: &[State]) -> Look {
    Look {
        fill: TRANSPARENT,
        text: "text.primary",
        border: "border.subtle",
    }
}

/// **A page in a view frame's navigation** (#3666): its name, its count and its mark.
pub(super) const VIEW_NAV: Family = Family {
    name: "view-nav",
    description: "A page in a view frame's navigation: its name, its count, and the mark that \
                  says it is the current page.",
    since: 13,
    parts: &[
        Part {
            name: "item",
            since: 13,
            description: "The page itself: its fill, its words' colour and its corners.",
            properties: &[Prop::Fill, Prop::Text, Prop::CornerRadius],
        },
        Part {
            name: "label",
            since: 13,
            description: "The page's name, cut short with an ellipsis rather than widening it.",
            properties: &[Prop::FontSize, Prop::FontWeight],
        },
        Part {
            name: "indicator",
            since: 13,
            description: "The mark at the current page's start edge, which tells it apart by more \
                          than its colour.",
            properties: &[Prop::Fill, Prop::CornerRadius],
        },
        Part {
            name: "toggle",
            since: super::NEXT,
            description: "The button at the end of a page that heads a group of pages, apart \
                          from the page's own, which opens and folds the group (Settings' nav, \
                          #2919): its fill, its chevron's colour and its corners.",
            properties: &[Prop::Fill, Prop::Text, Prop::CornerRadius],
        },
    ],
    variants: &[],
    grounds: &["surface.base", "surface.raised"],
    keeps_size: &[],
    sized_apart: &[],
    height: Some(Length::Token("size.control")),
    font_size: "font-size.md",
    line_height: 1.0,
    border_width: 0.0,
    defaults: view_nav_look,
    // What its wrapper marks: under the pointer, pressed, the current page, the keyboard on it.
    states: &[
        State::Hover,
        State::Pressed,
        State::Selected,
        State::Focused,
    ],
    focused: "[data-focus-visible]",
    // A group's toggle draws its chevron in the quiet words (`panel.css`).
    words: &[("toggle", "text.quiet")],
    corner_most: None,
    around: &[],
    fills: &[],
    under: &[],
    later: &[],
};

/// **A page's colours as `panel.css` draws them**: muted words at rest; the pointer's ground and
/// the primary words under the pointer; the accent's wash and the accent's words on the current
/// page, which the pointer does not change.
pub(super) fn view_nav_look(_variant: &str, states: &[State]) -> Look {
    if states.contains(&State::Selected) {
        return Look {
            fill: "accent.wash",
            text: "accent.text",
            border: TRANSPARENT,
        };
    }
    if states.contains(&State::Hover) {
        return Look {
            fill: "surface.hover",
            text: "text.primary",
            border: TRANSPARENT,
        };
    }
    Look {
        fill: TRANSPARENT,
        text: "text.muted",
        border: TRANSPARENT,
    }
}

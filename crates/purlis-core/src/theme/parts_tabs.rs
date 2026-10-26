// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The session tabs and the editor's toolbar** (#2914): the strip, each tab and the panel the
//! chosen tab joins, and the bar over a file its file tab joins, declared once here and once in
//! their wrappers, `app/src/ui/tabs/`.

use super::{Family, Length, Part, Prop, State, Variant, editor_look, tabs_look_in};

pub(super) const TABS: Family = Family {
    name: "tabs",
    description: "The session tabs: the strip, each tab, and the panel the chosen tab joins.",
    since: 8,
    parts: &[
        Part {
            name: "tab",
            since: 8,
            description: "One tab: its fill, words, corners and size; the chosen tab's flare and \
                      the hover pill's inset.",
            properties: &[
                Prop::Fill,
                Prop::Text,
                Prop::CornerRadius,
                Prop::Padding,
                Prop::Gap,
                Prop::FontSize,
                Prop::FontWeight,
                Prop::Height,
                Prop::MaxWidth,
                Prop::Flare,
                Prop::Inset,
            ],
        },
        Part {
            name: "label",
            since: 8,
            description: "The tab's name, cut short with an ellipsis rather than widening it.",
            properties: &[Prop::Text, Prop::FontSize, Prop::FontWeight],
        },
        Part {
            name: "closer",
            since: 8,
            description: "The tab's ×.",
            properties: &[Prop::Fill, Prop::Text, Prop::CornerRadius],
        },
        Part {
            name: "count",
            since: 8,
            description: "A count on a tab: how many of its tasks are working or waiting.",
            properties: &[Prop::Fill, Prop::Text, Prop::CornerRadius],
        },
        Part {
            name: "indicator",
            since: 8,
            description: "The mark on the chosen tab's edge: none until a theme gives it a height.",
            properties: &[Prop::Fill, Prop::Height, Prop::CornerRadius, Prop::Position],
        },
        Part {
            name: "strip",
            since: 8,
            description: "The row the tabs sit in: its ground and the line under it.",
            properties: &[Prop::Fill, Prop::BorderColour, Prop::BorderWidth],
        },
        Part {
            name: "layout",
            since: 8,
            description: "How the tabs are laid out: what they attach to, how they share the \
                      strip, the lines between them and the room between them.",
            properties: &[Prop::Attach, Prop::Sizing, Prop::Dividers, Prop::Gap],
        },
    ],
    variants: &[
        Variant {
            name: "chat",
            since: 8,
            description: "A chat's tab.",
            consent: None,
        },
        Variant {
            name: "view",
            since: 8,
            description: "A view's tab: a persona, a memory, a vault, a setting.",
            consent: None,
        },
        Variant {
            name: "file",
            since: 8,
            description: "A file's tab, which joins the editor's toolbar under it.",
            consent: None,
        },
        Variant {
            name: "background",
            since: 8,
            description: "A chat wrapping up in the background, drawn as a chip at the strip's \
                      start.",
            consent: None,
        },
    ],
    // What the strip is drawn on: the window behind a docked strip, Classic's own strip, and the
    // panel the chosen tab takes the fill of.
    grounds: &[
        "surface.base",
        "surface.deep",
        "surface.raised",
        "layer.chat",
        "terminal.background",
    ],
    keeps_size: &[],
    sized_apart: &[],
    height: Some(Length::Token("size.control")),
    font_size: "font-size.md",
    line_height: 1.0,
    border_width: 0.0,
    defaults: tabs_look_in,
    states: &[State::Hover, State::Selected],
    focused: "[data-focus-visible]",
    words: &[],
    around: &["strip", "layout"],
    corner_most: None,
    fills: &[],
};

pub(super) const EDITOR: Family = Family {
    name: "editor",
    description: "An editor view: the toolbar over a file, joined by its file tab.",
    since: 8,
    parts: &[Part {
        name: "toolbar",
        since: 8,
        description: "The bar over a file that names it and holds its actions; its fill is the \
                  chosen file tab's.",
        properties: &[
            Prop::Fill,
            Prop::Text,
            Prop::Padding,
            Prop::Gap,
            Prop::FontSize,
        ],
    }],
    variants: &[Variant {
        name: "default",
        since: 8,
        description: "The toolbar.",
        consent: None,
    }],
    grounds: &["surface.raised", "terminal.background"],
    keeps_size: &[],
    sized_apart: &[],
    height: None,
    font_size: "font-size.md",
    line_height: 1.0,
    border_width: 0.0,
    defaults: editor_look,
    states: &[],
    focused: "[data-focus-visible]",
    words: &[],
    around: &[],
    corner_most: None,
    fills: &[],
};

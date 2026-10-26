// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The dialog family** (#2910, spec #2897): a dialog and an alert dialog, declared once here
//! and once in its wrapper, `app/src/ui/dialog/Dialog.tsx`, whose stylesheet (`dialog.css`) draws
//! the defaults [`super::Look`] holds a copy of.
//!
//! A dialog is a surface, not a target: a box as tall as what it holds (`height: None`), whose
//! size, height, place and rows are the dialog rules', never a theme's. Its corner is held to the
//! inset of what it holds ([`DIALOG_CORNER_MOST`]), so a rounded dialog never cuts off a word or
//! an answer in a corner. Its quieter lines are measured in the tier each is drawn in
//! ([`DIALOG_TIERS`]).

use super::{Family, Look, NEVER_REFUSED, Part, Prop, State, Variant};

/// **The largest corner a dialog takes**: the inset of what it holds (`dialog.css`'s rows are
/// `space.6` in from its edge), so `full` is that and never a pill.
const DIALOG_CORNER_MOST: &str = "space.6";

/// **The dialog's quieter words as `dialog.css` draws them**, which a test holds to that file: the
/// line under the title muted, the question secondary and the footer's summary quiet.
pub(crate) const DIALOG_TIERS: &[(&str, &str)] = &[
    ("subtitle", "text.muted"),
    ("description", "text.secondary"),
    ("summary", "text.quiet"),
];

/// **A dialog** (#2910): its surface, its title, the lines that describe it and the rule over its
/// footer.
pub(super) const DIALOG: Family = Family {
    name: "dialog",
    description: "A dialog or an alert dialog: its surface, its title and its footer.",
    since: 7,
    parts: &[
        Part {
            name: "root",
            since: 7,
            description: "The dialog's surface: its fill, border, corners and lift.",
            properties: &[
                Prop::Fill,
                Prop::BorderColour,
                Prop::BorderWidth,
                Prop::CornerRadius,
                Prop::Shadow,
            ],
        },
        Part {
            name: "title",
            since: 7,
            description: "Its title, in the header.",
            properties: &[Prop::Text, Prop::FontSize, Prop::FontWeight],
        },
        Part {
            name: "subtitle",
            since: 7,
            description: "The line under the title: where it is, what it is about.",
            properties: &[Prop::Text],
        },
        Part {
            name: "description",
            since: 7,
            description: "What the question is, said first in the body.",
            properties: &[Prop::Text],
        },
        Part {
            name: "separator",
            since: 7,
            description: "The rule over the footer.",
            properties: &[Prop::Fill],
        },
        Part {
            name: "summary",
            since: 7,
            description: "What the footer says before its answers.",
            properties: &[Prop::Text],
        },
    ],
    variants: &[
        Variant {
            name: "dialog",
            since: 7,
            description: "A dialog: a form, a choice, a view of something.",
            consent: None,
        },
        Variant {
            name: "alert",
            since: 7,
            description: "An alert dialog: a question that interrupts.",
            consent: None,
        },
    ],
    grounds: &["surface.base"],
    keeps_size: &[],
    sized_apart: &[],
    height: None,
    font_size: "font-size.lg",
    line_height: 1.2,
    border_width: 1.0,
    defaults: dialog_look,
    states: &NEVER_REFUSED,
    focused: "[data-focus-visible]",
    words: DIALOG_TIERS,
    corner_most: Some(DIALOG_CORNER_MOST),
    around: &[],
    fills: &[],
};

/// **The dialog's colours as `app/src/ui/dialog/dialog.css` draws them**, which a test holds to
/// that file: the overlay surface, its words and its border, in every state alike.
pub(super) fn dialog_look(_variant: &str, _states: &[State]) -> Look {
    Look {
        fill: "surface.overlay",
        text: "text.primary",
        border: "border.subtle",
    }
}

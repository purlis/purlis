// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The button family** (#2904, spec #2897): a button, an icon button or a link button,
//! declared once here and once in its wrapper, `app/src/ui/button/`, whose stylesheet
//! (`button.css`) draws the defaults [`super::Look`] holds a copy of.

use super::{ALL, CONTROL_GROUNDS, Consent, Family, Length, NEVER_REFUSED, Part, Prop, Variant, button_look_in};

pub(super) const BUTTON: Family = Family {
    name: "button",
    description: "A button, an icon button or a link button.",
    since: 1,
    parts: &[
        Part {
            name: "root",
            since: 1,
            description: "The button itself: its fill, border, corners and size.",
            properties: ALL,
        },
        Part {
            name: "label",
            since: 1,
            description: "Its words.",
            properties: &[Prop::Text, Prop::FontSize, Prop::FontWeight],
        },
        Part {
            name: "icon",
            since: 1,
            description: "Its icon.",
            properties: &[Prop::Text],
        },
        Part {
            name: "shortcut",
            since: super::NEXT,
            description: "The key that presses it, after its words.",
            properties: &[Prop::Text, Prop::FontSize],
        },
    ],
    variants: &[
        Variant {
            name: "secondary",
            since: 1,
            description: "The plain button.",
            consent: None,
        },
        Variant {
            name: "primary",
            since: 1,
            description: "The answer purlis suggests.",
            consent: None,
        },
        Variant {
            name: "ghost",
            since: 1,
            description: "A button with no box until the pointer reaches it.",
            consent: None,
        },
        Variant {
            name: "danger",
            since: 1,
            description: "What cannot be undone.",
            consent: Some(Consent::Danger),
        },
        Variant {
            name: "grants",
            since: 1,
            description: "An answer that grants: an Allow, an Accept.",
            consent: Some(Consent::Grants),
        },
        Variant {
            name: "soft-accent",
            since: 2,
            description: "A suggestion that is not the answer: the accent's ground, no edge.",
            consent: None,
        },
        Variant {
            name: "warning-soft",
            since: 2,
            description: "What asks for care without ending anything.",
            consent: None,
        },
        Variant {
            name: "link",
            since: 2,
            description: "A button that reads as a link: no box, as tall as its words.",
            consent: None,
        },
        Variant {
            name: "quiet",
            since: super::NEXT,
            description: "A ghost with quiet words: a strip's end mark, a crumb, a pane's corner.",
            consent: None,
        },
    ],
    grounds: CONTROL_GROUNDS,
    keeps_size: &[
        "[data-size=\"sm\"]",
        "[data-size=\"lg\"]",
        "[data-scope=\"button-row\"][data-wraps] > *",
    ],
    sized_apart: &[
        (
            "[data-scope=\"button\"][data-part=\"root\"][data-size=\"sm\"]",
            "max(24px, var(--size-chrome), var(--space-7))",
        ),
        (
            "[data-scope=\"button\"][data-part=\"root\"][data-size=\"lg\"]",
            "calc(var(--size-control) + var(--space-2))",
        ),
    ],
    height: Some(Length::Token("size.control")),
    font_size: "font-size.base",
    line_height: 1.0,
    border_width: 1.0,
    defaults: button_look_in,
    states: &NEVER_REFUSED,
    focused: "[data-focus-visible]",
    words: &[],
    corner_most: None,
    around: &[],
    fills: &[],
    under: &[],
};

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The activity item family** (#2917, spec #2897 R116–R120, N17): a view's icon on a side's bar,
//! declared once here and once in its wrapper, `app/src/ui/activity/`, whose stylesheet
//! (`activity.css`) draws the defaults [`super::Look`] holds a copy of.
//!
//! Two variants: `bar`, an icon in a side's 36px column, which both corners draw since #3144
//! (the left column beside the project rail, the right one at the window's edge), and `tab`, 32×30
//! tabs across the top of a side panel, which no bar of the window draws now and a theme may still
//! name, as the manifest never takes a variant away. Its parts are the item itself (the root, which
//! carries the variant and the state), its icon, its count and an indicator: a mark the item
//! draws while it is chosen, which the Purlis look leaves empty and Purlis Classic paints as its
//! accent edge.

use super::{Family, Length, Look, Part, Prop, State, TRANSPARENT, Variant};

/// What a theme may set on the item: its colours, its edge and its corners. Its size is the
/// spec's (R116, R117) and the same in every theme, so no neighbour moves when a theme changes.
const ITEM: &[Prop] = &[
    Prop::Fill,
    Prop::Text,
    Prop::BorderColour,
    Prop::Shadow,
    Prop::BorderWidth,
    Prop::CornerRadius,
];

/// **An activity item** (R116–R120, #3144): an icon in a side's column, or a tab across the top of
/// a side panel. A top item switches its side's panel; a bottom one opens its half of the
/// bottom panel.
pub(super) const ACTIVITY: Family = Family {
    name: "activity",
    description: "A view's icon on a side's activity bar: an icon in the side's column, or a \
                  tab across the top of a side panel.",
    since: 10,
    parts: &[
        Part {
            name: "item",
            since: 10,
            description: "The item itself: its fill, its words' and icon's colour, its edge and \
                          its corners.",
            properties: ITEM,
        },
        Part {
            name: "icon",
            since: 10,
            description: "Its view's mark.",
            properties: &[Prop::Text],
        },
        Part {
            name: "count",
            since: 10,
            description: "The count at its top right: what waits in its view.",
            properties: &[Prop::Fill, Prop::Text],
        },
        Part {
            name: "indicator",
            since: 10,
            description: "The mark along its edge while it is chosen: none by default.",
            properties: &[Prop::Fill],
        },
    ],
    variants: &[
        Variant {
            name: "tab",
            since: 10,
            description: "A tab across the top of a side panel, 32 by 30. No bar of the \
                          window draws it since the left bar became a column.",
            consent: None,
        },
        Variant {
            name: "bar",
            since: 10,
            description: "An icon in a side's column, 36 square: the left bar beside the \
                          project rail and the right bar at the window's edge.",
            consent: None,
        },
    ],
    // The tabs on a side panel (the base surface, or the raised one a theme floats its panels
    // on), the right bar on the window's frame.
    grounds: &["surface.deep", "surface.base", "surface.raised"],
    keeps_size: &[],
    sized_apart: &[],
    // The tab's 30px, the lesser of the two, which a corner is clamped to.
    height: Some(Length::Number(30.0)),
    font_size: "font-size.xs",
    line_height: 1.0,
    border_width: 0.0,
    defaults: activity_look,
    // What its wrapper marks: under the pointer, pressed, its view open, the keyboard on it.
    states: &[
        State::Hover,
        State::Pressed,
        State::Selected,
        State::Focused,
    ],
    focused: "[data-focus-visible]",
    // The count's words and its own pill (R119): read on that pill, never on the item.
    words: &[("count", "text.secondary")],
    fills: &[("count", "surface.hover")],
    under: &[],
    corner_most: None,
    around: &[],
};

/// **An item's colours as `app/src/ui/activity/activity.css` draws them** (R118): muted at rest,
/// the card's fill and the primary words under the pointer, the accent's wash and the accent's
/// words while its view is open, which the pointer does not change.
fn activity_look(_variant: &str, states: &[State]) -> Look {
    if states.contains(&State::Selected) {
        return Look {
            fill: "accent.wash",
            text: "accent.text",
            border: TRANSPARENT,
        };
    }
    if states.contains(&State::Hover) {
        return Look {
            fill: "surface.overlay",
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

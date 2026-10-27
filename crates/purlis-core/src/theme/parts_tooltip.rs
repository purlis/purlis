// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The tooltip family** (#2908): every hint in the window, `app/src/ui/tooltip`. One look, so a
//! key names the part alone; no state a theme keys, since nothing on it is pressed or chosen.

use super::{Family, Part, Prop, SURFACE, tooltip_look};

/// **The tooltip** (#2908).
pub(super) const TOOLTIP: Family = Family {
    name: "tooltip",
    description: "A hint beside what it is about, under the pointer or the keyboard.",
    since: 5,
    parts: &[
        Part {
            name: "root",
            since: 5,
            description: "The tooltip's surface: its fill, words, edge, corners and shadow. \
                      Its size is its words'.",
            properties: SURFACE,
        },
        Part {
            name: "detail",
            since: 5,
            description: "A second line under an icon button's name.",
            properties: &[Prop::Text],
        },
        Part {
            name: "shortcut",
            since: super::NEXT,
            description: "The key that does what its target does, after its words.",
            properties: &[Prop::Text, Prop::FontSize],
        },
    ],
    variants: &[],
    // Drawn over the window, wherever its target is: on the grounds an overlay stands on.
    grounds: &["surface.base", "surface.raised", "surface.overlay"],
    keeps_size: &[],
    sized_apart: &[],
    height: None,
    font_size: "font-size.sm",
    line_height: 1.4,
    border_width: 1.0,
    defaults: tooltip_look,
    states: &[],
    focused: "[data-focus-visible]",
    words: &[("detail", "text.secondary"), ("shortcut", "text.quiet")],
    corner_most: None,
    around: &[],
    fills: &[],
    under: &[],
};

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Page tabs** (#3668, spec #2897, the skill page prototype's variant A): the tabs inside a
//! page that switch its panels, a member of the tabs family declared once here and once in its
//! wrapper, `app/src/ui/tabs/PageTabs.tsx`, whose stylesheet (`tabs.css`) draws the defaults
//! [`super::Look`] holds a copy of.
//!
//! Its root is a tab, which carries React Aria's state attributes; the row the tabs sit in and
//! the chosen tab's panel are drawn around it. The chosen tab is told by its indicator as well as
//! its words' colour, and no state changes a size.

use super::{Family, Length, Look, Part, Prop, State, TRANSPARENT};

/// **Page tabs** (#3668): a page's tabs and the chosen tab's panel.
pub(super) const PAGE_TABS: Family = Family {
    name: "page-tabs",
    description: "The tabs inside a page that switch its panels: a skill's Overview, Text, \
                  History, Grades and Runs.",
    since: super::NEXT,
    parts: &[
        Part {
            name: "tab",
            since: super::NEXT,
            description: "One tab: its words' colour, at rest and in each state.",
            properties: &[Prop::Text, Prop::Fill, Prop::CornerRadius],
        },
        Part {
            name: "label",
            since: super::NEXT,
            description: "The tab's words.",
            properties: &[Prop::FontSize, Prop::FontWeight],
        },
        Part {
            name: "count",
            since: super::NEXT,
            description: "The count after a tab's words: how many it holds.",
            properties: &[Prop::Text, Prop::FontSize],
        },
        Part {
            name: "indicator",
            since: super::NEXT,
            description: "The bar under the chosen tab, which tells it apart by more than its \
                          colour.",
            properties: &[Prop::Fill, Prop::Height, Prop::CornerRadius],
        },
        Part {
            name: "list",
            since: super::NEXT,
            description: "The row the tabs sit in: the line under it and the room between its \
                          tabs.",
            properties: &[Prop::BorderColour, Prop::BorderWidth, Prop::Gap, Prop::Padding],
        },
        Part {
            name: "panel",
            since: super::NEXT,
            description: "The chosen tab's panel: the room around what it holds.",
            properties: &[Prop::Padding],
        },
    ],
    variants: &[],
    // The panel a page is drawn in: the base surface, and the raised one a theme floats it on.
    grounds: &["surface.base", "surface.raised"],
    keeps_size: &[],
    sized_apart: &[],
    height: Some(Length::Token("size.control")),
    font_size: "font-size.md",
    line_height: 1.0,
    border_width: 0.0,
    defaults: page_tabs_look,
    // What React Aria marks on a tab: under the pointer, chosen, the keyboard on it.
    states: &[State::Hover, State::Selected, State::Focused],
    focused: "[data-focus-visible]",
    // The count's words, in the quiet tier.
    words: &[("count", "text.quiet")],
    corner_most: None,
    around: &["list", "panel"],
    fills: &[],
    under: &[],
};

/// **A tab's colours as `tabs.css` draws them**: muted words at rest, the primary words under
/// the pointer and on the chosen tab, and no ground of its own in any state.
pub(super) fn page_tabs_look(_variant: &str, states: &[State]) -> Look {
    let lit = states.contains(&State::Selected) || states.contains(&State::Hover);
    Look {
        fill: TRANSPARENT,
        text: if lit { "text.primary" } else { "text.muted" },
        border: TRANSPARENT,
    }
}

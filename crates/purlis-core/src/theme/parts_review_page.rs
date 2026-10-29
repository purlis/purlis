// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The review page** (#3672, spec #2897, a member of the panel family): a page in a view tab
//! whose body scrolls over a fixed action bar, declared once here and once in its wrapper,
//! `app/src/ui/review-page/ReviewPage.tsx`, whose stylesheet (`review-page.css`) draws the
//! defaults [`super::Look`] holds a copy of.
//!
//! The bar says what pressing its main action does and what it spends, then holds at most three
//! answers in a dialog's places, which are buttons of the button family. **Its edge is not a
//! theme's**: the bar is a consent surface where it holds a grant, so the line over it keeps the
//! window's hairline and no part here sets a border. It draws no state.

use super::{Family, Look, Part, Prop, State, TRANSPARENT};

/// **The review page** (#3672): the install review, the update review, the grade plan.
pub(super) const REVIEW_PAGE: Family = Family {
    name: "review-page",
    description: "A page whose body scrolls over a fixed action bar: what pressing its main \
                  action does and spends, then its answers. The install review, the update \
                  review and the grade plan.",
    since: super::NEXT,
    parts: &[
        Part {
            name: "root",
            since: super::NEXT,
            description: "The page: its body over its bar, on the panel's fill.",
            properties: &[Prop::Fill],
        },
        Part {
            name: "body",
            since: super::NEXT,
            description: "What is reviewed, which scrolls: the room around and between what it \
                          holds.",
            properties: &[Prop::Padding, Prop::Gap],
        },
        Part {
            name: "bar",
            since: super::NEXT,
            description: "The action bar, always in view at the page's foot: its ground and its \
                          room. Its edge is the window's, never a theme's.",
            properties: &[Prop::Fill, Prop::Padding, Prop::Gap],
        },
        Part {
            name: "summary",
            since: super::NEXT,
            description: "The sentence at the bar's start: what pressing the main action does \
                          and what it spends.",
            properties: &[Prop::Text, Prop::FontSize],
        },
        Part {
            name: "answers",
            since: super::NEXT,
            description: "The bar's answers, in a dialog's places: the room between them.",
            properties: &[Prop::Gap],
        },
    ],
    variants: &[],
    // The panel it is drawn in: the base surface, and the raised one a theme floats it on.
    grounds: &["surface.base", "surface.raised"],
    keeps_size: &[],
    sized_apart: &[],
    height: None,
    font_size: "font-size.md",
    line_height: 1.4,
    border_width: 0.0,
    defaults: review_page_look,
    states: &[],
    focused: "[data-focus-visible]",
    // The summary in the muted tier, as the prototypes draw it.
    words: &[("summary", "text.muted")],
    corner_most: None,
    around: &[],
    fills: &[],
    under: &[],
};

/// **The page's colours as `review-page.css` draws them**: no ground of its own (the panel's
/// shows), the window's words, and the subtle hairline over the bar.
pub(super) fn review_page_look(_variant: &str, _states: &[State]) -> Look {
    Look {
        fill: TRANSPARENT,
        text: "text.primary",
        border: "border.subtle",
    }
}

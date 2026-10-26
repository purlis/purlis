// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The families that hold what the window says** (#2912, spec #2897): a card, a Notice, a badge,
//! a count and a separator, each declared once here and once in its wrapper under `app/src/ui/`,
//! whose stylesheet draws the defaults [`super::Look`] holds a copy of.
//!
//! None of them is a target: each is a box as tall as what it holds (`height: None`), so
//! [`super::fits`] holds only their bounds and each corner is clamped to half the box.
//!
//! - **A card and a Notice hold answers** (an ask's Allow, a Notice's fix), so their fill is one of
//!   the grounds the button family is measured on ([`HOLDS_ANSWERS`]): an answer that grants is
//!   held apart from the primary on exactly those. A fill of none stands it on the surface under
//!   it, whose own fill a theme sets is held as that surface's family holds it (#3083).
//! - **An ask keeps an edge that shows** ([`super::Consent::Asks`]): the card that asks the
//!   person something and a Notice's box, as Allow keeps its accent edge. A theme may colour it,
//!   but an edge that would not show (none, or lost on its fill or its ground) or that is drawn
//!   nearer the accent than the family's own draws the family default, the waiting colour; and
//!   no theme may take a Notice's edge width.
//! - **A separator is its own part, never a control's border** (#2804, #2877): its colour is the
//!   `separator` family's, its width the theme's `separator.width` geometry, 0 allowed.

use super::{Consent, Family, Look, Part, Prop, State, TRANSPARENT, Variant};

/// **The families whose fill stands under answers**: a card and a Notice. Their fill is one of
/// the button family's grounds, or the theme's key is refused.
pub(super) const HOLDS_ANSWERS: &[&str] = &["card", "notice"];

/// What a word's part may set: its colour, size and weight.
const WORDS: &[Prop] = &[Prop::Text, Prop::FontSize, Prop::FontWeight];

/// **A card**: a box one step off the surface it stands on, holding a question, a report or a
/// group of rows.
pub(super) const CARD: Family = Family {
    name: "card",
    description: "A box one step off the surface it stands on: an ask, a question, a group of \
                  rows.",
    since: super::NEXT,
    parts: &[
        Part {
            name: "root",
            since: super::NEXT,
            description: "The card: its fill, words, edge, corners, lift and the room inside it.",
            properties: &[
                Prop::Fill,
                Prop::Text,
                Prop::BorderColour,
                Prop::BorderWidth,
                Prop::CornerRadius,
                Prop::Shadow,
                Prop::Padding,
                Prop::Gap,
            ],
        },
        Part {
            name: "title",
            since: super::NEXT,
            description: "Its first line, where it has one: whose it is, what it asks.",
            properties: WORDS,
        },
    ],
    variants: &[
        Variant {
            name: "raised",
            since: super::NEXT,
            description: "A card: a question, a report, a group of rows.",
            consent: None,
        },
        Variant {
            name: "ask",
            since: super::NEXT,
            description: "A card that asks the person something, edged in the waiting colour \
                          in every theme.",
            consent: Some(Consent::Asks),
        },
    ],
    // The Inbox's side and a chat's pane: the base ground, and the raised one a theme puts its
    // panels on (Purlis, #2913).
    grounds: &["surface.base", "surface.raised"],
    keeps_size: &[],
    sized_apart: &[],
    height: None,
    font_size: "font-size.sm",
    line_height: 1.4,
    border_width: 1.0,
    defaults: card_look,
    states: &[],
    focused: "[data-focus-visible]",
    words: &[],
    corner_most: None,
    around: &[],
    fills: &[],
};

/// **A card's colours as `app/src/ui/card/card.css` draws them**: the raised ground behind the
/// subtle hairline, an ask's edge in the waiting colour.
pub(super) fn card_look(variant: &str, _states: &[State]) -> Look {
    Look {
        fill: "surface.raised",
        text: "text.primary",
        border: if variant == "ask" {
            "state.waiting"
        } else {
            "border.subtle"
        },
    }
}

/// **A Notice** (CONTEXT.md, NO-1): a standing line about something true now, with its ways out.
/// Its root is its box in a pane, its row in the Inbox's list and the inline surface on a page;
/// where it stands is the window's (`data-placement`), what it looks like the theme's.
pub(super) const NOTICE: Family = Family {
    name: "notice",
    description: "A standing line about something true now, with its ways out: in a pane's row, \
                  in the Inbox's list, on a page.",
    since: super::NEXT,
    parts: &[
        Part {
            name: "root",
            since: super::NEXT,
            description: "The Notice's box: its fill, words, edge colour, corners and the room \
                          inside it. Its edge's width is the window's: no theme takes it away.",
            properties: &[
                Prop::Fill,
                Prop::Text,
                Prop::BorderColour,
                Prop::CornerRadius,
                Prop::Padding,
                Prop::Gap,
            ],
        },
        Part {
            name: "message",
            since: super::NEXT,
            description: "What it says.",
            properties: &[Prop::Text, Prop::FontSize],
        },
        Part {
            name: "whose",
            since: super::NEXT,
            description: "Whose it is, before what it says, for a chat that is not on screen.",
            properties: &[Prop::Text, Prop::FontWeight],
        },
    ],
    variants: &[
        Variant {
            name: "news",
            since: super::NEXT,
            description: "Nothing is wrong: the person is being told. Its waiting edge stays.",
            consent: Some(Consent::Asks),
        },
        Variant {
            name: "trouble",
            since: super::NEXT,
            description: "Something went wrong. Its waiting edge stays.",
            consent: Some(Consent::Asks),
        },
    ],
    // A pane's row on the base ground; the Inbox's side and a settings page, base or raised.
    grounds: &["surface.base", "surface.raised"],
    keeps_size: &[],
    sized_apart: &[],
    height: None,
    font_size: "font-size.sm",
    line_height: 1.4,
    border_width: 1.0,
    defaults: notice_look,
    states: &[],
    focused: "[data-focus-visible]",
    words: &[],
    corner_most: None,
    around: &[],
    fills: &[],
};

/// **A Notice's colours as `app/src/ui/notice/notice.css` draws them**: the raised ground edged in
/// the waiting colour (purlis speaking, #1647), and trouble's danger ground and words.
pub(super) fn notice_look(variant: &str, _states: &[State]) -> Look {
    if variant == "trouble" {
        Look {
            fill: "danger.surface",
            text: "danger.text",
            border: "state.waiting",
        }
    } else {
        Look {
            fill: "surface.raised",
            text: "text.primary",
            border: "state.waiting",
        }
    }
}

/// What a badge's and a count's root may set: colours, an edge, corners, room and type.
const MARK: &[Prop] = &[
    Prop::Fill,
    Prop::Text,
    Prop::BorderColour,
    Prop::BorderWidth,
    Prop::CornerRadius,
    Prop::Padding,
    Prop::FontSize,
    Prop::FontWeight,
];

/// **A badge**: a word or two beside what it is about, a level, a state ("Waiting for you",
/// "Confirmed", R66).
pub(super) const BADGE: Family = Family {
    name: "badge",
    description: "A word or two beside what it is about: where a value comes from, a state.",
    since: super::NEXT,
    parts: &[Part {
        name: "root",
        since: super::NEXT,
        description: "The badge: its fill, words, edge, corners and size.",
        properties: MARK,
    }],
    variants: &[
        Variant {
            name: "neutral",
            since: super::NEXT,
            description: "A word that says what something is.",
            consent: None,
        },
        Variant {
            name: "accent",
            since: super::NEXT,
            description: "Where a value comes from: the level it is set at.",
            consent: None,
        },
        Variant {
            name: "waiting",
            since: super::NEXT,
            description: "What waits on the person: \"Waiting for you\", \"deprecated\".",
            consent: None,
        },
        Variant {
            name: "ok",
            since: super::NEXT,
            description: "What is settled: \"Confirmed\".",
            consent: None,
        },
    ],
    grounds: &["surface.base", "surface.raised", "surface.overlay"],
    keeps_size: &[],
    sized_apart: &[],
    height: None,
    font_size: "font-size.xs",
    line_height: 1.4,
    border_width: 0.0,
    defaults: badge_look,
    states: &[],
    focused: "[data-focus-visible]",
    words: &[],
    corner_most: None,
    around: &[],
    fills: &[],
};

/// **A badge's colours as `app/src/ui/badge/badge.css` draws them**: a pill on the count's ground,
/// or on a wash in the colour of what it says.
pub(super) fn badge_look(variant: &str, _states: &[State]) -> Look {
    let (fill, text) = match variant {
        "accent" => ("accent.surface", "text.primary"),
        "waiting" => ("wash.warning", "state.waiting"),
        "ok" => (TRANSPARENT, "state.success"),
        _ => ("control.count", "text.secondary"),
    };
    Look {
        fill,
        text,
        border: TRANSPARENT,
    }
}

/// **A count**: a number on a pill, beside what it counts (R46, R79, R101, R119).
pub(super) const COUNT: Family = Family {
    name: "count",
    description: "A number on a pill beside what it counts: a view's items, what waits.",
    since: super::NEXT,
    parts: &[
        Part {
            name: "root",
            since: super::NEXT,
            description: "The count: its fill, number, edge, corners and size.",
            properties: MARK,
        },
        Part {
            name: "group",
            since: super::NEXT,
            description: "Counts drawn as one pill: its corners.",
            properties: &[Prop::CornerRadius],
        },
    ],
    variants: &[
        Variant {
            name: "neutral",
            since: super::NEXT,
            description: "How many there are.",
            consent: None,
        },
        Variant {
            name: "accent",
            since: super::NEXT,
            description: "How many are new.",
            consent: None,
        },
        Variant {
            name: "waiting",
            since: super::NEXT,
            description: "How many wait on the person, in the waiting colour.",
            consent: None,
        },
        Variant {
            name: "hot",
            since: super::NEXT,
            description: "How many need the person now, in the needs-you colours.",
            consent: None,
        },
    ],
    grounds: &[
        "surface.base",
        "surface.raised",
        "surface.overlay",
        "surface.deep",
    ],
    keeps_size: &[],
    sized_apart: &[],
    height: None,
    font_size: "font-size.xs",
    line_height: 1.2,
    border_width: 0.0,
    defaults: count_look,
    states: &[],
    focused: "[data-focus-visible]",
    words: &[],
    corner_most: None,
    around: &[],
    fills: &[],
};

/// **A count's colours as `app/src/ui/badge/badge.css` draws them.**
pub(super) fn count_look(variant: &str, _states: &[State]) -> Look {
    let (fill, text) = match variant {
        "accent" => ("accent.surface", "text.primary"),
        "waiting" => ("wash.warning", "state.waiting"),
        "hot" => ("needs-you.base", "needs-you.text"),
        _ => ("control.count", "text.secondary"),
    };
    Look {
        fill,
        text,
        border: TRANSPARENT,
    }
}

/// **A separator**: the line between two groups inside a surface, its own part and never a
/// control's border. Its width is the theme's `separator.width`; it has no words.
pub(super) const SEPARATOR: Family = Family {
    name: "separator",
    description: "The line between two groups inside a surface. Its width is the theme's \
                  separator.width, 0 for none.",
    since: super::NEXT,
    parts: &[Part {
        name: "root",
        since: super::NEXT,
        description: "The line: its colour.",
        properties: &[Prop::Fill],
    }],
    variants: &[],
    grounds: &["surface.base", "surface.raised", "surface.overlay"],
    keeps_size: &[],
    sized_apart: &[],
    height: None,
    font_size: "font-size.xs",
    line_height: 1.0,
    border_width: 0.0,
    defaults: separator_look,
    states: &[],
    focused: "[data-focus-visible]",
    words: &[],
    corner_most: None,
    around: &[],
    fills: &[],
};

/// **A separator's colour as `app/src/ui/separator/separator.css` draws it**: the subtle hairline.
/// A line has no words, so none is measured on it.
pub(super) fn separator_look(_variant: &str, _states: &[State]) -> Look {
    Look {
        fill: "border.subtle",
        text: TRANSPARENT,
        border: TRANSPARENT,
    }
}

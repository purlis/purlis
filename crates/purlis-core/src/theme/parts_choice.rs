// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The choice families** (#2907, spec #2897): the checkbox, the radio, the switch, the segmented
//! control and its segment, the slider and the option card, each declared once here and once in
//! its wrapper under `app/src/ui/choice/`, whose stylesheet (`choice.css`) draws the defaults
//! [`super::Look`] holds a copy of.
//!
//! A choice's root is the row a person presses (a checkbox's label, a segment, a card), which
//! carries its state: ticked, picked or on is [`State::Selected`]. None of them is pressed or
//! open as a part, so a key naming either is no part. Each is as tall as what it holds
//! (`height: None`), at least the target size its stylesheet keeps, so a theme sets its colours,
//! its corners and the room between what it holds, never a height. Its marks (a box, a dot, a
//! thumb, a check) take colours and corners; their size is the family's, so a tick never moves
//! anything.

use super::{Family, Look, Part, Prop, State, TRANSPARENT, Variant};

/// The states a choice's wrapper marks: under the pointer, ticked or picked, the keyboard on it,
/// and unavailable.
const CHOSEN: &[State] = &[
    State::Hover,
    State::Selected,
    State::Focused,
    State::Disabled,
];

/// The grounds a small choice is drawn on: every one a control stands on, as the button's.
const CONTROL_GROUNDS: &[&str] = &[
    "surface.base",
    "surface.raised",
    "surface.overlay",
    "surface.sunken",
    "surface.deep",
    "border.subtle",
    "tab.active",
    "danger.surface",
    "danger.wash",
    "layer.project",
    "layer.workspace",
    "layer.chat",
    "layer.selected",
    "terminal.background",
];

/// What a choice's row may set: its words' colour and the room between its mark and its words.
const ROW: &[Prop] = &[Prop::Text, Prop::Gap];

/// A part's words: their colour, size and weight.
const WORDS: &[Prop] = &[Prop::Text, Prop::FontSize, Prop::FontWeight];

/// A line under a choice's words: its colour and its size.
const LINE: &[Prop] = &[Prop::Text, Prop::FontSize];

/// A mark a choice is told by (a box, a dot's ring, a track): its fill, its edge and its corners.
const MARK: &[Prop] = &[
    Prop::Fill,
    Prop::BorderColour,
    Prop::BorderWidth,
    Prop::CornerRadius,
];

/// A mark that is only a shape (a thumb, a fill): its fill and its corners.
const SHAPE: &[Prop] = &[Prop::Fill, Prop::CornerRadius];

/// **The look of every small choice's row**, as `choice.css` draws it: no ground and no edge of
/// its own, its words primary, and faint while unavailable.
fn row_look(states: &[State]) -> Look {
    Look {
        fill: TRANSPARENT,
        text: if states.contains(&State::Disabled) {
            "text.faint"
        } else {
            "text.primary"
        },
        border: TRANSPARENT,
    }
}

/// **A checkbox** (#2907): its row, its box, its words and the line under them.
pub(super) const CHECKBOX: Family = Family {
    name: "checkbox",
    description: "A checkbox: its row, the box ticked, its words and the line under them.",
    since: super::NEXT,
    parts: &[
        Part {
            name: "root",
            since: super::NEXT,
            description: "The row pressed: its words' colour, its ground and corners as a chip.",
            properties: &[Prop::Fill, Prop::Text, Prop::CornerRadius, Prop::Gap],
        },
        Part {
            name: "box",
            since: super::NEXT,
            description: "The box ticked: its fill, its edge and its corners.",
            properties: MARK,
        },
        Part {
            name: "label",
            since: super::NEXT,
            description: "Its words.",
            properties: WORDS,
        },
        Part {
            name: "description",
            since: super::NEXT,
            description: "The line under its words on what it does.",
            properties: LINE,
        },
    ],
    variants: &[
        Variant {
            name: "box",
            since: super::NEXT,
            description: "A box ticked beside its words.",
            consent: None,
        },
        Variant {
            name: "chip",
            since: super::NEXT,
            description: "One of a few, small and side by side over the list they narrow.",
            consent: None,
        },
    ],
    grounds: CONTROL_GROUNDS,
    keeps_size: &[],
    sized_apart: &[],
    height: None,
    font_size: "font-size.base",
    line_height: 1.4,
    border_width: 0.0,
    defaults: checkbox_look,
    states: CHOSEN,
    focused: "[data-focus-visible]",
    words: &[("description", "text.quiet")],
    corner_most: None,
    around: &[],
    fills: &[],
    under: &[],
};

/// **A checkbox's colours as `choice.css` draws them**: a box's row has no ground of its own; a
/// chip stands on the control's ground in the secondary words, on the accent's wash in the
/// primary ones when it is ticked, and on the control's hover under the pointer.
pub(super) fn checkbox_look(variant: &str, states: &[State]) -> Look {
    if variant != "chip" {
        return row_look(states);
    }
    let selected = states.contains(&State::Selected);
    let fill = if selected {
        "accent.surface"
    } else if states.contains(&State::Hover) && !states.contains(&State::Disabled) {
        "control.hover"
    } else {
        "control.base"
    };
    let text = if states.contains(&State::Disabled) {
        "text.faint"
    } else if selected {
        "text.primary"
    } else {
        "text.secondary"
    };
    Look {
        fill,
        text,
        border: TRANSPARENT,
    }
}

/// **A radio** (#2907): one option of a radio group, its mark, its words and the line under them.
pub(super) const RADIO: Family = Family {
    name: "radio",
    description: "One option of a radio group: its row, the mark picked, its words and its line.",
    since: super::NEXT,
    parts: &[
        Part {
            name: "root",
            since: super::NEXT,
            description: "The row pressed: its words' colour and the room beside its mark.",
            properties: ROW,
        },
        Part {
            name: "mark",
            since: super::NEXT,
            description: "The circle picked: its fill and its edge.",
            properties: &[Prop::Fill, Prop::BorderColour, Prop::BorderWidth],
        },
        Part {
            name: "label",
            since: super::NEXT,
            description: "Its words.",
            properties: WORDS,
        },
        Part {
            name: "description",
            since: super::NEXT,
            description: "The line under its words on what it does.",
            properties: LINE,
        },
    ],
    variants: &[],
    grounds: CONTROL_GROUNDS,
    keeps_size: &[],
    sized_apart: &[],
    height: None,
    font_size: "font-size.base",
    line_height: 1.4,
    border_width: 0.0,
    defaults: radio_look,
    states: CHOSEN,
    focused: "[data-focus-visible]",
    words: &[("description", "text.quiet")],
    corner_most: None,
    around: &[],
    fills: &[],
    under: &[],
};

/// **A radio's colours as `choice.css` draws them**: its row's.
pub(super) fn radio_look(_variant: &str, states: &[State]) -> Look {
    row_look(states)
}

/// **A switch** (#2907, R63): its row, its track, its thumb and its words.
pub(super) const SWITCH: Family = Family {
    name: "switch",
    description: "A switch: its row, the track, the thumb that slides and its words.",
    since: super::NEXT,
    parts: &[
        Part {
            name: "root",
            since: super::NEXT,
            description: "The row pressed: its words' colour and the room beside its track.",
            properties: ROW,
        },
        Part {
            name: "track",
            since: super::NEXT,
            description: "The pill the thumb slides along: its fill and its corners.",
            properties: SHAPE,
        },
        Part {
            name: "thumb",
            since: super::NEXT,
            description: "The thumb, on the side it is switched to: its fill and its corners.",
            properties: SHAPE,
        },
        Part {
            name: "label",
            since: super::NEXT,
            description: "Its words.",
            properties: WORDS,
        },
    ],
    variants: &[],
    grounds: CONTROL_GROUNDS,
    keeps_size: &[],
    sized_apart: &[],
    height: None,
    font_size: "font-size.base",
    line_height: 1.4,
    border_width: 0.0,
    defaults: switch_look,
    states: CHOSEN,
    focused: "[data-focus-visible]",
    words: &[],
    corner_most: None,
    around: &[],
    fills: &[],
    under: &[],
};

/// **A switch's colours as `choice.css` draws them**: its row's.
pub(super) fn switch_look(_variant: &str, states: &[State]) -> Look {
    row_look(states)
}

/// **A segmented control** (#2907, R39): the ground its segments stand on, side by side.
pub(super) const SEGMENTED_CONTROL: Family = Family {
    name: "segmented-control",
    description: "A segmented control: the ground its segments stand on, side by side.",
    since: super::NEXT,
    parts: &[Part {
        name: "root",
        since: super::NEXT,
        description: "Its ground: its fill, its corners, its lift and the room around its segments.",
        properties: &[
            Prop::Fill,
            Prop::CornerRadius,
            Prop::Shadow,
            Prop::Padding,
            Prop::Gap,
        ],
    }],
    variants: &[],
    grounds: CONTROL_GROUNDS,
    keeps_size: &[],
    sized_apart: &[],
    height: None,
    font_size: "font-size.base",
    line_height: 1.0,
    border_width: 0.0,
    defaults: segmented_control_look,
    states: &[],
    focused: "[data-focus-visible]",
    words: &[],
    corner_most: None,
    around: &[],
    fills: &[],
    under: &[],
};

/// **A segmented control's colours as `choice.css` draws them**: the control's ground.
pub(super) fn segmented_control_look(_variant: &str, _states: &[State]) -> Look {
    Look {
        fill: "control.base",
        text: "text.primary",
        border: TRANSPARENT,
    }
}

/// **A segment** (#2907, R39): one view of a segmented control.
pub(super) const SEGMENT: Family = Family {
    name: "segment",
    description: "One segment of a segmented control: its ground, its words and its count.",
    since: super::NEXT,
    parts: &[
        Part {
            name: "root",
            since: super::NEXT,
            description: "The segment: its fill, its words' colour, its lift and its corners.",
            properties: &[Prop::Fill, Prop::Text, Prop::Shadow, Prop::CornerRadius],
        },
        Part {
            name: "label",
            since: super::NEXT,
            description: "Its words.",
            properties: &[Prop::Text],
        },
        Part {
            name: "count",
            since: super::NEXT,
            description: "How many things it stands for, after its words.",
            properties: &[Prop::Text],
        },
    ],
    variants: &[],
    // A segment stands on its segmented control's ground.
    grounds: &["control.base"],
    keeps_size: &[],
    sized_apart: &[],
    height: None,
    font_size: "font-size.base",
    line_height: 1.0,
    border_width: 0.0,
    defaults: segment_look,
    states: CHOSEN,
    focused: "[data-focus-visible]",
    words: &[],
    corner_most: None,
    around: &[],
    fills: &[],
    under: &[],
};

/// **A segment's colours as `choice.css` draws them**: no ground of its own in the quiet words;
/// under the pointer the control's hover in the primary ones; chosen, the window's ground in the
/// primary words; unavailable, faint.
pub(super) fn segment_look(_variant: &str, states: &[State]) -> Look {
    let disabled = states.contains(&State::Disabled);
    let (fill, text) = if states.contains(&State::Selected) {
        ("surface.base", "text.primary")
    } else if states.contains(&State::Hover) && !disabled {
        ("control.hover", "text.primary")
    } else {
        (TRANSPARENT, "text.quiet")
    };
    Look {
        fill,
        text: if disabled { "text.faint" } else { text },
        border: TRANSPARENT,
    }
}

/// **A slider** (#2907): its row, the line it moves along, the fill up to its value, its thumb
/// and the value beside them.
pub(super) const SLIDER: Family = Family {
    name: "slider",
    description: "A slider: its track, the fill up to its value, its thumb and its value.",
    since: super::NEXT,
    parts: &[
        Part {
            name: "root",
            since: super::NEXT,
            description: "The slider's row: its words' colour and the room beside its track.",
            properties: ROW,
        },
        Part {
            name: "track",
            since: super::NEXT,
            description: "The line it moves along: its fill and its corners.",
            properties: SHAPE,
        },
        Part {
            name: "fill",
            since: super::NEXT,
            description: "The track up to its value.",
            properties: SHAPE,
        },
        Part {
            name: "thumb",
            since: super::NEXT,
            description: "The thumb: its fill, its edge and its corners.",
            properties: &[Prop::Fill, Prop::BorderColour, Prop::CornerRadius],
        },
        Part {
            name: "output",
            since: super::NEXT,
            description: "Its value, beside the track.",
            properties: &[Prop::Text],
        },
    ],
    variants: &[],
    grounds: CONTROL_GROUNDS,
    keeps_size: &[],
    sized_apart: &[],
    height: None,
    font_size: "font-size.base",
    line_height: 1.4,
    border_width: 0.0,
    defaults: slider_look,
    // Its thumb, not its row, is under the pointer and the keyboard.
    states: &[State::Disabled],
    focused: "[data-focus-visible]",
    words: &[],
    corner_most: None,
    around: &[],
    fills: &[],
    under: &[],
};

/// **A slider's colours as `choice.css` draws them**: no ground, its value in the secondary
/// words, faint while unavailable.
pub(super) fn slider_look(_variant: &str, states: &[State]) -> Look {
    Look {
        fill: TRANSPARENT,
        text: if states.contains(&State::Disabled) {
            "text.faint"
        } else {
            "text.secondary"
        },
        border: TRANSPARENT,
    }
}

/// **An option card** (#2907, R24): one option a person compares before picking, its icon and
/// title, its detail and its check.
pub(super) const OPTION_CARD: Family = Family {
    name: "option-card",
    description: "An option card: its ground and edge, its icon, title, detail and check.",
    since: super::NEXT,
    parts: &[
        Part {
            name: "root",
            since: super::NEXT,
            description: "The card: its fill, its edge, its corners, its lift and its room.",
            properties: &[
                Prop::Fill,
                Prop::Text,
                Prop::BorderColour,
                Prop::Shadow,
                Prop::CornerRadius,
                Prop::Padding,
                Prop::Gap,
            ],
        },
        Part {
            name: "icon",
            since: super::NEXT,
            description: "Its mark, before its title; a tile is only this.",
            properties: &[Prop::Text],
        },
        Part {
            name: "title",
            since: super::NEXT,
            description: "Its name.",
            properties: WORDS,
        },
        Part {
            name: "detail",
            since: super::NEXT,
            description: "What it means, under its title.",
            properties: LINE,
        },
        Part {
            name: "check",
            since: super::NEXT,
            description: "The check the chosen card shows.",
            properties: &[Prop::Text],
        },
    ],
    variants: &[
        Variant {
            name: "card",
            since: super::NEXT,
            description: "A card: its icon and title, its detail under them, its check.",
            consent: None,
        },
        Variant {
            name: "tile",
            since: super::NEXT,
            description: "A square tile that is only its icon: a persona's mark, a colour.",
            consent: None,
        },
    ],
    grounds: &["surface.overlay", "surface.base", "surface.raised"],
    keeps_size: &[],
    sized_apart: &[],
    height: None,
    font_size: "font-size.base",
    line_height: 1.4,
    border_width: 1.5,
    defaults: option_card_look,
    states: CHOSEN,
    focused: "[data-focus-visible]",
    words: &[("detail", "text.quiet"), ("check", "accent.base")],
    corner_most: None,
    around: &[],
    fills: &[],
    under: &[],
};

/// **An option card's colours as `choice.css` draws them.** A card stands on the window's ground
/// in the secondary words, on the card fill (the control's ground) under the pointer; chosen, on the accent's wash
/// behind the accent's edge in the primary words. A tile has no ground of its own: its words are
/// muted, primary under the pointer and chosen, and chosen it takes the accent's edge. Unavailable,
/// either is faint.
pub(super) fn option_card_look(variant: &str, states: &[State]) -> Look {
    let selected = states.contains(&State::Selected);
    let disabled = states.contains(&State::Disabled);
    let hovered = states.contains(&State::Hover) && !disabled && !selected;
    let border = if selected { "accent.base" } else { TRANSPARENT };
    let (fill, text) = if variant == "tile" {
        (
            TRANSPARENT,
            if selected || hovered {
                "text.primary"
            } else {
                "text.muted"
            },
        )
    } else if selected {
        ("accent.surface", "text.primary")
    } else if hovered {
        ("control.base", "text.secondary")
    } else {
        ("surface.base", "text.secondary")
    };
    Look {
        fill,
        text: if disabled { "text.faint" } else { text },
        border,
    }
}

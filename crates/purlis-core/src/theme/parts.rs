// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Component parts** (spec #2897, #2903, ADR 0103): the second layer of a theme, beside its
//! tokens, that sets one named part of one component family: the button's root rounded to a pill,
//! a tab's indicator, a control's border turned off.
//!
//! # The manifest
//!
//! A **component part** is a named, stable element of a component family (the button's root,
//! label and icon). Each family declares its parts once, in the window's wrapper
//! (`data-scope="button" data-part="root"`) and here, in the engine's closed, versioned
//! [`MANIFEST`], which `purlis theme parts` prints ([`manifest_json`]) and the published schema
//! is generated from. A theme can name only the parts the manifest has.
//!
//! # The key grammar
//!
//! A theme's `parts` block holds keys `<family>.<part>[.<variant>][.<state>].<property>`:
//! `button.root.corner-radius`, `button.root.primary.hover.fill`. The state at rest is implied.
//! [`STATES`] and [`PROPERTIES`] are closed lists; a key that names something the manifest does
//! not have is said and left out, as an unknown token is, since a teammate's purlis may know it.
//!
//! # Values
//!
//! - **A colour names a semantic token** (`accent.base`), or `transparent`: never a hex or a
//!   palette name, so palettes, appearances, tints and the floors keep working.
//! - **A size is a number inside its bound, or a geometry token** (`radius.md`, `space.3`),
//!   which follows the person's density.
//! - **A corner is 0 up to `full`** (one value, or four: top left, top right, bottom right,
//!   bottom left), clamped as it is drawn to half the part's height, so `full` is a pill. A
//!   family may hold its corner to less (a dialog's, to the inset of what it holds, #2910).
//! - **Padding, gap, font size and height are bounded** so a control stays at least
//!   [`LEAST_TARGET`] px tall and its text fits, in both densities ([`fits`]).
//!
//! A value outside its type or bound refuses the theme (ADR 0097 §2), with a sentence that names
//! the bound. **A state never changes size**: inside a state only [`Prop::in_state`] properties
//! (colours, fills and shadows) may be set, and any other is refused with a sentence that says
//! why ([`NO_SIZE_IN_A_STATE`]).
//!
//! # What the window draws
//!
//! A property a theme leaves unset is the family default, which the window's own stylesheet
//! draws from the global tokens: **a theme that names no parts draws exactly as it did**, and
//! its [`stylesheet`] is empty. The engine writes the rest as one stylesheet keyed on
//! `data-scope` / `data-part`, the variant (`data-variant`) and React Aria's state attributes,
//! every value written here from its parsed form (ADR 0041, property 4).
//!
//! **The floors hold per part and per state** ([`fallen`]): each part's words on its fill, in
//! every variant and state the theme changes, on every ground the family is drawn on (4.5:1;
//! an icon 3:1), and a border that is a part's only edge at 3:1. A part that breaks one draws its
//! family default there. A disabled control is exempt, as WCAG has it.
//!
//! **The consent floors hold whatever the theme changed** (ADR 0097 §6), as written and in every
//! workspace tint: Allow's accent edge cannot be turned off (`border-width` 0 is refused on it,
//! and a theme that turns borders off everywhere still draws it), it contrasts 3:1 with its fill
//! and stays apart from Keep blocked's edge, and Allow never takes the primary look; danger's
//! words stay apart from the accent and danger never takes the primary look. Looks are compared
//! as drawn, by colour, never by the tokens' names. Where the consent variant's own default is
//! not enough, the variant it is held apart from draws its default too.
//!
//! **A size a theme sets is the family's default size** (md for the button): it never reaches a
//! control drawn at another of the family's sizes, nor one in a row that wraps, whose size the
//! family's own stylesheet keeps (#2904).

use std::collections::{BTreeMap, VecDeque};
use std::fmt::Write as _;
use std::sync::{Arc, Mutex, PoisonError};

use serde_json::{Map, Value, json};

use super::check::{contrast, is_opaque, onto, solid};
use super::geometry::{self, Geometry};
use super::tint;
use super::vocabulary::{PLATFORM, TOKENS};
use super::{Complaint, ResolvedTheme, stringify};

/// **The manifest's version: the last one a release shipped.** It grows once per release that adds
/// a family, a part, a variant, a state or a property, and the manifest never takes one away
/// (operator ruling 2026-10-25: one version per release, not one per pull request). A pull request
/// never changes it: what it adds is `since:` [`NEXT`], and the release step
/// (`node tools/theme-parts-release.mjs`, `docs/updating.md`) turns every [`NEXT`] into this
/// version plus one, raises this, and pins the new version's digest.
pub const VERSION: u32 = 12;

/// **The version what is added since the last release comes in**: a pull request that adds a
/// family, a part, a variant, a state or a property gives it `since: NEXT` (written
/// `super::NEXT` in a family's own file, and `parts::NEXT` in a test; never imported). It is a
/// marker: the release step replaces every one written in code with the number, and refuses a
/// stable tag while one is left. `purlis theme parts` lists such an addition with `since` one
/// above `version`, which is how a reader tells one no release has shipped yet.
pub const NEXT: u32 = VERSION + 1;

/// The least height a control keeps, in px at the default text size: WCAG 2.2's target size.
pub const LEAST_TARGET: f64 = 24.0;

/// Why a state may not change a size, as the refusal says it.
pub const NO_SIZE_IN_A_STATE: &str = "a state never changes a part's size, so nothing moves when \
     the pointer, a press, the keyboard or a choice reaches it; inside a state only fill, text, \
     border-colour and shadow may be set, and a tab's flare and hover inset, which are drawn \
     out of its flow and size nothing";

/// **One state of a part**, the state at rest left implied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum State {
    Hover,
    Pressed,
    Selected,
    Focused,
    Disabled,
    Open,
    /// A box whose value is refused (#3110, #3323): the text input's, the textarea's, the
    /// select's and the combobox's, whose wrappers mark it. A colour, never a size, as every
    /// state.
    Invalid,
}

/// Every state a key may name, in the order the stylesheet writes them.
pub const STATES: [State; 7] = [
    State::Hover,
    State::Pressed,
    State::Selected,
    State::Focused,
    State::Disabled,
    State::Open,
    State::Invalid,
];

/// **The states a control that is never refused names**: all but [`State::Invalid`], which only a
/// box a person gives a value to is in (the button's and the dialog's list).
pub(super) const NEVER_REFUSED: [State; 6] = [
    State::Hover,
    State::Pressed,
    State::Selected,
    State::Focused,
    State::Disabled,
    State::Open,
];

impl State {
    /// What a key names it by.
    pub fn name(self) -> &'static str {
        match self {
            Self::Hover => "hover",
            Self::Pressed => "pressed",
            Self::Selected => "selected",
            Self::Focused => "focused",
            Self::Disabled => "disabled",
            Self::Open => "open",
            Self::Invalid => "invalid",
        }
    }

    /// The attribute React Aria sets on a part in this state, which the stylesheet keys on.
    pub fn attribute(self) -> &'static str {
        match self {
            // An unavailable button is never drawn under the pointer or pressed (#2904).
            Self::Hover => "[data-hovered]:not([data-disabled])",
            Self::Pressed => "[data-pressed]:not([data-disabled])",
            Self::Selected => "[data-selected]",
            Self::Focused => "[data-focus-visible]",
            Self::Disabled => "[data-disabled]",
            Self::Open => "[data-open]",
            Self::Invalid => "[data-invalid]",
        }
    }

    /// The manifest version it came in.
    pub fn since(self) -> u32 {
        match self {
            Self::Invalid => NEXT,
            _ => 1,
        }
    }

    fn says(self) -> &'static str {
        match self {
            Self::Hover => "The pointer is over it.",
            Self::Pressed => "It is being pressed.",
            Self::Selected => "It is chosen, or a toggle that is on.",
            Self::Focused => "The keyboard is on it.",
            Self::Disabled => "It cannot be used now.",
            Self::Open => "What it opens is open.",
            Self::Invalid => "Its value is refused, and the words that say why describe it.",
        }
    }

    fn named(name: &str) -> Option<Self> {
        STATES.into_iter().find(|one| one.name() == name)
    }
}

/// **One property of a part**: the closed list. No opacity and no raw CSS.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Prop {
    Fill,
    Text,
    BorderColour,
    Shadow,
    BorderWidth,
    CornerRadius,
    Padding,
    Gap,
    FontSize,
    FontWeight,
    Height,
    MaxWidth,
    /// The inverse corners a chosen tab joins its panel with (#2914).
    Flare,
    /// How far an unchosen tab's hover pill stands in from its cell (#2914).
    Inset,
    /// A family's layout options (#2914): what its tabs attach to, how they share the strip,
    /// whether a line parts them, and which edge an indicator is drawn on.
    Attach,
    Sizing,
    Dividers,
    Position,
}

/// Every property, in the order the stylesheet writes them.
pub const PROPERTIES: [Prop; 18] = [
    Prop::Fill,
    Prop::Text,
    Prop::BorderColour,
    Prop::Shadow,
    Prop::BorderWidth,
    Prop::CornerRadius,
    Prop::Padding,
    Prop::Gap,
    Prop::FontSize,
    Prop::FontWeight,
    Prop::Height,
    Prop::MaxWidth,
    Prop::Flare,
    Prop::Inset,
    Prop::Attach,
    Prop::Sizing,
    Prop::Dividers,
    Prop::Position,
];

/// The most block padding a part takes, in px at the default text size.
const MOST_BLOCK: f64 = 12.0;

/// The bound a corner is held to, beside `full`, in px.
const MOST_CORNER: f64 = 9999.0;

impl Prop {
    /// What a key names it by.
    pub fn name(self) -> &'static str {
        match self {
            Self::Fill => "fill",
            Self::Text => "text",
            Self::BorderColour => "border-colour",
            Self::Shadow => "shadow",
            Self::BorderWidth => "border-width",
            Self::CornerRadius => "corner-radius",
            Self::Padding => "padding",
            Self::Gap => "gap",
            Self::FontSize => "font-size",
            Self::FontWeight => "font-weight",
            Self::Height => "height",
            Self::MaxWidth => "max-width",
            Self::Flare => "flare",
            Self::Inset => "inset",
            Self::Attach => "attach",
            Self::Sizing => "sizing",
            Self::Dividers => "dividers",
            Self::Position => "position",
        }
    }

    /// **Whether a state may set it**: colours, fills and shadows, never a size; and a tab's
    /// flare and hover inset, which are drawn out of its flow and size nothing (#2914).
    pub fn in_state(self) -> bool {
        matches!(
            self,
            Self::Fill | Self::Text | Self::BorderColour | Self::Shadow | Self::Flare | Self::Inset
        )
    }

    /// **The one state it is set in, where it has one**: a flare is the chosen tab's, and an
    /// inset the hover pill's (#2914).
    pub fn only_in(self) -> Option<State> {
        match self {
            Self::Flare => Some(State::Selected),
            Self::Inset => Some(State::Hover),
            _ => None,
        }
    }

    /// **The words it takes, where it is a choice** rather than a colour or a size (#2914).
    pub fn keywords(self) -> Option<&'static [&'static str]> {
        match self {
            Self::Attach => Some(&["panel", "strip"]),
            Self::Sizing => Some(&["content", "equal"]),
            Self::Dividers => Some(&["off", "on"]),
            Self::Position => Some(&["bottom", "top"]),
            _ => None,
        }
    }

    fn is_colour(self) -> bool {
        matches!(self, Self::Fill | Self::Text | Self::BorderColour)
    }

    fn named(name: &str) -> Option<Self> {
        PROPERTIES.into_iter().find(|one| one.name() == name)
    }

    /// Its number's bound, the unit it is written in, and the geometry tokens it may name
    /// instead, by their prefix: `None` for a colour or a shadow.
    fn bound(self) -> Option<Bound> {
        let bound = |least, most, unit, tokens| {
            Some(Bound {
                least,
                most,
                unit,
                tokens,
            })
        };
        match self {
            Self::BorderWidth => bound(0.0, 4.0, geometry::Unit::Px, None),
            Self::CornerRadius => bound(0.0, MOST_CORNER, geometry::Unit::Px, Some("radius.")),
            Self::Padding => bound(0.0, 32.0, geometry::Unit::Text, Some("space.")),
            Self::Gap => bound(0.0, 16.0, geometry::Unit::Text, Some("space.")),
            Self::FontSize => bound(10.0, 18.0, geometry::Unit::Text, Some("font-size.")),
            Self::FontWeight => bound(300.0, 700.0, geometry::Unit::Weight, Some("font-weight.")),
            Self::Height => bound(LEAST_TARGET, 48.0, geometry::Unit::Text, Some("size.")),
            Self::MaxWidth => bound(48.0, 480.0, geometry::Unit::Text, None),
            Self::Flare => bound(0.0, 16.0, geometry::Unit::Px, Some("radius.")),
            Self::Inset => bound(0.0, 8.0, geometry::Unit::Px, None),
            Self::Fill
            | Self::Text
            | Self::BorderColour
            | Self::Shadow
            | Self::Attach
            | Self::Sizing
            | Self::Dividers
            | Self::Position => None,
        }
    }

    /// The manifest version it came in.
    pub fn since(self) -> u32 {
        match self {
            Self::MaxWidth
            | Self::Flare
            | Self::Inset
            | Self::Attach
            | Self::Sizing
            | Self::Dividers
            | Self::Position => 8,
            _ => 1,
        }
    }

    fn says(self) -> &'static str {
        match self {
            Self::Fill => "Its fill: a semantic colour token, or transparent.",
            Self::Text => "Its words' and icons' colour: a semantic colour token.",
            Self::BorderColour => "Its border's colour: a semantic colour token, or transparent.",
            Self::Shadow => "Its shadow: an elevation step, none, raised or overlay.",
            Self::BorderWidth => "Its border's width in px, 0 for none.",
            Self::CornerRadius => {
                "Its corners: one value or four (top left, top right, bottom right, bottom \
                 left), each px, a radius token or full; drawn at most half its height."
            }
            Self::Padding => {
                "The room inside it: one value, or two (block, inline), each px at the default \
                 text size or a space token."
            }
            Self::Gap => {
                "The room between what it holds: px at the default text size or a space token."
            }
            Self::FontSize => "Its words' size: px at the default text size or a font-size token.",
            Self::FontWeight => "Its words' weight, or a font-weight token.",
            Self::Height => "Its height: px at the default text size or a size token.",
            Self::MaxWidth => "The widest it is drawn: px at the default text size.",
            Self::Flare => {
                "The inverse corners the chosen tab joins its panel with, in px or a radius \
                 token: 0 for none. Set only on the selected state."
            }
            Self::Inset => {
                "How far the hover pill stands in from the tab's cell: one value, or three (top, \
                 sides, bottom), each px. Set only on the hover state."
            }
            Self::Attach => {
                "What the tabs attach to: panel (the chosen tab joins the content panel under \
                 the strip) or strip (the tabs sit on a strip of their own)."
            }
            Self::Sizing => {
                "How the tabs share the strip: content (each as wide as its words) or \
                 equal (each an equal share)."
            }
            Self::Dividers => "Whether a line parts one tab from the next: on or off.",
            Self::Position => "The edge its mark is drawn on: top or bottom.",
        }
    }

    /// The CSS properties it is written to.
    fn css(self) -> &'static [&'static str] {
        match self {
            Self::Fill => &["background-color"],
            Self::Text => &["color"],
            Self::BorderColour => &["border-color"],
            Self::Shadow => &["box-shadow"],
            Self::BorderWidth => &["border-width"],
            Self::CornerRadius => &["border-radius"],
            Self::Padding => &["padding-block", "padding-inline"],
            Self::Gap => &["gap"],
            Self::FontSize => &["font-size"],
            Self::FontWeight => &["font-weight"],
            Self::Height => &["block-size"],
            Self::MaxWidth => &["max-inline-size"],
            // Drawn by the family's own stylesheet from custom properties (`written_to`), or a
            // word's declarations (`keyword_css`).
            Self::Flare
            | Self::Inset
            | Self::Attach
            | Self::Sizing
            | Self::Dividers
            | Self::Position => &[],
        }
    }
}

/// A size property's number bound.
#[derive(Debug, Clone, Copy)]
struct Bound {
    least: f64,
    most: f64,
    unit: geometry::Unit,
    tokens: Option<&'static str>,
}

/// Which consent a variant of a family carries (ADR 0097 §6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Consent {
    /// An answer that grants (an Allow, an Accept): its accent edge and its lock stay, and it
    /// never takes the primary look.
    Grants,
    /// What cannot be undone: its danger colour contrasts with its fill.
    Danger,
    /// A card that asks the person something (#2912): its waiting edge stays, 3:1 on its fill,
    /// whatever the theme says, as an ask contrasts with its surface (ADR 0097 §6).
    Asks,
}

impl Consent {
    /// Whether its root keeps its edge: an answer that grants, and an ask.
    fn keeps_edge(self) -> bool {
        matches!(self, Self::Grants | Self::Asks)
    }
}

/// One variant of a family.
#[derive(Debug, Clone, Copy)]
pub struct Variant {
    pub name: &'static str,
    /// The manifest version it came in.
    pub since: u32,
    pub description: &'static str,
    pub consent: Option<Consent>,
}

/// One part of a family.
#[derive(Debug, Clone, Copy)]
pub struct Part {
    pub name: &'static str,
    /// The manifest version it came in.
    pub since: u32,
    pub description: &'static str,
    /// The properties a theme may set on it.
    pub properties: &'static [Prop],
}

/// **One component family**: its parts, the first its root, which carries the variant and the
/// state; its variants; the manifest version it came in; and the grounds it is drawn on.
#[derive(Debug, Clone, Copy)]
pub struct Family {
    pub name: &'static str,
    pub description: &'static str,
    pub since: u32,
    pub parts: &'static [Part],
    pub variants: &'static [Variant],
    /// The surfaces it is drawn on, which a see-through fill shows: every one the window draws it
    /// on, found from the components (`app/src/theme/grounds.test.ts`).
    pub grounds: &'static [&'static str],
    /// The selectors of the roots drawn at another of the family's sizes, or in a row that keeps
    /// their size (a button in a wrapping row): a part's size never reaches them.
    keeps_size: &'static [&'static str],
    /// The roots drawn at another of the family's sizes, and the height each is drawn at, as the
    /// family's stylesheet writes it, which their corners are clamped to.
    sized_apart: &'static [(&'static str, &'static str)],
    /// The root's height at rest, which a corner is clamped to and a control keeps at least
    /// [`LEAST_TARGET`]: a size token, or a number of px it is drawn at in every density. `None`
    /// for a box that is as tall as what it holds (a panel, a section, an empty state), whose
    /// corners the browser keeps inside it and which is no target of its own.
    height: Option<Length>,
    /// Its words' size and line height at rest, and its border's width: what fits in its height.
    font_size: &'static str,
    line_height: f64,
    border_width: f64,
    /// Its colours at rest and in each set of states that hold at once, as the window's own
    /// stylesheet draws them.
    defaults: fn(&str, &[State]) -> Look,
    /// The states a key may name for it: the ones its wrapper marks (#2905). A text box is never
    /// pressed, chosen or open, and a field's root is only ever unavailable.
    pub states: &'static [State],
    /// The attribute its root is keyed on while it has the focus: React Aria's keyboard focus
    /// for a button, and for a field whenever it holds the caret, as a text box's `:focus-visible`
    /// has it (#2905).
    focused: &'static str,
    /// The words of a part that are not drawn in the root's colour, by the part's name: a field's
    /// help in the quiet tier and its error in the danger colour (#2905).
    words: &'static [(&'static str, &'static str)],
    /// The largest corner its root takes, a geometry token, where it holds what would be cut off
    /// beyond it: a dialog's answers in its corners (#2910). `None` for the corner its height or
    /// its box gives.
    corner_most: Option<&'static str>,
    /// **The parts drawn around its root rather than inside it** (#2914): a tab's strip, and the
    /// column that holds the strip and the panel its tabs attach to. Each is reached by its own
    /// attributes alone, and takes no variant and no state.
    pub around: &'static [&'static str],
    /// The fill of a part that is not the root's, by the part's name: an activity item's count on
    /// its own pill (#2917). Its words are held to this fill, and a fallback draws it again.
    fills: &'static [(&'static str, &'static str)],
    /// The ground a part's words are read on where the part paints none of its own and it is
    /// not the root's, by the part's name (#3099): a tab's count on the counts button its parent
    /// paints, a switcher's row on the popover it is drawn in. Its words are held to this ground,
    /// but a fallback never paints it, since the part does not.
    under: &'static [(&'static str, &'static str)],
}

impl Family {
    /// The attribute `state` is keyed on, on this family's root.
    fn attribute(&self, state: State) -> &'static str {
        if let Some((_, own)) = own_states(self).iter().find(|(one, _)| *one == state) {
            return own;
        }
        if state == State::Focused {
            self.focused
        } else {
            state.attribute()
        }
    }

    /// **The looks it is measured in**: each of its variants, or for a family that has none (a
    /// field, a text input), its one look, named `""`.
    fn looks(&self) -> Vec<&'static str> {
        if self.variants.is_empty() {
            vec![""]
        } else {
            self.variants.iter().map(|one| one.name).collect()
        }
    }

    /// The fill `part` is drawn on by default, when it has one of its own.
    fn own_fill(&self, part: usize) -> Option<&'static str> {
        let name = self.parts.get(part)?.name;
        self.fills
            .iter()
            .find(|(one, _)| *one == name)
            .map(|(_, token)| *token)
    }

    /// The colour `part`'s words are drawn in by default, for the tests that hold it to a
    /// family's stylesheet.
    #[cfg(test)]
    pub(super) fn own_words_of(&self, part: usize) -> Option<&'static str> {
        self.own_words(part)
    }

    /// What `part`'s words are read on by default, when it is not the root's fill: its own fill,
    /// or the ground it stands on ([`Family::under`]).
    fn own_ground(&self, part: usize) -> Option<&'static str> {
        let name = self.parts.get(part)?.name;
        self.own_fill(part).or_else(|| {
            self.under
                .iter()
                .find(|(one, _)| *one == name)
                .map(|(_, token)| *token)
        })
    }

    /// The colour `part`'s words are drawn in by default, when it is not the root's.
    fn own_words(&self, part: usize) -> Option<&'static str> {
        let name = self.parts.get(part)?.name;
        self.words
            .iter()
            .find(|(one, _)| *one == name)
            .map(|(_, token)| *token)
    }

    /// **The grounds `part`'s words are read on with no fill of their own**: the family's, or
    /// fewer where the window draws the part on fewer ([`PART_GROUNDS`], #3254).
    #[cfg(test)]
    pub(super) fn grounds_of(&self, part: usize) -> &'static [&'static str] {
        let name = self.parts.get(part).map(|one| one.name);
        PART_GROUNDS
            .iter()
            .find(|(family, one, _)| *family == self.name && Some(*one) == name)
            .map_or(self.grounds, |(_, _, grounds)| *grounds)
    }

    /// **Whether the window draws `variant` while `states` hold** (#3255): every look a key may
    /// name, unless one of its states is one only some variants are drawn in ([`DRAWN_ONLY`]).
    #[cfg(test)]
    pub(super) fn draws(&self, variant: &str, states: &[State]) -> bool {
        DRAWN_ONLY.iter().all(|(family, state, variants)| {
            *family != self.name || !states.contains(state) || variants.contains(&variant)
        })
    }
}

/// **The states only some of a family's variants are drawn in** (#3255), by family: a key may
/// still name the others (a theme's `button.root.danger.selected` is valid), but the window never
/// draws them, so the guards that walk the looks the window draws (`state_words_tests.rs`) leave
/// them out, and no pair holds a colour on a ground it is never drawn on.
/// - **A toggle is a plain or a ghost button** (ADR 0097 §6, #2958): the suggested answer, the
///   consent answers and the soft, warning and link looks are never pressed as one.
///   `ToggleButton` takes only these, and `app/src/theme/consentToggles.test.ts` holds every
///   other element that says it is pressed to them, reading this list.
/// - **A row that opens a submenu is a plain row**: the ways to end something are inside the
///   submenu, never the row that opens it (`docs/ui-primitives.md`, "Menus"), so a danger row is
///   never open. `app/src/ui/menu/submenuRows.guard.test.ts` holds every `Submenu`'s row to it.
#[cfg(test)]
const DRAWN_ONLY: &[(&str, State, &[&str])] = &[
    ("button", State::Selected, &["secondary", "ghost"]),
    ("menu", State::Open, &["default"]),
];

/// **The grounds a part's words are read on, where they are fewer than its family's** (#3254), by
/// family and part: what the window draws the part on, not every ground the family stands on.
#[cfg(test)]
const PART_GROUNDS: &[(&str, &str, &[&str])] = &[("field", "error", FIELD_ERROR_GROUNDS)];

/// **What a field's error is drawn on** (#3254): only a field that can refuse its value keeps its
/// error's line (`refusable` in `app/src/ui/field/Field.tsx`), and those stand on the window, a
/// pane and a dialog; the whole copy of a refusal cut short is drawn on the overlay surface
/// (#3033). `app/src/ui/field/refusingFields.guard.test.ts` lists every field that can refuse with
/// the ground it stands on, and holds this list to them.
#[cfg(test)]
const FIELD_ERROR_GROUNDS: &[&str] = &["surface.base", "surface.raised", "surface.overlay"];

/// A part of a family in a look, as a sentence names it: `button.root.primary`, or
/// `text-input.root` for a family that has no variants.
fn look_named(family: &Family, part: &str, variant: &str) -> String {
    if variant.is_empty() {
        format!("{}.{part}", family.name)
    } else {
        format!("{}.{part}.{variant}", family.name)
    }
}

/// What a theme may set on a control's root: every property a control has (the layout words, a
/// flare and an inset are a tab's, #2914).
const ALL: &[Prop] = &[
    Prop::Fill,
    Prop::Text,
    Prop::BorderColour,
    Prop::Shadow,
    Prop::BorderWidth,
    Prop::CornerRadius,
    Prop::Padding,
    Prop::Gap,
    Prop::FontSize,
    Prop::FontWeight,
    Prop::Height,
];

/// What a theme may set on a tooltip's surface: its colours, its edge, its corners and its
/// shadow. Its size is its words', so none of its sizes is a theme's.
const SURFACE: &[Prop] = &[
    Prop::Fill,
    Prop::Text,
    Prop::BorderColour,
    Prop::Shadow,
    Prop::BorderWidth,
    Prop::CornerRadius,
];

/// **Every surface a control is drawn on in the window**: a button's, and a field's, which stands
/// where a button does. `app/src/theme/grounds.test.ts` finds them from the components, and fails
/// until a new one is listed here.
const CONTROL_GROUNDS: &[&str] = &[
    "surface.base",
    "surface.raised",
    "surface.overlay",
    "surface.sunken",
    "surface.deep",
    "surface.hover",
    "border.subtle",
    "tab.active",
    "list.selected",
    "control.aimed",
    "danger.surface",
    "danger.wash",
    "layer.project",
    "layer.workspace",
    "layer.chat",
    "terminal.background",
];

// **The families, each in its own file** (`parts_<family>.rs`): `build.rs` reads the directory,
// declares a module for each file and assembles [`MANIFEST`] from every `pub(super) const
// <NAME>: Family` they hold, so a pull request that adds a family adds its own file and touches
// no list another family's pull request does. The order is the manifest's: what the first twelve
// versions shipped, in the order they shipped it, then every later family by the version it came
// in and its name, so a family added in one pull request never moves another's place, and what a
// release shipped is never reordered (`build.rs`, `families`).
include!(concat!(env!("OUT_DIR"), "/theme_parts_families.rs"));

#[cfg(test)]
pub(super) use dialog::DIALOG_TIERS;

/// **The families drawn in marks, not words** (#3143), each with its parts' fills by default
/// while a set of states holds: a resize separator's line and grip. What such a family draws in
/// a state is held at 3:1 on its grounds, as a non-text mark (WCAG 1.4.11), and it has no words
/// to hold; at rest its line is the window's hairline, which the token floors hold.
const MARKED: &[(&str, MarkFills)] = &[("panel-separator", separator::separator_fill)];

/// A family's parts' fills by default, by part and the states that hold.
type MarkFills = fn(usize, &[State]) -> &'static str;

/// **The attributes a family's root is keyed on in a state, where they are not React Aria's**
/// (#3143): the panel library marks its handles itself.
const OWN_STATES: &[(&str, &[(State, &str)])] = &[("panel-separator", separator::OWN_STATES)];

/// The default fills of `family`'s parts by state, when it is drawn in marks.
fn marked(family: &Family) -> Option<MarkFills> {
    MARKED
        .iter()
        .find(|(name, _)| *name == family.name)
        .map(|(_, fill)| *fill)
}

/// The attributes `family`'s root is keyed on in each state, where they are its own.
fn own_states(family: &Family) -> &'static [(State, &'static str)] {
    OWN_STATES
        .iter()
        .find(|(name, _)| *name == family.name)
        .map_or(&[], |(_, states)| states)
}

#[cfg(test)]
#[path = "state_words_tests.rs"]
mod state_words_tests;

#[cfg(test)]
#[path = "part_fills_tests.rs"]
mod part_fills_tests;

#[cfg(test)]
#[path = "surface_grounds_tests.rs"]
mod surface_grounds_tests;

/// **A field's colours as `app/src/ui/field/field.css` draws them**: no ground and no edge of
/// its own, its label in the primary tier. Its help's and its error's words are the family's
/// `words`.
fn field_look(_variant: &str, _states: &[State]) -> Look {
    Look {
        fill: TRANSPARENT,
        text: "text.primary",
        border: TRANSPARENT,
    }
}

/// **A text input's, a textarea's, a select's and a combobox's colours** as
/// `app/src/ui/field/field.css` and `app/src/ui/select/select.css` draw them: the sunken ground
/// behind the control's edge (`border.control`, none in Purlis), in every state but refused; the
/// keyboard's ring is drawn by the family, outside the parts. A refused box (`invalid`, #3110,
/// #3323) draws the danger colour on the same edge, inside the box.
fn box_look(_variant: &str, states: &[State]) -> Look {
    Look {
        fill: "surface.sunken",
        text: "text.primary",
        border: if states.contains(&State::Invalid) {
            "danger.base"
        } else {
            "border.control"
        },
    }
}

/// The colours of a part: its fill, its words and its border, each a token or `transparent`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Look {
    fill: &'static str,
    text: &'static str,
    border: &'static str,
}

/// `transparent`: the one colour a part may take that is not a token.
const TRANSPARENT: &str = "transparent";

/// **The button's colours in the states that hold at once**, as `button.css` resolves them:
/// a toggle that is on keeps its fill and edge under the pointer's, and under the pointer it
/// draws its own selected-and-hovered fill. Pressed, focused and open change no colour.
fn button_look_in(variant: &str, states: &[State]) -> Look {
    let selected = button_look(variant, Some(State::Selected));
    match (
        states.contains(&State::Hover),
        states.contains(&State::Selected),
    ) {
        (true, true) => Look {
            fill: "tab.active",
            ..selected
        },
        (false, true) => selected,
        (true, false) => button_look(variant, Some(State::Hover)),
        (false, false) => button_look(variant, None),
    }
}

/// **The button's colours as `app/src/ui/button/button.css` draws them**, which a test holds to
/// that file: at rest, under the pointer and pressed as a toggle (selected).
fn button_look(variant: &str, state: Option<State>) -> Look {
    let mut look = match variant {
        // The accent's fill and its words, behind the fill's edge (#3177).
        "primary" => Look {
            fill: "accent.fill",
            text: "accent.fill-text",
            border: "accent.fill-edge",
        },
        "ghost" => Look {
            fill: TRANSPARENT,
            text: "text.primary",
            border: TRANSPARENT,
        },
        "danger" => Look {
            fill: "control.base",
            text: "danger.base",
            border: "border.control",
        },
        "grants" => Look {
            fill: "control.base",
            text: "text.primary",
            border: "accent.base",
        },
        "soft-accent" => Look {
            fill: "accent.surface",
            text: "text.primary",
            border: TRANSPARENT,
        },
        "warning-soft" => Look {
            fill: "control.base",
            text: "state.waiting",
            border: TRANSPARENT,
        },
        "link" => Look {
            fill: TRANSPARENT,
            text: "markdown.link",
            border: TRANSPARENT,
        },
        // The quiet ghost (#3214): no box, quiet words.
        "quiet" => Look {
            fill: TRANSPARENT,
            text: "text.quiet",
            border: TRANSPARENT,
        },
        _ => Look {
            fill: "control.base",
            text: "text.primary",
            border: "border.control",
        },
    };
    match state {
        Some(State::Hover) => {
            look.fill = match variant {
                "primary" => "accent.fill-hover",
                "ghost" | "quiet" => "surface.hover",
                "danger" => "danger.surface",
                "soft-accent" => "list.selected",
                // A link is underlined under the pointer; its ground stays none.
                "link" => TRANSPARENT,
                _ => "control.hover",
            };
            // The quiet ghost's words come up to the primary on its hover fill (#3214).
            if variant == "quiet" {
                look.text = "text.primary";
            }
        }
        Some(State::Selected) => {
            look.fill = "accent.surface";
            look.border = "accent.base";
            // The primary's words are its fill's, so on the accent's surface they are the
            // primary words (#3177).
            if variant == "primary" {
                look.text = "text.primary";
            }
        }
        _ => {}
    }
    look
}

/// **The tooltip's colours as `app/src/ui/tooltip/tooltip.css` draws them**, which a test holds
/// to that file: the overlay surface's, in every state, since nothing on it is pressed.
fn tooltip_look(_variant: &str, _states: &[State]) -> Look {
    Look {
        fill: "surface.overlay",
        text: "text.primary",
        border: "border.subtle",
    }
}

/// **A tab's colours as `app/src/ui/tabs/tabs.css` draws them** (#2914), which a test holds to
/// that file: quiet words on the strip at rest; under the pointer, primary words on the card's
/// pill; chosen, primary words on the panel's fill, or a file's on the editor toolbar's.
fn tabs_look(variant: &str, state: Option<State>) -> Look {
    let mut look = Look {
        fill: TRANSPARENT,
        text: "text.quiet",
        border: TRANSPARENT,
    };
    match state {
        Some(State::Hover) => {
            look.fill = "surface.overlay";
            look.text = "text.primary";
        }
        Some(State::Selected) => {
            // A chat's tab and a file's join what is drawn under them, a terminal or code, which
            // is one surface; a view's joins the panel.
            look.fill = if variant == "file" || variant == "chat" {
                EDITOR_TOOLBAR
            } else {
                "surface.raised"
            };
            look.text = "text.primary";
        }
        _ => {}
    }
    look
}

/// **A tab's colours in the states that hold at once**: chosen wins, since an unchosen tab's
/// pill is the only fill the pointer draws, and a chosen tab under the pointer keeps its own.
fn tabs_look_in(variant: &str, states: &[State]) -> Look {
    let state = [State::Selected, State::Hover]
        .into_iter()
        .find(|one| states.contains(one));
    tabs_look(variant, state)
}

/// The editor toolbar's fill, the code surface under it, which a chosen file tab takes too, as a
/// chosen chat's tab takes its terminal's: one surface (the tab-strip prototype's toolbar).
const EDITOR_TOOLBAR: &str = "terminal.background";

/// **The editor toolbar's colours as `app/src/ui/tabs/tabs.css` draws them** (#2914).
fn editor_look(_variant: &str, _states: &[State]) -> Look {
    Look {
        fill: EDITOR_TOOLBAR,
        text: "text.muted",
        border: TRANSPARENT,
    }
}

/// **The distance in OKLab below which a person sees one colour** (`floors.rs`'s scale): how
/// near Allow's fill is held to Keep blocked's.
pub(super) const SEEN: f64 = 0.02;

/// How much nearer the primary than the family's defaults a consent variant may be drawn: none,
/// beyond what rounding a colour moves.
const NO_NEARER: f64 = 0.005;

/// **How far apart two looks are drawn**, in OKLab, on what a person reads as the look: the
/// fills as each shows on every ground `on` it is drawn on ([`grounds`]), the nearest ground
/// counting; the edges only where they stand out at a glance ([`super::ALLOW_APART`]), since a
/// hairline that differs less reads as the same edge; and, with `words`, the words.
fn look_apart(theme: &ResolvedTheme, on: &[String], a: &Drawn, b: &Drawn, words: bool) -> f64 {
    let one = |x: &str, y: &str| match (hex(theme, x), hex(theme, y)) {
        (None, None) => 0.0,
        (Some(_), Some(_)) => apart(theme, x, y).unwrap_or(1.0),
        _ => 1.0,
    };
    let shown = |fill: &str, ground: &str| {
        hex(theme, fill).map_or_else(|| ground.to_owned(), |fill| onto(fill, ground))
    };
    let fills = on
        .iter()
        .filter_map(|ground| {
            let (x, y) = (shown(a.fill, ground), shown(b.fill, ground));
            Some(tint::distance(tint::oklab_of(&x)?, tint::oklab_of(&y)?))
        })
        .reduce(f64::min)
        .unwrap_or_else(|| one(a.fill, b.fill));
    let edges = one(a.border, b.border);
    let mut far = if edges >= super::ALLOW_APART {
        fills.max(edges)
    } else {
        fills
    };
    if words {
        far = far.max(one(a.words[0], b.words[0]));
    }
    far
}

/// The variant every family's suggested answer is drawn in.
const PRIMARY: &str = "primary";

/// **Allow against the primary as a theme's tokens draw them** (#2948): one measure for each
/// state an answer that grants is drawn in. The token-level floor ([`super::floors`]) holds it.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct AllowLook {
    /// The state, `None` at rest.
    pub state: Option<State>,
    /// How far apart the two looks are drawn ([`look_apart`]).
    pub apart: f64,
    /// The primary's fill token, then Allow's: what a floor that fails draws as purlis's own.
    pub fills: [&'static str; 2],
}

/// The grounds every family with an answer that grants is drawn on: what its fills are measured
/// over.
pub(super) fn allow_grounds() -> impl Iterator<Item = &'static str> {
    MANIFEST
        .iter()
        .filter(|family| {
            family
                .variants
                .iter()
                .any(|variant| variant.consent == Some(Consent::Grants))
        })
        .flat_map(|family| family.grounds.iter().copied())
}

/// **How far Allow is drawn from the primary by the family defaults alone**, in `theme`: for
/// every family with an answer that grants, at rest and in each state whose look differs from
/// it, measured as the parts check measures looks ([`look_apart`]): the fills as each shows on
/// every ground the family is drawn on, the nearest ground counting, and the edges where they
/// stand out. A selected toggle is left out: an answer that grants is never a toggle.
pub(super) fn allow_from_primary(theme: &ResolvedTheme) -> Vec<AllowLook> {
    let mut found = Vec::new();
    for family in MANIFEST {
        let Some(grants) = family
            .variants
            .iter()
            .find(|variant| variant.consent == Some(Consent::Grants))
        else {
            continue;
        };
        let mut seen: Vec<(Drawn, Drawn)> = Vec::new();
        for state in std::iter::once(None).chain(
            STATES
                .into_iter()
                .filter(|state| {
                    family.states.contains(state)
                        && !matches!(state, State::Selected | State::Disabled)
                })
                .map(Some),
        ) {
            let states: Vec<State> = state.into_iter().collect();
            let allow = drawn(&[], family, grants.name, &states);
            let primary = drawn(&[], family, PRIMARY, &states);
            if seen.contains(&(allow.clone(), primary.clone())) {
                continue;
            }
            found.push(AllowLook {
                state,
                apart: look_apart(theme, &grounds(theme, family), &allow, &primary, false),
                fills: [primary.fill, allow.fill],
            });
            seen.push((allow, primary));
        }
    }
    found
}

/// A variant's button colours as the engine holds them, for the test that holds them to
/// `ui/button/button.css`: at rest, under the pointer, and selected.
#[cfg(test)]
pub(super) fn defaults_json(variant: &str) -> Value {
    let rest = button_look(variant, None);
    let hover = button_look(variant, Some(State::Hover));
    let selected = button_look(variant, Some(State::Selected));
    let both = button_look_in(variant, &[State::Hover, State::Selected]);
    json!({
        "fill": rest.fill,
        "text": rest.text,
        "border": rest.border,
        "hover": hover.fill,
        "hover_text": hover.text,
        "selected": { "fill": selected.fill, "border": selected.border },
        "selected_hover": { "fill": both.fill, "border": both.border },
    })
}

/// The field families' colours as the engine holds them, for the test that holds them to
/// `field.css`: the boxes', and each of the field's words.
#[cfg(test)]
pub(super) fn field_defaults_json() -> Value {
    let boxes = box_look("", &[]);
    let field = family("field").expect("the field family");
    let words = |part: &str| {
        let at = field.parts.iter().position(|one| one.name == part);
        at.and_then(|at| field.own_words(at))
            .unwrap_or(field_look("", &[]).text)
    };
    json!({
        "box": { "fill": boxes.fill, "text": boxes.text, "border": boxes.border },
        "label": words("label"),
        "help": words("help"),
        "error": words("error"),
    })
}

/// The dialog's colours as the engine holds them, for the test that holds them to `dialog.css`.
#[cfg(test)]
pub(super) fn dialog_defaults_json(variant: &str) -> Value {
    let rest = dialog::dialog_look(variant, &[]);
    json!({ "fill": rest.fill, "text": rest.text, "border": rest.border })
}

/// A family's colours at rest and under the pointer, as the engine holds them, for the tests that
/// hold them to the family's stylesheet: fill, text, border and the fill under the pointer.
#[cfg(test)]
pub(super) fn look_json(family_name: &str, variant: &str) -> Value {
    let family = family(family_name).expect("a family of the manifest");
    let rest = (family.defaults)(variant, &[]);
    let hover = (family.defaults)(variant, &[State::Hover]);
    json!({ "fill": rest.fill, "text": rest.text, "border": rest.border, "hover": hover.fill })
}

/// A family's colours with `states` holding, as the engine holds them, fill, words and border, for
/// the tests that hold them to the family's stylesheet.
#[cfg(test)]
pub(super) fn look_of(family_name: &str, variant: &str, states: &[State]) -> [&'static str; 3] {
    let family = family(family_name).expect("a family of the manifest");
    let look = (family.defaults)(variant, states);
    [look.fill, look.text, look.border]
}

/// A family's colours as the engine holds them, for the tests that hold them to its stylesheet:
/// the fill and the words of `variant` in `state`.
#[cfg(test)]
pub(super) fn look_pair_of(family: &str, variant: &str, state: Option<State>) -> (String, String) {
    let look = (self::family(family).expect("a family").defaults)(variant, state.as_slice());
    (look.fill.to_owned(), look.text.to_owned())
}

/// The activity item's colours while its view is open, for the test that holds them to
/// `activity.css`.
#[cfg(test)]
pub(super) fn activity_selected_json() -> Value {
    let family = family("activity").expect("the activity family");
    let chosen = (family.defaults)("tab", &[State::Selected]);
    json!({ "fill": chosen.fill, "text": chosen.text })
}

/// The panel separator's default fill of `part` while `states` hold, for the test that holds it
/// to `panel.css` (#3143).
#[cfg(test)]
pub(super) fn separator_fill_of(part: usize, states: &[State]) -> &'static str {
    separator::separator_fill(part, states)
}

/// The family `name` names.
pub fn family(name: &str) -> Option<&'static Family> {
    MANIFEST.iter().find(|one| one.name == name)
}

/// **A key of a theme's `parts` block**, parsed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Key {
    pub family: &'static str,
    pub part: &'static str,
    pub variant: Option<&'static str>,
    pub state: Option<State>,
    pub property: Prop,
}

impl std::fmt::Display for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.family, self.part)?;
        if let Some(variant) = self.variant {
            write!(f, ".{variant}")?;
        }
        if let Some(state) = self.state {
            write!(f, ".{}", state.name())?;
        }
        write!(f, ".{}", self.property.name())
    }
}

/// A length as a theme gives it: a number, or a geometry token.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Length {
    Number(f64),
    Token(&'static str),
}

/// One corner: a length, or `full`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Corner {
    Full,
    Length(Length),
}

/// A shadow: an elevation step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shadow {
    None,
    Raised,
    Overlay,
}

/// **A part's value, parsed**: what reaches the window is written from this, never from the
/// theme's text.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PartValue {
    /// A colour token, or `transparent`.
    Colour(&'static str),
    Shadow(Shadow),
    Length(Length),
    /// Top left, top right, bottom right, bottom left.
    Corners([Corner; 4]),
    /// Block, inline.
    Padding(Length, Length),
    /// One of a property's words (#2914): `panel`, `equal`, `on`, `bottom`.
    Word(&'static str),
    /// Top, sides, bottom (#2914).
    Inset([Length; 3]),
}

/// **A theme's component parts**, every key and its value: empty for a theme that names none.
pub type Parts = BTreeMap<Key, PartValue>;

/// Property names typed out of CSS habit, and the one each means.
const NEAR_MISSES: &[(&str, &str)] = &[
    ("border-color", "border-colour"),
    ("color", "text"),
    ("background", "fill"),
    ("background-color", "fill"),
    ("border-radius", "corner-radius"),
    ("radius", "corner-radius"),
    ("corner_radius", "corner-radius"),
];

/// The key `raw` names, or why it names nothing purlis has.
fn key(raw: &str) -> Result<Key, String> {
    let said = || {
        let property = raw.rsplit('.').next().unwrap_or_default();
        let hint = NEAR_MISSES
            .iter()
            .find(|(typed, _)| *typed == property)
            .map(|(_, meant)| format!("; did you mean `{meant}`?"))
            .unwrap_or_default();
        format!(
            "parts.{raw} is not a component part purlis has (`purlis theme parts` lists them), so \
             it was left out{hint}"
        )
    };
    let segments: Vec<&str> = raw.split('.').collect();
    if segments.len() < 3 || segments.len() > 5 {
        return Err(said());
    }
    let family = family(segments[0]).ok_or_else(said)?;
    let part = family
        .parts
        .iter()
        .find(|one| one.name == segments[1])
        .ok_or_else(said)?;
    let property = Prop::named(segments[segments.len() - 1]).ok_or_else(said)?;
    if !part.properties.contains(&property) {
        return Err(said());
    }
    let mut middle = segments[2..segments.len() - 1].iter();
    let mut variant = None;
    let mut state = None;
    let mut next = middle.next();
    if let Some(name) = next
        && let Some(one) = family.variants.iter().find(|one| one.name == *name)
    {
        variant = Some(one.name);
        next = middle.next();
    }
    if let Some(name) = next {
        let named = State::named(name).ok_or_else(said)?;
        // A state the family's wrapper never marks would reach nothing on screen.
        if !family.states.contains(&named) {
            return Err(said());
        }
        state = Some(named);
        next = middle.next();
    }
    if next.is_some() {
        return Err(said());
    }
    // A part drawn around the root is the same in every variant and state (#2914).
    if family.around.contains(&part.name) && (variant.is_some() || state.is_some()) {
        return Err(format!(
            "parts.{raw} is not a component part purlis has: {}.{} is drawn around every tab and \
             takes no variant or state, so it was left out",
            family.name, part.name
        ));
    }
    Ok(Key {
        family: family.name,
        part: part.name,
        variant,
        state,
        property,
    })
}

/// A colour token by its name, or `transparent`.
fn colour(value: &Value) -> Option<&'static str> {
    let text = value.as_str()?;
    if text == TRANSPARENT {
        return Some(TRANSPARENT);
    }
    TOKENS.iter().copied().find(|token| *token == text)
}

/// A length as `bound` holds it, or why not.
fn length(value: &Value, bound: Bound, at: &str) -> Result<Length, String> {
    if let Some(text) = value.as_str() {
        if let Some(prefix) = bound.tokens
            && text.starts_with(prefix)
            && let Some(step) = geometry::step_of(text)
        {
            return Ok(Length::Token(step.token));
        }
        let tokens = bound
            .tokens
            .map(|prefix| format!(", or a {prefix}* token"))
            .unwrap_or_default();
        return Err(format!(
            "{at} is {}, which is not a number{tokens}",
            stringify(value)
        ));
    }
    let Some(number) = value.as_f64().filter(|n| n.is_finite()) else {
        return Err(format!(
            "{at} is {}, which is not a number",
            stringify(value)
        ));
    };
    if number.fract() != 0.0 {
        return Err(format!(
            "{at} is {}, which is not a whole number",
            stringify(value)
        ));
    }
    if number < bound.least || number > bound.most {
        let unit = match bound.unit {
            geometry::Unit::Text => " px at the default text size",
            geometry::Unit::Px => " px",
            geometry::Unit::Weight | geometry::Unit::Ratio => "",
        };
        return Err(format!(
            "{at} is {}, outside its bound: it is a whole number from {} to {}{unit}",
            stringify(value),
            geometry::show(bound.least),
            geometry::show(bound.most)
        ));
    }
    Ok(Length::Number(number))
}

/// One corner, or why not.
fn corner(value: &Value, bound: Bound, at: &str) -> Result<Corner, String> {
    if value.as_str() == Some("full") {
        return Ok(Corner::Full);
    }
    length(value, bound, at)
        .map(Corner::Length)
        .map_err(|why| format!("{why}; a corner may also be \"full\""))
}

/// `key`'s value, parsed, or the sentence that says why not.
fn value(key: &Key, raw: &Value) -> Result<PartValue, String> {
    let at = format!("parts.{key}");
    let property = key.property;
    if key.state.is_some() && !property.in_state() {
        return Err(format!(
            "{at} is refused: {NO_SIZE_IN_A_STATE}. Give {} to {}.{}{} at rest",
            property.name(),
            key.family,
            key.part,
            key.variant.map(|v| format!(".{v}")).unwrap_or_default()
        ));
    }
    if let Some(only) = property.only_in()
        && key.state != Some(only)
    {
        return Err(format!(
            "{at} is refused: a tab's {} is drawn only while it is {}, so it is set as \
             {}.{}{}.{}.{}",
            property.name(),
            match only {
                State::Selected => "chosen",
                _ => "under the pointer",
            },
            key.family,
            key.part,
            key.variant.map(|v| format!(".{v}")).unwrap_or_default(),
            only.name(),
            property.name()
        ));
    }
    if let Some(words) = property.keywords() {
        return raw
            .as_str()
            .and_then(|text| words.iter().copied().find(|word| *word == text))
            .map(PartValue::Word)
            .ok_or_else(|| {
                format!(
                    "{at} is {}: it is one of {}",
                    stringify(raw),
                    words.join(" or ")
                )
            });
    }
    if property.is_colour() {
        return match colour(raw) {
            Some(TRANSPARENT) if property == Prop::Text => Err(format!(
                "{at} is \"transparent\": a part's words always have a colour"
            )),
            Some(token) => Ok(PartValue::Colour(token)),
            None => Err(format!(
                "{at} is {}: a part's colour names a semantic colour token (accent.base) or \
                 transparent, never a hex or a palette name, so palettes, appearances and \
                 tints keep working",
                stringify(raw)
            )),
        };
    }
    if property == Prop::Shadow {
        return match raw.as_str() {
            Some("none") => Ok(PartValue::Shadow(Shadow::None)),
            Some("raised") => Ok(PartValue::Shadow(Shadow::Raised)),
            Some("overlay") => Ok(PartValue::Shadow(Shadow::Overlay)),
            _ => Err(format!(
                "{at} is {}: a shadow is an elevation step, none, raised or overlay",
                stringify(raw)
            )),
        };
    }
    let Some(bound) = bound_of(key) else {
        return Err(format!("{at} has no value purlis reads"));
    };
    match property {
        Prop::Inset => match raw {
            Value::Array(three) if three.len() == 3 => {
                let mut sides = [Length::Number(0.0); 3];
                for (at_side, one) in three.iter().enumerate() {
                    sides[at_side] = length(one, bound, &format!("{at} (value {})", at_side + 1))?;
                }
                Ok(PartValue::Inset(sides))
            }
            Value::Array(_) => Err(format!(
                "{at} is a list of {} values: an inset is one value, or three (top, sides, bottom)",
                raw.as_array().map_or(0, Vec::len)
            )),
            one => length(one, bound, &at).map(|one| PartValue::Inset([one; 3])),
        },
        Prop::CornerRadius => match raw {
            Value::Array(four) if four.len() == 4 => {
                let mut corners = [Corner::Full; 4];
                for (at_corner, one) in four.iter().enumerate() {
                    corners[at_corner] =
                        corner(one, bound, &format!("{at} (corner {})", at_corner + 1))?;
                }
                Ok(PartValue::Corners(corners))
            }
            Value::Array(_) => Err(format!(
                "{at} is a list of {} values: corners are one value or four",
                raw.as_array().map_or(0, Vec::len)
            )),
            one => corner(one, bound, &at).map(|one| PartValue::Corners([one; 4])),
        },
        Prop::Padding => match raw {
            Value::Array(two) if two.len() == 2 => Ok(PartValue::Padding(
                length(
                    &two[0],
                    Bound {
                        most: MOST_BLOCK,
                        ..bound
                    },
                    &format!("{at} (block)"),
                )?,
                length(&two[1], bound, &format!("{at} (inline)"))?,
            )),
            Value::Array(_) => Err(format!(
                "{at} is a list of {} values: padding is one value, or two (block, inline)",
                raw.as_array().map_or(0, Vec::len)
            )),
            one => {
                let both = length(
                    one,
                    Bound {
                        most: MOST_BLOCK,
                        ..bound
                    },
                    &at,
                )?;
                Ok(PartValue::Padding(both, both))
            }
        },
        _ => length(raw, bound, &at).map(PartValue::Length),
    }
}

/// **A theme's `parts` block, read**: each key and value parsed. A key naming nothing purlis has
/// is said in `notes` and left out; a value outside its type or bound, a size inside a state, and
/// a consent part's edge turned off are said in `complaints`, which refuse the theme.
pub(super) fn read(
    raw: Option<&Value>,
    complaints: &mut Vec<Complaint>,
    notes: &mut Vec<Complaint>,
) -> Parts {
    let mut parts = Parts::new();
    let Some(raw) = raw else {
        return parts;
    };
    let Some(given) = raw.as_object() else {
        complaints.push(Complaint::said(
            "parts is an object of component part keys and their values",
        ));
        return parts;
    };
    for (name, raw) in given {
        let key = match key(name) {
            Ok(key) => key,
            Err(said) => {
                notes.push(Complaint::said(said));
                continue;
            }
        };
        match value(&key, raw) {
            Ok(parsed) => {
                if let Some(said) =
                    edge_removed(&key, &parsed).or_else(|| off_the_grounds(&key, &parsed))
                {
                    complaints.push(Complaint::said(said));
                    continue;
                }
                parts.insert(key, parsed);
            }
            Err(said) => complaints.push(Complaint::said(said)),
        }
    }
    joined(&mut parts);
    parts
}

/// **What a chosen tab joins, it takes the fill of** (#2914): the panel under a docked strip, and
/// a file's toolbar. A theme that gives the panel or the toolbar a fill and leaves the chosen
/// tab's unset draws the tab in it too, so the two stay one piece and the floors measure the
/// fill the tab is drawn in.
fn joined(parts: &mut Parts) {
    let key = |family, part, variant, state| Key {
        family,
        part,
        variant,
        state,
        property: Prop::Fill,
    };
    let chosen = key("tabs", "tab", None, Some(State::Selected));
    let file = key("tabs", "tab", Some("file"), Some(State::Selected));
    // The content panel the tabs attach to is the panel family's (#2913).
    let panel = [Some("content"), None]
        .into_iter()
        .find_map(|variant| parts.get(&key("panel", "root", variant, None)).copied());
    // Only a fill that is not the one a docked chosen tab already takes: a theme whose panel is
    // the default's draws its tabs from the family defaults, and costs nothing to measure.
    if let Some(fill) = panel.filter(|fill| *fill != PartValue::Colour("surface.raised")) {
        parts.entry(chosen).or_insert(fill);
        // A chosen chat's tab stays on its terminal's surface: the panel's fill is a view's.
        parts
            .entry(key("tabs", "tab", Some("chat"), Some(State::Selected)))
            .or_insert(PartValue::Colour(EDITOR_TOOLBAR));
    }
    // A chosen file tab joins its toolbar, whatever fill every other chosen tab takes.
    let toolbar = parts.get(&key("editor", "toolbar", None, None)).copied();
    if toolbar.is_some() || parts.contains_key(&chosen) {
        parts
            .entry(file)
            .or_insert(toolbar.unwrap_or(PartValue::Colour(EDITOR_TOOLBAR)));
    }
}

/// Why `key` would take a consent part's mark away, if it would: a border width of 0 on Allow,
/// or on an ask (#2912).
fn edge_removed(key: &Key, value: &PartValue) -> Option<String> {
    if key.property != Prop::BorderWidth || *value != PartValue::Length(Length::Number(0.0)) {
        return None;
    }
    let consent = key
        .variant
        .and_then(|name| variant(key.family, name))
        .and_then(|one| one.consent);
    if let Some(consent) = consent.filter(|one| one.keeps_edge()) {
        let what = if consent == Consent::Asks {
            "an ask keeps its waiting edge"
        } else {
            "an answer that grants keeps its accent edge and its lock"
        };
        return Some(format!(
            "parts.{key} is refused: {what}, so a theme can never disguise a consent decision"
        ));
    }
    // A refused box says so by its edge (#3110), and a state never sets a width, so a box with
    // no edge at rest could never show it.
    family(key.family)
        .filter(|one| one.states.contains(&State::Invalid) && key.part == one.parts[0].name)
        .map(|_| {
            let root = format!("{}.{}", key.family, key.part);
            format!(
                "parts.{key} is refused: a box that can be refused shows it by its edge, so its \
                 edge keeps a width; for no edge at rest, set {root}.border-colour to \
                 \"transparent\", and the refused box draws {root}.invalid.border-colour"
            )
        })
}

/// Why `key` would stand answers on a ground they are not measured on, if it would (#2912): a
/// card's or a Notice's fill is one of the grounds the button family is held on, so an answer
/// that grants stays apart from the primary wherever it is drawn (ADR 0103 §7).
fn off_the_grounds(key: &Key, value: &PartValue) -> Option<String> {
    let PartValue::Colour(token) = value else {
        return None;
    };
    if key.part != "root"
        || key.property != Prop::Fill
        || !content::HOLDS_ANSWERS.contains(&key.family)
        || *token == TRANSPARENT
    {
        return None;
    }
    let held = family("button")?.grounds;
    (!held.contains(token)).then(|| {
        format!(
            "parts.{key} is refused: {token} is not a ground the answers a {} holds are measured \
             on, so an Allow on it could not be held apart from the primary; name one of {}",
            key.family,
            held.join(", ")
        )
    })
}

/// The floor a refused box breaks when its edge is gone, or drawn as the edge it has while it is
/// not refused (#3110): then nothing on screen says the value is refused.
const REFUSAL_HIDDEN: &str = "a refused box keeps its own edge, told apart from the edge it has \
     when it is not refused";

/// Whether `look`, drawn while `states` hold [`State::Invalid`], hides the refusal: no edge, or an
/// edge not told at a glance from the one the same states draw without it.
fn refusal_hidden(
    theme: &ResolvedTheme,
    rules: &[Rule],
    family: &Family,
    variant: &str,
    states: &[State],
    look: &Drawn,
) -> bool {
    let Some(own) = edge(rules, family, variant, look) else {
        return true;
    };
    let valid: Vec<State> = states
        .iter()
        .copied()
        .filter(|one| *one != State::Invalid)
        .collect();
    let unrefused = drawn(rules, family, variant, &valid);
    let theirs = edge(rules, family, variant, &unrefused).unwrap_or(TRANSPARENT);
    alike(theme, own, theirs, super::ALLOW_APART)
}

/// **The parts that are marks, not controls** (#2914): a tab's indicator is a line a few px tall,
/// so its height is a mark's, not a target's.
const MARKS: &[(&str, &str)] = &[("tabs", "indicator")];

/// The most a mark is drawn tall, in px.
const MOST_MARK: f64 = 6.0;

/// The bound `key`'s value is held to: its property's, but a mark's height is a mark's.
fn bound_of(key: &Key) -> Option<Bound> {
    // The room between tabs is a strip's, as wide as a text-only theme's gaps (#2914).
    if key.property == Prop::Gap && (key.family, key.part) == ("tabs", "layout") {
        return Some(Bound {
            least: 0.0,
            most: 32.0,
            unit: geometry::Unit::Text,
            tokens: Some("space."),
        });
    }
    if key.property == Prop::Height && MARKS.contains(&(key.family, key.part)) {
        return Some(Bound {
            least: 0.0,
            most: MOST_MARK,
            unit: geometry::Unit::Px,
            tokens: None,
        });
    }
    key.property.bound()
}

fn variant(family_name: &str, name: &str) -> Option<&'static Variant> {
    family(family_name)?
        .variants
        .iter()
        .find(|one| one.name == name)
}

/// `child`'s parts over `parent`'s, key by key.
pub(super) fn over(parent: &Parts, child: Parts) -> Parts {
    let mut parts = parent.clone();
    parts.extend(child);
    parts
}

// --- Sizes -----------------------------------------------------------------------------------

/// A length's number in `geometry`, px at the default text size.
fn px(length: Length, geometry: &Geometry) -> f64 {
    match length {
        Length::Number(number) => number,
        Length::Token(token) => geometry
            .get(token)
            .copied()
            .or_else(|| geometry::step_of(token).map(|step| step.classic))
            .unwrap_or_default(),
    }
}

/// The value the rules give a variant's part at rest, if they give it one.
fn at_rest(
    parts: &Parts,
    family: &'static str,
    part: &'static str,
    variant: &'static str,
    property: Prop,
) -> Option<PartValue> {
    [Some(variant), None].into_iter().find_map(|variant| {
        parts
            .get(&Key {
                family,
                part,
                variant,
                state: None,
                property,
            })
            .copied()
    })
}

/// **A geometry token a part names is held to the part's bound too**, at the value the theme
/// draws it at in each density: each sentence names the key, the density and the bound.
fn bounded(parts: &Parts, comfortable: &Geometry, compact: &Geometry) -> Vec<String> {
    let mut said = Vec::new();
    for (key, value) in parts {
        let Some(bound) = bound_of(key) else {
            continue;
        };
        let lengths: Vec<(Length, f64)> = match value {
            PartValue::Length(length) => vec![(*length, bound.most)],
            PartValue::Padding(block, inline) => vec![(*block, MOST_BLOCK), (*inline, bound.most)],
            PartValue::Corners(_)
            | PartValue::Colour(_)
            | PartValue::Shadow(_)
            | PartValue::Word(_)
            | PartValue::Inset(_) => Vec::new(),
        };
        for (length, most) in lengths {
            let Length::Token(token) = length else {
                continue;
            };
            for (density, geometry) in [("comfortable", comfortable), ("compact", compact)] {
                let number = px(length, geometry);
                if number < bound.least || number > most {
                    said.push(format!(
                        "parts.{key} is {token}, {} in {density}, outside its bound: from {} to {}",
                        geometry::show(number),
                        geometry::show(bound.least),
                        geometry::show(most)
                    ));
                }
            }
        }
    }
    said
}

/// **Whether every control a theme's parts size stays a target and holds its words**, in both
/// densities (`comfortable` and `compact` are the appearance's geometry): each variant whose
/// size the theme sets is at least [`LEAST_TARGET`] px tall, and its words, its padding and its
/// border fit in that height. Each sentence says which does not, which refuses the theme.
pub(super) fn fits(parts: &Parts, comfortable: &Geometry, compact: &Geometry) -> Vec<String> {
    let mut said = bounded(parts, comfortable, compact);
    for family in MANIFEST {
        // A box as tall as what it holds is no target, and holds its words by growing.
        let Some(own_height) = family.height else {
            continue;
        };
        let root = family.parts[0].name;
        for variant in family.looks() {
            let sizes = [
                Prop::Height,
                Prop::Padding,
                Prop::FontSize,
                Prop::BorderWidth,
            ]
            .map(|property| at_rest(parts, family.name, root, variant, property));
            if sizes.iter().all(Option::is_none) {
                continue;
            }
            let label = at_rest(parts, family.name, "label", variant, Prop::FontSize);
            for (density, geometry) in [("comfortable", comfortable), ("compact", compact)] {
                let length_of = |value: Option<PartValue>, default: Length| match value {
                    Some(PartValue::Length(length)) => px(length, geometry),
                    _ => px(default, geometry),
                };
                let height = length_of(sizes[0], own_height);
                let padding = match sizes[1] {
                    Some(PartValue::Padding(block, _)) => px(block, geometry),
                    _ => 0.0,
                };
                let size = length_of(label.or(sizes[2]), Length::Token(family.font_size));
                let border = length_of(sizes[3], Length::Number(family.border_width));
                let named = look_named(family, root, variant);
                if height < LEAST_TARGET {
                    said.push(format!(
                        "parts: {named} is {}px tall in {density}, below the {}px a control keeps \
                         so it can be reached",
                        geometry::show(height),
                        geometry::show(LEAST_TARGET)
                    ));
                    continue;
                }
                let line = size * family.line_height + 2.0 * padding + 2.0 * border;
                if line > height {
                    said.push(format!(
                        "parts: {named}'s words, padding and border take {}px in {density}, \
                         more than its {}px height, so its words would not fit",
                        geometry::show(line),
                        geometry::show(height)
                    ));
                }
            }
        }
    }
    said
}

// --- Floors ----------------------------------------------------------------------------------

/// **A part drawn as its family default because it broke a floor** ([`fallen`]).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Fell {
    /// `<family>.<part>.<variant>`, with the state when it is not the state at rest.
    pub part: String,
    /// The floor it broke, as a person reads it.
    pub floor: String,
    /// The contrast it had, to two places, when a contrast floor is what it broke.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ratio: Option<f64>,
}

/// One rule of the stylesheet: a part of a family, in a variant and a state or every one, and
/// the values it sets.
#[derive(Debug, Clone)]
struct Rule {
    family: &'static Family,
    part: usize,
    variant: Option<&'static str>,
    state: Option<State>,
    values: BTreeMap<Prop, PartValue>,
    /// Whether it draws a family default back, for a part that broke a floor.
    fallback: bool,
}

impl Rule {
    /// The order the stylesheet writes rules in: the later wins. A refused box's fallback comes
    /// after every other fallback, so its danger edge wins over a hover's or a focus's default
    /// written after it (#3382's review, B1).
    fn order(&self) -> (bool, bool, bool, bool, usize) {
        (
            self.state.is_some(),
            self.variant.is_some(),
            self.fallback,
            self.fallback && self.state == Some(State::Invalid),
            self.part,
        )
    }

    /// Whether it reaches `variant` while `states` hold (none: at rest).
    fn reaches(&self, variant: &str, states: &[State]) -> bool {
        self.variant.is_none_or(|v| v == variant)
            && self.state.is_none_or(|state| states.contains(&state))
    }
}

/// Where a rule reaches: its family, its part, its variant and its state.
type Where = (
    &'static str,
    &'static str,
    Option<&'static str>,
    Option<State>,
);

/// The rules a theme's parts give, unordered.
fn rules(parts: &Parts) -> Vec<Rule> {
    let mut grouped: BTreeMap<Where, Rule> = BTreeMap::new();
    for (key, value) in parts {
        let Some(family) = family(key.family) else {
            continue;
        };
        let part = family
            .parts
            .iter()
            .position(|one| one.name == key.part)
            .unwrap_or_default();
        grouped
            .entry((key.family, key.part, key.variant, key.state))
            .or_insert_with(|| Rule {
                family,
                part,
                variant: key.variant,
                state: key.state,
                values: BTreeMap::new(),
                fallback: false,
            })
            .values
            .insert(key.property, *value);
    }
    grouped.into_values().collect()
}

/// The colour token `rules` draw `property` of `part` in, for `variant` while `states` hold, as
/// the stylesheet's order resolves them, or the family default's; `None` when nothing sets it (a
/// label's words follow the root's).
fn drawn_colour(
    rules: &[Rule],
    family: &Family,
    part: usize,
    variant: &str,
    states: &[State],
    property: Prop,
) -> Option<&'static str> {
    let mut ordered: Vec<&Rule> = rules
        .iter()
        .filter(|rule| {
            std::ptr::eq(rule.family, family) && rule.part == part && rule.reaches(variant, states)
        })
        .collect();
    ordered.sort_by_key(|rule| rule.order());
    let set = ordered
        .iter()
        .rev()
        .find_map(|rule| match rule.values.get(&property) {
            Some(PartValue::Colour(token)) => Some(*token),
            _ => None,
        });
    if set.is_some() {
        return set;
    }
    if part != 0 {
        return family.own_words(part);
    }
    let look = (family.defaults)(variant, states);
    Some(match property {
        Prop::Fill => look.fill,
        Prop::Text => look.text,
        _ => look.border,
    })
}

/// A part's look as drawn: the root's fill, border and words, and each other part's words.
#[derive(Debug, Clone, PartialEq)]
struct Drawn {
    fill: &'static str,
    border: &'static str,
    words: Vec<&'static str>,
    /// What each part's words are read on: its own fill where it has one, else the root's.
    grounds: Vec<&'static str>,
    /// Each part's fill, in a family drawn in marks ([`MARKED`]); empty in any other.
    marks: Vec<&'static str>,
}

fn drawn(rules: &[Rule], family: &Family, variant: &str, states: &[State]) -> Drawn {
    let colour = |part, property| drawn_colour(rules, family, part, variant, states, property);
    let root_text = colour(0, Prop::Text).unwrap_or("text.primary");
    let fill = colour(0, Prop::Fill).unwrap_or(TRANSPARENT);
    Drawn {
        fill,
        border: colour(0, Prop::BorderColour).unwrap_or(TRANSPARENT),
        words: (0..family.parts.len())
            .map(|part| colour(part, Prop::Text).unwrap_or(root_text))
            .collect(),
        grounds: (0..family.parts.len())
            .map(|part| {
                // A part with no words of its own (an indicator) is read as the root is.
                if part == 0 || !family.parts[part].properties.contains(&Prop::Text) {
                    return fill;
                }
                part_fill(rules, family, part, variant, states)
                    .or_else(|| family.own_ground(part))
                    .unwrap_or(fill)
            })
            .collect(),
        marks: marked(family).map_or_else(Vec::new, |by_default| {
            (0..family.parts.len())
                .map(|part| {
                    if part == 0 {
                        return fill;
                    }
                    part_fill(rules, family, part, variant, states)
                        .unwrap_or_else(|| by_default(part, states))
                })
                .collect()
        }),
    }
}

/// The fill a theme's rules give a part that is not the root, for `variant` while `states` hold,
/// as the stylesheet's order resolves them; `None` when nothing sets it.
fn part_fill(
    rules: &[Rule],
    family: &Family,
    part: usize,
    variant: &str,
    states: &[State],
) -> Option<&'static str> {
    let mut ordered: Vec<&Rule> = rules
        .iter()
        .filter(|rule| {
            std::ptr::eq(rule.family, family) && rule.part == part && rule.reaches(variant, states)
        })
        .collect();
    ordered.sort_by_key(|rule| rule.order());
    ordered
        .iter()
        .rev()
        .find_map(|rule| match rule.values.get(&Prop::Fill) {
            Some(PartValue::Colour(token)) => Some(*token),
            _ => None,
        })
}

/// A token's colour in `theme`, `None` for `transparent` and for one the platform draws.
fn hex<'a>(theme: &'a ResolvedTheme, token: &str) -> Option<&'a str> {
    if token == TRANSPARENT {
        return None;
    }
    let value = theme.value(token);
    (value != PLATFORM && !value.is_empty()).then_some(value)
}

/// **The family's grounds as `theme` draws them**, never none: each opaque one as it is, its
/// alpha read as drawn, and a wash laid over each opaque one, since a wash is drawn over what is
/// under it. With no opaque ground, each is measured as its colour drawn opaque; with no ground
/// purlis can read, on black and on white, the strictest.
fn grounds(theme: &ResolvedTheme, family: &Family) -> Vec<String> {
    let given: Vec<&str> = family
        .grounds
        .iter()
        .filter_map(|ground| hex(theme, ground))
        .collect();
    let under: Vec<String> = given
        .iter()
        .filter(|one| is_opaque(one))
        .map(|one| solid(one))
        .collect();
    if under.is_empty() {
        let drawn: Vec<String> = given.iter().map(|one| solid(one)).collect();
        return if drawn.is_empty() {
            vec!["#000000".to_owned(), "#ffffff".to_owned()]
        } else {
            drawn
        };
    }
    let mut drawn = under.clone();
    for one in given.iter().filter(|one| !is_opaque(one)) {
        drawn.extend(under.iter().map(|below| onto(one, below)));
    }
    drawn
}

/// The lowest contrast `front` has on `fill` over each ground `on` ([`grounds`]).
fn lowest(theme: &ResolvedTheme, on: &[String], fill: &str, front: &str) -> Option<f64> {
    let front = hex(theme, front)?;
    on.iter()
        .map(|ground| {
            let back = hex(theme, fill).map_or_else(|| ground.clone(), |f| onto(f, ground));
            contrast(&onto(front, &back), &back)
        })
        .reduce(f64::min)
        .map(|ratio| (ratio * 100.0).floor() / 100.0)
}

/// **The parts whose words are a mark, not words** (#3099): a tab's ×, and the title bar's ⌄
/// and pin, each a glyph to be told apart, held at 3:1 (WCAG 1.4.11) in the faint tier
/// ([`super::check::FAINT`]), as an icon and a select's chevron are.
const MARK_WORDS: &[(&str, &str)] = &[("tabs", "closer"), ("titlebar", "all"), ("titlebar", "pin")];

/// Whether `part` of `family` draws a mark rather than words: an icon, the chevron of a select
/// or a combobox, or one of [`MARK_WORDS`].
fn is_mark(family: &Family, part: &Part) -> bool {
    matches!(part.name, "icon" | "chevron") || MARK_WORDS.contains(&(family.name, part.name))
}

/// The floor a part's words or marks are held to: 4.5:1 for words, 3:1 for a mark.
fn floor_of(family: &Family, part: &Part) -> f64 {
    if is_mark(family, part) { 3.0 } else { 4.5 }
}

/// **What a look of a family drawn in marks breaks** (#3143), if anything: each part it draws,
/// held at 3:1 as a non-text mark on what it is drawn on (the root's fill over each of the
/// family's grounds, or the grounds themselves for the root), with more contrast asked for
/// raised as [`super::Least::floor`] raises it. A part it does not draw (`transparent`) holds
/// nothing. Its line at rest is a hairline the token floors hold, and so is that same line in a
/// state that does not change it; the grip, wider than that line, is then read on the grounds.
fn faint_mark(
    theme: &ResolvedTheme,
    rules: &[Rule],
    family: &Family,
    variant: &str,
    states: &[State],
    look: &Drawn,
) -> Option<(String, Option<f64>)> {
    if states.is_empty() {
        return None;
    }
    let line = look.marks[0] == drawn(rules, family, variant, &[]).marks[0];
    // With the keyboard on it, it draws a mark of its own: a focused handle that draws nothing
    // has no focus indicator (WCAG 2.4.7).
    if states.contains(&State::Focused)
        && look
            .marks
            .iter()
            .enumerate()
            .all(|(at, mark)| *mark == TRANSPARENT || (at == 0 && line))
    {
        return Some((
            "a handle with the keyboard on it draws its grip or its line in a state of its own"
                .to_owned(),
            None,
        ));
    }
    let floor = theme.least.floor(3.0);
    for (at, part) in family.parts.iter().enumerate() {
        if at == 0 && line {
            continue;
        }
        let on = if at == 0 || line {
            TRANSPARENT
        } else {
            look.marks[0]
        };
        if let Some(ratio) = lowest(theme, &grounds(theme, family), on, look.marks[at])
            && ratio < floor
        {
            return Some((
                format!(
                    "the {}'s mark contrasts {floor}:1 with what it is drawn on",
                    part.name
                ),
                Some(ratio),
            ));
        }
    }
    None
}

/// The value `rules` give `property` of `family`'s root in `variant` at rest, if any sets it:
/// the most specific rule that sets that property, as the stylesheet's order decides.
fn drawn_at_rest(
    rules: &[Rule],
    family: &Family,
    variant: &str,
    property: Prop,
) -> Option<PartValue> {
    let mut ordered: Vec<&Rule> = rules
        .iter()
        .filter(|rule| {
            std::ptr::eq(rule.family, family)
                && rule.part == 0
                && rule.state.is_none()
                && rule.reaches(variant, &[])
        })
        .collect();
    ordered.sort_by_key(|rule| rule.order());
    ordered
        .iter()
        .rev()
        .find_map(|rule| rule.values.get(&property).copied())
}

/// Whether `variant`'s root draws a border: a width above 0, the family's unless a rule sets one.
fn has_border(rules: &[Rule], family: &Family, variant: &str) -> bool {
    !matches!(
        drawn_at_rest(rules, family, variant, Prop::BorderWidth),
        Some(PartValue::Length(Length::Number(width))) if width == 0.0
    )
}

/// How far apart two drawn colours are in OKLab; `None` when either is not drawn (transparent,
/// or the platform's).
fn apart(theme: &ResolvedTheme, a: &str, b: &str) -> Option<f64> {
    let (a, b) = (hex(theme, a)?, hex(theme, b)?);
    Some(tint::distance(tint::oklab_of(a)?, tint::oklab_of(b)?))
}

/// Whether two colours are drawn alike, closer than `least` in OKLab: both not drawn, or both
/// drawn and near.
fn alike(theme: &ResolvedTheme, a: &str, b: &str, least: f64) -> bool {
    match (hex(theme, a), hex(theme, b)) {
        (None, None) => true,
        (Some(_), Some(_)) => apart(theme, a, b).is_some_and(|distance| distance < least),
        _ => false,
    }
}

/// The edge a root draws, or `None` when it draws none: no width, or a transparent colour.
fn edge(rules: &[Rule], family: &Family, variant: &str, look: &Drawn) -> Option<&'static str> {
    (look.border != TRANSPARENT && has_border(rules, family, variant)).then_some(look.border)
}

/// What a look a theme changed breaks of the reading floors, if anything: each part's words at
/// 4.5:1 and its icon at 3:1 on its fill, and a border that is the part's only edge at 3:1, or,
/// while `states` hold [`State::Invalid`], the edge that says a value is refused (#3110), on any
/// fill; with more contrast asked for, each raised as [`super::Least::floor`] raises it (#2925).
fn unreadable(
    theme: &ResolvedTheme,
    on: &[String],
    rules: &[Rule],
    family: &Family,
    variant: &str,
    states: &[State],
    look: &Drawn,
) -> Option<(String, Option<f64>)> {
    for (at, part) in family.parts.iter().enumerate() {
        // With more contrast asked for, the reading floors are raised (#2925).
        let floor = theme.least.floor(floor_of(family, part));
        if let Some(ratio) = lowest(theme, on, look.grounds[at], look.words[at])
            && ratio < floor
        {
            let what = if is_mark(family, part) {
                "marks"
            } else {
                "words"
            };
            return Some((
                format!(
                    "the {}'s {what} contrast {floor}:1 with its fill",
                    part.name
                ),
                Some(ratio),
            ));
        }
    }
    // A border is the part's edge where its fill is not told from what it is drawn on, and a
    // refused box's edge is what says so at a glance, whatever its fill.
    if let Some(border) = edge(rules, family, variant, look) {
        let fill = hex(theme, look.fill);
        let on_its_ground = states.contains(&State::Invalid)
            || fill.is_none_or(|fill| {
                !is_opaque(fill)
                    || on
                        .iter()
                        .any(|ground| ground.eq_ignore_ascii_case(&solid(fill)))
            });
        let floor = theme.least.floor(3.0);
        if on_its_ground
            && let Some(ratio) = lowest(theme, on, look.fill, border)
            && ratio < floor
        {
            return Some((
                format!("the root's border contrasts {floor}:1 with its fill"),
                Some(ratio),
            ));
        }
    }
    None
}

/// The consent floor `variant` of `family` breaks in `state`, as `theme` draws it (as written,
/// or tinted), if any; and whether the variant it is held apart from must draw its default too
/// when the consent variant's own default still breaks it.
enum Consented {
    /// The consent variant draws its default.
    Own(String, Option<f64>),
    /// The consent variant draws its default, and so does `other` when that is not enough.
    Both(String, &'static str),
}

fn consent_broken(
    theme: &ResolvedTheme,
    on: &[String],
    rules: &[Rule],
    family: &Family,
    variant: &Variant,
    states: &[State],
) -> Option<Consented> {
    let look = drawn(rules, family, variant.name, states);
    let primary = drawn(rules, family, PRIMARY, states);
    let default_look = drawn(&[], family, variant.name, states);
    let default_primary = drawn(&[], family, PRIMARY, states);
    // Parts never bring a consent variant nearer the primary than the family's defaults are, in
    // this same drawn theme: the farthest of its fill, its edge and (for danger) its words.
    let words = variant.consent == Some(Consent::Danger);
    let nearer = look_apart(theme, on, &look, &primary, words)
        < look_apart(theme, on, &default_look, &default_primary, words) - NO_NEARER;
    match variant.consent? {
        Consent::Grants => {
            let own = edge(rules, family, variant.name, &look);
            let ratio = own.and_then(|border| lowest(theme, on, look.fill, border));
            if ratio.is_none_or(|ratio| ratio < 3.0) {
                return Some(Consented::Own(
                    "an answer that grants keeps its accent edge, 3:1 on its fill".to_owned(),
                    ratio,
                ));
            }
            let keep_blocked_name = family.variants[0].name;
            let keep_blocked = drawn(rules, family, keep_blocked_name, states);
            let default_keep_blocked = drawn(&[], family, keep_blocked_name, states);
            let default_allow = drawn(&[], family, variant.name, states);
            if !alike(
                theme,
                default_allow.border,
                default_keep_blocked.border,
                super::ALLOW_APART,
            ) && let (Some(own), Some(theirs)) =
                (own, edge(rules, family, keep_blocked_name, &keep_blocked))
                && alike(theme, own, theirs, super::ALLOW_APART)
            {
                return Some(Consented::Both(
                    "Allow is distinguishable from Keep blocked by its edge".to_owned(),
                    keep_blocked_name,
                ));
            }
            // Allow is as plain as Keep blocked: told apart by its edge and its lock alone.
            if alike(theme, default_look.fill, default_keep_blocked.fill, SEEN)
                && !alike(theme, look.fill, keep_blocked.fill, SEEN)
            {
                return Some(Consented::Both(
                    "Allow is drawn on Keep blocked's fill".to_owned(),
                    keep_blocked_name,
                ));
            }
            if nearer {
                return Some(Consented::Both(
                    "Allow never takes the primary look".to_owned(),
                    PRIMARY,
                ));
            }
        }
        Consent::Asks => {
            let own = edge(rules, family, variant.name, &look);
            let ratio = own.and_then(|border| lowest(theme, on, look.fill, border));
            if ratio.is_none_or(|ratio| ratio < 3.0) {
                return Some(Consented::Own(
                    "an ask keeps its waiting edge, 3:1 on its fill".to_owned(),
                    ratio,
                ));
            }
            // It reads as waiting, never as the accent the primary is drawn in: never nearer the
            // accent than the family's own edge is, once near it.
            let from_accent = |border: &str| apart(theme, border, "accent.base");
            let theirs = edge(&[], family, variant.name, &default_look).and_then(from_accent);
            if let Some(distance) = own.and_then(from_accent)
                && distance < super::DANGER_APART
                && theirs.is_none_or(|theirs| distance < theirs - NO_NEARER)
            {
                return Some(Consented::Own(
                    "an ask's waiting edge stays apart from the accent".to_owned(),
                    None,
                ));
            }
        }
        Consent::Danger => {
            if apart(theme, look.words[0], "accent.base")
                .is_some_and(|distance| distance < super::DANGER_APART)
            {
                return Some(Consented::Own(
                    "danger is distinguishable from primary: its words are apart from the accent"
                        .to_owned(),
                    None,
                ));
            }
            let words_apart = |a: &Drawn, b: &Drawn| {
                apart(theme, a.words[0], b.words[0])
                    .is_none_or(|distance| distance >= super::DANGER_APART)
            };
            if words_apart(&default_look, &default_primary) && !words_apart(&look, &primary) {
                return Some(Consented::Both(
                    "danger is distinguishable from primary: its words are apart from the \
                     primary's"
                        .to_owned(),
                    PRIMARY,
                ));
            }
            if nearer {
                return Some(Consented::Both(
                    "danger is distinguishable from primary".to_owned(),
                    PRIMARY,
                ));
            }
        }
    }
    None
}

/// **The rules a theme's parts give, with the floors held.** Each variant and state the theme
/// changes is held to the reading floors, and so is each set of states that hold at once (hovered
/// and pressed, focused and selected) as the stylesheet's order resolves them. The consent floors
/// are held in every state but disabled and every set of them, whatever the theme changed, as
/// written and in each workspace tint the theme's recipe draws (ADR 0097 §6). A variant that
/// breaks one draws its family default there, said in the [`Fell`]s; where that is not enough, so
/// does the variant it is held apart from.
fn held(theme: &ResolvedTheme) -> (Vec<Rule>, Vec<Fell>) {
    held_from(theme, rules(&theme.parts), Vec::new())
}

/// The fallbacks a theme's held rules would still write if they were held once more: none, for
/// the hold ends only when a round writes none.
#[cfg(test)]
pub(super) fn held_again(theme: &ResolvedTheme) -> Vec<Fell> {
    let (rules, fell) = held(theme);
    let before = fell.len();
    let (_, again) = held_from(theme, rules, fell);
    again[before..].to_vec()
}

/// Whether `family`'s root, as the theme's held rules draw it while `states` (with
/// [`State::Invalid`] among them) hold, tells the refusal apart from the same box not refused.
#[cfg(test)]
pub(super) fn refusal_shown(theme: &ResolvedTheme, family_name: &str, states: &[State]) -> bool {
    let family = family(family_name).expect("a family of the manifest");
    let rules = &held_once(theme).0;
    let look = drawn(rules, family, "", states);
    !refusal_hidden(theme, rules, family, "", states, &look)
}

/// [`held`], from `rules` and what already `fell`.
fn held_from(
    theme: &ResolvedTheme,
    mut rules: Vec<Rule>,
    mut fell: Vec<Fell>,
) -> (Vec<Rule>, Vec<Fell>) {
    if rules.is_empty() {
        return (rules, fell);
    }
    // As written, and tinted by every colour a workspace can name: each of the theme's own and
    // every whole degree a workspace's own colour can take (#2526).
    let mut drawn_in = vec![theme.clone()];
    // Only the consent floors are held in every tint: a theme whose parts reach no family with a
    // consent variant (Purlis Classic's activity items, #2917) is measured as written, and costs
    // a launch nothing more.
    let consents = MANIFEST.iter().any(|family| {
        family.variants.iter().any(|one| one.consent.is_some())
            && rules.iter().any(|rule| std::ptr::eq(rule.family, family))
            // Built only for a family measured in them (#2912): one measured as written alone
            // needs none.
            && (colours(theme, &rules, family) || every_tint())
    });
    if consents && !theme.tint.tabs().is_empty() {
        drawn_in.extend(
            theme
                .tint
                .hues
                .iter()
                .map(|one| tint::tinted(theme, one.name)),
        );
        drawn_in.extend((0..360).map(|degree| tint::tinted_at(theme, f64::from(degree))));
    }
    #[cfg(test)]
    DRAWN_IN.with(|it| it.set(drawn_in.len()));
    for family in MANIFEST {
        if !rules.iter().any(|rule| std::ptr::eq(rule.family, family)) {
            continue;
        }
        // A theme that turns borders off still draws the edge of an answer that grants, and an
        // ask's.
        for variant in family
            .variants
            .iter()
            .filter(|one| one.consent.is_some_and(Consent::keeps_edge))
        {
            if !has_border(&rules, family, variant.name) {
                rules.push(Rule {
                    family,
                    part: 0,
                    variant: Some(variant.name),
                    state: None,
                    values: BTreeMap::from([(
                        Prop::BorderWidth,
                        PartValue::Length(Length::Number(family.border_width)),
                    )]),
                    fallback: true,
                });
            }
        }
        // **A family whose rules set no colour is measured as written alone** (#2912): its
        // consent variants are drawn in the family's own colours in every tint, which the
        // token-level floors hold in each tint (`floors.rs`, #2526), and what the theme sets (an
        // edge's width, a size) is the same in every tint. A family whose rules set a colour is
        // measured in every tint, as before.
        let drawn_in: &[ResolvedTheme] = if colours(theme, &rules, family) || every_tint() {
            &drawn_in
        } else {
            &drawn_in[..1]
        };
        // At rest and in each state the family names but disabled: a disabled control is
        // exempt, as WCAG has it, and cannot be answered.
        let singles: Vec<Vec<State>> = std::iter::once(Vec::new())
            .chain(states_of(family).into_iter().map(|one| vec![one]))
            .collect();
        let mut held = Held {
            family,
            rules: &mut rules,
            fell: &mut fell,
        };
        // Held again until no fallback is written: a fallback changes how a state draws, so
        // whatever was measured against that state before is measured against what it draws
        // now. It ends, since each round that writes one names a part that had not fallen.
        loop {
            let written = held.rules.len();
            for states in &singles {
                held.readable(theme, states);
            }
            for states in &singles {
                held.consented(drawn_in, states);
            }
            // Then each set of states that hold at once whose look no state alone, nor another
            // set already measured, draws. A fallback can name a state, so the sets are read
            // again; they end, since there are finitely many.
            let mut measured: Vec<Alike> = singles
                .iter()
                .map(|states| alike_as(held.rules, family, states))
                .collect();
            loop {
                let next: Vec<Vec<State>> = combined(family)
                    .into_iter()
                    .filter(|states| {
                        let one = alike_as(held.rules, family, states);
                        let new = !measured.contains(&one);
                        if new {
                            measured.push(one);
                        }
                        new
                    })
                    .collect();
                if next.is_empty() {
                    break;
                }
                for states in &next {
                    held.readable(theme, states);
                    held.consented(drawn_in, states);
                }
            }
            if held.rules.len() == written {
                break;
            }
        }
    }
    // A surface's fill the theme sets is a ground of every family drawn on it (#3099, from
    // #3083): held until no surface falls back, since each that does draws its own fill again.
    while held_on_surfaces(theme, &mut drawn_in, &mut rules, &mut fell) {}
    #[cfg(test)]
    DRAWN_IN.with(|it| it.set(drawn_in.len()));
    (rules, fell)
}

#[cfg(test)]
thread_local! {
    /// Set by a test: every family is measured in every tint, skip or none.
    static EVERY_TINT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Whether every family is measured in every tint: only ever, in a test, to hold the skip to it.
fn every_tint() -> bool {
    #[cfg(test)]
    {
        EVERY_TINT.with(std::cell::Cell::get)
    }
    #[cfg(not(test))]
    {
        false
    }
}

/// **What falls of `theme` with every family measured in every tint**, for the test that holds a
/// family measured as written alone to the same.
#[cfg(test)]
pub(super) fn fallen_in_every_tint(theme: &ResolvedTheme) -> Vec<Fell> {
    EVERY_TINT.with(|one| one.set(true));
    let fell = held(theme).1;
    EVERY_TINT.with(|one| one.set(false));
    fell
}

/// **Whether a rule's colour only takes an edge off a variant that carries no consent**: the same
/// in every tint, and it only stands that variant further from the answers the floors hold apart
/// from it (#2912).
/// **Whether `family`'s rules set a colour a tint can change what its consent floors read**: any
/// colour but an edge taken off a variant with no consent ([`edge_taken_off`]) and an untinted
/// ground of a family that holds answers ([`ground_held`]).
fn colours(theme: &ResolvedTheme, rules: &[Rule], family: &Family) -> bool {
    rules.iter().any(|rule| {
        std::ptr::eq(rule.family, family)
            && !rule.fallback
            && rule.values.iter().any(|(property, value)| {
                property.is_colour()
                    && !edge_taken_off(rule, *property, *value)
                    && !ground_held(theme, rule, *property, *value)
            })
    })
}

/// **Whether a rule only sets a ground no tint reaches, of a family that holds answers** (a
/// card's or a Notice's fill, #2912): its token, and each of the surfaces the family stands on,
/// is outside the theme's tint recipe, so every tint draws the ground and what it is laid over as
/// written, and what the family's consent floors read there is what they read as written. The
/// ground is one of the button family's own (a key naming another is refused), on which the floors
/// hold Allow apart from the primary in every tint already. A tinted ground, or one over a tinted
/// surface, is measured in every tint: a translucent ground laid over a surface, or two colours
/// near each other, need not keep their contrast when a tint turns them.
fn ground_held(theme: &ResolvedTheme, rule: &Rule, property: Prop, value: PartValue) -> bool {
    let PartValue::Colour(token) = value else {
        return false;
    };
    let tinted = theme.tint.tabs();
    property == Prop::Fill
        && rule.part == 0
        && rule.state.is_none()
        && content::HOLDS_ANSWERS.contains(&rule.family.name)
        && !tinted.contains(&token)
        && rule
            .family
            .grounds
            .iter()
            .all(|ground| !tinted.contains(ground))
}

fn edge_taken_off(rule: &Rule, property: Prop, value: PartValue) -> bool {
    property == Prop::BorderColour
        && value == PartValue::Colour(TRANSPARENT)
        && rule
            .variant
            .and_then(|name| variant(rule.family.name, name))
            .is_some_and(|one| one.consent.is_none())
}

/// A surface's look whose fill a theme sets: the surface, its variant, its default fill and the
/// fill drawn in its place.
type SurfaceSet = (&'static Family, &'static str, &'static str, &'static str);

/// **The fills a theme's parts set on the surfaces other families are drawn on**: each surface's
/// look whose root draws, at rest, another fill than its family default's, with that default and
/// the fill drawn in its place. A family drawn on the default is drawn on the fill set.
fn surfaces_set(rules: &[Rule]) -> Vec<SurfaceSet> {
    let mut found = Vec::new();
    for surface in MANIFEST {
        if !surface.parts[0].properties.contains(&Prop::Fill)
            || !rules.iter().any(|rule| std::ptr::eq(rule.family, surface))
        {
            continue;
        }
        for variant in surface.looks() {
            let default = (surface.defaults)(variant, &[]).fill;
            let set = drawn(rules, surface, variant, &[]).fill;
            if default != TRANSPARENT && set != TRANSPARENT && set != default {
                found.push((surface, variant, default, set));
            }
        }
    }
    found
}

/// **The surfaces each family with a consent variant is drawn on** (#3083): a button stands on
/// a panel, in a popover and in a dialog, and a menu's row in the popover a menu is. A ground
/// token a family names can be more than one surface's (the overlay is a popover's and a
/// dialog's), so where a family stands is said here, not read from its grounds.
const STANDS_ON: &[(&str, &[&str])] = &[
    ("panel", &["button"]),
    ("popover", &["button", "menu"]),
    ("dialog", &["button"]),
];

/// **The fill-taking families no consent family stands on**, each with why (#3083 review): a
/// family whose root takes a fill and whose default fill is a ground a family with a consent
/// variant names is in [`STANDS_ON`] or here, and a test holds every one to one list or the
/// other, so a new Allow or danger answer on a filled surface cannot go unmeasured unseen.
/// Every family is measured on a filled surface once #3268 lands.
#[cfg(test)]
pub(super) const HOLDS_NO_CONSENT: &[(&str, &str)] = &[
    (
        "text-input",
        "a field's box holds the words typed into it, never an answer",
    ),
    (
        "textarea",
        "a field's box holds the words typed into it, never an answer",
    ),
    (
        "editor",
        "its toolbar holds ghost and plain buttons, never an Allow or a danger answer (#3268)",
    ),
    ("tooltip", "it holds words only"),
    ("select", "a control's box holds its value and its chevron"),
    (
        "combobox",
        "a control's box holds its value and its chevron",
    ),
    (
        "option-card",
        "a choice holds its title, its detail and its check, never an answer",
    ),
];

/// The families drawn on `surface` whose grounds name its `default` fill but not `set`: what a
/// surface whose fill a theme turns from `default` to `set` brings a new ground under.
fn drawn_on(
    surface: &'static Family,
    default: &str,
    set: &str,
) -> impl Iterator<Item = &'static Family> {
    let on: &[&str] = STANDS_ON
        .iter()
        .find(|(name, _)| *name == surface.name)
        .map_or(&[], |(_, families)| *families);
    MANIFEST.iter().filter(move |family| {
        on.contains(&family.name)
            && family.grounds.contains(&default)
            && !family.grounds.contains(&set)
    })
}

/// `set` as `theme` draws it under what stands on `surface`: laid over each ground the surface
/// itself is drawn on, for a fill with alpha.
fn surface_ground(theme: &ResolvedTheme, surface: &Family, set: &str) -> Vec<String> {
    let Some(fill) = hex(theme, set) else {
        return grounds(theme, surface);
    };
    if is_opaque(fill) {
        return vec![solid(fill)];
    }
    grounds(theme, surface)
        .iter()
        .map(|under| onto(fill, under))
        .collect()
}

/// **The floor a family breaks on a surface's fill a theme sets**, if any: each of its looks
/// at rest and in each state but disabled, held to the reading floors as written, and its
/// consent variants to the consent floors in every drawn theme, on that fill alone.
fn broken_on(
    theme: &ResolvedTheme,
    drawn_in: &[ResolvedTheme],
    rules: &[Rule],
    family: &Family,
    surface: &Family,
    set: &str,
) -> Option<(String, Option<f64>)> {
    let singles: Vec<Vec<State>> = std::iter::once(Vec::new())
        .chain(states_of(family).into_iter().map(|one| vec![one]))
        .collect();
    let on = surface_ground(theme, surface, set);
    for states in &singles {
        for variant in family.looks() {
            let look = drawn(rules, family, variant, states);
            if let Some(broken) = unreadable(theme, &on, rules, family, variant, states, &look) {
                return Some(broken);
            }
        }
    }
    for one in drawn_in {
        let on = surface_ground(one, surface, set);
        for states in &singles {
            for variant in family.variants.iter().filter(|one| one.consent.is_some()) {
                match consent_broken(one, &on, rules, family, variant, states) {
                    Some(Consented::Own(floor, ratio)) => return Some((floor, ratio)),
                    Some(Consented::Both(floor, _)) => return Some((floor, None)),
                    None => {}
                }
            }
        }
    }
    None
}

/// **Each surface whose fill a theme sets, held under what is drawn on it** (#3099, from
/// #3083): every family drawn on the surface's default ground is measured on the fill set in its
/// place, and a surface under which one breaks a floor draws its family default there, said in
/// a [`Fell`] that names the family. Whether one fell, so the hold is asked again.
fn held_on_surfaces(
    theme: &ResolvedTheme,
    drawn_in: &mut Vec<ResolvedTheme>,
    rules: &mut Vec<Rule>,
    fell: &mut Vec<Fell>,
) -> bool {
    let before = fell.len();
    for (surface, variant, default, set) in surfaces_set(rules) {
        for family in drawn_on(surface, default, set) {
            // The consent floors hold in every tint, a surface's fill under a consent variant
            // included (ADR 0097 §6), as they do where a theme's parts reach the family itself.
            let consent = family.variants.iter().any(|one| one.consent.is_some());
            if consent && drawn_in.len() == 1 && !theme.tint.tabs().is_empty() {
                drawn_in.extend(
                    theme
                        .tint
                        .hues
                        .iter()
                        .map(|one| tint::tinted(theme, one.name)),
                );
                drawn_in.extend((0..360).map(|degree| tint::tinted_at(theme, f64::from(degree))));
            }
            if let Some((floor, ratio)) = broken_on(theme, drawn_in, rules, family, surface, set) {
                Held {
                    family: surface,
                    rules: &mut *rules,
                    fell: &mut *fell,
                }
                .fall(
                    variant,
                    &[],
                    format!("what a {} draws on it: {floor}", family.name),
                    ratio,
                );
                break;
            }
        }
    }
    fell.len() > before
}

/// The states `family` names but disabled, in the order the stylesheet writes them.
fn states_of(family: &Family) -> Vec<State> {
    STATES
        .into_iter()
        .filter(|one| *one != State::Disabled && family.states.contains(one))
        .collect()
}

/// Every set of two or more of the states `family` names that a part can be in at once, but
/// disabled: the smaller first.
fn combined(family: &Family) -> Vec<Vec<State>> {
    // A family keyed on marks of its own (#3143) is dragged alone: the panel library's
    // `data-separator` is `active` then and nothing else, and no keyboard mark is on it. Under
    // the pointer and with the keyboard can hold at once.
    if !own_states(family).is_empty() {
        let both = vec![State::Hover, State::Focused];
        return if both.iter().all(|one| family.states.contains(one)) {
            vec![both]
        } else {
            Vec::new()
        };
    }
    let states = states_of(family);
    let mut sets: Vec<Vec<State>> = (0u32..1 << states.len())
        .filter(|bits| bits.count_ones() >= 2)
        .map(|bits| {
            states
                .iter()
                .enumerate()
                .filter(|(at, _)| bits & (1 << at) != 0)
                .map(|(_, one)| *one)
                .collect()
        })
        .collect();
    sets.sort_by_key(Vec::len);
    sets
}

/// What decides how a family is drawn while `states` hold: which of them its rules name, and
/// its defaults in each variant. Two sets alike in it draw every variant alike.
type Alike = (Vec<State>, Vec<Look>);

fn alike_as(rules: &[Rule], family: &Family, states: &[State]) -> Alike {
    let named: Vec<State> = states
        .iter()
        .copied()
        .filter(|state| {
            rules
                .iter()
                .any(|rule| std::ptr::eq(rule.family, family) && rule.state == Some(*state))
        })
        .collect();
    let defaults = family
        .looks()
        .into_iter()
        .map(|variant| (family.defaults)(variant, states))
        .collect();
    (named, defaults)
}

/// The floors of one family being held: its rules, with the fallbacks added, and what fell.
struct Held<'a> {
    family: &'static Family,
    rules: &'a mut Vec<Rule>,
    fell: &'a mut Vec<Fell>,
}

impl Held<'_> {
    /// Each variant the theme changes while `states` hold, held to the reading floors.
    fn readable(&mut self, theme: &ResolvedTheme, states: &[State]) {
        for variant in self.family.looks() {
            let look = drawn(self.rules, self.family, variant, states);
            // A refused box's edge is held whether or not the theme names its refused state: a
            // rule at rest or in another state reaches it too (#3110).
            if states.contains(&State::Invalid)
                && refusal_hidden(theme, self.rules, self.family, variant, states, &look)
            {
                self.fall(variant, states, REFUSAL_HIDDEN.to_owned(), None);
                // The refused default is the edge the theme already draws when the box is not
                // refused (a danger edge at rest): then that edge draws its default too, so the
                // two are told apart again (#3382's review, B1).
                let redrawn = drawn(self.rules, self.family, variant, states);
                if refusal_hidden(theme, self.rules, self.family, variant, states, &redrawn) {
                    let valid: Vec<State> = states
                        .iter()
                        .copied()
                        .filter(|one| *one != State::Invalid)
                        .collect();
                    self.fall(variant, &valid, REFUSAL_HIDDEN.to_owned(), None);
                }
                continue;
            }
            if look == drawn(&[], self.family, variant, states) {
                continue;
            }
            let broken = if marked(self.family).is_some() {
                faint_mark(theme, self.rules, self.family, variant, states, &look)
            } else {
                let on = grounds(theme, self.family);
                unreadable(theme, &on, self.rules, self.family, variant, states, &look)
            };
            if let Some((floor, ratio)) = broken {
                self.fall(variant, states, floor, ratio);
            }
        }
    }

    /// Each consent variant while `states` hold, held to the consent floors in every drawn theme.
    fn consented(&mut self, drawn_in: &[ResolvedTheme], states: &[State]) {
        let family = self.family;
        for variant in family.variants.iter().filter(|one| one.consent.is_some()) {
            for one in drawn_in {
                // A variant already drawn as its default is held by the token floors; a
                // fallback would change nothing, so what it is held apart from falls back.
                let own_default = drawn(self.rules, family, variant.name, states)
                    == drawn(&[], family, variant.name, states);
                let on = grounds(one, family);
                match consent_broken(one, &on, self.rules, family, variant, states) {
                    None => {}
                    Some(Consented::Own(floor, ratio)) => {
                        if !own_default {
                            self.fall(variant.name, states, floor, ratio);
                        }
                    }
                    Some(Consented::Both(floor, other)) => {
                        if !own_default {
                            self.fall(variant.name, states, floor.clone(), None);
                        }
                        if consent_broken(one, &on, self.rules, family, variant, states).is_some() {
                            self.fall(other, states, floor, None);
                        }
                    }
                }
            }
        }
    }

    /// `variant` draws its family default while `states` hold, said once in a [`Fell`] that names
    /// them. The default is written for one state: in a set, the last of them any of the
    /// family's rules names. A fallback is written after every theme rule, so the set draws that
    /// state's default, which the floors already hold.
    fn fall(&mut self, variant: &'static str, states: &[State], floor: String, ratio: Option<f64>) {
        let family = self.family;
        let part = named(family, variant, states);
        if self.fell.iter().any(|one| one.part == part) {
            return;
        }
        self.fell.push(Fell { part, floor, ratio });
        // A refused box draws its refused default in any set it is refused in, written after
        // the theme's rules, so its danger edge is back whatever else holds (#3110).
        let state = if states.contains(&State::Invalid) {
            Some(State::Invalid)
        } else {
            states
                .iter()
                .rev()
                .find(|state| {
                    self.rules.iter().any(|rule| {
                        std::ptr::eq(rule.family, family) && rule.state == Some(**state)
                    })
                })
                .or(states.last())
                .copied()
        };
        // A family with no variants has one look, which a rule for every variant reaches.
        let reached = (!variant.is_empty()).then_some(variant);
        let written = self.rules.iter().any(|rule| {
            std::ptr::eq(rule.family, family)
                && rule.fallback
                && rule.variant == reached
                && rule.state == state
                && !rule.values.contains_key(&Prop::BorderWidth)
        });
        if !written {
            self.rules.extend(fallback(family, variant, state));
        }
    }
}

/// `<family>.<root>[.<variant>]`, with the states that hold joined by `+` (`hover+pressed`).
fn named(family: &Family, variant: &str, states: &[State]) -> String {
    let mut named = look_named(family, family.parts[0].name, variant);
    if !states.is_empty() {
        let names: Vec<&str> = states.iter().map(|state| state.name()).collect();
        let _ = write!(named, ".{}", names.join("+"));
    }
    named
}

/// The rules that draw `family`'s default colours back for `variant` in `state`: the root's,
/// and the words of every other part following it.
fn fallback(family: &'static Family, variant: &'static str, state: Option<State>) -> Vec<Rule> {
    let look = (family.defaults)(variant, state.as_slice());
    // A family with no variants has one look, which a rule for every variant reaches.
    let variant = (!variant.is_empty()).then_some(variant);
    // Only the colours the root takes: a field's root draws none of its own.
    let root: BTreeMap<Prop, PartValue> = [
        (Prop::Fill, look.fill),
        (Prop::Text, look.text),
        (Prop::BorderColour, look.border),
    ]
    .into_iter()
    .filter(|(property, _)| family.parts[0].properties.contains(property))
    .map(|(property, token)| (property, PartValue::Colour(token)))
    .collect();
    let mut rules = Vec::new();
    if !root.is_empty() {
        rules.push(Rule {
            family,
            part: 0,
            variant,
            state,
            values: root,
            fallback: true,
        });
    }
    for part in 1..family.parts.len() {
        // A part of a family drawn in marks draws its own fill back, for the state (#3143).
        let by_state = marked(family).map(|by_default| by_default(part, state.as_slice()));
        if let Some(fill) = family.own_fill(part).or(by_state) {
            rules.push(Rule {
                family,
                part,
                variant,
                state,
                values: BTreeMap::from([(Prop::Fill, PartValue::Colour(fill))]),
                fallback: true,
            });
        }
        if family.parts[part].properties.contains(&Prop::Text) {
            let words = family.own_words(part).unwrap_or(look.text);
            rules.push(Rule {
                family,
                part,
                variant,
                state,
                values: BTreeMap::from([(Prop::Text, PartValue::Colour(words))]),
                fallback: true,
            });
        }
    }
    rules
}

/// **The parts of `theme` drawn as their family default because they broke a floor**, each with
/// the floor: empty for a theme whose parts clear them all, and for one that names no parts.
pub fn fallen(theme: &ResolvedTheme) -> Vec<Fell> {
    held_once(theme).1.clone()
}

/// The rules with the floors held, and what fell.
type Measured = (Vec<Rule>, Vec<Fell>);

/// How many themes' measures are kept: every appearance and option of a gallery's worth of
/// parts themes.
const KEPT: usize = 64;

/// The themes measured last, the latest first, each with its measure.
type Kept = VecDeque<(ResolvedTheme, Arc<Measured>)>;

#[cfg(not(test))]
static MEASURED: Mutex<Kept> = Mutex::new(VecDeque::new());

#[cfg(test)]
thread_local! {
    /// Each test thread keeps its own, so what one test measures is never another's.
    static MEASURED: &'static Mutex<Kept> = Box::leak(Box::new(Mutex::new(VecDeque::new())));
}

fn measured() -> std::sync::MutexGuard<'static, Kept> {
    #[cfg(not(test))]
    let kept: &'static Mutex<Kept> = &MEASURED;
    #[cfg(test)]
    let kept: &'static Mutex<Kept> = MEASURED.with(|kept| *kept);
    kept.lock().unwrap_or_else(PoisonError::into_inner)
}

/// **A theme's parts are measured once** (#2954): [`held`] passes over every workspace tint, and
/// writing the stylesheet, reporting what fell and the gallery's cards all ask for the same
/// theme. The measure is a function of the whole theme, so a theme equal in every value is the
/// same theme, and the last [`KEPT`] are kept. A theme that names no parts costs nothing.
fn held_once(theme: &ResolvedTheme) -> Arc<Measured> {
    if theme.parts.is_empty() {
        return Arc::new((Vec::new(), Vec::new()));
    }
    let kept = |measured: &mut Kept| {
        let at = measured.iter().position(|(one, _)| one == theme)?;
        let hit = measured.remove(at)?;
        let held = Arc::clone(&hit.1);
        measured.push_front(hit);
        Some(held)
    };
    if let Some(held) = kept(&mut measured()) {
        return held;
    }
    #[cfg(test)]
    PASSES.with(|passes| passes.set(passes.get() + 1));
    let held = Arc::new(held(theme));
    let mut kept_now = measured();
    if let Some(already) = kept(&mut kept_now) {
        return already;
    }
    kept_now.push_front((theme.clone(), Arc::clone(&held)));
    kept_now.truncate(KEPT);
    held
}

#[cfg(test)]
thread_local! {
    /// The passes [`held`] made on this thread, for the tests that hold the sharing.
    static PASSES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
thread_local! {
    /// How many looks the last [`held`] on this thread measured the consent floors in: the theme
    /// as written, and each tint where its parts reach a consent family (#2917).
    static DRAWN_IN: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// The looks the last measurement on this thread held the consent floors in.
#[cfg(test)]
pub(super) fn drawn_in() -> usize {
    DRAWN_IN.with(std::cell::Cell::get)
}

/// The passes over a theme's tints made on this thread so far.
#[cfg(test)]
pub(super) fn passes() -> usize {
    PASSES.with(std::cell::Cell::get)
}

// --- The stylesheet --------------------------------------------------------------------------

/// The custom property a token is written to: `surface.base` is `--surface-base`.
fn var(token: &str) -> String {
    format!("var(--{})", token.replace('.', "-"))
}

fn written_length(length: Length, unit: geometry::Unit) -> String {
    match length {
        Length::Token(token) => geometry::step_of(token)
            .map_or_else(String::new, |step| format!("var({})", step.property)),
        Length::Number(number) => match unit {
            geometry::Unit::Text => geometry::text_length(number),
            geometry::Unit::Px => format!("{}px", geometry::show(number)),
            geometry::Unit::Weight | geometry::Unit::Ratio => geometry::show(number),
        },
    }
}

/// The custom property a root's height is written to as well, which its corners are clamped
/// by: set by a rule that sets the height, so a variant's own height reaches a corner a rule for
/// every variant sets.
const HEIGHT_PROPERTY: &str = "--purlis-part-height";

/// Half the root's height as it is drawn, which a corner is clamped to: `None` for a box as
/// tall as what it holds, whose corners the browser keeps inside it.
fn half_height(family: &Family) -> Option<String> {
    let own = match family.height? {
        Length::Token(token) => geometry::step_of(token)
            .map_or_else(String::new, |step| format!("var({})", step.property)),
        Length::Number(px) => format!("{}px", geometry::show(px)),
    };
    Some(format!("calc(var({HEIGHT_PROPERTY}, {own}) / 2)"))
}

/// The unit a rule's length for `property` is written in: its bound's, a mark's height in px.
fn unit_of(rule: &Rule, property: Prop) -> geometry::Unit {
    let part = rule.family.parts[rule.part].name;
    if property == Prop::Height && MARKS.contains(&(rule.family.name, part)) {
        return geometry::Unit::Px;
    }
    property.bound().map_or(geometry::Unit::Px, |b| b.unit)
}

/// **The custom property a number the family's own stylesheet draws from is written to**
/// (#2914): a flare and an inset are drawn by the tab's own outlines, and the room between tabs
/// is the strip's.
fn written_to(rule: &Rule, property: Prop) -> Option<&'static str> {
    let part = rule.family.parts[rule.part].name;
    Some(match (rule.family.name, part, property) {
        ("tabs", "tab", Prop::Flare) => "--tabs-flare",
        ("tabs", "tab", Prop::Inset) => "--tabs-hover-inset",
        ("tabs", "layout", Prop::Gap) => "--tabs-gap",
        _ => return None,
    })
}

/// **The elements a rule's `property` is drawn on** (#2914): its own, but a tab's fill under the
/// pointer is its pill, which an unchosen tab draws, and a chosen tab's fill is its flares' too.
fn targets(rule: &Rule, property: Prop, sized: bool) -> Vec<String> {
    let own = selector(rule, sized);
    let part = rule.family.parts[rule.part].name;
    // **A chat pane's band draws its Notices' ground and words itself** (#3142, #2912): the
    // state washes, never tinted, and the primary tier on them. A theme's Notice colours reach a
    // Notice anywhere else, so none can draw a band Notice in the terminal's own fill.
    if rule.family.name == "notice" && matches!(property, Prop::Fill | Prop::Text) {
        return vec![own.replacen(
            "[data-part=\"root\"]",
            "[data-part=\"root\"]:not([data-placement=\"pane\"])",
            1,
        )];
    }
    if (rule.family.name, part, property) != ("tabs", "tab", Prop::Fill) {
        return vec![own];
    }
    match rule.state {
        Some(State::Hover) => root_selectors(rule, sized)
            .iter()
            .map(|root| format!(":where({root}:not([data-selected]))::before"))
            .collect(),
        Some(State::Selected) => vec![
            own.clone(),
            format!("{own}::before"),
            format!("{own}::after"),
        ],
        _ => vec![own],
    }
}

/// The selectors of the parts drawn around the tabs, and of a chosen tab and its flares.
const LAYOUT: &str = ":where([data-scope=\"tabs\"][data-part=\"layout\"])";
const STRIP: &str = ":where([data-scope=\"tabs\"][data-part=\"strip\"])";
const CHOSEN: &str = ":where([data-scope=\"tabs\"][data-part=\"tab\"][data-selected]:not([data-variant=\"file\"])), \
     :where([data-scope=\"tabs\"][data-part=\"tab\"][data-selected]:not([data-variant=\"file\"]))::before, \
     :where([data-scope=\"tabs\"][data-part=\"tab\"][data-selected]:not([data-variant=\"file\"]))::after";

/// **What the tabs attach to, as the rules it writes** (#2914), before every other rule so a
/// fill a theme names wins: on a strip of their own (`strip`) the strip takes the chat strip's
/// shade, a chosen tab is a step lighter and nothing flares
/// (`--tabs-joined`, which `tabs.css` draws the flares and the panel's corners from); docked on
/// the panel (`panel`, Purlis's) the strip has no ground and a chosen tab takes the fill of what
/// it joins: a chat's its terminal's, a file's its toolbar's, a view's the panel's.
fn attached(word: &str) -> &'static [(&'static str, &'static str)] {
    if word == "strip" {
        &[
            (LAYOUT, "--tabs-joined: 0;"),
            (STRIP, "background-color: var(--tabs-strip-shade);"),
            (CHOSEN, "background-color: var(--tabs-chosen-shade);"),
        ]
    } else {
        // Docked, each chosen tab takes the family default of what it joins.
        &[
            (LAYOUT, "--tabs-joined: 1;"),
            (STRIP, "background-color: transparent;"),
        ]
    }
}

/// **What a layout word draws** (#2914), as custom properties `tabs.css` reads, or an
/// indicator's edge. `attach` is written first, on its own ([`attached`]).
fn word_css(property: Prop, word: &str) -> &'static str {
    match (property, word) {
        (Prop::Sizing, "equal") => "--tabs-grow: 1; --tabs-basis: 0px;",
        (Prop::Sizing, _) => "--tabs-grow: 0; --tabs-basis: auto;",
        (Prop::Dividers, "on") => "--tabs-divider: 1px;",
        (Prop::Dividers, _) => "--tabs-divider: 0px;",
        (Prop::Position, "top") => "inset-block-start: 0; inset-block-end: auto;",
        (Prop::Position, _) => "inset-block-start: auto; inset-block-end: 0;",
        _ => "",
    }
}

/// A colour as a stylesheet writes it.
fn colour_css(token: &str) -> String {
    if token == TRANSPARENT {
        TRANSPARENT.to_owned()
    } else {
        var(token)
    }
}

/// One property's declarations, written from its parsed value.
fn declarations(rule: &Rule, property: Prop, value: PartValue) -> String {
    if let Some(custom) = written_to(rule, property) {
        let unit = unit_of(rule, property);
        let written = match value {
            PartValue::Colour(token) => colour_css(token),
            PartValue::Length(length) => written_length(length, unit),
            PartValue::Inset(sides) => sides
                .iter()
                .map(|side| written_length(*side, geometry::Unit::Px))
                .collect::<Vec<_>>()
                .join(" "),
            PartValue::Padding(..)
            | PartValue::Corners(_)
            | PartValue::Shadow(_)
            | PartValue::Word(_) => return String::new(),
        };
        let mut said = format!("{custom}: {written};");
        // A pill that fills its cell takes no corners of its own; one inset from it is rounded.
        if let PartValue::Inset(sides) = value
            && sides.iter().all(|side| *side == Length::Number(0.0))
        {
            said.push_str(" --tabs-hover-corner: 0px;");
        }
        return said;
    }
    // **A panel's fill is said to what it holds too** (#3331): `--panel-fill`, so a part of a
    // view that must be opaque over the view's own rows (a sticky head) takes the panel's
    // colour, whichever the theme names, and not the window's.
    if (
        rule.family.name,
        rule.family.parts[rule.part].name,
        property,
    ) == ("panel", "root", Prop::Fill)
        && let PartValue::Colour(token) = value
    {
        let fill = colour_css(token);
        return format!("background-color: {fill}; --panel-fill: {fill};");
    }
    // A strip's line is under it, never around it.
    if (rule.family.name, rule.family.parts[rule.part].name) == ("tabs", "strip") {
        match (property, value) {
            (Prop::BorderWidth, PartValue::Length(length)) => {
                return format!(
                    "border-block-end-width: {}; border-block-end-style: solid;",
                    written_length(length, geometry::Unit::Px)
                );
            }
            (Prop::BorderColour, PartValue::Colour(token)) => {
                return format!("border-block-end-color: {};", colour_css(token));
            }
            _ => {}
        }
    }
    let css = property.css();
    match value {
        PartValue::Word(word) => word_css(property, word).to_owned(),
        PartValue::Inset(_) => String::new(),
        PartValue::Colour(TRANSPARENT) => format!("{}: transparent;", css[0]),
        PartValue::Colour(token) => format!("{}: {};", css[0], var(token)),
        PartValue::Shadow(Shadow::None) => format!("{}: none;", css[0]),
        PartValue::Shadow(Shadow::Raised) => format!("{}: var(--elevation-raised);", css[0]),
        PartValue::Shadow(Shadow::Overlay) => format!("{}: var(--elevation-overlay);", css[0]),
        PartValue::Padding(block, inline) => format!(
            "{}: {}; {}: {};",
            css[0],
            written_length(block, geometry::Unit::Text),
            css[1],
            written_length(inline, geometry::Unit::Text)
        ),
        PartValue::Corners(corners) => {
            // A corner is clamped to half the root's height as it is drawn. `full` is the
            // largest corner, which the browser draws as a pill at any height.
            // A control's corner is clamped to half the height it is drawn at; a box's, which is as
            // tall as what it holds, to half its own box, corner by corner (`50%` resolves on each
            // axis), and `full` on a box is that half (ADR 0103, amended 2026-10-22).
            // A family that holds its corner to less draws at most that (a dialog's, the inset
            // of what it holds, so its answers are never cut off in a corner, #2910).
            let control = (rule.part == 0).then(|| half_height(rule.family)).flatten();
            let most = if rule.part == 0 {
                rule.family.corner_most
            } else {
                None
            };
            let one = |corner: Corner| match (corner, &control, most) {
                (Corner::Full, _, Some(most)) => var(most),
                (Corner::Length(length), _, Some(most)) => format!(
                    "min({}, {})",
                    written_length(length, geometry::Unit::Px),
                    var(most)
                ),
                (Corner::Full, Some(_), None) => format!("{}px", geometry::show(MOST_CORNER)),
                (Corner::Full, None, None) => "50%".to_owned(),
                (Corner::Length(length), Some(half), None) => {
                    format!(
                        "min({}, {half})",
                        written_length(length, geometry::Unit::Px)
                    )
                }
                (Corner::Length(length), None, None) => {
                    format!("min({}, 50%)", written_length(length, geometry::Unit::Px))
                }
            };
            let written: Vec<String> = if corners.iter().all(|c| *c == corners[0]) {
                vec![one(corners[0])]
            } else {
                corners.iter().map(|c| one(*c)).collect()
            };
            format!("{}: {};", css[0], written.join(" "))
        }
        PartValue::Length(length) => {
            let unit = unit_of(rule, property);
            let mut said = format!("{}: {};", css[0], written_length(length, unit));
            if property == Prop::BorderWidth {
                said.push_str(" border-style: solid;");
            }
            if property == Prop::Height && rule.part == 0 {
                let _ = write!(
                    said,
                    " {HEIGHT_PROPERTY}: {};",
                    written_length(length, unit)
                );
            }
            said
        }
    }
}

/// The selectors a rule's root is reached by: the wrapper's attributes, and with `sized` none of
/// the roots drawn at another of the family's sizes or in a row that keeps their size (the
/// family's `keeps_size`), whose size the family's own stylesheet keeps.
fn root_selectors(rule: &Rule, sized: bool) -> Vec<String> {
    let family = rule.family;
    let mut data = format!(
        "[data-scope=\"{}\"][data-part=\"{}\"]",
        family.name, family.parts[0].name
    );
    if let Some(variant) = rule.variant {
        let _ = write!(data, "[data-variant=\"{variant}\"]");
    }
    if sized && !family.keeps_size.is_empty() {
        let _ = write!(data, ":not({})", family.keeps_size.join(", "));
    }
    if let Some(state) = rule.state {
        data.push_str(family.attribute(state));
    }
    vec![data]
}

/// A rule's selector, without specificity: the stylesheet's order alone says which rule wins.
fn selector(rule: &Rule, sized: bool) -> String {
    let part = rule.family.parts[rule.part].name;
    if rule.family.around.contains(&part) {
        return format!(
            ":where([data-scope=\"{}\"][data-part=\"{part}\"])",
            rule.family.name
        );
    }
    let roots = root_selectors(rule, sized);
    let reached: Vec<String> = if rule.part == 0 {
        roots
    } else {
        let name = rule.family.parts[rule.part].name;
        // A toned part keeps its tone's colours whatever a theme sets (#2917): the needs-you
        // count stays hot (R120).
        let untoned = if TONED.contains(&(rule.family.name, name)) {
            ":not([data-tone])"
        } else {
            ""
        };
        let part = format!(
            "[data-scope=\"{}\"][data-part=\"{}\"]{untoned}",
            rule.family.name, name
        );
        roots.iter().map(|root| format!("{root} {part}")).collect()
    };
    format!(":where({})", reached.join(", "))
}

/// The parts whose tone (`data-tone`) is drawn by the family alone: a theme's colours reach them
/// only where no tone is set (the activity item's count, whose needs-you tone is hot, R120).
const TONED: &[(&str, &str)] = &[("activity", "count")];

/// Whether a property changes a control's size: what the roots at another of the family's sizes
/// and in a wrapping row keep as their own.
fn sizes(property: Prop) -> bool {
    matches!(
        property,
        Prop::Padding
            | Prop::Gap
            | Prop::FontSize
            | Prop::FontWeight
            | Prop::Height
            | Prop::BorderWidth
    )
}

/// **The stylesheet the window draws `theme`'s parts with**: one rule per part, variant and state
/// the theme sets, keyed on `data-scope` / `data-part`, the variant and the state's attribute, in
/// the `parts` layer above the family defaults, every value written from its parsed form. Empty
/// for a theme that names no parts, which draws exactly as the family defaults do.
pub fn stylesheet(theme: &ResolvedTheme) -> String {
    let mut rules = held_once(theme).0.clone();
    if rules.is_empty() {
        return String::new();
    }
    rules.sort_by_key(Rule::order);
    let mut sheet = String::from("@layer parts {\n");
    // The roots at another of the family's sizes are clamped to their own height, in each
    // family whose corners the theme sets.
    let corners = |family: &Family| {
        rules.iter().any(|rule| {
            std::ptr::eq(rule.family, family)
                && rule.part == 0
                && rule.values.contains_key(&Prop::CornerRadius)
        })
    };
    for family in MANIFEST.iter().filter(|family| corners(family)) {
        for (root, height) in family.sized_apart {
            let _ = writeln!(sheet, "  :where({root}) {{ {HEIGHT_PROPERTY}: {height}; }}");
        }
    }
    // What the tabs attach to, first: a fill a theme names over it wins.
    for rule in &rules {
        if let Some(PartValue::Word(word)) = rule.values.get(&Prop::Attach) {
            for (on, said) in attached(word) {
                let _ = writeln!(sheet, "  {on} {{ {said} }}");
            }
        }
    }
    // **The separator's colour reaches every line between groups** (#2912): the other families'
    // own lines (a menu's rule, a dialog's footer rule, the status bar's line), written before
    // the families' rules so a family's own key for its line wins, and the hairlines a
    // stylesheet draws as a row's edge, which read `--separator-fill`.
    if let Some(PartValue::Colour(token)) = rules
        .iter()
        .rev()
        .filter(|rule| rule.family.name == "separator" && rule.part == 0 && rule.state.is_none())
        .find_map(|rule| rule.values.get(&Prop::Fill))
    {
        let fill = if *token == TRANSPARENT {
            TRANSPARENT.to_owned()
        } else {
            var(token)
        };
        let _ = writeln!(
            sheet,
            "  :where({}) {{ background-color: {fill}; }}",
            SEPARATOR_LINES.join(", ")
        );
        let _ = writeln!(sheet, "  :root {{ --separator-fill: {fill}; }}");
    }
    // **The card's ground and edge width reach the Inbox's list of Notices** (#2912, #3177): the
    // list is a card of rows, so a theme that draws its cards on another ground, or with no
    // hairline, draws the list so. Only a ground the Notices' own words are measured on (or none)
    // reaches it: its rows hold Allow, and a news row never takes another tone's ground. The
    // card's edge width also reaches a Notice alone on a page, whose subtle hairline is an
    // inline surface's edge, not the waiting edge an ask keeps.
    for property in [Prop::Fill, Prop::BorderWidth] {
        let Some(rule) = rules.iter().rev().find(|rule| {
            rule.family.name == "card"
                && rule.part == 0
                && rule.variant.is_none()
                && rule.state.is_none()
                && rule.values.contains_key(&property)
        }) else {
            continue;
        };
        let value = rule.values[&property];
        let reaches = match (property, value) {
            (Prop::Fill, PartValue::Colour(token)) => (token == TRANSPARENT
                || content::NOTICE.grounds.contains(&token))
            .then_some(CARD_EDGED),
            (Prop::BorderWidth, _) => Some(CARD_EDGED_TOO),
            _ => None,
        };
        if let Some(reaches) = reaches {
            let said = declarations(rule, property, value);
            let _ = writeln!(sheet, "  :where({reaches}) {{ {said} }}");
        }
    }
    for rule in &rules {
        for sized in [false, true] {
            // Each target with what is drawn on it, in the order the properties are written.
            let mut blocks: Vec<(String, Vec<String>)> = Vec::new();
            for property in PROPERTIES
                .iter()
                .filter(|property| sizes(**property) == sized)
            {
                let Some(value) = rule.values.get(property) else {
                    continue;
                };
                let said = declarations(rule, *property, *value);
                if said.is_empty() {
                    continue;
                }
                for target in targets(rule, *property, sized) {
                    match blocks.iter_mut().find(|(at, _)| *at == target) {
                        Some((_, list)) => list.push(said.clone()),
                        None => blocks.push((target, vec![said.clone()])),
                    }
                }
            }
            for (target, said) in blocks {
                let _ = writeln!(sheet, "  {target} {{ {} }}", said.join(" "));
            }
        }
    }
    sheet.push_str("}\n");
    sheet
}

/// The other families' lines between groups, which the separator part's colour reaches too.
const SEPARATOR_LINES: &[&str] = &[
    "[data-scope=\"popover\"][data-part=\"rule\"]",
    "[data-scope=\"dialog\"][data-part=\"separator\"]",
    "[data-scope=\"status-bar\"][data-part=\"separator\"]",
];

/// **What takes the card's ground and edge width** (#3177): the Inbox's list of Notices, a card of rows, in
/// the Inbox and on the page a window with no project shows (not its band or its lines, which
/// draw no box).
const CARD_EDGED: &str =
    "[data-scope=\"notice-list\"][data-part=\"root\"]:is([data-at=\"inbox\"], [data-at=\"page\"])";

/// **What takes the card's edge width alone**: the list, and a Notice alone on a page, an inline
/// surface whose hairline is decorative (#3177, the operator's no-hairline ruling).
const CARD_EDGED_TOO: &str = "[data-scope=\"notice-list\"][data-part=\"root\"]:is([data-at=\"inbox\"], [data-at=\"page\"]), \
     [data-scope=\"notice\"][data-part=\"root\"][data-placement=\"alone\"]";

// --- The manifest as JSON ----------------------------------------------------------------------

fn bound_json(property: Prop) -> Value {
    if let Some(words) = property.keywords() {
        return json!({ "enum": words });
    }
    let Some(bound) = property.bound() else {
        return if property == Prop::Shadow {
            json!({ "enum": ["none", "raised", "overlay"] })
        } else {
            json!({ "tokens": "colour", "or": [TRANSPARENT] })
        };
    };
    let unit = match bound.unit {
        geometry::Unit::Text => "px at the default text size",
        geometry::Unit::Px => "px",
        geometry::Unit::Weight => "font weight",
        geometry::Unit::Ratio => "ratio",
    };
    let mut said = json!({
        "least": geometry::show(bound.least).parse::<i64>().unwrap_or_default(),
        "most": geometry::show(bound.most).parse::<i64>().unwrap_or_default(),
        "unit": unit,
    });
    if let Some(prefix) = bound.tokens {
        said["tokens"] = Value::from(format!("{prefix}*"));
    }
    if property == Prop::CornerRadius {
        said["or"] = json!(["full"]);
        said["clamped"] = Value::from("half the part's height");
    }
    if property == Prop::Padding {
        said["block_most"] = Value::from(12);
    }
    if property == Prop::Inset {
        said["values"] = Value::from("one, or three: top, sides, bottom");
    }
    said
}

/// **The closed manifest as `purlis theme parts` prints it**: the grammar, the states with the
/// attribute each is keyed on, the properties with their bounds and whether a state may set
/// them, and every family with its parts and variants, each with its description and the
/// manifest version it came in. What no release has shipped yet is in it, with `since` [`NEXT`].
pub fn manifest_json() -> Value {
    // `VERSION + 1` and not the marker itself, which the release step replaces where it is
    // written.
    manifest_as_of(VERSION + 1)
}

/// **The manifest as it stood at `version`**: only what came in at that version or before. As of
/// [`VERSION`] it is what the last release shipped, which a test holds to that version's digest,
/// so an unreleased addition never changes what a released theme was written against.
pub fn manifest_as_of(version: u32) -> Value {
    let states: Vec<Value> = STATES
        .iter()
        .filter(|state| state.since() <= version)
        .map(|state| {
            json!({
                "name": state.name(),
                "attribute": state.attribute(),
                "description": state.says(),
                "since": state.since(),
            })
        })
        .collect();
    let properties: Vec<Value> = PROPERTIES
        .iter()
        .filter(|property| property.since() <= version)
        .map(|property| {
            json!({
                "name": property.name(),
                "in_state": property.in_state(),
                "only_in": property.only_in().map(State::name),
                "bounds": bound_json(*property),
                "description": property.says(),
                "since": property.since(),
            })
        })
        .collect();
    let families: Vec<Value> = MANIFEST
        .iter()
        .filter(|family| family.since <= version)
        .map(|family| {
            let mut said = json!({
                "name": family.name,
                "description": family.description,
                "since": family.since,
                "focused": family.focused,
                "states": family.states.iter().filter(|state| state.since() <= version).map(|state| state.name()).collect::<Vec<_>>(),
                "parts": family.parts.iter().filter(|part| part.since <= version).map(|part| {
                    let mut said = json!({
                        "name": part.name,
                        "description": part.description,
                        "since": part.since,
                        "properties": part.properties.iter().filter(|p| p.since() <= version).map(|p| p.name()).collect::<Vec<_>>(),
                    });
                    // Only what a family has that the first did not, so the button's entry
                    // reads as it did at version 2.
                    if family.around.contains(&part.name) {
                        said["around"] = Value::Bool(true);
                    }
                    if MARKS.contains(&(family.name, part.name)) {
                        said["mark_height"] = json!({
                            "least": 0,
                            "most": geometry::show(MOST_MARK).parse::<i64>().unwrap_or_default(),
                            "unit": "px",
                        });
                    }
                    said
                }).collect::<Vec<_>>(),
                "variants": family.variants.iter().filter(|variant| variant.since <= version).map(|variant| json!({
                    "name": variant.name,
                    "description": variant.description,
                    "since": variant.since,
                    "consent": variant.consent.is_some(),
                })).collect::<Vec<_>>(),
            });
            // Only what a family has that the others do not (#3143): drawn in marks, and keyed on
            // attributes of its own in a state.
            if marked(family).is_some() {
                said["marks"] = Value::Bool(true);
            }
            if !own_states(family).is_empty() {
                said["attributes"] = own_states(family)
                    .iter()
                    .map(|(state, attribute)| (state.name().to_owned(), Value::from(*attribute)))
                    .collect::<Map<String, Value>>()
                    .into();
            }
            said
        })
        .collect();
    json!({
        "version": VERSION,
        "grammar": "<family>.<part>[.<variant>][.<state>].<property>",
        "rest": "the state at rest is implied: a key without a state",
        "least_target": LEAST_TARGET as i64,
        "no_size_in_a_state": NO_SIZE_IN_A_STATE,
        "states": states,
        "properties": properties,
        "families": families,
    })
}

/// **Whether the manifest holds anything no release has shipped yet**: a family, a part, a
/// variant, a state or a property `since` [`NEXT`].
pub fn has_unreleased() -> bool {
    STATES.iter().any(|state| state.since() > VERSION)
        || PROPERTIES.iter().any(|property| property.since() > VERSION)
        || MANIFEST.iter().any(|family| {
            family.since > VERSION
                || family.parts.iter().any(|part| part.since > VERSION)
                || family
                    .variants
                    .iter()
                    .any(|variant| variant.since > VERSION)
        })
}

/// The manifest's version as the schema says it: the last release's, and what has been added
/// since, when there is any.
fn version_said() -> String {
    if has_unreleased() {
        format!("{VERSION}, plus unreleased additions marked since {NEXT}")
    } else {
        VERSION.to_string()
    }
}

/// **The schema of a theme's `parts` block**: every key the manifest has, each with the schema
/// of its value, for an editor to complete and check.
pub(super) fn schema() -> Value {
    let mut keys = Map::new();
    // By the families' names, not the manifest's order: the committed schema is written by every
    // pull request that adds a family, and two that add two families then add their keys in two
    // places (operator ruling 2026-10-26), where a family appended at the manifest's end would
    // land beside every other.
    let mut by_name: Vec<&Family> = MANIFEST.iter().collect();
    by_name.sort_by_key(|family| family.name);
    for family in by_name {
        for part in family.parts {
            let variants =
                std::iter::once(None).chain(family.variants.iter().map(|one| Some(one.name)));
            for variant in variants {
                let states = std::iter::once(None).chain(family.states.iter().copied().map(Some));
                for state in states {
                    for property in part.properties {
                        if state.is_some() && !property.in_state() {
                            continue;
                        }
                        if property.only_in().is_some_and(|only| state != Some(only)) {
                            continue;
                        }
                        if family.around.contains(&part.name)
                            && (variant.is_some() || state.is_some())
                        {
                            continue;
                        }
                        let key = Key {
                            family: family.name,
                            part: part.name,
                            variant,
                            state,
                            property: *property,
                        };
                        let def = if *property == Prop::Height
                            && MARKS.contains(&(family.name, part.name))
                        {
                            "mark-height".to_owned()
                        } else {
                            property.name().to_owned()
                        };
                        keys.insert(
                            key.to_string(),
                            json!({ "$ref": format!("#/$defs/part-{def}") }),
                        );
                    }
                }
            }
        }
    }
    json!({
        "description": format!("Component parts (manifest version {}): keys <family>.<part>[.<variant>][.<state>].<property>, the state at rest implied. A property not named is the family default. Inside a state only fill, text, border-colour and shadow may be set, and a tab's flare and hover inset: a state never changes a part's size. `purlis theme parts` lists every family, part, state and property with its bounds.", version_said()),
        "type": "object",
        "additionalProperties": false,
        "properties": keys,
    })
}

/// The `$defs` of each property's value, added to the theme schema.
pub(super) fn schema_defs() -> Map<String, Value> {
    let colour_tokens: Vec<&str> = TOKENS.to_vec();
    let mut defs = Map::new();
    for property in PROPERTIES {
        let def = match property {
            Prop::Fill | Prop::BorderColour => json!({
                "description": property.says(),
                "enum": colour_tokens.iter().copied().chain([TRANSPARENT]).collect::<Vec<_>>(),
            }),
            Prop::Text => json!({ "description": property.says(), "enum": colour_tokens }),
            Prop::Shadow => json!({
                "description": property.says(),
                "enum": ["none", "raised", "overlay"],
            }),
            Prop::Attach | Prop::Sizing | Prop::Dividers | Prop::Position => json!({
                "description": property.says(),
                "enum": property.keywords().unwrap_or_default(),
            }),
            _ => {
                let bound = property.bound().unwrap_or(Bound {
                    least: 0.0,
                    most: 0.0,
                    unit: geometry::Unit::Px,
                    tokens: None,
                });
                let number = |most: f64| {
                    json!({
                        "type": "integer",
                        "minimum": geometry::show(bound.least).parse::<i64>().unwrap_or_default(),
                        "maximum": geometry::show(most).parse::<i64>().unwrap_or_default(),
                    })
                };
                let tokens: Vec<&str> = bound
                    .tokens
                    .map(|prefix| {
                        geometry::STEPS
                            .iter()
                            .map(|step| step.token)
                            .filter(|token| token.starts_with(prefix))
                            .collect()
                    })
                    .unwrap_or_default();
                let one = |most: f64| {
                    if tokens.is_empty() {
                        number(most)
                    } else {
                        json!({ "anyOf": [number(most), { "enum": tokens }] })
                    }
                };
                match property {
                    Prop::CornerRadius => {
                        let corner = json!({ "anyOf": [one(bound.most), { "const": "full" }] });
                        json!({
                            "description": property.says(),
                            "anyOf": [corner, { "type": "array", "items": corner, "minItems": 4, "maxItems": 4 }],
                        })
                    }
                    Prop::Padding => json!({
                        "description": property.says(),
                        "anyOf": [
                            one(MOST_BLOCK),
                            { "type": "array", "prefixItems": [one(MOST_BLOCK), one(bound.most)], "items": false, "minItems": 2 }
                        ],
                    }),
                    Prop::Inset => json!({
                        "description": property.says(),
                        "anyOf": [
                            one(bound.most),
                            { "type": "array", "items": one(bound.most), "minItems": 3, "maxItems": 3 }
                        ],
                    }),
                    _ => {
                        let mut def = one(bound.most);
                        def["description"] = Value::from(property.says());
                        def
                    }
                }
            }
        };
        defs.insert(format!("part-{}", property.name()), def);
    }
    defs.insert(
        "part-mark-height".to_owned(),
        json!({
            "description": "A mark's height, in px: 0 for none.",
            "type": "integer",
            "minimum": 0,
            "maximum": geometry::show(MOST_MARK).parse::<i64>().unwrap_or_default(),
        }),
    );
    defs
}

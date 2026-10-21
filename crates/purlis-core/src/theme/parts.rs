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
//!   bottom left), clamped as it is drawn to half the part's height, so `full` is a pill.
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
//! an icon 3:1). A part that breaks one draws its family default there. The consent parts keep
//! their marks: Allow's accent edge cannot be turned off (`border-width` 0 is refused on it, and
//! a theme that turns borders off everywhere still draws it), and Allow never takes the primary
//! look.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use serde_json::{Map, Value, json};

use super::check::{contrast, onto};
use super::geometry::{self, Geometry};
use super::tint;
use super::vocabulary::{PLATFORM, TOKENS};
use super::{Complaint, ResolvedTheme, stringify};

/// **The manifest's version**: it grows when a family, a part, a variant or a property is added,
/// and never takes one away.
pub const VERSION: u32 = 1;

/// The least height a control keeps, in px at the default text size: WCAG 2.2's target size.
pub const LEAST_TARGET: f64 = 24.0;

/// Why a state may not change a size, as the refusal says it.
pub const NO_SIZE_IN_A_STATE: &str = "a state never changes a part's size, so nothing moves when \
     the pointer, a press, the keyboard or a choice reaches it; inside a state only fill, text, \
     border-colour and shadow may be set";

/// **One state of a part**, the state at rest left implied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum State {
    Hover,
    Pressed,
    Selected,
    Focused,
    Disabled,
    Open,
}

/// Every state a key may name, in the order the stylesheet writes them.
pub const STATES: [State; 6] = [
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
        }
    }

    /// The attribute React Aria sets on a part in this state, which the stylesheet keys on.
    pub fn attribute(self) -> &'static str {
        match self {
            Self::Hover => "[data-hovered]",
            Self::Pressed => "[data-pressed]",
            Self::Selected => "[data-selected]",
            Self::Focused => "[data-focus-visible]",
            Self::Disabled => "[data-disabled]",
            Self::Open => "[data-open]",
        }
    }

    /// The same state on today's control classes, until a family moves to its wrapper.
    fn on_a_class(self) -> &'static str {
        match self {
            Self::Hover => ":hover:not(:disabled, [aria-disabled=\"true\"])",
            Self::Pressed => ":active:not(:disabled, [aria-disabled=\"true\"])",
            Self::Selected => "[aria-pressed=\"true\"]",
            Self::Focused => ":focus-visible",
            Self::Disabled => ":is(:disabled, [aria-disabled=\"true\"])",
            Self::Open => "[aria-expanded=\"true\"]",
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
}

/// Every property, in the order the stylesheet writes them.
pub const PROPERTIES: [Prop; 11] = [
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
        }
    }

    /// **Whether a state may set it**: colours, fills and shadows, never a size.
    pub fn in_state(self) -> bool {
        matches!(
            self,
            Self::Fill | Self::Text | Self::BorderColour | Self::Shadow
        )
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
            Self::Fill | Self::Text | Self::BorderColour | Self::Shadow => None,
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
}

/// One variant of a family.
#[derive(Debug, Clone, Copy)]
pub struct Variant {
    pub name: &'static str,
    pub description: &'static str,
    pub consent: Option<Consent>,
    /// The class today's markup draws it with, until the family moves to its wrapper; `None`
    /// for the variant a class without one draws.
    class: Option<&'static str>,
}

/// One part of a family.
#[derive(Debug, Clone, Copy)]
pub struct Part {
    pub name: &'static str,
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
    /// The surfaces it is drawn on, which a see-through fill shows.
    pub grounds: &'static [&'static str],
    /// The class today's markup draws its root with, until it moves to its wrapper (#2904).
    class: Option<&'static str>,
    /// The root's height at rest, the size token it is drawn at, which a corner is clamped to.
    height: &'static str,
    /// Its words' size and line height at rest, and its border's width: what fits in its height.
    font_size: &'static str,
    line_height: f64,
    border_width: f64,
    /// Its colours at rest and in each state, as the window's own stylesheet draws them.
    defaults: fn(&str, Option<State>) -> Look,
}

/// What a theme may set on a part with words: every property.
const ALL: &[Prop] = &PROPERTIES;

/// **The closed manifest**: every family a theme may name, at [`VERSION`].
pub const MANIFEST: &[Family] = &[Family {
    name: "button",
    description: "A button, an icon button or a link button.",
    since: 1,
    parts: &[
        Part {
            name: "root",
            description: "The button itself: its fill, border, corners and size.",
            properties: ALL,
        },
        Part {
            name: "label",
            description: "Its words.",
            properties: &[Prop::Text, Prop::FontSize, Prop::FontWeight],
        },
        Part {
            name: "icon",
            description: "Its icon.",
            properties: &[Prop::Text],
        },
    ],
    variants: &[
        Variant {
            name: "secondary",
            description: "The plain button.",
            consent: None,
            class: None,
        },
        Variant {
            name: "primary",
            description: "The answer purlis suggests.",
            consent: None,
            class: Some("ui-button-primary"),
        },
        Variant {
            name: "ghost",
            description: "A button with no box until the pointer reaches it.",
            consent: None,
            class: Some("ui-button-ghost"),
        },
        Variant {
            name: "danger",
            description: "What cannot be undone.",
            consent: Some(Consent::Danger),
            class: Some("ui-button-danger"),
        },
        Variant {
            name: "grants",
            description: "An answer that grants: an Allow, an Accept.",
            consent: Some(Consent::Grants),
            class: Some("ui-button-grants"),
        },
    ],
    grounds: &["surface.base", "surface.raised", "surface.overlay"],
    class: Some("ui-button"),
    height: "size.control",
    font_size: "font-size.base",
    line_height: 1.0,
    border_width: 1.0,
    defaults: button_look,
}];

/// The colours of a part: its fill, its words and its border, each a token or `transparent`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Look {
    fill: &'static str,
    text: &'static str,
    border: &'static str,
}

/// `transparent`: the one colour a part may take that is not a token.
const TRANSPARENT: &str = "transparent";

/// **The button's colours as `app/src/primitives/controls.css` draws them**, which a test holds
/// to that file: at rest, under the pointer and pressed as a toggle (selected).
fn button_look(variant: &str, state: Option<State>) -> Look {
    let mut look = match variant {
        "primary" => Look {
            fill: "accent.surface",
            text: "text.primary",
            border: "accent.base",
        },
        "ghost" => Look {
            fill: TRANSPARENT,
            text: "text.primary",
            border: TRANSPARENT,
        },
        "danger" => Look {
            fill: "control.base",
            text: "danger.base",
            border: "border.strong",
        },
        "grants" => Look {
            fill: "control.base",
            text: "text.primary",
            border: "accent.base",
        },
        _ => Look {
            fill: "control.base",
            text: "text.primary",
            border: "border.strong",
        },
    };
    match state {
        Some(State::Hover) => {
            look.fill = match variant {
                "primary" => "tab.active",
                "ghost" => "surface.hover",
                "danger" => "danger.surface",
                _ => "control.hover",
            };
        }
        Some(State::Selected) => {
            look.fill = "accent.surface";
            look.border = "accent.base";
        }
        _ => {}
    }
    look
}

/// The variant every family's suggested answer is drawn in.
const PRIMARY: &str = "primary";

/// A variant's button colours as the engine holds them, for the test that holds them to
/// `controls.css`: at rest, under the pointer, and selected.
#[cfg(test)]
pub(super) fn defaults_json(variant: &str) -> Value {
    let rest = button_look(variant, None);
    let hover = button_look(variant, Some(State::Hover));
    let selected = button_look(variant, Some(State::Selected));
    json!({
        "fill": rest.fill,
        "text": rest.text,
        "border": rest.border,
        "hover": hover.fill,
        "selected": { "fill": selected.fill, "border": selected.border },
    })
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
}

/// **A theme's component parts**, every key and its value: empty for a theme that names none.
pub type Parts = BTreeMap<Key, PartValue>;

/// The key `raw` names, or why it names nothing purlis has.
fn key(raw: &str) -> Result<Key, String> {
    let said = || {
        format!(
            "parts.{raw} is not a component part purlis has (`purlis theme parts` lists them), so \
             it was left out"
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
        state = Some(State::named(name).ok_or_else(said)?);
        next = middle.next();
    }
    if next.is_some() {
        return Err(said());
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
    let Some(bound) = property.bound() else {
        return Err(format!("{at} has no value purlis reads"));
    };
    match property {
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
                        most: 12.0,
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
                        most: 12.0,
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
                if let Some(said) = edge_removed(&key, &parsed) {
                    complaints.push(Complaint::said(said));
                    continue;
                }
                parts.insert(key, parsed);
            }
            Err(said) => complaints.push(Complaint::said(said)),
        }
    }
    parts
}

/// Why `key` would take a consent part's mark away, if it would: a border width of 0 on Allow.
fn edge_removed(key: &Key, value: &PartValue) -> Option<String> {
    let consent = key
        .variant
        .and_then(|name| variant(key.family, name))
        .and_then(|one| one.consent);
    (consent == Some(Consent::Grants)
        && key.property == Prop::BorderWidth
        && *value == PartValue::Length(Length::Number(0.0)))
    .then(|| {
        format!(
            "parts.{key} is refused: an answer that grants keeps its accent edge and its lock, \
             so a theme can never disguise a consent decision"
        )
    })
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

/// **Whether every control a theme's parts size stays a target and holds its words**, in both
/// densities (`comfortable` and `compact` are the appearance's geometry): each variant whose
/// size the theme sets is at least [`LEAST_TARGET`] px tall, and its words, its padding and its
/// border fit in that height. Each sentence says which does not, which refuses the theme.
pub(super) fn fits(parts: &Parts, comfortable: &Geometry, compact: &Geometry) -> Vec<String> {
    let mut said = Vec::new();
    for family in MANIFEST {
        let root = family.parts[0].name;
        for variant in family.variants {
            let sizes = [
                Prop::Height,
                Prop::Padding,
                Prop::FontSize,
                Prop::BorderWidth,
            ]
            .map(|property| at_rest(parts, family.name, root, variant.name, property));
            if sizes.iter().all(Option::is_none) {
                continue;
            }
            let label = at_rest(parts, family.name, "label", variant.name, Prop::FontSize);
            for (density, geometry) in [("comfortable", comfortable), ("compact", compact)] {
                let length_of = |value: Option<PartValue>, default: Length| match value {
                    Some(PartValue::Length(length)) => px(length, geometry),
                    _ => px(default, geometry),
                };
                let height = length_of(sizes[0], Length::Token(family.height));
                let padding = match sizes[1] {
                    Some(PartValue::Padding(block, _)) => px(block, geometry),
                    _ => 0.0,
                };
                let size = length_of(label.or(sizes[2]), Length::Token(family.font_size));
                let border = length_of(sizes[3], Length::Number(family.border_width));
                let named = format!("{}.{root}.{}", family.name, variant.name);
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
    /// The order the stylesheet writes rules in: the later wins.
    fn order(&self) -> (bool, bool, bool, usize) {
        (
            self.state.is_some(),
            self.variant.is_some(),
            self.fallback,
            self.part,
        )
    }

    fn reaches(&self, variant: &str, state: Option<State>) -> bool {
        self.variant.is_none_or(|v| v == variant) && (self.state.is_none() || self.state == state)
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

/// The colour token `rules` draw `property` of `part` in, for `variant` in `state`, or the
/// family default's; `None` when nothing sets it (a label's words follow the root's).
fn drawn_colour(
    rules: &[Rule],
    family: &Family,
    part: usize,
    variant: &str,
    state: Option<State>,
    property: Prop,
) -> Option<&'static str> {
    let mut ordered: Vec<&Rule> = rules
        .iter()
        .filter(|rule| {
            std::ptr::eq(rule.family, family) && rule.part == part && rule.reaches(variant, state)
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
    if set.is_some() || part != 0 {
        return set;
    }
    let look = (family.defaults)(variant, state);
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
}

fn drawn(rules: &[Rule], family: &Family, variant: &str, state: Option<State>) -> Drawn {
    let colour = |part, property| drawn_colour(rules, family, part, variant, state, property);
    let root_text = colour(0, Prop::Text).unwrap_or("text.primary");
    Drawn {
        fill: colour(0, Prop::Fill).unwrap_or(TRANSPARENT),
        border: colour(0, Prop::BorderColour).unwrap_or(TRANSPARENT),
        words: (0..family.parts.len())
            .map(|part| colour(part, Prop::Text).unwrap_or(root_text))
            .collect(),
    }
}

/// A token's colour in `theme`, `None` for `transparent` and for one the platform draws.
fn hex<'a>(theme: &'a ResolvedTheme, token: &str) -> Option<&'a str> {
    if token == TRANSPARENT {
        return None;
    }
    let value = theme.value(token);
    (value != PLATFORM && !value.is_empty()).then_some(value)
}

/// The lowest contrast `front` has on `fill` over each of the family's grounds.
fn lowest(theme: &ResolvedTheme, family: &Family, fill: &str, front: &str) -> Option<f64> {
    let front = hex(theme, front)?;
    family
        .grounds
        .iter()
        .filter_map(|ground| {
            let ground = hex(theme, ground)?;
            let back = hex(theme, fill).map_or_else(|| ground.to_owned(), |f| onto(f, ground));
            Some(contrast(&onto(front, &back), &back))
        })
        .reduce(f64::min)
        .map(|ratio| (ratio * 100.0).floor() / 100.0)
}

/// The floor a part's words or marks are held to: 4.5:1 for words, 3:1 for an icon.
fn floor_of(part: &Part) -> f64 {
    if part.name == "icon" { 3.0 } else { 4.5 }
}

/// What `look` breaks, for `variant` of `family` in `state`, if anything.
fn broken(
    theme: &ResolvedTheme,
    family: &Family,
    variant: &Variant,
    state: Option<State>,
    look: &Drawn,
    keep_blocked: &Drawn,
) -> Option<(String, Option<f64>)> {
    for (at, part) in family.parts.iter().enumerate() {
        let floor = floor_of(part);
        if let Some(ratio) = lowest(theme, family, look.fill, look.words[at])
            && ratio < floor
        {
            return Some((
                format!(
                    "the {}'s {} contrast {floor}:1 with its fill",
                    part.name,
                    if part.name == "icon" {
                        "marks"
                    } else {
                        "words"
                    }
                ),
                Some(ratio),
            ));
        }
    }
    if variant.consent == Some(Consent::Grants) && state != Some(State::Disabled) {
        let edge = lowest(theme, family, look.fill, look.border);
        if edge.is_none_or(|ratio| ratio < 3.0) {
            return Some((
                "an answer that grants keeps its accent edge, 3:1 on its fill".to_owned(),
                edge,
            ));
        }
        let apart = |a: &str, b: &str| match (hex(theme, a), hex(theme, b)) {
            (Some(a), Some(b)) => match (tint::oklab_of(a), tint::oklab_of(b)) {
                (Some(a), Some(b)) => tint::distance(a, b),
                _ => 0.0,
            },
            _ => 0.0,
        };
        if apart(look.border, keep_blocked.border) < super::ALLOW_APART {
            return Some((
                "Allow is distinguishable from Keep blocked by its edge".to_owned(),
                None,
            ));
        }
    }
    None
}

/// **The rules a theme's parts give, with the floors held**: each variant and state the theme
/// changes is measured, and one that breaks a floor draws its family default there, said in the
/// [`Fell`]s.
fn held(theme: &ResolvedTheme) -> (Vec<Rule>, Vec<Fell>) {
    let mut rules = rules(&theme.parts);
    let mut fell = Vec::new();
    if rules.is_empty() {
        return (rules, fell);
    }
    let states: Vec<Option<State>> = std::iter::once(None).chain(STATES.map(Some)).collect();
    for family in MANIFEST {
        if !rules.iter().any(|rule| std::ptr::eq(rule.family, family)) {
            continue;
        }
        for variant in family.variants {
            for state in &states {
                let look = drawn(&rules, family, variant.name, *state);
                let default = drawn(&[], family, variant.name, *state);
                if look == default {
                    continue;
                }
                let keep_blocked = drawn(&rules, family, family.variants[0].name, *state);
                if let Some((floor, ratio)) =
                    broken(theme, family, variant, *state, &look, &keep_blocked)
                {
                    fell.push(Fell {
                        part: named(family, variant.name, *state),
                        floor,
                        ratio,
                    });
                    rules.extend(fallback(family, variant.name, *state));
                }
            }
        }
        // Allow never takes the primary look: when it would, Allow draws its own default, and
        // when that is still the primary's look, so does the primary.
        let grants = family
            .variants
            .iter()
            .filter(|one| one.consent == Some(Consent::Grants));
        for allow in grants {
            for state in &states {
                let same = |rules: &[Rule]| {
                    let a = drawn(rules, family, allow.name, *state);
                    let b = drawn(rules, family, PRIMARY, *state);
                    a.fill == b.fill && a.border == b.border
                };
                // A state the family draws alike in both (a toggle that is on) is the family's.
                if same(&[]) || !same(&rules) {
                    continue;
                }
                fell.push(Fell {
                    part: named(family, allow.name, *state),
                    floor: "Allow never takes the primary look".to_owned(),
                    ratio: None,
                });
                rules.extend(fallback(family, allow.name, *state));
                if same(&rules) {
                    rules.extend(fallback(family, PRIMARY, *state));
                }
            }
        }
        // A theme that turns borders off everywhere still draws Allow's edge.
        for allow in family
            .variants
            .iter()
            .filter(|one| one.consent == Some(Consent::Grants))
        {
            let width = rules
                .iter()
                .filter(|rule| {
                    std::ptr::eq(rule.family, family)
                        && rule.part == 0
                        && rule.state.is_none()
                        && rule.reaches(allow.name, None)
                })
                .max_by_key(|rule| rule.order())
                .and_then(|rule| rule.values.get(&Prop::BorderWidth).copied());
            if width == Some(PartValue::Length(Length::Number(0.0))) {
                rules.push(Rule {
                    family,
                    part: 0,
                    variant: Some(allow.name),
                    state: None,
                    values: BTreeMap::from([(
                        Prop::BorderWidth,
                        PartValue::Length(Length::Number(family.border_width)),
                    )]),
                    fallback: true,
                });
            }
        }
    }
    (rules, fell)
}

fn named(family: &Family, variant: &str, state: Option<State>) -> String {
    let mut named = format!("{}.{}.{variant}", family.name, family.parts[0].name);
    if let Some(state) = state {
        let _ = write!(named, ".{}", state.name());
    }
    named
}

/// The rules that draw `family`'s default colours back for `variant` in `state`: the root's,
/// and the words of every other part following it.
fn fallback(family: &'static Family, variant: &'static str, state: Option<State>) -> Vec<Rule> {
    let look = (family.defaults)(variant, state);
    let mut rules = vec![Rule {
        family,
        part: 0,
        variant: Some(variant),
        state,
        values: BTreeMap::from([
            (Prop::Fill, PartValue::Colour(look.fill)),
            (Prop::Text, PartValue::Colour(look.text)),
            (Prop::BorderColour, PartValue::Colour(look.border)),
        ]),
        fallback: true,
    }];
    for part in 1..family.parts.len() {
        if family.parts[part].properties.contains(&Prop::Text) {
            rules.push(Rule {
                family,
                part,
                variant: Some(variant),
                state,
                values: BTreeMap::from([(Prop::Text, PartValue::Colour(look.text))]),
                fallback: true,
            });
        }
    }
    rules
}

/// **The parts of `theme` drawn as their family default because they broke a floor**, each with
/// the floor: empty for a theme whose parts clear them all, and for one that names no parts.
pub fn fallen(theme: &ResolvedTheme) -> Vec<Fell> {
    held(theme).1
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

/// The root's height in `rule`'s variant, as a CSS length: the theme's at rest, else the
/// family's.
fn height_of(rules: &[Rule], rule: &Rule) -> String {
    let variant = rule.variant;
    rules
        .iter()
        .filter(|one| {
            std::ptr::eq(one.family, rule.family)
                && one.part == 0
                && one.state.is_none()
                && (one.variant.is_none() || one.variant == variant)
        })
        .max_by_key(|one| one.order())
        .and_then(|one| match one.values.get(&Prop::Height) {
            Some(PartValue::Length(length)) => Some(written_length(*length, geometry::Unit::Text)),
            _ => None,
        })
        .unwrap_or_else(|| {
            geometry::step_of(rule.family.height)
                .map_or_else(String::new, |step| format!("var({})", step.property))
        })
}

/// One property's declarations, written from its parsed value.
fn declarations(rules: &[Rule], rule: &Rule, property: Prop, value: PartValue) -> String {
    let css = property.css();
    match value {
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
            // A corner is clamped to half the root's height as it is drawn, so `full` is a pill.
            let half = if rule.part == 0 {
                Some(format!("calc({} / 2)", height_of(rules, rule)))
            } else {
                None
            };
            let one = |corner: Corner| match (corner, &half) {
                (Corner::Full, Some(half)) => half.clone(),
                (Corner::Full, None) => format!("{}px", geometry::show(MOST_CORNER)),
                (Corner::Length(length), Some(half)) => {
                    format!(
                        "min({}, {half})",
                        written_length(length, geometry::Unit::Px)
                    )
                }
                (Corner::Length(length), None) => written_length(length, geometry::Unit::Px),
            };
            let written: Vec<String> = if corners.iter().all(|c| *c == corners[0]) {
                vec![one(corners[0])]
            } else {
                corners.iter().map(|c| one(*c)).collect()
            };
            format!("{}: {};", css[0], written.join(" "))
        }
        PartValue::Length(length) => {
            let unit = property.bound().map_or(geometry::Unit::Px, |b| b.unit);
            let mut said = format!("{}: {};", css[0], written_length(length, unit));
            if property == Prop::BorderWidth {
                said.push_str(" border-style: solid;");
            }
            said
        }
    }
}

/// The selectors a rule's root is reached by: the wrapper's attributes, and today's class while
/// the family is drawn by one.
fn root_selectors(rule: &Rule) -> Vec<String> {
    let family = rule.family;
    let mut data = format!(
        "[data-scope=\"{}\"][data-part=\"{}\"]",
        family.name, family.parts[0].name
    );
    if let Some(variant) = rule.variant {
        let _ = write!(data, "[data-variant=\"{variant}\"]");
    }
    if let Some(state) = rule.state {
        data.push_str(state.attribute());
    }
    let mut selectors = vec![data];
    if let Some(class) = family.class {
        let mut on = format!(".{class}");
        if let Some(name) = rule.variant {
            match family
                .variants
                .iter()
                .find(|one| one.name == name)
                .and_then(|one| one.class)
            {
                Some(own) => {
                    let _ = write!(on, ".{own}");
                }
                None => {
                    let others: Vec<String> = family
                        .variants
                        .iter()
                        .filter_map(|one| one.class.map(|c| format!(".{c}")))
                        .collect();
                    let _ = write!(on, ":not({})", others.join(", "));
                }
            }
        }
        if let Some(state) = rule.state {
            on.push_str(state.on_a_class());
        }
        selectors.push(on);
    }
    selectors
}

/// A rule's selector, without specificity: the stylesheet's order alone says which rule wins.
fn selector(rule: &Rule) -> String {
    let roots = root_selectors(rule);
    let reached: Vec<String> = if rule.part == 0 {
        roots
    } else {
        let part = format!(
            "[data-scope=\"{}\"][data-part=\"{}\"]",
            rule.family.name, rule.family.parts[rule.part].name
        );
        roots.iter().map(|root| format!("{root} {part}")).collect()
    };
    format!(":where({})", reached.join(", "))
}

/// **The stylesheet the window draws `theme`'s parts with**: one rule per part, variant and state
/// the theme sets, keyed on `data-scope` / `data-part`, the variant and the state's attribute, in
/// the `parts` layer above the family defaults, every value written from its parsed form. Empty
/// for a theme that names no parts, which draws exactly as the family defaults do.
pub fn stylesheet(theme: &ResolvedTheme) -> String {
    let (mut rules, _) = held(theme);
    if rules.is_empty() {
        return String::new();
    }
    rules.sort_by_key(Rule::order);
    let mut sheet = String::from("@layer parts {\n");
    for rule in &rules {
        let said: Vec<String> = PROPERTIES
            .iter()
            .filter_map(|property| {
                rule.values
                    .get(property)
                    .map(|value| declarations(&rules, rule, *property, *value))
            })
            .collect();
        let _ = writeln!(sheet, "  {} {{ {} }}", selector(rule), said.join(" "));
    }
    sheet.push_str("}\n");
    sheet
}

// --- The manifest as JSON ----------------------------------------------------------------------

fn bound_json(property: Prop) -> Value {
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
    said
}

/// **The closed manifest as `purlis theme parts` prints it**: the grammar, the states with the
/// attribute each is keyed on, the properties with their bounds and whether a state may set
/// them, and every family with its parts and variants, each with its description and the
/// manifest version it came in.
pub fn manifest_json() -> Value {
    let states: Vec<Value> = STATES
        .iter()
        .map(|state| {
            json!({
                "name": state.name(),
                "attribute": state.attribute(),
                "description": state.says(),
            })
        })
        .collect();
    let properties: Vec<Value> = PROPERTIES
        .iter()
        .map(|property| {
            json!({
                "name": property.name(),
                "in_state": property.in_state(),
                "bounds": bound_json(*property),
                "description": property.says(),
            })
        })
        .collect();
    let families: Vec<Value> = MANIFEST
        .iter()
        .map(|family| {
            json!({
                "name": family.name,
                "description": family.description,
                "since": family.since,
                "parts": family.parts.iter().map(|part| json!({
                    "name": part.name,
                    "description": part.description,
                    "properties": part.properties.iter().map(|p| p.name()).collect::<Vec<_>>(),
                })).collect::<Vec<_>>(),
                "variants": family.variants.iter().map(|variant| json!({
                    "name": variant.name,
                    "description": variant.description,
                    "consent": variant.consent.is_some(),
                })).collect::<Vec<_>>(),
            })
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

/// **The schema of a theme's `parts` block**: every key the manifest has, each with the schema
/// of its value, for an editor to complete and check.
pub(super) fn schema() -> Value {
    let mut keys = Map::new();
    for family in MANIFEST {
        for part in family.parts {
            let variants =
                std::iter::once(None).chain(family.variants.iter().map(|one| Some(one.name)));
            for variant in variants {
                let states = std::iter::once(None).chain(STATES.map(Some));
                for state in states {
                    for property in part.properties {
                        if state.is_some() && !property.in_state() {
                            continue;
                        }
                        let key = Key {
                            family: family.name,
                            part: part.name,
                            variant,
                            state,
                            property: *property,
                        };
                        keys.insert(
                            key.to_string(),
                            json!({ "$ref": format!("#/$defs/part-{}", property.name()) }),
                        );
                    }
                }
            }
        }
    }
    json!({
        "description": format!("Component parts (manifest version {VERSION}): keys <family>.<part>[.<variant>][.<state>].<property>, the state at rest implied. A property not named is the family default. Inside a state only fill, text, border-colour and shadow may be set: a state never changes a part's size. `purlis theme parts` lists every family, part, state and property with its bounds."),
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
                            one(12.0),
                            { "type": "array", "prefixItems": [one(12.0), one(bound.most)], "items": false, "minItems": 2 }
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
    defs
}

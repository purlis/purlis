// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What the person sets that every theme honours** (spec #2313, #2326, #2925, ADR 0097 §6):
//! density, the font-family override, reduced motion, the minimum contrast and their own
//! colours. Text size is the window's own setting (`textSize.ts`).
//!
//! They are kept on this machine, in the layout file beside the text sizes, under `appearance`:
//!
//! ```json
//! "appearance": {
//!   "density": "compact",
//!   "fonts": { "terminal": "Iosevka Term" },
//!   "motion": "reduced",
//!   "contrast": "more",
//!   "colours": { "accent.base": "#8f7bff" }
//! }
//! ```
//!
//! **Reduced motion and more contrast are layered on the operating system's** (#2925): either one
//! asking is enough. `motion: "reduced"` turns every motion off, as the system's reduced motion
//! does (`motion.ts`). `contrast: "more"` draws a theme's appearance tagged high contrast, as the
//! system's Increase contrast does, and raises the floors the engine holds ([`super::least`]):
//! words to 7:1 and marks to 4.5:1.
//!
//! **The person's colours are drawn over any theme, held to the floors** ([`Layer::over`]): a
//! colour that would put a pair of the check under its floor, and lower than the theme has it,
//! is left out with a sentence, and the consent floors are held after them, as after a theme's
//! own values.
//!
//! **The person's layer is drawn over the theme's** ([`Layer`]): a theme gives a comfortable and
//! a compact geometry, and the person's density says which is drawn; a font the person names is
//! drawn in place of the theme's, in every theme. A family the person names is held to the same
//! closed grammar as a theme's label ([`super::fonts::is_family`]) and written by the engine,
//! quoted, in front of core's family for the slot, so an installed face that is not there falls
//! back to core's.

use std::collections::BTreeMap;

use serde_json::Value;

use super::check::{self, PAIRS, Pair};
use super::fonts::{FAMILY_RULE, SLOTS, core_stack, is_family};
use super::least::{self, Least};
use super::vocabulary::{TOKENS, is_colour, is_platform};
use super::{Complaint, ResolvedTheme, stringify};

/// The settings `appearance` holds.
const KEYS: [&str; 5] = ["density", "fonts", "motion", "contrast", "colours"];

/// The modal scrim's token, which [`Layer::over`] holds to [`check::SCRIM_DIM`].
const SCRIM: &str = "overlay.scrim";

/// How tightly the window is laid out.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Density {
    #[default]
    Comfortable,
    Compact,
}

/// **The person's settings**, as read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Person {
    pub density: Density,
    /// The family the person named for each slot (`ui`, `mono`, `terminal`), when they did.
    pub fonts: BTreeMap<&'static str, String>,
    /// Whether the person turned motion off in purlis, whatever the system says.
    pub reduce_motion: bool,
    /// The contrast the person asks purlis to hold, at least.
    pub contrast: Least,
    /// The colour the person gives each token they set, over every theme.
    pub colours: BTreeMap<&'static str, String>,
}

/// **The person's layer**: what the window draws over the theme in force, whatever it is.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct Layer {
    /// Which of the theme's two geometries is drawn.
    pub density: Density,
    /// The CSS family list for each slot the person named, written by the engine: the slot's
    /// custom property for `ui` and `mono` (`--font-ui`, `--font-mono`), `terminal` for xterm.
    pub fonts: BTreeMap<&'static str, String>,
    /// Whether every motion is off, whatever the system says.
    pub reduce_motion: bool,
    /// The contrast held, at least: more raises the floors ([`super::least`]).
    pub contrast: Least,
    /// The person's colour for each token they set, drawn over the theme by [`Self::over`].
    pub colours: BTreeMap<&'static str, String>,
}

impl Layer {
    /// **What this layer draws over `theme`**: the geometry of the person's density. The
    /// window draws exactly this: it writes the theme's geometry of [`Self::density`].
    pub fn drawn_geometry<'a>(&self, theme: &'a super::ResolvedTheme) -> &'a super::Geometry {
        match self.density {
            Density::Comfortable => &theme.geometry,
            Density::Compact => &theme.compact,
        }
    }

    /// **The family `slot` is drawn in over `theme`**: the person's, when they named one, else
    /// the theme's. The window draws exactly this: it writes the theme's fonts, then the
    /// person's over them.
    pub fn drawn_font(&self, slot: &str, theme: &super::ResolvedTheme) -> String {
        if let Some(mine) = self.fonts.get(slot) {
            return mine.clone();
        }
        match slot {
            "ui" => theme.fonts.ui.stack.clone(),
            "mono" => theme.fonts.mono.stack.clone(),
            _ => theme.fonts.terminal.stack.clone(),
        }
    }
}

impl Layer {
    /// Whether this layer changes the colours a theme draws: a colour of the person's, or more
    /// contrast. A layer that does not draws every theme as it is.
    pub fn colours_anything(&self) -> bool {
        !self.colours.is_empty() || self.contrast == Least::More
    }

    /// **`theme` with the person's colours drawn over it, and its contrast raised**, and what was
    /// left out. Each colour is drawn unless it puts a pair of the check (the theme's [`PAIRS`],
    /// and each ANSI colour on the terminal's background) under its floor and lower than the
    /// theme has it: such a colour is left out, with a sentence, and the theme's is drawn. Then,
    /// with more contrast, every pair is raised to its raised floor ([`least::raise`]). The
    /// consent floors are held after this, by [`super::Drawn::under`].
    pub fn over(&self, theme: &ResolvedTheme) -> (ResolvedTheme, Vec<Complaint>) {
        let mut drawn = theme.clone();
        let mut said = Vec::new();
        for (token, colour) in &self.colours {
            drawn.values.insert(token, colour.clone());
        }
        let measured = measured_pairs();
        // A colour left out can bring another pair back over its floor, so each round measures
        // again; each round leaves one colour out at least, so it ends.
        loop {
            let under = measured.iter().find_map(|pair| {
                pair.tokens()
                    .any(|token| drawn.value(token) != theme.value(token))
                    .then_some(())?;
                let ratio = check::ratio(&drawn, pair)?;
                let was = check::ratio(theme, pair);
                (ratio < pair.floor && was.is_none_or(|was| ratio < was)).then_some((pair, ratio))
            });
            let Some((pair, ratio)) = under else {
                break;
            };
            // The person's colours in the pair: the one left out is one whose leaving out alone
            // puts the pair right, when there is one, else the first.
            let mine: Vec<&'static str> = pair
                .tokens()
                .filter(|token| drawn.value(token) != theme.value(token))
                .collect();
            let puts_right = |token: &'static str| {
                let mut without = drawn.clone();
                without.values.insert(token, theme.value(token).to_owned());
                check::ratio(&without, pair).is_none_or(|now| now >= pair.floor)
            };
            let out = mine
                .iter()
                .copied()
                .find(|token| puts_right(token))
                .unwrap_or(mine[0]);
            let with: String = mine
                .iter()
                .filter(|token| **token != out)
                .map(|token| format!(" with your colour for {token}, {},", drawn.value(token)))
                .collect();
            said.push(Complaint::about(
                out,
                format!(
                    "your colour for {out}, {}, is left out in {}: {} on {}{with} would \
                     contrast {ratio}:1, under its floor of {}:1, so the theme's is drawn",
                    drawn.value(out),
                    theme.name,
                    pair.front,
                    pair.back,
                    pair.floor
                ),
            ));
            let theirs = theme.value(out).to_owned();
            drawn.values.insert(out, theirs);
        }
        // The scrim is held as a measure of its own, not a pair (#2987). A window's ground or
        // words of the person's that would thin the theme's scrim break a pair of words above.
        let dim = check::scrim_dim(&drawn);
        if drawn.value(SCRIM) != theme.value(SCRIM)
            && dim < check::SCRIM_DIM
            && dim < check::scrim_dim(theme)
        {
            said.push(Complaint::about(
                SCRIM,
                format!(
                    "your colour for {SCRIM}, {}, is left out in {}: it would move the window \
                     only {dim}:1, under its floor of {}:1, so the theme's is drawn",
                    drawn.value(SCRIM),
                    theme.name,
                    check::SCRIM_DIM
                ),
            ));
            drawn.values.insert(SCRIM, theme.value(SCRIM).to_owned());
        }
        if self.contrast == Least::More {
            drawn = least::raise(&drawn);
        }
        (drawn, said)
    }
}

/// Every pair a person's colour is held to: the check's [`PAIRS`], and each ANSI colour on the
/// terminal's background at its own floor.
fn measured_pairs() -> Vec<Pair> {
    PAIRS.iter().copied().chain(check::ansi_pairs()).collect()
}

impl Person {
    /// **Reads the layout file's `appearance`**: what is not there is the default, said nothing
    /// of; what is there and not a value of its type is the default, and said.
    pub fn read(raw: Option<&Value>) -> (Self, Vec<Complaint>) {
        let mut said = Vec::new();
        let mut person = Self::default();
        let Some(raw) = raw else {
            return (person, said);
        };
        let Some(given) = raw.as_object() else {
            said.push(Complaint::said(format!(
                "appearance is {}, which is not an object of density, fonts, motion, contrast and colours",
                stringify(raw)
            )));
            return (person, said);
        };
        for key in given.keys() {
            if !KEYS.contains(&key.as_str()) {
                said.push(Complaint::said(format!(
                    "{key} is not a setting of appearance, so it is left out"
                )));
            }
        }
        match given.get("density") {
            None => {}
            Some(Value::String(it)) if it == "comfortable" => {}
            Some(Value::String(it)) if it == "compact" => person.density = Density::Compact,
            Some(other) => said.push(Complaint::said(format!(
                "density is {}, which is \"comfortable\" or \"compact\", so it is comfortable",
                stringify(other)
            ))),
        }
        match given.get("fonts") {
            None => {}
            Some(Value::Object(fonts)) => {
                for (slot, family) in fonts {
                    let Some(slot) = SLOTS.iter().copied().find(|it| it == slot) else {
                        said.push(Complaint::said(format!(
                            "{slot} is not a font you can set: ui, mono or terminal"
                        )));
                        continue;
                    };
                    match family.as_str().map(str::trim) {
                        // An empty field is the theme's font.
                        Some("") => {}
                        Some(name) if is_family(name) => {
                            person.fonts.insert(slot, name.to_owned());
                        }
                        _ => said.push(Complaint::said(format!(
                            "the {slot} font {} {FAMILY_RULE}, so it is the theme's",
                            stringify(family)
                        ))),
                    }
                }
            }
            Some(other) => said.push(Complaint::said(format!(
                "fonts is {}, which is not an object of ui, mono and terminal",
                stringify(other)
            ))),
        }
        match given.get("motion") {
            None => {}
            Some(Value::String(it)) if it == "system" => {}
            Some(Value::String(it)) if it == "reduced" => person.reduce_motion = true,
            Some(other) => said.push(Complaint::said(format!(
                "motion is {}, which is \"system\" or \"reduced\", so it follows the system",
                stringify(other)
            ))),
        }
        match given.get("contrast") {
            None => {}
            Some(Value::String(it)) if it == "standard" => {}
            Some(Value::String(it)) if it == "more" => person.contrast = Least::More,
            Some(other) => said.push(Complaint::said(format!(
                "contrast is {}, which is \"standard\" or \"more\", so it is standard",
                stringify(other)
            ))),
        }
        match given.get("colours") {
            None => {}
            Some(Value::Object(colours)) => {
                for (token, colour) in colours {
                    let Some(token) = TOKENS.iter().copied().find(|it| it == token) else {
                        said.push(Complaint::said(format!(
                            "{token} is not a colour token of the theme vocabulary, so it is left out"
                        )));
                        continue;
                    };
                    match colour.as_str().map(str::trim) {
                        // An empty field is the theme's colour.
                        Some("") => {}
                        Some(value) if is_colour(value) && !is_platform(token, value) => {
                            person.colours.insert(token, value.to_lowercase());
                        }
                        _ => said.push(Complaint::about(
                            token,
                            format!(
                                "your colour for {token}, {}, is not #rgb, #rgba, #rrggbb or \
                                 #rrggbbaa, so the theme's is drawn",
                                stringify(colour)
                            ),
                        )),
                    }
                }
            }
            Some(other) => said.push(Complaint::said(format!(
                "colours is {}, which is not an object of tokens and colours",
                stringify(other)
            ))),
        }
        (person, said)
    }

    /// **The layer this person draws over every theme.**
    pub fn layer(&self) -> Layer {
        Layer {
            density: self.density,
            fonts: self
                .fonts
                .iter()
                .map(|(slot, family)| (*slot, format!("\"{family}\", {}", core_stack(slot))))
                .collect(),
            reduce_motion: self.reduce_motion,
            contrast: self.contrast,
            colours: self.colours.clone(),
        }
    }
}

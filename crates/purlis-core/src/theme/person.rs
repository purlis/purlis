// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What the person sets that every theme honours** (spec #2313, #2326, ADR 0097 §6): density
//! and the font-family override. Text size and reduced motion are the window's own settings
//! already (`textSize.ts`, `motion.ts`).
//!
//! They are kept on this machine, in the layout file beside the text sizes, under `appearance`:
//!
//! ```json
//! "appearance": { "density": "compact", "fonts": { "terminal": "Iosevka Term" } }
//! ```
//!
//! **The person's layer is drawn over the theme's** ([`Layer`]): a theme gives a comfortable and
//! a compact geometry, and the person's density says which is drawn; a font the person names is
//! drawn in place of the theme's, in every theme. A family the person names is held to the same
//! closed grammar as a theme's label ([`super::fonts::is_family`]) and written by the engine,
//! quoted, in front of core's family for the slot, so an installed face that is not there falls
//! back to core's.

use std::collections::BTreeMap;

use serde_json::Value;

use super::fonts::{FAMILY_RULE, SLOTS, core_stack, is_family};
use super::{Complaint, stringify};

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
}

/// **The person's layer**: what the window draws over the theme in force, whatever it is.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct Layer {
    /// Which of the theme's two geometries is drawn.
    pub density: Density,
    /// The CSS family list for each slot the person named, written by the engine: the slot's
    /// custom property for `ui` and `mono` (`--font-ui`, `--font-mono`), `terminal` for xterm.
    pub fonts: BTreeMap<&'static str, String>,
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
                "appearance is {}, which is not an object of density and fonts",
                stringify(raw)
            )));
            return (person, said);
        };
        for key in given.keys() {
            if key != "density" && key != "fonts" {
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
        }
    }
}

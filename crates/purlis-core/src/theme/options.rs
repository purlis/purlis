// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Theme options** (spec #2313, #2325): toggles and choices a theme declares, each value a layer
//! of token values drawn over every appearance.
//!
//! ```json
//! "options": [
//!   { "id": "borders", "kind": "toggle", "label": "Borders", "default": false,
//!     "description": "Draws a rule between panes.",
//!     "on": { "tokens": { "border.subtle": "rule" } } },
//!   { "id": "accent", "kind": "choice", "label": "Accent", "default": "lavender",
//!     "values": [
//!       { "id": "lavender", "label": "Lavender", "tokens": { "accent.base": "#b4a7f5" } },
//!       { "id": "teal", "label": "Teal", "tokens": { "accent.base": "#2bb3a3" },
//!         "appearances": { "day": { "tokens": { "accent.base": "#0f6e64" } } } }
//!     ] }
//! ]
//! ```
//!
//! - **A toggle** is on or off: its `on` layer is drawn when it is on, nothing when it is off.
//! - **A choice** is one of its named values, at least two, each a layer.
//! - **A layer** is token values for every appearance (`tokens`), and optionally token values for
//!   one appearance by its id (`appearances`), drawn over the first. A token's value is hex or a
//!   name in the theme's palette, the grammar an appearance's tokens have.
//! - **There are no free-number options**: an option is a toggle or a choice, and a value is a
//!   layer of tokens, never a number the person types.
//! - **The theme check covers every combination** of appearance and option values, so a theme
//!   holds at most [`MOST_COMBINATIONS`] of them and one past that is refused with its count.
//!
//! **Option values are an input** ([`Values`]): which value of each option is drawn, by the
//! option's id. A toggle's values are `on` and `off`. An option a person did not choose draws its
//! default, and a value the theme does not have draws the default and is said. Where the
//! values come from (the person's choice or a committed default, at each level) is the levels'
//! to decide, by the same precedence as the appearance.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value};

use super::format::{Tokens, is_name, tokens};
use super::{Complaint, ResolvedTheme, stringify};

/// The most appearance × option combinations a theme may have: what the theme check covers.
pub const MOST_COMBINATIONS: usize = 64;

/// The most options a theme may declare. Two values each, six already make 64 combinations.
pub(super) const MOST_OPTIONS: usize = 6;

/// The longest label, in characters.
pub(super) const MOST_LABEL: usize = 64;

/// The longest description, in characters.
pub(super) const MOST_DESCRIPTION: usize = 280;

/// A toggle's two values.
pub const ON: &str = "on";
pub const OFF: &str = "off";

/// **Option values**: the value drawn for each option, by the option's id. A toggle's is
/// [`ON`] or [`OFF`]; a choice's is one of its values' ids.
pub type Values = BTreeMap<String, String>;

/// Whether an option is a toggle or a choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Toggle,
    Choice,
}

/// **One theme option**: what a person sees of it, and its values, each a layer.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct ThemeOption {
    /// Its id, which option values name it by.
    pub id: String,
    /// What a person calls it.
    pub label: String,
    /// One line on what it changes; empty when the theme says none.
    pub description: String,
    pub kind: Kind,
    /// The value drawn when nothing chose one.
    pub default: String,
    /// Its values, in the theme's order: a toggle's are `off` and `on`.
    pub values: Vec<OptionValue>,
}

impl ThemeOption {
    /// The value `id`, when the option has it.
    fn value(&self, id: &str) -> Option<&OptionValue> {
        self.values.iter().find(|one| one.id == id)
    }
}

/// One value of an option.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct OptionValue {
    pub id: String,
    /// What a person calls it.
    pub label: String,
    /// What it draws.
    #[serde(skip)]
    layer: Layer,
}

/// Token values, for every appearance and for single ones: whatever an appearance's `tokens`
/// may hold, read by the same reader ([`tokens`]).
#[derive(Debug, Clone, Default, PartialEq)]
struct Layer {
    tokens: Tokens,
    appearances: BTreeMap<String, Tokens>,
}

impl Layer {
    /// The layer over `theme`, one of its appearances: its tokens for every appearance, then
    /// its tokens for this one.
    fn onto(&self, theme: &mut ResolvedTheme) {
        self.tokens.onto(theme);
        if let Some(own) = self.appearances.get(&theme.id) {
            own.onto(theme);
        }
    }
}

/// **What one option was drawn with**: the option, its value, and whether that value was chosen
/// (`false`: nothing chose one, so it is the theme's default).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct OptionDrawn {
    pub option: ThemeOption,
    pub value: String,
    pub chosen: bool,
}

/// The value of each of `options` that `values` draws, and whether it was chosen, saying in
/// `complaints` each value or option `theme` does not have.
pub(super) fn pick(
    theme: &str,
    options: &[ThemeOption],
    values: &Values,
    complaints: &mut Vec<Complaint>,
) -> Vec<(String, bool)> {
    let picked = options
        .iter()
        .map(|option| match values.get(&option.id) {
            None => (option.default.clone(), false),
            Some(given) if option.value(given).is_some() => (given.clone(), true),
            Some(given) => {
                complaints.push(Complaint::said(format!(
                    "the option {} of {theme} has no value {}, so its default, {}, is drawn",
                    option.id,
                    stringify(&Value::String(given.clone())),
                    option.default
                )));
                (option.default.clone(), false)
            }
        })
        .collect();
    for id in values.keys() {
        if !options.iter().any(|option| &option.id == id) {
            complaints.push(Complaint::said(format!(
                "{theme} has no option {}, so it is left out",
                stringify(&Value::String(id.clone()))
            )));
        }
    }
    picked
}

/// `base`, one appearance as the theme declares it, drawn with `picked` ([`pick`]): each option's
/// value layered over it in the theme's order.
pub(super) fn draw(
    base: &ResolvedTheme,
    options: &[ThemeOption],
    picked: &[(String, bool)],
) -> ResolvedTheme {
    let mut drawn = base.clone();
    drawn.options = Vec::with_capacity(options.len());
    for (option, (value, chosen)) in options.iter().zip(picked) {
        if let Some(it) = option.value(value) {
            it.layer.onto(&mut drawn);
        }
        drawn.options.push(OptionDrawn {
            option: option.clone(),
            value: value.clone(),
            chosen: *chosen,
        });
    }
    drawn
}

/// How many combinations of appearance and option values `appearances` and `options` make,
/// saturating.
pub(super) fn combinations(appearances: usize, options: &[ThemeOption]) -> usize {
    options.iter().fold(appearances, |made, option| {
        made.saturating_mul(option.values.len())
    })
}

/// The sentence a theme past [`MOST_COMBINATIONS`] is refused with.
pub(super) fn too_many(called: &str, appearances: usize, options: &[ThemeOption]) -> String {
    let made = combinations(appearances, options);
    let mut factors = vec![format!(
        "{appearances} {}",
        if appearances == 1 {
            "appearance"
        } else {
            "appearances"
        }
    )];
    factors.extend(options.iter().map(|one| one.values.len().to_string()));
    format!(
        "{called} has {made} combinations of appearance and option values ({}), and the theme \
         check covers at most {MOST_COMBINATIONS}",
        factors.join(" x ")
    )
}

/// Every combination of option values, the first option's slowest, each value in its option's
/// order.
pub(super) fn every(options: &[ThemeOption]) -> Vec<Values> {
    let mut all = vec![Values::new()];
    for option in options {
        all = all
            .into_iter()
            .flat_map(|made| {
                option.values.iter().map(move |value| {
                    let mut next = made.clone();
                    next.insert(option.id.clone(), value.id.clone());
                    next
                })
            })
            .collect();
    }
    all
}

/// `child`'s options over `parent`'s: one with a parent's id takes its place, a new one is added
/// after them.
pub(super) fn over(parent: &[ThemeOption], child: Vec<ThemeOption>) -> Vec<ThemeOption> {
    let mut merged = parent.to_vec();
    for one in child {
        match merged.iter().position(|it| it.id == one.id) {
            Some(at) => merged[at] = one,
            None => merged.push(one),
        }
    }
    merged
}

/// The appearance ids `options`' layers name that are not in `ids`, said in `complaints`.
pub(super) fn hold_appearances(
    options: &[ThemeOption],
    ids: &BTreeSet<&str>,
    complaints: &mut Vec<Complaint>,
) {
    for option in options {
        for value in &option.values {
            for named in value.layer.appearances.keys() {
                if !ids.contains(named.as_str()) {
                    complaints.push(Complaint::said(format!(
                        "option {} value {} names appearance {named}, which the theme does not \
                         have",
                        option.id, value.id
                    )));
                }
            }
        }
    }
}

/// **A theme's `options`**, read, saying in `complaints` what is wrong with any of it (which
/// refuses the theme) and in `notes` a token name purlis does not have (left out).
pub(super) fn read(
    raw: Option<&Value>,
    palette: &BTreeMap<String, String>,
    complaints: &mut Vec<Complaint>,
    notes: &mut Vec<Complaint>,
) -> Vec<ThemeOption> {
    let Some(raw) = raw else {
        return Vec::new();
    };
    let Some(given) = raw.as_array() else {
        complaints.push(Complaint::said("options is a list of options"));
        return Vec::new();
    };
    if given.len() > MOST_OPTIONS {
        complaints.push(Complaint::said(format!(
            "options lists {} options, and a theme has at most {MOST_OPTIONS}",
            given.len()
        )));
        return Vec::new();
    }
    let mut out: Vec<ThemeOption> = Vec::new();
    for (at, raw) in given.iter().enumerate() {
        let Some(one) = raw.as_object() else {
            complaints.push(Complaint::said(format!(
                "option {} is not an object",
                at + 1
            )));
            continue;
        };
        let before = complaints.len();
        let option = read_one(one, at, palette, complaints, notes);
        if let Some(option) = option.filter(|_| complaints.len() == before) {
            if out.iter().any(|it| it.id == option.id) {
                complaints.push(Complaint::said(format!(
                    "two options are called {}",
                    option.id
                )));
            } else {
                out.push(option);
            }
        }
    }
    out
}

/// The keys of an option.
const OPTION_KEYS: &[&str] = &[
    "id",
    "kind",
    "label",
    "description",
    "default",
    "on",
    "values",
];

/// The keys of a choice's value.
const VALUE_KEYS: &[&str] = &["id", "label", "tokens", "appearances"];

/// The keys of a layer: a toggle's `on`, and one appearance's in `appearances`.
const LAYER_KEYS: &[&str] = &["tokens", "appearances"];

fn read_one(
    one: &Map<String, Value>,
    at: usize,
    palette: &BTreeMap<String, String>,
    complaints: &mut Vec<Complaint>,
    notes: &mut Vec<Complaint>,
) -> Option<ThemeOption> {
    let id = match one.get("id") {
        Some(Value::String(id)) if is_name(id) => id.clone(),
        None => {
            complaints.push(Complaint::said(format!("option {} has no id", at + 1)));
            return None;
        }
        Some(other) => {
            complaints.push(Complaint::said(format!(
                "option id {} is not lower-case letters, digits and dashes starting with a \
                 letter, at most 32 long",
                stringify(other)
            )));
            return None;
        }
    };
    // An option of a kind purlis does not have is said as that, and nothing more: its other
    // keys are that kind's.
    let kind = one.get("kind").and_then(Value::as_str);
    if !matches!(kind, Some("toggle" | "choice")) {
        complaints.push(Complaint::said(format!(
            "option {id} is of kind {}; a theme option is a toggle or a choice, and there are no \
             free-number options",
            kind.map_or_else(
                || "nothing".to_owned(),
                |kind| stringify(&Value::String(kind.to_owned()))
            )
        )));
        return None;
    }
    for key in one.keys() {
        if !OPTION_KEYS.contains(&key.as_str()) {
            complaints.push(Complaint::said(format!(
                "{key} is not a key of option {id}"
            )));
        }
    }
    let label = text(
        one.get("label"),
        &format!("the label of option {id}"),
        MOST_LABEL,
        true,
        complaints,
    );
    let description = text(
        one.get("description"),
        &format!("the description of option {id}"),
        MOST_DESCRIPTION,
        false,
        complaints,
    );
    let at_option = format!("option {id}");
    let (kind, default, values) = match kind {
        Some("toggle") => {
            if one.contains_key("values") {
                complaints.push(Complaint::said(format!(
                    "option {id} is a toggle, which has an on layer and no values"
                )));
            }
            let default = match one.get("default") {
                Some(Value::Bool(on)) => if *on { ON } else { OFF }.to_owned(),
                given => {
                    complaints.push(Complaint::said(format!(
                        "the default of option {id} is {}; a toggle's default is true or false",
                        given.map_or_else(|| "missing".to_owned(), stringify)
                    )));
                    OFF.to_owned()
                }
            };
            let on = match one.get("on") {
                Some(Value::Object(on)) => layer(
                    on,
                    true,
                    &format!("the on layer of {at_option}"),
                    palette,
                    complaints,
                    notes,
                ),
                Some(_) => {
                    complaints.push(Complaint::said(format!(
                        "on in option {id} is a layer of token values"
                    )));
                    Layer::default()
                }
                None => {
                    complaints.push(Complaint::said(format!(
                        "option {id} is a toggle with no on layer"
                    )));
                    Layer::default()
                }
            };
            let values = vec![
                OptionValue {
                    id: OFF.to_owned(),
                    label: "Off".to_owned(),
                    layer: Layer::default(),
                },
                OptionValue {
                    id: ON.to_owned(),
                    label: "On".to_owned(),
                    layer: on,
                },
            ];
            (Kind::Toggle, default, values)
        }
        Some("choice") => {
            if one.contains_key("on") {
                complaints.push(Complaint::said(format!(
                    "option {id} is a choice, which has values and no on layer"
                )));
            }
            let values = choice_values(one.get("values"), &id, palette, complaints, notes);
            let default = match one.get("default") {
                Some(Value::String(default)) if values.iter().any(|it| &it.id == default) => {
                    default.clone()
                }
                given => {
                    complaints.push(Complaint::said(format!(
                        "the default of option {id} is {}, which is not one of its values",
                        given.map_or_else(|| "missing".to_owned(), stringify)
                    )));
                    String::new()
                }
            };
            (Kind::Choice, default, values)
        }
        // Held above: an option is a toggle or a choice.
        _ => return None,
    };
    Some(ThemeOption {
        id,
        label,
        description,
        kind,
        default,
        values,
    })
}

/// A choice's values: at least two, each with its own id.
fn choice_values(
    raw: Option<&Value>,
    id: &str,
    palette: &BTreeMap<String, String>,
    complaints: &mut Vec<Complaint>,
    notes: &mut Vec<Complaint>,
) -> Vec<OptionValue> {
    let Some(given) = raw.and_then(Value::as_array) else {
        complaints.push(Complaint::said(format!(
            "option {id} is a choice, whose values are a list"
        )));
        return Vec::new();
    };
    if given.len() < 2 || given.len() > MOST_COMBINATIONS {
        complaints.push(Complaint::said(format!(
            "option {id} has {} values; a choice has from 2 to {MOST_COMBINATIONS}",
            given.len()
        )));
        return Vec::new();
    }
    let mut out: Vec<OptionValue> = Vec::new();
    for (at, raw) in given.iter().enumerate() {
        let Some(one) = raw.as_object() else {
            complaints.push(Complaint::said(format!(
                "value {} of option {id} is not an object",
                at + 1
            )));
            continue;
        };
        let value = match one.get("id") {
            Some(Value::String(value)) if is_name(value) => value.clone(),
            given => {
                complaints.push(Complaint::said(format!(
                    "value {} of option {id} has the id {}, which is not lower-case letters, \
                     digits and dashes starting with a letter, at most 32 long",
                    at + 1,
                    given.map_or_else(|| "missing".to_owned(), stringify)
                )));
                continue;
            }
        };
        if out.iter().any(|it| it.id == value) {
            complaints.push(Complaint::said(format!(
                "two values of option {id} are called {value}"
            )));
            continue;
        }
        for key in one.keys() {
            if !VALUE_KEYS.contains(&key.as_str()) {
                complaints.push(Complaint::said(format!(
                    "{key} is not a key of value {value} of option {id}"
                )));
            }
        }
        let label = text(
            one.get("label"),
            &format!("the label of value {value} of option {id}"),
            MOST_LABEL,
            true,
            complaints,
        );
        let layer = layer(
            one,
            false,
            &format!("value {value} of option {id}"),
            palette,
            complaints,
            notes,
        );
        out.push(OptionValue {
            id: value,
            label,
            layer,
        });
    }
    out
}

/// A label or a description: one line of text a person reads, at most `most` characters, with no
/// control character; `required` when it may not be missing or empty.
fn text(
    raw: Option<&Value>,
    what: &str,
    most: usize,
    required: bool,
    complaints: &mut Vec<Complaint>,
) -> String {
    match raw {
        None if !required => String::new(),
        Some(Value::String(given))
            if given.chars().count() <= most
                && !given.chars().any(char::is_control)
                && !(required && given.trim().is_empty()) =>
        {
            given.trim().to_owned()
        }
        given => {
            complaints.push(Complaint::said(format!(
                "{what} is {}; it is one line of text, {}at most {most} characters",
                given.map_or_else(|| "missing".to_owned(), stringify),
                if required { "not empty, " } else { "" }
            )));
            String::new()
        }
    }
}

/// A layer from `from`'s `tokens` and `appearances`. `alone` when `from` is only a layer (a
/// toggle's `on`), whose other keys are said here; a choice's value says its own.
fn layer(
    from: &Map<String, Value>,
    alone: bool,
    at: &str,
    palette: &BTreeMap<String, String>,
    complaints: &mut Vec<Complaint>,
    notes: &mut Vec<Complaint>,
) -> Layer {
    let every = tokens(from.get("tokens"), at, palette, complaints, notes);
    let mut appearances = BTreeMap::new();
    match from.get("appearances") {
        None => {}
        Some(Value::Object(given)) => {
            for (id, one) in given {
                let here = format!("{at} for appearance {id}");
                if !is_name(id) {
                    complaints.push(Complaint::said(format!(
                        "{at} names the appearance {}, which is not an appearance id",
                        stringify(&Value::String(id.clone()))
                    )));
                    continue;
                }
                let Some(one) = one.as_object() else {
                    complaints.push(Complaint::said(format!(
                        "{here} is a layer of token values"
                    )));
                    continue;
                };
                for key in one.keys() {
                    if key != "tokens" {
                        complaints.push(Complaint::said(format!("{key} is not a key of {here}")));
                    }
                }
                appearances.insert(
                    id.clone(),
                    tokens(one.get("tokens"), &here, palette, complaints, notes),
                );
            }
        }
        Some(_) => complaints.push(Complaint::said(format!(
            "appearances in {at} is an object of appearance ids to layers"
        ))),
    }
    // A toggle's `on` holds only a layer's keys; a choice's value holds its own as well, which
    // its reader says.
    if alone {
        for key in from.keys() {
            if !LAYER_KEYS.contains(&key.as_str()) {
                complaints.push(Complaint::said(format!("{key} is not a key of {at}")));
            }
        }
    }
    Layer {
        tokens: every,
        appearances,
    }
}

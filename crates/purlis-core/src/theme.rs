// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The theme engine** (spec #2313, #2320): the one place that reads a theme, validates it,
//! resolves which theme the window draws, and checks whether its words can be read.
//!
//! Every surface asks this module, and none of them holds a rule of its own. The window is
//! handed a [`ResolvedTheme`] — every token's final value and the motion — and only applies it:
//! it writes the custom properties and hands xterm its object (`app/src/theme/theme.ts`).
//!
//! # The interface
//!
//! - [`load`] reads one theme document (today's format, format 1: a name, an appearance, colour
//!   tokens and motion) and always answers with a complete theme and what it had to put right
//!   ([`Resolved`]). **It never fails**: a token that is missing, misspelled or malformed falls
//!   back to the built-in of the same appearance, and the substitution is said. ADR 0026 holds
//!   cold start at 2 s, and the window has to come up whatever a file says.
//! - [`built_ins`] and [`built_in`] are the themes purlis ships, compiled into the core
//!   (`theme/built-in/*.json`), by their ids and the ids they had before (#1838).
//! - [`resolve`] decides which theme the window draws from every source there is today: the
//!   project's or workspace's pick, the person's own theme file, the themes approved extensions
//!   contribute, and the operating system's appearance ([`Inputs`]).
//! - [`check`] measures any theme against the contrast pairs and the terminal's floors, as it
//!   ships and tinted by every workspace colour, and answers with a [`Report`].
//!
//! The spec's later formats (appearances, options, levels, import and export) grow these same
//! four entries: `resolve` takes more inputs, `check` covers more combinations, and a resolved
//! theme carries more than colour and motion.
//!
//! # Values are closed grammars
//!
//! A colour is hex and only hex ([`vocabulary::is_colour`]); a duration is a whole number of
//! milliseconds; an easing is four bounded numbers. What reaches the window is written from those
//! parsed values, never copied from a file's text (ADR 0041, property 4), so a theme from an
//! extension or a hand-edited file has nowhere to put anything but a colour or a number.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde_json::Value;

mod check;
mod resolve;
mod tint;
pub mod vocabulary;

pub use check::{Finding, PAIRS, Pair, Report};
pub use resolve::{Drawn, Inputs, Offered, On, resolve};
pub use tint::{PALETTE, hue_of, tint_hex, tinted};
use vocabulary::{DURATIONS, EASINGS, LONGEST, TOKENS, is_colour, is_token, names_geometry};

#[cfg(test)]
mod tests;

/// Whether a theme is drawn on a dark window or a light one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Appearance {
    Dark,
    Light,
}

/// A theme's motion, complete: every duration and every easing has a value.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Motion {
    /// Each of [`DURATIONS`], in milliseconds.
    pub durations: BTreeMap<&'static str, u32>,
    /// Each of [`EASINGS`], as `[x1, y1, x2, y2]`.
    pub easings: BTreeMap<&'static str, [f64; 4]>,
}

/// **A theme, complete**: what the window draws. Every token in [`TOKENS`] has a value, and so
/// does every motion token.
///
/// **This shape is today's (format 1) and will change for format 2** (spec #2313): typed values
/// beyond colour and motion (geometry, fonts, icons), keys for each appearance × option
/// combination, and schema errors beside the findings. Its users are the window's wire types and
/// the engine's own tests, which change with it.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct ResolvedTheme {
    /// What to call it, as a person would.
    pub name: String,
    /// Which built-in a partial theme falls back to, and the `color-scheme` the window gets.
    pub appearance: Appearance,
    /// Each token's final value.
    pub values: BTreeMap<&'static str, String>,
    /// How the window moves.
    pub motion: Motion,
}

impl ResolvedTheme {
    /// The value of `token`, which every resolved theme has for every token of [`TOKENS`].
    pub fn value(&self, token: &str) -> &str {
        self.values.get(token).map_or("", String::as_str)
    }
}

/// One thing [`load`] had to put right: a value the file did not supply usably.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Complaint {
    /// What was wrong, in the words to put in front of whoever wrote the file.
    pub said: String,
    /// The token it was about, when it was about one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
}

impl Complaint {
    fn said(said: impl Into<String>) -> Self {
        Self {
            said: said.into(),
            token: None,
        }
    }

    fn about(token: &str, said: impl Into<String>) -> Self {
        Self {
            said: said.into(),
            token: Some(token.to_owned()),
        }
    }
}

/// A theme, and what it cost to get one.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Resolved {
    pub theme: ResolvedTheme,
    pub complaints: Vec<Complaint>,
}

/// The built-in theme files, compiled in: the one copy of each.
const BUILT_IN_FILES: [&str; 2] = [
    include_str!("theme/built-in/purlis-dark.json"),
    include_str!("theme/built-in/purlis-light.json"),
];

/// **The themes purlis ships**, dark first: the theme the window comes up in. Their ids are
/// [`crate::extension::BUILT_IN_THEMES`], which a test holds to these.
pub fn built_ins() -> &'static [ResolvedTheme] {
    static BUILT: OnceLock<Vec<ResolvedTheme>> = OnceLock::new();
    BUILT.get_or_init(|| {
        BUILT_IN_FILES
            .iter()
            .map(|text| built_in_of(text))
            .collect()
    })
}

/// A built-in file as a theme. A built-in is checked into this crate and a test proves each one
/// complete, so a gap here is a defect of this build and not something a person did.
fn built_in_of(text: &str) -> ResolvedTheme {
    let document: Value = serde_json::from_str(text).expect("a built-in theme is JSON");
    let appearance = match document.get("appearance").and_then(Value::as_str) {
        Some("light") => Appearance::Light,
        _ => Appearance::Dark,
    };
    let name = document
        .get("name")
        .and_then(Value::as_str)
        .expect("a built-in theme has a name")
        .to_owned();
    let tokens = document.get("tokens").and_then(Value::as_object);
    let values = TOKENS
        .iter()
        .map(|token| {
            let value = tokens
                .and_then(|tokens| tokens.get(*token))
                .and_then(Value::as_str)
                .filter(|value| is_colour(value))
                .unwrap_or_else(|| panic!("the built-in theme {name} has no {token}"));
            (*token, value.to_owned())
        })
        .collect();
    let motion = motion_of(document.get("motion"))
        .unwrap_or_else(|missing| panic!("the built-in theme {name} has no {missing}"));
    ResolvedTheme {
        name,
        appearance,
        values,
        motion,
    }
}

/// A built-in's motion, every token present, or the first token it lacks.
fn motion_of(raw: Option<&Value>) -> Result<Motion, &'static str> {
    let given = raw.and_then(Value::as_object);
    let field = |token: &str| given.and_then(|given| given.get(token));
    let mut durations = BTreeMap::new();
    for token in DURATIONS {
        durations.insert(*token, field(token).and_then(duration).ok_or(*token)?);
    }
    let mut easings = BTreeMap::new();
    for token in EASINGS {
        easings.insert(*token, field(token).and_then(easing).ok_or(*token)?);
    }
    Ok(Motion { durations, easings })
}

/// The theme purlis draws when nothing has said otherwise: the dark built-in.
pub fn default_theme() -> &'static ResolvedTheme {
    &built_ins()[0]
}

/// The built-in of `appearance`: what a theme of that appearance falls back to, token by token.
pub fn built_in_for(appearance: Appearance) -> &'static ResolvedTheme {
    built_ins()
        .iter()
        .find(|theme| theme.appearance == appearance)
        .unwrap_or_else(default_theme)
}

/// The built-in `id` names, by its id or by an id it had before the rename (#1838).
pub fn built_in(id: &str) -> Option<&'static ResolvedTheme> {
    let id = [crate::names::THEME_DARK, crate::names::THEME_LIGHT]
        .into_iter()
        .find(|name| name.recognises(id))
        .map_or(id, |name| name.write);
    built_ins().iter().find(|theme| theme.name == id)
}

/// Why a theme's geometry was ignored, after the key that named it.
const GEOMETRY_IS_CORE: &str =
    "is geometry, which is the same in every theme; a theme sets colour, shadow colour and motion";

/// **Reads one theme document** (format 1), and always answers with a complete theme.
///
/// `document` is untrusted: the parse of a file purlis did not write. Nothing here panics and
/// nothing trusts a type. A token that is missing falls back to the built-in of the theme's
/// appearance silently, since not naming a token is how a small theme says the built-in's is
/// right; a value that is there and unusable falls back too, and is said.
pub fn load(document: &Value) -> Resolved {
    let Some(from) = document.as_object() else {
        return Resolved {
            theme: default_theme().clone(),
            complaints: vec![Complaint::said("a theme is a JSON object")],
        };
    };
    let mut complaints = Vec::new();

    let appearance = match from.get("appearance").and_then(Value::as_str) {
        Some("light") => Appearance::Light,
        Some("dark") => Appearance::Dark,
        _ => {
            complaints.push(Complaint::said(
                "appearance is \"dark\" or \"light\"; read this one as dark",
            ));
            Appearance::Dark
        }
    };
    let base = built_in_for(appearance);

    let name = match from.get("name").and_then(Value::as_str).map(str::trim) {
        Some(name) if !name.is_empty() => name.to_owned(),
        _ => {
            complaints.push(Complaint::said(format!(
                "a theme has a name; called this one {}",
                base.name
            )));
            base.name.clone()
        }
    };

    let empty = serde_json::Map::new();
    let tokens = match from.get("tokens") {
        Some(Value::Object(tokens)) => tokens,
        _ => {
            complaints.push(Complaint::said(
                "tokens is an object of token names to colours",
            ));
            &empty
        }
    };
    for given in tokens.keys() {
        if is_token(given) {
            continue;
        }
        if names_geometry(given) {
            complaints.push(Complaint::about(
                given,
                format!("{given} {GEOMETRY_IS_CORE}"),
            ));
        } else {
            complaints.push(Complaint::about(
                given,
                format!("{given} is not a purlis token"),
            ));
        }
    }
    // Geometry is core (ADR 0094): a theme that tries to size the window is told so, by key,
    // rather than being silently ignored. Nothing it says is read.
    for key in from.keys() {
        if names_geometry(key) {
            complaints.push(Complaint::said(format!("{key} {GEOMETRY_IS_CORE}")));
        }
    }

    let mut values = BTreeMap::new();
    for token in TOKENS {
        let fallback = base.value(token);
        match tokens.get(*token) {
            Some(Value::String(given)) if is_colour(given) => {
                values.insert(*token, given.clone());
            }
            given => {
                values.insert(*token, fallback.to_owned());
                if let Some(given) = given {
                    complaints.push(Complaint::about(
                        token,
                        format!(
                            "{token} is {}, which is not #rgb, #rgba, #rrggbb or #rrggbbaa; used \
                             {fallback}",
                            stringify(given)
                        ),
                    ));
                }
            }
        }
    }

    // A theme that says nothing about motion moves the way the built-in of its appearance does,
    // exactly as a token it does not name is coloured by that built-in.
    let motion = load_motion(from.get("motion"), &base.motion, &mut complaints);

    Resolved {
        theme: ResolvedTheme {
            name,
            appearance,
            values,
            motion,
        },
        complaints,
    }
}

/// [`load`] of a theme's text: `None` when the text is not JSON at all, which the caller decides
/// about (an extension's theme that is not JSON is dropped, and the window keeps what it has).
pub fn load_text(text: &str) -> Option<Resolved> {
    serde_json::from_str::<Value>(text)
        .ok()
        .map(|document| load(&document))
}

/// A value as JSON writes it, for a sentence that names what a file said.
fn stringify(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_default()
}

/// A duration a theme may hold: a whole number of milliseconds, `0` to [`LONGEST`]. A string
/// (`"150ms"`) is not one, on purpose: a number has nothing in it to escape.
fn duration(value: &Value) -> Option<u32> {
    let number = value.as_f64()?;
    (number.fract() == 0.0 && (0.0..=f64::from(LONGEST)).contains(&number)).then_some(number as u32)
}

/// An easing a theme may hold: four finite numbers, the two `x` in 0..1 (CSS's own rule; a
/// declaration that breaks it is dropped whole) and the two `y` in -1..2, which leaves room for
/// an overshoot and none for a curve that flings a surface off the window.
fn easing(value: &Value) -> Option<[f64; 4]> {
    let numbers = value.as_array()?;
    if numbers.len() != 4 || !numbers.iter().all(Value::is_number) {
        return None;
    }
    let read: Vec<f64> = numbers.iter().filter_map(Value::as_f64).collect();
    let [x1, y1, x2, y2] = <[f64; 4]>::try_from(read).ok()?;
    let within = |n: f64, low: f64, high: f64| n.is_finite() && n >= low && n <= high;
    (within(x1, 0.0, 1.0) && within(x2, 0.0, 1.0) && within(y1, -1.0, 2.0) && within(y2, -1.0, 2.0))
        .then_some([x1, y1, x2, y2])
}

/// Reads a theme's `motion`, filling what it does not supply usably from `base` and saying so.
fn load_motion(raw: Option<&Value>, base: &Motion, complaints: &mut Vec<Complaint>) -> Motion {
    let Some(raw) = raw else {
        return base.clone();
    };
    let empty = serde_json::Map::new();
    let given = match raw {
        Value::Object(given) => given,
        _ => {
            complaints.push(Complaint::said(
                "motion is an object of motion token names to values",
            ));
            &empty
        }
    };
    for name in given.keys() {
        if !DURATIONS.contains(&name.as_str()) && !EASINGS.contains(&name.as_str()) {
            complaints.push(Complaint::said(format!(
                "{name} is not a purlis motion token"
            )));
        }
    }
    let mut motion = base.clone();
    for token in DURATIONS {
        let Some(value) = given.get(*token) else {
            continue;
        };
        match duration(value) {
            Some(ms) => {
                motion.durations.insert(*token, ms);
            }
            None => complaints.push(Complaint::said(format!(
                "{token} is {}, which is not a whole number of milliseconds from 0 to {LONGEST}; \
                 used {}",
                stringify(value),
                base.durations[token]
            ))),
        }
    }
    for token in EASINGS {
        let Some(value) = given.get(*token) else {
            continue;
        };
        match easing(value) {
            Some(curve) => {
                motion.easings.insert(*token, curve);
            }
            None => {
                let used: Vec<String> = base.easings[token].iter().map(f64::to_string).collect();
                complaints.push(Complaint::said(format!(
                    "{token} is {}, which is not [x1, y1, x2, y2] with each x from 0 to 1 and \
                     each y from -1 to 2; used [{}]",
                    stringify(value),
                    used.join(", ")
                )));
            }
        }
    }
    motion
}

/// **The person's own theme** (`theme.json` beside `machine.json`, M6.7), as the window is
/// handed it when it is created: where the file is, whether there is one, why a file that is
/// there is not in force, and — when it is — the theme it resolves to and what was put right.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct Theirs {
    /// Where the file is, or would be. Empty only when the machine has no config home.
    pub path: String,
    /// Whether there is a file at all.
    pub found: bool,
    /// Why a file that is there is not in force: not readable, not JSON, not an object.
    pub trouble: Option<String>,
    /// The theme it resolves to, when it is in force.
    pub theme: Option<ResolvedTheme>,
    /// What resolving it had to put right.
    pub complaints: Vec<Complaint>,
}

/// Reads the person's own theme under `config_root` and resolves it. A machine with no config
/// home has none. Bounded and link-refusing, as every file beside `machine.json` is read
/// ([`crate::windowprefs::read_theme`]).
pub fn read_theirs(config_root: Option<&std::path::Path>) -> Theirs {
    let Some(root) = config_root else {
        return Theirs::default();
    };
    let reading = crate::windowprefs::read_theme(root);
    let resolved = reading.document.as_ref().map(load);
    Theirs {
        path: reading.path,
        found: reading.found,
        trouble: reading.trouble,
        complaints: resolved
            .as_ref()
            .map(|it| it.complaints.clone())
            .unwrap_or_default(),
        theme: resolved.map(|it| it.theme),
    }
}

/// **The person's own theme, read once per process** and kept, so every window and every later
/// answer resolves from the same read, whichever window was created first or whether one was
/// at all. [`Self::forget`] lets the next ask read the file again: after Use built-in moved it
/// aside, and for live reload (#2324).
#[derive(Debug, Default)]
pub struct TheirsKept {
    kept: std::sync::Mutex<Option<Theirs>>,
}

impl TheirsKept {
    /// Nothing read yet.
    pub const fn new() -> Self {
        Self {
            kept: std::sync::Mutex::new(None),
        }
    }

    /// The person's theme under `config_root`: read and resolved on the first ask, the same
    /// answer on every ask after it until [`Self::forget`].
    pub fn get(&self, config_root: Option<&std::path::Path>) -> Theirs {
        self.kept
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get_or_insert_with(|| read_theirs(config_root))
            .clone()
    }

    /// Lets the next [`Self::get`] read the file again.
    pub fn forget(&self) {
        *self
            .kept
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
    }
}

/// **Checks a theme**: every contrast pair, the terminal's ANSI floors and the selected row kept
/// apart, as the theme is and tinted by every colour a workspace can name. A theme with no
/// findings can be read.
pub fn check(theme: &ResolvedTheme) -> Report {
    check::check(theme)
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The theme engine** (spec #2313, #2320, #2322): the one place that reads a theme, validates
//! it, resolves which theme the window draws, and checks whether its words can be read.
//!
//! Every surface asks this module, and none of them holds a rule of its own. The window is
//! handed a [`ResolvedTheme`] — one appearance, every token's final value and the motion — and
//! only applies it: it writes the custom properties and hands xterm its object
//! (`app/src/theme/theme.ts`).
//!
//! # The interface
//!
//! - [`load`] reads one theme document and always answers with a complete [`Theme`] — every
//!   appearance, each complete — and what it had to say about it ([`Resolved`]).
//!   - **Format 2** (a theme folder's `theme.json`, [`Library`]): a name, optionally a theme it
//!     `extends`, a palette of named colours, and one or more appearances, each tagged dark or
//!     light and optionally high contrast. A format 2 theme that breaks its grammar anywhere is
//!     **refused whole** (ADR 0097 §2): it draws Purlis, and the complaints say why.
//!   - **Format 1** (today's single-appearance file) keeps loading as a theme with one
//!     appearance, with its per-token fallback: a token that is missing, misspelled or malformed
//!     falls back to Purlis Classic's of the same appearance, and the substitution is said.
//!
//!   **It never fails**: ADR 0026 holds cold start at 2 s, and the window has to come up
//!   whatever a file says.
//! - [`classic`] is Purlis Classic, the theme purlis ships, compiled into the core from its theme
//!   folder (`theme/built-in/purlis-classic/`); [`built_ins`] and [`built_in`] are its
//!   appearances by the ids a pick names them by, old ones included (#1838).
//! - [`resolve`] decides which theme, and which of its appearances, the window draws from every
//!   source there is today: the project's or workspace's pick, the person's own theme file, the
//!   themes approved extensions contribute, and the operating system's appearance ([`Inputs`]).
//! - [`check`] measures one appearance against the contrast pairs and the terminal's floors, as
//!   it ships and tinted by every workspace colour, and answers with a [`Report`].
//! - [`schema`] is the published JSON Schema of format 2, generated from this module's own
//!   vocabulary (`docs/schema/theme.schema.json`, held to it by a test).
//!
//! The spec's later steps (theme options, the choice levels, geometry and fonts, import and
//! export) grow these same entries: `resolve` takes more inputs, `check` covers more
//! combinations, and a resolved theme carries more than colour and motion.
//!
//! # Values are closed grammars
//!
//! A colour is hex ([`vocabulary::is_colour`]) or, in format 2, the name of a colour in the
//! theme's own palette, which the engine replaces with that hex; a duration is a whole number of
//! milliseconds; an easing is four bounded numbers. What reaches the window is written from those
//! parsed values, never copied from a file's text (ADR 0041, property 4), so a theme from an
//! extension or a hand-edited file has nowhere to put anything but a colour or a number.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde_json::Value;

mod check;
mod format;
mod resolve;
mod schema;
mod tint;
pub mod vocabulary;

pub use check::{Finding, PAIRS, Pair, Report};
pub use format::{Library, MOST_APPEARANCES, THEME_JSON};
pub use resolve::{Drawn, Inputs, Offered, On, resolve};
pub use schema::{SCHEMA_URL, schema, schema_text};
pub use tint::{PALETTE, hue_of, tint_hex, tinted};
use vocabulary::{DURATIONS, EASINGS, LONGEST, TOKENS, is_colour, is_token, names_geometry};

#[cfg(test)]
mod format_tests;
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

/// **One appearance of a theme, complete**: what the window draws. Every token in [`TOKENS`] has
/// a value, and so does every motion token.
///
/// **It will grow** (spec #2313): typed values beyond colour and motion (geometry #2326, fonts,
/// icons) and the option values it was drawn with (#2325). Its users are the window's wire types
/// and the engine's own tests, which change with it.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct ResolvedTheme {
    /// The name the window knows what it draws by: the theme's name, or for Purlis Classic's
    /// appearances the id a pick names each by (`purlis-dark`, `purlis-light`), which the
    /// window's picks and Settings name until the choice levels replace the old ids (#2323).
    pub name: String,
    /// The appearance's id inside its theme (`dark`, `day`): a format 1 file's is its tag.
    pub id: String,
    /// Its tag: which of Purlis Classic's appearances a partial one falls back to, and the
    /// `color-scheme` the window gets.
    pub appearance: Appearance,
    /// Whether it is tagged high contrast.
    pub high_contrast: bool,
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

/// **A theme**: its name and every appearance it has, each complete, in the order it declares
/// them. Never without an appearance.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Theme {
    /// What to call it, as a person would.
    pub name: String,
    /// Its appearances, the first one what it draws when nothing chose another.
    pub appearances: Vec<ResolvedTheme>,
}

impl Theme {
    /// What the theme draws when nothing chose an appearance: its first.
    pub fn first(&self) -> &ResolvedTheme {
        &self.appearances[0]
    }

    /// Its appearance tagged `appearance`: the first such that is not high contrast, else the
    /// first such at all.
    pub fn of(&self, appearance: Appearance) -> Option<&ResolvedTheme> {
        let tagged = || {
            self.appearances
                .iter()
                .filter(|one| one.appearance == appearance)
        };
        tagged()
            .find(|one| !one.high_contrast)
            .or_else(|| tagged().next())
    }

    /// The theme as one appearance: what a format 1 file is.
    fn one(theme: ResolvedTheme) -> Self {
        Self {
            name: theme.name.clone(),
            appearances: vec![theme],
        }
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
    /// The theme, complete: Purlis when the document was refused.
    pub theme: Theme,
    /// What was put right, or why it was refused.
    pub complaints: Vec<Complaint>,
    /// Whether the document was refused whole, so [`Self::theme`] is Purlis and not the
    /// document's (a format 2 theme that breaks its grammar, ADR 0097 §2).
    pub refused: bool,
}

impl Resolved {
    /// The document refused, for `complaints`: Purlis is drawn in its place.
    fn refusal(complaints: Vec<Complaint>) -> Self {
        Self {
            theme: classic().clone(),
            complaints,
            refused: true,
        }
    }
}

/// Purlis Classic's `theme.json`, compiled in: the one copy of it.
const CLASSIC_FILE: &str = include_str!("theme/built-in/purlis-classic/theme.json");

/// Purlis Classic's id: its folder's name, which a theme `extends` it by.
pub const CLASSIC: &str = "purlis-classic";

/// **Purlis Classic**, the theme purlis ships: today's look, a dark and a light appearance. Its
/// appearances are named by the ids a pick names them by ([`crate::extension::BUILT_IN_THEMES`],
/// which a test holds to these), dark first: the theme the window comes up in.
pub fn classic() -> &'static Theme {
    static BUILT: OnceLock<Theme> = OnceLock::new();
    BUILT.get_or_init(|| built_in_of(CLASSIC_FILE))
}

/// **The appearances purlis ships**, dark first: Purlis Classic's.
pub fn built_ins() -> &'static [ResolvedTheme] {
    &classic().appearances
}

/// The built-in folder as a theme. It is checked into this crate and a test proves it complete,
/// so a gap here is a defect of this build and not something a person did. Every token is
/// given, so nothing falls back: this is what everything else falls back to.
fn built_in_of(text: &str) -> Theme {
    let document: Value = serde_json::from_str(text).expect("a built-in theme is JSON");
    let name = document
        .get("name")
        .and_then(Value::as_str)
        .expect("a built-in theme has a name")
        .to_owned();
    let picks = [
        crate::names::THEME_DARK.write,
        crate::names::THEME_LIGHT.write,
    ];
    let appearances = document
        .get("appearances")
        .and_then(Value::as_array)
        .expect("a built-in theme has appearances")
        .iter()
        .zip(picks)
        .map(|(one, pick)| {
            let field = |key: &str| one.get(key).and_then(Value::as_str);
            let appearance = match field("appearance") {
                Some("light") => Appearance::Light,
                _ => Appearance::Dark,
            };
            let tokens = one.get("tokens").and_then(Value::as_object);
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
            let motion = motion_of(one.get("motion"))
                .unwrap_or_else(|missing| panic!("the built-in theme {name} has no {missing}"));
            ResolvedTheme {
                name: pick.to_owned(),
                id: field("id")
                    .expect("a built-in appearance has an id")
                    .to_owned(),
                appearance,
                high_contrast: false,
                values,
                motion,
            }
        })
        .collect();
    Theme { name, appearances }
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

/// The appearance purlis draws when nothing has said otherwise: Purlis Classic's dark.
pub fn default_theme() -> &'static ResolvedTheme {
    classic().first()
}

/// Purlis Classic's appearance of `appearance`: what a theme's appearance of that tag falls back
/// to, token by token.
pub fn built_in_for(appearance: Appearance) -> &'static ResolvedTheme {
    classic().of(appearance).unwrap_or_else(default_theme)
}

/// The built-in appearance `id` names, by its id or by an id it had before the rename (#1838):
/// the old ids map as the spec's table says, each to Purlis Classic's appearance of its tag
/// until the switch-over to Purlis.
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

/// **Reads one theme document**, format 1 or format 2, and always answers with a complete
/// theme. A format 2 theme may extend only Purlis Classic here; [`Library::load`] reads one
/// that extends a theme folder.
///
/// `document` is untrusted: the parse of a file purlis did not write. Nothing here panics and
/// nothing trusts a type.
pub fn load(document: &Value) -> Resolved {
    Library::new().load(None, document)
}

/// Whether `document` is format 2: it names the schema, a parent, a palette or appearances,
/// none of which format 1 has.
fn is_format_2(document: &serde_json::Map<String, Value>) -> bool {
    ["$schema", "extends", "palette", "appearances"]
        .iter()
        .any(|key| document.contains_key(*key))
}

/// **Reads a format 1 document**: one appearance. A token that is missing falls back to Purlis
/// Classic's of the theme's appearance silently, since not naming a token is how a small theme
/// says Classic's is right; a value that is there and unusable falls back too, and is said.
fn load_format_1(from: &serde_json::Map<String, Value>) -> Resolved {
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
        theme: Theme::one(ResolvedTheme {
            name,
            id: match appearance {
                Appearance::Dark => "dark",
                Appearance::Light => "light",
            }
            .to_owned(),
            appearance,
            high_contrast: false,
            values,
            motion,
        }),
        complaints,
        refused: false,
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
    pub theme: Option<Theme>,
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
    // A refused theme is not in force: the window draws Purlis and says why, as it does for a
    // file that is not a theme at all.
    if let Some(refused) = resolved.as_ref().filter(|it| it.refused) {
        let why: Vec<&str> = refused
            .complaints
            .iter()
            .map(|one| one.said.as_str())
            .collect();
        return Theirs {
            path: reading.path,
            found: reading.found,
            trouble: Some(format!("purlis cannot draw it: {}", why.join("; "))),
            ..Theirs::default()
        };
    }
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

/// **Checks one appearance of a theme**: every contrast pair, the terminal's ANSI floors and the
/// selected row kept apart, as it is and tinted by every colour a workspace can name. An
/// appearance with no findings can be read.
pub fn check(theme: &ResolvedTheme) -> Report {
    check::check(theme)
}

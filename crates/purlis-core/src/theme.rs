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
//!     light and optionally high contrast.
//!   - **Format 1** (today's single-appearance file) keeps loading as a theme with one
//!     appearance.
//!
//!   **One rule for both, and for every source**: a token the theme does not name is Purlis
//!   Classic's of the same appearance, silently; a token or motion name purlis does not have,
//!   and a key format 1 does not have, is said and left out; a value that is there and outside
//!   its type or bound, a key format 2 does not have, geometry in format 1, or a document that
//!   is not a theme **refuses the theme whole**
//!   (ADR 0097 §2). A refused theme draws Purlis, and the complaints name each value.
//!   **It never fails**: ADR 0026 holds cold start at 2 s, and the window has to come up
//!   whatever a file says.
//! - [`classic`] is Purlis Classic, the theme purlis ships, compiled into the core from its theme
//!   folder (`theme/built-in/purlis-classic/`); [`built_ins`] and [`built_in`] are its
//!   appearances by the ids a pick names them by, old ones included (#1838).
//! - [`resolve`] decides which theme, and which of its appearances, the window draws from every
//!   source there is today: the project's or workspace's pick, the person's own theme file, the
//!   themes approved extensions contribute, and the operating system's appearance ([`Inputs`]).
//! - **The consent floors hold whenever a theme is drawn** ([`Floor`], ADR 0097 §6): every
//!   answer [`resolve`], [`follow`] and [`draw`] give draws purlis's own values for the tokens of
//!   a floor the theme breaks, and says so in [`Drawn::held`].
//! - **The system's contrast is an input** ([`Contrast`], #2328): asking for more draws a
//!   theme's appearance tagged high contrast, of the tag that would be drawn, when it has one.
//! - **The native window follows the theme** ([`Native`], #2328): [`ResolvedTheme::native`] is
//!   the appearance the operating system draws the window's own parts in and its colour before
//!   the first frame, and [`first_frame`] is what a window draws as it is created.
//! - [`check`] measures one appearance against the contrast pairs and the terminal's floors, as
//!   it ships and tinted by every workspace colour, and answers with a [`Report`].
//! - [`import`] makes a theme folder's files from a VS Code colour theme, a base16 or base24
//!   scheme, an iTerm2 or a Ghostty palette through a fixed mapping table ([`table`]), and says
//!   every source key no token takes; [`export`] packs a theme folder as a zip that [`unpack`]
//!   gives back file for file (#2334).
//! - [`schema`] is the published JSON Schema of format 2, generated from this module's own
//!   vocabulary (`docs/schema/theme.v2.schema.json`, held to it by a test), and [`SCHEMA_URL`]
//!   its address at this release, which a format only ever grows at (`published`).
//!
//! **Geometry and fonts** (#2326, [`geometry`], [`Fonts`]) are read from a theme folder within
//! their bounds, and [`Person`] reads the person's density and font override, whose [`Layer`]
//! the window draws over every theme. The spec's later steps (theme options, the choice levels,
//! import and export) grow these same entries: `resolve` takes more inputs, `check` covers more
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

mod archive;
mod check;
pub mod choices;
mod floors;
mod folders;
mod fonts;
mod format;
pub mod gallery;
pub mod geometry;
mod icons;
mod import;
pub mod levels;
mod native;
mod options;
mod person;
mod published;
mod resolve;
mod schema;
mod tint;
mod typed;
pub mod vocabulary;
mod warnings;

pub use archive::{
    MOST_FILE_BYTES, MOST_FILES, MOST_FOLDER_BYTES, MOST_ZIP_BYTES, NotWritten, Packed, Staged,
    export, gather, is_folder_id, is_reserved, pack, rename_no_replace, stage, unpack,
};
pub use check::{Finding, PAIRS, Pair, Report};
pub use floors::{ALLOW_APART, DANGER_APART, Floor, Held};
pub use fonts::{
    CLASSIC_MONO, CLASSIC_TERMINAL, CLASSIC_UI, Face, Font, Fonts, MOST_FACES, MOST_FONT_BYTES,
    Style, served,
};
pub use format::{Library, MOST_APPEARANCES, THEME_JSON};
pub use geometry::Geometry;
pub use icons::{Icons, MOST_PATH, MOST_PATHS, Symbol, is_path};
pub use import::{
    Files, Imported, LeftOut, MOST_SOURCE_BYTES, SCOPES, SourceKind, detect, folder_name, import,
    scopes_markdown, table, table_markdown,
};
pub use levels::{Choice, Folders, Level, LevelInputs, Levels, resolve_levels};
pub use native::{Native, first_frame};
pub use options::{
    Kind, MOST_COMBINATIONS, OFF, ON, OptionDrawn, OptionValue, ThemeOption, Values,
};
pub use person::{Density, Layer, Person};
pub use published::{SCHEMA_FILE, SCHEMA_URL};
pub use resolve::{Drawn, Inputs, Offered, On, draw, follow, resolve};
pub use schema::{schema, schema_text};
pub use tint::{
    APART, Apart, Hue, PALETTE, Rule, Tint, apart_chroma, apart_chroma_of, hue_of, tint_hex,
    tint_hex_within, tinted,
};
pub use typed::Typed;
use vocabulary::{
    DURATIONS, EASINGS, LONGEST, TOKENS, is_colour, is_platform, is_token, names_geometry,
};
pub use warnings::{combination as combination_said, said as warnings_said};

#[cfg(test)]
mod floors_tests;
#[cfg(test)]
mod folders_tests;
#[cfg(test)]
mod format_tests;
#[cfg(test)]
mod gallery_tests;
#[cfg(test)]
mod geometry_tests;
#[cfg(test)]
mod icons_tests;
#[cfg(test)]
mod import_tests;
#[cfg(test)]
mod levels_tests;
#[cfg(test)]
mod native_tests;
#[cfg(test)]
mod options_tests;
#[cfg(test)]
mod published_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tint_tests;
#[cfg(test)]
mod typed_tests;

/// Whether a theme is drawn on a dark window or a light one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Appearance {
    Dark,
    Light,
}

/// **What contrast the operating system asks for** (#2328): more when the person turned on
/// Increase contrast, or `prefers-contrast: more` says so. Asking for more draws a theme's
/// appearance tagged high contrast, of the tag that would be drawn anyway, when it has one.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Contrast {
    /// Nothing asked: an appearance tagged high contrast is drawn only when it is the one chosen.
    #[default]
    Standard,
    /// More contrast asked for.
    More,
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
/// It carries the icon set (#2337), the geometry for both densities and the fonts (#2326); and the
/// option values it was drawn with (#2325). Its users are the window's wire types
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
    /// Each token that is not a colour (#2327): the syntax roles' italic and bold, and the
    /// terminal's cursor, line height and bold text.
    pub typed: Typed,
    /// The theme's icon set (#2337), the same in each of its appearances: empty when the rails
    /// draw the window's own marks, as Purlis Classic's do.
    #[serde(skip_serializing_if = "Icons::is_empty")]
    pub icons: Icons,
    /// Each geometry token's value, comfortable ([`geometry::STEPS`]).
    pub geometry: Geometry,
    /// Each geometry token's value when the person chose compact.
    pub compact: Geometry,
    /// What the theme's `compact` layer tunes, with its parent's: what a theme that extends
    /// this one tunes over.
    #[serde(skip)]
    pub(crate) tuned: Geometry,
    /// The UI, mono and terminal fonts.
    pub fonts: Fonts,
    /// Each of the theme's options (#2325), in its order, with the value it was drawn with:
    /// empty for a theme with none.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<OptionDrawn>,
    /// How a workspace's colour tints the theme (#2329), the same in each of its appearances:
    /// Purlis Classic's recipe unless the theme says otherwise.
    pub tint: Tint,
}

impl ResolvedTheme {
    /// The value of `token`, which every resolved theme has for every token of [`TOKENS`].
    pub fn value(&self, token: &str) -> &str {
        self.values.get(token).map_or("", String::as_str)
    }
}

/// **A theme**: its name and every appearance it has, each complete, in the order it declares
/// them, and its options. Never without an appearance.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Theme {
    /// What to call it, as a person would.
    pub name: String,
    /// Its appearances, the first one what it draws when nothing chose another, each drawn with
    /// its option values: each option's default, until [`Self::with`] draws others.
    ///
    /// **Derived**: drawn from the appearances the theme declares and its options when it is
    /// read. Edit a theme through its reader ([`load`], [`Library`]) or draw other values with
    /// [`Self::with`]; a change made here in place is lost the next time the theme is drawn.
    pub appearances: Vec<ResolvedTheme>,
    /// Its options (#2325), in its order: a theme it extends' first, then its own.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<ThemeOption>,
    /// Its appearances as it declares them, before any option is drawn over them: what
    /// [`Self::with`] draws from, and what a theme that extends this one layers over.
    #[serde(skip)]
    bases: Vec<ResolvedTheme>,
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

    /// Its appearance tagged `appearance` for `contrast`: with more contrast asked for, the first
    /// such tagged high contrast when it has one; otherwise, and with none asked, [`Self::of`].
    pub fn of_for(&self, appearance: Appearance, contrast: Contrast) -> Option<&ResolvedTheme> {
        let sharp = || {
            self.appearances
                .iter()
                .find(|one| one.appearance == appearance && one.high_contrast)
        };
        match contrast {
            Contrast::More => sharp().or_else(|| self.of(appearance)),
            Contrast::Standard => self.of(appearance),
        }
    }

    /// What the theme draws when nothing chose an appearance, for `contrast`: its first, or, with
    /// more contrast asked for, its appearance of the first's tag that is tagged high contrast,
    /// when it has one.
    pub fn first_for(&self, contrast: Contrast) -> &ResolvedTheme {
        let first = self.first();
        match contrast {
            Contrast::More if !first.high_contrast => self
                .of_for(first.appearance, contrast)
                .filter(|one| one.high_contrast)
                .unwrap_or(first),
            _ => first,
        }
    }

    /// The theme as one appearance: what a format 1 file is.
    fn one(theme: ResolvedTheme) -> Self {
        Self::made(theme.name.clone(), vec![theme], Vec::new())
    }

    /// The theme `name` with `bases`, its appearances as it declares them, and `options`: each
    /// appearance drawn with every option's default.
    fn made(name: String, bases: Vec<ResolvedTheme>, options: Vec<ThemeOption>) -> Self {
        let picked = options::pick(&name, &options, &Values::new(), &mut Vec::new());
        Self {
            appearances: bases
                .iter()
                .map(|base| options::draw(base, &options, &picked))
                .collect(),
            name,
            options,
            bases,
        }
    }

    /// **The theme drawn with `values`**: each option's value from `values` where it names one
    /// the option has, else its default. A value or an option the theme does not have is said
    /// in the complaints, and the default is drawn: a choice made for another version of the
    /// theme never refuses it.
    #[must_use]
    pub fn with(&self, values: &Values) -> (Self, Vec<Complaint>) {
        let mut complaints = Vec::new();
        let picked = options::pick(&self.name, &self.options, values, &mut complaints);
        let theme = Self {
            appearances: self
                .bases
                .iter()
                .map(|base| options::draw(base, &self.options, &picked))
                .collect(),
            ..self.clone()
        };
        (theme, complaints)
    }

    /// How many combinations of appearance and option values the theme has: what the theme
    /// check covers, at most [`MOST_COMBINATIONS`].
    pub fn combinations(&self) -> usize {
        options::combinations(self.bases.len(), &self.options)
    }

    /// Its appearances as it declares them, before any option is drawn: a parent's, for a theme
    /// that extends it.
    pub(crate) fn bases(&self) -> &[ResolvedTheme] {
        &self.bases
    }
}

/// One thing [`load`] found wrong with a theme, which refuses it: a value the file did not
/// supply usably.
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
    /// Why it was refused; empty when it was not.
    pub complaints: Vec<Complaint>,
    /// Whether the document was refused whole, so [`Self::theme`] is Purlis and not the
    /// document's (a format 2 theme that breaks its grammar, ADR 0097 §2).
    pub refused: bool,
}

impl Resolved {
    /// The document refused for one reason.
    pub(crate) fn refused_for(why: String) -> Self {
        Self::refusal(vec![Complaint::said(why)])
    }

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

/// Purlis Classic's `theme.json` as its folder holds it: what `purlis theme export
/// purlis-classic` packs.
pub const CLASSIC_THEME_JSON: &str = CLASSIC_FILE;

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
                        .filter(|value| is_colour(value) || is_platform(token, value))
                        .unwrap_or_else(|| panic!("the built-in theme {name} has no {token}"));
                    (*token, value.to_owned())
                })
                .collect();
            let motion = motion_of(one.get("motion"))
                .unwrap_or_else(|missing| panic!("the built-in theme {name} has no {missing}"));
            let typed = typed::complete(tokens)
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
                typed,
                icons: Icons::default(),
                // Core's geometry and fonts are Purlis Classic's: its folder names none.
                geometry: geometry::classic(),
                compact: geometry::classic_compact(),
                tuned: Geometry::new(),
                fonts: Fonts::classic(),
                options: Vec::new(),
                tint: Tint::default(),
            }
        })
        .collect();
    Theme::made(name, appearances, Vec::new())
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
    // Purlis Classic by its own id is the theme, which draws its first appearance when nothing
    // chose another.
    if id == CLASSIC {
        return Some(classic().first());
    }
    let id = [crate::names::THEME_DARK, crate::names::THEME_LIGHT]
        .into_iter()
        .find(|name| name.recognises(id))
        .map_or(id, |name| name.write);
    built_ins().iter().find(|theme| theme.name == id)
}

/// Why a format 1 theme that names geometry is refused, after the key that named it: geometry
/// is set in a theme folder's `theme.json` (format 2), within its bounds (#2326).
const GEOMETRY_IS_CORE: &str =
    "is geometry, which a theme sets in a theme folder's theme.json (format 2), not here";

/// The keys of a format 1 theme file.
const FORMAT_1_KEYS: &[&str] = &["name", "appearance", "tokens", "motion"];

/// **Reads one theme document**, format 1 or format 2, and always answers with a complete
/// theme. A format 2 theme may extend only Purlis Classic here; [`Library::load`] reads one
/// that extends a theme folder.
///
/// `document` is untrusted: the parse of a file purlis did not write. Nothing here panics and
/// nothing trusts a type.
pub fn load(document: &Value) -> Resolved {
    Library::new().load(None, document)
}

/// Whether `name` is a theme folder's name a pick may name it by: lower-case letters, digits and
/// dashes, starting with a letter, at most 32 long — the grammar of an appearance's id too.
pub fn is_folder_name(name: &str) -> bool {
    format::is_name(name)
}

/// Whether `document` is format 2: it names the schema, a parent, a palette or appearances,
/// none of which format 1 has.
fn is_format_2(document: &serde_json::Map<String, Value>) -> bool {
    ["$schema", "extends", "palette", "appearances"]
        .iter()
        .any(|key| document.contains_key(*key))
}

/// **Reads a format 1 document**: one appearance. A token that is missing is Purlis Classic's of
/// the theme's appearance, silently, since not naming a token is how a small theme says
/// Classic's is right; a value that is there and unusable refuses the theme, and is said. A
/// token or motion name purlis does not have is said and left out, and does not refuse it: a
/// name reaches nothing, and a teammate's purlis may have a token this one does not yet.
fn load_format_1(from: &serde_json::Map<String, Value>) -> Resolved {
    let mut complaints = Vec::new();
    let mut notes = Vec::new();

    // A key the file does not have says nothing: no appearance is dark, as no token is Purlis
    // Classic's. A key that is there and not a value of its type refuses the theme.
    let appearance = match from.get("appearance") {
        None => Appearance::Dark,
        Some(Value::String(tag)) if tag == "light" => Appearance::Light,
        Some(Value::String(tag)) if tag == "dark" => Appearance::Dark,
        Some(other) => {
            complaints.push(Complaint::said(format!(
                "appearance is {}; a theme's appearance is \"dark\" or \"light\"",
                stringify(other)
            )));
            Appearance::Dark
        }
    };
    let base = built_in_for(appearance);

    let name = match from.get("name").and_then(Value::as_str).map(str::trim) {
        Some(name) if !name.is_empty() => name.to_owned(),
        _ => {
            complaints.push(Complaint::said("a theme has a name"));
            base.name.clone()
        }
    };

    let empty = serde_json::Map::new();
    let tokens = match from.get("tokens") {
        Some(Value::Object(tokens)) => tokens,
        // A theme that names no token is Purlis Classic's of its appearance, as one that names
        // a few is for the rest.
        None => &empty,
        Some(_) => {
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
        if typed::is_typed(given) {
            notes.push(Complaint::about(
                given,
                format!(
                    "{given} is set in a theme folder's theme.json (format 2), so it is left out"
                ),
            ));
        } else if names_geometry(given) {
            complaints.push(Complaint::about(
                given,
                format!("{given} {GEOMETRY_IS_CORE}"),
            ));
        } else {
            notes.push(Complaint::about(
                given,
                format!("{given} is not a purlis token, so it is left out"),
            ));
        }
    }
    // Format 1 is frozen, and people's files and extensions' themes carry keys of their own
    // (`author`, `description`, `$comment`): a key it does not have is said and left out, as an
    // unknown token name is. Geometry still refuses the theme, by name (ADR 0097 §7).
    for key in from.keys() {
        if FORMAT_1_KEYS.contains(&key.as_str()) {
            continue;
        }
        if names_geometry(key) {
            complaints.push(Complaint::said(format!("{key} {GEOMETRY_IS_CORE}")));
        } else {
            notes.push(Complaint::said(format!(
                "{key} is not a key of a theme file, so it is left out"
            )));
        }
    }

    let mut values = BTreeMap::new();
    for token in TOKENS {
        match tokens.get(*token) {
            Some(Value::String(given)) if is_colour(given) => {
                values.insert(*token, given.clone());
            }
            given => {
                values.insert(*token, base.value(token).to_owned());
                if let Some(given) = given {
                    complaints.push(Complaint::about(
                        token,
                        format!(
                            "{token} is {}, which is not #rgb, #rgba, #rrggbb or #rrggbbaa",
                            stringify(given)
                        ),
                    ));
                }
            }
        }
    }

    // A theme that says nothing about motion moves the way the built-in of its appearance does,
    // exactly as a token it does not name is coloured by that built-in.
    let motion = load_motion(
        from.get("motion"),
        &base.motion,
        &mut complaints,
        &mut notes,
    );

    // A value outside its type refuses the whole theme (ADR 0097 §2), as in format 2: the
    // window draws Purlis and the complaints name each value.
    if !complaints.is_empty() {
        complaints.extend(notes);
        return Resolved::refusal(complaints);
    }
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
            typed: base.typed.clone(),
            icons: Icons::default(),
            geometry: base.geometry.clone(),
            compact: base.compact.clone(),
            tuned: base.tuned.clone(),
            fonts: base.fonts.clone(),
            options: Vec::new(),
            tint: Tint::default(),
        }),
        complaints: notes,
        refused: false,
    }
}

/// [`load`] of a theme's text. Text that is not JSON is refused like any other theme that is
/// not one: Purlis is drawn, and the complaint says why.
pub fn load_text(text: &str) -> Resolved {
    load_text_in(text, None)
}

/// [`load_text`] of a `theme.json` read from `folder`, the theme's folder, whose `fonts/` its
/// fonts are read from: the person's own, in their config folder ([`read_theirs`]), whose
/// handle is opened when the text has been read.
pub(crate) fn load_text_in(text: &str, folder: Option<&std::path::Path>) -> Resolved {
    match load_text_document(text) {
        Ok(document) => match folder {
            Some(folder) => Library::new().load_in(None, &document, folder),
            None => Library::new().load(None, &document),
        },
        Err(why) => Resolved::refusal(vec![Complaint::said(why)]),
    }
}

/// A theme's text as JSON, or the one sentence every source says when it is not.
fn load_text_document(text: &str) -> Result<Value, String> {
    serde_json::from_str::<Value>(text).map_err(|why| format!("a theme is JSON: {why}"))
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

/// Reads a theme's `motion` over `base`, saying in `complaints` what it does not supply usably
/// (which refuses the theme) and in `notes` a motion name purlis does not have (left out).
fn load_motion(
    raw: Option<&Value>,
    base: &Motion,
    complaints: &mut Vec<Complaint>,
    notes: &mut Vec<Complaint>,
) -> Motion {
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
            notes.push(Complaint::said(format!(
                "{name} is not a purlis motion token, so it is left out"
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
                "{token} is {}, which is not a whole number of milliseconds from 0 to {LONGEST}",
                stringify(value),
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
            None => complaints.push(Complaint::said(format!(
                "{token} is {}, which is not [x1, y1, x2, y2] with each x from 0 to 1 and each y \
                 from -1 to 2",
                stringify(value),
            ))),
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
    /// Whether it is a format 1 file in force, which purlis offers, once, to move into a theme
    /// folder ([`levels::move_into_a_folder`]).
    pub format_1: bool,
}

/// Reads the person's own theme under `config_root` and resolves it. A machine with no config
/// home has none. Bounded and link-refusing, as every file beside `machine.json` is read
/// ([`crate::windowprefs::read_theme_text`]).
pub fn read_theirs(config_root: Option<&std::path::Path>) -> Theirs {
    let Some(root) = config_root else {
        return Theirs::default();
    };
    let reading = crate::windowprefs::read_theme_text(root);
    let Some(text) = reading.text else {
        return Theirs {
            path: reading.path,
            found: reading.found,
            trouble: reading.trouble,
            ..Theirs::default()
        };
    };
    // The person's `theme.json` sits in their config folder, which is its theme folder: its
    // fonts are that folder's `fonts/`.
    let resolved = load_text_in(&text, Some(&crate::machine::dir(root)));
    let format_1 = serde_json::from_str::<Value>(&text)
        .ok()
        .as_ref()
        .and_then(Value::as_object)
        .is_some_and(|document| !is_format_2(document));
    // A refused theme is not in force: the window draws Purlis and says why. What refused it is
    // kept as it was found, beside the sentence.
    if resolved.refused {
        let why: Vec<&str> = resolved
            .complaints
            .iter()
            .map(|one| one.said.as_str())
            .collect();
        return Theirs {
            path: reading.path,
            found: true,
            trouble: Some(format!("purlis cannot draw it: {}", why.join("; "))),
            theme: None,
            complaints: resolved.complaints,
            format_1: false,
        };
    }
    Theirs {
        path: reading.path,
        found: true,
        trouble: None,
        complaints: resolved.complaints,
        theme: Some(resolved.theme),
        format_1,
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

/// One combination of appearance and option values the theme check covered, and its report.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Checked {
    /// The appearance's id.
    pub appearance: String,
    /// The value of each option it was drawn with: empty for a theme with no options.
    pub options: Values,
    pub report: Report,
    /// The consent floors the window holds when it draws this combination, and the tokens it
    /// draws as purlis's own for them ([`draw`]); `None` when it clears them.
    pub held: Option<Held>,
}

/// **Checks every combination of a theme's appearances and option values** (#2325): [`check`]
/// of each appearance drawn with each combination of option values, the appearances in the
/// theme's order and, within each, the combinations with the first option's values slowest. A
/// theme has at most [`MOST_COMBINATIONS`]: one with more is refused when it is read. Each
/// says what the window draws of it too: the consent floors it holds there ([`Checked::held`]).
pub fn check_every(theme: &Theme) -> Vec<Checked> {
    let every = options::every(&theme.options);
    let drawn: Vec<(Values, Theme)> = every
        .into_iter()
        .map(|values| {
            let (drawn, _) = theme.with(&values);
            (values, drawn)
        })
        .collect();
    let mut out = Vec::with_capacity(theme.combinations());
    for at in 0..theme.bases.len() {
        for (values, drawn) in &drawn {
            let one = &drawn.appearances[at];
            out.push(Checked {
                appearance: one.id.clone(),
                options: values.clone(),
                report: check::check(one),
                held: floors::hold(one).1,
            });
        }
    }
    out
}

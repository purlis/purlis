// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Format 2** (spec #2313, #2322): a theme folder's `theme.json`.
//!
//! ```json
//! {
//!   "$schema": "…/theme.schema.json",
//!   "name": "Dusk",
//!   "extends": "purlis-classic",
//!   "palette": { "lavender": "#b4a7f5" },
//!   "appearances": [
//!     { "id": "dark", "appearance": "dark", "tokens": { "accent.base": "lavender" } },
//!     { "id": "contrast", "appearance": "dark", "high-contrast": true, "tokens": { … } }
//!   ]
//! }
//! ```
//!
//! - **`extends`** names another theme: Purlis Classic (`purlis-classic`, or an old id), or a
//!   theme folder the [`Library`] holds, by the folder's name. The child starts from its parent's
//!   appearances; an appearance with a parent's id layers over that one, and a new id is added
//!   after them. A parent that is not there, a parent purlis cannot draw, and themes that extend
//!   one another in a circle each refuse the theme with a sentence that names them.
//! - **The palette** is named colours, hex only. A token's value is hex or a palette name: a
//!   closed grammar, never CSS. A name is local to the theme that declares it, so a child cannot
//!   name its parent's (ADR 0041's amendment of 2026-10-16).
//! - **An appearance** has an id, a tag (`dark` or `light`) and optionally `high-contrast`, and
//!   its token and motion values. One without a parent's appearance to layer over starts from
//!   Purlis Classic's of its tag, so a theme holds only what it changes.
//!
//! **A format 2 theme that breaks its grammar anywhere is refused whole** (ADR 0097 §2): it draws
//! Purlis, and every complaint says what is wrong. Geometry, fonts, icons, options and tint are
//! not read yet (#2325, #2326), so naming one refuses the theme rather than drawing something
//! other than what it says.

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{Map, Value};

use super::vocabulary::{TOKENS, is_colour, names_geometry};
use super::{
    Appearance, CLASSIC, Complaint, GEOMETRY_IS_CORE, Resolved, ResolvedTheme, Theme, built_in_for,
    classic, is_format_2, load_format_1, load_motion, stringify,
};

/// The file a theme folder is read from.
pub const THEME_JSON: &str = "theme.json";

/// The most appearances a theme may have: the spec's cap on what the check covers (every
/// appearance × option combination, 64 at most).
pub const MOST_APPEARANCES: usize = 64;

/// The most colours a palette may name.
const MOST_COLOURS: usize = 256;

/// The largest `theme.json` purlis reads.
const MOST_BYTES: u64 = 512 * 1024;

/// The keys of a format 2 `theme.json` this purlis reads.
const KEYS: &[&str] = &["$schema", "name", "extends", "palette", "appearances"];

/// The keys the spec gives format 2 that this purlis does not read yet.
const NOT_YET: &[&str] = &["fonts", "icons", "options", "tint"];

/// The keys of an appearance.
const APPEARANCE_KEYS: &[&str] = &["id", "appearance", "high-contrast", "tokens", "motion"];

/// The grammar of a palette name and an appearance id, as a sentence.
const NAME_RULE: &str =
    "is not lower-case letters, digits and dashes starting with a letter, at most 32 long";

/// Whether `name` is a palette name or an appearance id: `[a-z][a-z0-9-]{0,31}`.
pub(super) fn is_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    name.len() <= 32
        && bytes.next().is_some_and(|b| b.is_ascii_lowercase())
        && bytes.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// **The themes a theme may extend**: Purlis Classic, and the theme folders handed to it, by
/// their ids (a folder's name). Purlis Classic's ids are its own: no folder takes them.
#[derive(Debug, Clone, Default)]
pub struct Library {
    folders: BTreeMap<String, Value>,
}

/// Why a theme could not be read.
struct Refused {
    complaints: Vec<Complaint>,
    why: Why,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Why {
    /// Its grammar: the complaints say where.
    Broken,
    /// A parent that is not there, which the theme that names it says.
    Missing,
    /// A circle of `extends`: said once, where it closes, and handed up unchanged.
    Circle,
}

impl Refused {
    fn because(complaints: Vec<Complaint>) -> Self {
        Self {
            complaints,
            why: Why::Broken,
        }
    }
}

impl Library {
    /// Purlis Classic, and no folder.
    pub fn new() -> Self {
        Self::default()
    }

    /// The library with `document` as the theme `id`.
    #[must_use]
    pub fn with(mut self, id: impl Into<String>, document: Value) -> Self {
        self.folders.insert(id.into(), document);
        self
    }

    /// The library with the theme folder at `folder`, by its name. A folder whose `theme.json`
    /// cannot be read is held as nothing, and a theme that extends it is told so.
    #[must_use]
    pub fn with_folder(self, folder: &Path) -> Self {
        let Some(id) = folder.file_name().and_then(|name| name.to_str()) else {
            return self;
        };
        match read_folder(folder) {
            Ok(document) => self.with(id, document),
            Err(_) => self,
        }
    }

    /// **Reads one theme document**, `id` the theme's own id when it has one (a folder's name),
    /// and always answers with a complete theme.
    pub fn load(&self, id: Option<&str>, document: &Value) -> Resolved {
        let Some(from) = document.as_object() else {
            return Resolved::refusal(vec![Complaint::said("a theme is a JSON object")]);
        };
        if !is_format_2(from) {
            return load_format_1(from);
        }
        let mut path: Vec<String> = id.into_iter().map(str::to_owned).collect();
        match self.format_2(from, &mut path) {
            Ok(theme) => Resolved {
                theme,
                complaints: Vec::new(),
                refused: false,
            },
            Err(refused) => Resolved::refusal(refused.complaints),
        }
    }

    /// [`Self::load`] of the theme the library holds as `id`.
    pub fn load_id(&self, id: &str) -> Resolved {
        match self.folders.get(id) {
            Some(document) => self.load(Some(id), document),
            None => Resolved::refusal(vec![Complaint::said(format!(
                "\"{id}\" is not a theme purlis has"
            ))]),
        }
    }

    /// **Reads the theme folder at `folder`**: its `theme.json`, which may sit alone or beside
    /// `fonts/`, `icons/`, a preview and a licence. Its id is the folder's name.
    pub fn load_folder(&self, folder: &Path) -> Resolved {
        match read_folder(folder) {
            Ok(document) => self.load(folder.file_name().and_then(|name| name.to_str()), &document),
            Err(why) => Resolved::refusal(vec![Complaint::said(why)]),
        }
    }

    /// A format 2 theme, or why not. `path` is the ids of the themes being read, the one that
    /// extends first, so a circle is seen when it closes.
    fn format_2(
        &self,
        from: &Map<String, Value>,
        path: &mut Vec<String>,
    ) -> Result<Theme, Refused> {
        let mut complaints = Vec::new();
        for key in from.keys() {
            if NOT_YET.contains(&key.as_str()) {
                complaints.push(Complaint::said(format!(
                    "{key} is not read by this purlis yet, so the theme cannot be drawn as written"
                )));
            } else if !KEYS.contains(&key.as_str()) {
                complaints.push(Complaint::said(format!("{key} is not a key of theme.json")));
            }
        }
        if from.get("$schema").is_some_and(|it| !it.is_string()) {
            complaints.push(Complaint::said("$schema is the address of a schema"));
        }
        let name = match from.get("name").and_then(Value::as_str).map(str::trim) {
            Some(name) if !name.is_empty() => name.to_owned(),
            _ => {
                complaints.push(Complaint::said("a theme has a name"));
                String::new()
            }
        };
        let called = if name.is_empty() { "this theme" } else { &name };
        let palette = palette(from.get("palette"), &mut complaints);

        // The parent's appearances, complete: what this theme's layer over.
        let mut appearances: Vec<ResolvedTheme> = match from.get("extends") {
            None => Vec::new(),
            Some(Value::String(parent)) => match self.parent(parent, path) {
                Ok(theme) => theme.appearances,
                Err(refused) if refused.why == Why::Circle => return Err(refused),
                Err(refused) if refused.why == Why::Missing => {
                    complaints.push(Complaint::said(format!(
                        "{called} extends \"{parent}\", which is not a theme purlis has"
                    )));
                    Vec::new()
                }
                Err(refused) => {
                    complaints.push(Complaint::said(format!(
                        "{called} extends \"{parent}\", which purlis cannot draw"
                    )));
                    complaints.extend(refused.complaints);
                    Vec::new()
                }
            },
            Some(_) => {
                complaints.push(Complaint::said("extends is the id of another theme"));
                Vec::new()
            }
        };

        let given: &[Value] = match from.get("appearances") {
            None => &[],
            Some(Value::Array(given)) => given,
            Some(_) => {
                complaints.push(Complaint::said("appearances is a list of appearances"));
                &[]
            }
        };
        let inherited = appearances.len();
        let mut seen: Vec<String> = Vec::new();
        for (at, raw) in given.iter().enumerate() {
            let Some(one) = raw.as_object() else {
                complaints.push(Complaint::said(format!(
                    "appearance {} is not an object",
                    at + 1
                )));
                continue;
            };
            let Some(id) = appearance_id(one, at, &mut complaints) else {
                continue;
            };
            if seen.contains(&id) {
                complaints.push(Complaint::said(format!("two appearances are called {id}")));
                continue;
            }
            seen.push(id.clone());
            let parents = appearances[..inherited].iter().position(|it| it.id == id);
            let layered = appearance(
                one,
                &id,
                parents.map(|at| &appearances[at]),
                &palette,
                &mut complaints,
            );
            match (layered, parents) {
                (Some(layered), Some(at)) => appearances[at] = layered,
                (Some(layered), None) => appearances.push(layered),
                (None, _) => {}
            }
        }
        if appearances.is_empty() && complaints.is_empty() {
            complaints.push(Complaint::said(format!("{called} has no appearance")));
        }
        if appearances.len() > MOST_APPEARANCES {
            complaints.push(Complaint::said(format!(
                "{called} has {} appearances, and a theme has at most {MOST_APPEARANCES}",
                appearances.len()
            )));
        }
        if !complaints.is_empty() {
            return Err(Refused::because(complaints));
        }
        for one in &mut appearances {
            one.name.clone_from(&name);
        }
        Ok(Theme { name, appearances })
    }

    /// The theme `parent` names, read, or why not.
    fn parent(&self, parent: &str, path: &mut Vec<String>) -> Result<Theme, Refused> {
        if path.iter().any(|id| id == parent) {
            let from = path.iter().position(|id| id == parent).unwrap_or_default();
            let mut circle: Vec<&str> = path[from..].iter().map(String::as_str).collect();
            circle.push(parent);
            return Err(Refused {
                complaints: vec![Complaint::said(format!(
                    "the themes extend one another in a circle: {}",
                    circle.join(" extends ")
                ))],
                why: Why::Circle,
            });
        }
        if is_classic(parent) {
            return Ok(classic().clone());
        }
        let Some(document) = self.folders.get(parent) else {
            return Err(Refused {
                complaints: Vec::new(),
                why: Why::Missing,
            });
        };
        let Some(from) = document.as_object() else {
            return Err(Refused::because(vec![Complaint::said(
                "a theme is a JSON object",
            )]));
        };
        if !is_format_2(from) {
            let loaded = load_format_1(from);
            return if loaded.complaints.is_empty() {
                Ok(loaded.theme)
            } else {
                Err(Refused::because(loaded.complaints))
            };
        }
        path.push(parent.to_owned());
        let read = self.format_2(from, path);
        path.pop();
        read
    }
}

/// Whether `id` names Purlis Classic: its id, or an id one of its appearances is picked by.
fn is_classic(id: &str) -> bool {
    id == CLASSIC
        || [crate::names::THEME_DARK, crate::names::THEME_LIGHT]
            .iter()
            .any(|name| name.recognises(id) || name.write == id)
}

/// A folder's `theme.json`, parsed, or why not. Read without following a link, as a plain file
/// and under a size cap: a theme folder may be anybody's.
fn read_folder(folder: &Path) -> Result<Value, String> {
    use std::io::Read;
    let file = folder.join(THEME_JSON);
    let shown = file.display();
    let mut open = match crate::contain::open_no_link(folder, &file) {
        Ok(open) => open,
        Err(gone) if gone.kind() == std::io::ErrorKind::NotFound => {
            return Err(format!("{} has no {THEME_JSON}", folder.display()));
        }
        Err(why) => return Err(format!("purlis could not read {shown}: {why}")),
    };
    let found = open
        .metadata()
        .map_err(|why| format!("purlis could not read {shown}: {why}"))?;
    if !found.file_type().is_file() {
        return Err(format!("{shown} is not a plain file"));
    }
    if found.len() > MOST_BYTES {
        return Err(format!(
            "{shown} is {} bytes, and a theme is never larger than {MOST_BYTES}",
            found.len()
        ));
    }
    let mut text = String::new();
    open.read_to_string(&mut text)
        .map_err(|why| format!("purlis could not read {shown}: {why}"))?;
    serde_json::from_str(&text).map_err(|why| format!("{shown} is not JSON: {why}"))
}

/// A theme's palette, each name to its hex, saying what is wrong with any of it.
fn palette(raw: Option<&Value>, complaints: &mut Vec<Complaint>) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let Some(raw) = raw else {
        return out;
    };
    let Some(given) = raw.as_object() else {
        complaints.push(Complaint::said("palette is an object of names to colours"));
        return out;
    };
    if given.len() > MOST_COLOURS {
        complaints.push(Complaint::said(format!(
            "palette names {} colours, and a palette has at most {MOST_COLOURS}",
            given.len()
        )));
        return out;
    }
    for (name, value) in given {
        if !is_name(name) {
            complaints.push(Complaint::said(format!(
                "palette name {} {NAME_RULE}",
                stringify(&Value::String(name.clone()))
            )));
            continue;
        }
        match value.as_str() {
            Some(hex) if is_colour(hex) => {
                out.insert(name.clone(), hex.to_owned());
            }
            _ => complaints.push(Complaint::said(format!(
                "palette colour {name} is {}, which is not #rgb, #rgba, #rrggbb or #rrggbbaa",
                stringify(value)
            ))),
        }
    }
    out
}

/// An appearance's id, or `None` (and why) when it has no usable one.
fn appearance_id(
    one: &Map<String, Value>,
    at: usize,
    complaints: &mut Vec<Complaint>,
) -> Option<String> {
    match one.get("id") {
        None => {
            complaints.push(Complaint::said(format!("appearance {} has no id", at + 1)));
            None
        }
        Some(Value::String(id)) if is_name(id) => Some(id.clone()),
        Some(other) => {
            complaints.push(Complaint::said(format!(
                "appearance id {} {NAME_RULE}",
                stringify(other)
            )));
            None
        }
    }
}

/// One appearance, layered over `parent` (the parent theme's of the same id) or else over Purlis
/// Classic's of its tag; `None` when anything in it is wrong, which is said.
fn appearance(
    one: &Map<String, Value>,
    id: &str,
    parent: Option<&ResolvedTheme>,
    palette: &BTreeMap<String, String>,
    complaints: &mut Vec<Complaint>,
) -> Option<ResolvedTheme> {
    let before = complaints.len();
    for key in one.keys() {
        if !APPEARANCE_KEYS.contains(&key.as_str()) {
            complaints.push(Complaint::said(format!(
                "{key} is not a key of an appearance"
            )));
        }
    }
    let tag = match one.get("appearance") {
        Some(Value::String(tag)) if tag == "dark" => Some(Appearance::Dark),
        Some(Value::String(tag)) if tag == "light" => Some(Appearance::Light),
        None => None,
        Some(other) => {
            complaints.push(Complaint::said(format!(
                "appearance {id} is {}; an appearance is \"dark\" or \"light\"",
                stringify(other)
            )));
            return None;
        }
    };
    let appearance = match (tag, parent) {
        (Some(tag), Some(parent)) if tag != parent.appearance => {
            complaints.push(Complaint::said(format!(
                "appearance {id} is {} in the theme it extends, so it cannot be {}",
                tag_word(parent.appearance),
                tag_word(tag)
            )));
            return None;
        }
        (Some(tag), _) => tag,
        (None, Some(parent)) => parent.appearance,
        (None, None) => {
            complaints.push(Complaint::said(format!(
                "appearance {id} says neither \"dark\" nor \"light\""
            )));
            return None;
        }
    };
    let high_contrast = match one.get("high-contrast") {
        None => parent.is_some_and(|it| it.high_contrast),
        Some(Value::Bool(high)) => *high,
        Some(other) => {
            complaints.push(Complaint::said(format!(
                "high-contrast in appearance {id} is {}, which is not true or false",
                stringify(other)
            )));
            false
        }
    };
    let base = parent.unwrap_or_else(|| built_in_for(appearance));
    let mut values = base.values.clone();
    match one.get("tokens") {
        None => {}
        Some(Value::Object(tokens)) => {
            for (token, value) in tokens {
                let Some(known) = TOKENS.iter().copied().find(|it| *it == token) else {
                    complaints.push(Complaint::about(
                        token,
                        if names_geometry(token) {
                            format!("{token} in appearance {id} {GEOMETRY_IS_CORE}")
                        } else {
                            format!("{token} in appearance {id} is not a purlis token")
                        },
                    ));
                    continue;
                };
                match colour(value, palette) {
                    Some(hex) => {
                        values.insert(known, hex);
                    }
                    None => complaints.push(Complaint::about(
                        token,
                        format!(
                            "{token} in appearance {id} is {}, which is neither a hex colour \
                             nor a name in this theme's palette",
                            stringify(value)
                        ),
                    )),
                }
            }
        }
        Some(_) => complaints.push(Complaint::said(format!(
            "tokens in appearance {id} is an object of token names to colours"
        ))),
    }
    let mut said = Vec::new();
    let motion = load_motion(one.get("motion"), &base.motion, &mut said);
    complaints.extend(said);
    if complaints.len() != before {
        return None;
    }
    Some(ResolvedTheme {
        name: String::new(),
        id: id.to_owned(),
        appearance,
        high_contrast,
        values,
        motion,
    })
}

/// A token's value: hex, or the hex of a name in `palette`. Anything else is not a colour.
fn colour(value: &Value, palette: &BTreeMap<String, String>) -> Option<String> {
    let given = value.as_str()?;
    if is_colour(given) {
        return Some(given.to_owned());
    }
    palette.get(given).cloned()
}

fn tag_word(appearance: Appearance) -> &'static str {
    match appearance {
        Appearance::Dark => "dark",
        Appearance::Light => "light",
    }
}

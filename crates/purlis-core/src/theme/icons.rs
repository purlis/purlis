// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A theme's icon set** (spec #2313, #2337): what each view's tab on the rails is drawn as.
//!
//! A theme folder's `theme.json` names its set by a path inside the folder (`"icons":
//! "icons/icons.json"`), and the file is data, never SVG:
//!
//! ```json
//! { "views": { "chats": { "paths": ["M22 17a2 2 0 0 1-2 2H6.828…"] } },
//!   "glyphs": { "spark": { "paths": ["M12 2.7v18.6…"] } } }
//! ```
//!
//! - **A view is named by the window's own id** ([`RAIL_VIEWS`]). A name purlis does not have is
//!   said and left out, as an unknown token is: a newer purlis may have a view this one does not.
//! - **A glyph is named from [`GLYPHS`]** (#2922): the harness glyphs a profile names as its icon
//!   and a built-in harness is drawn with. They are held to the same rules as a view's symbol,
//!   and an unknown name is said and left out the same way. A set names views, glyphs or both.
//! - **A symbol is outlines on a 24 grid, stroked**: the window draws each path in the rail's
//!   own colour for its state, 2 wide with round caps and joins. None of that is a field, so a
//!   set can say only where its lines go.
//! - **A path is today's path grammar** (`app/src/theme/icons.ts`): it starts with a move, and
//!   holds path commands and numbers, nothing else, at most [`MOST_PATH`] long. A symbol has at
//!   least one path and at most [`MOST_PATHS`].
//! - **A set extends its parent's**: a view the theme does not draw is its parent's, and a view
//!   no theme draws is the window's own mark.
//!
//! Anything else refuses the theme whole (ADR 0097 §2).

use std::collections::BTreeMap;

use serde_json::Value;

use super::Complaint;
use super::vocabulary::{GLYPHS, RAIL_VIEWS};

/// The longest path a symbol may hold: the icon theme's own cap (`MOST_PATH` in `icons.ts`).
pub const MOST_PATH: usize = 16384;

/// The most paths one symbol may hold.
pub const MOST_PATHS: usize = 16;

/// The most views a set may name before purlis reads it: room for every view and for views a
/// newer purlis has. The same cap holds a set's glyphs.
const MOST_VIEWS: usize = 64;

/// The keys of a set's file.
const SET_KEYS: &[&str] = &["views", "glyphs"];

/// The keys of a symbol.
const SYMBOL_KEYS: &[&str] = &["paths"];

/// **An icon set**, each view named to its symbol. Empty for a theme that draws the window's own
/// marks, as Purlis Classic does.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct Icons {
    pub views: BTreeMap<&'static str, Symbol>,
    /// The harness glyphs it draws, by their name in [`GLYPHS`] (#2922).
    pub glyphs: BTreeMap<&'static str, Symbol>,
}

impl Icons {
    /// Whether the set draws no view and no glyph.
    pub fn is_empty(&self) -> bool {
        self.views.is_empty() && self.glyphs.is_empty()
    }

    /// This set over `parent`: each view and glyph this set draws, and every other one the
    /// parent draws.
    #[must_use]
    pub fn over(&self, parent: &Self) -> Self {
        let laid = |mine: &BTreeMap<&'static str, Symbol>,
                    theirs: &BTreeMap<&'static str, Symbol>| {
            let mut all = theirs.clone();
            all.extend(mine.iter().map(|(name, symbol)| (*name, symbol.clone())));
            all
        };
        Self {
            views: laid(&self.views, &parent.views),
            glyphs: laid(&self.glyphs, &parent.glyphs),
        }
    }
}

/// **One icon**: outlines on a 24 grid, each drawn stroked in the rail's colour for its state.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Symbol {
    pub paths: Vec<String>,
}

/// Whether `d` is path data in the grammar: a move first, then path commands, numbers and their
/// separators.
pub fn is_path(d: &str) -> bool {
    let mut bytes = d.bytes();
    d.len() <= MOST_PATH
        && bytes.next().is_some_and(|b| b == b'M' || b == b'm')
        && bytes.all(|b| b"MmZzLlHhVvCcSsQqTtAa0123456789eE.,+- \t\n\r".contains(&b))
}

/// **Reads a set's file**, `file` its path as the theme names it, saying what is wrong with any
/// of it in `complaints` and the view names it left out in `notes`.
pub(super) fn read(
    file: &str,
    document: &Value,
    complaints: &mut Vec<Complaint>,
    notes: &mut Vec<Complaint>,
) -> Icons {
    let mut icons = Icons::default();
    let Some(set) = document.as_object() else {
        complaints.push(Complaint::said(format!(
            "{file} is not an icon set: a set is a JSON object"
        )));
        return icons;
    };
    for key in set.keys() {
        if !SET_KEYS.contains(&key.as_str()) {
            complaints.push(Complaint::said(format!(
                "{key} is not a key of an icon set ({file})"
            )));
        }
    }
    let (views, glyphs) = (set.get("views"), set.get("glyphs"));
    if views.is_none() && glyphs.is_none() {
        complaints.push(Complaint::said(format!("{file} names no views or glyphs")));
        return icons;
    }
    if let Some(views) = views {
        icons.views = named(file, views, Names::VIEWS, complaints, notes);
    }
    if let Some(glyphs) = glyphs {
        icons.glyphs = named(file, glyphs, Names::GLYPHS, complaints, notes);
    }
    icons
}

/// One of a set's two maps, and the words its complaints say it in.
struct Names {
    /// The key of the set's file.
    key: &'static str,
    /// Every name the map may hold.
    known: &'static [&'static str],
    /// What one of them is, in a complaint: `view`.
    one: &'static str,
    /// What the left-out note says a name is not.
    not_a: &'static str,
}

impl Names {
    const VIEWS: Self = Self {
        key: "views",
        known: RAIL_VIEWS,
        one: "view",
        not_a: "purlis view",
    };
    const GLYPHS: Self = Self {
        key: "glyphs",
        known: GLYPHS,
        one: "glyph",
        not_a: "purlis glyph",
    };
}

/// The symbols of one of a set's maps, `raw` as the file holds it, each name held to `names`.
fn named(
    file: &str,
    raw: &Value,
    names: Names,
    complaints: &mut Vec<Complaint>,
    notes: &mut Vec<Complaint>,
) -> BTreeMap<&'static str, Symbol> {
    let mut drawn = BTreeMap::new();
    let Names {
        key,
        known,
        one,
        not_a,
    } = names;
    let Some(raw) = raw.as_object() else {
        complaints.push(Complaint::said(format!(
            "{key} in {file} is an object of {one} names to icons"
        )));
        return drawn;
    };
    // The cap is held before anything is read, so a set of any size costs one count.
    if raw.len() > MOST_VIEWS {
        complaints.push(Complaint::said(format!(
            "{file} names {} {key}, and a set names at most {MOST_VIEWS}",
            raw.len()
        )));
        return drawn;
    }
    for (name, symbol_raw) in raw {
        let Some(known) = known.iter().copied().find(|it| it == name) else {
            notes.push(Complaint::said(format!(
                "{name} in {file} is not a {not_a}, so it is left out"
            )));
            continue;
        };
        if let Some(symbol) = symbol(file, known, symbol_raw, complaints) {
            drawn.insert(known, symbol);
        }
    }
    drawn
}

/// The symbol `raw` for `view`, or why not in `complaints`.
fn symbol(file: &str, view: &str, raw: &Value, complaints: &mut Vec<Complaint>) -> Option<Symbol> {
    let Some(one) = raw.as_object() else {
        complaints.push(Complaint::said(format!(
            "{view} in {file} is not an icon: an icon is an object with its paths"
        )));
        return None;
    };
    let before = complaints.len();
    for key in one.keys() {
        if !SYMBOL_KEYS.contains(&key.as_str()) {
            complaints.push(Complaint::said(format!(
                "{key} is not a key of an icon ({view} in {file})"
            )));
        }
    }
    let given: &[Value] = match one.get("paths") {
        Some(Value::Array(given)) if !given.is_empty() && given.len() <= MOST_PATHS => given,
        _ => {
            complaints.push(Complaint::said(format!(
                "paths of {view} in {file} is a list of 1 to {MOST_PATHS} paths"
            )));
            &[]
        }
    };
    let mut paths = Vec::with_capacity(given.len());
    for (at, d) in given.iter().enumerate() {
        match d.as_str() {
            Some(d) if is_path(d) => paths.push(d.to_owned()),
            Some(d) if d.len() > MOST_PATH => complaints.push(Complaint::said(format!(
                "path {} of {view} in {file} is {} long, and a path is at most {MOST_PATH}",
                at + 1,
                d.len()
            ))),
            _ => complaints.push(Complaint::said(format!(
                "path {} of {view} in {file} is not path data: a move, then path commands and \
                 numbers",
                at + 1
            ))),
        }
    }
    if complaints.len() != before {
        return None;
    }
    Some(Symbol { paths })
}

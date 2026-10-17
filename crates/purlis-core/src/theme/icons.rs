// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A theme's icon set** (spec #2313, #2337): what each view's tab on the rails is drawn as.
//!
//! A theme folder's `theme.json` names its set by a path inside the folder (`"icons":
//! "icons/icons.json"`), and the file is data, never SVG:
//!
//! ```json
//! { "views": { "chats": { "paths": ["M22 17a2 2 0 0 1-2 2H6.828…"] } } }
//! ```
//!
//! - **A view is named by the window's own id** ([`RAIL_VIEWS`]). A name purlis does not have is
//!   said and left out, as an unknown token is: a newer purlis may have a view this one does not.
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
use super::vocabulary::RAIL_VIEWS;

/// The longest path a symbol may hold: the icon theme's own cap (`MOST_PATH` in `icons.ts`).
pub const MOST_PATH: usize = 16384;

/// The most paths one symbol may hold.
pub const MOST_PATHS: usize = 16;

/// The most views a set may name before purlis reads it: room for every view and for views a
/// newer purlis has.
const MOST_VIEWS: usize = 64;

/// The keys of a set's file.
const SET_KEYS: &[&str] = &["views"];

/// The keys of a symbol.
const SYMBOL_KEYS: &[&str] = &["paths"];

/// **An icon set**, each view named to its symbol. Empty for a theme that draws the window's own
/// marks, as Purlis Classic does.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct Icons {
    pub views: BTreeMap<&'static str, Symbol>,
}

impl Icons {
    /// Whether the set draws no view.
    pub fn is_empty(&self) -> bool {
        self.views.is_empty()
    }

    /// This set over `parent`: each view this set draws, and every other view the parent draws.
    #[must_use]
    pub fn over(&self, parent: &Self) -> Self {
        let mut views = parent.views.clone();
        views.extend(
            self.views
                .iter()
                .map(|(view, symbol)| (*view, symbol.clone())),
        );
        Self { views }
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
    let views = match set.get("views") {
        Some(Value::Object(views)) => views,
        None => {
            complaints.push(Complaint::said(format!("{file} names no views")));
            return icons;
        }
        Some(_) => {
            complaints.push(Complaint::said(format!(
                "views in {file} is an object of view names to icons"
            )));
            return icons;
        }
    };
    // The cap is held before anything is read, so a set of any size costs one count.
    if views.len() > MOST_VIEWS {
        complaints.push(Complaint::said(format!(
            "{file} names {} views, and a set names at most {MOST_VIEWS}",
            views.len()
        )));
        return icons;
    }
    for (view, raw) in views {
        let Some(known) = RAIL_VIEWS.iter().copied().find(|it| it == view) else {
            notes.push(Complaint::said(format!(
                "{view} in {file} is not a purlis view, so it is left out"
            )));
            continue;
        };
        if let Some(symbol) = symbol(file, known, raw, complaints) {
            icons.views.insert(known, symbol);
        }
    }
    icons
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

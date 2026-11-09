// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The project's settings file, merged by its structure** (#5824 F5, F6).
//!
//! **List-shaped settings are sets** (F5, D4): the keys that hold a list every machine adds to,
//! such as the hosts each Always allow adds, are written one item per line, sorted and once each
//! ([`as_set`]), and two machines' changes to them are merged as sets: both sides' items, less
//! any either side removed since the two parted. [`SETS`] is the one list of which keys those
//! are.
//!
//! **Every other key merges by itself** (F6): a key only one side changed (added, removed or
//! given a new value) takes that side, written as that side wrote it; a table both sides changed
//! is merged key by key inside it; and a key both changed, each its own way, is a conflict,
//! named by its dotted path for the person to settle ([`merged`]).
//!
//! Everything is done on the parsed TOML, with `toml_edit` keeping this machine's comments and
//! layout, so a merge never writes a conflict marker into the file.

use std::collections::BTreeSet;

/// **The keys whose lists are sets**, as paths of keys from the top of the settings file; `*` is
/// any one key. Each holds strings, added to by a grant on one machine and removed by a revoke on
/// another; their order means nothing.
pub const SETS: &[&[&str]] = &[
    // The project's hosts (`sandbox::hosts::KEY`): every Always allow appends one.
    &["sandbox", "hosts"],
    // The project's browser sites (`sandbox::browser_sites::KEY`).
    &["sandbox", "browser-sites"],
    // A persona's own hosts (`sandbox::persona::KEY`).
    &["sandbox", "personas", "*", "hosts"],
    // Not `[dispatch.grants]`: a grant there is a name or a limited grant's table, read in order.
];

/// Whether the key at `path` holds a set ([`SETS`]).
pub fn is_set<S: AsRef<str>>(path: &[S]) -> bool {
    SETS.iter().any(|set| {
        set.len() == path.len()
            && set
                .iter()
                .zip(path)
                .all(|(want, key)| *want == "*" || *want == key.as_ref())
    })
}

/// **`list` written as a set**: its strings sorted and once each, one per line, each line's own
/// comment kept. A list holding anything but strings is only laid out one per line.
pub fn as_set(list: &mut toml_edit::Array) {
    let mut items: Vec<toml_edit::Value> = list.iter().cloned().collect();
    if items.iter().all(|item| item.as_str().is_some()) {
        items.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
        items.dedup_by(|a, b| a.as_str() == b.as_str());
    }
    let trailing = comments_in(list.trailing().as_str().unwrap_or_default());
    list.clear();
    for mut item in items {
        let above = comments_in(item.decor().prefix().and_then(|p| p.as_str()).unwrap_or(""));
        item.decor_mut().set_prefix(format!("\n{above}    "));
        item.decor_mut().set_suffix("");
        list.push_formatted(item);
    }
    list.set_trailing_comma(!list.is_empty());
    list.set_trailing(if list.is_empty() {
        trailing
    } else {
        format!("\n{trailing}")
    });
}

/// The comment lines in a run of whitespace and comments, each indented and ended.
fn comments_in(raw: &str) -> String {
    raw.lines()
        .map(str::trim)
        .filter(|line| line.starts_with('#'))
        .map(|line| format!("    {line}\n"))
        .collect()
}

/// What merging two sides of the settings file came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Merged {
    /// The merged text: this machine's, with the other's changes in.
    Clean(String),
    /// Keys both sides changed, each its own way, as dotted paths: a person's to settle.
    Conflict(Vec<String>),
}

/// **Merge `ours` and `theirs`, two changes of `base`**, key by key (#5824 F6). A set's lists
/// merge as sets ([`SETS`]). Any other key only one side changed (added, removed or given a new
/// value) takes that side, written as that side wrote it; a table both changed is merged key by
/// key within it; and a key both changed, each its own way, is a conflict, named. Values are
/// compared as TOML reads them, so a change of spacing alone is no change. `Err` where a side is
/// not TOML.
pub fn merged(base: &str, ours: &str, theirs: &str) -> Result<Merged, String> {
    let read = |side: &str, text: &str| {
        text.parse::<toml::Table>()
            .map(toml::Value::Table)
            .map_err(|e| not_toml(side, e.message()))
    };
    let (base, ours_read, theirs_read) = (
        read("the merge base", base)?,
        read("this machine's", ours)?,
        read("the remote's", theirs)?,
    );
    let mut found = Found::default();
    walk(
        &mut Vec::new(),
        Sides {
            base: Some(&base),
            ours: Some(&ours_read),
            theirs: Some(&theirs_read),
        },
        &mut found,
    );
    if !found.conflicts.is_empty() {
        return Ok(Merged::Conflict(found.conflicts));
    }
    let document = |side: &str, text: &str| {
        text.parse::<toml_edit::DocumentMut>()
            .map_err(|e| not_toml(side, e.message()))
    };
    let mut doc = document("this machine's", ours)?;
    let from = document("the remote's", theirs)?;
    for edit in found.edits {
        apply(&mut doc, &from, edit)?;
    }
    Ok(Merged::Clean(doc.to_string()))
}

/// A key both sides changed, each its own way (#5852), with each side's value as TOML writes
/// it; `None` where that side removed the key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyConflict {
    /// The key's dotted path.
    pub key: String,
    /// This machine's value.
    pub ours: Option<String>,
    /// The remote's value.
    pub theirs: Option<String>,
}

/// **The settings file merged around the keys in conflict** (#5852): everything [`merged`]
/// settles by itself settled, and this machine's value standing in for each key both sides
/// changed, which are listed with both values. Never a conflict marker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Around {
    pub text: String,
    pub keys: Vec<KeyConflict>,
}

/// [`merged`], settled around the keys in conflict ([`Around`]).
pub fn around(base: &str, ours: &str, theirs: &str) -> Result<Around, String> {
    let (doc, from, found) = settled_around(base, ours, theirs)?;
    let value_at = |doc: &toml_edit::DocumentMut, path: &[String]| {
        let (key, parents) = path.split_last()?;
        let mut table: &dyn toml_edit::TableLike = doc.as_table();
        for parent in parents {
            table = table.get(parent)?.as_table_like()?;
        }
        let item = table.get(key)?;
        Some(match item {
            toml_edit::Item::Value(value) => value.to_string().trim().to_owned(),
            other => other.to_string().trim().to_owned(),
        })
    };
    let keys = found
        .conflict_paths
        .iter()
        .map(|path| KeyConflict {
            key: dotted(path),
            ours: value_at(&doc, path),
            theirs: value_at(&from, path),
        })
        .collect();
    Ok(Around {
        text: doc.to_string(),
        keys,
    })
}

/// **The settings file once the person chose** (#5852): [`around`], with the remote's value
/// taken for each key named in `theirs_for` (dotted paths), and this machine's kept for the
/// rest.
pub fn chosen(
    base: &str,
    ours: &str,
    theirs: &str,
    theirs_for: &[String],
) -> Result<String, String> {
    let (mut doc, from, found) = settled_around(base, ours, theirs)?;
    for path in found.conflict_paths {
        if theirs_for.contains(&dotted(&path)) {
            apply(&mut doc, &from, Edit::Theirs(path))?;
        }
    }
    Ok(doc.to_string())
}

/// This machine's document with every change [`walk`] found made, the remote's document, and
/// what was found, conflicts included.
fn settled_around(
    base: &str,
    ours: &str,
    theirs: &str,
) -> Result<(toml_edit::DocumentMut, toml_edit::DocumentMut, Found), String> {
    let read = |side: &str, text: &str| {
        text.parse::<toml::Table>()
            .map(toml::Value::Table)
            .map_err(|e| not_toml(side, e.message()))
    };
    let (base_read, ours_read, theirs_read) = (
        read("the merge base", base)?,
        read("this machine's", ours)?,
        read("the remote's", theirs)?,
    );
    let mut found = Found::default();
    walk(
        &mut Vec::new(),
        Sides {
            base: Some(&base_read),
            ours: Some(&ours_read),
            theirs: Some(&theirs_read),
        },
        &mut found,
    );
    let document = |side: &str, text: &str| {
        text.parse::<toml_edit::DocumentMut>()
            .map_err(|e| not_toml(side, e.message()))
    };
    let mut doc = document("this machine's", ours)?;
    let from = document("the remote's", theirs)?;
    for edit in std::mem::take(&mut found.edits) {
        apply(&mut doc, &from, edit)?;
    }
    Ok((doc, from, found))
}

/// Why a side of the merge could not be read: `side` is not TOML, in the parser's words.
fn not_toml(side: &str, message: &str) -> String {
    format!("{side} is not TOML: {}", crate::shown::short(message))
}

/// A change [`walk`] found the merge needs in this machine's document.
enum Edit {
    /// The set at `path` is now `items`.
    Set {
        path: Vec<String>,
        items: Vec<String>,
    },
    /// The key at this path is the remote's, as it wrote it, or gone where it has none.
    Theirs(Vec<String>),
}

/// What [`walk`] found: the changes the merge makes in this machine's document, and the keys
/// both sides changed, each its own way.
#[derive(Default)]
struct Found {
    edits: Vec<Edit>,
    conflicts: Vec<String>,
    /// The keys of [`Found::conflicts`], as paths.
    conflict_paths: Vec<Vec<String>>,
}

/// One key's value on each side of the merge, `None` where that side has no such key.
#[derive(Clone, Copy)]
struct Sides<'a> {
    base: Option<&'a toml::Value>,
    ours: Option<&'a toml::Value>,
    theirs: Option<&'a toml::Value>,
}

/// The strings of a list, `None` where `value` is not a list of strings. An absent key is an
/// empty set.
fn strings(value: Option<&toml::Value>) -> Option<Vec<String>> {
    match value {
        None => Some(Vec::new()),
        Some(toml::Value::Array(items)) => items
            .iter()
            .map(|item| item.as_str().map(str::to_owned))
            .collect(),
        Some(_) => None,
    }
}

/// Walk the three sides at `path`, noting in `found` what the merge changes in this machine's
/// document and where it conflicts.
fn walk(path: &mut Vec<String>, sides: Sides<'_>, found: &mut Found) {
    let Sides { base, ours, theirs } = sides;
    if is_set(path)
        && let (Some(was), Some(mine), Some(remote)) =
            (strings(base), strings(ours), strings(theirs))
    {
        let was: BTreeSet<String> = was.into_iter().collect();
        let mine: BTreeSet<String> = mine.into_iter().collect();
        let remote: BTreeSet<String> = remote.into_iter().collect();
        let gone: BTreeSet<&String> = was
            .iter()
            .filter(|item| !mine.contains(*item) || !remote.contains(*item))
            .collect();
        let set: BTreeSet<String> = mine
            .union(&remote)
            .filter(|item| !gone.contains(item))
            .cloned()
            .collect();
        if set != mine {
            found.edits.push(Edit::Set {
                path: path.clone(),
                items: set.into_iter().collect(),
            });
        }
        return;
    }
    if ours == theirs || theirs == base {
        return;
    }
    if ours == base {
        found.edits.push(Edit::Theirs(path.clone()));
        return;
    }
    if let (Some(toml::Value::Table(mine)), Some(toml::Value::Table(remote))) = (ours, theirs) {
        let was = match base {
            Some(toml::Value::Table(was)) => Some(was),
            _ => None,
        };
        let keys: BTreeSet<&String> = mine
            .keys()
            .chain(remote.keys())
            .chain(was.into_iter().flat_map(|was| was.keys()))
            .collect();
        for key in keys {
            path.push(key.clone());
            let below = Sides {
                base: was.and_then(|was| was.get(key)),
                ours: mine.get(key),
                theirs: remote.get(key),
            };
            walk(path, below, found);
            path.pop();
        }
        return;
    }
    found.conflicts.push(dotted(path));
    found.conflict_paths.push(path.clone());
}

/// A key's path as TOML writes it, dotted, each key quoted where it has to be.
fn dotted(path: &[String]) -> String {
    path.iter()
        .map(|key| {
            toml_edit::Key::new(key.as_str())
                .display_repr()
                .into_owned()
        })
        .collect::<Vec<_>>()
        .join(".")
}

/// `table` and the tables below it, with the place each had in the remote's file forgotten.
fn unplace(table: &mut toml_edit::Table) {
    table.set_position(None);
    for (_, item) in table.iter_mut() {
        if let toml_edit::Item::Table(below) = item {
            unplace(below);
        }
    }
}

/// The table at `parents` in `doc`, and whether it is an inline table. It is there: [`walk`]
/// only goes into a table both sides have.
fn parent_of<'d>(
    doc: &'d mut toml_edit::DocumentMut,
    parents: &[String],
) -> Result<(&'d mut dyn toml_edit::TableLike, bool), String> {
    let mut table: &mut dyn toml_edit::TableLike = doc.as_table_mut();
    let mut inline = false;
    for parent in parents {
        let item = table
            .get_mut(parent)
            .ok_or_else(|| format!("{} is not a table here", dotted(parents)))?;
        inline = inline || item.is_inline_table();
        table = item
            .as_table_like_mut()
            .ok_or_else(|| format!("{} is not a table here", dotted(parents)))?;
    }
    Ok((table, inline))
}

/// Make `edit` in `doc`, taking what the remote wrote from `from`.
fn apply(
    doc: &mut toml_edit::DocumentMut,
    from: &toml_edit::DocumentMut,
    edit: Edit,
) -> Result<(), String> {
    match edit {
        Edit::Theirs(path) => {
            let (key, parents) = path.split_last().ok_or("a key with no name")?;
            let mut theirs: Option<&dyn toml_edit::TableLike> = Some(from.as_table());
            for parent in parents {
                theirs = theirs
                    .and_then(|table| table.get(parent))
                    .and_then(toml_edit::Item::as_table_like);
            }
            let item = theirs.and_then(|table| table.get(key)).cloned();
            let (table, inline) = parent_of(doc, parents)?;
            match item {
                Some(mut item) => {
                    // A table the remote added goes after this machine's, not where it stood there.
                    if let toml_edit::Item::Table(added) = &mut item {
                        unplace(added);
                    }
                    // Inside an inline table, a `[header]` table cannot stand: it goes in as the
                    // inline table it reads as, never left to the table to drop or refuse.
                    if inline && !item.is_value() {
                        item =
                            toml_edit::Item::Value(item.into_value().map_err(|_| {
                                format!("{} cannot be written inline", dotted(&path))
                            })?);
                    }
                    table.insert(key, item);
                }
                None => {
                    table.remove(key);
                }
            }
        }
        Edit::Set { path, items } => {
            let (key, parents) = path.split_last().ok_or("a set with no key")?;
            let (table, _) = parent_of(doc, parents)?;
            if items.is_empty() {
                table.remove(key);
                return Ok(());
            }
            let list = table
                .entry(key)
                .or_insert(toml_edit::value(toml_edit::Array::new()));
            let list = list
                .as_array_mut()
                .ok_or_else(|| format!("{} is not a list", dotted(&path)))?;
            // This machine's items stay as written, a comment above one included.
            list.retain(|item| item.as_str().is_some_and(|s| items.iter().any(|x| x == s)));
            let kept: BTreeSet<String> = list
                .iter()
                .filter_map(|item| item.as_str().map(str::to_owned))
                .collect();
            for item in items.iter().filter(|item| !kept.contains(*item)) {
                list.push(item.as_str());
            }
            as_set(list);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;

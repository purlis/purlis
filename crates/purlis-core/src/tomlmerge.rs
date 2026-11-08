// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **List-shaped settings are sets** (#5824 F5, D4): the keys of `charter.toml` that hold a list
//! every machine adds to, such as the hosts each Always allow appends, are written one item per
//! line, sorted and once each ([`as_set`]), and two machines' changes to them are merged as sets
//! ([`merged`]): both sides' items, less any either side removed since the two parted.
//!
//! [`SETS`] is the one list of which keys those are. A key not in it is merged as a whole value.
//!
//! Everything is done on the parsed TOML, with `toml_edit` keeping this machine's comments and
//! layout, so a merge never writes a conflict marker into the file.

use std::collections::BTreeSet;

/// **The keys whose lists are sets**, as paths of keys from the top of `charter.toml`; `*` is
/// any one key. Each holds strings, added to by a grant on one machine and removed by a revoke on
/// another; their order means nothing.
pub const SETS: &[&[&str]] = &[
    // The project's hosts (`sandbox::hosts::KEY`): every Always allow appends one.
    &["sandbox", "hosts"],
    // The project's browser sites (`sandbox::browser_sites::KEY`).
    &["sandbox", "browser-sites"],
    // A persona's own hosts (`sandbox::persona::KEY`).
    &["sandbox", "personas", "*", "hosts"],
    // Who may dispatch to whom (`dispatchgrant::KEY`): asking persona to its targets.
    &["dispatch", "grants", "*"],
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

/// What merging two sides of `charter.toml` came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Merged {
    /// The merged text: this machine's, with the other's changes in.
    Clean(String),
    /// Keys both sides changed, each its own way, as dotted paths: a person's to settle.
    Conflict(Vec<String>),
}

/// **Merge `ours` and `theirs`, two changes of `base`**, key by key. A set's lists merge as sets
/// ([`SETS`]); any other key both sides changed is a conflict. `Err` where a side is not TOML.
pub fn merged(base: &str, ours: &str, theirs: &str) -> Result<Merged, String> {
    let parsed = |side: &str, text: &str| {
        text.parse::<toml::Table>()
            .map(toml::Value::Table)
            .map_err(|e| format!("{side} is not TOML: {}", crate::shown::short(e.message())))
    };
    let (b, o, t) = (
        parsed("the merge base", base)?,
        parsed("this machine's", ours)?,
        parsed("the remote's", theirs)?,
    );
    let mut edits = Vec::new();
    let mut conflicts = Vec::new();
    walk(
        &mut Vec::new(),
        Some(&b),
        Some(&o),
        Some(&t),
        &mut edits,
        &mut conflicts,
    );
    if !conflicts.is_empty() {
        return Ok(Merged::Conflict(conflicts));
    }
    let mut doc: toml_edit::DocumentMut = ours.parse().map_err(|e: toml_edit::TomlError| {
        format!(
            "this machine's is not TOML: {}",
            crate::shown::short(e.message())
        )
    })?;
    for edit in edits {
        apply(&mut doc, edit)?;
    }
    Ok(Merged::Clean(doc.to_string()))
}

/// A change [`walk`] found the merge needs in this machine's document.
enum Edit {
    /// The set at this path is now these items.
    Set(Vec<String>, Vec<String>),
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

/// Walk the three sides at `path`, noting what the merge changes in ours and where it conflicts.
fn walk(
    path: &mut Vec<String>,
    b: Option<&toml::Value>,
    o: Option<&toml::Value>,
    t: Option<&toml::Value>,
    edits: &mut Vec<Edit>,
    conflicts: &mut Vec<String>,
) {
    if is_set(path)
        && let (Some(b), Some(o), Some(t)) = (strings(b), strings(o), strings(t))
    {
        let (b, o_set, t): (BTreeSet<String>, BTreeSet<String>, BTreeSet<String>) = (
            b.into_iter().collect(),
            o.iter().cloned().collect(),
            t.into_iter().collect(),
        );
        let gone: BTreeSet<&String> = b
            .iter()
            .filter(|x| !o_set.contains(*x) || !t.contains(*x))
            .collect();
        let set: BTreeSet<String> = o_set
            .union(&t)
            .filter(|x| !gone.contains(x))
            .cloned()
            .collect();
        if set != o_set {
            edits.push(Edit::Set(path.clone(), set.into_iter().collect()));
        }
        return;
    }
    if o == t {
        return;
    }
    if let (Some(toml::Value::Table(o)), Some(toml::Value::Table(t))) = (o, t) {
        let b = match b {
            Some(toml::Value::Table(b)) => Some(b),
            _ => None,
        };
        let keys: BTreeSet<&String> = o
            .keys()
            .chain(t.keys())
            .chain(b.into_iter().flat_map(|b| b.keys()))
            .collect();
        for key in keys {
            path.push(key.clone());
            walk(
                path,
                b.and_then(|b| b.get(key)),
                o.get(key),
                t.get(key),
                edits,
                conflicts,
            );
            path.pop();
        }
        return;
    }
    conflicts.push(dotted(path));
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

/// Make `edit` in `doc`.
fn apply(doc: &mut toml_edit::DocumentMut, edit: Edit) -> Result<(), String> {
    match edit {
        Edit::Set(path, items) => {
            let (key, parents) = path.split_last().ok_or("a set with no key")?;
            let mut table: &mut dyn toml_edit::TableLike = doc.as_table_mut();
            for parent in parents {
                table = table
                    .entry(parent)
                    .or_insert_with(|| toml_edit::Item::Table(toml_edit::Table::new()))
                    .as_table_like_mut()
                    .ok_or_else(|| format!("{} is not a table", dotted(parents)))?;
            }
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
#[path = "tomlmerge_tests.rs"]
mod tests;

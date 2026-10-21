// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What a skill declares about how it is used** (SK-12, #2775; decisions 19 and 61): whether
//! it is manual-only, and the arguments it takes, read from the standard frontmatter keys every
//! harness that reads them shares.
//!
//! - `disable-model-invocation: true` makes a skill **manual-only**: no harness picks it by
//!   itself, it runs only when asked for by name, and its description stays out of the chat's
//!   listing, so it costs nothing in the skill budget ([`crate::budget`]).
//! - `arguments` names its arguments, in order: a space-separated string (`arguments: pr
//!   focus`), a flow list (`arguments: [pr, focus]`) or a block list (`arguments:` then one
//!   `- pr` line each).
//! - `argument-hint` is the one line that says what to pass, where no names are declared.
//!
//! purlis adds no key of its own (ADR 0099 §1), and nothing here knows a harness: how the
//! arguments are passed is each harness adapter's.

use std::path::Path;

use crate::Skill;

/// What a skill declares about how it is used.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Declared {
    /// Whether it is manual-only: run only when asked for by name.
    pub manual: bool,
    /// The arguments it takes, by name, in the order it takes them.
    pub arguments: Vec<String>,
    /// What to pass it, in one line, where it says.
    pub hint: Option<String>,
}

impl Declared {
    /// Whether it asks for anything before it runs: named arguments, or a hint at what to pass.
    pub fn takes_arguments(&self) -> bool {
        !self.arguments.is_empty() || self.hint.is_some()
    }
}

/// **What a person gave a skill's arguments** in Use skill… (decision 61), each value on one
/// line: what a harness adapter passes in its own syntax.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Given {
    /// It takes none, or none was given.
    #[default]
    Nothing,
    /// A value for each argument it names, in its order. A value left empty is passed empty
    /// where a later one is given, so each keeps its place, and not at all at the end.
    Named(Vec<(String, String)>),
    /// The one line given for a skill that names no arguments and says what to pass.
    Raw(String),
}

impl Given {
    /// **What `values` give a skill that declares `declared`**: one value per named argument, in
    /// its order, or the first value as the one line for a skill that names none. Each value on
    /// one line, its white space collapsed, so nothing typed can send it. Empty values at the
    /// end are not given.
    pub fn of(declared: &Declared, values: &[String]) -> Given {
        let line = |value: &str| value.split_whitespace().collect::<Vec<_>>().join(" ");
        if declared.arguments.is_empty() {
            return match values.first().map(|value| line(value)) {
                Some(raw) if !raw.is_empty() && declared.hint.is_some() => Given::Raw(raw),
                _ => Given::Nothing,
            };
        }
        let mut named: Vec<(String, String)> = declared
            .arguments
            .iter()
            .enumerate()
            .map(|(at, name)| {
                let value = values.get(at).map(|value| line(value)).unwrap_or_default();
                (name.clone(), value)
            })
            .collect();
        while named.last().is_some_and(|(_, value)| value.is_empty()) {
            named.pop();
        }
        if named.is_empty() {
            Given::Nothing
        } else {
            Given::Named(named)
        }
    }
}

/// The key that makes a skill manual-only.
const MANUAL: &str = "disable-model-invocation";
/// The key that names a skill's arguments.
const ARGUMENTS: &str = "arguments";
/// The key that says what to pass a skill.
const HINT: &str = "argument-hint";

/// What `skill` declares, read from its `SKILL.md`: nothing for one that cannot be read.
pub fn declared(skill: &Skill) -> Declared {
    declared_at(&skill.path)
}

/// What the `SKILL.md` at `path` declares: nothing for one that cannot be read.
pub fn declared_at(path: &Path) -> Declared {
    std::fs::read_to_string(path)
        .map(|text| declared_in(&text))
        .unwrap_or_default()
}

/// What a `SKILL.md`'s text declares, read by the one frontmatter rule ([`crate::frontmatter`]),
/// and a block list of arguments from the lines under its key.
pub fn declared_in(text: &str) -> Declared {
    let pairs = crate::frontmatter(text);
    let value = |key: &str| {
        pairs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    };
    let manual = value(MANUAL).is_some_and(|v| truthy(unquoted(v)));
    let arguments = match value(ARGUMENTS).map(unquoted) {
        Some("") => block_list(text, ARGUMENTS),
        Some(listed) => inline_list(listed),
        None => Vec::new(),
    };
    let hint = value(HINT)
        .map(unquoted)
        .filter(|hint| !hint.is_empty())
        .map(str::to_owned);
    Declared {
        manual,
        arguments,
        hint,
    }
}

/// Whether a frontmatter boolean says yes: `true`, `yes`, `on` or `1`, in any case.
fn truthy(value: &str) -> bool {
    ["true", "yes", "on", "1"]
        .iter()
        .any(|yes| value.eq_ignore_ascii_case(yes))
}

/// `value` without one pair of quotes around it.
fn unquoted(value: &str) -> &str {
    let value = value.trim();
    for quote in ['"', '\''] {
        if let Some(inner) = value
            .strip_prefix(quote)
            .and_then(|rest| rest.strip_suffix(quote))
        {
            return inner.trim();
        }
    }
    value
}

/// The names of a one-line list: `[a, b]` or `a b`.
fn inline_list(value: &str) -> Vec<String> {
    let inner = value
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'));
    let names: Vec<&str> = match inner {
        Some(inner) => inner.split(',').collect(),
        None => value.split_whitespace().collect(),
    };
    names
        .into_iter()
        .map(unquoted)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect()
}

/// The names of the block list under `key` in the frontmatter: each `- name` line right after
/// the key's own line.
fn block_list(text: &str, key: &str) -> Vec<String> {
    let Some(rest) = text.strip_prefix("---") else {
        return Vec::new();
    };
    let block = rest.find("---").map_or(rest, |end| &rest[..end]);
    let mut lines = block.lines();
    let found = lines.by_ref().any(|line| {
        line.split_once(':')
            .is_some_and(|(k, v)| k.trim() == key && v.trim().is_empty())
    });
    if !found {
        return Vec::new();
    }
    lines
        .map(str::trim)
        .take_while(|line| line.starts_with('-'))
        .map(|line| unquoted(line.trim_start_matches('-')))
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
mod tests;

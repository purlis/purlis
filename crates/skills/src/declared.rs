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
    /// Its `when_to_use`, which Claude Code joins to its description in its listing.
    pub when_to_use: Option<String>,
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
    Named(Vec<Argument>),
    /// The one line given for a skill that names no arguments and says what to pass.
    Raw(String),
}

/// One named argument and the value given it, on one line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Argument {
    /// Its name, as the skill declares it.
    pub name: String,
    /// What was given it, possibly empty.
    pub value: String,
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
        let mut named: Vec<Argument> = declared
            .arguments
            .iter()
            .enumerate()
            .map(|(at, name)| Argument {
                name: name.clone(),
                value: values.get(at).map(|value| line(value)).unwrap_or_default(),
            })
            .collect();
        while named.last().is_some_and(|one| one.value.is_empty()) {
            named.pop();
        }
        if named.is_empty() {
            Given::Nothing
        } else {
            Given::Named(named)
        }
    }

    /// **What was given, in plain words**: `name: value; …` with `none` for a value left empty,
    /// the one line as it was given, or `None` for nothing. What a harness that substitutes no
    /// arguments is passed, and what a skill started through `skill_load` is (SK-15).
    pub fn in_words(&self) -> Option<String> {
        match self {
            Given::Nothing => None,
            Given::Raw(line) => Some(line.clone()),
            Given::Named(values) => Some(
                values
                    .iter()
                    .map(|one| {
                        let value = if one.value.is_empty() {
                            "none"
                        } else {
                            &one.value
                        };
                        format!("{}: {value}", one.name)
                    })
                    .collect::<Vec<_>>()
                    .join("; "),
            ),
        }
    }
}

/// The key that makes a skill manual-only.
const MANUAL: &str = "disable-model-invocation";
/// The key that names a skill's arguments.
const ARGUMENTS: &str = "arguments";
/// The key that says what to pass a skill.
const HINT: &str = "argument-hint";
/// The key that adds to when a skill applies.
const WHEN_TO_USE: &str = "when_to_use";

/// What `skill` declares, read from its `SKILL.md`: nothing for one that cannot be read.
pub fn declared(skill: &Skill) -> Declared {
    declared_at(&skill.path)
}

/// What the `SKILL.md` at `path` declares: nothing for one that cannot be read.
pub(crate) fn declared_at(path: &Path) -> Declared {
    std::fs::read_to_string(path)
        .map(|text| declared_in(&text))
        .unwrap_or_default()
}

/// What a `SKILL.md`'s text declares, read by the one frontmatter rule ([`crate::frontmatter`]),
/// and a block list of arguments from the frontmatter's lines under its key.
pub(crate) fn declared_in(text: &str) -> Declared {
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
    let when_to_use = value(WHEN_TO_USE)
        .map(unquoted)
        .map(|when| {
            if when.starts_with(['>', '|']) {
                block_text(text, WHEN_TO_USE)
            } else {
                when.to_owned()
            }
        })
        .filter(|when| !when.is_empty());
    Declared {
        manual,
        arguments,
        hint,
        when_to_use,
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
/// the key's own line, read from the frontmatter's lines ([`crate::read::frontmatter_lines`]).
fn block_list(text: &str, key: &str) -> Vec<String> {
    let lines = crate::read::frontmatter_lines(text);
    let Some(at) = lines.iter().position(|line| {
        line.split_once(':')
            .is_some_and(|(k, v)| k.trim() == key && v.trim().is_empty())
    }) else {
        return Vec::new();
    };
    lines[at + 1..]
        .iter()
        .map(|line| line.trim())
        .take_while(|line| line.starts_with('-'))
        .map(|line| unquoted(line.trim_start_matches('-')))
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect()
}

/// The indented lines under `key`, a YAML block scalar, joined by spaces.
fn block_text(text: &str, key: &str) -> String {
    let lines = crate::read::frontmatter_lines(text);
    let Some(at) = lines
        .iter()
        .position(|line| line.split_once(':').is_some_and(|(k, _)| k.trim() == key))
    else {
        return String::new();
    };
    lines[at + 1..]
        .iter()
        .take_while(|line| line.starts_with([' ', '\t']))
        .map(|line| line.trim())
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests;

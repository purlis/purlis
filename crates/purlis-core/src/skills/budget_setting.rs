// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **`settings.skills.budget`** (#4610; ADR 0102 as amended 2026-11-02): the skill budget a
//! project or a workspace writes over its harnesses' numbers.
//!
//! `{ skills = 80, chars = 20000 }`, either key or both. The project's is `[skills.budget]` in
//! `charter.toml`, and a workspace's `settings.skills.budget` in its `workspace.json` wins for
//! each key it writes. A number not written stays the adapter's, on each harness.
//!
//! **A value past a harness's own limit is said, never refused** ([`warnings`]). Past the budget
//! the router still hides what it may, and pins stay the way to keep one skill listed.

use std::path::Path;

use purlis_skills::budget::Set;

use crate::harness::Harness;
use crate::settings::workspace::{KEY, SKILLS};
use crate::settings::{Refusal, Which, layer_text};

/// The key in `settings.skills`, and in `charter.toml`'s `[skills]`, that holds the budget.
pub const BUDGET: &str = "budget";
/// The key in [`BUDGET`] that holds the most skills a chat lists.
pub const SKILLS_KEY: &str = "skills";
/// The key in [`BUDGET`] that holds the most characters of what is listed of them.
pub const CHARS_KEY: &str = "chars";

/// **What `value` writes**, leniently: a key that is no whole number of at least 1 writes
/// nothing, so the number below it stands, and a save would have refused it ([`refused`]).
fn set_of(value: Option<&toml::Value>) -> Set {
    let read = |key: &str| {
        value
            .and_then(|value| value.get(key))
            .and_then(toml::Value::as_integer)
            .filter(|number| *number >= 1)
            .and_then(|number| usize::try_from(number).ok())
    };
    Set {
        skills: read(SKILLS_KEY),
        chars: read(CHARS_KEY),
    }
}

/// `over`, with each number `set` writes put in its place.
fn laid_over(over: Set, set: Set) -> Set {
    Set {
        skills: set.skills.or(over.skills),
        chars: set.chars.or(over.chars),
    }
}

/// **What the plane at `root` writes for a chat in `workspace`**: the project's, then the
/// workspace's over it, key by key.
pub fn set_in(root: &Path, workspace: Option<&str>) -> Set {
    let text = layer_text(root, Which::Shared);
    let shared = text
        .text()
        .and_then(|text| text.parse::<toml::Table>().ok());
    let project = set_of(
        shared
            .as_ref()
            .and_then(|top| top.get(SKILLS))
            .and_then(|table| table.get(BUDGET)),
    );
    let table = workspace.and_then(|name| crate::settings::workspace::read(root, name));
    let here = set_of(
        table
            .as_ref()
            .and_then(|top| top.get(SKILLS))
            .and_then(|table| table.get(BUDGET)),
    );
    laid_over(project, here)
}

/// **The budget in force on `harness` for a chat in `workspace`** of the plane at `root`: the
/// adapter's numbers, with what the plane writes over them.
pub fn budget_in(root: &Path, workspace: Option<&str>, harness: Harness) -> purlis_skills::Budget {
    harness.skill_budget().with(set_in(root, workspace))
}

/// **What each harness would drop at what `set` writes**, one sentence each, in harness order:
/// what a Settings row says beside the numbers. Nothing where nothing is written, since the
/// adapters' own numbers sit under their limits. Never a refusal.
pub fn warnings(set: Set) -> Vec<String> {
    if set == Set::default() {
        return Vec::new();
    }
    Harness::ALL
        .iter()
        .filter_map(|harness| harness.skill_limit_past(harness.skill_budget().with(set)))
        .collect()
}

/// What `value`, the budget at `shown`, holds that [`set_of`] would not read, each at its key
/// under `at`.
fn refused(value: &toml::Value, shown: &str, file: &str, at: &[&str]) -> Vec<Refusal> {
    let shape = format!("a budget is {{\"{SKILLS_KEY}\": 60, \"{CHARS_KEY}\": 15000}}");
    let Some(table) = value.as_table() else {
        return vec![Refusal::at(
            format!("{shown} in {file} is not a table, so it changes nothing — {shape}"),
            at,
        )];
    };
    table
        .iter()
        .filter_map(|(key, value)| {
            let why = if key != SKILLS_KEY && key != CHARS_KEY {
                format!("{shown}.{key} in {file} is not read — {shape}")
            } else if value.as_integer().is_some_and(|number| number >= 1) {
                return None;
            } else {
                format!(
                    "{shown}.{key} in {file} is not a whole number of at least 1, so the \
                     harness's own number stands — {shape}"
                )
            };
            let mut place = at.to_vec();
            place.push(key);
            Some(Refusal::at(why, &place))
        })
        .collect()
}

/// What a workspace's `settings.skills.budget` holds that its reader would not read.
pub fn refused_in_workspace(value: &toml::Value, file: &str) -> Vec<Refusal> {
    refused(
        value,
        &format!("{KEY}.{SKILLS}.{BUDGET}"),
        file,
        &[SKILLS, BUDGET],
    )
}

/// **What `text`, the settings file `which` of the project at `root`, says wrong of `[skills]`**:
/// the Shared file holds `budget` alone, and the Local file no `[skills]`, since a budget is the
/// project's to agree on.
pub fn keyed_at(root: &Path, text: &str, which: Which) -> Vec<Refusal> {
    let file = which.file_at(root);
    let Ok(top) = text.parse::<toml::Table>() else {
        return Vec::new();
    };
    let Some(value) = top.get(SKILLS) else {
        return Vec::new();
    };
    if which != Which::Shared {
        return vec![Refusal::at(
            format!(
                "[{SKILLS}] in {file} is not read: a project's skill budget is set in {}",
                Which::Shared.file_at(root)
            ),
            &[SKILLS],
        )];
    }
    let Some(table) = value.as_table() else {
        return vec![Refusal::at(
            format!("[{SKILLS}] in {file} is not a table"),
            &[SKILLS],
        )];
    };
    table
        .iter()
        .flat_map(|(key, value)| {
            if key == BUDGET {
                refused(
                    value,
                    &format!("[{SKILLS}] {BUDGET}"),
                    file,
                    &[SKILLS, BUDGET],
                )
            } else {
                vec![Refusal::at(
                    format!(
                        "[{SKILLS}] {} in {file} is not read: a project's [{SKILLS}] holds \
                         {BUDGET} alone",
                        crate::shown::short(key)
                    ),
                    &[SKILLS, key],
                )]
            }
        })
        .collect()
}

#[cfg(test)]
mod tests;

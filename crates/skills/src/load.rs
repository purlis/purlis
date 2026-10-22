// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Finding and loading a skill of a chat's set by name** (SK-13, #2776; ADR 0063 as amended
//! 2026-10-20): what purlis's `skill_find` and `skill_load` answer, on every harness, and all a
//! harness on [`crate::Route::Mcp`] has of its skills.
//!
//! Everything here is over one chat's [`SkillSet`]: a skill the set leaves out is never found and
//! never loaded, and a skill is named, never addressed by a path. A load hands back the skill's
//! `SKILL.md` and the paths of the other files in its folder, which a harness reads with its own
//! tools when the skill says to; and what the skill would narrow ([`Narrowing`]), which the
//! caller says it applied or did not, never silently.

use std::path::{Path, PathBuf};

use crate::{Effective, SkillSet};

/// **The one line a chat on [`crate::Route::Mcp`] is told about its skills**, whatever their
/// number: they are found with `skill_find` and loaded with `skill_load`. A chat on any other
/// route is told about its skills by that route, and never also by this line.
pub const POINTER: &str = "⬢ **This chat's skills** are reached through purlis's tools: when a \
     request may match one, find it with `skill_find` and load it with `skill_load`, then follow \
     what it says before acting.";

/// The most skills one `skill_find` answers with.
pub(crate) const MOST_FOUND: usize = 10;

/// The most bytes of a `SKILL.md` a load hands back. A skill is instructions to read, and one
/// past this is no skill a chat should be handed whole.
const MOST_BYTES: u64 = 256 * 1024;

/// The most other files of a skill's folder a load names.
const MOST_FILES: usize = 200;

/// How deep in a skill's folder a load looks for its other files.
const MOST_DEPTH: usize = 4;

/// **What a skill would take away from the chat that uses it** (decision 58): a skill may only
/// narrow a chat, never widen it. Read from its frontmatter: the tools its `disallowed-tools`
/// names, and `context: fork`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Narrowing {
    /// The tools it takes away, by the names its frontmatter gives them.
    pub tools_taken: Vec<String>,
    /// Whether it runs in a fork of the chat rather than in the chat itself.
    pub fork: bool,
}

impl Narrowing {
    /// Whether it narrows the chat at all.
    pub fn any(&self) -> bool {
        self.fork || !self.tools_taken.is_empty()
    }

    /// What it narrows, as the end of a sentence: "takes away Write and Edit, and runs in a
    /// fork".
    pub fn said(&self) -> String {
        let mut parts = Vec::new();
        if !self.tools_taken.is_empty() {
            parts.push(format!("takes away {}", joined(&self.tools_taken)));
        }
        if self.fork {
            parts.push("runs in a fork of the chat".to_owned());
        }
        parts.join(", and ")
    }
}

/// `words` as a person reads a list: `a`, `a and b`, `a, b and c`.
fn joined(words: &[String]) -> String {
    match words {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// **What the `SKILL.md` text `text` narrows**, from its frontmatter ([`crate::frontmatter`]):
/// `disallowed-tools`, a comma-separated list, and `context: fork`.
pub(crate) fn narrowing(text: &str) -> Narrowing {
    let pairs = crate::frontmatter(text);
    let value = |key: &str| {
        pairs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    };
    Narrowing {
        tools_taken: value("disallowed-tools")
            .map(|tools| {
                tools
                    .split(',')
                    .map(str::trim)
                    .filter(|tool| !tool.is_empty())
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default(),
        fork: value("context").is_some_and(|context| context.eq_ignore_ascii_case("fork")),
    }
}

/// **The skill of `set` named `name`**: by its whole name, or by the name after a harness's
/// namespace (`skills:deploy` is `deploy`), as a chat may give it. `None` where the set holds
/// none of that name, whatever is on disk.
pub fn by_name<'a>(set: &'a SkillSet, name: &str) -> Option<&'a Effective> {
    let name = name.trim();
    let leaf = name.rsplit_once(':').map_or(name, |(_, leaf)| leaf);
    set.effective
        .iter()
        .find(|effective| effective.skill.name == name || effective.skill.name == leaf)
}

/// **The skills of `set` that match `query`**, best first and at most [`MOST_FOUND`]: each word
/// of the query, ignoring case, found in a skill's name counts twice and in its description once.
/// A skill no word is found in is left out, and a tie goes by name. An empty query matches
/// nothing.
pub fn find<'a>(set: &'a SkillSet, query: &str) -> Vec<&'a Effective> {
    let words: Vec<String> = query
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_lowercase)
        .collect();
    let mut scored: Vec<(usize, &Effective)> = set
        .effective
        .iter()
        .filter_map(|effective| {
            let name = effective.skill.name.to_lowercase();
            let description = effective.skill.description.to_lowercase();
            let score: usize = words
                .iter()
                .map(|word| {
                    2 * usize::from(name.contains(word.as_str()))
                        + usize::from(description.contains(word.as_str()))
                })
                .sum();
            (score > 0).then_some((score, effective))
        })
        .collect();
    scored.sort_by(|(a, one), (b, other)| {
        b.cmp(a).then_with(|| one.skill.name.cmp(&other.skill.name))
    });
    scored
        .into_iter()
        .take(MOST_FOUND)
        .map(|(_, effective)| effective)
        .collect()
}

/// A skill, loaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Loaded {
    /// Its `SKILL.md`, whole.
    pub text: String,
    /// Every other file in its folder, by its whole path, in order: what the skill may tell the
    /// chat to read or run.
    pub files: Vec<PathBuf>,
    /// What it would narrow.
    pub narrowing: Narrowing,
}

/// **Loads `effective`, a skill of a chat's set**: its `SKILL.md` and the other files of its
/// folder. Nothing is followed through a link: a `SKILL.md` that is a link is refused, and a link
/// inside the folder is not named, so a load never hands over a file outside the skill. Refused,
/// with why, for a `SKILL.md` that cannot be read or is past [`MOST_BYTES`].
pub fn load(effective: &Effective) -> Result<Loaded, String> {
    let path = &effective.skill.path;
    let name = &effective.skill.name;
    let unreadable = |why: String| format!("the skill {name} could not be loaded: {why}");
    let meta = std::fs::symlink_metadata(path).map_err(|e| unreadable(e.to_string()))?;
    if !meta.is_file() {
        return Err(unreadable("its SKILL.md is not a plain file".to_owned()));
    }
    if meta.len() > MOST_BYTES {
        return Err(unreadable(format!(
            "its SKILL.md is larger than {} KiB",
            MOST_BYTES / 1024
        )));
    }
    let text = std::fs::read_to_string(path).map_err(|e| unreadable(e.to_string()))?;
    let mut files = Vec::new();
    if let Some(folder) = path.parent() {
        others(folder, path, 0, &mut files);
    }
    files.sort();
    files.truncate(MOST_FILES);
    Ok(Loaded {
        narrowing: narrowing(&text),
        text,
        files,
    })
}

/// Every plain file under `dir` but `skill_md`, into `files`, to [`MOST_DEPTH`] and at most
/// [`MOST_FILES`]: never through a link, which is not named either.
fn others(dir: &Path, skill_md: &Path, depth: usize, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        if files.len() >= MOST_FILES {
            return;
        }
        let Ok(meta) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if meta.is_file() && path != skill_md {
            files.push(path);
        } else if meta.is_dir() && depth + 1 < MOST_DEPTH {
            others(&path, skill_md, depth + 1, files);
        }
    }
}

#[cfg(test)]
mod tests;

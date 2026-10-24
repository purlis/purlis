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

/// **What the `SKILL.md` text `text` narrows**, from its frontmatter read as YAML
/// ([`crate::handed`]): the tools `disallowed-tools` takes away, and `context: fork`. Nothing
/// for a frontmatter purlis cannot read, which it hands to no chat.
pub(crate) fn narrowing(text: &str) -> Narrowing {
    crate::handed::parse(text)
        .map(|read| Narrowing {
            tools_taken: read.tools_taken(),
            fork: read.fork(),
        })
        .unwrap_or_default()
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

/// **Loads `effective`, a skill of a chat's set, as a harness that reads `honours` is handed
/// it**: the `SKILL.md` purlis writes for it ([`crate::handed_text`]), so a load shows what
/// every other route hands, and the other files of its folder.
///
/// **Nothing is read through a link** (SK-36): its folder is reached from its level's root
/// ([`Effective::root`]) one part at a time, each opened through the handle of the folder above
/// without following a link, and its `SKILL.md` and every file it names are read and listed
/// through that held folder ([`crate::held::Folder`]). So a folder swapped for a link after the
/// set was read is refused, and a load never reads or names a file outside the skill. Refused,
/// with why, for a `SKILL.md` purlis cannot read or hands to no chat.
pub fn load(effective: &Effective, honours: &crate::Honours) -> Result<Loaded, String> {
    use crate::held::{Folder, Opened};
    let path = &effective.skill.path;
    let name = &effective.skill.name;
    let refused = |why: crate::Refusal| format!("the skill {name} could not be loaded: {why}");
    let rel = path
        .parent()
        .and_then(|folder| folder.strip_prefix(&effective.root).ok());
    let folder = match rel.map(|rel| Folder::walk(&effective.root, rel)) {
        Some(Opened::Folder(folder)) => folder,
        Some(Opened::Link) => return Err(refused(crate::Refusal::FolderLink)),
        Some(Opened::Absent) | None => {
            return Err(refused(crate::Refusal::Unreadable(
                "its folder is not there".to_owned(),
            )));
        }
    };
    let own = crate::handed::read_held(&folder, crate::FILE.as_ref()).map_err(refused)?;
    let text = crate::handed_text(&own, honours).map_err(refused)?;
    Ok(Loaded {
        narrowing: narrowing(&text),
        text,
        files: files_in(&folder),
    })
}

/// **Every other file of the folder of the skill whose `SKILL.md` is at `skill_md`**, by its
/// whole path, in order: plain files only, never one reached through a link, to [`MOST_DEPTH`]
/// and at most [`MOST_FILES`]. The folder is opened without following a link, and nothing in it
/// is named for a folder that is one. What the lint reads (SK-8).
pub fn other_files(skill_md: &Path) -> Vec<PathBuf> {
    match skill_md.parent().map(crate::held::Folder::at) {
        Some(crate::held::Opened::Folder(folder)) => files_in(&folder),
        _ => Vec::new(),
    }
}

/// Every other file of the skill held at `folder`, as [`other_files`] names them.
fn files_in(folder: &crate::held::Folder) -> Vec<PathBuf> {
    let mut files = Vec::new();
    others(folder, 0, &mut files);
    files.sort();
    files.truncate(MOST_FILES);
    files
}

/// Every plain file under the held folder `dir` but a `SKILL.md`, into `files`, to
/// [`MOST_DEPTH`] and at most [`MOST_FILES`]: each folder opened through the handle of the one
/// above, never through a link, which is not named either.
fn others(dir: &crate::held::Folder, depth: usize, files: &mut Vec<PathBuf>) {
    use crate::held::{Kind, Opened};
    for name in dir.names() {
        if files.len() >= MOST_FILES {
            return;
        }
        match dir.kind(&name) {
            // Only the top `SKILL.md` is the skill's: a nested one is no file of it a harness is
            // handed, so it never reads as a skill of its own.
            Some(Kind::File) if name != crate::FILE => files.push(dir.shown().join(&name)),
            Some(Kind::Folder) if depth + 1 < MOST_DEPTH => {
                if let Opened::Folder(inner) = dir.child(&name) {
                    others(&inner, depth + 1, files);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests;

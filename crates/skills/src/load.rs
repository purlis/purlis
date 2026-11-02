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

/// **What a chat its harness was handed skills for is told of one the harness does not know**
/// (#4510): a skill can name another that was hidden by the router or installed since the chat
/// started, and the harness's own tool can only answer that it knows no such skill.
pub const NOT_KNOWN: &str = "⬢ **When your harness does not know a skill**: if its own way of \
     starting a skill says it does not know a skill's name, load that skill with purlis's \
     `skill_load` tool before giving up or working around it. purlis does not hand all of this \
     chat's skills to its harness.";

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

/// **The skill of `set` named `name`**: by its whole name, or by the name after one of purlis's
/// own namespaces (`skills:deploy` is `deploy`), as a chat may give it. Another plugin's
/// `other:deploy` is not this set's `deploy` (#4510). `None` where the set holds none of that
/// name, whatever is on disk.
pub fn by_name<'a>(set: &'a SkillSet, name: &str) -> Option<&'a Effective> {
    let name = name.trim();
    // A built-in skill by any of its names: never the plane's skill of an old one's leaf.
    if name.contains(':')
        && let Some(crate::Reference::BuiltIn(own)) = crate::Reference::parse(name)
    {
        return set.effective.iter().find(|effective| {
            effective.origin == crate::Origin::BuiltIn && effective.skill.name == own
        });
    }
    let bare = match name.rsplit_once(':') {
        Some(("skills", leaf)) => leaf,
        _ => name,
    };
    set.effective
        .iter()
        .find(|effective| effective.skill.name == name || effective.skill.name == bare)
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
    /// Whether it is manual-only, by its own `SKILL.md` read as YAML ([`crate::handed`]), as every
    /// harness is told it: run only when the person asks.
    pub manual: bool,
}

/// **Loads `effective`, a skill of a chat's set, as a harness that reads `honours` is handed
/// it**: the `SKILL.md` purlis writes for it ([`crate::handed_text`]), so a load shows what
/// every other route hands, and the other files of its folder, read and named through the
/// skill's folder held from its level's root ([`SkillFolder`]). Refused, with why, for a folder
/// reached through a link, or a `SKILL.md` purlis cannot read or hands to no chat.
pub fn load(effective: &Effective, honours: &crate::Honours) -> Result<Loaded, String> {
    let name = &effective.skill.name;
    let refused = |why: crate::Refusal| format!("the skill {name} could not be loaded: {why}");
    let folder = SkillFolder::of(effective).map_err(refused)?;
    let own = folder.skill_md().map_err(refused)?;
    let text = crate::handed_text(&own, honours).map_err(refused)?;
    Ok(Loaded {
        narrowing: narrowing(&text),
        manual: crate::handed::parse(&own).is_ok_and(|read| read.manual()),
        text,
        files: folder.other_files(),
    })
}

/// One entry of a skill's folder, by its path inside the folder: a folder, or a plain file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Entry {
    Folder(PathBuf),
    File(PathBuf),
}

/// **A skill's folder, held open from its level's root** (SK-36): `root` (the plane's root, the
/// bundle's plugin folder, a clone's root) opened as it is, then every part below it to the
/// skill's folder through the handle of the one above, never following a link
/// ([`crate::held::Folder`]). Its `SKILL.md` and every other file are read and listed through
/// that handle, so a folder swapped for a link after the skill was found is refused, and nothing
/// outside the skill is read or named. The one way purlis reads a skill's folder: a load, the
/// set's folder a harness is handed, the lint and the badges.
pub struct SkillFolder(crate::held::Folder);

impl SkillFolder {
    /// The folder of the skill whose `SKILL.md` is at `skill_md`, under `root`.
    pub fn open(root: &Path, skill_md: &Path) -> Result<Self, crate::Refusal> {
        use crate::held::{Folder, Opened};
        let rel = skill_md
            .parent()
            .and_then(|folder| folder.strip_prefix(root).ok());
        match rel.map(|rel| Folder::walk(root, rel)) {
            Some(Opened::Folder(folder)) => Ok(Self(folder)),
            Some(Opened::Link) => Err(crate::Refusal::FolderLink),
            Some(Opened::Absent) | None => Err(crate::Refusal::Unreadable(
                "its folder is not there".to_owned(),
            )),
        }
    }

    /// The folder of `effective`, under its level's root ([`Effective::root`]).
    pub fn of(effective: &Effective) -> Result<Self, crate::Refusal> {
        Self::open(&effective.root, &effective.skill.path)
    }

    /// Its folder, as it is shown.
    pub fn path(&self) -> &Path {
        self.0.shown()
    }

    /// Its `SKILL.md`, read as [`crate::read_skill_md`] reads one.
    pub fn skill_md(&self) -> Result<String, crate::Refusal> {
        crate::handed::read_held(&self.0, crate::FILE)
    }

    /// Its `SKILL.md`, read as [`crate::read_skill_md_of`] reads one of a skill from `origin`.
    pub fn skill_md_of(&self, origin: crate::Origin) -> Result<String, crate::Refusal> {
        crate::handed::read_held_as(&self.0, crate::FILE, crate::held::HardLinks::of(origin))
    }

    /// **Every folder and plain file under it but a `SKILL.md`**, by its path inside it, each
    /// folder before what it holds and in order of name: never through a link, which is not
    /// named either, to [`MOST_DEPTH`] and at most [`MOST_FILES`] files. Only the top `SKILL.md`
    /// is the skill's: a nested one is no file of it a harness is handed, so it never reads as a
    /// skill of its own.
    pub fn entries(&self) -> Vec<Entry> {
        let mut entries = Vec::new();
        let mut files = 0;
        walk(&self.0, Path::new(""), 0, &mut entries, &mut files);
        entries
    }

    /// **The text of the file at `rel` inside it**, of at most `most` bytes: every folder on the
    /// way opened through the handle of the one above and the file through its folder's, never
    /// following a link, refused as a skill's own file is. `None` where it cannot be read so.
    pub fn read(&self, rel: &Path, most: u64) -> Option<String> {
        self.read_counted(rel, most, crate::held::HardLinks::Refused)
            .text
            .ok()
    }

    /// [`Self::read`] of a file of a skill from `origin`: with a hard link read where its
    /// `SKILL.md` is read so ([`Self::skill_md_of`], #3459).
    pub fn read_of(&self, rel: &Path, most: u64, origin: crate::Origin) -> Option<String> {
        self.read_counted(rel, most, crate::held::HardLinks::of(origin))
            .text
            .ok()
    }

    /// [`Self::read`], a hard link read or refused as `hard_links` says, with why it read
    /// nothing and the bytes it read ([`crate::held::Counted`], #3459): what a budget of bytes
    /// counts.
    pub fn read_counted(
        &self,
        rel: &Path,
        most: u64,
        hard_links: crate::held::HardLinks,
    ) -> crate::held::Counted {
        use crate::held::{Counted, NotRead, Opened};
        let absent = || Counted {
            text: Err(NotRead::Unreadable("it is not there".to_owned())),
            read: 0,
        };
        let Some(name) = rel.file_name() else {
            return absent();
        };
        let mut parts = rel.parent().into_iter().flat_map(Path::components);
        let first = match parts.next() {
            None => return self.0.read_counted(name, most, hard_links),
            Some(std::path::Component::Normal(part)) => part,
            Some(_) => return absent(),
        };
        let Opened::Folder(mut at) = self.0.child(first, crate::held::Follow::NoLink) else {
            return absent();
        };
        for part in parts {
            let std::path::Component::Normal(part) = part else {
                return absent();
            };
            let Opened::Folder(next) = at.child(part, crate::held::Follow::NoLink) else {
                return absent();
            };
            at = next;
        }
        at.read_counted(name, most, hard_links)
    }

    /// Every other file of it, by its whole path, in order: its [`Self::entries`]' files.
    pub fn other_files(&self) -> Vec<PathBuf> {
        self.entries()
            .into_iter()
            .filter_map(|entry| match entry {
                Entry::File(rel) => Some(self.path().join(rel)),
                Entry::Folder(_) => None,
            })
            .collect()
    }
}

/// [`SkillFolder::entries`] under the held folder `dir`, at `rel` inside the skill.
fn walk(
    dir: &crate::held::Folder,
    rel: &Path,
    depth: usize,
    entries: &mut Vec<Entry>,
    files: &mut usize,
) {
    use crate::held::{Follow, Kind, Opened};
    // A folder of more names than purlis reads of one (SK-52), or one that cannot be read
    // whole, ends the walk as the most files do: what was listed before it stays listed.
    let Ok(names) = dir.names() else {
        *files = MOST_FILES;
        return;
    };
    for name in names {
        if *files >= MOST_FILES {
            return;
        }
        let path = rel.join(&name);
        match dir.kind(&name) {
            Some(Kind::File) if name != crate::FILE => {
                entries.push(Entry::File(path));
                *files += 1;
            }
            Some(Kind::Folder) if depth + 1 < MOST_DEPTH => {
                if let Opened::Folder(inner) = dir.child(&name, Follow::NoLink) {
                    entries.push(Entry::Folder(path.clone()));
                    walk(&inner, &path, depth + 1, entries, files);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests;

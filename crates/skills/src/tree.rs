// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A skill source's tree, as git lists it** (SK-10, #2773; ADR 0100): which entries of a
//! fetched commit purlis would write, which it leaves out and why, and which folders are skills.
//!
//! A source is read from git's own object store, never checked out, so nothing in it is ever a
//! file on disk until purlis writes it under a name it made. This is the rule for those names:
//! only a plain file or an executable one, never a link or another repository; every part of its
//! path a plain name (printable ASCII, not `.` or `..`, no separator, nothing starting `.git`);
//! and no two paths that a case-insensitive disk would make one.

use serde::Serialize;

/// The most bytes one file of an installed skill may hold.
pub const MOST_FILE_BYTES: u64 = 1024 * 1024;

/// The most files one installed skill may hold.
pub const MOST_FILES: usize = 1000;

/// How deep under the source's folder a skill's folder may be: `skills/<name>/SKILL.md` is two.
pub const MOST_SKILL_DEPTH: usize = 4;

/// A file of the tree that purlis would write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Its path's parts, each a [`plain_name`].
    pub path: Vec<String>,
    pub executable: bool,
    /// The blob git holds it as.
    pub oid: String,
    pub size: u64,
}

impl Entry {
    /// Its path, `/` between the parts.
    pub fn shown(&self) -> String {
        self.path.join("/")
    }
}

/// Why an entry of the tree is not written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Why {
    /// A link: it would point wherever it says.
    Link,
    /// Another repository (a submodule): purlis fetches none.
    Submodule,
    /// A name purlis does not write: `..`, a separator, anything starting `.git`, or one not
    /// plain ASCII.
    Name,
    /// A name that a disk which ignores case would make the same as another's.
    Case,
    /// Larger than [`MOST_FILE_BYTES`].
    TooLarge,
    /// Not a file git knows how to hand over.
    Other,
}

impl std::fmt::Display for Why {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Link => "a link, which purlis never installs",
            Self::Submodule => "another repository inside it, which purlis never fetches",
            Self::Name => "a name purlis never writes",
            Self::Case => "the same name as another file on a disk that ignores case",
            Self::TooLarge => return write!(f, "larger than {} MiB", MOST_FILE_BYTES >> 20),
            Self::Other => "not a plain file",
        })
    }
}

/// An entry of the tree left out, its path as git lists it, and why.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct LeftOut {
    pub path: String,
    pub why: Why,
}

/// **Whether `name` is one part of a path purlis writes**: printable ASCII, at most 255 bytes,
/// not `.` or `..`, no `/`, `\` or `:`, no trailing dot or blank, nothing that starts `.git` in
/// any case (git's own control files, which would change how the project's git reads the folder),
/// nor git's short form of `.git` (`git~1`).
pub(crate) fn plain_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    !name.is_empty()
        && name.len() <= 255
        && name != "."
        && name != ".."
        && name.bytes().all(|b| (0x20..0x7f).contains(&b))
        && !name.contains(['/', '\\', ':'])
        && !name.ends_with(['.', ' '])
        && !lower.starts_with(".git")
        && !lower.starts_with("git~")
}

/// The most entries a source's tree may list: past it nothing is read.
pub const MOST_ENTRIES: usize = 50_000;

/// The most skill folders one source may hold: past it the source is not browsed.
pub const MOST_SKILLS: usize = 200;

/// The most bytes a path purlis writes may have, the level's folder and the skill's name
/// included.
pub const MOST_PATH_BYTES: usize = 1024;

/// What a source's tree is, by git's word for an entry's mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    File,
    Executable,
    Link,
    Submodule,
    Other,
}

/// One record of `git ls-tree -r -z --full-tree <commit>`: its mode, its object and its path's
/// bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub mode: Mode,
    pub oid: String,
    pub path: Vec<u8>,
}

/// Why a source's tree is not read at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unread {
    /// git answered something it does not write.
    NotGit,
    /// More than [`MOST_ENTRIES`].
    TooManyEntries,
}

impl std::fmt::Display for Unread {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotGit => f.write_str("its files could not be listed"),
            Self::TooManyEntries => write!(
                f,
                "it holds more than {MOST_ENTRIES} files, the most purlis reads of a skill source"
            ),
        }
    }
}

/// **The records `listing` holds** — `git ls-tree -r -z --full-tree <commit>`'s answer — counted
/// before any is read, so a tree past [`MOST_ENTRIES`] costs one scan. A record that is not one
/// git writes refuses the whole listing.
pub fn records(listing: &[u8]) -> Result<Vec<Record>, Unread> {
    if listing.iter().filter(|b| **b == 0).count() > MOST_ENTRIES {
        return Err(Unread::TooManyEntries);
    }
    let mut out = Vec::new();
    for record in listing.split(|b| *b == 0) {
        if record.is_empty() {
            continue;
        }
        let tab = record
            .iter()
            .position(|b| *b == b'\t')
            .ok_or(Unread::NotGit)?;
        let head = std::str::from_utf8(&record[..tab]).map_err(|_| Unread::NotGit)?;
        let mut fields = head.split(' ');
        let (Some(mode), Some(kind), Some(oid), None) =
            (fields.next(), fields.next(), fields.next(), fields.next())
        else {
            return Err(Unread::NotGit);
        };
        if !matches!(oid.len(), 40 | 64) || !oid.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(Unread::NotGit);
        }
        let mode = match (mode, kind) {
            ("100644", "blob") => Mode::File,
            ("100755", "blob") => Mode::Executable,
            ("120000", "blob") => Mode::Link,
            ("160000", "commit") => Mode::Submodule,
            _ => Mode::Other,
        };
        out.push(Record {
            mode,
            oid: oid.to_owned(),
            path: record[tab + 1..].to_vec(),
        });
    }
    Ok(out)
}

/// **The files purlis would write of `records`, and those it leaves out**, each blob's size
/// asked of `size_of` (`None`: the fetch left it out, past [`MOST_FILE_BYTES`]). A path whose
/// case-folded form, or a folder of whose, another path already took is left out, so of two such
/// paths the first git lists is the one written; each path's folded prefixes are kept in a set,
/// so a tree costs one pass.
pub fn read_listing(
    records: Vec<Record>,
    size_of: impl Fn(&str) -> Option<u64>,
) -> (Vec<Entry>, Vec<LeftOut>) {
    use std::collections::HashMap;
    let mut entries = Vec::new();
    let mut left = Vec::new();
    let mut folded: HashMap<String, String> = HashMap::new();
    for record in records {
        let shown = String::from_utf8_lossy(&record.path).into_owned();
        let out = |why| LeftOut {
            path: shown.clone(),
            why,
        };
        let executable = match record.mode {
            Mode::File => false,
            Mode::Executable => true,
            Mode::Link => {
                left.push(out(Why::Link));
                continue;
            }
            Mode::Submodule => {
                left.push(out(Why::Submodule));
                continue;
            }
            Mode::Other => {
                left.push(out(Why::Other));
                continue;
            }
        };
        let Ok(text) = std::str::from_utf8(&record.path) else {
            left.push(out(Why::Name));
            continue;
        };
        let parts: Vec<String> = text.split('/').map(str::to_owned).collect();
        if !parts.iter().all(|part| plain_name(part)) {
            left.push(out(Why::Name));
            continue;
        }
        match size_of(&record.oid) {
            Some(size) if size <= MOST_FILE_BYTES => {
                let mut clash = false;
                let mut prefix = String::new();
                let mut seen = Vec::new();
                for part in &parts {
                    if !prefix.is_empty() {
                        prefix.push('/');
                    }
                    prefix.push_str(part);
                    let lower = prefix.to_ascii_lowercase();
                    match folded.get(&lower) {
                        Some(original) if *original != prefix => clash = true,
                        Some(_) => {}
                        None => seen.push((lower, prefix.clone())),
                    }
                }
                if clash {
                    left.push(out(Why::Case));
                    continue;
                }
                folded.extend(seen);
                entries.push(Entry {
                    path: parts,
                    executable,
                    oid: record.oid,
                    size,
                });
            }
            _ => left.push(out(Why::TooLarge)),
        }
    }
    (entries, left)
}

/// The parts of `folder`, a folder inside a source as a person writes it (`skills`,
/// `packs/ops/`), or `None` where one of them is not a [`plain_name`]. An empty folder is the
/// source's top.
pub fn folder_parts(folder: &str) -> Option<Vec<String>> {
    let parts: Vec<String> = folder
        .split('/')
        .filter(|part| !part.is_empty())
        .map(str::to_owned)
        .collect();
    parts.iter().all(|part| plain_name(part)).then_some(parts)
}

/// **The folders of `entries` under `folder` that hold a skill**: each that has a `SKILL.md` of
/// its own at most [`MOST_SKILL_DEPTH`] folders down, `folder` itself included (a source that is
/// one skill). A skill folder inside another is part of that one, not a skill of its own.
pub fn skill_folders(entries: &[Entry], folder: &[String]) -> Vec<Vec<String>> {
    let mut found: Vec<Vec<String>> = entries
        .iter()
        .filter(|entry| entry.path.starts_with(folder))
        .filter(|entry| entry.path.last().is_some_and(|name| name == crate::FILE))
        .map(|entry| entry.path[..entry.path.len() - 1].to_vec())
        .filter(|dir| dir.len() - folder.len() <= MOST_SKILL_DEPTH)
        .collect();
    found.sort_by(|a, b| a.len().cmp(&b.len()).then_with(|| a.cmp(b)));
    let mut out: Vec<Vec<String>> = Vec::new();
    for dir in found {
        if !out.iter().any(|outer| dir.starts_with(outer)) {
            out.push(dir);
        }
    }
    out.sort();
    out
}

/// The entries of the skill whose folder is `dir`, each with its path inside that folder.
pub fn files_of<'a>(entries: &'a [Entry], dir: &[String]) -> Vec<(Vec<String>, &'a Entry)> {
    entries
        .iter()
        .filter(|entry| entry.path.len() > dir.len() && entry.path.starts_with(dir))
        .map(|entry| (entry.path[dir.len()..].to_vec(), entry))
        .collect()
}

/// The entries left out that are inside the folder `dir`, with their paths inside it.
pub fn left_in(left: &[LeftOut], dir: &[String]) -> Vec<LeftOut> {
    let prefix = if dir.is_empty() {
        String::new()
    } else {
        format!("{}/", dir.join("/"))
    };
    left.iter()
        .filter_map(|one| {
            one.path.strip_prefix(&prefix).map(|inside| LeftOut {
                path: inside.to_owned(),
                why: one.why,
            })
        })
        .collect()
}

/// **The name and description of a fetched `SKILL.md`**, where every reader reads the same ones:
/// its frontmatter fenced by lines that are `---` alone, parsed as the YAML it is into one mapping
/// of plain keys, none twice, with no anchor, alias or tag anywhere, and its `name` and
/// `description` single scalars that purlis's own line reader ([`crate::parsed`]) reads alike.
/// Anything else is refused, in a sentence: a source's skill is installed only where the name a
/// harness loads is the name the review showed.
pub fn plain_identity(text: &str) -> Result<(String, String), String> {
    use yaml_rust2::parser::{Event, Parser};
    let refused = |why: &str| Err(format!("its frontmatter {why}"));
    let mut lines = text.split_inclusive('\n');
    if lines.next().map(|l| l.trim_end_matches(['\n', '\r'])) != Some("---") {
        return refused("does not start with a line that is `---` alone");
    }
    let start = text.find('\n').map_or(text.len(), |at| at + 1);
    let mut at = start;
    let mut end = None;
    for line in lines {
        if line.trim_end_matches(['\n', '\r']) == "---" {
            end = Some(at);
            break;
        }
        at += line.len();
    }
    let Some(end) = end else {
        return refused("has no closing line that is `---` alone");
    };
    // purlis's line reader ends the block at the first `---` anywhere: the same place, or the
    // two would read different keys.
    if crate::read::frontmatter_block(text) != Some((3, end)) {
        return refused("holds `---` inside a value");
    }
    let yaml = &text[start..end];
    let mut parser = Parser::new_from_str(yaml);
    let mut next = || parser.next_token().map(|(event, _)| event);
    let bad = |_| "is not YAML purlis reads".to_owned();
    if !matches!(next().map_err(bad)?, Event::StreamStart)
        || !matches!(next().map_err(bad)?, Event::DocumentStart)
    {
        return refused("is not one YAML document");
    }
    match next().map_err(bad)? {
        Event::MappingStart(0, None) => {}
        _ => return refused("is not one plain mapping"),
    }
    let mut keys: Vec<String> = Vec::new();
    let (mut name, mut description) = (None, None);
    loop {
        let key = match next().map_err(bad)? {
            Event::MappingEnd => break,
            Event::Scalar(key, yaml_rust2::scanner::TScalarStyle::Plain, 0, None) => key,
            _ => return refused("has a key that is not a plain word"),
        };
        if keys.contains(&key) {
            return refused(&format!("names {key} twice"));
        }
        keys.push(key.clone());
        // The value: one scalar for the two keys purlis reads, anything plain for the rest.
        let mut depth = 0usize;
        loop {
            match next().map_err(bad)? {
                Event::Alias(_) => return refused("holds an alias"),
                Event::Scalar(value, _, 0, None) => {
                    if depth == 0 {
                        match key.as_str() {
                            "name" => name = Some(value),
                            "description" => description = Some(value),
                            _ => {}
                        }
                        break;
                    }
                }
                Event::SequenceStart(0, None) | Event::MappingStart(0, None) => {
                    if matches!(key.as_str(), "name" | "description") {
                        return refused(&format!("gives {key} more than one line of words"));
                    }
                    depth += 1;
                }
                Event::SequenceEnd | Event::MappingEnd if depth > 0 => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ => return refused("holds an anchor, a tag or something purlis does not read"),
            }
        }
    }
    if !matches!(next().map_err(bad)?, Event::DocumentEnd)
        || !matches!(next().map_err(bad)?, Event::StreamEnd)
    {
        return refused("is more than one YAML document");
    }
    let (Some(name), Some(description)) = (name, description) else {
        return refused("names no name or no description");
    };
    let read = crate::parsed(text, std::path::PathBuf::new())
        .map_err(|_| "its frontmatter is not one purlis reads".to_owned())?;
    if read.name != name || read.description != description {
        return refused("reads one way as YAML and another line by line");
    }
    Ok((name, description))
}

#[cfg(test)]
mod tests;

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A skill source's tree, as git lists it** (SK-10, #2773; ADR 0100): which entries of a
//! fetched commit purlis would write, which it leaves out and why, and which folders are skills.
//!
//! A source is read from git's own object store, never checked out, so nothing in it is ever a
//! file on disk until purlis writes it under a name it made. This is the rule for those names:
//! only a plain file or an executable one, never a link or another repository; every part of its
//! path a plain name (printable ASCII, not `.` or `..`, no separator, never a git control file);
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
    /// A name purlis does not write: `..`, a separator, a git control file, or one not plain
    /// ASCII.
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
            Self::TooLarge => "larger than 1 MiB",
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

/// The names of git's own control files: one inside a skill would change how the project's git
/// reads the skill's folder.
const GIT_FILES: [&str; 4] = [".git", ".gitattributes", ".gitignore", ".gitmodules"];

/// **Whether `name` is one part of a path purlis writes**: printable ASCII, at most 255 bytes,
/// not `.` or `..`, no `/`, `\` or `:`, no trailing dot or blank, and no git control file
/// whatever its case, nor git's short form of `.git` (`git~1`).
pub fn plain_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    !name.is_empty()
        && name.len() <= 255
        && name != "."
        && name != ".."
        && name.bytes().all(|b| (0x20..0x7f).contains(&b))
        && !name.contains(['/', '\\', ':'])
        && !name.ends_with(['.', ' '])
        && !GIT_FILES.contains(&lower.as_str())
        && !lower.starts_with("git~")
}

/// **The tree `listing` names** — `git ls-tree -r -z -l <commit>`'s answer — as the files purlis
/// would write and those it leaves out. A path whose case-folded form another path already took
/// is left out, so of two such paths the first git lists is the one written.
pub fn read_listing(listing: &[u8]) -> (Vec<Entry>, Vec<LeftOut>) {
    let mut entries = Vec::new();
    let mut left = Vec::new();
    let mut folded: Vec<(String, String)> = Vec::new();
    for record in listing.split(|b| *b == 0) {
        if record.is_empty() {
            continue;
        }
        let Some(tab) = record.iter().position(|b| *b == b'\t') else {
            continue;
        };
        let (head, path) = (&record[..tab], &record[tab + 1..]);
        let path_shown = String::from_utf8_lossy(path).into_owned();
        let head = String::from_utf8_lossy(head);
        let mut fields = head.split_whitespace();
        let (Some(mode), Some(kind), Some(oid), size) =
            (fields.next(), fields.next(), fields.next(), fields.next())
        else {
            continue;
        };
        let out = |why| LeftOut {
            path: path_shown.clone(),
            why,
        };
        let executable = match (mode, kind) {
            ("100644", "blob") => false,
            ("100755", "blob") => true,
            ("120000", _) => {
                left.push(out(Why::Link));
                continue;
            }
            ("160000", _) => {
                left.push(out(Why::Submodule));
                continue;
            }
            _ => {
                left.push(out(Why::Other));
                continue;
            }
        };
        let Ok(text) = std::str::from_utf8(path) else {
            left.push(out(Why::Name));
            continue;
        };
        let parts: Vec<String> = text.split('/').map(str::to_owned).collect();
        if !parts.iter().all(|part| plain_name(part)) {
            left.push(out(Why::Name));
            continue;
        }
        let size: u64 = size.and_then(|s| s.parse().ok()).unwrap_or(u64::MAX);
        if size > MOST_FILE_BYTES {
            left.push(out(Why::TooLarge));
            continue;
        }
        // Every prefix of the path, folded: a folder `Scripts/` clashes with `scripts/` too.
        let mut clash = false;
        let mut prefix = String::new();
        let mut seen = Vec::new();
        for part in &parts {
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(part);
            let lower = prefix.to_ascii_lowercase();
            match folded.iter().find(|(f, _)| *f == lower) {
                Some((_, original)) if *original != prefix => clash = true,
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
            oid: oid.to_owned(),
            size,
        });
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

#[cfg(test)]
mod tests;

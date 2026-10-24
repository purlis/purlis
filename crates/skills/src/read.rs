// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Reading skill folders, and the briefing listing a harness without a skills route is given
//! (ADR 0063).

use std::path::{Path, PathBuf};

use crate::{Missing, Skill};

/// Where the skills sit inside the bundled plugin directory. Claude Code reads a plugin's
/// `skills/` itself, which is why it is this name.
pub const DIR_IN_BUNDLE: &str = "skills";

/// A skill's file inside its folder.
pub const FILE: &str = "SKILL.md";

/// The skills directory inside the bundled plugin at `plugin`, when it is there as a folder of
/// its own: a link there is never read through ([`scan`]).
pub fn in_bundle(plugin: &Path) -> Option<PathBuf> {
    Some(plugin.join(DIR_IN_BUNDLE))
        .filter(|dir| std::fs::symlink_metadata(dir).is_ok_and(|meta| meta.is_dir()))
}

/// Every folder in `dir`: the skill it holds, or the folder and what it lacks to be one. A file
/// beside the folders is neither. In the order the directory lists them.
///
/// **Nothing is read through a link** (SK-36): `dir` is opened without following a link at its
/// last step, each folder in it and its `SKILL.md` through the handle of the folder above, so a
/// link put in place after a look is never read through either. A folder that is a link to a
/// folder is left out as a link ([`Missing::Link`]), and so is `dir` where it is one, whatever
/// it leads to: what is listed of a skill, and every file a load names of it, is inside it.
pub fn scan(dir: &Path) -> Vec<Result<Skill, (PathBuf, Missing)>> {
    held::scan(dir)
}

/// The text of a `SKILL.md` as [`scan`] reads it, or what it lacks: at most [`crate::MOST_BYTES`],
/// and text.
fn text_of(file: impl std::io::Read) -> Result<String, Missing> {
    use std::io::Read as _;
    let most = crate::MOST_BYTES;
    let mut text = String::new();
    file.take(most + 1)
        .read_to_string(&mut text)
        .map_err(|_| Missing::File)?;
    if text.len() as u64 > most {
        return Err(Missing::TooLarge);
    }
    Ok(text)
}

/// [`scan`], through folder handles.
#[cfg(unix)]
mod held {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt as _;
    use std::path::{Path, PathBuf};

    use rustix::fd::OwnedFd;
    use rustix::fs::{AtFlags, FileType, Mode, OFlags};

    use super::{FILE, parsed, text_of};
    use crate::{Missing, Skill};

    fn folder_flags() -> OFlags {
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC
    }

    /// What `name` in `at` is, never following a link; or what it leads to where `follow`.
    fn kind(at: &OwnedFd, name: &OsStr, follow: bool) -> Option<FileType> {
        let flags = if follow {
            AtFlags::empty()
        } else {
            AtFlags::SYMLINK_NOFOLLOW
        };
        let stat = rustix::fs::statat(at, name, flags).ok()?;
        Some(FileType::from_raw_mode(stat.st_mode))
    }

    /// Whether `path` is a link, asked without following it.
    fn is_link(path: &Path) -> bool {
        std::fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_symlink())
    }

    pub(super) fn scan(dir: &Path) -> Vec<Result<Skill, (PathBuf, Missing)>> {
        let top = match rustix::fs::open(dir, folder_flags(), Mode::empty()) {
            Ok(top) => top,
            Err(_) if is_link(dir) => return vec![Err((dir.to_path_buf(), Missing::Link))],
            Err(_) => return Vec::new(),
        };
        let Ok(listing) = rustix::fs::Dir::read_from(&top) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for entry in listing.filter_map(Result::ok) {
            let name = OsStr::from_bytes(entry.file_name().to_bytes()).to_owned();
            if name == "." || name == ".." {
                continue;
            }
            let path = dir.join(&name);
            match kind(&top, &name, false) {
                Some(FileType::Directory) => {}
                // A link is told as one only where it leads to a folder: one to a file is a file
                // beside the folders, which is nothing.
                Some(FileType::Symlink) => {
                    if kind(&top, &name, true) == Some(FileType::Directory) {
                        out.push(Err((path, Missing::Link)));
                    }
                    continue;
                }
                _ => continue,
            }
            let folder = match rustix::fs::openat(&top, &name, folder_flags(), Mode::empty()) {
                Ok(folder) => folder,
                // It turned into a link since it was looked at.
                Err(_) if kind(&top, &name, false) == Some(FileType::Symlink) => {
                    out.push(Err((path, Missing::Link)));
                    continue;
                }
                Err(_) => continue,
            };
            out.push(whole(&folder, path.join(FILE)).map_err(|missing| (path, missing)));
        }
        out
    }

    /// The skill in the folder held at `folder`, whose `SKILL.md` is said to be at `shown`.
    fn whole(folder: &OwnedFd, shown: PathBuf) -> Result<Skill, Missing> {
        let flags = OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC;
        let file = match rustix::fs::openat(folder, FILE, flags, Mode::empty()) {
            Ok(file) => std::fs::File::from(file),
            Err(_) if kind(folder, OsStr::new(FILE), false) == Some(FileType::Symlink) => {
                return Err(Missing::Link);
            }
            Err(_) => return Err(Missing::File),
        };
        let meta = file.metadata().map_err(|_| Missing::File)?;
        if !meta.is_file() {
            return Err(Missing::File);
        }
        if meta.len() > crate::MOST_BYTES {
            return Err(Missing::TooLarge);
        }
        parsed(&text_of(file)?, shown)
    }
}

/// [`scan`] off unix, by path, with the same answers where nothing changes in between.
#[cfg(not(unix))]
mod held {
    use std::path::{Path, PathBuf};

    use super::{FILE, parsed, text_of};
    use crate::{Missing, Skill};

    fn is_link(path: &Path) -> bool {
        std::fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_symlink())
    }

    pub(super) fn scan(dir: &Path) -> Vec<Result<Skill, (PathBuf, Missing)>> {
        if is_link(dir) {
            return vec![Err((dir.to_path_buf(), Missing::Link))];
        }
        let Ok(entries) = std::fs::read_dir(dir) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for path in entries.filter_map(Result::ok).map(|entry| entry.path()) {
            if is_link(&path) {
                if path.is_dir() {
                    out.push(Err((path, Missing::Link)));
                }
                continue;
            }
            if !std::fs::symlink_metadata(&path).is_ok_and(|meta| meta.is_dir()) {
                continue;
            }
            let file = path.join(FILE);
            let found = if is_link(&file) {
                Err(Missing::Link)
            } else {
                match std::fs::symlink_metadata(&file) {
                    Ok(meta) if !meta.is_file() => Err(Missing::File),
                    Ok(meta) if meta.len() > crate::MOST_BYTES => Err(Missing::TooLarge),
                    Ok(_) => std::fs::File::open(&file)
                        .map_err(|_| Missing::File)
                        .and_then(text_of)
                        .and_then(|text| parsed(&text, file)),
                    Err(_) => Err(Missing::File),
                }
            };
            out.push(found.map_err(|missing| (path, missing)));
        }
        out
    }
}

/// **The skill a `SKILL.md` whose text is `text` holds**, at `path`, or what it lacks: by the
/// same rule every skill folder is read by. For a caller that read the file itself, through a
/// handle that follows no link (purlis-core's harness and repo skills, SK-6).
pub fn parsed(text: &str, path: PathBuf) -> Result<Skill, Missing> {
    let pairs = frontmatter(text);
    let value = |key: &str| {
        pairs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
            .filter(|v| !v.is_empty())
    };
    Ok(Skill {
        name: value("name").ok_or(Missing::Name)?,
        description: value("description").ok_or(Missing::Description)?,
        path,
    })
}

/// Every skill in `dir`, by name. A folder with no `SKILL.md`, or one whose frontmatter names no
/// `name` or no `description`, is not a skill any harness would load, and is left out.
pub fn read(dir: &Path) -> Vec<Skill> {
    let mut skills: Vec<Skill> = scan(dir).into_iter().filter_map(Result::ok).collect();
    skills.sort_by(|a, b| a.name.cmp(&b.name));
    skills
}

/// A `SKILL.md`'s frontmatter, as `key: value` pairs in the order written: one line per key,
/// which is how every built-in skill is written.
///
/// The one rule purlis reads a frontmatter by, a persona's too
/// (`purlis_core::personas::frontmatter`): the block between the first two `---`, split into
/// lines as Python's `str.splitlines` splits them, each line at its first `:`, key and value
/// stripped as Python's `str.strip` strips, and a line with an empty key dropped.
pub fn frontmatter(text: &str) -> Vec<(String, String)> {
    frontmatter_lines(text)
        .into_iter()
        .filter_map(|line| {
            let (key, value) = line.split_once(':')?;
            let key = py_strip(key);
            (!key.is_empty()).then(|| (key.to_owned(), py_strip(value).to_owned()))
        })
        .collect()
}

/// **Where a frontmatter is**, as byte offsets into `text`: from just after the opening `---`
/// to the start of the next `---`, or `None` where it has none. The one rule every reader and
/// writer of a frontmatter keeps ([`frontmatter`], and the rename of a skill's text in
/// [`crate::change`]).
pub(crate) fn frontmatter_block(text: &str) -> Option<(usize, usize)> {
    let rest = text.strip_prefix("---")?;
    let end = rest.find("---")?;
    Some((3, 3 + end))
}

/// The lines of a `SKILL.md`'s frontmatter, by the rule [`frontmatter`] reads it by: the block
/// [`frontmatter_block`] finds, stripped and split as Python splits lines. Empty where there is
/// no frontmatter. What a reader of a key that spans lines (a block list) reads.
pub(crate) fn frontmatter_lines(text: &str) -> Vec<&str> {
    frontmatter_block(text)
        .map(|(start, end)| lines(py_strip(&text[start..end])))
        .unwrap_or_default()
}

/// `str.strip()`: Python's whitespace is Rust's and the four separator controls U+001C-U+001F.
fn py_strip(text: &str) -> &str {
    text.trim_matches(|c: char| {
        c.is_whitespace() || matches!(c, '\u{1c}' | '\u{1d}' | '\u{1e}' | '\u{1f}')
    })
}

/// `str.splitlines()`: every line break Python knows, `\r\n` as one, and no empty last line.
fn lines(text: &str) -> Vec<&str> {
    fn is_break(c: char) -> bool {
        matches!(
            c,
            '\n' | '\r'
                | '\u{b}'
                | '\u{c}'
                | '\u{1c}'
                | '\u{1d}'
                | '\u{1e}'
                | '\u{85}'
                | '\u{2028}'
                | '\u{2029}'
        )
    }
    let mut out = Vec::new();
    let mut start = 0;
    let mut it = text.char_indices().peekable();
    while let Some((i, c)) = it.next() {
        if !is_break(c) {
            continue;
        }
        out.push(&text[start..i]);
        if c == '\r' && it.peek().is_some_and(|&(_, next)| next == '\n') {
            it.next();
            start = i + 2;
        } else {
            start = i + c.len_utf8();
        }
    }
    if start < text.len() {
        out.push(&text[start..]);
    }
    out
}

/// The briefing section that lists `skills`, or none when there are none.
///
/// It is what a harness with skills of its own shows its model: each skill's name, when to use
/// it, and the file to read when it applies. The model reads that file with its own tools.
///
/// **A manual-only skill** ([`crate::declared`]) is purlis's equivalent of the harness's own
/// switch ([`crate::compat::ManualOnly::Listing`], SK-8): its description stays out, so the
/// model never picks it by itself, and it is named with its file on one line of its own, to read
/// only when the person names it. Its description costs nothing.
pub fn listing(skills: &[Skill]) -> Option<String> {
    if skills.is_empty() {
        return None;
    }
    let mut text = String::from(
        "⬢ **purlis's built-in skills** — this harness does not load them itself, so they are listed \
         here. When a request matches one, read its `SKILL.md` and follow it before acting; a \
         prompt that names a skill by its name means this list.",
    );
    let (manual, chosen): (Vec<&Skill>, Vec<&Skill>) = skills
        .iter()
        .partition(|skill| crate::declared(skill).manual);
    for skill in chosen {
        text.push_str(&format!(
            "\n- `{}` — {} (`{}`)",
            skill.name,
            skill.description,
            skill.path.display()
        ));
    }
    if !manual.is_empty() {
        let named: Vec<String> = manual
            .iter()
            .map(|skill| format!("`{}` (`{}`)", skill.name, skill.path.display()))
            .collect();
        text.push_str(&format!(
            "\n- Only when the person names one, never by your own choice: {}",
            named.join(", ")
        ));
    }
    Some(text)
}

#[cfg(test)]
mod tests;

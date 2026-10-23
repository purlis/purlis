// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What a harness is handed of one plane skill** (SK-8, #2771; decision 58): a folder of its own
//! in the set's folder, holding the `SKILL.md` purlis writes for it
//! ([`purlis_skills::handed_text`]) and a link to each plain file of the skill's own folder, in
//! folders of their own as the skill has them.
//!
//! **Nothing is handed through a link.** A skill whose folder or `SKILL.md` is a link is refused
//! and handed to no chat, whose tab says so ([`super::not_here`]). A link inside the skill's folder
//! is never linked to, so every link handed resolves to a plain file inside the skill's own
//! folder. Only the top `SKILL.md` is the skill's: a nested one is left out, so no harness that
//! scans a folder for `SKILL.md` files reads one as a skill of its own.
//!
//! On unix every folder is read through its handle (`O_DIRECTORY | O_NOFOLLOW`), and every
//! folder, file and link handed is made relative to the handle of the folder it is made in.

use std::path::{Path, PathBuf};

use purlis_skills::{Honours, Refusal, Skill};

/// How deep in a skill's folder purlis looks for files to hand: as deep as a load looks.
const MOST_DEPTH: usize = 4;

/// The most files of one skill purlis hands.
const MOST_FILES: usize = 200;

/// One entry of a skill's folder as it is handed: a folder of its own, or a link to a plain file
/// of the skill's, by its path inside the folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Entry {
    Folder(PathBuf),
    File(PathBuf),
}

/// **One plane skill, as a set's folder hands it.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Handed {
    pub(super) name: String,
    /// The skill's own folder.
    pub(super) folder: PathBuf,
    /// The `SKILL.md` purlis writes for it.
    pub(super) text: String,
    /// Every entry handed beside it, each folder before what it holds.
    pub(super) entries: Vec<Entry>,
}

/// **`skill`, as a harness that reads `honours` is handed it**, or why it is handed to no chat:
/// its folder or `SKILL.md` is a link, or its frontmatter is refused
/// ([`purlis_skills::Refusal`]). The one place that decides, for the set's folder, the Codex
/// briefing and the Notice alike.
pub(super) fn of(skill: &Skill, honours: &Honours) -> Result<Handed, Refusal> {
    let folder = skill.path.parent().ok_or(Refusal::FolderLink)?;
    let own = purlis_skills::read_skill_md(&skill.path)?;
    let text = purlis_skills::handed_text(&own, honours)?;
    let mut entries = Vec::new();
    walk(folder, Path::new(""), 0, &mut entries);
    Ok(Handed {
        name: skill.name.clone(),
        folder: folder.to_path_buf(),
        text,
        entries,
    })
}

/// Every plain file and folder under `folder`'s `rel`, into `entries`, never through a link and
/// never a `SKILL.md`, to [`MOST_DEPTH`] and at most [`MOST_FILES`] files, in order of name.
#[cfg(unix)]
fn walk(folder: &Path, rel: &Path, depth: usize, entries: &mut Vec<Entry>) {
    use rustix::fs::FileType;
    let Ok(dir) = super::at::folder(&folder.join(rel)) else {
        return;
    };
    let Ok(mut found) = super::at::entries(&dir) else {
        return;
    };
    found.sort_by(|a, b| a.name.cmp(&b.name));
    for one in found {
        let files = entries
            .iter()
            .filter(|e| matches!(e, Entry::File(_)))
            .count();
        if files >= MOST_FILES {
            return;
        }
        let path = rel.join(&one.name);
        match one.kind {
            FileType::RegularFile if one.name != purlis_skills::FILE => {
                entries.push(Entry::File(path));
            }
            FileType::Directory if depth + 1 < MOST_DEPTH => {
                entries.push(Entry::Folder(path.clone()));
                walk(folder, &path, depth + 1, entries);
            }
            _ => {}
        }
    }
}

/// [`walk`] where there are no handles to read a folder by.
#[cfg(not(unix))]
fn walk(folder: &Path, rel: &Path, depth: usize, entries: &mut Vec<Entry>) {
    let Ok(listed) = std::fs::read_dir(folder.join(rel)) else {
        return;
    };
    let mut names: Vec<std::ffi::OsString> = listed
        .filter_map(Result::ok)
        .map(|e| e.file_name())
        .collect();
    names.sort();
    for name in names {
        let files = entries
            .iter()
            .filter(|e| matches!(e, Entry::File(_)))
            .count();
        if files >= MOST_FILES {
            return;
        }
        let path = rel.join(&name);
        let Ok(meta) = std::fs::symlink_metadata(folder.join(&path)) else {
            continue;
        };
        if meta.is_file() && name != purlis_skills::FILE {
            entries.push(Entry::File(path));
        } else if meta.is_dir() && depth + 1 < MOST_DEPTH {
            entries.push(Entry::Folder(path.clone()));
            walk(folder, &path, depth + 1, entries);
        }
    }
}

/// What a digest of a set's folder covers of one handed skill: its name, its folder, the text
/// handed and every entry.
pub(super) fn digested(one: &Handed, hash: &mut impl sha2::Digest) {
    hash.update(one.name.as_bytes());
    hash.update([0]);
    hash.update(one.folder.as_os_str().as_encoded_bytes());
    hash.update([0]);
    hash.update(one.text.as_bytes());
    hash.update([0]);
    for entry in &one.entries {
        let (kind, path) = match entry {
            Entry::Folder(path) => (b'd', path),
            Entry::File(path) => (b'f', path),
        };
        hash.update([kind]);
        hash.update(path.as_os_str().as_encoded_bytes());
        hash.update([0]);
    }
}

/// **Makes `one`'s folder in the set's `skills/` folder `skills`**, through its handle: a folder
/// named for the skill, its `SKILL.md`, and each entry, a folder made or a link to the skill's
/// own file.
#[cfg(unix)]
pub(super) fn make_in(skills: &std::os::fd::OwnedFd, one: &Handed) -> std::io::Result<()> {
    use std::io::Write as _;
    use std::os::fd::AsFd;
    rustix::fs::mkdirat(
        skills,
        one.name.as_str(),
        rustix::fs::Mode::from_raw_mode(0o755),
    )?;
    let own = super::at::folder_in(skills, &one.name)?;
    super::at::new_file_in(&own, purlis_skills::FILE)?.write_all(one.text.as_bytes())?;
    for entry in &one.entries {
        let (Entry::Folder(rel) | Entry::File(rel)) = entry;
        let parent = rel.parent().filter(|p| !p.as_os_str().is_empty());
        let name = rel
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| std::io::Error::other("a name that is not text"))?;
        let dir = match parent {
            Some(parent) => super::at::folder_under(own.as_fd(), parent)?,
            None => own.try_clone()?,
        };
        match entry {
            Entry::Folder(_) => {
                rustix::fs::mkdirat(&dir, name, rustix::fs::Mode::from_raw_mode(0o755))?;
            }
            Entry::File(_) => rustix::fs::symlinkat(one.folder.join(rel), &dir, name)?,
        }
    }
    Ok(())
}

/// [`make_in`] where there are no handles to make a folder's entries by.
#[cfg(not(unix))]
pub(super) fn make_at(skills: &Path, one: &Handed) -> std::io::Result<()> {
    let own = skills.join(&one.name);
    std::fs::create_dir(&own)?;
    std::fs::write(own.join(purlis_skills::FILE), &one.text)?;
    for entry in &one.entries {
        match entry {
            Entry::Folder(rel) => std::fs::create_dir(own.join(rel))?,
            Entry::File(rel) => link_file(&one.folder.join(rel), &own.join(rel))?,
        }
    }
    Ok(())
}

#[cfg(windows)]
fn link_file(target: &Path, at: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_file(target, at)
}

#[cfg(not(any(unix, windows)))]
fn link_file(target: &Path, at: &Path) -> std::io::Result<()> {
    std::fs::copy(target, at).map(|_| ())
}

/// **Whether `skills`, a set's `skills/` folder, holds exactly `handed`**: one folder per skill,
/// each with its `SKILL.md` as handed and every entry as made. Read following no link but the
/// ones handed.
pub(super) fn holds(skills: &Path, handed: &[Handed]) -> bool {
    let count = std::fs::read_dir(skills).map(Iterator::count).unwrap_or(0);
    count == handed.len()
        && handed.iter().all(|one| {
            let own = skills.join(&one.name);
            std::fs::symlink_metadata(&own).is_ok_and(|meta| meta.is_dir())
                && purlis_skills::read_skill_md(&own.join(purlis_skills::FILE))
                    .ok()
                    .as_deref()
                    == Some(one.text.as_str())
                && one.entries.iter().all(|entry| match entry {
                    Entry::Folder(rel) => {
                        std::fs::symlink_metadata(own.join(rel)).is_ok_and(|meta| meta.is_dir())
                    }
                    Entry::File(rel) => {
                        std::fs::read_link(own.join(rel)).ok() == Some(one.folder.join(rel))
                    }
                })
        })
}

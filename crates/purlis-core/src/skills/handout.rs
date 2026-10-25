// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What a harness is handed of one skill of a set's folder** (SK-8, #2771; decision 58): a
//! plane skill, or a built-in one on a route that names it its own; a folder of its own
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

use purlis_skills::{Effective, Honours, Refusal, SkillFolder};

/// One entry of a skill's folder as it is handed: a folder of its own, or a link to a plain file
/// of the skill's, by its path inside the folder ([`SkillFolder::entries`]).
pub(super) use purlis_skills::Entry;

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

/// **`effective`, as a harness that reads `honours` is handed it**, or why it is handed to no
/// chat: a folder on the way to it from its level's root or its `SKILL.md` is a link, or its
/// frontmatter is refused ([`purlis_skills::Refusal`]). Its folder is held from that root, and
/// its `SKILL.md` read and its entries listed through that handle ([`SkillFolder`], SK-36), so a
/// folder swapped for a link after the set was read hands nothing. The one place that decides,
/// for the set's folder, the Codex briefing and the Notice alike.
pub(super) fn of(effective: &Effective, honours: &Honours) -> Result<Handed, Refusal> {
    let folder = SkillFolder::of(effective)?;
    let own = folder.skill_md()?;
    // Named as the set holds it: a built-in skill handed under a name of its own says that name.
    let text = purlis_skills::handed_text_as(&own, honours, &effective.skill.name)?;
    Ok(Handed {
        name: effective.skill.name.clone(),
        folder: folder.path().to_path_buf(),
        text,
        entries: folder.entries(),
    })
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
                && purlis_skills::read_skill_md(skills, &own.join(purlis_skills::FILE))
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

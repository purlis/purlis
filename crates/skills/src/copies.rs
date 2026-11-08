// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **One skill from two sources** (#5737, spec #5714): a skill of the plane's that a harness
//! also finds itself, in the person's own folders or a plugin, under the same name.
//!
//! - **The same content takes one slot.** Where every file of the plane's copy is the same as
//!   the harness's, purlis leaves its own copy out of the chat, naming the copy listed in its
//!   place ([`Reason::SameAs`]). purlis only ever leaves out what it hands, so the copy kept is
//!   the harness's.
//! - **Different content is never decided for the person.** Both copies are listed, and each
//!   pair is named ([`differing_copies`]) so the chat's Skills view can say so.
//!
//! A built-in skill never gives way (SK-37), and a repo's copy keeps the narrowest-copy rule it
//! has (decision 51, [`crate::Outcome::RepoWins`]): neither is a copy here.

use std::path::{Path, PathBuf};

use crate::{Effective, LeftOut, Origin, OutsideSkill, Owner, Reason, Route, SkillSet};

/// The most files purlis compares of one skill's folder: past it two folders are not the same.
const MOST_FILES: usize = 1000;

/// **Whether two skill folders hold the same content**: the same files at the same paths, each
/// with the same bytes. A folder that cannot be read whole, holds a link, or holds more than
/// [`MOST_FILES`] files is the same as none, so its copies are both listed.
pub fn same_content(one: &Path, other: &Path) -> bool {
    let (Some(one_files), Some(other_files)) = (files_of(one), files_of(other)) else {
        return false;
    };
    one_files == other_files
        && one_files.iter().all(|relative| {
            match (
                std::fs::read(one.join(relative)),
                std::fs::read(other.join(relative)),
            ) {
                (Ok(a), Ok(b)) => a == b,
                _ => false,
            }
        })
}

/// Every file under `dir`, by its path from it, sorted; `None` where it cannot be read whole or
/// holds anything but folders and plain files.
fn files_of(dir: &Path) -> Option<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut folders = vec![PathBuf::new()];
    while let Some(relative) = folders.pop() {
        for entry in std::fs::read_dir(dir.join(&relative)).ok()? {
            let entry = entry.ok()?;
            let kind = entry.file_type().ok()?;
            let path = relative.join(entry.file_name());
            if kind.is_dir() {
                folders.push(path);
            } else if kind.is_file() {
                files.push(path);
            } else {
                return None;
            }
            if files.len() > MOST_FILES {
                return None;
            }
        }
    }
    files.sort();
    Some(files)
}

/// The folder a skill sits in: its `SKILL.md`'s.
fn folder_of(path: &Path) -> &Path {
    path.parent().unwrap_or(path)
}

/// Whether `one` is a copy that can stand beside the plane's: one the harness finds itself,
/// on, and read.
fn harness_copy(one: &OutsideSkill) -> bool {
    one.origin == Origin::Harness && !one.off && one.unread.is_none()
}

impl SkillSet {
    /// **This set, without each plane skill the harness already has the same copy of** (#5737):
    /// a skill of `own` (the person's own and their plugins', as the chat's start read them)
    /// with its name and the same content ([`same_content`]). Each is left out naming the copy
    /// kept, which the harness lists itself. A built-in skill is never left out, nor a skill
    /// whose copies differ.
    #[must_use]
    pub fn without_copies_of(mut self, own: &[OutsideSkill]) -> Self {
        let mut left = Vec::new();
        self.effective.retain(|e: &Effective| {
            if e.route == Route::Own || !e.origin.of_the_plane() {
                return true;
            }
            let kept = own.iter().filter(|one| harness_copy(one)).find(|one| {
                one.skill.name == e.skill.name
                    && same_content(folder_of(&e.skill.path), folder_of(&one.skill.path))
            });
            match kept {
                Some(kept) => {
                    left.push(LeftOut {
                        name: e.skill.name.clone(),
                        origin: e.origin,
                        local: e.local,
                        folder: folder_of(&e.skill.path).to_path_buf(),
                        reason: Reason::SameAs {
                            kept: kept.owner.clone(),
                        },
                    });
                    false
                }
                None => true,
            }
        });
        self.left_out.extend(left);
        crate::widest_first(&mut self.left_out);
        self
    }
}

/// **Whose one copy of a skill is**, as a person reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CopyOf {
    /// One purlis hands: the plane's at a level, or a built-in one.
    Plane { origin: Origin, local: bool },
    /// One the harness finds itself.
    Own(Owner),
}

/// `the project's`, `the local workspace's`, `plugin figma`, `your own`.
impl std::fmt::Display for CopyOf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Plane {
                origin: Origin::BuiltIn,
                ..
            } => f.write_str("purlis's built-in"),
            Self::Plane { origin, local } => {
                write!(f, "the {}'s", crate::level_shown(*origin, *local))
            }
            Self::Own(Owner::You) => f.write_str("your own"),
            Self::Own(Owner::Plugin(name)) => write!(f, "plugin {name}"),
            Self::Own(Owner::Repo(name)) => write!(f, "repo {name}"),
        }
    }
}

/// **Two copies of one name whose content differs** (#5737), both listed to the chat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Differ {
    pub name: String,
    pub one: CopyOf,
    pub other: CopyOf,
}

/// **Every pair of copies of one name that differ** (#5737), among the skills `set` hands its
/// chat and the copies its harness finds itself (`own`), by name and then in the order met: the
/// plane's copy first, then the harness's in the order read. Both copies of each pair are listed;
/// purlis never picks one. A repo's copy is no copy here (decision 51 keeps the narrower), nor
/// is one turned off or not read.
pub fn differing_copies(set: &SkillSet, own: &[OutsideSkill]) -> Vec<Differ> {
    let handed = set
        .effective
        .iter()
        .filter(|e| e.route != Route::Own)
        .map(|e| {
            (
                &e.skill.name,
                CopyOf::Plane {
                    origin: e.origin,
                    local: e.local,
                },
                folder_of(&e.skill.path),
            )
        });
    let found = own.iter().filter(|one| harness_copy(one)).map(|one| {
        (
            &one.skill.name,
            CopyOf::Own(one.owner.clone()),
            folder_of(&one.skill.path),
        )
    });
    let copies: Vec<(&String, CopyOf, &Path)> = handed.chain(found).collect();
    let mut pairs = Vec::new();
    for (at, (name, one, folder)) in copies.iter().enumerate() {
        for (other_name, other, other_folder) in &copies[at + 1..] {
            if name == other_name && folder != other_folder && !same_content(folder, other_folder) {
                pairs.push(Differ {
                    name: (*name).clone(),
                    one: one.clone(),
                    other: other.clone(),
                });
            }
        }
    }
    pairs.sort_by(|a, b| a.name.cmp(&b.name));
    pairs
}

#[cfg(test)]
mod tests;

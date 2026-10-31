// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The names and levels a skill had before, as this machine keeps them** (#4018): where git
//! cannot follow it. A skill only on this machine (`skills.local/`) has no history in git, so a
//! rename of it, or a move into or out of `skills.local/`, leaves its grades under a name or
//! level git never saw it leave ([`super::super::history::places`] follows only what git holds).
//! The verbs keep each such earlier name and level here, with when it was left, and its Grades
//! tab and a grade's scorecard read the grades of each ([`super::read_followed`]).
//!
//! An earlier place lends only the rows recorded before it was left (`until`): a skill that
//! takes the old name there since is another, and its grades are none of these. A grade of it
//! that was under way as it left ends after that, by the old name, so the place lends the rows
//! of those grades too (`grades`, #4104). A rename or a move of the skill takes what it had
//! along to its new place, so the list holds every earlier place it had on this machine, newest
//! first, at most [`MOST_EARLIER`].
//!
//! **The places are the folder's that took the key** (#4104): each keeps which folder that was
//! (`folder`, [`identity`]), and lends its rows only while the folder at the key is that one. A
//! skill made at the name some other way (an install, a draft's accept, a folder made by hand
//! after the first was removed by hand) is another folder, and shows none of them. A new skill
//! made with its name by a verb drops them besides; a delete keeps them, and a Restore takes the
//! folder it brings back as theirs ([`refound`]), so the grades come back with it.
//!
//! **Tier:** Machine, device-bound. It lives in this machine's app state of the project,
//! `<data>/app-state/<clone key>/skill-earlier-names.json` ([`FILE`], `crate::appstate`), as the
//! grades of a skill only on this machine do. Read through no link, at most [`MOST_FILE_BYTES`],
//! and a file that does not hold together reads as empty. Losing it only leaves those grades
//! under the name they were recorded by.

use std::collections::BTreeMap;
use std::io::Read as _;
use std::path::{Path, PathBuf};

use super::{Level, plain};

/// The file in this machine's app state of the project that holds each skill's earlier places.
pub const FILE: &str = "skill-earlier-names.json";

/// The lock file beside [`FILE`] whose `flock` each change holds while it reads, changes and
/// writes the file.
pub const LOCK: &str = "skill-earlier-names.lock";

/// The version of the file this purlis writes and reads.
const VERSION: u32 = 1;

/// The most bytes of the file read.
const MOST_FILE_BYTES: u64 = 256 * 1024;

/// The most skills the file keeps earlier places for.
const MOST_SKILLS: usize = 512;

/// The most earlier places kept for one skill: past them the oldest go.
pub const MOST_EARLIER: usize = 16;

/// The most grades under way one earlier place keeps, past any project's (one runs at a time).
const MOST_UNDER_WAY: usize = 16;

/// How long a change waits for another to let go of [`LOCK`].
const LOCK_WAIT: std::time::Duration = std::time::Duration::from_secs(2);

/// **A place a skill had before on this machine**: its name and its level's folder there
/// (`skills.local`, `workspaces/ops/skills`), and when it left it, as an RFC 3339 time.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Earlier {
    pub name: String,
    pub level: String,
    pub until: String,
    /// **The grades of it under way as it left** (#4104), by id: each is recorded as it ends,
    /// after `until`, by the name and level it had here, and is lent all the same.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub grades: Vec<String>,
    /// **Which folder took the key it is kept by** ([`identity`], #4104): it lends nothing to
    /// another. `None` where that could not be read, or in a file written before this field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
}

impl Earlier {
    /// When it left the place, where that reads.
    pub fn until(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        chrono::DateTime::parse_from_rfc3339(&self.until)
            .ok()
            .map(|at| at.with_timezone(&chrono::Utc))
    }

    /// Whether it says only what purlis writes.
    fn sound(&self) -> bool {
        plain(&self.name)
            && Level::of_rel(&self.level).is_some()
            && self.until().is_some()
            && self.grades.len() <= MOST_UNDER_WAY
            && self.grades.iter().all(|grade| plain(grade))
            && self.folder.as_deref().is_none_or(|word| {
                word.len() <= 128
                    && word
                        .bytes()
                        .all(|b| b.is_ascii_digit() || b"-:".contains(&b))
            })
    }
}

/// **Which folder holds the copy of `name` at `level`** in the project at `root`, as this machine
/// tells folders apart: its inode and when it was made, where that reads. A rename or a move
/// keeps it; a folder removed and made again, by hand or by a verb, is another. Not its device,
/// which a volume mounted again may change.
/// `None` where no folder holds the copy, or this system says none of it.
pub fn identity(root: &Path, level: &Level, name: &str) -> Option<String> {
    let folder = super::super::verbs::find(root, level, name).ok()?;
    let meta = std::fs::symlink_metadata(folder).ok()?;
    if !meta.is_dir() {
        return None;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        // Made at, where the system keeps it: an inode is given again once its folder is gone.
        let made = meta
            .created()
            .ok()
            .and_then(|at| at.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or_else(|| "-".to_owned(), |since| since.as_nanos().to_string());
        Some(format!("{}:{made}", meta.ino()))
    }
    #[cfg(not(unix))]
    {
        None
    }
}

/// The file, as it is written: each skill's earlier places by the copy it is now
/// ([`super::after_edit::key`]), newest first.
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
struct Kept {
    v: u32,
    #[serde(default)]
    skills: BTreeMap<String, Vec<Earlier>>,
}

/// Where the file of the project at `root` is: `None` where this machine has no data home.
fn path_of(root: &Path) -> Option<PathBuf> {
    Some(crate::appstate::dir(root)?.join(FILE))
}

/// The file as it reads: nothing where it is absent, not a file of its own, too large, or not
/// what purlis writes. An entry that does not hold together is left out.
fn read(root: &Path) -> Kept {
    let Some(path) = path_of(root) else {
        return Kept::default();
    };
    let Ok(meta) = std::fs::symlink_metadata(&path) else {
        return Kept::default();
    };
    if !meta.is_file() || meta.len() > MOST_FILE_BYTES {
        return Kept::default();
    }
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    let Ok(file) = crate::contain::nofollow(&mut options).open(&path) else {
        return Kept::default();
    };
    let mut text = String::new();
    if file
        .take(MOST_FILE_BYTES)
        .read_to_string(&mut text)
        .is_err()
    {
        return Kept::default();
    }
    match serde_json::from_str::<Kept>(&text) {
        Ok(kept) if kept.v == VERSION && kept.skills.len() <= MOST_SKILLS => Kept {
            v: VERSION,
            skills: kept
                .skills
                .into_iter()
                .map(|(key, earlier)| {
                    let sound: Vec<Earlier> = earlier
                        .into_iter()
                        .filter(Earlier::sound)
                        .take(MOST_EARLIER)
                        .collect();
                    (key, sound)
                })
                .filter(|(_, earlier)| !earlier.is_empty())
                .collect(),
        },
        _ => Kept::default(),
    }
}

/// What a change that failed says.
fn not_kept(why: &str) -> String {
    format!(
        "Its earlier name could not be kept on this machine ({}), so its grades from before stay \
         under that name.",
        crate::shown::short(why)
    )
}

/// Takes [`LOCK`] for one change of the file of the project at `root`: the lock, and the project's
/// app state the file is written in.
fn held(root: &Path) -> Result<(crate::filelock::Held, PathBuf), String> {
    let dir = crate::appstate::made(root).map_err(|why| not_kept(&why))?;
    let mut options = std::fs::OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let file = crate::contain::nofollow(&mut options)
        .open(dir.join(LOCK))
        .map_err(|why| not_kept(&why.to_string()))?;
    let held = crate::filelock::lock_within(file, LOCK_WAIT, "a skill's earlier names")
        .map_err(|why| not_kept(&why.to_string()))?;
    Ok((held, dir))
}

/// Writes `kept` as the file in the project's app state `dir`, whole or not at all, private,
/// through no link.
fn write(dir: &Path, kept: &Kept) -> Result<(), String> {
    let text = serde_json::to_string_pretty(kept).map_err(|why| not_kept(&why.to_string()))?;
    crate::plane::write_private(dir, &dir.join(FILE), text.as_bytes())
        .map_err(|why| not_kept(&why.to_string()))
}

/// **The places the copy of `name` at `level` had before on this machine**, newest first: none
/// where it had none git could not follow. A place kept for another folder than the one at the
/// key now ([`identity`]) is none of this copy's.
pub fn of(root: &Path, level: &Level, name: &str) -> Vec<Earlier> {
    let Some(mut earlier) =
        super::after_edit::key(level, name).and_then(|key| read(root).skills.remove(&key))
    else {
        return Vec::new();
    };
    if earlier.iter().any(|one| one.folder.is_some()) {
        let now = identity(root, level, name);
        earlier.retain(|one| one.folder.is_none() || one.folder == now);
    }
    earlier
}

/// **Keeps that the copy of `old` at `from` is `new` at `to` now** (#4018), as of `now`: what a
/// rename, a move between levels and a share do. Where either side is only on this machine
/// (`skills.local/`), git cannot follow the change, so `old` at `from` is kept as an earlier
/// place, and after it `followed`, the places git followed the copy through before (name and
/// level's folder), which its history no longer reads once it leaves git. The earlier places it
/// had go along either way. Nothing is written where there is nothing to keep.
pub fn moved(
    root: &Path,
    (from, old): (&Level, &str),
    (to, new): (&Level, &str),
    followed: &[(String, String)],
    now: chrono::DateTime<chrono::Utc>,
) -> Result<(), String> {
    let (Some(was), Some(is), Some(level)) = (
        super::after_edit::key(from, old),
        super::after_edit::key(to, new),
        from.rel(),
    ) else {
        return Ok(());
    };
    let unfollowed = from.local || to.local;
    // Read before the lock is taken, so a change git follows makes no state.
    if was == is || (!unfollowed && !read(root).skills.contains_key(&was)) {
        return Ok(());
    }
    // The places it had are lent only while the folder that took the new key is there.
    let taken = identity(root, to, new);
    let under_way: Vec<String> = super::underway::of(root, from, old)
        .into_iter()
        .take(MOST_UNDER_WAY)
        .collect();
    let (_held, dir) = held(root)?;
    let mut kept = read(root);
    let mut earlier = kept.skills.remove(&was).unwrap_or_default();
    if unfollowed {
        let until = now.to_rfc3339();
        let left = std::iter::once((old.to_owned(), level, under_way))
            .chain(
                followed
                    .iter()
                    .cloned()
                    .map(|(name, level)| (name, level, Vec::new())),
            )
            .map(|(name, level, grades)| Earlier {
                name,
                level,
                until: until.clone(),
                grades,
                folder: None,
            });
        earlier.splice(0..0, left);
    }
    // A place kept for another folder than the one that moved is none of its own (#4104): a
    // rename or a move keeps the folder, so it is the one at the new key now.
    earlier.retain(|one| one.folder.is_none() || one.folder == taken);
    earlier.truncate(MOST_EARLIER);
    for one in &mut earlier {
        one.folder.clone_from(&taken);
    }
    if !earlier.is_empty() {
        if !kept.skills.contains_key(&is) && kept.skills.len() >= MOST_SKILLS {
            return Err(not_kept(&format!(
                "it keeps the earlier names of {MOST_SKILLS} skills already"
            )));
        }
        kept.skills.insert(is, earlier);
    }
    kept.v = VERSION;
    write(&dir, &kept)
}

/// **Takes the folder at the key of the copy of `name` at `level` as the one its earlier places
/// are kept for** ([`identity`]): what a Restore does, as the folder it brings back may be
/// another than the one deleted. Nothing where it had none.
pub fn refound(root: &Path, level: &Level, name: &str) -> Result<(), String> {
    let Some(key) = super::after_edit::key(level, name) else {
        return Ok(());
    };
    if !read(root).skills.contains_key(&key) {
        return Ok(());
    }
    let now = identity(root, level, name);
    let (_held, dir) = held(root)?;
    let mut kept = read(root);
    let Some(earlier) = kept.skills.get_mut(&key) else {
        return Ok(());
    };
    for one in earlier.iter_mut() {
        one.folder.clone_from(&now);
    }
    kept.v = VERSION;
    write(&dir, &kept)
}

/// **Drops the earlier places of the copy of `name` at `level`**: what a new skill made at its
/// name does. Nothing where it had none.
pub fn dropped(root: &Path, level: &Level, name: &str) -> Result<(), String> {
    let Some(key) = super::after_edit::key(level, name) else {
        return Ok(());
    };
    if !read(root).skills.contains_key(&key) {
        return Ok(());
    }
    let (_held, dir) = held(root)?;
    let mut kept = read(root);
    if kept.skills.remove(&key).is_none() {
        return Ok(());
    }
    kept.v = VERSION;
    write(&dir, &kept)
}

#[cfg(test)]
mod tests;

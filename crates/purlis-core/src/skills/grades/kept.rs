// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Each grade's chat, kept to open from its scorecard** (SK-59, #3239; spec #2762 decision 43,
//! "each fixture's chat one click away"): a grade's chat is drawn nowhere while it runs, and its
//! throwaway piece goes when it closes, so purlis keeps what its terminal showed as it ended, the
//! screen and its scrollback as the escape sequences that draw them
//! (`purlis_core::engine`'s snapshot). A row of the scorecard opens it read-only in a tab.
//!
//! - **Kept on this machine**, in its app state of the project (`crate::appstate`),
//!   `<data>/app-state/<clone key>/skill-grade-chats/<skill>/<grade>/<chat>.screen`:
//!   the chat ran here, and what it showed is this machine's to keep. A teammate who pulls the
//!   eval record sees the grade and not its chats, and the scorecard says so.
//! - **Kept for a skill's newest grades** ([`KEPT_GRADES`]): once a grade of the skill ends,
//!   the chats of its older grades go ([`sweep`]). A grade's id is a ULID, so its name sorts
//!   by when it began.
//! - **Read as this machine wrote it**: each name a plain one, each folder a folder of its own
//!   (never a link), each file at most [`MOST_SCREEN_BYTES`] and opened following no link, so a
//!   file put there by anything else is never shown.
//!
//! - **A router's grade keeps its chats the same way** (#3617), in a store of its own,
//!   `<data>/app-state/<clone key>/router-grade-chats/<router>/<workspace>/<persona>/<grade>/<chat>.screen`
//!   (`_` for no workspace or no persona, a name neither can take), kept for the
//!   [`KEPT_GRADES`] newest grades of the router chosen in that scope, as a router's grades are
//!   read by their scope ([`Of::Router`]).
//!
//! **Tier:** Machine, device-bound, transient, never committed: what this machine's chats showed.

use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};

/// **A name this module puts in a path**: a plain one ([`super::plain`]) that is more than dots,
/// so `.` and `..` never name a folder of it.
fn named(word: &str) -> bool {
    super::plain(word) && !word.bytes().all(|b| b == b'.')
}

/// The folder of this machine's app state of the project the kept chats are in.
pub const DIR_NAME: &str = "skill-grade-chats";

/// The folder of this machine's app state of the project a router's grades' chats are in (#3617).
pub const ROUTERS_DIR_NAME: &str = "router-grade-chats";

/// What a folder name of [`ROUTERS_DIR_NAME`] says where a router was chosen by no workspace, or
/// by no persona: a name neither can take, as each starts with a letter or a digit.
const NONE: &str = "_";

/// **Whose grades' chats are kept** (#3617): a skill's, or a router's chosen in one scope, whose
/// grades are read by that scope (`super::routers::grades_of`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Of {
    Skill(String),
    Router {
        router: String,
        workspace: Option<String>,
        persona: Option<String>,
    },
}

impl Of {
    /// The folders, from this machine's app state of the project down, its grades' folders are
    /// in, each a plain name, or `None` where one is not.
    fn folders(&self) -> Option<Vec<&str>> {
        let folders = match self {
            Of::Skill(skill) => vec![DIR_NAME, skill.as_str()],
            Of::Router {
                router,
                workspace,
                persona,
            } => vec![
                ROUTERS_DIR_NAME,
                router.as_str(),
                workspace.as_deref().unwrap_or(NONE),
                persona.as_deref().unwrap_or(NONE),
            ],
        };
        folders.iter().all(|one| named(one)).then_some(folders)
    }
}

/// How many of a skill's grades keep their chats: the newest.
pub const KEPT_GRADES: usize = 3;

/// The most bytes of one chat's screen kept: a screen past it is not kept, since cutting it
/// would cut the sequences that draw it.
pub const MOST_SCREEN_BYTES: usize = 4 * 1024 * 1024;

/// The most grade folders of one skill looked at: a folder with more is swept no further.
const MOST_GRADE_FOLDERS: usize = 600;

/// The file a kept chat's screen is in.
const SCREEN: &str = "screen";

/// The folder of `of`'s kept chats in the project at `root`, and each folder on the way to it
/// from this machine's app state, or `None` for a name that is not a plain one, or where this
/// machine has no data home.
fn owner_dirs(root: &Path, of: &Of) -> Option<Vec<PathBuf>> {
    let mut at = crate::appstate::dir(root)?;
    let mut dirs = Vec::new();
    for folder in of.folders()? {
        at = at.join(folder);
        dirs.push(at.clone());
    }
    Some(dirs)
}

/// The folder of `of`'s kept chats in the project at `root` ([`owner_dirs`]).
fn owner_dir(root: &Path, of: &Of) -> Option<PathBuf> {
    owner_dirs(root, of)?.pop()
}

/// The file of `chat`'s screen in `of`'s grade `grade`, and each folder on the way to it from
/// this machine's app state, or `None` where a name is not plain.
fn screen_path(root: &Path, of: &Of, grade: &str, chat: &str) -> Option<(PathBuf, Vec<PathBuf>)> {
    if !(named(grade) && named(chat)) {
        return None;
    }
    let mut dirs = owner_dirs(root, of)?;
    let grade_dir = dirs.last()?.join(grade);
    dirs.push(grade_dir.clone());
    Some((grade_dir.join(format!("{chat}.{SCREEN}")), dirs))
}

/// `dir` made where it is not, private, and refused where it is not a folder of its own.
fn made_private(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    if !std::fs::symlink_metadata(dir)?.is_dir() {
        return Err(std::io::Error::other("the folder is not a folder"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

/// **Keeps `screen`, what chat `chat` of `skill`'s grade `grade` showed as it ended**, in the
/// project at `root`. Refused for a name that is not plain and a screen past
/// [`MOST_SCREEN_BYTES`]; written whole, private, following no link.
pub fn keep(
    root: &Path,
    skill: &str,
    grade: &str,
    chat: &str,
    screen: &str,
) -> std::io::Result<()> {
    keep_of(root, &Of::Skill(skill.to_owned()), grade, chat, screen)
}

/// [`keep`], of `of`'s grade `grade`: a skill's, or a router's (#3617).
pub fn keep_of(root: &Path, of: &Of, grade: &str, chat: &str, screen: &str) -> std::io::Result<()> {
    crate::appstate::made(root).map_err(std::io::Error::other)?;
    let (path, on_the_way) = screen_path(root, of, grade, chat)
        .ok_or_else(|| std::io::Error::other("a name is not a plain one"))?;
    if screen.len() > MOST_SCREEN_BYTES {
        return Err(std::io::Error::other("the screen is too big to keep"));
    }
    // The record's folder, the skill's (or the router's and its scope's) and the grade's, each
    // made private and checked to be no link before the next is made inside it.
    for dir in &on_the_way {
        made_private(dir)?;
    }
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut file = crate::contain::nofollow(&mut options).open(&path)?;
    file.write_all(screen.as_bytes())
}

/// **What chat `chat` of `skill`'s grade `grade` showed as it ended**, where this machine kept
/// it: read through no link, at most [`MOST_SCREEN_BYTES`].
pub fn kept(root: &Path, skill: &str, grade: &str, chat: &str) -> Option<String> {
    kept_of(root, &Of::Skill(skill.to_owned()), grade, chat)
}

/// [`kept`], of `of`'s grade `grade`: a skill's, or a router's (#3617).
pub fn kept_of(root: &Path, of: &Of, grade: &str, chat: &str) -> Option<String> {
    let (path, folders) = screen_path(root, of, grade, chat)?;
    if !folders
        .iter()
        .all(|dir| std::fs::symlink_metadata(dir).is_ok_and(|meta| meta.is_dir()))
    {
        return None;
    }
    let meta = std::fs::symlink_metadata(&path).ok()?;
    if !meta.is_file() || meta.len() > MOST_SCREEN_BYTES as u64 {
        return None;
    }
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    let file = crate::contain::nofollow(&mut options).open(&path).ok()?;
    let mut text = String::new();
    file.take(MOST_SCREEN_BYTES as u64)
        .read_to_string(&mut text)
        .ok()?;
    Some(text)
}

/// **The grades of `of` whose chats this machine keeps**, newest first.
fn grades_kept(root: &Path, of: &Of) -> Vec<String> {
    let Some(dir) = owner_dir(root, of) else {
        return Vec::new();
    };
    if !std::fs::symlink_metadata(&dir).is_ok_and(|meta| meta.is_dir()) {
        return Vec::new();
    }
    let Ok(reader) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = reader
        .take(MOST_GRADE_FOLDERS + 1)
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| named(name))
        .collect();
    if names.len() > MOST_GRADE_FOLDERS {
        return Vec::new();
    }
    names.sort();
    names.reverse();
    names
}

/// **Takes away the chats of every grade of `skill` but its [`KEPT_GRADES`] newest**, in the
/// project at `root`: what a grade's end does. Only folders this module names, never through a
/// link. Answers the grades whose chats went.
pub fn sweep(root: &Path, skill: &str) -> Vec<String> {
    sweep_of(root, &Of::Skill(skill.to_owned()))
}

/// [`sweep`], of `of`'s grades: a skill's, or a router's chosen in one scope (#3617).
pub fn sweep_of(root: &Path, of: &Of) -> Vec<String> {
    let Some(dir) = owner_dir(root, of) else {
        return Vec::new();
    };
    let mut gone = Vec::new();
    for grade in grades_kept(root, of).into_iter().skip(KEPT_GRADES) {
        let folder = dir.join(&grade);
        if std::fs::symlink_metadata(&folder).is_ok_and(|meta| meta.is_dir())
            && std::fs::remove_dir_all(&folder).is_ok()
        {
            gone.push(grade);
        }
    }
    gone
}

/// **Keeps `old`'s kept chats under its new name `new`** (#3918): what a rename of the skill does
/// on this machine, where no other copy keeps the name `old`, so a grade from before it opens
/// its chats from the skill's tab under its new name. Each grade folder of `old` goes to
/// `new`'s; one that could not go, or that `new` holds already, stays where it is, and `old`'s
/// folder goes only once it is empty. Then `new`'s are swept to its [`KEPT_GRADES`] newest. Only
/// folders this module names, never through a link; nothing where either name is not plain or
/// `old` keeps none.
pub fn renamed(root: &Path, old: &str, new: &str) {
    let (old, new) = (Of::Skill(old.to_owned()), Of::Skill(new.to_owned()));
    let (Some(from), Some(to)) = (owner_dir(root, &old), owner_dir(root, &new)) else {
        return;
    };
    if old == new || !std::fs::symlink_metadata(&from).is_ok_and(|meta| meta.is_dir()) {
        return;
    }
    if made_private(&to).is_err() {
        return;
    }
    for grade in grades_kept(root, &old) {
        let there = to.join(&grade);
        if std::fs::symlink_metadata(&there).is_err() {
            let _ = std::fs::rename(from.join(&grade), there);
        }
    }
    // Only an empty folder goes: what was not moved is still kept under the old name.
    let _ = std::fs::remove_dir(&from);
    sweep_of(root, &new);
}

#[cfg(test)]
mod tests;

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

/// How many of a skill's grades keep their chats: the newest.
pub const KEPT_GRADES: usize = 3;

/// The most bytes of one chat's screen kept: a screen past it is not kept, since cutting it
/// would cut the sequences that draw it.
pub const MOST_SCREEN_BYTES: usize = 4 * 1024 * 1024;

/// The most grade folders of one skill looked at: a folder with more is swept no further.
const MOST_GRADE_FOLDERS: usize = 600;

/// The file a kept chat's screen is in.
const SCREEN: &str = "screen";

/// The folder of `skill`'s kept chats in the project at `root`, or `None` for a name that is not
/// a plain one, or where this machine has no data home.
fn skill_dir(root: &Path, skill: &str) -> Option<PathBuf> {
    if !named(skill) {
        return None;
    }
    Some(crate::appstate::dir(root)?.join(DIR_NAME).join(skill))
}

/// The file of `chat`'s screen in `skill`'s grade `grade`, or `None` where a name is not plain.
fn screen_path(root: &Path, skill: &str, grade: &str, chat: &str) -> Option<PathBuf> {
    (named(grade) && named(chat))
        .then(|| skill_dir(root, skill))
        .flatten()
        .map(|dir| dir.join(grade).join(format!("{chat}.{SCREEN}")))
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
    crate::appstate::made(root).map_err(std::io::Error::other)?;
    let path = screen_path(root, skill, grade, chat)
        .ok_or_else(|| std::io::Error::other("a name is not a plain one"))?;
    if screen.len() > MOST_SCREEN_BYTES {
        return Err(std::io::Error::other("the screen is too big to keep"));
    }
    // The record's folder, the skill's and the grade's, each made private and checked to be no
    // link before the next is made inside it.
    let grade_dir = path
        .parent()
        .ok_or_else(|| std::io::Error::other("the screen has no folder"))?;
    let mut on_the_way: Vec<&Path> = grade_dir.ancestors().take(3).collect();
    on_the_way.reverse();
    for dir in on_the_way {
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
    let path = screen_path(root, skill, grade, chat)?;
    let folders = [
        crate::appstate::dir(root)?.join(DIR_NAME),
        skill_dir(root, skill)?,
        path.parent()?.to_path_buf(),
    ];
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

/// **The grades of `skill` whose chats this machine keeps**, newest first.
fn grades_kept(root: &Path, skill: &str) -> Vec<String> {
    let Some(dir) = skill_dir(root, skill) else {
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
    let Some(dir) = skill_dir(root, skill) else {
        return Vec::new();
    };
    let mut gone = Vec::new();
    for grade in grades_kept(root, skill).into_iter().skip(KEPT_GRADES) {
        let folder = dir.join(&grade);
        if std::fs::symlink_metadata(&folder).is_ok_and(|meta| meta.is_dir())
            && std::fs::remove_dir_all(&folder).is_ok()
        {
            gone.push(grade);
        }
    }
    gone
}

#[cfg(test)]
mod tests;

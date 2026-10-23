// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The sets' folders, read and removed through handles** (SK-24). Every folder a tidy
//! looks in is opened as a folder itself (`O_DIRECTORY | O_NOFOLLOW`), listed through that
//! handle, and what it removes is removed relative to it (`unlinkat`). A link put in place of
//! a folder after it was checked is never followed, because nothing goes back through its path.

use std::os::fd::{AsFd, OwnedFd};
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rustix::fs::{AtFlags, CWD, FileType, Mode, OFlags};

/// An entry of a folder, as [`entries`] reads it: never through a link.
pub(crate) struct Entry {
    pub(crate) name: String,
    pub(crate) kind: FileType,
    pub(crate) modified: Option<SystemTime>,
}

fn folder_flags() -> OFlags {
    OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC
}

/// `at`, opened as a folder itself.
pub(crate) fn folder(at: &Path) -> std::io::Result<OwnedFd> {
    Ok(rustix::fs::openat(CWD, at, folder_flags(), Mode::empty())?)
}

/// `name` in `dir`, opened as a folder itself.
pub(crate) fn folder_in(dir: impl AsFd, name: &str) -> std::io::Result<OwnedFd> {
    Ok(rustix::fs::openat(
        dir,
        name,
        folder_flags(),
        Mode::empty(),
    )?)
}

/// `name` in `dir`, opened to be written, made where it is not there: never through a link.
pub(crate) fn file_in(dir: impl AsFd, name: &str) -> std::io::Result<std::fs::File> {
    let flags = OFlags::WRONLY | OFlags::CREATE | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    let fd = rustix::fs::openat(dir, name, flags, Mode::from_raw_mode(0o644))?;
    Ok(std::fs::File::from(fd))
}

/// `name` in `dir`, made new to be written: refused where anything of that name is there, a
/// link included, so the file is the caller's own from its first byte.
pub(crate) fn new_file_in(dir: impl AsFd, name: &str) -> std::io::Result<std::fs::File> {
    let flags = OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    let fd = rustix::fs::openat(dir, name, flags, Mode::from_raw_mode(0o644))?;
    Ok(std::fs::File::from(fd))
}

/// The folder `rel` under `dir`, each of its parts opened as a folder itself in the one before.
pub(crate) fn folder_under(dir: impl AsFd, rel: &Path) -> std::io::Result<OwnedFd> {
    let mut parts = rel.components().filter_map(|part| match part {
        std::path::Component::Normal(name) => name.to_str(),
        _ => None,
    });
    let first = parts
        .next()
        .ok_or_else(|| std::io::Error::other("no folder named"))?;
    let mut at = folder_in(&dir, first)?;
    for part in parts {
        at = folder_in(&at, part)?;
    }
    Ok(at)
}

/// The folder `name` made in `dir`: refused where anything of that name is there.
pub(crate) fn make_folder_in(dir: impl AsFd, name: &str) -> std::io::Result<()> {
    Ok(rustix::fs::mkdirat(dir, name, Mode::from_raw_mode(0o755))?)
}

/// What `name` in `dir` is, itself, and when it last changed; `None` where it is not there.
pub(crate) fn entry_in(dir: impl AsFd, name: &str) -> std::io::Result<Option<Entry>> {
    match rustix::fs::statat(dir, name, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(stat) => Ok(Some(Entry {
            name: name.to_owned(),
            kind: FileType::from_raw_mode(stat.st_mode as rustix::fs::RawMode),
            modified: u64::try_from(stat.st_mtime)
                .ok()
                .map(|seconds| UNIX_EPOCH + Duration::from_secs(seconds)),
        })),
        Err(rustix::io::Errno::NOENT) => Ok(None),
        Err(why) => Err(why.into()),
    }
}

/// Every entry of the folder `dir` with a name that is text, through its handle.
pub(crate) fn entries(dir: impl AsFd) -> std::io::Result<Vec<Entry>> {
    let mut all = Vec::new();
    for entry in rustix::fs::Dir::read_from(&dir)? {
        let entry = entry?;
        let Ok(name) = entry.file_name().to_str() else {
            continue;
        };
        if name == "." || name == ".." {
            continue;
        }
        if let Some(entry) = entry_in(&dir, name)? {
            all.push(entry);
        }
    }
    Ok(all)
}

/// Removes the file or link `name` in `dir`, itself.
pub(crate) fn remove_file_in(dir: impl AsFd, name: &str) -> std::io::Result<()> {
    Ok(rustix::fs::unlinkat(dir, name, AtFlags::empty())?)
}

/// Removes the empty folder `name` in `dir`.
pub(crate) fn remove_empty_folder_in(dir: impl AsFd, name: &str) -> std::io::Result<()> {
    Ok(rustix::fs::unlinkat(dir, name, AtFlags::REMOVEDIR)?)
}

/// How many folders deep [`remove_tree_in`] goes: a set's folder is two (`skills/<name>`), so a
/// tree deeper than this is not one purlis made, and it is left in place.
pub(crate) const DEEPEST: usize = 64;

/// **Removes the folder `name` in `dir` and all it holds**, each folder in it opened as a folder
/// itself before anything in it is removed: a link anywhere in it is removed as a link, and a
/// folder that has become a link is refused, never followed. A tree more than [`DEEPEST`]
/// folders deep is refused, and what is below that depth is left in place.
pub(crate) fn remove_tree_in(dir: impl AsFd, name: &str) -> std::io::Result<()> {
    remove_tree_below(dir, name, 0)
}

fn remove_tree_below(dir: impl AsFd, name: &str, depth: usize) -> std::io::Result<()> {
    if depth >= DEEPEST {
        return Err(std::io::Error::other(format!(
            "it is more than {DEEPEST} folders deep, so what is below that was left in place"
        )));
    }
    let folder = folder_in(&dir, name)?;
    for entry in entries(&folder)? {
        if entry.kind == FileType::Directory {
            remove_tree_below(&folder, &entry.name, depth + 1)?;
        } else {
            remove_file_in(&folder, &entry.name)?;
        }
    }
    remove_empty_folder_in(dir, name)
}

/// Whether an error says what was opened as a folder is a link or a file.
pub(crate) fn not_a_folder(why: &std::io::Error) -> bool {
    why.raw_os_error().is_some_and(|code| {
        code == rustix::io::Errno::LOOP.raw_os_error()
            || code == rustix::io::Errno::NOTDIR.raw_os_error()
    })
}

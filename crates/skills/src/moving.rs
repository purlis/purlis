// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Moving a skill's folder through handles** (#2770, SK-36): both sides opened one folder at a
//! time with `O_NOFOLLOW`, only a plain folder or file moved, and the move one `renameat` between
//! the two held folders that never replaces anything. A link anywhere on either way refuses the
//! move, and one put there after a look is never followed, as no path is walked twice.
//!
//! The one mover of a skill's folder: Share here, and purlis-core's rename, move, delete and
//! restore.

use std::path::Path;

use crate::beside;

#[cfg(unix)]
use rustix::fd::OwnedFd;

/// The flags every folder on the way is opened with: as a folder, never through a link.
#[cfg(unix)]
pub const FOLDER: rustix::fs::OFlags = rustix::fs::OFlags::RDONLY
    .union(rustix::fs::OFlags::DIRECTORY)
    .union(rustix::fs::OFlags::CLOEXEC)
    .union(rustix::fs::OFlags::NOFOLLOW);

/// The folder `names` names under `root`, opened one component at a time following no link,
/// making those from the `make_from`th on (counting from 1) that are not there. `root` itself is
/// opened as it is: a folder purlis trusts (the plane's root, its state folder).
#[cfg(unix)]
pub fn open_dir(root: &Path, names: &[String], make_from: usize) -> rustix::io::Result<OwnedFd> {
    use rustix::fs::{CWD, Mode, OFlags, mkdirat, openat};
    let mut at = openat(
        CWD,
        root,
        FOLDER.difference(OFlags::NOFOLLOW),
        Mode::empty(),
    )?;
    for (index, name) in names.iter().enumerate() {
        at = match openat(&at, name.as_str(), FOLDER, Mode::empty()) {
            Ok(next) => next,
            Err(rustix::io::Errno::NOENT) if index + 1 >= make_from => {
                mkdirat(&at, name.as_str(), Mode::from_raw_mode(0o755))?;
                openat(&at, name.as_str(), FOLDER, Mode::empty())?
            }
            Err(why) => return Err(why),
        };
    }
    Ok(at)
}

/// The names of `path`'s folders under `root`, and its own name, or why it is no plain path
/// under it.
fn steps(root: &Path, path: &Path) -> Result<(Vec<String>, String), String> {
    let inside = path
        .strip_prefix(root)
        .map_err(|_| "it is outside where it may be".to_owned())?;
    let mut names = Vec::new();
    for part in inside.components() {
        match part {
            std::path::Component::Normal(name) => names.push(name.to_string_lossy().into_owned()),
            _ => return Err("its path is not a plain one".to_owned()),
        }
    }
    let leaf = names
        .pop()
        .ok_or_else(|| "its path names nothing".to_owned())?;
    Ok((names, leaf))
}

/// **Moves the entry at `from`, under `from_root`, to `to`, under `to_root`**, never over
/// anything there and following no link on either way; folders of `to` from the `make_from`th on
/// are made where they are not there. Refused where it is not a plain folder or file.
#[cfg(unix)]
pub fn move_between(
    from_root: &Path,
    from: &Path,
    to_root: &Path,
    to: &Path,
    make_from: usize,
) -> Result<(), String> {
    use rustix::fs::{AtFlags, FileType, statat};
    let (from_dirs, from_leaf) = steps(from_root, from)?;
    let (to_dirs, to_leaf) = steps(to_root, to)?;
    let from_at = open_dir(from_root, &from_dirs, usize::MAX)
        .map_err(|why| format!("its folder could not be opened ({why})"))?;
    let stat = statat(&from_at, from_leaf.as_str(), AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|why| format!("it is not there ({why})"))?;
    let kind = FileType::from_raw_mode(stat.st_mode);
    if kind != FileType::Directory && kind != FileType::RegularFile {
        return Err("it is not a plain folder or file, so purlis does not move it".to_owned());
    }
    let to_at = open_dir(to_root, &to_dirs, make_from)
        .map_err(|why| format!("the folder it goes into could not be made ({why})"))?;
    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios"
    ))]
    match rustix::fs::renameat_with(
        &from_at,
        from_leaf.as_str(),
        &to_at,
        to_leaf.as_str(),
        rustix::fs::RenameFlags::NOREPLACE,
    ) {
        Ok(()) => return Ok(()),
        Err(rustix::io::Errno::EXIST) => {
            return Err("something of its name is already where it goes".to_owned());
        }
        Err(rustix::io::Errno::INVAL | rustix::io::Errno::NOSYS | rustix::io::Errno::NOTSUP) => {}
        Err(why) => return Err(format!("it could not be moved ({why})")),
    }
    if statat(&to_at, to_leaf.as_str(), AtFlags::SYMLINK_NOFOLLOW).is_ok() {
        return Err("something of its name is already where it goes".to_owned());
    }
    rustix::fs::renameat(&from_at, from_leaf.as_str(), &to_at, to_leaf.as_str())
        .map_err(|why| format!("it could not be moved ({why})"))
}

/// [`move_between`] where no folder can be held open: refused, as a skill is never moved by a
/// path a link could redirect.
#[cfg(not(unix))]
pub fn move_between(_: &Path, _: &Path, _: &Path, _: &Path, _: usize) -> Result<(), String> {
    Err("a skill is moved only where its folders can be held open".to_owned())
}

/// **Moves a skill's folder, and the file beside it** ([`beside`]), from `from` to `to`, both in
/// the plane, each by [`move_between`]. Where the file beside it cannot be moved (a link, say),
/// the folder is put back the same way and nothing is moved.
pub fn move_skill(plane: &Path, from: &Path, to: &Path, make_from: usize) -> Result<(), String> {
    move_between(plane, from, plane, to, make_from)?;
    let (toml_from, toml_to) = (beside(from), beside(to));
    if toml_from.symlink_metadata().is_ok()
        && let Err(why) = move_between(plane, &toml_from, plane, &toml_to, usize::MAX)
    {
        // The folder moved and the file beside it did not: put the folder back.
        let _ = move_between(plane, to, plane, from, usize::MAX);
        return Err(why);
    }
    Ok(())
}

#[cfg(test)]
mod tests;

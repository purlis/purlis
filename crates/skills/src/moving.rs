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

/// **Why a move did not happen**, and whether anything moved all the same.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unmoved {
    /// Refused or failed with everything where it was, or put back where it was.
    Nothing(String),
    /// Something moved and could not be put back: the sentence says what is where.
    Stranded(String),
}

impl std::fmt::Display for Unmoved {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Nothing(why) | Self::Stranded(why) => f.write_str(why),
        }
    }
}

impl From<Unmoved> for String {
    fn from(unmoved: Unmoved) -> Self {
        unmoved.to_string()
    }
}

/// `path` under `root`, as the person reads it.
fn shown(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}

/// Renames `from_leaf` in `from_at` to `to_leaf` in `to_at`, never over anything there.
#[cfg(unix)]
fn rename_new(
    from_at: &OwnedFd,
    from_leaf: &str,
    to_at: &OwnedFd,
    to_leaf: &str,
) -> Result<(), String> {
    use rustix::fs::{AtFlags, statat};
    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios"
    ))]
    match rustix::fs::renameat_with(
        from_at,
        from_leaf,
        to_at,
        to_leaf,
        rustix::fs::RenameFlags::NOREPLACE,
    ) {
        Ok(()) => return Ok(()),
        Err(rustix::io::Errno::EXIST) => {
            return Err("something of its name is already where it goes".to_owned());
        }
        Err(rustix::io::Errno::INVAL | rustix::io::Errno::NOSYS | rustix::io::Errno::NOTSUP) => {}
        Err(why) => return Err(format!("it could not be moved ({why})")),
    }
    if statat(to_at, to_leaf, AtFlags::SYMLINK_NOFOLLOW).is_ok() {
        return Err("something of its name is already where it goes".to_owned());
    }
    rustix::fs::renameat(from_at, from_leaf, to_at, to_leaf)
        .map_err(|why| format!("it could not be moved ({why})"))
}

/// **Moves the entry at `from`, under `from_root`, to `to`, under `to_root`**, never over
/// anything there and following no link on either way; folders of `to` from the `make_from`th on
/// are made where they are not there. Refused where it is not a plain folder or file.
///
/// **It proves it moved what it looked at** (SK-45): after the rename the entry where it went is
/// looked at again, without following a link, and where it is not the one looked at before (its
/// device, inode or kind differ: swapped in that instant) it is put back and the move refused.
/// [`Unmoved::Stranded`] where that put-back fails, naming where the entry is.
#[cfg(unix)]
pub fn move_between(
    from_root: &Path,
    from: &Path,
    to_root: &Path,
    to: &Path,
    make_from: usize,
) -> Result<(), Unmoved> {
    use rustix::fs::{AtFlags, FileType, statat};
    let (from_dirs, from_leaf) = steps(from_root, from).map_err(Unmoved::Nothing)?;
    let (to_dirs, to_leaf) = steps(to_root, to).map_err(Unmoved::Nothing)?;
    let from_at = open_dir(from_root, &from_dirs, usize::MAX)
        .map_err(|why| Unmoved::Nothing(format!("its folder could not be opened ({why})")))?;
    let looked = statat(&from_at, from_leaf.as_str(), AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|why| Unmoved::Nothing(format!("it is not there ({why})")))?;
    let kind = FileType::from_raw_mode(looked.st_mode);
    if kind != FileType::Directory && kind != FileType::RegularFile {
        return Err(Unmoved::Nothing(
            "it is not a plain folder or file, so purlis does not move it".to_owned(),
        ));
    }
    let to_at = open_dir(to_root, &to_dirs, make_from).map_err(|why| {
        Unmoved::Nothing(format!("the folder it goes into could not be made ({why})"))
    })?;
    #[cfg(test)]
    hook::before_rename(from, to);
    rename_new(&from_at, &from_leaf, &to_at, &to_leaf).map_err(Unmoved::Nothing)?;
    let same = statat(&to_at, to_leaf.as_str(), AtFlags::SYMLINK_NOFOLLOW).is_ok_and(|moved| {
        moved.st_dev == looked.st_dev
            && moved.st_ino == looked.st_ino
            && FileType::from_raw_mode(moved.st_mode) == kind
    });
    if same {
        return Ok(());
    }
    #[cfg(test)]
    hook::before_rename(to, from);
    match rename_new(&to_at, &to_leaf, &from_at, &from_leaf) {
        Ok(()) => Err(Unmoved::Nothing(
            "it changed as it was moved, so it was put back".to_owned(),
        )),
        Err(why) => Err(Unmoved::Stranded(format!(
            "it changed as it was moved and could not be put back ({why}): what took its \
             place is at {}",
            shown(to_root, to)
        ))),
    }
}

/// [`move_between`] where no folder can be held open: refused, as a skill is never moved by a
/// path a link could redirect.
#[cfg(not(unix))]
pub fn move_between(_: &Path, _: &Path, _: &Path, _: &Path, _: usize) -> Result<(), Unmoved> {
    Err(Unmoved::Nothing(
        "a skill is moved only where its folders can be held open".to_owned(),
    ))
}

/// **Moves a skill's folder, and the file beside it** ([`beside`]), from `from` to `to`, both in
/// the plane, each by [`move_between`]. Where the file beside it cannot be moved (a link, say),
/// the folder is put back the same way and nothing is moved; where that put-back fails too,
/// [`Unmoved::Stranded`] says the folder is where it went and the file is where it was.
pub fn move_skill(plane: &Path, from: &Path, to: &Path, make_from: usize) -> Result<(), Unmoved> {
    move_between(plane, from, plane, to, make_from)?;
    let (toml_from, toml_to) = (beside(from), beside(to));
    if toml_from.symlink_metadata().is_ok()
        && let Err(why) = move_between(plane, &toml_from, plane, &toml_to, usize::MAX)
    {
        // The folder moved and the file beside it did not: put the folder back.
        return match (move_between(plane, to, plane, from, usize::MAX), why) {
            (Ok(()), why) => Err(why),
            (Err(back), why) => Err(Unmoved::Stranded(format!(
                "its folder was moved to {} and could not be put back ({back}), and {} was not \
                 moved ({why})",
                shown(plane, to),
                shown(plane, &toml_from)
            ))),
        };
    }
    Ok(())
}

/// A test's way in between the look at the leaf and its rename: the instant a swap would land.
///
/// Thread-local, so it fires only in the test that set it: `cargo test` runs tests on a pool.
#[cfg(test)]
pub(crate) mod hook {
    use std::cell::RefCell;
    use std::path::Path;

    type Hook = Box<dyn FnMut(&Path, &Path)>;

    thread_local! {
        static BEFORE_RENAME: RefCell<Option<Hook>> = const { RefCell::new(None) };
    }

    /// Run `f(from, to)` after the leaf is looked at and before it is renamed, for every
    /// [`super::move_between`] on this thread until the returned guard drops.
    pub(crate) fn set(f: impl FnMut(&Path, &Path) + 'static) -> Unset {
        BEFORE_RENAME.with(|h| *h.borrow_mut() = Some(Box::new(f)));
        Unset
    }

    pub(crate) struct Unset;

    impl Drop for Unset {
        fn drop(&mut self) {
            BEFORE_RENAME.with(|h| *h.borrow_mut() = None);
        }
    }

    pub(crate) fn before_rename(from: &Path, to: &Path) {
        let taken = BEFORE_RENAME.with(|h| h.borrow_mut().take());
        let Some(mut f) = taken else {
            return;
        };
        f(from, to);
        BEFORE_RENAME.with(|h| {
            let mut slot = h.borrow_mut();
            if slot.is_none() {
                *slot = Some(f);
            }
        });
    }
}

#[cfg(test)]
mod tests;

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **One folder, read whole, outside any branch** (#5183): a skill's folder, which the Skill
//! studio's Files tab shows nested and opens a file of. The folder may be the project's, the
//! app's own (a built-in skill) or a harness's (a third-party skill); which folder is the
//! caller's to decide (`skills::listed_folder`), and this module only reads inside it.
//!
//! Read as a branch's folder is: each folder listed through a descriptor opened one component
//! at a time with no link followed, a file opened the same way, and what a file is decided by
//! its bytes ([`super::Opened`]). A link is listed and never followed or opened.

use std::path::{Path, PathBuf};

use super::{Kind, Opened};

/// The most entries a walk lists: a skill's folder holds a few dozen.
pub const MOST: usize = 2_000;

/// One entry of a walked folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Walked {
    /// Its path in the folder, `/` between its parts.
    pub path: String,
    pub kind: Kind,
    /// Why it does not open, as the window shows it; `None` for a file that opens and a folder.
    pub refused: Option<String>,
}

/// A folder, walked: every entry, each folder's before its files and each kind in the order a
/// person reads names, a folder's entries straight after it; the first [`MOST`], and how many
/// more were listed past them.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Walk {
    pub entries: Vec<Walked>,
    pub more: usize,
}

/// `folder`, walked: see [`Walk`]. Git's own `.git` is left out, a link is listed with why it
/// does not open and never followed, and something that is not a regular file (a FIFO, a
/// socket) is listed as a file that does not open, and a folder that cannot be listed as a row
/// that says why. Past [`MOST`] entries no further folder is listed, and what the folders listed
/// hold past them is counted in `more`: at least that many are not shown.
pub fn walk(folder: &Path) -> Result<Walk, String> {
    let mut walk = Walk::default();
    // Depth first, each folder's entries straight after it: what is waiting is a stack of the
    // entries still to place, in reverse.
    let mut stack: Vec<(PathBuf, super::Listed)> = Vec::new();
    let list = |relative: &Path| -> Result<Vec<super::Listed>, String> {
        let mut listed = super::listing(folder, relative).map_err(|e| {
            if relative.as_os_str().is_empty() {
                format!("purlis could not read the folder: {e}")
            } else {
                format!("purlis could not read '{}': {e}", super::slashed(relative))
            }
        })?;
        listed.retain(|one| !super::gits(Path::new(&one.name)));
        listed.sort_by(|a, b| {
            (a.kind != Kind::Folder)
                .cmp(&(b.kind != Kind::Folder))
                .then_with(|| super::natural(&a.name.to_string_lossy(), &b.name.to_string_lossy()))
        });
        Ok(listed)
    };
    stack.extend(
        list(Path::new(""))?
            .into_iter()
            .rev()
            .map(|one| (PathBuf::new(), one)),
    );
    while let Some((parent, one)) = stack.pop() {
        if walk.entries.len() >= MOST {
            walk.more += 1;
            continue;
        }
        let relative = parent.join(&one.name);
        let path = super::slashed(&relative);
        // A folder that cannot be listed is one row that says why, never the whole walk.
        let inside = if one.kind == Kind::Folder {
            list(&relative).map(Some)
        } else {
            Ok(None)
        };
        let refused = match one.kind {
            Kind::Link => Some(format!(
                "'{path}' is a link, and purlis opens no link in a skill's folder"
            )),
            Kind::File if !one.file => Some(format!("'{path}' is not a file")),
            _ => inside.as_ref().err().cloned(),
        };
        walk.entries.push(Walked {
            path,
            kind: one.kind,
            refused,
        });
        if let Ok(Some(inside)) = inside {
            stack.extend(inside.into_iter().rev().map(|one| (relative.clone(), one)));
        }
    }
    Ok(walk)
}

/// The file `path` names in `folder`, as [`super::open`] opens a branch's: refused when its
/// spelling leaves the folder or names git's, when a link is on its way or is the file itself,
/// and when it is not a regular file.
pub fn open(folder: &Path, path: &str) -> Result<Opened, String> {
    let spelled = super::spelled_inside(path)
        .map_err(|_| format!("'{path}' is not a path inside the skill's folder"))?;
    let held = super::hold_folder(folder, folder)
        .map_err(|e| format!("purlis could not read the skill's folder: {e}"))?;
    let file = super::open_inside(&held, Path::new(&spelled)).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            format!("'{path}' is not in the skill's folder any more")
        } else {
            format!("purlis could not open '{path}': it is a link or not a file ({e})")
        }
    })?;
    let meta = file
        .metadata()
        .map_err(|e| format!("purlis could not read '{path}': {e}"))?;
    if !meta.is_file() {
        return Err(format!("'{path}' is not a file"));
    }
    super::read_opened(file, meta.len(), path).map_err(|refused| refused.to_string())
}

/// **Which file a file is**: its device and inode, so a file read again is known to be the same
/// one and not one put in its place.
pub type Identity = (u64, u64);

/// **A file anywhere on this machine, by its absolute path** (#6216): opened from `/` one folder
/// at a time, following no link on the way or at the file itself, as a branch's file is opened.
/// A path with a link anywhere on it, or with `..` or `.` in it, is refused, and so is anything
/// but a regular file. Answered with the open file and its [`Identity`].
pub fn open_absolute(path: &Path) -> Result<(std::fs::File, Identity), String> {
    let refused = || format!("purlis could not open '{}'", path.display());
    let (Some(folder), Some(leaf)) = (path.parent(), path.file_name()) else {
        return Err(refused());
    };
    let plain = path.is_absolute()
        && path.components().all(|part| {
            matches!(
                part,
                std::path::Component::RootDir | std::path::Component::Normal(_)
            )
        });
    if !plain {
        return Err(refused());
    }
    let root = Path::new("/");
    let held = super::hold_folder(root, folder).map_err(|_| refused())?;
    let file = super::open_inside(&held, Path::new(leaf)).map_err(|_| refused())?;
    let meta = file.metadata().map_err(|_| refused())?;
    if !meta.is_file() {
        return Err(format!("'{}' is not a file", path.display()));
    }
    Ok((file, identity(&meta)))
}

/// The [`Identity`] of the file at `path`, opened as [`open_absolute`] opens it.
pub fn absolute_identity(path: &Path) -> Option<Identity> {
    open_absolute(path).ok().map(|(_, identity)| identity)
}

/// **The file at `path`, read whole**, as [`open_absolute`] opens it, only while it is still the
/// file `expected` names: one put in its place since reads nothing.
pub fn read_absolute(path: &Path, expected: Identity) -> Result<Opened, String> {
    let (file, found) = open_absolute(path)?;
    if found != expected {
        return Err(format!(
            "'{}' is not the file that was opened from its link",
            path.display()
        ));
    }
    let bytes = file
        .metadata()
        .map_err(|e| format!("purlis could not read '{}': {e}", path.display()))?
        .len();
    super::read_opened(file, bytes, &path.display().to_string())
        .map_err(|refused| refused.to_string())
}

#[cfg(unix)]
fn identity(meta: &std::fs::Metadata) -> Identity {
    use std::os::unix::fs::MetadataExt as _;
    (meta.dev(), meta.ino())
}

#[cfg(not(unix))]
fn identity(meta: &std::fs::Metadata) -> Identity {
    // No inode here: the size and the time it was written stand in for one.
    let written = meta
        .modified()
        .ok()
        .and_then(|at| at.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos() as u64);
    (meta.len(), written)
}

#[cfg(test)]
#[path = "folder/tests.rs"]
mod tests;

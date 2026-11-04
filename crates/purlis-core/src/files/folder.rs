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

#[cfg(test)]
#[path = "folder/tests.rs"]
mod tests;

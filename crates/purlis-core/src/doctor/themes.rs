// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The `themes` row (#2314): what sits in a plane's reserved `themes/` folders, at the plane root
//! and in each workspace (`docs/plane-format.md`, ADR 0098).
//!
//! A `themes/` folder holds theme folders and nothing else. A theme folder is a folder, never a
//! link, holding a `theme.json` that is a file: a theme's fonts, icons and licence live beside
//! its `theme.json`, so a bare `theme.json` in `themes/` is a theme with no folder of its own.
//! A name starting with `.` (git's `.gitkeep`, a desktop's own files) is never a theme's and is
//! left alone. Nothing here reads a theme: the theme engine checks what is inside a theme folder.
//!
//! **No row where no `themes/` exists**, so a plane without themes prints the rows it printed
//! before.

use std::path::Path;

use super::{DISPLAY_LIMIT, Doctor, Row, one_line};

const NAME: &str = "themes";

/// The name a theme folder's own file has.
const THEME_FILE: &str = "theme.json";

/// How many of the entries that are not theme folders the row names before it counts the rest.
const SHOWN: usize = 5;

/// What the reserved folders hold: theme folders counted, and what is not one, by its path
/// relative to the plane root.
#[derive(Default)]
struct Found {
    any: bool,
    themes: usize,
    strays: Vec<String>,
}

pub(super) fn themes(d: &Doctor) -> Option<Row> {
    let mut found = Found::default();
    if let Err(why) = gather(&d.root, &mut found) {
        return Some(Row::not_checked(NAME, why));
    }
    if !found.any {
        return None;
    }
    if found.strays.is_empty() {
        let count = match found.themes {
            1 => "1 theme folder".to_owned(),
            n => format!("{n} theme folders"),
        };
        return Some(Row::ok(NAME, count));
    }
    found.strays.sort();
    let total = found.strays.len();
    let mut named: Vec<String> = found.strays.iter().take(SHOWN).cloned().collect();
    if total > SHOWN {
        named.push(format!("and {} more", total - SHOWN));
    }
    let lead = if total == 1 {
        "1 entry in a themes/ folder is not a theme folder".to_owned()
    } else {
        format!("{total} entries in a themes/ folder are not theme folders")
    };
    Some(Row::warn(
        NAME,
        format!("{lead}: {}", named.join(", ")),
        "A themes/ folder holds only theme folders: put each theme in a folder of its own, \
         themes/<name>/theme.json, and move anything else out of themes/.",
    ))
}

/// Every reserved `themes/` under `root`: the plane root's, then each workspace's.
fn gather(root: &Path, found: &mut Found) -> Result<(), String> {
    look(root, "themes", found)?;
    let workspaces = root.join("workspaces");
    let Ok(entries) = std::fs::read_dir(&workspaces) else {
        // No workspaces is the `schema` row's to report, not this one's.
        return Ok(());
    };
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        // A name starting with `.` is never a workspace (`workspaces/`, plane-format).
        if name.starts_with('.') || !is_dir(&entry.path()) {
            continue;
        }
        look(root, &format!("workspaces/{name}/themes"), found)?;
    }
    Ok(())
}

/// One reserved folder, `relative` to `root`.
fn look(root: &Path, relative: &str, found: &mut Found) -> Result<(), String> {
    let at = root.join(relative);
    let Ok(meta) = std::fs::symlink_metadata(&at) else {
        return Ok(());
    };
    found.any = true;
    if !meta.is_dir() {
        let what = if meta.file_type().is_symlink() {
            "a link, not a folder"
        } else {
            "not a folder"
        };
        found.strays.push(format!("{} ({what})", shown(relative)));
        return Ok(());
    }
    let entries = std::fs::read_dir(&at).map_err(|e| format!("{}: {e}", shown(relative)))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("{}: {e}", shown(relative)))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let path = entry.path();
        let theme_file = path.join(THEME_FILE);
        let is_theme = is_dir(&path)
            && std::fs::symlink_metadata(&theme_file).is_ok_and(|meta| meta.is_file());
        if is_theme {
            found.themes += 1;
        } else {
            found.strays.push(shown(&format!("{relative}/{name}")));
        }
    }
    Ok(())
}

/// A real directory at `path`, never a link to one.
fn is_dir(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|meta| meta.is_dir())
}

/// A plane path as the row repeats it back: one line, bounded.
fn shown(relative: &str) -> String {
    one_line(relative, DISPLAY_LIMIT)
}

#[cfg(test)]
#[path = "themes_tests.rs"]
mod tests;

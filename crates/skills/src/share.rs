// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Share**: a local skill made an ordinary skill at the same level (ADR 0099 §1; SK-4, #2767).
//!
//! A local skill's folder, in its level's `skills.local/`, moves into that level's `skills/`,
//! with its `<folder>.toml` where it has one, so a project skill keeps its grant. Nothing is
//! copied: one folder, in one place, before and after. The plane's save then commits it as it
//! commits any other shared file.

use std::path::{Path, PathBuf};

use crate::{level_dir, read};

/// The folder in `dir` whose skill is named `name`: its frontmatter's `name`, which is a
/// skill's identity, never its folder's name. The first by folder name where two have one.
pub fn named(dir: &Path, name: &str) -> Option<PathBuf> {
    let mut found: Vec<PathBuf> = read::scan_dir(dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|skill| skill.name == name)
        .filter_map(|skill| skill.path.parent().map(Path::to_path_buf))
        .collect();
    found.sort();
    found.into_iter().next()
}

/// **Shares the local skill named `name`** at the plane root (`workspace` `None`) or in
/// `workspace`: moves its folder from the level's `skills.local/` into its `skills/`, and its
/// `<folder>.toml` with it, and says where the folder now is. Refused, moving nothing, where
/// the level has no local skill of that name or its `skills/` already holds that folder's name.
pub fn share(plane: &Path, workspace: Option<&str>, name: &str) -> Result<PathBuf, String> {
    let at = match workspace {
        None => "at the project's root".to_owned(),
        Some(ws) => format!("in workspace {ws}"),
    };
    let none = || format!("no local skill is named {name:?} {at}");
    let local = level_dir(plane, workspace, true).ok_or_else(none)?;
    let shared = level_dir(plane, workspace, false).ok_or_else(none)?;
    let from = named(&local, name).ok_or_else(none)?;
    let folder = from.file_name().unwrap_or_default().to_os_string();
    let to = shared.join(&folder);
    let shown = to
        .strip_prefix(plane)
        .unwrap_or(&to)
        .to_string_lossy()
        .into_owned();
    if to.symlink_metadata().is_ok() {
        return Err(format!(
            "{shown} is already there, so nothing was moved: rename one of the two folders first"
        ));
    }
    let (toml_from, toml_to) = (crate::beside(&from), crate::beside(&to));
    let toml_name = toml_from.file_name().unwrap_or_default().to_os_string();
    if toml_from.is_file() && toml_to.symlink_metadata().is_ok() {
        return Err(format!(
            "{} is already there, so nothing was moved: rename one of the two folders first",
            toml_to
                .strip_prefix(plane)
                .unwrap_or(&toml_to)
                .to_string_lossy()
        ));
    }
    std::fs::create_dir_all(&shared)
        .map_err(|why| format!("{shown} could not be made ({why}), so nothing was moved"))?;
    std::fs::rename(&from, &to)
        .map_err(|why| format!("the folder could not be moved to {shown} ({why})"))?;
    if toml_from.is_file()
        && let Err(why) = std::fs::rename(&toml_from, &toml_to)
    {
        // The folder is shared; its grant stayed behind, so put the folder back.
        let _ = std::fs::rename(&to, &from);
        return Err(format!(
            "its {} could not be moved beside it ({why}), so nothing was moved",
            toml_name.to_string_lossy()
        ));
    }
    Ok(to)
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Share**: a local skill made an ordinary skill at the same level (ADR 0099 §1; SK-4, #2767).
//!
//! A local skill's folder, in its level's `skills.local/`, moves into that level's `skills/`,
//! with its `<folder>.toml` where it has one, so a project skill keeps its grant. Nothing is
//! copied: one folder, in one place, before and after. The plane's save then commits it as it
//! commits any other shared file.

use std::path::{Path, PathBuf};

use crate::{Unmoved, Whose, level_rel, move_skill, read};

/// The folder in `dir` whose skill is named `name`: its frontmatter's `name`, which is a
/// skill's identity, never its folder's name. The first by folder name where two have one.
///
/// The folders above `dir` are trusted as they are: for a folder that is no level of a plane.
/// A plane level's is [`named_at`].
pub fn named(dir: &Path, name: &str) -> Option<PathBuf> {
    first_named(read::scan_dir(dir), name)
}

/// [`named`] in the level `at`'s `skills.local/` where `local`, reached from the plane's root
/// without following a link (SK-36): a workspace's or persona's folder that is a link holds no
/// skill of the plane's, so nothing is found there.
pub fn named_at(plane: &Path, at: &Whose, local: bool, name: &str) -> Option<PathBuf> {
    let rel = level_rel(at, local)?;
    first_named(read::scan(plane, Path::new(&rel)), name)
}

/// The folder of the first skill named `name` among `found`, by folder name.
fn first_named(
    found: Vec<Result<crate::Skill, (PathBuf, crate::Missing)>>,
    name: &str,
) -> Option<PathBuf> {
    let mut found: Vec<PathBuf> = found
        .into_iter()
        .filter_map(Result::ok)
        .filter(|skill| skill.name == name)
        .filter_map(|skill| skill.path.parent().map(Path::to_path_buf))
        .collect();
    found.sort();
    found.into_iter().next()
}

/// **What a share moved**: the local skill's folder it took, and where that folder is now. What
/// a caller commits is these two, never a folder it looked up again by path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shared {
    /// The folder in the level's `skills.local/` it was.
    pub from: PathBuf,
    /// The folder in the level's `skills/` it is now.
    pub to: PathBuf,
}

/// **Shares the local skill named `name`** at the level `at` (the plane root's, a workspace's
/// or a persona's): moves its folder from the level's `skills.local/` into its `skills/`, and its
/// `<folder>.toml` with it, and says where it was and where it is now. Refused, moving nothing,
/// where the level has no local skill of that name, its `skills/` already holds that folder's
/// name, or anything on either way is a link.
///
/// **Nothing is moved through a link** (SK-36): the folder is found by a walk from the plane's
/// root ([`read::scan`]), and moved, with the file beside it, by the one mover
/// ([`move_skill`]), which opens both sides one folder at a time without following a link and
/// moves a plain folder or file alone. A `skills/` that is a link, a `<folder>.toml` that is
/// one, or a folder on the way swapped for one after the look is refused, not followed.
pub fn share(plane: &Path, at: &Whose, name: &str) -> Result<Shared, String> {
    let said = match at {
        Whose::Project => "at the project's root".to_owned(),
        Whose::Workspace(ws) => format!("in workspace {ws}"),
        Whose::Persona(persona) => format!("of persona {persona}"),
    };
    let none = || format!("no local skill is named {name:?} {said}");
    let shared = level_rel(at, false).ok_or_else(none)?;
    let from = named_at(plane, at, true, name).ok_or_else(none)?;
    let folder = from.file_name().unwrap_or_default().to_os_string();
    let to = plane.join(&shared).join(&folder);
    let shown = to
        .strip_prefix(plane)
        .unwrap_or(&to)
        .to_string_lossy()
        .into_owned();
    // A look ahead for a clear sentence alone: the move itself never replaces anything.
    if to.symlink_metadata().is_ok() {
        return Err(format!(
            "{shown} is already there, so nothing was moved: rename one of the two folders first"
        ));
    }
    let (toml_from, toml_to) = (crate::beside(&from), crate::beside(&to));
    if toml_from.symlink_metadata().is_ok() && toml_to.symlink_metadata().is_ok() {
        return Err(format!(
            "{} is already there, so nothing was moved: rename one of the two folders first",
            toml_to
                .strip_prefix(plane)
                .unwrap_or(&toml_to)
                .to_string_lossy()
        ));
    }
    // Only the level's own `skills/` is made; the folders above it are the level's.
    let make_from = Path::new(&shared).components().count();
    move_skill(plane, &from, &to, make_from).map_err(|unmoved| match unmoved {
        Unmoved::Nothing(why) => {
            format!("{name} could not be moved to {shown} ({why}), so nothing was moved")
        }
        // Something moved and stayed: say what, never that nothing was moved (SK-45).
        Unmoved::Stranded(what) => format!("{name} could not be moved to {shown}: {what}"),
    })?;
    Ok(Shared { from, to })
}

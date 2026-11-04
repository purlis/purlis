// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A skill's folder, for its page's Files tab** (#5183): which folder a skill the Skill studio
//! lists is in, as the core finds it again.
//!
//! The window names a skill's folder by the row's own `folder`, which for a built-in or a
//! third-party skill is a path outside the project. It is never trusted as a directory: the
//! core lists the studio again, as it lists it for the window, and answers a folder only when a
//! row of that list is in it. So the Files tab reaches the folders the studio shows, and
//! nothing else on this machine.

use std::path::{Path, PathBuf};

/// The folder `folder` names, where a skill the studio lists for a chat at `plane`'s root or in
/// `workspace`, as `persona` or on none, is in; else why not, in the window's sentence.
pub fn listed_folder(
    built_in: Option<&Path>,
    plane: &Path,
    workspace: Option<&str>,
    persona: Option<&str>,
    env: &crate::harness_plugin::Env<'_>,
    folder: &str,
) -> Result<PathBuf, String> {
    let refused = || format!("'{folder}' is not the folder of a skill the studio lists here");
    if folder.is_empty() {
        return Err(refused());
    }
    // Only what Needs you reads: every row and its folder, and no badge or router's word.
    let list = super::studio_list_for(
        super::Reading::Needs,
        built_in,
        plane,
        workspace,
        persona,
        env,
        &[],
    );
    let listed = list
        .rows
        .iter()
        .chain(list.harnesses.iter().flat_map(|one| one.rows.iter()))
        .any(|row| row.folder == folder);
    if !listed {
        return Err(refused());
    }
    // A row's folder is relative to the project where it is inside it, else whole.
    let path = Path::new(folder);
    Ok(if path.is_absolute() {
        path.to_path_buf()
    } else {
        plane.join(path)
    })
}

#[cfg(test)]
#[path = "folder/tests.rs"]
mod tests;

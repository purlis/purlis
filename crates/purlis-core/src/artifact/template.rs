// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A template shipped with something else, placed in a workspace** (ADR 0122 §1 and §9): an
//! extension's (#5973) or a skill's (#5974). Either way the template is copied, without its
//! data files, into the workspace's `artifacts/<folder>/`, so an artifact is still a folder of
//! the workspace, and the copy is then pinned through the one pin path.
//!
//! # A skill's template
//!
//! A skill ships a template in its own folder as `artifacts/<name>/index.html`, beside its
//! `SKILL.md` ([`IN_A_SKILL`]). Its run asks for it by the skill's name and the template's
//! ([`place_from_skill`]): the skill is looked up **by name in the chat's own skill set**, never
//! by a path, and the template is read from inside that skill's folder alone. It is copied to
//! `artifacts/<name>/` of the workspace, read within the size cap and scanned for anything
//! shaped like a secret before a byte is written ([`place`]).
//!
//! A skill runs again, so **a folder already there holding the same template is left as it
//! is** ([`Placed::There`]): its data files, written by the skill's earlier runs, stay. One
//! holding another template is never written over; to take the skill's newer template, unpin
//! and remove the folder, then run the skill again.

use std::path::{Path, PathBuf};

use super::serve::Snapshot;

/// The folder inside a skill's own folder that holds the templates it ships.
pub const IN_A_SKILL: &str = "artifacts";

/// What [`place`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placed {
    /// The template was copied into a new folder.
    Copied,
    /// The folder was there already with the same template, and was left as it is.
    There,
}

/// **The folder of skill `skill` in `set`**, a chat's own skill set: where its `SKILL.md` is.
pub fn skill_folder(set: &crate::skills::SkillSet, skill: &str) -> Result<PathBuf, String> {
    let effective = purlis_skills::by_name(set, skill).ok_or_else(|| {
        format!(
            "this chat has no skill named '{}', so no template of it was placed",
            crate::personas::one_line(skill)
        )
    })?;
    effective
        .skill
        .path
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| format!("the skill '{}' has no folder", effective.skill.name))
}

/// **Template `name` of the skill at `skill`**: its `artifacts/<name>/`, every link followed,
/// or why it is not a folder inside the skill holding a page.
pub fn of_skill(skill: &Path, name: &str) -> Result<PathBuf, String> {
    if let Some(why) = super::folder_refusal(name) {
        return Err(why);
    }
    inside(skill, &format!("{IN_A_SKILL}/{name}"), name, "skill")
}

/// **Place template `name` of skill `skill` of `set` in the workspace at `workspace`**, in the
/// plane at `root`: [`skill_folder`], [`of_skill`], then [`place`] under the same name.
pub fn place_from_skill(
    root: &Path,
    workspace: &Path,
    set: &crate::skills::SkillSet,
    skill: &str,
    name: &str,
) -> Result<Placed, String> {
    let folder = skill_folder(set, skill)?;
    let source = of_skill(&folder, name)?;
    place(root, workspace, &source, name)
}

/// **Place the template at `source` in the workspace's `artifacts/<folder>/`**: read within the
/// size cap and scanned before anything is written, then copied without its data files; or,
/// where the folder is there with the same template, left as it is. A refusal writes nothing.
pub fn place(root: &Path, workspace: &Path, source: &Path, folder: &str) -> Result<Placed, String> {
    if let Some(why) = super::folder_refusal(folder) {
        return Err(why);
    }
    let snapshot = Snapshot::read(source, folder)?;
    super::scanned(source, folder)?;
    if workspace
        .join(super::DIR)
        .join(folder)
        .symlink_metadata()
        .is_ok()
    {
        let there = super::resolve(workspace, folder)
            .and_then(|real| Snapshot::read(&real, folder))
            .map_err(|why| not_over(folder, &why))?;
        if there.fingerprint() != snapshot.fingerprint() {
            return Err(not_over(folder, "it holds another page"));
        }
        return Ok(Placed::There);
    }
    copy_in(root, workspace, folder, &snapshot)?;
    Ok(Placed::Copied)
}

fn not_over(folder: &str, why: &str) -> String {
    format!(
        "there is already a folder '{}' in {}/ ({why}), so the template is not copied over it. \
         Unpin it and remove the folder to take the template again",
        crate::personas::one_line(folder),
        super::DIR
    )
}

/// Where `folder` of the `whose` (an extension or a skill) at `home` is, every link followed,
/// or why it is not a folder inside it. `shown` names the template in the sentence.
pub(crate) fn inside(
    home: &Path,
    folder: &str,
    shown: &str,
    whose: &str,
) -> Result<PathBuf, String> {
    let home_real = home
        .canonicalize()
        .map_err(|_| format!("the {whose} of '{shown}' cannot be read"))?;
    let real = home
        .join(folder)
        .canonicalize()
        .map_err(|_| format!("the {whose} has no folder {folder} for '{shown}'"))?;
    if !real.starts_with(&home_real) || real == home_real || !real.is_dir() {
        return Err(format!(
            "the folder of '{shown}' is not a folder inside its {whose}, so it is not placed"
        ));
    }
    Ok(real)
}

/// Write the template's files, not its data, into a new `artifacts/<folder>/` of the workspace.
/// Never over a folder that is there; a failure part way removes what it wrote.
pub(crate) fn copy_in(
    root: &Path,
    workspace: &Path,
    folder: &str,
    snapshot: &Snapshot,
) -> Result<(), String> {
    let dir = workspace.join(super::DIR);
    std::fs::create_dir_all(&dir).map_err(|e| {
        format!(
            "purlis could not make {}/ in the workspace ({})",
            super::DIR,
            crate::rewrite::os_words(&e)
        )
    })?;
    let at = dir.join(folder);
    match std::fs::create_dir(&at) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            return Err(not_over(folder, "it is there already"));
        }
        Err(e) => {
            return Err(format!(
                "purlis could not make '{folder}' in {}/ ({})",
                super::DIR,
                crate::rewrite::os_words(&e)
            ));
        }
    }
    let written = (|| {
        for path in snapshot.paths().filter(|path| !super::serve::is_data(path)) {
            let target = at.join(path);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            crate::rewrite::replace(
                root,
                &target,
                snapshot.get(path).unwrap_or_default(),
                crate::rewrite::Mode::Kept,
            )?;
        }
        Ok::<(), std::io::Error>(())
    })();
    written.map_err(|e| {
        let _ = std::fs::remove_dir_all(&at);
        format!(
            "purlis could not copy the template into '{folder}' ({})",
            crate::rewrite::os_words(&e)
        )
    })
}

#[cfg(test)]
#[path = "template/tests.rs"]
mod tests;

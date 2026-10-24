// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The skills from outside the plane a harness finds itself** (SK-6, #2769; ADR 0099 §1): the
//! person's own, in the folders and plugins its adapter names ([`Adapter::skills_at`], SK-5's),
//! and a repo's own, in the folders it reads in a clone ([`Adapter::repo_skill_dirs`]). Read
//! only, and never written. Each is a [`purlis_skills::OutsideSkill`], so `purlis-skills` lists
//! it beside the plane's skills and settles a clash ([`purlis_skills::SkillSet::with_outside`]).
//!
//! **A clone is the chat's to write, so nothing in it is read through a link** ([`Folder`]):
//! each folder on the way to a repo's skill and its `SKILL.md` is opened through the handle of the
//! folder above it, never following a link at its last step, so a link put in place after a look
//! is never read through. A skill folder that is a link is listed by its name, saying purlis does
//! not follow it. The person's own folders are theirs, and a skill there that is a link, as an
//! installer leaves one, is read where it leads.

use std::path::{Path, PathBuf};

use purlis_skills::{Origin, OutsideSkill, Owner, Skill, Switch};

use super::{Adapter, Child, Choices, Env, Folder, SkillsAt};

/// Whether a link where a skill's folder should be is read through.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Links {
    /// Read where it leads: the person's own folders.
    Followed,
    /// Listed by its name and never read through: a clone, which a chat can write.
    Named,
}

/// **The person's own skills the harness of `adapter` finds on this machine** with `env`, and
/// its plugins', in the order of its places and then of the skills' folder names, each turned off
/// where `choices` say so. A skill one plugin has in two folders (two installs, or synced twice)
/// is listed once: it is one skill to the harness.
pub fn harness_skills(
    adapter: &dyn Adapter,
    env: &Env<'_>,
    choices: &Choices,
) -> Vec<OutsideSkill> {
    let mut out: Vec<OutsideSkill> = Vec::new();
    for (dir, plugin) in roots(&adapter.skills_at(env)) {
        let Some(held) = Folder::open(&dir, true) else {
            continue;
        };
        for skill in skills_in(&held, &dir, Links::Followed) {
            let owner = plugin.clone().map_or(Owner::You, Owner::Plugin);
            // The person's own folder is read as it is, a link an installer left included.
            let root = skill
                .path
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_default();
            let one = outside(adapter, choices, (skill, root), Origin::Harness, owner);
            if !out.iter().any(|had| had.id == one.id) {
                out.push(one);
            }
        }
    }
    out
}

/// **The repo skills of `clones`** the harness of `adapter` reads in each, in their order and then
/// the order of the folders it reads, each turned off where `choices` say so.
pub fn repo_skills(
    adapter: &dyn Adapter,
    choices: &Choices,
    clones: &[crate::repos::Repo],
) -> Vec<OutsideSkill> {
    let mut out = Vec::new();
    for clone in clones {
        let Some(top) = Folder::open(&clone.path, false) else {
            continue;
        };
        for rel in adapter.repo_skill_dirs() {
            let Some(dir) = walk(&top, rel) else {
                continue;
            };
            for skill in skills_in(&dir, &clone.path.join(rel), Links::Named) {
                let owner = Owner::Repo(clone.name.clone());
                // A clone is the chat's to write: read from the folder it sits in, the clone's
                // own folder and every one below opened through held folders (SK-36).
                let root = clone
                    .path
                    .parent()
                    .map_or_else(|| clone.path.clone(), Path::to_path_buf);
                out.push(outside(
                    adapter,
                    choices,
                    (skill, root),
                    Origin::Repo,
                    owner,
                ));
            }
        }
    }
    out
}

/// One skill from outside the plane, by the name the harness knows it by: a plugin's prefixed
/// with its plugin's name.
fn outside(
    adapter: &dyn Adapter,
    choices: &Choices,
    (skill, root): (Skill, PathBuf),
    origin: Origin,
    owner: Owner,
) -> OutsideSkill {
    let id = match &owner {
        Owner::Plugin(plugin) => format!("{plugin}:{}", skill.name),
        Owner::You | Owner::Repo(_) => skill.name.clone(),
    };
    OutsideSkill {
        off: choices.skill(adapter.harness(), &id).0 == Some(false),
        switch: adapter
            .skill_named_alone(&id)
            .map_or(Switch::Alone, Switch::Not),
        id,
        skill,
        root,
        origin,
        owner,
    }
}

/// The folders of skills in `at`, where the harness says it finds them (SK-5's
/// [`Adapter::skills_at`]), each with the plugin whose they are: a folder as it is, and a
/// plugin's folders as its manifest names them, in its newest version's folder where it keeps
/// one per version. A plugin whose folder purlis cannot find has none to list.
fn roots(at: &[SkillsAt]) -> Vec<(PathBuf, Option<String>)> {
    let mut out = Vec::new();
    for place in at {
        match place {
            SkillsAt::Folder { dir, plugin } => out.push((dir.clone(), plugin.clone())),
            SkillsAt::Plugin {
                plugin,
                dir,
                manifest,
                versions,
            } => {
                let base = if *versions {
                    crate::skills::folders(dir).pop()
                } else {
                    Some(dir.clone())
                };
                if let Some(base) = base {
                    for skills in crate::skills::skill_dirs(&base, manifest) {
                        out.push((base.join(skills), Some(plugin.clone())));
                    }
                }
            }
            SkillsAt::Unlisted(_) => {}
        }
    }
    out
}

/// The folder `rel` (`a/b`) under `top`, opened one step at a time and never through a link.
fn walk(top: &Folder, rel: &str) -> Option<Folder> {
    let mut steps = rel.split('/');
    let step = |at: &Folder, name: &str| match at.child(name, false) {
        Child::Folder(next) => Some(next),
        Child::Link | Child::Absent => None,
    };
    let mut at = step(top, steps.next()?)?;
    for name in steps {
        at = step(&at, name)?;
    }
    Some(at)
}

/// Every whole skill in `dir`, held open at `shown` (the path a skill's `SKILL.md` is said to be
/// at), by its folder's name, a link in it taken as `links` says. A folder that is not a whole
/// skill is not listed: it is no skill the harness loads either.
fn skills_in(dir: &Folder, shown: &Path, links: Links) -> Vec<Skill> {
    let mut names = dir.names();
    names.sort();
    let mut out = Vec::new();
    for name in names {
        let folder = match dir.child(&name, links == Links::Followed) {
            Child::Folder(folder) => folder,
            Child::Link => {
                out.push(Skill {
                    name: name.clone(),
                    description: "A link purlis does not follow, so what it holds is not read \
                                  here. A chat in this repo still follows it."
                        .to_owned(),
                    path: shown.join(&name).join(purlis_skills::FILE),
                });
                continue;
            }
            Child::Absent => continue,
        };
        let Ok(Some(text)) = folder.read(purlis_skills::FILE) else {
            continue;
        };
        if let Ok(skill) = purlis_skills::parsed(&text, shown.join(&name).join(purlis_skills::FILE))
        {
            out.push(skill);
        }
    }
    out
}

#[cfg(test)]
mod tests;

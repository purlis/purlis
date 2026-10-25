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

use purlis_skills::{Origin, OutsideSkill, Owner, Skill, Switch, Unread};

use super::{Adapter, Child, Choices, Env, Folder, Kind, SkillsAt};

/// One skill [`skills_in`] found: the skill, the root its `SKILL.md` is read from, and why
/// purlis did not read it, where it did not.
type Found = (Skill, PathBuf, Option<Unread>);

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
        // The person's own folder is read as it is, a link an installer left included.
        for found in skills_in(&held, &dir, Links::Followed, &dir) {
            let owner = plugin.clone().map_or(Owner::You, Owner::Plugin);
            let one = outside(adapter, choices, found, Origin::Harness, owner);
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
            // A clone is the chat's to write: read from the folder it sits in, the clone's own
            // folder and every one below opened through held folders (SK-36).
            let root = clone
                .path
                .parent()
                .map_or_else(|| clone.path.clone(), Path::to_path_buf);
            for found in skills_in(&dir, &clone.path.join(rel), Links::Named, &root) {
                let owner = Owner::Repo(clone.name.clone());
                out.push(outside(adapter, choices, found, Origin::Repo, owner));
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
    (skill, root, unread): Found,
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
        unread,
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
/// at), by its folder's name, a link in it taken as `links` says, each with the root its
/// `SKILL.md` is read from: `root` where links are named, its own folder where they are followed.
/// A folder that is not a whole skill is not listed: it is no skill the harness loads either.
///
/// **Each `SKILL.md` is read with one limit, [`purlis_skills::MOST_BYTES`], never through a
/// link** (SK-44). In a clone (`Links::Named`), as every skill's own file is
/// ([`purlis_skills::read_skill_md`]): through folders held from its root, a hard link refused.
/// In the person's own folders (`Links::Followed`), through the folder held open, a hard link
/// read: the Nix store, `cp -al` and `rsync --link-dest` leave them there. One purlis does not
/// read is listed by its folder's name, saying so ([`Unread::Refused`]), as a link is
/// ([`Unread::Link`]); and a `dir` purlis cannot list whole is read for none of them, and listed
/// once by its own name ([`Unread::Crowded`]).
fn skills_in(dir: &Folder, shown: &Path, links: Links, root: &Path) -> Vec<Found> {
    let unread = |name: &str, description: String, why: Unread| -> Found {
        let skill = Skill {
            name: name.to_owned(),
            description,
            path: shown.join(name).join(purlis_skills::FILE),
        };
        (skill, root.to_path_buf(), Some(why))
    };
    let mut names = match dir.names() {
        Ok(names) => names,
        Err(why) => {
            let skill = Skill {
                name: shown
                    .file_name()
                    .map_or_else(String::new, |name| name.to_string_lossy().into_owned()),
                description: format!(
                    "{}, so none of its skills is read here. A chat still finds them.",
                    capitalized(&why.said())
                ),
                path: shown.join(purlis_skills::FILE),
            };
            return vec![(skill, root.to_path_buf(), Some(Unread::Crowded))];
        }
    };
    names.sort();
    let refused = format!(
        "purlis does not read a SKILL.md that is a link{} or larger than {} KiB, so it is not \
         checked here. A chat still loads it.",
        match links {
            Links::Named => ", a hard link",
            Links::Followed => "",
        },
        purlis_skills::MOST_BYTES / 1024
    );
    let mut out = Vec::new();
    for name in names {
        let path = shown.join(&name).join(purlis_skills::FILE);
        let folder = match dir.child(&name, links == Links::Followed) {
            Child::Folder(folder) => folder,
            Child::Link => {
                out.push(unread(
                    &name,
                    "A link purlis does not follow, so what it holds is not read here. A chat in \
                     this repo still follows it."
                        .to_owned(),
                    Unread::Link,
                ));
                continue;
            }
            Child::Absent => continue,
        };
        let (text, root) = match links {
            Links::Named => match purlis_skills::read_skill_md(root, &path) {
                Ok(text) => (text, root.to_path_buf()),
                Err(purlis_skills::Refusal::FileLink | purlis_skills::Refusal::TooLarge) => {
                    out.push(unread(&name, refused.clone(), Unread::Refused));
                    continue;
                }
                Err(_) => continue,
            },
            Links::Followed => {
                match folder.read_at_most(purlis_skills::FILE, purlis_skills::MOST_BYTES) {
                    Ok(Some(text)) => (text, shown.join(&name)),
                    Ok(None) => continue,
                    Err(()) => {
                        if matches!(
                            folder.kind(purlis_skills::FILE, false),
                            Some(Kind::File | Kind::Link)
                        ) {
                            out.push(unread(&name, refused.clone(), Unread::Refused));
                        }
                        continue;
                    }
                }
            }
        };
        if let Ok(skill) = purlis_skills::parsed(&text, path) {
            out.push((skill, root, None));
        }
    }
    out
}

/// `said` with its first letter a capital, to start a sentence.
fn capitalized(said: &str) -> String {
    let mut chars = said.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

#[cfg(test)]
mod tests;

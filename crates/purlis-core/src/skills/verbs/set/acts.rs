// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The rest of what the Library's Select mode does to several skills at once** (#5332): pin or
//! unpin, archive or unarchive, and move to another level, each as **one commit, all or nothing**,
//! as a delete and a turn are ([`super`]). Each answers what it changed, which is what its Undo
//! changes back, so one that already was so is left as it was.

use std::path::{Path, PathBuf};

use purlis_skills::Lifecycle;

use super::super::{
    Ctx, Level, MovePlan, Whose, beside, find, head, move_checked, move_folder, move_followed, rel,
    unstage,
};
use super::Member;
use crate::counts::counted;
use crate::skills::routing::{self, PinnedBy};
use crate::skills::{Grant, grant_of};

/// What a save that made no commit staged of `paths`, taken out of the index.
fn unstaged(plane: &Path, paths: &[String]) {
    let specs: Vec<String> = paths
        .iter()
        .filter(|path| !path.is_empty())
        .map(|path| crate::planegit::literally(path))
        .collect();
    unstage(plane, &specs);
}

/// Whose chats `by` names, in words.
fn whose(by: &PinnedBy) -> String {
    match by {
        PinnedBy::Workspace(ws) => format!("{ws}'s chats"),
        PinnedBy::Persona(persona) => format!("chats as {persona}"),
    }
}

/// **Pins every one of `names` (`on`) or unpins them** for `by`'s chats, as one commit, refused
/// whole where one cannot be. Answers those it changed.
pub fn pin_set(
    cx: &mut Ctx<'_>,
    by: &PinnedBy,
    names: &[String],
    on: bool,
) -> Result<Vec<String>, String> {
    let plane = cx.plane;
    let file = routing::pins_file(plane, by);
    let kept = kept_as_is(&[plane.join(&file)]);
    let changed = routing::pin_many(plane, by, names, on)?;
    if changed.is_empty() {
        return Ok(changed);
    }
    let commits_now = crate::planegit::paths_unsaved(plane, std::slice::from_ref(&file)).is_none();
    let before = head(plane);
    let (word, did) = if on {
        ("pin", "pinned")
    } else {
        ("unpin", "unpinned")
    };
    let count = changed.len();
    let whose = whose(by);
    cx.commit(
        std::slice::from_ref(&file),
        &format!("skill {word} {count} for {whose}"),
    );
    if commits_now && head(plane) == before {
        unstaged(plane, std::slice::from_ref(&file));
        put_as_is(&kept);
        return Err(format!(
            "its commit could not be made, so no skill was {did}"
        ));
    }
    cx.done(format!(
        "{} {did} for {whose}",
        counted(count, "skill", "skills")
    ));
    Ok(changed)
}

/// **Sets the lifecycle state of each skill of `changes`** (archived, or in use again) as one
/// commit of the files beside them, refused whole where one cannot be found or written. Answers
/// each it changed with the state it had, which is what its Undo sets.
pub fn lifecycle_set(
    cx: &mut Ctx<'_>,
    changes: &[(Member, Lifecycle)],
) -> Result<Vec<(Member, Lifecycle)>, String> {
    let plane = cx.plane;
    crate::settings::writable(plane)?;
    if changes.is_empty() {
        return Err("no skill was named".to_owned());
    }
    // Every one is read before the first is written.
    let mut planned: Vec<(&Member, &Lifecycle, PathBuf, Lifecycle)> = Vec::new();
    for (member, to) in changes {
        let name = &member.name;
        let folder =
            find(plane, &member.level, name).map_err(|why| format!("{why}, so nothing changed"))?;
        let was = purlis_skills::about_of(&folder).map(|about| about.lifecycle).map_err(|_| {
            format!(
                "what purlis keeps about {name} is not in a form purlis reads, so nothing changed"
            )
        })?;
        if was != *to {
            planned.push((member, to, folder, was));
        }
    }
    if planned.is_empty() {
        return Ok(Vec::new());
    }
    let put_back = |done: &[(&Member, &Lifecycle, PathBuf, Lifecycle)]| {
        for (_, _, folder, was) in done.iter().rev() {
            let _ = purlis_skills::set_lifecycle(folder, was);
        }
    };
    for (at, (member, to, folder, _)) in planned.iter().enumerate() {
        if let Err(why) = purlis_skills::set_lifecycle(folder, to) {
            put_back(&planned[..at]);
            tracing::warn!(target: "purlis::skills", "{}'s lifecycle was not written: {why}", member.name);
            return Err(format!(
                "what purlis keeps about {} could not be changed, so nothing changed",
                member.name
            ));
        }
    }
    let paths: Vec<String> = planned
        .iter()
        .map(|(_, _, folder, _)| rel(plane, &beside(folder)))
        .collect();
    let commits_now = crate::planegit::paths_unsaved(plane, &paths).is_none();
    let before = head(plane);
    let archiving = planned
        .iter()
        .all(|(_, to, _, _)| **to == Lifecycle::Archived);
    let count = planned.len();
    let word = if archiving { "archive" } else { "lifecycle" };
    cx.commit(&paths, &format!("skill {word} {count}"));
    if commits_now && head(plane) == before {
        unstaged(plane, &paths);
        put_back(&planned);
        return Err("its commit could not be made, so nothing changed".to_owned());
    }
    for (member, to, _, _) in &planned {
        if **to == Lifecycle::Archived
            && let Some(wider) =
                crate::skills::lifecycle::wider_copy(plane, &member.level, &member.name)
        {
            // An archived copy takes no name, so the next copy out loads where this one did.
            (cx.say)(crate::repocmd::Say::Info(
                format!(
                    "The {wider} copy of {} now loads in its place, wherever it reaches.",
                    member.name
                )
                .into(),
            ));
        }
    }
    cx.done(if archiving {
        format!(
            "{} archived: they never load until they are restored",
            counted(count, "skill", "skills")
        )
    } else {
        format!("{} changed", counted(count, "skill", "skills"))
    });
    Ok(planned
        .into_iter()
        .map(|(member, _, _, was)| (member.clone(), was))
        .collect())
}

/// **One skill's move**: the skill, where it goes, and the grant it takes into the project's
/// skills ([`super::super::needs_grant`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Moving {
    pub member: Member,
    pub to: Level,
    pub grant: Option<Grant>,
}

/// The bytes of each file at `paths`, or that it is not there: what a move put back restores.
fn kept_as_is(paths: &[PathBuf]) -> Vec<(PathBuf, Option<Vec<u8>>)> {
    paths
        .iter()
        .map(|path| (path.clone(), std::fs::read(path).ok()))
        .collect()
}

/// Each file of `kept` back as it was: its bytes, or not there.
fn put_as_is(kept: &[(PathBuf, Option<Vec<u8>>)]) {
    for (path, bytes) in kept {
        match bytes {
            Some(bytes) => {
                let _ = std::fs::write(path, bytes);
            }
            None => {
                let _ = std::fs::remove_file(path);
            }
        }
    }
}

/// Every file a move may change beside the skills: each workspace's settings and each persona's
/// file, which a turn-off and a persona's list are kept in.
fn followers(plane: &Path) -> Vec<PathBuf> {
    let workspaces = crate::workspaces::Plane::open(plane)
        .workspaces()
        .unwrap_or_default()
        .into_iter()
        .map(|ws| plane.join(crate::settings::workspace::named(&ws)));
    let personas = crate::personaverbs::names(plane)
        .into_iter()
        .map(|persona| crate::personas::def_path(plane, &persona));
    workspaces.chain(personas).collect()
}

/// **Moves every skill of `moves` as one change**: checked whole before the first moves, each
/// folder moved, then what follows each ([`super::super::move_to`]), then one commit. A folder
/// that cannot move, or a commit that cannot land, puts every one back. Answers the moves that
/// undo it: each back where it was, with the grant it had there.
pub fn move_set(cx: &mut Ctx<'_>, moves: &[Moving]) -> Result<Vec<Moving>, String> {
    let plane = cx.plane;
    crate::settings::writable(plane)?;
    if moves.is_empty() {
        return Err("no skill was named to move".to_owned());
    }
    let mut planned: Vec<(&Moving, MovePlan)> = Vec::new();
    for one in moves {
        let name = &one.member.name;
        if planned.iter().any(|(other, _)| {
            other.member == one.member || (other.member.name == *name && other.to == one.to)
        }) {
            return Err(format!(
                "{name} is named twice, or two skills named {name} would go to the {} skills, so \
                 nothing was moved",
                one.to
            ));
        }
        let plan = move_checked(cx, &one.member.level, name, &one.to, one.grant.as_ref())
            .map_err(|why| format!("{name} cannot move ({why}), so nothing was moved"))?;
        planned.push((one, plan));
    }
    let back: Vec<Moving> = planned
        .iter()
        .map(|(one, plan)| Moving {
            member: Member {
                level: one.to.clone(),
                name: one.member.name.clone(),
            },
            to: one.member.level.clone(),
            grant: (one.member.level.at == Whose::Project)
                .then(|| grant_of(&plan.folder).unwrap_or(Grant::Root)),
        })
        .collect();
    let followed_files = followers(plane);
    let kept = kept_as_is(
        &planned
            .iter()
            .map(|(_, plan)| beside(&plan.folder))
            .chain(followed_files)
            .collect::<Vec<_>>(),
    );
    let folders_back = |done: &[(&Moving, MovePlan)]| -> Vec<String> {
        let mut left = Vec::new();
        for (one, plan) in done.iter().rev() {
            let depth = one.member.level.depth();
            if let Err(why) =
                purlis_skills::move_skill(plane, &plan.moved, plane, &plan.folder, depth)
            {
                left.push(format!("{} ({why:?})", one.member.name));
            }
        }
        left
    };
    let none_moved = |why: String, left: &[String]| {
        if left.is_empty() {
            format!("{why}, so nothing was moved")
        } else {
            format!(
                "{why}, and {} could not be moved back: each is where the move left it",
                left.join(", ")
            )
        }
    };
    for (at, (one, plan)) in planned.iter().enumerate() {
        if let Err(why) = move_folder(plane, plan, &one.member.name) {
            let left = folders_back(&planned[..at]);
            return Err(none_moved(why, &left));
        }
    }
    let mut touched = Vec::new();
    for (one, plan) in &planned {
        touched.extend(move_followed(
            cx,
            plan,
            &one.member.level,
            &one.member.name,
            &one.to,
            one.grant.as_ref(),
        ));
    }
    let commits_now = crate::planegit::paths_unsaved(plane, &touched).is_none();
    let before = head(plane);
    let count = planned.len();
    let to = &planned[0].0.to;
    let same_to = planned.iter().all(|(one, _)| one.to == *to);
    let subject = if same_to {
        format!("skill mv {count} {to}")
    } else {
        format!("skill mv {count}")
    };
    cx.commit(&touched, &subject);
    if commits_now && head(plane) == before {
        unstaged(plane, &touched);
        let left = folders_back(&planned);
        for (one, _) in &planned {
            super::super::after_edit_follows(
                cx,
                (&one.to, &one.member.name),
                (&one.member.level, &one.member.name),
                &[],
            );
        }
        put_as_is(&kept);
        return Err(none_moved("its commit could not be made".to_owned(), &left));
    }
    cx.done(if same_to {
        format!(
            "{} moved to the {to} skills",
            counted(count, "skill", "skills")
        )
    } else {
        format!("{} moved", counted(count, "skill", "skills"))
    });
    Ok(back)
}

#[cfg(test)]
mod tests;

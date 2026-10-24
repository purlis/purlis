// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Deprecating, archiving and restoring a skill, its owners, and the skills unused for 60 days**
//! (SK-19, #2782; spec #2762 decisions 62 and 63), as the skill's tab and `purlis skill deprecate
//! | archive | owners` change them. Each change is written into the file beside the skill's folder
//! ([`purlis_skills::set_lifecycle`], [`purlis_skills::set_owners`]) and is one commit of that
//! file alone, through the plane's one save.
//!
//! Owners are informational for now: on an organisation's project, a change to an owned skill by
//! someone else goes through a review on the forge once the policy layer is there (SD-14, #697).

use std::collections::HashMap;
use std::path::Path;

use purlis_skills::{Deprecated, Lifecycle};

use super::verbs::{Ctx, Level, find, git, rel};
use super::{beside, named};

/// How many days a skill goes without a run before its tab suggests deprecating it.
pub const UNUSED_DAYS: u64 = 60;

/// **Sets the lifecycle state of the skill `name` at `level`** to `lifecycle`, committed. A
/// replacement must name another skill there is: one of the project's or a built-in one.
/// `Ok(false)` where it already was so, and nothing is written.
pub fn set(
    cx: &mut Ctx<'_>,
    level: &Level,
    name: &str,
    lifecycle: &Lifecycle,
) -> Result<bool, String> {
    let plane = cx.plane;
    crate::settings::writable(plane)?;
    let folder = find(plane, level, name)?;
    if let Lifecycle::Deprecated(Deprecated {
        replacement: Some(other),
    }) = lifecycle
    {
        replacement_there(cx, name, other)?;
    }
    let was = purlis_skills::about_of(&folder).map(|about| about.lifecycle);
    if was.as_ref() == Ok(lifecycle) {
        return Ok(false);
    }
    purlis_skills::set_lifecycle(&folder, lifecycle).map_err(|why| not_kept(name, &why))?;
    let word = lifecycle.word();
    if *lifecycle == Lifecycle::Archived
        && let Some(wider) = wider_copy(plane, level, name)
    {
        // An archived copy takes no name, so the next copy out loads where this one did.
        (cx.say)(crate::repocmd::Say::Info(format!(
            "The {wider} copy of {name} now loads in its place, wherever it reaches."
        )));
    }
    cx.done(match lifecycle {
        Lifecycle::Active => format!("{name} is in use again"),
        Lifecycle::Deprecated(Deprecated {
            replacement: Some(other),
        }) => format!("{name} is deprecated: use {other} instead"),
        Lifecycle::Deprecated(_) => format!("{name} is deprecated"),
        Lifecycle::Archived => format!("{name} is archived: it never loads until it is restored"),
    });
    cx.commit(
        &[rel(plane, &beside(&folder))],
        &format!("skill {word} {name}"),
    );
    Ok(true)
}

/// What a person is told where what purlis keeps beside the skill `name` could not be changed:
/// a sentence with no file in it, as the window's words are; what went wrong goes to the log.
fn not_kept(name: &str, why: &str) -> String {
    tracing::warn!(target: "purlis::skills", "what is kept beside {name} was not changed: {why}");
    format!(
        "What purlis keeps about {name} could not be changed, so nothing changed: it is not in a \
         form purlis reads, or could not be written."
    )
}

/// The nearest wider level than `level` that holds a skill named `name` in use: the copy that
/// loads in its place once it is archived (ADR 0099 §3, narrowest first).
fn wider_copy(plane: &Path, level: &Level, name: &str) -> Option<Level> {
    let mut wider: Vec<Level> = Vec::new();
    // A workspace's or a persona's level is narrower than the project's (ADR 0099 §3).
    let narrower = level.at != super::verbs::At::Project;
    if narrower && level.local {
        wider.push(level.clone().local(false));
    }
    if narrower {
        wider.push(Level::project().local(true));
    }
    if narrower || level.local {
        wider.push(Level::project());
    }
    wider.into_iter().find(|one| {
        one.dir(plane)
            .and_then(|dir| named(&dir, name))
            .is_some_and(|folder| {
                purlis_skills::about_of(&folder)
                    .map_or(true, |about| about.lifecycle != Lifecycle::Archived)
            })
    })
}

/// Refused where `other` cannot replace `name`: not a skill's name, the skill itself, or a name
/// no skill of the project's and no built-in skill has.
fn replacement_there(cx: &Ctx<'_>, name: &str, other: &str) -> Result<(), String> {
    if let Some(why) = purlis_skills::change::name_problem(other) {
        return Err(format!("{other:?} cannot replace {name}: {why}"));
    }
    if other == name {
        return Err(format!("{name} cannot replace itself"));
    }
    let in_the_project = Level::every(cx.plane)
        .iter()
        .filter_map(|level| level.dir(cx.plane))
        .any(|dir| named(&dir, other).is_some());
    let built_in = cx
        .built_in
        .is_some_and(|dir| super::read(dir).iter().any(|skill| skill.name == other));
    if in_the_project || built_in {
        Ok(())
    } else {
        Err(format!(
            "no skill is named {other}, so it cannot replace {name}: name one there is"
        ))
    }
}

/// **Sets the owners of the skill `name` at `level`** to `owners`, committed. `Ok(false)` where
/// they already were, and nothing is written.
pub fn set_owners(
    cx: &mut Ctx<'_>,
    level: &Level,
    name: &str,
    owners: &[String],
) -> Result<bool, String> {
    let plane = cx.plane;
    crate::settings::writable(plane)?;
    let folder = find(plane, level, name)?;
    if purlis_skills::about_of(&folder).is_ok_and(|about| about.owners == owners) {
        return Ok(false);
    }
    if let Some((owner, why)) = owners
        .iter()
        .find_map(|owner| purlis_skills::lifecycle::owner_problem(owner).map(|why| (owner, why)))
    {
        return Err(format!(
            "{owner:?} cannot be an owner: {why}, so nothing changed"
        ));
    }
    if owners.len() > purlis_skills::lifecycle::MOST_OWNERS {
        return Err(format!(
            "a skill names at most {} owners, so nothing changed",
            purlis_skills::lifecycle::MOST_OWNERS
        ));
    }
    purlis_skills::set_owners(&folder, owners).map_err(|why| not_kept(name, &why))?;
    cx.done(if owners.is_empty() {
        format!("{name} names no owners now")
    } else {
        format!("{name} is owned by {}", owners.join(", "))
    });
    cx.commit(
        &[rel(plane, &beside(&folder))],
        &format!("skill owners {name}"),
    );
    Ok(true)
}

/// A skill in use that no chat ran for [`UNUSED_DAYS`] or more: its tab suggests deprecating it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Unused {
    pub name: String,
    pub level: Level,
    /// How many whole days since its last run, or since it was made where it never ran.
    pub days: u64,
}

/// **Each skill of the project in use that no chat ran for [`UNUSED_DAYS`] days or more** by
/// `now` (seconds since 1970), at the project's and each workspace's levels: counted from its
/// last run of that copy, or, where it never ran, from when it was made (its first commit, or its
/// `SKILL.md`'s time for a skill git does not have). A deprecated or archived skill is not
/// suggested again. One read of the skill log and one of git.
pub fn unused(plane: &Path, now: u64) -> Vec<Unused> {
    // The last run of each copy, by the skill's name without a harness's namespace, its level
    // and its workspace: one pass over the log.
    let mut last: HashMap<(String, purlis_skills::runs::Level, Option<String>), u64> =
        HashMap::new();
    for run in crate::skilluse::runs(plane) {
        let (Some(copy), Some(at)) = (&run.copy, run.started_secs()) else {
            continue;
        };
        let key = (
            run.skill_name().to_owned(),
            copy.level,
            copy.workspace.clone(),
        );
        let held = last.entry(key).or_insert(at);
        *held = (*held).max(at);
    }
    let made = first_commits(plane);
    let mut out = Vec::new();
    for level in Level::every(plane) {
        let Some(dir) = level.dir(plane) else {
            continue;
        };
        let copy = super::history::copy_of(&level);
        for skill in super::read(&dir) {
            let folder = skill
                .path
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_default();
            let in_use = purlis_skills::about_of(&folder)
                .map(|about| about.lifecycle == Lifecycle::Active)
                .unwrap_or(true);
            if !in_use {
                continue;
            }
            let last_run = last
                .get(&(
                    skill.name.clone(),
                    copy,
                    level.workspace_name().map(str::to_owned),
                ))
                .copied();
            let since = last_run.or_else(|| made_at(plane, &made, &skill.path));
            let Some(since) = since else {
                continue;
            };
            let days = now.saturating_sub(since) / 86_400;
            if days >= UNUSED_DAYS {
                out.push(Unused {
                    name: skill.name.clone(),
                    level: level.clone(),
                    days,
                });
            }
        }
    }
    out
}

/// When the skill whose `SKILL.md` is at `path` was made: its first commit, else the file's own
/// time, read without following a link.
fn made_at(plane: &Path, made: &HashMap<String, u64>, path: &Path) -> Option<u64> {
    made.get(&rel(plane, path)).copied().or_else(|| {
        std::fs::symlink_metadata(path)
            .ok()
            .filter(std::fs::Metadata::is_file)
            .and_then(|meta| meta.modified().ok())
            .and_then(|at| at.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|at| at.as_secs())
    })
}

/// The first commit time of every shared skill's `SKILL.md` git has, by its plane-relative path:
/// one `git log` of the paths added.
fn first_commits(plane: &Path) -> HashMap<String, u64> {
    let globs = super::verbs::shared_globs();
    let mut args = vec![
        "log",
        "--diff-filter=A",
        "--no-renames",
        "--name-only",
        "--format=%x01%ct",
        "--",
    ];
    args.extend(globs.iter().map(String::as_str));
    let log = git(plane, &args).unwrap_or_default();
    let mut made = HashMap::new();
    for record in log.split('\u{1}').filter(|one| !one.trim().is_empty()) {
        let mut lines = record.lines().filter(|line| !line.trim().is_empty());
        let Some(at) = lines.next().and_then(|at| at.trim().parse::<u64>().ok()) else {
            continue;
        };
        // Newest first: the last seen is the first made.
        for path in lines {
            made.insert(path.trim().to_owned(), at);
        }
    }
    made
}

#[cfg(test)]
mod tests;

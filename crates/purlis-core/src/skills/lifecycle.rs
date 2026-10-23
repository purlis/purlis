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
    purlis_skills::set_lifecycle(&folder, lifecycle)?;
    let word = lifecycle.word();
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
    purlis_skills::set_owners(&folder, owners)?;
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
    let mut last: HashMap<(String, String, Option<String>), u64> = HashMap::new();
    for run in crate::skilluse::runs(plane) {
        let Some(copy) = &run.copy else {
            continue;
        };
        let Some(at) = chrono::DateTime::parse_from_rfc3339(&run.started)
            .ok()
            .and_then(|at| u64::try_from(at.timestamp()).ok())
        else {
            continue;
        };
        let leaf = run
            .skill
            .split_once(':')
            .map_or(run.skill.as_str(), |(_, leaf)| leaf);
        let key = (leaf.to_owned(), copy.level.word(), copy.workspace.clone());
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
                    copy.word(),
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
    let globs: Vec<String> = [None, Some("*")]
        .into_iter()
        .filter_map(|ws| purlis_skills::level_rel(ws, false))
        .map(|dir| format!(":(glob){dir}/*/SKILL.md"))
        .collect();
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

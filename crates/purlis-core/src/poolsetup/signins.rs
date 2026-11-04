// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The Sign-ins step's profiles, each against the pool's limit** (#4482, Q41, stories 28 to 30):
//! this machine's profiles, how much each would send to the pool's runner and whether that is over
//! what the pool lets a profile send, and which are in use, to be ticked first.
//!
//! It is measured as a send measures it ([`crate::profilestore::settings_measured`]), with no
//! file read, and held to the pool's limits as a send is held
//! ([`crate::profilestore::onrunner::hits_against`]), so what the step says over is what a start
//! there refuses.

use std::collections::BTreeSet;
use std::path::Path;

use crate::poollimits::{Chosen, LimitHit, LimitId};
use crate::runners::{self, Refused};

/// What a mebibyte is, in bytes.
const MIB: u64 = 1024 * 1024;

/// One profile, measured.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sized {
    pub name: String,
    /// How much of it a send carries, rounded up to a MiB, and in how many files. `None` where it
    /// could not be measured, with why in `unread`.
    pub mib: Option<u64>,
    pub files: Option<u64>,
    pub unread: Option<String>,
    /// The pool's limits it is over: each with what to raise it to, where there is a value.
    pub hits: Vec<LimitHit>,
    /// The project's default, or a profile an open chat runs on.
    pub in_use: bool,
}

/// This machine's profiles against pool `pool`'s limits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sizes {
    /// What the pool lets one profile send, in MiB and in files.
    pub most_mib: u64,
    pub most_files: u64,
    pub profiles: Vec<Sized>,
}

/// **One profile of `measured` bytes in `files`, held to `limits`.**
#[must_use]
pub fn sized(
    name: &str,
    measured: Result<(u64, u64), String>,
    limits: &Chosen,
    highest: &Chosen,
    in_use: bool,
) -> Sized {
    match measured {
        Ok((bytes, files)) => Sized {
            name: name.to_owned(),
            mib: Some(bytes.div_ceil(MIB)),
            files: Some(files),
            unread: None,
            hits: crate::profilestore::onrunner::hits_against(limits, highest, bytes, files),
            in_use,
        },
        Err(why) => Sized {
            name: name.to_owned(),
            mib: None,
            files: None,
            unread: Some(crate::shown::short(&why)),
            hits: Vec::new(),
            in_use,
        },
    }
}

/// **The profiles in use in the project at `plane`**: its default, and those its open chats run.
/// Nothing is read but the project's own record, so a long history costs nothing.
#[must_use]
pub fn in_use(plane: &Path) -> BTreeSet<String> {
    let mut used: BTreeSet<String> = crate::profiles::current(plane)
        .default
        .into_iter()
        .collect();
    if let Ok(record) = crate::reopen::read_or_refusal(plane) {
        used.extend(record.chats.into_iter().filter_map(|chat| chat.profile));
    }
    used
}

/// **This machine's profiles against pool `pool`'s limits**, with those in use in `plane` marked.
pub fn sizes(
    config: &Path,
    plane: Option<&Path>,
    pool: &str,
    highest: &Chosen,
) -> Result<Sizes, Refused> {
    let limits = runners::chosen_limits_of(config, pool)?;
    let records = crate::profilestore::list(config).map_err(Refused::Other)?;
    let used = plane.map(in_use).unwrap_or_default();
    let profiles = records
        .iter()
        .map(|record| {
            let measured = measured(record);
            sized(
                &record.name,
                measured,
                &limits,
                highest,
                used.contains(&record.name),
            )
        })
        .collect();
    Ok(Sizes {
        most_mib: most(&limits, highest, LimitId::ProfileSize),
        most_files: most(&limits, highest, LimitId::ProfileFiles),
        profiles,
    })
}

/// **Records the profiles `names` on pool `pool`** (#4718): what Sign-ins' button and Add a
/// profile… in the window do, and what `purlis pool add-profile` does, with the same checks and
/// words. It only records: nothing is sent, and the pool's runner keeps them the next time its
/// profiles are asked. A pool that names none keeps the default of the project at `project`
/// beside them, so with no project open it is refused. Each name must be one of this machine's
/// profiles that goes to a runner, checked before anything is written.
pub fn record(
    config: &Path,
    project: Option<&Path>,
    pool: &str,
    names: &[String],
) -> Result<(), String> {
    changeable(config, pool, project.is_some())?;
    let records = crate::profilestore::list(config)?;
    for name in names {
        let record = records.iter().find(|p| &p.name == name).ok_or_else(|| {
            format!(
                "this machine has no profile named '{}', so none was added to pool {pool}",
                crate::shown::short(name)
            )
        })?;
        crate::profilestore::onrunner::Kept::of(record)?;
    }
    let default = project
        .and_then(|root| crate::profiles::for_launch(root).0.for_a_new_chat())
        .filter(|default| records.iter().any(|r| &r.name == default));
    for name in names {
        runners::add_pool_profile(config, pool, name, default.as_deref())
            .map_err(|why| why.to_string())?;
    }
    Ok(())
}

/// **Refused, in words, where pool `pool` names no profiles and no project is open**
/// (#4047): what it keeps is the open project's default, so a change has nothing to start from.
pub fn changeable(config: &Path, pool: &str, project_open: bool) -> Result<(), String> {
    let names_none = runners::profiles_of(config, pool)
        .map_err(|why| why.to_string())?
        .is_none();
    if names_none && !project_open {
        return Err(format!(
            "pool {pool} keeps the profile a new chat in the open project gets, and no project \
             is open; open one to change its profiles"
        ));
    }
    Ok(())
}

/// **What Sign-ins' button runs, by hand** (#4718, Q30): `adding` recorded on pool `pool`, then
/// each of `signing_in` signed in there. None where it would run nothing.
#[must_use]
pub fn by_hand(pool: &str, adding: &[String], signing_in: &[String]) -> Option<super::See> {
    let pool = if runners::usable_pool_name(pool).is_ok() {
        crate::shellseg::shell_quote(pool)
    } else {
        "POOL".to_owned()
    };
    let profile = |name: &String| {
        if crate::profiles::name_ok(name) {
            crate::shellseg::shell_quote(name)
        } else {
            "PROFILE".to_owned()
        }
    };
    let mut lines = Vec::new();
    if !adding.is_empty() {
        let names: Vec<String> = adding.iter().map(profile).collect();
        lines.push(format!(
            "purlis pool add-profile {pool} {}",
            names.join(" ")
        ));
    }
    lines.extend(
        signing_in
            .iter()
            .map(|name| format!("purlis profile sign-in {} --pool {pool}", profile(name))),
    );
    (!lines.is_empty()).then(|| super::See {
        note: "These record the ticked profiles on the pool, then sign in each one that needs \
               it. Run them yourself if you prefer, and purlis picks them up."
            .to_owned(),
        text: lines.join("\n"),
    })
}

/// The most limit `id` lets a profile send: the pool's, held to what its runner takes, as a send
/// is held ([`crate::profilestore::onrunner::hits_against`]).
fn most(limits: &Chosen, highest: &Chosen, id: LimitId) -> u64 {
    let chosen = limits.in_force(id);
    highest.get(id).map_or(chosen, |top| top.min(chosen))
}

/// What a send of `record` to a runner carries: its bytes and its files.
fn measured(record: &crate::profilestore::MachineProfile) -> Result<(u64, u64), String> {
    let kept = crate::profilestore::onrunner::Kept::of(record)?.with_settings_of(record);
    let Some(folder) = &kept.settings_in else {
        return Ok((0, 0));
    };
    let wholes = crate::profilestore::settings_measured(&kept.harness, folder)?;
    Ok(wholes
        .values()
        .fold((0, 0), |(bytes, files), (more, count)| {
            (
                bytes + more,
                files + u64::try_from(*count).unwrap_or(u64::MAX),
            )
        }))
}

#[cfg(test)]
mod tests;

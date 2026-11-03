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
    in_use: bool,
) -> Sized {
    match measured {
        Ok((bytes, files)) => Sized {
            name: name.to_owned(),
            mib: Some(bytes.div_ceil(MIB)),
            files: Some(files),
            unread: None,
            hits: crate::profilestore::onrunner::hits_against(
                limits,
                &Chosen::default(),
                bytes,
                files,
            ),
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
pub fn sizes(config: &Path, plane: Option<&Path>, pool: &str) -> Result<Sizes, Refused> {
    let limits = runners::chosen_limits_of(config, pool)?;
    let records = crate::profilestore::list(config).map_err(Refused::Other)?;
    let used = plane.map(in_use).unwrap_or_default();
    let profiles = records
        .iter()
        .map(|record| {
            let measured = measured(record);
            sized(&record.name, measured, &limits, used.contains(&record.name))
        })
        .collect();
    Ok(Sizes {
        most_mib: limits.in_force(LimitId::ProfileSize),
        most_files: limits.in_force(LimitId::ProfileFiles),
        profiles,
    })
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

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **This machine's profiles on its pools' runners** (#2049, spec #2041's *Profiles on runners*).
//!
//! A chat on a pool's runner starts on a profile the runner keeps of its own: the desktop has
//! its runner keep each of its profiles (`keep_profile`, session protocol 1.43), over a link of
//! its own to it, in a folder the runner makes, with nothing of this machine's folder in it
//! ([`purlis_core::profilestore::onrunner`]). A subscription is signed in there through its
//! harness's own flow, run on the runner from a shell tab here ([`sign_in_line`]), once per
//! device: no sign-in is copied between devices.
//!
//! **Only a runner purlis can show is the person's own keeps a profile and is signed in**
//! ([`owned`]): until a pool can be marked owned (#2561), a Docker pool whose engine is a local
//! socket on this machine, as for browser logins and tools (D-2050-4). Any other pool is refused
//! in words, and nothing is sent to it.
//!
//! Adding a pool ends with its profiles (Settings › Pools, `pool_profiles`); a start on a pool
//! keeps its profile there first ([`Linked`]): signed in, it starts with nobody asked, a moved
//! chat's too, and signed out, it waits as one Ask, *Sign in <profile> on pool <pool>*.

use std::path::{Path, PathBuf};
use std::time::Duration;

use purlis_core::profilestore::{self, onrunner, sign_in::Standing};

use crate::runnerlinks::{Machine, ToARunner};

/// **Readies a chat's profile on the pool's runner it starts on** (#2049): what a start on a
/// pool asks before it is sent there.
pub trait ProfilesOnPools: Send + Sync {
    /// `Ok` where a chat in the project at `root` on `profile` may start on pool `pool`'s
    /// runner, or why not, in words.
    fn ready(&self, root: &Path, pool: &str, profile: &str) -> Result<(), String>;
}

/// The runners this machine keeps under its config home, each reached over a link of its own.
pub struct Linked {
    config: PathBuf,
}

impl Linked {
    pub fn at(config: PathBuf) -> Self {
        Self { config }
    }
}

impl ProfilesOnPools for Linked {
    fn ready(&self, root: &Path, pool: &str, profile: &str) -> Result<(), String> {
        onrunner::ready_on(&self.config, root, pool, profile, &|kept| {
            keep_on(&self.config, pool, kept)
        })
    }
}

/// **Whether pool `pool`'s runner is shown to be the person's own** (#2049, D-2050-4): a Docker
/// pool whose engine is a local socket on this machine. The socket, or why purlis keeps no
/// profile there and signs none in.
pub fn owned() -> Result<PathBuf, String> {
    purlis_core::dockerpool::Docker::engine_on_this_machine().map_err(|why| {
        format!(
            "{why}, so its runners are not known to be yours, and purlis keeps no profile there \
             and signs none in until a pool can be marked as yours"
        )
    })
}

/// **Has pool `pool`'s runner, as this machine keeps it under `config`, keep `kept`**, over a
/// link of its own: whether its harness says it is signed in there. Only a runner shown to be
/// the person's own ([`owned`]) is sent anything.
pub fn keep_on(config: &Path, pool: &str, kept: &onrunner::Kept) -> Result<Standing, String> {
    owned()?;
    let to = ToARunner::of(&Machine::here(config)?, pool)?;
    crate::runnerlinks::keep_profile(&to, kept.clone().into())
        .map(|word| onrunner::standing_of_word(&word))
}

/// One of this machine's profiles, as a pool's runner stands with it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PoolProfileRow {
    pub name: String,
    /// The harness, as a profile's kind names it.
    pub kind: String,
    /// The harness as people call it: `Claude Code`.
    pub harness: String,
    /// Whether its harness says it is signed in on the runner: `signed-in`, `signed-out` or
    /// `cannot-tell`. Null where it is not kept there.
    pub standing: Option<String>,
    /// Why it is not kept on the runner, in words, where it is not.
    pub not_kept: Option<String>,
    /// The plans Sign in offers there, where purlis names the provider.
    pub plans: Vec<crate::machineprofiles::PlanRow>,
}

/// **Every profile of this machine's, kept on pool `pool`'s runner**, as the runner stands with
/// each (#2049): what adding a pool ends with, and what Settings › Pools shows. Refused, with
/// nothing sent, for a pool not shown to be the person's own.
#[tauri::command]
#[specta::specta]
pub async fn pool_profiles(pool: String) -> Result<Vec<PoolProfileRow>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let config = config_root()?;
        owned()?;
        let records = profilestore::list(&config)?;
        let to = ToARunner::of(&Machine::here(&config)?, &pool)?;
        // Each that goes to a runner, kept there over one link.
        let kept: Vec<_> = records.iter().map(onrunner::Kept::of).collect();
        let sent: Vec<_> = kept
            .iter()
            .filter_map(|kept| kept.as_ref().ok().cloned().map(Into::into))
            .collect();
        let mut said = crate::runnerlinks::keep_profiles(&to, sent)?.into_iter();
        Ok(records
            .iter()
            .zip(kept)
            .map(|(record, kept)| {
                let standing = kept.and_then(|_| {
                    said.next()
                        .unwrap_or_else(|| Err("the runner did not answer".to_owned()))
                        .map(|word| onrunner::standing_of_word(&word))
                });
                row(record, standing)
            })
            .collect())
    })
    .await
    .map_err(|why| format!("the pool's profiles did not answer: {why}"))?
}

/// `record`'s row on a pool's runner: where it stands there, or why it is not kept there.
fn row(record: &profilestore::MachineProfile, kept: Result<Standing, String>) -> PoolProfileRow {
    let (standing, not_kept) = match kept {
        Ok(standing) => (Some(standing.word().to_owned()), None),
        Err(why) => (None, Some(why)),
    };
    PoolProfileRow {
        name: record.name.clone(),
        kind: record.harness.clone(),
        harness: purlis_core::harness_declaration::builtin(&record.harness)
            .map_or_else(|| record.harness.clone(), |d| d.title.clone()),
        standing,
        not_kept,
        plans: profilestore::sign_in::providers(&record.harness)
            .iter()
            .map(|plan| crate::machineprofiles::PlanRow {
                id: plan.provider.clone(),
                title: plan.title.clone(),
            })
            .collect(),
    }
}

fn config_root() -> Result<PathBuf, String> {
    purlis_core::machine::config_root()
        .ok_or_else(|| "There is no config home on this machine to keep a pool in.".to_owned())
}

/// **The line a shell here runs to sign `name` in on pool `pool`'s runner** (#2049), to
/// `provider`'s plan where one is named: the runner's own `purlis profile sign-in`, through
/// this machine's `docker` on the engine shown to be the person's own, in the container as it
/// was made, checked first. Only one of this machine's profiles that goes to a runner, kept
/// there first, so the runner has it to sign in.
pub fn sign_in_line(pool: &str, name: &str, provider: Option<&str>) -> Result<String, String> {
    let config = config_root()?;
    let engine = owned()?;
    let record = profilestore::list(&config)?
        .into_iter()
        .find(|p| p.name == name)
        .ok_or_else(|| {
            format!(
                "this machine has no profile named '{}', so no sign-in was typed.",
                purlis_core::shown::short(name)
            )
        })?;
    let kept = onrunner::Kept::of(&record)?;
    keep_on(&config, pool, &kept)?;
    let runner = purlis_core::runners::runner_for(&config, pool).map_err(|why| why.to_string())?;
    crate::runnerlinks::docker()?.sign_in_line(&runner.made, &engine, name, provider)
}

/// How often, and for how long, a profile's standing on a pool is asked again after its Sign
/// in, so its Ask leaves the Inbox once it is signed in there.
const WATCHED_EVERY: Duration = Duration::from_secs(5);
const WATCHED_FOR: Duration = Duration::from_secs(15 * 60);

/// **Asks pool `pool`'s runner again every few seconds whether `name` is signed in there**, for
/// a while after its Sign in, and once it is not signed out, lets its waits go and calls
/// `told`. On a thread of its own.
pub fn watch(pool: String, name: String, told: impl FnOnce() + Send + 'static) {
    std::thread::spawn(move || {
        let Ok(config) = config_root() else {
            return;
        };
        let Some(kept) = profilestore::list(&config)
            .ok()
            .and_then(|records| records.into_iter().find(|p| p.name == name))
            .and_then(|record| onrunner::Kept::of(&record).ok())
        else {
            return;
        };
        let ends = std::time::Instant::now() + WATCHED_FOR;
        while std::time::Instant::now() < ends {
            std::thread::sleep(WATCHED_EVERY);
            if matches!(
                keep_on(&config, &pool, &kept),
                Ok(Standing::SignedIn | Standing::CannotTell)
            ) {
                onrunner::let_go(None, &pool, &name);
                told();
                return;
            }
        }
    });
}

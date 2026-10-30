// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **This machine's profiles on its pools' runners** (#2049, spec #2041's *Profiles on runners*).
//!
//! A chat on a pool's runner starts on a profile the runner keeps of its own: the desktop has
//! its runner keep each of its profiles (`keep_profile`, session protocol 1.43), over a link of
//! its own to it, with nothing of this machine's folder in it
//! ([`purlis_core::profilestore::onrunner`]). A subscription is signed in there through its
//! harness's own flow, run on the runner from a shell tab here ([`sign_in_line`]), once per
//! device: no sign-in is copied between devices.
//!
//! **Only a runner purlis can show is the person's own is sent a profile or signed in**
//! ([`owned`], the check browser logins and tools use, D-2050-4): until a pool can be marked
//! owned (#2561), a Docker pool whose engine is a local socket on this machine. To any other
//! pool nothing is sent, and its chats start on the profiles its runner has of its own; its
//! profiles are not offered for sign-in. A runner of a purlis before 1.43 is sent nothing either.
//!
//! Adding a pool ends with its profiles (Settings › Pools, `pool_profiles`); a start on a pool
//! keeps its profile there first ([`Linked`]): signed in, it starts with nobody asked, a moved
//! chat's too, and signed out, it waits as one Ask, *Profile <profile> on pool <pool>*, whose
//! Sign in on pool <pool> signs it in there.

use std::cell::RefCell;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use purlis_core::profilestore::sign_in::{Standing, driven};
use purlis_core::profilestore::{self, onrunner};

use crate::runnerlinks::{Machine, ToARunner};

/// **Readies a chat's profile on the pool's runner it starts on** (#2049): what a start on a
/// pool asks before it is sent there.
pub trait ProfilesOnPools: Send + Sync {
    /// `Ok` where a chat in the project at `root` on `profile` may start on pool `pool`'s
    /// runner, or why not, in words. `Ok` carries what the chat's tab says of settings that
    /// stayed behind (#4081), where some did.
    fn ready(&self, root: &Path, pool: &str, profile: &str) -> Result<Option<String>, String>;

    /// [`Self::ready`], for a chat that moves to pool `pool`'s runner (#2048): said of the move.
    /// What stayed behind is said by the chat's start there, which keeps its profile again.
    fn ready_to_move(&self, root: &Path, pool: &str, profile: &str) -> Result<(), String> {
        self.ready(root, pool, profile).map(|_| ())
    }
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
    fn ready(&self, root: &Path, pool: &str, profile: &str) -> Result<Option<String>, String> {
        let behind = RefCell::default();
        // A chat's start is what wakes a suspended runner (#2051), so its keep does too.
        onrunner::ready_on(&self.config, root, pool, profile, &|kept| {
            keep_on_as(owned(), &self.config, pool, kept, Waking::Yes, &behind)
        })?;
        Ok(onrunner::left_behind_note(
            pool,
            profile,
            &behind.into_inner(),
        ))
    }

    fn ready_to_move(&self, root: &Path, pool: &str, profile: &str) -> Result<(), String> {
        onrunner::ready_to_move_on(&self.config, root, pool, profile, &|kept| {
            keep_on_as(
                owned(),
                &self.config,
                pool,
                kept,
                Waking::Yes,
                &RefCell::default(),
            )
        })
    }
}

/// **Whether this machine's pools' runners are shown to be the person's own** (#2049, by
/// [`purlis_core::poollogins::owned`], D-2050-4): the engine's local socket, or why not.
pub fn owned() -> Result<PathBuf, String> {
    let engine = purlis_core::dockerpool::Docker::engine_on_this_machine();
    purlis_core::poollogins::owned(engine.clone())?;
    engine
}

/// **Has pool `pool`'s runner, as this machine keeps it under `config`, keep `kept`**, over a
/// link of its own: whether its harness says it is signed in there. `None`, with nothing sent,
/// for a runner not shown to be the person's own ([`owned`]) and one of a purlis before 1.43:
/// what it runs on is its own to say.
pub fn keep_on(
    config: &Path,
    pool: &str,
    kept: &onrunner::Kept,
) -> Result<Option<Standing>, String> {
    keep_on_as(owned(), config, pool, kept, Waking::No, &RefCell::default())
}

/// Whether a link made to keep a profile starts a suspended runner again (#2051): only a chat's
/// start does; a look, a poll and a sign-in leave it as it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Waking {
    Yes,
    No,
}

/// The runner of pool `pool` this machine keeps under `config`, linked to waking it or not.
fn to_a_runner(config: &Path, pool: &str, waking: Waking) -> Result<ToARunner, String> {
    let machine = Machine::here(config)?;
    let kept = purlis_core::runners::runner_for(config, pool).map_err(|why| why.to_string())?;
    match waking {
        Waking::Yes => ToARunner::of_kept_waking(&machine, &kept),
        Waking::No => ToARunner::of_kept(&machine, &kept),
    }
}

/// [`keep_on`], with `owned` what [`owned`] said of this machine's pools: nothing is read or
/// sent where it said no. `behind` is told what of its settings stayed behind (#4081).
fn keep_on_as(
    owned: Result<PathBuf, String>,
    config: &Path,
    pool: &str,
    kept: &onrunner::Kept,
    waking: Waking,
    behind: &RefCell<Vec<onrunner::LeftBehind>>,
) -> Result<Option<Standing>, String> {
    if owned.is_err() {
        return Ok(None);
    }
    // A cluster pool's runner is a team's cluster's, never shown to be the person's own: no
    // profile of this machine's is sent there, and its chats start on its own (#3292).
    if purlis_core::runners::cluster_runner_for(config, pool)
        .map_err(|why| why.to_string())?
        .is_some()
    {
        return Ok(None);
    }
    // Its settings, where it carries them, are read only once the pool is shown to be owned
    // and its runner is one that keeps them (1.50, #3448), and sent to no other.
    let to = to_a_runner(config, pool, waking)?;
    let Some(said) = crate::runnerlinks::keep_profiles(&to, vec![kept.clone()])? else {
        return Ok(None);
    };
    said.into_iter()
        .next()
        .unwrap_or_else(|| Err("the runner did not answer".to_owned()))
        .map(|kept| {
            behind.replace(kept.left_behind);
            Some(onrunner::standing_on_the_wire(kept.standing))
        })
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

/// Why a runner of a purlis before 1.43 keeps none of this machine's profiles.
const OLDER_RUNNER: &str = "its runner runs a purlis before 1.43, so its chats start on the \
                            profiles it has of its own";

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
        let to = to_a_runner(&config, &pool, Waking::No)?;
        // Each that goes to a runner, kept there over one link.
        let kept: Vec<_> = records.iter().map(onrunner::Kept::of).collect();
        let sent: Vec<_> = kept
            .iter()
            .filter_map(|kept| kept.as_ref().ok().cloned())
            .collect();
        let said = crate::runnerlinks::keep_profiles(&to, sent)?;
        let mut said = said.map(Vec::into_iter);
        Ok(records
            .iter()
            .zip(kept)
            .map(|(record, kept)| {
                let standing = kept.and_then(|_| match said.as_mut() {
                    None => Err(OLDER_RUNNER.to_owned()),
                    Some(said) => said
                        .next()
                        .unwrap_or_else(|| Err("the runner did not answer".to_owned()))
                        .map(|kept| onrunner::standing_on_the_wire(kept.standing)),
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
/// `provider`'s plan where its harness serves several: the runner's own `purlis profile
/// sign-in`, through this machine's `docker` with settings of purlis's own, on the engine shown
/// to be the person's own, in the container as it was made, checked first. Only one of this
/// machine's profiles that goes to a runner, kept there first, so the runner has it to sign in.
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
    profilestore::sign_in::plan_named(&record.harness, provider)?;
    let kept = onrunner::Kept::of(&record)?;
    if keep_on(&config, pool, &kept)?.is_none() {
        return Err(format!(
            "Pool {pool}: {OLDER_RUNNER}. No sign-in was typed."
        ));
    }
    let runner = purlis_core::runners::runner_for(&config, pool).map_err(|why| why.to_string())?;
    crate::runnerlinks::docker()?.sign_in_line(&runner.made, &engine, name, provider)
}

/// **One step of a sign-in on a pool's runner** that purlis drives with no terminal (#4042), as
/// the window's sign-in strip shows it ([`driven::Event`]).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(tag = "at", rename_all = "kebab-case")]
pub enum PoolSignInStep {
    /// Open this page in the browser here: the harness vendor's own.
    Open { url: String },
    /// Ask for the code the page shows.
    Code,
    /// The harness refused the code, in its words.
    Refused { said: String },
    /// The login ended well, and the runner is asked whether it is signed in.
    Checking,
    /// The runner's own status check says it is signed in.
    SignedIn,
    /// purlis could not follow the login: what it printed.
    CouldNotFollow { said: String },
    /// It did not sign in, and why.
    Failed { why: String },
    /// It was cancelled, and its login ended.
    Cancelled,
}

impl From<driven::Event> for PoolSignInStep {
    fn from(event: driven::Event) -> Self {
        match event {
            driven::Event::Open { url } => Self::Open { url },
            driven::Event::AskCode => Self::Code,
            driven::Event::Refused { said } => Self::Refused { said },
            driven::Event::Checking => Self::Checking,
            driven::Event::SignedIn => Self::SignedIn,
            driven::Event::CouldNotFollow { said } => Self::CouldNotFollow { said },
            driven::Event::Failed { why } => Self::Failed { why },
            driven::Event::Cancelled => Self::Cancelled,
        }
    }
}

/// A sign-in purlis drives on a pool's runner, held in this process for its length alone.
struct SignIn {
    flow: u32,
    pool: String,
    name: String,
    driving: Arc<driven::Driving>,
}

static SIGN_INS: Mutex<Vec<SignIn>> = Mutex::new(Vec::new());
static NEXT_SIGN_IN: AtomicU32 = AtomicU32::new(1);

/// How long one ask for a sign-in's next step waits before it answers there is none yet.
const STEP_WITHIN: Duration = Duration::from_secs(20);

/// **Starts signing `name` in on pool `pool`'s runner with no terminal** (#4042, spec #4023):
/// the runner's own `purlis profile sign-in`, kept there first and run there through this
/// machine's `docker` as [`sign_in_line`]'s is, but with pipes, its output read for the steps
/// [`pool_sign_in_next`] answers. Nothing of this machine's sign-in goes to the runner (ADR
/// 0095). A sign-in of the same profile on the same pool already running is cancelled. Answers
/// the sign-in's number.
#[tauri::command]
#[specta::specta]
pub async fn pool_sign_in_start(
    pool: String,
    name: String,
    provider: Option<String>,
) -> Result<u32, String> {
    // Numbered as it is asked, so of two starts the one asked last is the one kept.
    let flow = NEXT_SIGN_IN.fetch_add(1, Ordering::SeqCst);
    tauri::async_runtime::spawn_blocking(move || {
        let config = config_root()?;
        let engine = owned()?;
        let record = record_named(&config, &name)?;
        profilestore::sign_in::plan_named(&record.harness, provider.as_deref())?;
        let kept = onrunner::Kept::of(&record)?;
        if keep_on(&config, &pool, &kept)?.is_none() {
            return Err(format!(
                "Pool {pool}: {OLDER_RUNNER}. Nothing was signed in."
            ));
        }
        let runner =
            purlis_core::runners::runner_for(&config, &pool).map_err(|why| why.to_string())?;
        let login = crate::runnerlinks::docker()?.driven_sign_in(
            &runner.made,
            &engine,
            &name,
            provider.as_deref(),
        )?;
        let check = {
            let pool = pool.clone();
            move || match keep_on(&config, &pool, &kept) {
                Ok(Some(standing)) => standing,
                _ => Standing::CannotTell,
            }
        };
        let driving = Arc::new(driven::drive(&login, &record.harness, check)?);
        // One of the same profile on the same pool is let go and ended, after the lock.
        let before: Vec<SignIn> = {
            let mut held = SIGN_INS.lock().unwrap_or_else(PoisonError::into_inner);
            let (same, others): (Vec<_>, Vec<_>) = std::mem::take(&mut *held)
                .into_iter()
                .partition(|one| one.pool == pool && one.name == name);
            *held = others;
            let (later, before): (Vec<_>, Vec<_>) =
                same.into_iter().partition(|one| one.flow > flow);
            if later.is_empty() {
                held.push(SignIn {
                    flow,
                    pool,
                    name,
                    driving,
                });
                before
            } else {
                held.extend(later);
                drop(held);
                driving.cancel();
                for one in before {
                    one.driving.cancel();
                }
                return Err("a later sign-in of this profile took its place".to_owned());
            }
        };
        for one in before {
            one.driving.cancel();
        }
        Ok(flow)
    })
    .await
    .map_err(|why| format!("the sign-in did not start: {why}"))?
}

/// **The next step of sign-in `flow`**, waiting a while for one; null while there is none yet.
/// After its last step, the sign-in is let go; signed in, its waits on the pool are too.
#[tauri::command]
#[specta::specta]
pub async fn pool_sign_in_next(flow: u32) -> Result<Option<PoolSignInStep>, String> {
    let driving = driving(flow)?;
    let event = tauri::async_runtime::spawn_blocking(move || driving.next(STEP_WITHIN))
        .await
        .map_err(|why| format!("the sign-in did not answer: {why}"))?;
    if let Some(event) = event.as_ref().filter(|event| event.is_last()) {
        let mut held = SIGN_INS.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(at) = held.iter().position(|one| one.flow == flow) {
            let over = held.remove(at);
            if *event == driven::Event::SignedIn {
                onrunner::let_go(None, &over.pool, &over.name);
            }
        }
    }
    Ok(event.map(PoolSignInStep::from))
}

/// **Passes the code the person pasted to sign-in `flow`**, as one line.
#[tauri::command]
#[specta::specta]
pub async fn pool_sign_in_paste(flow: u32, code: String) -> Result<(), String> {
    let driving = driving(flow)?;
    tauri::async_runtime::spawn_blocking(move || driving.paste(&code))
        .await
        .map_err(|why| format!("the code was not passed on: {why}"))?
}

/// **Cancels sign-in `flow`**: what drives its login is ended, and it is let go.
#[tauri::command]
#[specta::specta]
pub fn pool_sign_in_cancel(flow: u32) {
    let gone: Vec<SignIn> = {
        let mut held = SIGN_INS.lock().unwrap_or_else(PoisonError::into_inner);
        let (gone, kept) = std::mem::take(&mut *held)
            .into_iter()
            .partition(|one| one.flow == flow);
        *held = kept;
        gone
    };
    for one in gone {
        one.driving.cancel();
    }
}

fn driving(flow: u32) -> Result<Arc<driven::Driving>, String> {
    SIGN_INS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .iter()
        .find(|one| one.flow == flow)
        .map(|one| Arc::clone(&one.driving))
        .ok_or_else(|| "that sign-in is over".to_owned())
}

fn record_named(config: &Path, name: &str) -> Result<profilestore::MachineProfile, String> {
    profilestore::list(config)?
        .into_iter()
        .find(|p| p.name == name)
        .ok_or_else(|| {
            format!(
                "this machine has no profile named '{}', so nothing was signed in.",
                purlis_core::shown::short(name)
            )
        })
}

/// How often, and for how long, a profile's standing on a pool is asked again after its Sign
/// in, so its Ask leaves the Inbox once it is signed in there.
const WATCHED_EVERY: Duration = Duration::from_secs(5);
const WATCHED_FOR: Duration = Duration::from_secs(15 * 60);

/// The (pool, profile) pairs a watcher asks about now: one each, however often Sign in is
/// pressed.
static WATCHED: Mutex<Option<HashSet<(String, String)>>> = Mutex::new(None);

/// **Asks pool `pool`'s runner again every few seconds whether `name` is signed in there**, for
/// a while after its Sign in, and once it is not signed out, lets its waits go and calls
/// `told`. On a thread of its own, one per pool and profile: a second Sign in while one asks
/// starts none.
pub fn watch(pool: String, name: String, told: impl FnOnce() + Send + 'static) {
    let key = (pool.clone(), name.clone());
    if !claimed(
        WATCHED
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get_or_insert_with(HashSet::new),
        &key,
    ) {
        return;
    }
    std::thread::spawn(move || {
        watching(&pool, &name, told);
        if let Some(watched) = WATCHED
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_mut()
        {
            watched.remove(&key);
        }
    });
}

/// Whether `key` is claimed for a watcher now, in `watched`: only where none watches it.
fn claimed(watched: &mut HashSet<(String, String)>, key: &(String, String)) -> bool {
    watched.insert(key.clone())
}

fn watching(pool: &str, name: &str, told: impl FnOnce()) {
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
            keep_on(&config, pool, &kept),
            Ok(Some(Standing::SignedIn | Standing::CannotTell))
        ) {
            onrunner::let_go(None, pool, name);
            told();
            return;
        }
    }
}

#[cfg(test)]
mod tests;

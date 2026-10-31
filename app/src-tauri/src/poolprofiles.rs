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
//! ([`yours`], D-2050-4): a Docker pool whose engine is a local socket on this machine
//! ([`owned`]), or a cluster pool the person marked *This is my own* whose claim the cluster's
//! operator vouches for at that moment (#3819, the gate values and tokens pass, #4071). To any
//! other pool nothing is sent, and its chats start on the profiles its runner has of its own; its
//! profiles are not offered for sign-in. A runner of a purlis before 1.43 is sent nothing either.
//! A cluster runner signs a profile in over this machine's own `kubectl exec` into the pod it was
//! paired with, as a Docker runner's does over `docker exec`; where it lives decides how long it
//! lasts: a kept claim's home outlives its pod, a fresh claim's lasts only for that claim.
//!
//! **A pool keeps only the profiles chosen for it** (#4047, `profiles` in runners.json): the
//! ones it names, or, where it names none, the profile a new chat in the open project gets. Its
//! row adds one (`pool_profile_add`) and removes one (`pool_profile_remove`), which the runner
//! forgets, its sign-in there with it; a chat that starts on or moves to it on another adds it.
//!
//! **A runner keeps only what is chosen for its pool** (#4060, #3849): as a pool's profiles are
//! asked, and as the first chat this run starts on it, a runner of 1.57 or later names the
//! profiles it keeps for this machine; it forgets each one no longer chosen there or no longer
//! on this machine, and a pool made before #4047 names what it kept ([`tidy`]). A profile
//! removed on this machine is forgotten on each pool's runner there and then
//! ([`forget_off_the_pools`]), by Remove in the window or `purlis profile remove` in the
//! person's own terminal ([`listen_for_the_command_line`]), and one that is away is asked again
//! later. A marked cluster pool's runner is held to this too, each time behind [`yours`] (#4183).
//! A runner draining after an update is not tidied (#4142): its chats finish on what it has, and
//! its home goes with it.
//!
//! Adding a pool ends with its profiles (Settings › Pools, `pool_profiles`); a start on a pool
//! keeps its profile there first ([`Linked`]): signed in, it starts with nobody asked, a moved
//! chat's too, and signed out, it waits as one Ask, *Profile <profile> on pool <pool>*, whose
//! Sign in on pool <pool> signs it in there.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use purlis_core::dockerpool::Typed;
use purlis_core::profilestore::sign_in::{Standing, callback, driven};
use purlis_core::profilestore::{self, onrunner};

use crate::runnerlinks::{Machine, ToARunner};

/// **Readies a chat's profile on the pool's runner it starts on** (#2049): what a start on a
/// pool asks before it is sent there.
pub trait ProfilesOnPools: Send + Sync {
    /// `Ok` where a chat in the project at `root` on `profile` may start on pool `pool`'s
    /// runner, or why not, in words. `Ok` carries what the chat's tab says of settings that
    /// stayed behind (#4081), where some did.
    fn ready(&self, root: &Path, pool: &str, profile: &str) -> Result<Option<String>, String>;

    /// [`Self::ready`], for a chat that moves to pool `pool`'s runner (#2048): said of the move,
    /// and a profile signed out there is the sign-in the move waits on (#3477). What stayed
    /// behind is said by the chat's start there, which keeps its profile again.
    fn ready_to_move(
        &self,
        root: &Path,
        pool: &str,
        profile: &str,
    ) -> Result<(), onrunner::NotYet> {
        self.ready(root, pool, profile)?;
        Ok(())
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
        let _readying = Readying::of(pool, profile);
        let yours = yours(&self.config, pool);
        chosen_by_a_chat(&yours, &self.config, root, pool, profile);
        let behind = RefCell::default();
        tidy_once(&yours, &self.config, root, pool);
        // A chat's start is what wakes a suspended runner (#2051), so its keep does too.
        onrunner::ready_on(&self.config, root, pool, profile, &|kept| {
            keep_on_as(
                yours.clone(),
                &self.config,
                pool,
                kept,
                Waking::Yes,
                &behind,
            )
        })?;
        Ok(onrunner::left_behind_note(
            pool,
            profile,
            &behind.into_inner(),
        ))
    }

    fn ready_to_move(
        &self,
        root: &Path,
        pool: &str,
        profile: &str,
    ) -> Result<(), onrunner::NotYet> {
        let _readying = Readying::of(pool, profile);
        let yours = yours(&self.config, pool);
        chosen_by_a_chat(&yours, &self.config, root, pool, profile);
        tidy_once(&yours, &self.config, root, pool);
        onrunner::ready_to_move_on(&self.config, pool, profile, &|kept| {
            keep_on_as(
                yours.clone(),
                &self.config,
                pool,
                kept,
                Waking::Yes,
                &RefCell::default(),
            )
        })
    }
}

/// **A chat in the project at `root` that starts on or moves to pool `pool` on `profile` adds
/// it to the pool** (#4047), so its runner keeps it from now on: only one of this machine's
/// profiles, on a pool `yours` showed to be the person's own. One already on it is left as it is;
/// a pool that named none names it from now on, beside the project's default, so another
/// project's chats never have its runner forget it ([`tidy`], #4060). What could not be
/// recorded is logged, and the start goes on: the runner is asked to keep the profile all the
/// same.
fn chosen_by_a_chat(
    yours: &Result<Yours, String>,
    config: &Path,
    root: &Path,
    pool: &str,
    profile: &str,
) {
    if yours.is_err() {
        return;
    }
    let records = profilestore::list(config).unwrap_or_default();
    let ours = records.iter().any(|r| r.name == profile);
    let Ok(named) = purlis_core::runners::profiles_of(config, pool) else {
        return;
    };
    if !ours || named.is_some_and(|named| named.iter().any(|one| one == profile)) {
        return;
    }
    let default = default_in(Some(root), &records);
    if let Err(why) =
        purlis_core::runners::add_pool_profile(config, pool, profile, default.as_deref())
    {
        tracing::warn!("purlis: pool {pool} does not name the profile {profile}: {why}");
    }
}

/// Whether a tidy keeps a profile the runner names, asked just before it would be forgotten.
type Keep = Box<dyn Fn(&str) -> bool + Send>;

/// What one [`tidy`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Tidied {
    /// Nothing was asked: a pool that names none, with no default to judge by.
    NothingToJudge,
    /// The runner runs a purlis before 1.57, which cannot say what it keeps.
    Older,
    /// The names forgotten, and whether the runner refused any, under a chat running on it.
    Done {
        forgotten: Vec<String>,
        refused: bool,
    },
}

/// **Has pool `pool`'s runner keep only the profiles chosen for it** (#4060, #3849), through
/// `forget_all_but` ([`crate::runnerlinks::forget_all_but`]), which names what the runner keeps
/// for this machine and forgets each name the predicate it is given does not keep:
///
/// - **A pool that names its profiles** keeps those this machine still has among `records`.
///   One chosen there that this machine no longer has is forgotten, and leaves the record.
/// - **A pool that names none** (one made before #4047) forgets only what this machine no
///   longer has. What its runner keeps of this machine's beside `default`, the open project's,
///   is named on the pool from then on, so its row shows each one to remove, and no project
///   opened next has the runner forget another's default, sign-in and conversations with it.
///
/// Never forgotten by a tidy: this machine's adopted folder (the harness's own folder on the
/// runner, which a chat with no profile there uses too), which a pool that names its own is not
/// given back once the person took it off, and a profile a chat's start readies now
/// ([`Readying`]), which the start names on the pool. Nothing is asked of a pool that names none with no default (no project
/// open, or a default that is the runner's own).
///
/// **A cluster runner's word never widens what goes to it** (#4183): what is named on a pool is
/// sent to its runner, and a list of the runner's own is not the person's choice. So a cluster
/// pool that names none names the project's default from its first tidy on, and is held to it as
/// any pool that names its own is.
fn tidy(
    config: &Path,
    pool: &str,
    records: &[profilestore::MachineProfile],
    default: Option<&str>,
    forget_all_but: impl FnOnce(Keep) -> Result<Option<crate::runnerlinks::Listed>, String>,
) -> Result<Tidied, String> {
    let mut named = purlis_core::runners::profiles_of(config, pool)
        .map_err(|_| format!("there is no pool named {pool} on this machine"))?;
    // Unsure which kind it is, it is not treated as a Docker pool.
    let a_docker_pool = matches!(
        purlis_core::runners::cluster_runner_for(config, pool),
        Ok(None)
    );
    if let (None, false, Some(default)) = (&named, a_docker_pool, default) {
        let names = vec![default.to_owned()];
        purlis_core::runners::name_pool_profiles_where_none(config, pool, names.clone())
            .map_err(|why| why.to_string())?;
        named = Some(names);
    }
    if named.is_none() && default.is_none() {
        return Ok(Tidied::NothingToJudge);
    }
    let ours: Vec<String> = records.iter().map(|r| r.name.clone()).collect();
    let adopted: Vec<String> = records
        .iter()
        .filter(|r| r.adopted.is_some())
        .map(|r| r.name.clone())
        .collect();
    let keep: Keep = {
        let (named, ours, adopted, pool) = (
            named.clone(),
            ours.clone(),
            adopted.clone(),
            pool.to_owned(),
        );
        Box::new(move |name: &str| {
            let here = ours.iter().any(|one| one == name);
            let chosen = named
                .as_ref()
                .is_none_or(|named| named.iter().any(|one| one == name));
            (here && chosen) || adopted.iter().any(|one| one == name) || readying(&pool, name)
        })
    };
    let Some(listed) = forget_all_but(keep)? else {
        return Ok(Tidied::Older);
    };
    // What the runner keeps on is named on the pool, so its row shows it.
    match &named {
        None => {
            let mut names: Vec<String> = default.map(str::to_owned).into_iter().collect();
            for name in listed.kept.iter().filter(|n| ours.contains(n)) {
                if !names.contains(name) {
                    names.push(name.clone());
                }
            }
            if names.len() > 1 {
                purlis_core::runners::name_pool_profiles_where_none(config, pool, names)
                    .map_err(|why| why.to_string())?;
            }
        }
        Some(named) => {
            for name in listed
                .kept
                .iter()
                .filter(|n| ours.contains(n) && !named.contains(n) && !adopted.contains(n))
            {
                purlis_core::runners::add_pool_profile(config, pool, name, default)
                    .map_err(|why| why.to_string())?;
            }
        }
    }
    let mut forgotten = Vec::new();
    let mut refused = Vec::new();
    for (name, said) in listed.forgot {
        match said {
            Ok(()) => {
                let_go_everywhere(pool, &name);
                forgotten.push(name);
            }
            Err(why) => {
                tracing::warn!(
                    "purlis: pool {pool}'s runner did not forget the profile {}: {why}",
                    purlis_core::shown::short(&name)
                );
                refused.push(name);
            }
        }
    }
    // A chosen name this machine no longer has leaves the record, unless the runner still
    // keeps it.
    for name in named.iter().flatten().filter(|n| !ours.contains(n)) {
        if !refused.contains(name) && !listed.kept.contains(name) {
            purlis_core::runners::remove_pool_profile(config, pool, name, default)
                .map_err(|why| why.to_string())?;
        }
    }
    Ok(Tidied::Done {
        forgotten,
        refused: !refused.is_empty(),
    })
}

/// The app, once its setup has run: what [`let_go_everywhere`] tells the windows through.
static APP: std::sync::OnceLock<tauri::AppHandle> = std::sync::OnceLock::new();

/// **Every project's wait for profile `name` on pool `pool` goes, and each is told** (#4228), so
/// no other open project keeps drawing its Ask.
fn let_go_everywhere(pool: &str, name: &str) {
    let_go_telling(pool, name, |went| {
        if let Some(app) = APP.get().cloned() {
            // On a thread of its own: a tidy runs inside a chat's start, and telling takes the
            // locks of every open project.
            std::thread::spawn(move || crate::machineprofiles::tell_asks_at(&app, &went));
        }
    });
}

/// [`onrunner::let_go`] for every project, `tell` handed the projects whose wait went, and not
/// called where none did.
fn let_go_telling(pool: &str, name: &str, tell: impl FnOnce(Vec<PathBuf>)) {
    let went = onrunner::let_go(None, pool, name);
    if !went.is_empty() {
        tell(went);
    }
}

/// **A profile a chat's start readies on a pool now** (#4060): no tidy forgets it under the
/// start, between its runner keeping it and the chat starting there. Let go as it drops.
struct Readying {
    pool: String,
    profile: String,
}

static READYING: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());

impl Readying {
    fn of(pool: &str, profile: &str) -> Readying {
        READYING
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((pool.to_owned(), profile.to_owned()));
        Readying {
            pool: pool.to_owned(),
            profile: profile.to_owned(),
        }
    }
}

impl Drop for Readying {
    fn drop(&mut self) {
        let mut readying = READYING.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(at) = readying
            .iter()
            .position(|(pool, profile)| *pool == self.pool && *profile == self.profile)
        {
            readying.remove(at);
        }
    }
}

/// Whether a chat's start readies profile `name` on pool `pool` now.
fn readying(pool: &str, name: &str) -> bool {
    READYING
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .iter()
        .any(|(p, n)| p == pool && n == name)
}

/// The pools whose runner was asked this run to keep only what is chosen ([`tidy`]): a chat's
/// start asks only the first time, and asks again where it could not be told, nothing could be
/// judged, or the runner refused a forget.
static TIDIED: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// [`tidy`], for pool `pool` as a chat in the project at `root` starts on it, the first time
/// this run: only a pool `yours` showed to be the person's own, of either kind (#4183), its
/// runner woken as the start wakes it. What could not be done is logged, and the start goes on.
fn tidy_once(yours: &Result<Yours, String>, config: &Path, root: &Path, pool: &str) {
    let Ok(yours) = yours else {
        return;
    };
    {
        let mut tidied = TIDIED.lock().unwrap_or_else(PoisonError::into_inner);
        if tidied.iter().any(|one| one == pool) {
            return;
        }
        tidied.push(pool.to_owned());
    }
    let tidied = profilestore::list(config).and_then(|records| {
        let default = default_in(Some(root), &records);
        let to = to_a_runner(config, pool, Waking::Yes, yours)?;
        tidy(config, pool, &records, default.as_deref(), |keep| {
            crate::runnerlinks::forget_all_but(&to, keep)
        })
    });
    match tidied {
        Ok(Tidied::Older | Tidied::Done { refused: false, .. }) => {}
        Ok(Tidied::NothingToJudge | Tidied::Done { refused: true, .. }) => untidied(pool),
        Err(why) => {
            untidied(pool);
            tracing::warn!(
                "purlis: pool {pool}'s runner was not told what it no longer keeps: {why}"
            );
        }
    }
}

/// Pool `pool`'s runner is asked again, at the next chat's start there, what it keeps.
fn untidied(pool: &str) {
    TIDIED
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .retain(|one| one != pool);
}

/// **Has each pool's runner forget profile `name`, just removed on this machine** (#3849), its
/// folder and sign-in there with it, and takes it off each pool that named it: only a pool
/// [`yours`] shows to be the person's own at that moment, of either kind (#4183), never woken for
/// it, each asked at once. A runner that is away or not shown to be yours now, or that could not
/// forget it, is asked again at the next chat's start there or as its pool's profiles are shown
/// ([`tidy`]), which a runner of 1.57 or later answers; until then a pool that named it still
/// does, so its row offers Remove. `Some` note, in words, naming the pools that kept it whose
/// runner runs a purlis before 1.52, which keeps it.
pub fn forget_off_the_pools(name: &str) -> Option<String> {
    asking_the_runners_to_forget(name).unwrap_or_else(|why| {
        tracing::warn!("purlis: no pool's runner was asked to forget a profile: {why}");
        None
    })
}

/// [`forget_off_the_pools`], or why no runner could be asked at all: what the command line is
/// told, so it never says they were.
fn asking_the_runners_to_forget(name: &str) -> Result<Option<String>, String> {
    let config = config_root()?;
    let config = config.as_path();
    let pools = purlis_core::runners::pool_names(config).map_err(|why| why.to_string())?;
    Ok(forget_off(config, &pools, name, &|pool| {
        forget_on(config, pool, name)
    }))
}

/// Has pool `pool`'s runner forget profile `name`, once it is shown to be the person's own now:
/// nothing is asked of one that is not.
fn forget_on(config: &Path, pool: &str, name: &str) -> Result<Option<()>, String> {
    let yours = yours(config, pool)?;
    let to = to_a_runner(config, pool, Waking::No, &yours)?;
    crate::runnerlinks::forget_profile(&to, name)
}

/// [`forget_off_the_pools`], over `pools`, with `forget` what asks one pool's runner.
fn forget_off(
    config: &Path,
    pools: &[String],
    name: &str,
    forget: &(dyn Fn(&str) -> Result<Option<()>, String> + Sync),
) -> Option<String> {
    // First: its sign-in is carried off no draining runner (#4145), whatever else answers.
    for pool in pools {
        if let Err(why) = purlis_core::runners::hold_from_carrying(config, pool, name) {
            tracing::warn!("purlis: a drain of pool {pool} may carry {name}: {why}");
        }
    }
    let said: Vec<_> = std::thread::scope(|scope| {
        let asked: Vec<_> = pools
            .iter()
            .map(|pool| scope.spawn(move || forget(pool)))
            .collect();
        asked
            .into_iter()
            .map(|one| {
                one.join()
                    .unwrap_or_else(|_| Err("the runner was not asked".to_owned()))
            })
            .collect()
    });
    let mut older = Vec::new();
    for (pool_name, forgot) in pools.iter().zip(said) {
        let pool_name = pool_name.as_str();
        let named = purlis_core::runners::profiles_of(config, pool_name)
            .ok()
            .flatten()
            .map(|named| named.iter().any(|one| one == name));
        match forgot {
            Ok(None) => {
                // Kept where the pool names it, or names none and so kept every profile.
                if named.is_none_or(|named| named) {
                    older.push(pool_name.to_owned());
                }
                continue;
            }
            Ok(Some(())) => {
                let_go_everywhere(pool_name, name);
            }
            Err(why) => {
                // Still named, so its row offers Remove until a tidy finds it gone there.
                untidied(pool_name);
                tracing::info!(
                    "purlis: pool {pool_name}'s runner is asked again to forget the profile \
                     {name}: {why}"
                );
                continue;
            }
        }
        if named == Some(true)
            && let Err(why) =
                purlis_core::runners::remove_pool_profile(config, pool_name, name, None)
        {
            tracing::warn!("purlis: pool {pool_name} still names {name}: {why}");
        }
    }
    older_note(&older, &purlis_core::shown::short(name))
}

/// Has [`let_go_everywhere`] tell the windows of `app` from here on (#4228).
pub fn tells_through(app: tauri::AppHandle) {
    let _ = APP.set(app);
}

/// **Listens for `purlis profile remove` in the person's own terminal** (#4141), off the launch's
/// path, and has the runners told as Remove in the window has them told. Said and left at that
/// where it cannot listen: the command says the runners forget it at their next tidy.
#[cfg(unix)]
pub fn listen_for_the_command_line() {
    std::thread::spawn(|| {
        let Ok(config) = config_root() else {
            return;
        };
        let socket = purlis_core::windowline::socket();
        let listening = purlis_core::windowline::serve_on(&socket, move |asked| {
            purlis_core::windowline::answer(&config, asked, asking_the_runners_to_forget)
        });
        if let Err(why) = listening {
            tracing::warn!("purlis: `purlis profile remove` will not reach this window: {why}");
        }
    });
}

/// What the person is told of the pools in `older`, whose runner keeps the removed profile
/// `name` as it runs a purlis before 1.52; none where there is none.
fn older_note(older: &[String], name: &str) -> Option<String> {
    match older {
        [] => None,
        [one] => Some(format!(
            "Pool {one} keeps the profile '{name}' and its sign-in on its runner, which runs a \
             purlis before 1.52, until the runner is made again."
        )),
        [rest @ .., last] => Some(format!(
            "Pools {} and {last} keep the profile '{name}' and its sign-in on their runners, \
             which run a purlis before 1.52, until each runner is made again.",
            rest.join(", ")
        )),
    }
}

/// **Whether this machine's Docker pools' runners are shown to be the person's own** (#2049, by
/// [`purlis_core::poollogins::owned`], D-2050-4): the engine's local socket, or why not.
pub fn owned() -> Result<PathBuf, String> {
    let engine = purlis_core::dockerpool::Docker::engine_on_this_machine();
    purlis_core::poollogins::owned(engine.clone())?;
    engine
}

/// **How pool `pool` is shown to be the person's own** ([`yours`]), so its runner may keep a
/// profile of this machine's and sign it in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Yours {
    /// A Docker pool, on this machine's engine at this local socket ([`owned`]).
    Docker(PathBuf),
    /// A cluster pool the person marked whose claim its operator vouched for just now (#3819):
    /// the runner the gate judged, which is the only one a link or a sign-in then reaches
    /// ([`still_yours`]).
    Cluster(Box<purlis_core::runners::ClusterKept>),
}

/// **Whether pool `pool`'s runner is shown to be the person's own now** (#2049, #3819), or why
/// not, in words. A Docker pool by [`owned`]. A cluster pool by the gate a vault value or a token
/// passes ([`purlis_core::poollogins::Bound::of_a_cluster_pool`], #4071): the person's mark, no
/// chat run there without the sandbox, and its operator vouching for its claim now, read again
/// after. Its cluster is asked only for a marked pool.
pub fn yours(config: &Path, pool: &str) -> Result<Yours, String> {
    let Some((cluster, kept)) =
        purlis_core::runners::cluster_runner_for(config, pool).map_err(|why| why.to_string())?
    else {
        return owned().map(Yours::Docker);
    };
    yours_on_a_cluster(
        config,
        pool,
        &cluster,
        &kept,
        &purlis_core::pools::vouched_now,
    )
}

/// [`yours`] of the cluster pool `pool`, kept as `cluster` with its runner `kept`, with `vouch`
/// what asks its cluster.
fn yours_on_a_cluster(
    config: &Path,
    pool: &str,
    cluster: &purlis_core::runners::ClusterPool,
    kept: &purlis_core::runners::ClusterKept,
    vouch: &dyn Fn(&Path, &str) -> Result<String, purlis_core::clusterpool::Refused>,
) -> Result<Yours, String> {
    purlis_core::pools::cluster_runner_yours(config, pool, cluster, kept, vouch)
        .map(|judged| Yours::Cluster(Box::new(judged)))
}

/// [`purlis_core::pools::cluster_runner_still_yours`]: the cluster pool `pool` and its runner,
/// still as the gate judged them, read again right before anything reaches it.
fn still_yours(
    config: &Path,
    pool: &str,
    judged: &purlis_core::runners::ClusterKept,
) -> Result<
    (
        purlis_core::runners::ClusterPool,
        purlis_core::runners::ClusterKept,
    ),
    String,
> {
    purlis_core::pools::cluster_runner_still_yours(config, pool, judged)
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
    keep_on_as(
        yours(config, pool),
        config,
        pool,
        kept,
        Waking::No,
        &RefCell::default(),
    )
}

/// Whether a link made to keep a profile starts a suspended runner again (#2051): only a chat's
/// start does; a look, a poll and a sign-in leave it as it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Waking {
    Yes,
    No,
}

/// The runner of pool `pool` this machine keeps under `config`, as `yours` showed it to be the
/// person's own, linked to waking it or not: a cluster pool's, still as the gate judged it,
/// through this machine's `kubectl`, which never wakes anything (#3819).
fn to_a_runner(
    config: &Path,
    pool: &str,
    waking: Waking,
    yours: &Yours,
) -> Result<ToARunner, String> {
    if let Yours::Cluster(judged) = yours {
        let (cluster, kept) = still_yours(config, pool, judged)?;
        let kubectl = crate::clusterpools::kubectl(config)?;
        return ToARunner::of_cluster(config, &kubectl, &cluster, &kept);
    }
    let machine = Machine::here(config)?;
    let kept = purlis_core::runners::runner_for(config, pool).map_err(|why| why.to_string())?;
    match waking {
        Waking::Yes => ToARunner::of_kept_waking(&machine, &kept),
        Waking::No => ToARunner::of_kept(&machine, &kept),
    }
}

/// **The Docker runner of pool `pool`, still the one a sign-in started on**: a runner made again
/// since holds no login of this sign-in, so nothing goes to it.
fn still_the_runner(
    config: &Path,
    pool: &str,
    started: &purlis_core::runners::Kept,
) -> Result<purlis_core::runners::Kept, String> {
    let now = purlis_core::runners::runner_for(config, pool).map_err(|why| why.to_string())?;
    if the_same_runner(&now, started) {
        Ok(now)
    } else {
        Err(format!(
            "pool {pool}'s runner was made again since this sign-in started, so nothing went to \
             it. Try again"
        ))
    }
}

/// Whether `a` and `b` are one runner: the same container, answering as the same device.
fn the_same_runner(a: &purlis_core::runners::Kept, b: &purlis_core::runners::Kept) -> bool {
    a.made == b.made && a.pinned == b.pinned
}

/// [`keep_on`], with `yours` what [`yours`] said of pool `pool`: nothing is read or sent where
/// it said no. `behind` is told what of its settings stayed behind (#4081).
fn keep_on_as(
    yours: Result<Yours, String>,
    config: &Path,
    pool: &str,
    kept: &onrunner::Kept,
    waking: Waking,
    behind: &RefCell<Vec<onrunner::LeftBehind>>,
) -> Result<Option<Standing>, String> {
    let Ok(yours) = yours else {
        return Ok(None);
    };
    // Its settings, where it carries them, are read only once the pool is shown to be owned
    // and its runner is one that keeps them (1.50, #3448), and sent to no other.
    let to = to_a_runner(config, pool, waking, &yours)?;
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
    /// `cannot-tell`, or `fetching` while the runner fetches the version it pins (#4008), which
    /// the window asks again after. Null where it is not kept there.
    pub standing: Option<String>,
    /// Why it is not kept on the runner, in words, where it is not.
    pub not_kept: Option<String>,
    /// The plans Sign in offers there, where purlis names the provider.
    pub plans: Vec<crate::machineprofiles::PlanRow>,
}

/// Why a runner of a purlis before 1.43 keeps none of this machine's profiles.
const OLDER_RUNNER: &str = "its runner runs a purlis before 1.43, so its chats start on the \
                            profiles it has of its own";

/// **A pool's profiles** (#4047): the ones its runner keeps, as it stands with each, and this
/// machine's others that Add a profile… offers.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PoolProfiles {
    pub kept: Vec<PoolProfileRow>,
    pub addable: Vec<AddableProfile>,
    /// A cluster runner on a fresh claim (#3819, spec #4023 Q6): a sign-in there lasts only for
    /// this claim, since nothing of its home outlives its pod.
    pub fresh_claim: bool,
}

/// One of this machine's profiles that is not on a pool yet, and could go there.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct AddableProfile {
    pub name: String,
    /// The harness as people call it: `Claude Code`.
    pub harness: String,
}

/// **The profiles chosen for a pool, `kept`, among this machine's `records`** (#4047), in the
/// order chosen: each one's record, or why this machine has none of that name, so it can still
/// be removed. And this machine's others that a runner keeps, which Add a profile… offers.
fn chosen<'a>(
    records: &'a [profilestore::MachineProfile],
    kept: &[String],
) -> (
    Vec<Result<&'a profilestore::MachineProfile, String>>,
    Vec<&'a profilestore::MachineProfile>,
) {
    let on_pool = kept
        .iter()
        .map(|name| {
            records.iter().find(|r| &r.name == name).ok_or_else(|| {
                format!(
                    "this machine has no profile named '{}' any more",
                    purlis_core::shown::short(name)
                )
            })
        })
        .collect();
    let addable = records
        .iter()
        .filter(|r| !kept.contains(&r.name) && onrunner::Kept::of(r).is_ok())
        .collect();
    (on_pool, addable)
}

/// The profile a new chat in the project at `root` gets, among this machine's `records`: what a
/// pool that names no profiles keeps (#4047). None with no project open, and none where it is
/// a built-in this machine keeps no record of, which a runner has of its own.
fn default_in(root: Option<&Path>, records: &[profilestore::MachineProfile]) -> Option<String> {
    of_this_machine(
        root.and_then(|root| purlis_core::profiles::for_launch(root).0.for_a_new_chat()),
        records,
    )
}

/// `default`, where it is one of this machine's `records`.
fn of_this_machine(
    default: Option<String>,
    records: &[profilestore::MachineProfile],
) -> Option<String> {
    default.filter(|default| records.iter().any(|r| &r.name == default))
}

/// Refused, in words, where pool `pool` names no profiles and no project is open: what it keeps
/// is the open project's default, so a change has nothing to start from (#4047).
fn changeable(config: &Path, pool: &str, root: Option<&Path>) -> Result<(), String> {
    let names_none = purlis_core::runners::profiles_of(config, pool)
        .map_err(|why| why.to_string())?
        .is_none();
    if names_none && root.is_none() {
        return Err(format!(
            "pool {pool} keeps the profile a new chat in the open project gets, and no project \
             is open; open one to change its profiles"
        ));
    }
    Ok(())
}

/// The profiles pool `pool`, kept under `config`, keeps on its runner, with `default` the
/// project's (#4047), a Docker pool or a cluster pool (#3819). Refused for a pool this machine
/// keeps none of.
fn kept_on(config: &Path, pool: &str, default: Option<&str>) -> Result<Vec<String>, String> {
    purlis_core::runners::profiles_kept_of(config, pool, default)
        .map_err(|_| format!("there is no pool named {pool} on this machine"))
}

/// **The profiles chosen for pool `pool`, kept on its runner** (#2049, #4047), as the runner
/// stands with each, and this machine's others Add a profile… offers: the ones its `profiles`
/// names, or, where it names none, the profile a new chat in the open project `plane` gets. What
/// adding a pool ends with, and what Settings › Pools shows. Refused, with nothing sent, for a
/// pool not shown to be the person's own.
#[tauri::command]
#[specta::specta]
pub async fn pool_profiles(
    planes: tauri::State<'_, crate::planes::Planes>,
    pool: String,
    open: OpenProject,
) -> Result<PoolProfiles, String> {
    let root = root_of(&planes, open.plane.as_ref())?;
    tauri::async_runtime::spawn_blocking(move || {
        let config = config_root()?;
        let yours = yours(&config, &pool)?;
        let records = profilestore::list(&config)?;
        let default = default_in(root.as_deref(), &records);
        kept_on(&config, &pool, default.as_deref())?;
        let to = to_a_runner(&config, &pool, Waking::No, &yours)?;
        // What is no longer chosen is forgotten first, so the row shows what the runner keeps.
        if let Err(why) = tidy(&config, &pool, &records, default.as_deref(), |keep| {
            crate::runnerlinks::forget_all_but(&to, keep)
        }) {
            tracing::warn!(
                "purlis: pool {pool}'s runner was not told what it no longer keeps: {why}"
            );
        }
        let names = kept_on(&config, &pool, default.as_deref())?;
        // Where the cluster cannot say, no note is shown: it is said only of a fresh claim.
        let fresh_claim = matches!(yours, Yours::Cluster(_))
            && purlis_core::pools::claim_keeps_its_home(&config, &pool) == Ok(false);
        let (on_pool, addable) = chosen(&records, &names);
        // Each that goes to a runner, kept there over one link.
        let kept: Vec<_> = on_pool
            .iter()
            .map(|record| record.clone().and_then(onrunner::Kept::of))
            .collect();
        let sent: Vec<_> = kept
            .iter()
            .filter_map(|kept| kept.as_ref().ok().cloned())
            .collect();
        let said = crate::runnerlinks::keep_profiles(&to, sent)?;
        let mut said = said.map(Vec::into_iter);
        let kept = on_pool
            .iter()
            .zip(&names)
            .zip(kept)
            .map(|((record, name), kept)| {
                let standing = kept.and_then(|_| match said.as_mut() {
                    None => Err(OLDER_RUNNER.to_owned()),
                    Some(said) => said
                        .next()
                        .unwrap_or_else(|| Err("the runner did not answer".to_owned()))
                        .map(|kept| onrunner::standing_on_the_wire(kept.standing)),
                });
                match record {
                    Ok(record) => row(record, standing),
                    Err(why) => PoolProfileRow {
                        name: name.clone(),
                        kind: String::new(),
                        harness: String::new(),
                        standing: None,
                        not_kept: Some(why.clone()),
                        plans: Vec::new(),
                    },
                }
            })
            .collect();
        Ok(PoolProfiles {
            kept,
            fresh_claim,
            addable: addable
                .into_iter()
                .map(|record| AddableProfile {
                    name: record.name.clone(),
                    harness: harness_title(&record.harness),
                })
                .collect(),
        })
    })
    .await
    .map_err(|why| format!("the pool's profiles did not answer: {why}"))?
}

/// **Adds this machine's profile `name` to pool `pool`** (#4047, Add a profile…): its record
/// names it from now on, beside the open project `plane`'s default where it named none. Its
/// runner keeps it the next time the pool's profiles are asked.
#[tauri::command]
#[specta::specta]
pub async fn pool_profile_add(
    planes: tauri::State<'_, crate::planes::Planes>,
    pool: String,
    name: String,
    open: OpenProject,
) -> Result<(), String> {
    let root = root_of(&planes, open.plane.as_ref())?;
    tauri::async_runtime::spawn_blocking(move || {
        let config = config_root()?;
        yours(&config, &pool)?;
        changeable(&config, &pool, root.as_deref())?;
        let records = profilestore::list(&config)?;
        let record = records.iter().find(|p| p.name == name).ok_or_else(|| {
            format!(
                "this machine has no profile named '{}', so none was added to pool {pool}",
                purlis_core::shown::short(&name)
            )
        })?;
        onrunner::Kept::of(record)?;
        let default = default_in(root.as_deref(), &records);
        purlis_core::runners::add_pool_profile(&config, &pool, &name, default.as_deref())
            .map_err(|why| why.to_string())
    })
    .await
    .map_err(|why| format!("the profile was not added: {why}"))?
}

/// **Takes profile `name` off pool `pool`** (#4047): its runner forgets it first, its folder
/// and its sign-in there with it, then the pool's record leaves it out, starting from the
/// open project `plane`'s default where it named none. A runner that could not forget it leaves
/// the record as it was, so the removal can be asked again. The runner is woken for it, as a
/// chat's start wakes it, since what it keeps must go. `Some` note for a runner of a purlis
/// before 1.52, which keeps the profile until it is made again.
#[tauri::command]
#[specta::specta]
pub async fn pool_profile_remove(
    planes: tauri::State<'_, crate::planes::Planes>,
    pool: String,
    name: String,
    open: OpenProject,
) -> Result<Option<String>, String> {
    let root = root_of(&planes, open.plane.as_ref())?;
    tauri::async_runtime::spawn_blocking(move || {
        let config = config_root()?;
        let yours = yours(&config, &pool)?;
        changeable(&config, &pool, root.as_deref())?;
        let shown = purlis_core::shown::short(&name);
        // First: no drain carries its sign-in back to the pool's runner (#4145).
        purlis_core::runners::hold_from_carrying(&config, &pool, &name)
            .map_err(|why| why.to_string())?;
        let to = to_a_runner(&config, &pool, Waking::Yes, &yours)?;
        let forgotten = crate::runnerlinks::forget_profile(&to, &name).map_err(|why| {
            format!(
                "pool {pool}'s runner did not forget the profile '{shown}', so it stays on the \
                 pool: {why}"
            )
        })?;
        let default = default_in(root.as_deref(), &profilestore::list(&config)?);
        purlis_core::runners::remove_pool_profile(&config, &pool, &name, default.as_deref())
            .map_err(|why| why.to_string())?;
        let_go_everywhere(&pool, &name);
        Ok(forgotten.is_none().then(|| {
            format!(
                "The profile '{shown}' is off pool {pool}, but its runner runs a purlis before \
                 1.52, so it keeps the profile and its sign-in until the runner is made again."
            )
        }))
    })
    .await
    .map_err(|why| format!("the profile was not removed: {why}"))?
}

/// The open project a command is asked for: a type of its own, so its plane stays a `PlaneId`.
#[derive(Debug, serde::Deserialize, specta::Type)]
pub struct OpenProject {
    pub plane: Option<crate::planes::PlaneId>,
}

/// The root of the open project `plane`, whose default a pool that names no profiles keeps;
/// none with no project open. Refused for a project this window does not hold.
fn root_of(
    planes: &crate::planes::Planes,
    plane: Option<&crate::planes::PlaneId>,
) -> Result<Option<PathBuf>, String> {
    plane
        .map(|plane| planes.held(plane).map(|held| held.root().to_path_buf()))
        .transpose()
}

/// The harness `kind` as people call it: `Claude Code`.
fn harness_title(kind: &str) -> String {
    purlis_core::harness_declaration::builtin(kind)
        .map_or_else(|| kind.to_owned(), |d| d.title.clone())
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
        harness: harness_title(&record.harness),
        standing,
        not_kept,
        plans: crate::machineprofiles::PlanRow::of_harness(&record.harness),
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
/// With `device`, by the harness's device-code flow (#4044).
pub fn sign_in_line(
    pool: &str,
    name: &str,
    provider: Option<&str>,
    device: bool,
) -> Result<String, String> {
    let config = config_root()?;
    let yours = yours(&config, pool)?;
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
    if keep_on_as(
        Ok(yours.clone()),
        &config,
        pool,
        &kept,
        Waking::No,
        &RefCell::default(),
    )?
    .is_none()
    {
        return Err(format!(
            "Pool {pool}: {OLDER_RUNNER}. No sign-in was typed."
        ));
    }
    let typed = if device {
        Typed::DeviceInATerminal
    } else {
        Typed::InATerminal
    };
    login_on(&yours, &config, pool, name, provider, typed)?.line()
}

/// **The line a shell here runs to sign `name` out on pool `pool`'s Docker runner** (#4124), for a
/// harness whose sign-out asks which sign-in goes: the runner's own `purlis profile sign-out`, in
/// the terminal the person answers in. Once the line is ready, no drain carries its sign-in
/// (#4145), and a draining runner keeps its own until it is removed. A cluster pool's is refused
/// in words.
pub fn sign_out_line(pool: &str, name: &str) -> Result<String, String> {
    let config = config_root()?;
    let engine = match yours(&config, pool)? {
        Yours::Docker(engine) => engine,
        Yours::Cluster(_) => {
            return Err(format!(
                "pool {pool} is a cluster pool, and purlis opens this sign-out only on a Docker \
                 pool's runner so far, so nothing was typed."
            ));
        }
    };
    let record = record_named(&config, name)?;
    onrunner::Kept::of(&record)?;
    let runner = this_desktops_runner(&config, pool)?;
    let line = crate::runnerlinks::docker()?.sign_out_line(&runner.made, &engine, name)?;
    purlis_core::runners::hold_from_carrying(&config, pool, name).map_err(|why| why.to_string())?;
    Ok(line)
}

/// **The runner's own sign-in of `name` on pool `pool`'s runner, by `typed`**: through this
/// machine's `docker`, checked as it was made, on a Docker pool; through its `kubectl`, into the
/// pod the pool was paired with and vouched for as every link to it is, on a cluster pool
/// (#3819).
fn login_on(
    yours: &Yours,
    config: &Path,
    pool: &str,
    name: &str,
    provider: Option<&str>,
    typed: Typed,
) -> Result<profilestore::sign_in::Login, String> {
    match yours {
        Yours::Docker(engine) => {
            // Only a runner of this desktop's purlis holds a login to what drives it (#4043).
            let runner = match typed {
                Typed::InATerminal | Typed::DeviceInATerminal => {
                    purlis_core::runners::runner_for(config, pool).map_err(|why| why.to_string())?
                }
                Typed::Driven | Typed::InOwnTerminal(_) | Typed::DeviceInOwnTerminal(_) => {
                    this_desktops_runner(config, pool)?
                }
            };
            crate::runnerlinks::docker()?.checked_sign_in(
                &runner.made,
                engine,
                name,
                provider,
                typed,
            )
        }
        Yours::Cluster(judged) => {
            still_yours(config, pool, judged)?;
            if !matches!(typed, Typed::InATerminal | Typed::DeviceInATerminal) {
                this_desktops_cluster_runner(config, pool, judged)?;
            }
            purlis_core::pools::cluster_sign_in(
                config,
                &crate::clusterpools::kubectl(config)?,
                pool,
                judged,
                name,
                provider,
                typed,
            )
        }
    }
}

/// **One step of a sign-in on a pool's runner** that purlis drives with no terminal (#4042), as
/// the window's sign-in strip shows it ([`driven::Event`]).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(tag = "at", rename_all = "kebab-case")]
pub enum PoolSignInStep {
    /// Open this page in the browser here: the harness vendor's own.
    Open { url: String },
    /// Show this code to enter on the vendor's own page `url`, before that page is opened.
    Device { url: String, code: String },
    /// Open this page in the browser here, where the person approves: this machine is listening
    /// for the browser's answer, which purlis carries to the runner, so nothing is pasted (#4044).
    Approve { url: String },
    /// This machine's `port` is taken, so the browser's answer could not reach the runner: no
    /// page was opened.
    PortTaken { port: u16 },
    /// Ask for the code the page shows.
    Code,
    /// What a login in the strip's terminal printed, escapes and all.
    Printed { text: String },
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
            driven::Event::DeviceCode { url, code } => Self::Device { url, code },
            driven::Event::Approve { url, .. } => Self::Approve { url },
            driven::Event::PortTaken { port } => Self::PortTaken { port },
            driven::Event::AskCode => Self::Code,
            driven::Event::Printed { text } => Self::Printed { text },
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
    /// On a cluster pool, the runner the gate judged (#3819): each code passed on checks it
    /// is still so.
    judged: Option<Box<purlis_core::runners::ClusterKept>>,
}

static SIGN_INS: Mutex<Vec<SignIn>> = Mutex::new(Vec::new());
static NEXT_SIGN_IN: AtomicU32 = AtomicU32::new(1);

/// How long one ask for a sign-in's next step waits before it answers there is none yet.
const STEP_WITHIN: Duration = Duration::from_secs(20);

/// The size of the terminal a sign-in's strip draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, specta::Type)]
pub struct PoolSignInTerminal {
    pub columns: u16,
    pub rows: u16,
}

/// **Starts signing `name` in on pool `pool`'s runner with no terminal** (#4042, spec #4023):
/// the runner's own `purlis profile sign-in`, kept there first and run there through this
/// machine's `docker` as [`sign_in_line`]'s is, but with pipes, its output read for the steps
/// [`pool_sign_in_next`] answers. Nothing of this machine's sign-in goes to the runner (ADR
/// 0095). A login that waits for the browser is answered through this machine's port, for as
/// long as it runs: the redirect, and only that, is carried to the runner over a link of its own
/// (`sign_in_redirect`, #4044). A sign-in of the same profile on the same pool already running is
/// cancelled. Answers the sign-in's number. With `terminal`, where purlis could not follow the
/// login or the person asked for a device code (#4106), it runs in a terminal the runner makes, for the person to drive in the strip (#4043):
/// what it prints is [`PoolSignInStep::Printed`], and what is typed goes back by
/// [`pool_sign_in_type`]. A login that waits for the browser runs there by its harness's
/// device-code flow, since nothing carries the browser's answer to that terminal (#4044).
#[tauri::command]
#[specta::specta]
pub async fn pool_sign_in_start(
    pool: String,
    name: String,
    provider: Option<String>,
    terminal: Option<PoolSignInTerminal>,
) -> Result<u32, String> {
    // Numbered as it is asked, so of two starts the one asked last is the one kept.
    let flow = NEXT_SIGN_IN.fetch_add(1, Ordering::SeqCst);
    tauri::async_runtime::spawn_blocking(move || {
        let config = config_root()?;
        let yours = yours(&config, &pool)?;
        let record = record_named(&config, &name)?;
        profilestore::sign_in::plan_named(&record.harness, provider.as_deref())?;
        let kept = onrunner::Kept::of(&record)?;
        if keep_on_as(
            Ok(yours.clone()),
            &config,
            &pool,
            &kept,
            Waking::No,
            &RefCell::default(),
        )?
        .is_none()
        {
            return Err(format!(
                "Pool {pool}: {OLDER_RUNNER}. Nothing was signed in."
            ));
        }
        let judged = match &yours {
            Yours::Cluster(kept) => Some(kept.clone()),
            Yours::Docker(_) => None,
        };
        let typed = match terminal {
            // A login approved in the browser restarts by its device code: nothing carries the
            // browser's answer to a terminal (#4044).
            Some(size) => {
                let size = purlis_core::engine::Size {
                    columns: size.columns,
                    rows: size.rows,
                };
                if driven::waits_for_the_browser(&record.harness, provider.as_deref()) {
                    Typed::DeviceInOwnTerminal(size)
                } else {
                    Typed::InOwnTerminal(size)
                }
            }
            None => Typed::Driven,
        };
        // A cluster pool's is held to the runner its gate judged; a Docker pool's to this one.
        let started = match &yours {
            Yours::Docker(_) => Some(
                purlis_core::runners::runner_for(&config, &pool).map_err(|why| why.to_string())?,
            ),
            Yours::Cluster(_) => None,
        };
        let login = login_on(&yours, &config, &pool, &name, provider.as_deref(), typed)?;
        let carry: callback::Carry = {
            let (config, pool, yours) = (config.clone(), pool.clone(), yours.clone());
            Arc::new(move |target: &str| {
                let to = match &started {
                    Some(started) => ToARunner::of_kept(
                        &Machine::here(&config)?,
                        &still_the_runner(&config, &pool, started)?,
                    )?,
                    None => to_a_runner(&config, &pool, Waking::No, &yours)?,
                };
                crate::runnerlinks::sign_in_redirect(&to, target)
            })
        };
        let check = {
            let pool = pool.clone();
            move || match keep_on(&config, &pool, &kept) {
                Ok(Some(standing)) => standing,
                _ => Standing::CannotTell,
            }
        };
        let driving = Arc::new(match terminal {
            Some(_) => driven::in_a_terminal(&login, check)?,
            None => driven::drive_with(
                &login,
                &record.harness,
                provider.as_deref(),
                Some(carry),
                check,
            )?,
        });
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
                    judged,
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
pub async fn pool_sign_in_next(
    app: tauri::AppHandle,
    flow: u32,
) -> Result<Option<PoolSignInStep>, String> {
    // Asked at each step, as a login at its device code may take nothing typed.
    let event = tauri::async_runtime::spawn_blocking(move || {
        still_driving(flow).map(|driving| driving.next(STEP_WITHIN))
    })
    .await
    .map_err(|why| format!("the sign-in did not answer: {why}"))??;
    if let Some(event) = event.as_ref().filter(|event| event.is_last()) {
        let over = {
            let mut held = SIGN_INS.lock().unwrap_or_else(PoisonError::into_inner);
            held.iter()
                .position(|one| one.flow == flow)
                .map(|at| held.remove(at))
        };
        if let Some(over) = over
            && *event == driven::Event::SignedIn
        {
            // Told with no lock held; every project whose wait went has an Ask to stop drawing.
            let went = onrunner::let_go(None, &over.pool, &over.name);
            crate::machineprofiles::tell_asks_at(&app, &went);
        }
    }
    Ok(event.map(PoolSignInStep::from))
}

/// **Passes the code the person pasted to sign-in `flow`**, as one line: on a cluster pool, only
/// while its runner is still as the gate judged it ([`still_driving`]); else the sign-in is
/// cancelled and the code goes nowhere.
#[tauri::command]
#[specta::specta]
pub async fn pool_sign_in_paste(flow: u32, code: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || still_driving(flow)?.paste(&code))
        .await
        .map_err(|why| format!("the code was not passed on: {why}"))?
}

/// **What drives sign-in `flow`, where what it was opened on still holds** (#3819): on a
/// cluster pool, the runner the gate judged, still marked and run without the sandbox by no
/// chat ([`still_yours`]); else the sign-in is cancelled and nothing goes to it. A sign-in no
/// longer held is refused.
fn still_driving(flow: u32) -> Result<Arc<driven::Driving>, String> {
    let (driving, judged) = SIGN_INS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .iter()
        .find(|one| one.flow == flow)
        .map(|one| {
            (
                Arc::clone(&one.driving),
                one.judged.clone().map(|judged| (one.pool.clone(), judged)),
            )
        })
        .ok_or_else(|| "that sign-in is over".to_owned())?;
    if let Some((pool, judged)) = judged {
        let held = config_root().and_then(|config| still_yours(&config, &pool, &judged));
        if held.is_err() {
            pool_sign_in_cancel(flow);
            // Not the start's words: by now something of it may have reached the runner.
            return Err(format!(
                "the sign-in was stopped, as pool {pool} or its runner changed. Try again."
            ));
        }
    }
    Ok(driving)
}

/// **Cancels every sign-in running on pool `pool`** (#3819): its mark was taken back.
pub fn cancel_sign_ins_on(pool: &str) {
    let gone: Vec<SignIn> = {
        let mut held = SIGN_INS.lock().unwrap_or_else(PoisonError::into_inner);
        let (gone, kept) = std::mem::take(&mut *held)
            .into_iter()
            .partition(|one| one.pool == pool);
        *held = kept;
        gone
    };
    for one in gone {
        one.driving.cancel();
    }
}

/// **Passes what the person typed in the strip's terminal to sign-in `flow`**, as it is: on a
/// cluster pool, only while its runner is still as the gate judged it ([`still_driving`]).
#[tauri::command]
#[specta::specta]
pub async fn pool_sign_in_type(flow: u32, text: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || still_driving(flow)?.typed(&text))
        .await
        .map_err(|why| format!("what was typed was not passed on: {why}"))?
}

/// **Signs `name` out on pool `pool`'s runner** (#4043): the harness's own sign-out, run there,
/// and on each runner of the pool still draining (#4121, #4145), whose chats then sign in again.
/// `Some` words naming the draining runners, where there are any.
#[tauri::command]
#[specta::specta]
pub async fn pool_sign_out(pool: String, name: String) -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let config = config_root()?;
        let yours = yours(&config, &pool)?;
        let record = record_named(&config, &name)?;
        let kept = onrunner::Kept::of(&record)?;
        let mut draining = Vec::new();
        match &yours {
            Yours::Docker(_) => {
                let docker = crate::runnerlinks::docker()?;
                // First, so that no drain carries it even if the rest stops short.
                draining = purlis_core::pools::sign_out_draining(&config, &docker, &pool, &name)
                    .map_err(|why| why.to_string())?;
                let runner = this_desktops_runner(&config, &pool)
                    .map_err(|why| with_draining_said(why, &name, &draining))?;
                docker
                    .sign_out(&runner.made, &name)
                    .map_err(|why| with_draining_said(why, &name, &draining))?;
                // A follow since the first look leaves a runner draining that was not reached.
                for late in purlis_core::pools::sign_out_draining(&config, &docker, &pool, &name)
                    .map_err(|why| with_draining_said(why.to_string(), &name, &draining))?
                {
                    if !draining.iter().any(|(runner, _)| *runner == late.0) {
                        draining.push(late);
                    }
                }
            }
            // Over this machine's kubectl, to the runner the gate judged, still so (#3819). A
            // cluster pool's one runner never drains.
            Yours::Cluster(judged) => {
                still_yours(&config, &pool, judged)?;
                this_desktops_cluster_runner(&config, &pool, judged)?;
                purlis_core::pools::cluster_sign_out(
                    &config,
                    &crate::clusterpools::kubectl(&config)?,
                    &pool,
                    judged,
                    &name,
                )?;
            }
        }
        // Its harness says whether it is signed out, not the sign-out's exit status alone.
        profilestore::sign_in::forget_of(&record);
        signed_out_by_its_harness(&name, &pool, keep_on(&config, &pool, &kept))
            .map_err(|why| with_draining_said(why, &name, &draining))?;
        Ok(draining_said(&name, &draining))
    })
    .await
    .map_err(|why| format!("the sign-out did not run: {why}"))?
}

/// `why`, followed by what the sign-out did on the draining runners.
fn with_draining_said(
    why: String,
    name: &str,
    draining: &[purlis_core::pools::DrainingSignOut],
) -> String {
    match draining_said(name, draining) {
        Some(said) => format!("{why} {said}"),
        None => why,
    }
}

/// **What the person is told of a sign-out on a pool's draining runners** (#4121): the ones it
/// reached, and the ones it did not, whose sign-in ends with them and is not carried. None where
/// the pool has no draining runner.
fn draining_said(name: &str, draining: &[purlis_core::pools::DrainingSignOut]) -> Option<String> {
    let name = purlis_core::shown::short(name);
    let reached: Vec<&str> = draining
        .iter()
        .filter(|(_, did)| did.is_ok())
        .map(|(runner, _)| runner.as_str())
        .collect();
    let missed: Vec<&str> = draining
        .iter()
        .filter(|(_, did)| did.is_err())
        .map(|(runner, _)| runner.as_str())
        .collect();
    for (runner, did) in draining {
        if let Err(why) = did {
            tracing::info!("purlis: draining runner {runner} did not sign {name} out: {why}");
        }
    }
    let mut said = Vec::new();
    if !reached.is_empty() {
        said.push(format!(
            "Also signed out on draining {}: its chats there sign in again.",
            runners_named(&reached)
        ));
    }
    if !missed.is_empty() {
        said.push(format!(
            "Not reached on draining {}: its sign-in stays there until the runner is removed, \
             and is not carried.",
            runners_named(&missed)
        ));
    }
    (!said.is_empty()).then(|| said.join(" "))
}

/// `runner a`, or `runners a, b and c`.
fn runners_named(names: &[&str]) -> String {
    match names {
        [one] => format!("runner {one}"),
        [rest @ .., last] => format!("runners {} and {last}", rest.join(", ")),
        [] => String::new(),
    }
}

/// **What a sign-out says, from what `name`'s harness on pool `pool`'s runner answered after it
/// ran**: done only where its status check says signed out. Any other answer leaves it unconfirmed.
fn signed_out_by_its_harness(
    name: &str,
    pool: &str,
    asked: Result<Option<Standing>, String>,
) -> Result<(), String> {
    match asked {
        Ok(Some(Standing::SignedOut)) => Ok(()),
        Ok(Some(Standing::SignedIn)) => Err(format!(
            "{name} is still signed in on pool {pool}: its harness's sign-out ran, and its \
             status check says it is signed in"
        )),
        Ok(Some(Standing::CannotTell | Standing::Fetching) | None) | Err(_) => Err(format!(
            "{name} may still be signed in on pool {pool}: its harness's sign-out ran, and purlis \
             could not confirm it. Look at the pool's row"
        )),
    }
}

/// **[`this_desktops_runner`], of a cluster pool** (#3819): its runner's `purlis`, asked over
/// `kubectl exec`, says this desktop's version, or it is refused in the same words.
fn this_desktops_cluster_runner(
    config: &Path,
    pool: &str,
    judged: &purlis_core::runners::ClusterKept,
) -> Result<(), String> {
    let ours = crate::runnerlinks::desktop_version();
    let why = match purlis_core::pools::cluster_runner_version(
        config,
        &crate::clusterpools::kubectl(config)?,
        pool,
        judged,
    ) {
        Ok(theirs) if theirs == ours => return Ok(()),
        Ok(theirs) => {
            format!("pool {pool}'s runner runs purlis {theirs}, and this desktop runs {ours}")
        }
        Err(why) => format!(
            "pool {pool}'s runner's version could not be read, so it may not run this desktop's \
             {ours}: {why}"
        ),
    };
    Err(format!(
        "{why}, so purlis signs nothing in or out there itself. Sign in with Open in terminal, \
         or once the runner runs this desktop's version."
    ))
}

/// **Pool `pool`'s runner, where it runs this desktop's purlis**: one that runs another holds no
/// sign-in to what drives it (#4043), and has no sign-out of purlis's own, so it is refused in
/// words, with Open in terminal the way to sign in there.
fn this_desktops_runner(config: &Path, pool: &str) -> Result<purlis_core::runners::Kept, String> {
    let runner = purlis_core::runners::runner_for(config, pool).map_err(|why| why.to_string())?;
    match crate::runnerlinks::behind(&Machine::here(config)?, &runner) {
        None => Ok(runner),
        Some(why) => Err(format!(
            "{why}, so purlis signs nothing in or out there itself. Sign in with Open in \
             terminal, or once the runner runs this desktop's version."
        )),
    }
}

/// **Cancels sign-in `flow`**: what drives its login is ended, and with its input the login
/// ends on the runner.
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

/// Whether a runner's answer says the profile is signed in: what a watcher asks. A runner that
/// cannot tell, or is still fetching the harness, says nothing of a sign-in the person may still
/// be finishing (#4009).
fn says_signed_in(said: &Result<Option<Standing>, String>) -> bool {
    matches!(said, Ok(Some(Standing::SignedIn)))
}

/// Whether a watched profile is still there to sign in on a pool: this machine has it, the pool
/// exists (`named`'s outer `None` where it does not), and the pool names it, or names none and so
/// keeps every profile. What cannot be read is not gone.
fn still_on_the_pool<E, F>(
    records: Result<Vec<profilestore::MachineProfile>, E>,
    named: Result<Option<Option<Vec<String>>>, F>,
    name: &str,
) -> bool {
    records.map_or(true, |records| records.iter().any(|p| p.name == name))
        && named.map_or(true, |pool| {
            pool.is_some_and(|named| named.is_none_or(|named| named.iter().any(|one| one == name)))
        })
}

/// **What a watcher asks and lets go for `name` on pool `pool`** ([`crate::signinwatch`]), or
/// `None` where this machine no longer has the profile, or it cannot go on a runner.
pub fn watched_sign_in(pool: String, name: String) -> Option<crate::signinwatch::SignIn> {
    let config = config_root().ok()?;
    let kept = profilestore::list(&config)
        .ok()
        .and_then(|records| records.into_iter().find(|p| p.name == name))
        .and_then(|record| onrunner::Kept::of(&record).ok())?;
    let asked = pool.clone();
    let (config_there, pool_there, name_there) = (config.clone(), pool.clone(), name.clone());
    Some(crate::signinwatch::SignIn {
        pool: Some(pool.clone()),
        name: name.clone(),
        there: Box::new(move || {
            still_on_the_pool(
                profilestore::list(&config_there),
                purlis_core::runners::pools(&config_there).map(|pools| {
                    pools
                        .iter()
                        .find(|one| one.name() == pool_there)
                        .map(|one| one.profiles().map(<[String]>::to_vec))
                }),
                &name_there,
            )
        }),
        signed_in: Box::new(move || says_signed_in(&keep_on(&config, &asked, &kept))),
        let_go: Box::new(move |whose| match whose {
            crate::signinwatch::Whose::Everywhere => onrunner::let_go(None, &pool, &name),
            crate::signinwatch::Whose::InTheProject(root, seen) => {
                onrunner::let_go_older(root, &pool, &name, seen);
                Vec::new()
            }
        }),
    })
}

#[cfg(test)]
mod tests;

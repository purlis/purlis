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

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use purlis_core::dockerpool::Typed;
use purlis_core::poollimits::LimitId;
use purlis_core::profilestore::sign_in::{Standing, callback, driven};
use purlis_core::profilestore::{self, onrunner};

use crate::runnerlinks::{Machine, ToARunner};

/// **Readies a chat's profile on the pool's runner it starts on** (#2049): what a start on a
/// pool asks before it is sent there.
pub trait ProfilesOnPools: Send + Sync {
    /// `Ok` where a chat in the project at `root` on `profile` may start on pool `pool`'s
    /// runner, or why not, in words.
    fn ready(&self, root: &Path, pool: &str, profile: &str) -> Result<(), String>;

    /// [`Self::ready`], for a chat that moves to pool `pool`'s runner (#2048): said of the move,
    /// and a profile signed out there is the sign-in the move waits on (#3477).
    fn ready_to_move(
        &self,
        root: &Path,
        pool: &str,
        profile: &str,
    ) -> Result<(), onrunner::NotYet> {
        Ok(self.ready(root, pool, profile)?)
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
    fn ready(&self, root: &Path, pool: &str, profile: &str) -> Result<(), String> {
        let _readying = Readying::of(pool, profile);
        let yours = yours(&self.config, pool);
        chosen_by_a_chat(&yours, &self.config, root, pool, profile);
        tidy_once(&yours, &self.config, root, pool);
        // A chat's start is what wakes a suspended runner (#2051), so its keep does too.
        onrunner::ready_on(&self.config, root, pool, profile, &|kept| {
            keep_on_as(yours.clone(), &self.config, pool, kept, Waking::Yes)
        })
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
            keep_on_as(yours.clone(), &self.config, pool, kept, Waking::Yes)
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

/// The runners ([`tidied_as`]) asked this run to keep only what is chosen ([`tidy`]): a chat's
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
    let runner = tidied_as(pool, yours);
    {
        let mut tidied = TIDIED.lock().unwrap_or_else(PoisonError::into_inner);
        if tidied.contains(&runner) {
            return;
        }
        tidied.push(runner.clone());
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
        Ok(Tidied::NothingToJudge | Tidied::Done { refused: true, .. }) => untidied(&runner),
        Err(why) => {
            untidied(&runner);
            tracing::warn!(
                "purlis: pool {pool}'s runner was not told what it no longer keeps: {why}"
            );
        }
    }
}

/// What [`TIDIED`] knows pool `pool`'s runner by: on a cluster pool, the claim it was made by, so
/// a claim made again is asked again.
fn tidied_as(pool: &str, yours: &Yours) -> String {
    match yours {
        Yours::Docker(_) => pool.to_owned(),
        Yours::Cluster(judged) => format!("{pool}/{}", judged.claimed.uid),
    }
}

/// Pool `pool`'s runner, whichever claim it was made by, is asked again at the next chat's start.
fn untidied_on(pool: &str) {
    let of_a_claim = format!("{pool}/");
    TIDIED
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .retain(|one| one != pool && !one.starts_with(&of_a_claim));
}

/// `runner` ([`tidied_as`]) is asked again, at the next chat's start there, what it keeps.
fn untidied(runner: &str) {
    TIDIED
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .retain(|one| one != runner);
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
                untidied_on(pool_name);
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
        let listening = purlis_core::windowline::serve_watching(&socket, move |asked, hung_up| {
            purlis_core::windowline::answer(
                &config,
                asked,
                asking_the_runners_to_forget,
                |name, pool| {
                    if !crate::poolsignin_confirm::take_the_yes(name, pool) {
                        return Err("it was not confirmed in this window".to_owned());
                    }
                    keep_for_the_command(
                        yours(&config, pool),
                        &config,
                        pool,
                        name,
                        &|yours, kept| keep_on_as(Ok(yours), &config, pool, kept, Waking::No),
                    )
                },
                |name, pool, code| match APP.get() {
                    Some(app) => {
                        crate::poolsignin_confirm::ask_the_person(app, name, pool, code, hung_up)
                    }
                    None => Err("this window is still starting".to_owned()),
                },
            )
        });
        if let Err(why) = listening {
            tracing::warn!("purlis: `purlis profile remove` will not reach this window: {why}");
        }
    });
}

/// **`purlis profile sign-in --pool` keeps `name` on pool `pool`'s runner through this window**
/// (#4112), as Sign in on its line does, behind the same gate, `yours` being what it said of the
/// pool: nothing is written or sent where it said no. The pool names it first, so no tidy has the
/// runner forget what the person just signed in, and leaves it out again where nothing was kept.
/// A pool that names none is refused, as Add a profile… is with no project open: naming it would
/// drop the open project's default, which this line does not know.
#[cfg(unix)]
fn keep_for_the_command(
    yours: Result<Yours, String>,
    config: &Path,
    pool: &str,
    name: &str,
    keep: &KeepOnARunner<'_>,
) -> Result<Option<String>, String> {
    let yours = yours?;
    let kept = onrunner::Kept::of(&record_named(config, name)?)?;
    let named = purlis_core::runners::profiles_of(config, pool)
        .map_err(|why| why.to_string())?
        .ok_or_else(|| {
            format!(
                "pool {pool} does not name its profiles, so this line cannot add one: use Add \
                 a profile… on its row in Settings › Pools"
            )
        })?;
    purlis_core::runners::add_pool_profile(config, pool, name, None)
        .map_err(|why| why.to_string())?;
    let said = match keep(yours, &kept) {
        Ok(Some(_)) => return Ok(None),
        Ok(None) => format!("Pool {pool}: {OLDER_RUNNER}"),
        Err(why) => why,
    };
    if !named.iter().any(|one| one == name)
        && let Err(why) = purlis_core::runners::remove_pool_profile(config, pool, name, None)
    {
        tracing::warn!("purlis: pool {pool} still names the profile {name}: {why}");
    }
    Err(said)
}

/// What has a pool's runner keep a profile, for [`keep_for_the_command`].
#[cfg(unix)]
type KeepOnARunner<'a> = dyn Fn(Yours, &onrunner::Kept) -> Result<Option<Standing>, String> + 'a;

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
    keep_on_as(yours(config, pool), config, pool, kept, Waking::No)
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
/// it said no.
fn keep_on_as(
    yours: Result<Yours, String>,
    config: &Path,
    pool: &str,
    kept: &onrunner::Kept,
    waking: Waking,
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
        .map(|kept| Some(onrunner::standing_on_the_wire(kept.standing)))
        .map_err(|why| late_said(pool, &kept.limits, why))
}

/// `why` a runner kept no profile: where the pool's wait ran out, in words the window can
/// ask of for what to raise it to (spec #4275). By purlis's own constant, never a runner's words.
fn late_said(pool: &str, limits: &purlis_core::poollimits::Chosen, why: String) -> String {
    if why == crate::runnerlinks::ANSWER_LATE {
        let secs = limits.in_force(LimitId::AnswerWait);
        let hit = limits.waited(LimitId::AnswerWait, secs, None);
        format!("{why}: {}", purlis_core::poollimits::hit_on(pool, hit))
    } else {
        why
    }
}

/// **A limit a start was refused on**, as Start a chat offers to raise it (spec #4275).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PoolLimitHitRow {
    /// The limit's key: what [`pool_limit_set`] takes.
    pub key: String,
    /// Its name in plain words.
    pub plain: String,
    /// The word beside a number of it.
    pub unit: String,
    pub in_force: u32,
    pub needed: u32,
    /// The value offered, which the person can edit; none where no value would hold it.
    pub suggested: Option<u32>,
}

/// **What a refused start on a pool offers**: the pool, and each limit it was refused on.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PoolLimitOffer {
    pub pool: String,
    pub hits: Vec<PoolLimitHitRow>,
}

/// The offer of the refusal `over`, where it reached a pool.
fn offer_of(over: onrunner::OverLimit) -> Option<PoolLimitOffer> {
    let number = |value: u64| u32::try_from(value).unwrap_or(u32::MAX);
    Some(PoolLimitOffer {
        pool: over.pool?,
        hits: over
            .hits
            .iter()
            .map(|hit| {
                let limit = hit.limit.limit();
                PoolLimitHitRow {
                    key: limit.key.to_owned(),
                    plain: limit.plain.to_owned(),
                    unit: limit.unit.word().to_owned(),
                    in_force: number(hit.in_force),
                    needed: number(hit.needed),
                    suggested: hit.suggested.map(number),
                }
            })
            .collect(),
    })
}

/// **The limits a start on `profile` was refused on, in the words `refused`** (spec #4275): what
/// Start a chat offers beside that refusal. Asked of the core, never read from the words.
#[tauri::command]
#[specta::specta]
pub async fn pool_limit_offer(
    profile: String,
    refused: String,
    root: Option<String>,
    cwd: Option<String>,
) -> Option<PoolLimitOffer> {
    onrunner::over_limit(&profile, &refused)
        .and_then(offer_of)
        .or_else(|| {
            // The pool this start runs on, by this machine's store: nothing a runner said.
            let root = PathBuf::from(root?);
            let workspace =
                crate::chats::workspace_at_start(Some(&root), cwd.as_deref().map(Path::new));
            let pool =
                purlis_core::pools::runs_on(&config_root().ok()?, &root, workspace.as_deref())
                    .ok()??;
            offer_of_hits(pool, &refused)
        })
}

/// The offer of the limits of `pool` that purlis measured and said in the words `refused`.
fn offer_of_hits(pool: String, refused: &str) -> Option<PoolLimitOffer> {
    let hits = purlis_core::poollimits::hits_said_in(&pool, refused);
    if hits.is_empty() {
        return None;
    }
    offer_of(onrunner::OverLimit {
        profile: String::new(),
        pool: Some(pool),
        hits,
        said: refused.to_owned(),
    })
}

/// **The limits that refused a move to pool `pool` in the words `refused`** (#4313): what the
/// move's line offers beside that refusal, as [`pool_limit_offer`] does for a start.
#[tauri::command]
#[specta::specta]
pub async fn pool_limit_offer_for_move(pool: String, refused: String) -> Option<PoolLimitOffer> {
    onrunner::over_limit_on(&pool, &refused)
        .and_then(offer_of)
        .or_else(|| offer_of_hits(pool, &refused))
}

/// **One of a pool's limits, as its page draws it** (#4285).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PoolLimitRow {
    /// Its key: what [`pool_limit_set`] and [`pool_limit_check`] take.
    pub key: String,
    /// Its name in plain words.
    pub plain: String,
    /// The word beside a number of it.
    pub unit: String,
    /// purlis's default.
    pub default: u32,
    /// The default in words: `128 MiB`.
    pub default_said: String,
    /// What the person set; `null` where it follows purlis's default.
    pub set: Option<u32>,
    /// When a change applies, where its group doesn't say it for all of its limits.
    pub applies: Option<String>,
    /// Why the value in the pool's file can't be used, where one can't (#4381), or is above what
    /// the team defining the pool allows (#4287).
    pub refused: Option<String>,
    /// The highest the team defining the pool allows, in words, where it set one (#4287).
    pub highest_said: Option<String>,
    /// Whether its default is what the team defining the pool set (#4287).
    pub default_by_team: bool,
    /// The value the team suggests, above what holds: one press sets it, as the person's (#4287).
    pub suggested: Option<u32>,
    /// That value in words.
    pub suggested_said: Option<String>,
}

/// **A group of a pool's limits**: what they affect, and the limits that aren't under Advanced.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PoolLimitGroup {
    pub label: String,
    pub help: String,
    /// When a change applies, where that's the same for all of its limits.
    pub applies: Option<String>,
    pub limits: Vec<PoolLimitRow>,
}

/// **A pool's limits, as its page draws them** (#4285): whatever the core's list holds, by group,
/// and those under Advanced. A group with no limit isn't listed.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PoolLimitsPage {
    pub groups: Vec<PoolLimitGroup>,
    pub advanced: Vec<PoolLimitRow>,
    /// The limits changed since its runner was made, in plain words (#4286): what waits for it
    /// to be replaced. Empty where nothing waits, the only time Replace is offered.
    pub waiting: Vec<String>,
}

/// What the page draws of the values `chosen`: a limit shaping a runner only on a pool whose runner
/// this machine makes (`docker`), and the limits in `waiting`.
fn limits_shown(
    chosen: &purlis_core::poollimits::Chosen,
    refused: &[purlis_core::poollimits::RefusedValue],
    docker: bool,
    waiting: &[purlis_core::poollimits::LimitId],
) -> PoolLimitsPage {
    use purlis_core::poollimits::{Group, Limit, all};
    let number = |value: u64| u32::try_from(value).unwrap_or(u32::MAX);
    let row = |limit: &Limit, says: bool| PoolLimitRow {
        key: limit.key.to_owned(),
        plain: limit.plain.to_owned(),
        unit: limit.unit.word().to_owned(),
        default: number(chosen.default_of(limit.id)),
        default_said: limit.unit.said(chosen.default_of(limit.id)),
        set: chosen.get(limit.id).map(number),
        applies: says.then(|| limit.takes_effect.said().to_owned()),
        refused: refused
            .iter()
            .find(|one| one.limit == limit.id)
            .map(purlis_core::poollimits::RefusedValue::said)
            .or_else(|| chosen.over_team_said(limit.id)),
        default_by_team: chosen.default_by_team(limit.id),
        suggested: chosen.team_suggests(limit.id).map(number),
        suggested_said: chosen
            .team_suggests(limit.id)
            .map(|value| limit.unit.said(value)),
        highest_said: chosen
            .team_highest(limit.id)
            .map(|top| format!("The team allows up to {}.", limit.unit.said(top))),
    };
    let groups = Group::ALL
        .into_iter()
        .filter_map(|group| {
            let held: Vec<&Limit> = all()
                .iter()
                // The vouch wait is the add's alone: no later attempt waits on it, so a value
                // here would change nothing.
                .filter(|limit| limit.group == group && !limit.advanced && !limit.clusters_only)
                .filter(|limit| docker || !limit.shapes_runner())
                .collect();
            let first = held.first()?;
            let same = held
                .iter()
                .all(|limit| limit.takes_effect == first.takes_effect);
            Some(PoolLimitGroup {
                label: group.plain().to_owned(),
                help: group.help().to_owned(),
                applies: same.then(|| first.takes_effect.said().to_owned()),
                limits: held.iter().map(|limit| row(limit, !same)).collect(),
            })
        })
        .collect();
    PoolLimitsPage {
        groups,
        advanced: all()
            .iter()
            .filter(|limit| limit.advanced)
            .map(|limit| row(limit, true))
            .collect(),
        waiting: waiting
            .iter()
            .map(|id| id.limit().plain.to_owned())
            .collect(),
    }
}

/// The limit named `key`, or why there is none.
fn limit_named(key: &str) -> Result<&'static purlis_core::poollimits::Limit, String> {
    purlis_core::poollimits::of_key(key).ok_or_else(|| {
        format!(
            "a pool has no limit named {}",
            purlis_core::shown::short(key)
        )
    })
}

/// **Pool `pool`'s limits** (#4285), as its page draws them.
#[tauri::command]
#[specta::specta]
pub async fn pool_limits(pool: String) -> Result<PoolLimitsPage, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let config = config_root()?;
        let said = |why: purlis_core::runners::Refused| why.to_string();
        let held = purlis_core::runners::limits_held_of(&config, &pool).map_err(said)?;
        let waiting = purlis_core::runners::changes_waiting(&config, &pool).map_err(said)?;
        Ok(limits_shown(
            &held.chosen,
            &held.refused,
            held.docker,
            &waiting,
        ))
    })
    .await
    .map_err(|why| format!("the limits were not read: {why}"))?
}

/// **The value in the words `typed` for limit `key`, or why they aren't one it can be** (#4285):
/// the file's and the command's check, asked as the person types.
#[tauri::command]
#[specta::specta]
pub async fn pool_limit_check(pool: String, key: String, typed: String) -> Result<u32, String> {
    tauri::async_runtime::spawn_blocking(move || {
        limit_checked_on(&config_root()?, &pool, &key, &typed)
    })
    .await
    .map_err(|why| format!("the value was not checked: {why}"))?
}

/// [`pool_limit_check`], on the machine at `config`. What the team defining a cluster pool allows
/// is refused as it is typed (#4287).
fn limit_checked_on(config: &Path, pool: &str, key: &str, typed: &str) -> Result<u32, String> {
    let limit = limit_named(key)?;
    let chosen =
        purlis_core::runners::chosen_limits_of(config, pool).map_err(|why| why.to_string())?;
    let value = limit.check_typed(typed, chosen.team_highest(limit.id))?;
    u32::try_from(value).map_err(|_| format!("{} is too large a number here", limit.plain))
}

/// **Saves `value` as pool `pool`'s limit `key`, or, `null`, returns it to following purlis's
/// default** (spec #4275), the person's own press in their window: a chat has no way to it, and
/// can only propose through the edit ask (Q6). It applies as the limit says, with nothing
/// restarted.
#[tauri::command]
#[specta::specta]
pub async fn pool_limit_set(pool: String, key: String, value: Option<u32>) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let limit = limit_named(&key)?;
        purlis_core::runners::set_pool_limit(&config_root()?, &pool, limit.id, value.map(u64::from))
            .map_err(|why| why.to_string())
    })
    .await
    .map_err(|why| format!("the limit was not saved: {why}"))?
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
        // Held to the pool's limits, so a value raised applies to the next ask (#4397).
        let limits = purlis_core::runners::chosen_limits_of(&config, &pool)
            .map_err(|why| why.to_string())?;
        let kept: Vec<_> = on_pool
            .iter()
            .map(|record| {
                record
                    .clone()
                    .and_then(onrunner::Kept::of)
                    .map(|kept| kept.held_to(limits))
            })
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
                        .map(|kept| onrunner::standing_on_the_wire(kept.standing))
                        .map_err(|why| late_said(&pool, &limits, why)),
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
    may_sign_in_on(&config, pool)?;
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
    if keep_on_as(Ok(yours.clone()), &config, pool, &kept, Waking::No)?.is_none() {
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
    // A sign-in open on it would say signed in over this sign-out, so it ends first.
    for one in let_go(|one| one.pool == pool && one.name == name) {
        one.driving
            .stop("the sign-in was stopped, as a sign-out of this profile started.");
    }
    // The running runner too: a follow while the line waits makes it one that drains.
    purlis_core::runners::hold_every_runner_from_carrying(&config, pool, name)
        .map_err(|why| why.to_string())?;
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
    /// The pool's limit it was started under, in minutes (spec #4275).
    wait: u64,
    driving: Arc<driven::Driving>,
    /// On a cluster pool, the runner the gate judged (#3819): each code passed on checks it
    /// is still so.
    judged: Option<Box<purlis_core::runners::ClusterKept>>,
}

static SIGN_INS: Mutex<Vec<SignIn>> = Mutex::new(Vec::new());
static NEXT_SIGN_IN: AtomicU32 = AtomicU32::new(1);

/// Why a sign-in ended, where another of the same profile on the same pool replaced it.
const REPLACED: &str = "a later sign-in of this profile took its place";

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
/// stopped. Answers the sign-in's number. With `terminal`, where purlis could not follow the
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
        may_sign_in_on(&config, &pool)?;
        let record = record_named(&config, &name)?;
        profilestore::sign_in::plan_named(&record.harness, provider.as_deref())?;
        let kept = onrunner::Kept::of(&record)?;
        if keep_on_as(Ok(yours.clone()), &config, &pool, &kept, Waking::No)?.is_none() {
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
        // The pool's limit as it stands: only the wait changes, and every check above stands.
        let longest = purlis_core::runners::longest_login_of(&config, &pool);
        let wait = longest.as_secs() / 60;
        let login = login_on(&yours, &config, &pool, &name, provider.as_deref(), typed)?;
        let carry: callback::Carry = {
            let (config, pool, yours) = (config.clone(), pool.clone(), yours.clone());
            Arc::new(move |target: &str| {
                may_sign_in_on(&config, &pool)?;
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
            Some(_) => driven::in_a_terminal(&login, longest, check)?,
            None => driven::drive_with(
                &login,
                &record.harness,
                provider.as_deref(),
                Some(carry),
                longest,
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
                    wait,
                    driving,
                    judged,
                });
                before
            } else {
                held.extend(later);
                drop(held);
                driving.cancel();
                for one in before {
                    one.driving.stop(REPLACED);
                }
                return Err(REPLACED.to_owned());
            }
        };
        for one in before {
            one.driving.stop(REPLACED);
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
    let mut said_over = None;
    if let Some(event) = event.as_ref().filter(|event| event.is_last()) {
        let over = {
            let mut held = SIGN_INS.lock().unwrap_or_else(PoisonError::into_inner);
            held.iter()
                .position(|one| one.flow == flow)
                .map(|at| held.remove(at))
        };
        if let Some(over) = over.as_ref()
            && *event == driven::Event::SignedIn
        {
            let (pool, name) = (over.pool.clone(), over.name.clone());
            // Off this worker: a carry holds the runners' lock for minutes.
            tauri::async_runtime::spawn_blocking(move || {
                carries_again_after_a_sign_in(&pool, &name);
            });
            // Told with no lock held; every project whose wait went has an Ask to stop drawing.
            let went = onrunner::let_go(None, &over.pool, &over.name);
            crate::machineprofiles::tell_asks_at(&app, &went);
        }
        if let Some(over) = over
            && matches!(event, driven::Event::Failed { why } if why == driven::TOOK_TOO_LONG)
        {
            said_over = Some(sign_in_ran_out(&over.pool, over.wait));
        }
    }
    Ok(event.map(|event| match said_over {
        Some(why) => PoolSignInStep::from(driven::Event::Failed { why }),
        None => PoolSignInStep::from(event),
    }))
}

/// A sign-in on `pool` ended at the pool's limit of `wait` minutes, in words the window can ask
/// of for what to raise it to (spec #4275).
fn sign_in_ran_out(pool: &str, wait: u64) -> String {
    let chosen = config_root()
        .ok()
        .and_then(|config| purlis_core::runners::chosen_limits_of(&config, pool).ok())
        .unwrap_or_default();
    let hit = chosen.waited(LimitId::SignInWait, wait, None);
    format!(
        "{}: {}",
        driven::TOOK_TOO_LONG,
        purlis_core::poollimits::hit_on(pool, hit)
    )
}

/// **Passes the code the person pasted to sign-in `flow`**, as one line: on a cluster pool, only
/// while its runner is still as the gate judged it ([`still_driving`]); else the sign-in is
/// stopped and the code goes nowhere.
#[tauri::command]
#[specta::specta]
pub async fn pool_sign_in_paste(flow: u32, code: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || still_driving(flow)?.paste(&code))
        .await
        .map_err(|why| format!("the code was not passed on: {why}"))?
}

/// `name` is signed in on pool `pool`'s runner, as its harness says: a drain carries it again.
fn carries_again_after_a_sign_in(pool: &str, name: &str) {
    let carries = config_root().and_then(|config| {
        purlis_core::runners::carries_again(&config, pool, name).map_err(|why| why.to_string())
    });
    if let Err(why) = carries {
        tracing::warn!("purlis: a drain of pool {pool} will not carry {name}: {why}");
    }
}

/// Pool `pool`'s runner may be signed in on, or why not: no chat ran there without the sandbox,
/// on a pool of either kind.
fn may_sign_in_on(config: &Path, pool: &str) -> Result<(), String> {
    if purlis_core::runnertaint::pool_ran_unsandboxed(config, pool) {
        return Err(purlis_core::runnertaint::signs_nothing_in_on(pool));
    }
    Ok(())
}

/// **What drives sign-in `flow`, where what it was opened on still holds** (#3819): on a
/// cluster pool, the runner the gate judged, still marked ([`still_yours`]); on either kind, run
/// without the sandbox by no chat ([`may_sign_in_on`]); else the sign-in is stopped, saying why (#4277), and nothing goes to
/// it. A sign-in no longer held is refused.
fn still_driving(flow: u32) -> Result<Arc<driven::Driving>, String> {
    let (driving, pool, judged) = SIGN_INS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .iter()
        .find(|one| one.flow == flow)
        .map(|one| {
            (
                Arc::clone(&one.driving),
                one.pool.clone(),
                one.judged.clone(),
            )
        })
        .ok_or_else(|| "that sign-in is over".to_owned())?;
    let held = config_root().and_then(|config| {
        may_sign_in_on(&config, &pool)?;
        match &judged {
            Some(judged) => still_yours(&config, &pool, judged).map(|_| ()),
            None => Ok(()),
        }
    });
    if held.is_err() {
        // Not the start's words: by now something of it may have reached the runner.
        let why =
            format!("the sign-in was stopped, as pool {pool} or its runner changed. Try again.");
        for one in let_go(|one| one.flow == flow) {
            one.driving.stop(&why);
        }
        return Err(why);
    }
    Ok(driving)
}

/// **Cancels every sign-in running on pool `pool`** (#3819): its mark was taken back.
pub fn cancel_sign_ins_on(pool: &str) {
    let why = format!("the sign-in was stopped, as pool {pool} is no longer marked yours.");
    for one in let_go(|one| one.pool == pool) {
        one.driving.stop(&why);
    }
}

/// **The sign-ins `which` picks, no longer held**, for the caller to end with no lock held.
fn let_go(which: impl Fn(&SignIn) -> bool) -> Vec<SignIn> {
    let mut held = SIGN_INS.lock().unwrap_or_else(PoisonError::into_inner);
    let (gone, kept) = std::mem::take(&mut *held).into_iter().partition(which);
    *held = kept;
    gone
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
                draining =
                    purlis_core::pools::sign_out_draining(&config, &docker, &pool, &name, &[])
                        .map_err(|why| why.to_string())?;
                let runner = this_desktops_runner(&config, &pool)
                    .map_err(|why| with_draining_said(why, &name, &draining))?;
                docker
                    .sign_out(&runner.made, &name)
                    .map_err(|why| with_draining_said(why, &name, &draining))?;
                // A follow since the first look leaves a runner draining that was not reached.
                let late = purlis_core::pools::sign_out_draining(
                    &config, &docker, &pool, &name, &draining,
                )
                .map_err(|why| with_draining_said(why.to_string(), &name, &draining))?;
                draining.extend(late);
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
    for one in let_go(|one| one.flow == flow) {
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

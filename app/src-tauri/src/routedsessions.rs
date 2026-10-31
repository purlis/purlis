// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A project's chats, each on the host its workspace runs them on** (#2046, ADR 0078 §7):
//! this machine's session host, or the runner of the pool this machine's store names for the
//! workspace (`chatsRunOn` in `machine.json`, the person's ruling of 2026-10-20).
//!
//! **One project, one numbering.** Every number is dealt by this machine's host, those of the
//! chats a runner runs included, and a runner is given the number its chat was dealt; so no two
//! chats of the project ever share one, wherever each runs, and the board, Needs you and the
//! record keep telling them apart as before.
//!
//! **Where a chat runs is decided as it starts, and kept.** A harness chat in a workspace that
//! runs on a pool is started there by its names, and every command about it after (its views,
//! what is typed into it, its asks, its stop) goes to the host that started it. A shell runs on
//! this machine, wherever its workspace's chats run: its window composed it here. A chat cannot
//! choose its host: the setting is in this machine's store, which no chat writes, and a
//! committed file never names a pool.
//!
//! **This machine's host keeps the project's board**, for every chat, those on runners
//! included, so it is held there before any chat starts on a runner.
//!
//! **The runners' branches come back** into each clone's `refs/remotes/<pool>/*` as a runner
//! chat ends, and when the window comes back into focus, as the project is fetched then
//! (#2002), at most once a minute. One fetch runs at a time, and chats that end together are
//! fetched for once.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use purlis_core::engine::Size;

use crate::host::{
    Composed, Ends, Named, Opening, Readiness, Reconnecting, SessionHost, Sink, Watching,
};

/// Which pool a workspace's chats run on, as this machine's store says it now: `None` for this
/// machine, or the refusal where it names a pool this machine does not keep.
pub type RunsOn = Arc<dyn Fn(&str) -> Result<Option<String>, String> + Send + Sync>;

/// Every pool and the workspaces of this project that run their chats on it, as the store says
/// them now.
pub type OnPools = Arc<dyn Fn() -> BTreeMap<String, Vec<String>> + Send + Sync>;

/// The project's chats on the runner of a pool: made the first time the pool is needed.
pub type ToAPool = Arc<dyn Fn(&str) -> Result<Box<dyn SessionHost>, String> + Send + Sync>;

/// The least time between two fetches of the runners' branches the window's focus asks for.
const FETCH_BACK_AT_MOST_EVERY: Duration = Duration::from_secs(60);

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A project's chats, routed by workspace (#2046).
pub struct RoutedSessions {
    root: PathBuf,
    here: Box<dyn SessionHost>,
    runs_on: RunsOn,
    on_pools: OnPools,
    to_a_pool: ToAPool,
    shared: Arc<Shared>,
    /// The pool every start runs on, whatever it is ([`Self::every_start_on`]).
    every_start_on: Option<String>,
}

/// A host left with chats that stayed on a draining runner.
struct Left {
    pool: String,
    made: u64,
    host: Arc<dyn SessionHost>,
}

/// What every pool's chats share with the router: what is told to each as it is made.
#[derive(Default)]
struct Shared {
    pools: Mutex<BTreeMap<String, Arc<dyn SessionHost>>>,
    /// Each pool's host left with the chats that stayed on a draining runner (#4248): its pool's
    /// next chats start on a host of their own, on the pool's runner now.
    left: Mutex<Vec<Left>>,
    /// The chats that stayed on a draining runner, and how they are found in `left`.
    stayed: Mutex<HashMap<u32, u64>>,
    /// Held while a pool's host is replaced for a start: two starts make one host between them.
    replacing: Mutex<()>,
    /// How many hosts were made, which names each one.
    made: AtomicU64,
    /// Which made host each pool's now is.
    current: Mutex<BTreeMap<String, u64>>,
    /// The pool each chat that runs on one runs on, by its number.
    chats: Mutex<HashMap<u32, String>>,
    switch: Mutex<Option<Arc<crate::killswitch::KillSwitch>>>,
    ends: Mutex<Option<Ends>>,
    reconnecting: Mutex<Option<Reconnecting>>,
    relayed: Mutex<Option<Arc<crate::hooks::Relayed>>>,
    skill_runs: Mutex<Option<crate::host::SkillRunsHeard>>,
    /// The highest number said to have gone out, for a pool made after it was said.
    dealt: AtomicU32,
    /// When the runners' branches were last fetched for the window's focus.
    fetched: Mutex<Option<Instant>>,
    /// Held while the runners' branches are fetched, so no two fetches run into one clone.
    fetching: Mutex<()>,
    /// The pools a fetch for a chat's end waits to run for: an end while one waits adds none,
    /// since the one that waits fetches what that chat did too.
    ending: Mutex<BTreeSet<String>>,
}

impl RoutedSessions {
    /// The chats of the project at `root`: on `here`, this machine's host, or on the runner of
    /// the pool `runs_on` names for their workspace, reached through `to_a_pool`.
    pub fn new(
        root: &Path,
        here: Box<dyn SessionHost>,
        runs_on: RunsOn,
        on_pools: OnPools,
        to_a_pool: ToAPool,
    ) -> Self {
        RoutedSessions {
            root: root.to_path_buf(),
            here,
            runs_on,
            on_pools,
            to_a_pool,
            shared: Arc::default(),
            every_start_on: None,
        }
    }

    /// **The project at `root`'s chats, each on the host its workspace runs them on** (#2046):
    /// `here`, or the runner of the pool `config`'s store names for its workspace, reached through
    /// a link the window's host relays to it.
    pub(crate) fn of_the_project(
        here: Box<dyn SessionHost>,
        connection: &Arc<crate::hostedsessions::Connection>,
        config: &Path,
        root: &Path,
    ) -> RoutedSessions {
        let (config, plane) = (config.to_path_buf(), root.to_path_buf());
        let runs_on: RunsOn = {
            let (config, plane) = (config.clone(), plane.clone());
            Arc::new(move |workspace| purlis_core::pools::runs_on(&config, &plane, Some(workspace)))
        };
        let on_pools: OnPools = {
            let (config, plane) = (config.clone(), plane.clone());
            Arc::new(move || purlis_core::pools::workspaces_on_pools(&config, &plane))
        };
        let to_a_pool: ToAPool = {
            let (connection, plane) = (Arc::clone(connection), plane.clone());
            Arc::new(move |pool| {
                purlis_core::runners::runner_of(&config, pool).map_err(|why| format!("{why}."))?;
                Ok(
                    Box::new(crate::hostedsessions::HostedSessions::on_a_pools_runner(
                        connection.to_a_pool(pool),
                        &plane,
                    )) as Box<dyn SessionHost>,
                )
            })
        };
        RoutedSessions::new(root, here, runs_on, on_pools, to_a_pool)
    }

    /// [`Self::of_the_project`], every start on the runner of `pool`, a shell's too, and every
    /// harness chat whatever its workspace: the behaviour suite's Docker pool target (#2045),
    /// which holds each behaviour to the runner's host. The app runs a shell where its window
    /// composed it, and a harness chat where its workspace's chats run.
    #[cfg(test)]
    pub(crate) fn every_start_on(mut self, pool: &str) -> Self {
        self.every_start_on = Some(pool.to_owned());
        self
    }

    /// Starts a chat on the runner of `pool` with `start`, given the number this machine's
    /// host dealt it (or `wanted`): this machine's host holds the project and keeps its board
    /// first, and the chat is known as the pool's before it starts, so its first hook line is
    /// heard from there alone. A runner that starts it under another number has it closed.
    fn on_a_pool<T: Numbered>(
        &self,
        pool: &str,
        wanted: Option<u32>,
        start: impl FnOnce(&dyn SessionHost, u32) -> Result<T, String>,
    ) -> Result<T, String> {
        let host = self.pool_for_a_start(pool)?;
        self.here.hold_here();
        let number = wanted
            .filter(|wanted| *wanted > 0)
            .unwrap_or_else(|| self.here.deal());
        self.shared.dealt.fetch_max(number, Ordering::Relaxed);
        lock(&self.shared.chats).insert(number, pool.to_owned());
        lock(&self.shared.stayed).remove(&number);
        match start(host.as_ref(), number) {
            Ok(started) if started.number() == number => Ok(started),
            Ok(started) => {
                lock(&self.shared.chats).remove(&number);
                let _ = host.close(started.number());
                Err(format!(
                    "the runner of pool {pool} started the chat as chat {}, not as chat \
                     {number}, which this project dealt it, so it was closed",
                    started.number()
                ))
            }
            Err(why) => {
                lock(&self.shared.chats).remove(&number);
                // The runner the link reached began to drain as this start went out, between the
                // window's look and the host's: nothing started, and the next start goes to the
                // pool's runner now (#4248).
                Err(
                    if why == purlis_session_protocol::session::A_DRAINING_RUNNER_STARTS_NONE {
                        format!(
                            "pool {pool}'s runner was replaced just as this chat started, so \
                             nothing was started: start it again, and it runs on the new runner"
                        )
                    } else {
                        why
                    },
                )
            }
        }
    }

    /// The chats on the runner of `pool`, made now where they were not, and told what every
    /// host of the project is told.
    fn pool(&self, pool: &str) -> Result<Arc<dyn SessionHost>, String> {
        pool_of(&self.shared, &self.to_a_pool, pool)
    }

    /// **The host a new chat on `pool` starts on** (#4248): one whose runner drains now stays with
    /// its chats, and a host of its own is made for the pool's runner now.
    fn pool_for_a_start(&self, pool: &str) -> Result<Arc<dyn SessionHost>, String> {
        let _one_at_a_time = lock(&self.shared.replacing);
        let host = self.pool(pool)?;
        if !host.stays_on_a_draining_runner() {
            return Ok(host);
        }
        let shared = &self.shared;
        let made = lock(&shared.current).get(pool).copied().unwrap_or_default();
        {
            let mut stayed = lock(&shared.stayed);
            for (chat, on) in lock(&shared.chats).iter() {
                if on == pool {
                    stayed.entry(*chat).or_insert(made);
                }
            }
        }
        lock(&shared.pools).remove(pool);
        lock(&shared.left).push(Left {
            pool: pool.to_owned(),
            made,
            host,
        });
        self.pool(pool)
    }

    /// The host chat `id` runs on: a pool's, where it was started there, else this machine's.
    fn host_of(&self, id: u32) -> Option<Arc<dyn SessionHost>> {
        if let Some(made) = lock(&self.shared.stayed).get(&id).copied() {
            return lock(&self.shared.left)
                .iter()
                .find(|left| left.made == made)
                .map(|left| Arc::clone(&left.host));
        }
        let pool = lock(&self.shared.chats).get(&id).cloned()?;
        lock(&self.shared.pools).get(&pool).cloned()
    }

    /// Every pool's chats made so far, those left on a draining runner among them.
    fn pools(&self) -> Vec<Arc<dyn SessionHost>> {
        lock(&self.shared.pools)
            .values()
            .cloned()
            .chain(
                lock(&self.shared.left)
                    .iter()
                    .map(|left| Arc::clone(&left.host)),
            )
            .collect()
    }

    /// Every pool's host by its pool, those left on a draining runner among them.
    fn named_pools(&self) -> Vec<(String, Arc<dyn SessionHost>)> {
        every_host(&self.shared)
            .into_iter()
            .map(|(pool, _, host)| (pool, host))
            .collect()
    }

    /// The pool `workspace` runs its chats on, where it runs them on one.
    fn route(&self, workspace: Option<&str>) -> Result<Option<String>, String> {
        if let Some(pool) = &self.every_start_on {
            return Ok(Some(pool.clone()));
        }
        match workspace {
            Some(workspace) => (self.runs_on)(workspace),
            None => Ok(None),
        }
    }
}

fn pool_of(
    shared: &Arc<Shared>,
    to_a_pool: &ToAPool,
    pool: &str,
) -> Result<Arc<dyn SessionHost>, String> {
    if let Some(made) = lock(&shared.pools).get(pool) {
        return Ok(Arc::clone(made));
    }
    let mut made = to_a_pool(pool)?;
    let number = shared.made.fetch_add(1, Ordering::Relaxed);
    if let Some(switch) = lock(&shared.switch).clone() {
        made.stopped_by(switch);
    }
    if let Some(relayed) = lock(&shared.relayed).clone() {
        made.hear_through(relayed.for_a_pool(pool, routes_of_host(shared, pool, number)));
    }
    if let Some(ends) = lock(&shared.ends).clone() {
        made.when_one_ends(ends);
    }
    if let Some(reconnecting) = lock(&shared.reconnecting).clone() {
        made.when_reconnecting(reconnecting);
    }
    if let Some(tell) = lock(&shared.skill_runs).clone() {
        made.when_skill_runs_heard(of_the_pool(pool, tell));
    }
    let dealt = shared.dealt.load(Ordering::Relaxed);
    if dealt > 0 {
        made.already_dealt(dealt);
    }
    let made: Arc<dyn SessionHost> = Arc::from(made);
    let mut pools = lock(&shared.pools);
    if let Some(already) = pools.get(pool) {
        return Ok(Arc::clone(already));
    }
    lock(&shared.current).insert(pool.to_owned(), number);
    Ok(Arc::clone(pools.entry(pool.to_owned()).or_insert(made)))
}

/// Every host of every pool, as its pool, its number and itself: each pool's now first, so what a
/// draining runner has comes back beside what the pool's runner has ([`fetch_back_all`]).
fn every_host(shared: &Shared) -> Vec<(String, u64, Arc<dyn SessionHost>)> {
    let current = lock(&shared.current).clone();
    let mut hosts: Vec<_> = lock(&shared.pools)
        .iter()
        .map(|(pool, host)| {
            let made = current.get(pool).copied().unwrap_or_default();
            (pool.clone(), made, Arc::clone(host))
        })
        .collect();
    hosts.extend(
        lock(&shared.left)
            .iter()
            .map(|left| (left.pool.clone(), left.made, Arc::clone(&left.host))),
    );
    hosts
}

/// `tell`, told each skill run as one of `pool`'s runner's (#3084).
fn of_the_pool(pool: &str, tell: crate::host::SkillRunsHeard) -> crate::host::SkillRunsHeard {
    let pool = pool.to_owned();
    Arc::new(move |_, run| tell(Some(&pool), run))
}

/// [`routes_of`], as host `made` of `pool` hears its lines: a chat that runs on another host of the
/// pool is not one of its own, so a draining runner speaks for no chat on the pool's runner now, nor
/// that one for a chat that stayed (#4248).
fn routes_of_host(shared: &Arc<Shared>, pool: &str, made: u64) -> crate::hooks::Routes {
    let shared = Arc::downgrade(shared);
    let pool = pool.to_owned();
    Arc::new(move |chat| {
        let shared = shared.upgrade()?;
        let on = lock(&shared.chats).get(&chat).cloned()?;
        let host = match lock(&shared.stayed).get(&chat) {
            Some(stayed) => *stayed,
            None => lock(&shared.current).get(&pool).copied()?,
        };
        (host == made).then_some(on)
    })
}

/// Which pool each chat runs on, as the router has it now: `None` for this machine's.
fn routes_of(shared: &Arc<Shared>) -> crate::hooks::Routes {
    let shared = Arc::downgrade(shared);
    Arc::new(move |chat| {
        shared
            .upgrade()
            .and_then(|shared| lock(&shared.chats).get(&chat).cloned())
    })
}

fn fetch_back_all(shared: &Arc<Shared>, on_pools: &OnPools, to_a_pool: &ToAPool) -> Vec<String> {
    let mut failed = Vec::new();
    for (pool, workspaces) in on_pools() {
        failed.extend(fetch_back_pool(shared, to_a_pool, &pool, &workspaces));
    }
    failed
}

/// The branches of `workspaces` back from each runner of `pool`, each draining one's beside its
/// runner's now (#4248).
fn fetch_back_pool(
    shared: &Arc<Shared>,
    to_a_pool: &ToAPool,
    pool: &str,
    workspaces: &[String],
) -> Vec<String> {
    let mut failed = Vec::new();
    let host = match pool_of(shared, to_a_pool, pool) {
        Ok(host) => Some(host),
        Err(why) => {
            failed.push(why);
            None
        }
    };
    // Those whose runner went are let go of first, with their chats.
    let gone: Vec<u64> = lock(&shared.left)
        .iter()
        .filter(|left| left.pool == pool)
        .map(|left| (left.made, Arc::clone(&left.host)))
        .collect::<Vec<_>>()
        .into_iter()
        .filter(|(_, host)| host.runner_is_gone())
        .map(|(made, _)| made)
        .collect();
    lock(&shared.left).retain(|left| !gone.contains(&left.made));
    lock(&shared.stayed).retain(|_, made| !gone.contains(made));
    let left: Vec<Arc<dyn SessionHost>> = lock(&shared.left)
        .iter()
        .filter(|left| left.pool == pool)
        .map(|left| Arc::clone(&left.host))
        .collect();
    // The pool's runner last: where both have a branch, its commit is the one kept.
    for host in left.into_iter().chain(host) {
        for workspace in workspaces {
            failed.extend(host.fetch_back(workspace, pool));
        }
    }
    failed
}

/// What a start on a pool answers, by the number its chat was started as.
trait Numbered {
    fn number(&self) -> u32;
}

impl Numbered for u32 {
    fn number(&self) -> u32 {
        *self
    }
}

impl Numbered for Composed {
    fn number(&self) -> u32 {
        self.number
    }
}

impl SessionHost for RoutedSessions {
    /// On this machine: a start the window composed (a shell, or a program that is no harness
    /// a runner composes) runs where the window composed it.
    fn open(
        &self,
        wanted: Option<u32>,
        opening: &Opening,
        announce: &dyn Fn(u32),
    ) -> Result<u32, String> {
        match &self.every_start_on {
            Some(pool) => self.on_a_pool(pool, wanted, |host, number| {
                host.open(Some(number), opening, announce)
            }),
            None => self.here.open(wanted, opening, announce),
        }
    }

    fn open_named(
        &self,
        wanted: Option<u32>,
        named: &Named,
        hearing: purlis_core::sandbox::plan::Hearing,
        announce: &dyn Fn(u32),
    ) -> Result<Composed, String> {
        let Some(pool) = self.route(named.workspace.as_deref())? else {
            return self.here.open_named(wanted, named, hearing, announce);
        };
        self.on_a_pool(&pool, wanted, |host, number| {
            host.open_named(Some(number), named, hearing, announce)
        })
    }

    fn composes_its_harness_chats(&self) -> bool {
        self.here.composes_its_harness_chats()
    }

    fn pool_for(&self, workspace: Option<&str>) -> Option<String> {
        self.route(workspace).ok().flatten()
    }

    fn composes_its_harness_chats_in(&self, workspace: Option<&str>) -> bool {
        match self.route(workspace) {
            // A start whose pool is not kept is refused as it opens, in its own words.
            Ok(Some(_)) | Err(_) => true,
            Ok(None) => self.here.composes_its_harness_chats_in(workspace),
        }
    }

    fn composes_the_harness_chat(&self, id: u32) -> bool {
        match self.host_of(id) {
            Some(host) => host.composes_its_harness_chats(),
            None => self.here.composes_the_harness_chat(id),
        }
    }

    fn input(&self, id: u32, bytes: &[u8]) -> Result<(), String> {
        match self.host_of(id) {
            Some(host) => host.input(id, bytes),
            None => self.here.input(id, bytes),
        }
    }

    fn resize(&self, id: u32, size: Size) -> Result<(), String> {
        match self.host_of(id) {
            Some(host) => host.resize(id, size),
            None => self.here.resize(id, size),
        }
    }

    fn watch(&self, id: u32, sink: Sink) -> Result<Watching, String> {
        match self.host_of(id) {
            Some(host) => host.watch(id, sink),
            None => self.here.watch(id, sink),
        }
    }

    fn unwatch(&self, id: u32, view: u32) -> Result<(), String> {
        match self.host_of(id) {
            Some(host) => host.unwatch(id, view),
            None => self.here.unwatch(id, view),
        }
    }

    fn close(&self, id: u32) -> Result<(), String> {
        let closed = match self.host_of(id) {
            Some(host) => host.close(id),
            None => self.here.close(id),
        };
        if closed.is_ok() {
            lock(&self.shared.chats).remove(&id);
            lock(&self.shared.stayed).remove(&id);
        }
        closed
    }

    fn end_all(&self) {
        for pool in self.pools() {
            pool.end_all();
        }
        self.here.end_all();
    }

    fn stop_every_program(&self) -> usize {
        self.pools()
            .iter()
            .map(|pool| pool.stop_every_program())
            .sum::<usize>()
            + self.here.stop_every_program()
    }

    fn stopped_by(&mut self, switch: Arc<crate::killswitch::KillSwitch>) {
        *lock(&self.shared.switch) = Some(Arc::clone(&switch));
        self.here.stopped_by(switch);
    }

    fn process_id(&self, id: u32) -> Option<u32> {
        match self.host_of(id) {
            Some(host) => host.process_id(id),
            None => self.here.process_id(id),
        }
    }

    /// Told on every host; a runner chat's end also fetches its workspace's branches back.
    fn when_one_ends(&self, tell: Ends) {
        let told: Ends = {
            let shared = Arc::downgrade(&self.shared);
            let to_a_pool = Arc::clone(&self.to_a_pool);
            let root = self.root.clone();
            let on_pools = Arc::clone(&self.on_pools);
            Arc::new(move |id, exit| {
                tell(id, exit);
                let Some(shared) = shared.upgrade() else {
                    return;
                };
                let Some(pool) = lock(&shared.chats).get(&id).cloned() else {
                    return;
                };
                // One fetch waits for each pool at most: chats that end together fetch once.
                if !lock(&shared.ending).insert(pool.clone()) {
                    return;
                }
                let (to_a_pool, on_pools, root) =
                    (Arc::clone(&to_a_pool), Arc::clone(&on_pools), root.clone());
                // Off the thread that tells: a fetch crosses the link.
                let spawned = std::thread::Builder::new()
                    .name("purlis-runner-end-fetch".into())
                    .spawn({
                        let shared = Arc::clone(&shared);
                        let pool = pool.clone();
                        move || {
                            let _one_at_a_time = lock(&shared.fetching);
                            lock(&shared.ending).remove(&pool);
                            let workspaces = on_pools().remove(&pool).unwrap_or_default();
                            for why in fetch_back_pool(&shared, &to_a_pool, &pool, &workspaces) {
                                tracing::warn!("purlis: in {}: {why}", root.display());
                            }
                        }
                    });
                if let Err(why) = spawned {
                    lock(&shared.ending).remove(&pool);
                    tracing::warn!("purlis: the runner's branches were not fetched: {why}");
                }
            })
        };
        *lock(&self.shared.ends) = Some(Arc::clone(&told));
        for pool in self.pools() {
            pool.when_one_ends(Arc::clone(&told));
        }
        self.here.when_one_ends(told);
    }

    fn when_reconnecting(&self, tell: Reconnecting) {
        *lock(&self.shared.reconnecting) = Some(Arc::clone(&tell));
        for pool in self.pools() {
            pool.when_reconnecting(Arc::clone(&tell));
        }
        self.here.when_reconnecting(tell);
    }

    /// Each pool's runner tells its own, as the pool's (#3084); this machine's host tells none.
    fn when_skill_runs_heard(&self, tell: crate::host::SkillRunsHeard) {
        *lock(&self.shared.skill_runs) = Some(Arc::clone(&tell));
        for (name, pool) in self.named_pools() {
            pool.when_skill_runs_heard(of_the_pool(&name, Arc::clone(&tell)));
        }
    }

    fn reconnecting(&self) -> Vec<u32> {
        let mut all: BTreeSet<u32> = self.here.reconnecting().into_iter().collect();
        for pool in self.pools() {
            all.extend(pool.reconnecting());
        }
        all.into_iter().collect()
    }

    fn lost_with_its_host(&self, id: u32) -> bool {
        match self.host_of(id) {
            Some(host) => host.lost_with_its_host(id),
            None => self.here.lost_with_its_host(id),
        }
    }

    fn running(&self) -> Vec<u32> {
        let mut all: BTreeSet<u32> = self.here.running().into_iter().collect();
        for pool in self.pools() {
            all.extend(pool.running());
        }
        all.into_iter().collect()
    }

    fn readiness(&self, id: u32) -> Result<Readiness, String> {
        match self.host_of(id) {
            Some(host) => host.readiness(id),
            None => self.here.readiness(id),
        }
    }

    /// Dealt by this machine's host alone, for every chat of the project.
    fn deal(&self) -> u32 {
        let number = self.here.deal();
        self.shared.dealt.fetch_max(number, Ordering::Relaxed);
        number
    }

    fn dealt(&self) -> u32 {
        self.pools()
            .iter()
            .map(|pool| pool.dealt())
            .chain([self.here.dealt(), self.shared.dealt.load(Ordering::Relaxed)])
            .max()
            .unwrap_or_default()
    }

    fn already_dealt(&self, dealt: u32) {
        self.shared.dealt.fetch_max(dealt, Ordering::Relaxed);
        for pool in self.pools() {
            pool.already_dealt(dealt);
        }
        self.here.already_dealt(dealt);
    }

    /// A confinement the window planned is this machine's: a runner composes its own. Where every
    /// start runs on a pool ([`Self::every_start_on`]), the window's shells run there too, and so
    /// does the confinement it planned for one.
    fn confine(
        &self,
        plan: &purlis_core::sandbox::plan::Plan,
        hearing: purlis_core::sandbox::plan::Hearing,
    ) -> Result<purlis_core::sandbox::Confinement, String> {
        match &self.every_start_on {
            Some(pool) => self.pool(pool)?.confine(plan, hearing),
            None => self.here.confine(plan, hearing),
        }
    }

    fn reports_to(&self) -> Option<PathBuf> {
        self.here.reports_to()
    }

    /// Saved on this machine, where its clones are; the window's focus also fetches every
    /// runner's branches back, at most once a minute.
    fn keep_saving(
        &self,
        root: &Path,
        told: crate::autosave::Told,
    ) -> Box<dyn crate::autosave::Saver> {
        let inner = self.here.keep_saving(root, told);
        Box::new(FetchingBack::start(
            inner,
            Arc::clone(&self.shared),
            Arc::clone(&self.on_pools),
            Arc::clone(&self.to_a_pool),
        ))
    }

    /// Each host hears its own lines, and only for the chats that run on it: this machine's
    /// host through `relayed`, each pool's runner through one made beside it.
    fn hear_through(&self, relayed: Arc<crate::hooks::Relayed>) {
        relayed.routed(routes_of(&self.shared));
        *lock(&self.shared.relayed) = Some(Arc::clone(&relayed));
        for (name, made, pool) in every_host(&self.shared) {
            pool.hear_through(relayed.for_a_pool(&name, routes_of_host(&self.shared, &name, made)));
        }
        self.here.hear_through(relayed);
    }

    fn hold_here(&self) {
        self.here.hold_here();
    }

    /// On the runner of `pool`: this machine's host keeps the board, so it holds the project
    /// first.
    fn move_ahead(&self, pool: &str, workspace: &str) -> Result<(), String> {
        let host = self.pool_for_a_start(pool)?;
        self.here.hold_here();
        host.move_ahead(pool, workspace)
    }

    fn move_in(&self, pool: &str, chats: &[crate::host::Moving]) -> Result<(), String> {
        self.pool_for_a_start(pool)?.move_in(pool, chats)
    }

    fn move_out(
        &self,
        pool: &str,
        chats: &[crate::host::Moving],
    ) -> Result<crate::host::CameBack, String> {
        let left: Vec<Arc<dyn SessionHost>> = lock(&self.shared.left)
            .iter()
            .filter(|left| left.pool == pool)
            .map(|left| Arc::clone(&left.host))
            .collect();
        let current = self.pool(pool)?;
        if left.is_empty() {
            return current.move_out(pool, chats);
        }
        // During a drain they come back whole from the one runner that has them all, the pool's
        // first: a chat moved there has left an older copy on the draining one (#4248). Chats on
        // two runners are not mixed into one work tree.
        let mut asked = current.move_out(pool, chats);
        for host in &left {
            if asked.is_ok() {
                break;
            }
            asked = host.move_out(pool, chats);
        }
        asked.map_err(|why| {
            format!(
                "pool {pool}'s runner is being replaced, and no one runner has all of these chats, \
                 so nothing moved. Close the chats on the older runner, or move them once it has \
                 gone: {why}"
            )
        })
    }

    fn pool_of(&self, id: u32) -> Option<String> {
        lock(&self.shared.chats).get(&id).cloned()
    }

    /// Each pool's own host answers this, to the router: the router itself drains with nothing.
    fn stays_on_a_draining_runner(&self) -> bool {
        false
    }

    /// As [`Self::stays_on_a_draining_runner`].
    fn runner_is_gone(&self) -> bool {
        false
    }

    /// Asked of the host of the pool it runs on, where there is one already: never made for it.
    fn clones_come_back(&self, id: u32) -> bool {
        self.host_of(id)
            .is_some_and(|host| host.clones_come_back(id))
    }

    /// None for the project as a whole: each pool's is said by [`Self::runner_logs`] (#3393).
    fn runner_log(&self) -> Option<crate::runnerlog::RunnerLogNow> {
        None
    }

    /// Each pool's runner log this project's chats read (#3393).
    fn runner_logs(&self) -> Vec<(String, crate::runnerlog::RunnerLogNow)> {
        // Each pool's runner now: a draining runner's log waits (#4271).
        let pools: Vec<(String, Arc<dyn SessionHost>)> = lock(&self.shared.pools)
            .iter()
            .map(|(pool, host)| (pool.clone(), Arc::clone(host)))
            .collect();
        pools
            .into_iter()
            .filter_map(|(pool, host)| host.runner_log().map(|log| (pool, log)))
            .collect()
    }

    /// Asked of this machine's host, then of each pool a workspace of the project runs its
    /// chats on; one left running on a pool is kept as that pool's.
    /// Asked of this machine's host alone: a chat whose workspace is not known runs here.
    fn left_running(&self, number: u32) -> bool {
        self.here.left_running(number)
    }

    /// Asked of this machine's host, then of the one pool `workspace` runs its chats on: a
    /// chat that pool says it runs is kept as that pool's. No other pool is asked, so no runner
    /// takes a chat of a workspace that does not run on it.
    fn left_running_in(&self, number: u32, workspace: Option<&str>) -> bool {
        if self.here.left_running(number) {
            return true;
        }
        let Ok(Some(pool)) = self.route(workspace) else {
            return false;
        };
        let Ok(host) = self.pool(&pool) else {
            return false;
        };
        if host.left_running(number) {
            lock(&self.shared.chats).insert(number, pool);
            return true;
        }
        false
    }

    fn reattach(&self, number: u32, announce: &dyn Fn(u32)) -> Result<u32, String> {
        match self.host_of(number) {
            Some(host) => host.reattach(number, announce),
            None => self.here.reattach(number, announce),
        }
    }

    fn confined_again(
        &self,
        number: u32,
        hearing: purlis_core::sandbox::plan::Hearing,
    ) -> Option<Result<purlis_core::sandbox::Confinement, String>> {
        match self.host_of(number) {
            Some(host) => host.confined_again(number, hearing),
            None => self.here.confined_again(number, hearing),
        }
    }

    fn state_not_reattached(&self) -> Option<String> {
        self.here.state_not_reattached()
    }

    fn ran_with(&self, number: u32) -> Option<purlis_core::sandbox::plan::Plan> {
        match self.host_of(number) {
            Some(host) => host.ran_with(number),
            None => self.here.ran_with(number),
        }
    }

    fn prompt_key_started(
        &self,
        number: u32,
        minted: Option<purlis_core::skillspan::PromptSalt>,
    ) -> Option<purlis_core::skillspan::PromptSalt> {
        match self.host_of(number) {
            Some(host) => host.prompt_key_started(number, minted),
            None => self.here.prompt_key_started(number, minted),
        }
    }

    fn prompt_key_left(&self, number: u32) -> Option<purlis_core::skillspan::PromptSalt> {
        match self.host_of(number) {
            Some(host) => host.prompt_key_left(number),
            None => self.here.prompt_key_left(number),
        }
    }

    fn ran_in(&self, number: u32) -> Option<purlis_session_protocol::session::RunSandbox> {
        match self.host_of(number) {
            Some(host) => host.ran_in(number),
            None => self.here.ran_in(number),
        }
    }

    fn close_what_was_not_reattached(&self) {
        for pool in self.pools() {
            pool.close_what_was_not_reattached();
        }
        self.here.close_what_was_not_reattached();
    }

    /// This machine's host's alone: a browser runs only there (#2025).
    fn browsers_left(&self) -> Vec<purlis_session_protocol::session::HeldProgram> {
        self.here.browsers_left()
    }
}

/// The project's saver, which also fetches the runners' branches back as the window comes back
/// into focus, at most once a minute (#2046).
struct FetchingBack {
    inner: Box<dyn crate::autosave::Saver>,
    poke: std::sync::mpsc::Sender<crate::autosave::Poke>,
}

impl FetchingBack {
    fn start(
        inner: Box<dyn crate::autosave::Saver>,
        shared: Arc<Shared>,
        on_pools: OnPools,
        to_a_pool: ToAPool,
    ) -> FetchingBack {
        let (poke, poked) = std::sync::mpsc::channel::<crate::autosave::Poke>();
        let on = inner.poker();
        let shared = Arc::downgrade(&shared);
        let started = std::thread::Builder::new()
            .name("purlis-runners-fetch".into())
            .spawn(move || {
                while let Ok(poke) = poked.recv() {
                    let _ = on.send(poke);
                    let Some(shared) = shared.upgrade() else {
                        return;
                    };
                    if poke != crate::autosave::Poke::Fetch {
                        continue;
                    }
                    {
                        let mut fetched = lock(&shared.fetched);
                        if fetched.is_some_and(|at| at.elapsed() < FETCH_BACK_AT_MOST_EVERY) {
                            continue;
                        }
                        *fetched = Some(Instant::now());
                    }
                    let _one_at_a_time = lock(&shared.fetching);
                    for why in fetch_back_all(&shared, &on_pools, &to_a_pool) {
                        tracing::warn!("purlis: {why}");
                    }
                }
            });
        if let Err(why) = started {
            tracing::warn!("purlis: the runners' branches will not be fetched on focus: {why}");
        }
        FetchingBack { inner, poke }
    }
}

impl crate::autosave::Saver for FetchingBack {
    fn poker(&self) -> std::sync::mpsc::Sender<crate::autosave::Poke> {
        self.poke.clone()
    }

    fn asks_mid_turn_of(&self, ask: crate::autosave::MidTurn) {
        self.inner.asks_mid_turn_of(ask);
    }

    fn stop_by(self: Box<Self>, deadline: Instant) -> bool {
        self.inner.stop_by(deadline)
    }
}

#[cfg(test)]
mod tests;

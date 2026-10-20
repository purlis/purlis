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
//! (#2002), at most once a minute.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
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
}

/// What every pool's chats share with the router: what is told to each as it is made.
#[derive(Default)]
struct Shared {
    pools: Mutex<BTreeMap<String, Arc<dyn SessionHost>>>,
    /// The pool each chat that runs on one runs on, by its number.
    chats: Mutex<HashMap<u32, String>>,
    switch: Mutex<Option<Arc<crate::killswitch::KillSwitch>>>,
    ends: Mutex<Option<Ends>>,
    reconnecting: Mutex<Option<Reconnecting>>,
    relayed: Mutex<Option<Arc<crate::hooks::Relayed>>>,
    /// The highest number said to have gone out, for a pool made after it was said.
    dealt: AtomicU32,
    /// When the runners' branches were last fetched for the window's focus.
    fetched: Mutex<Option<Instant>>,
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
        }
    }

    /// The chats on the runner of `pool`, made now where they were not, and told what every
    /// host of the project is told.
    fn pool(&self, pool: &str) -> Result<Arc<dyn SessionHost>, String> {
        pool_of(&self.shared, &self.to_a_pool, pool)
    }

    /// The host chat `id` runs on: a pool's, where it was started there, else this machine's.
    fn host_of(&self, id: u32) -> Option<Arc<dyn SessionHost>> {
        let pool = lock(&self.shared.chats).get(&id).cloned()?;
        lock(&self.shared.pools).get(&pool).cloned()
    }

    /// Every pool's chats made so far.
    fn pools(&self) -> Vec<Arc<dyn SessionHost>> {
        lock(&self.shared.pools).values().cloned().collect()
    }

    /// The pool `workspace` runs its chats on, where it runs them on one.
    fn route(&self, workspace: Option<&str>) -> Result<Option<String>, String> {
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
    if let Some(switch) = lock(&shared.switch).clone() {
        made.stopped_by(switch);
    }
    if let Some(relayed) = lock(&shared.relayed).clone() {
        made.hear_through(relayed);
    }
    if let Some(ends) = lock(&shared.ends).clone() {
        made.when_one_ends(ends);
    }
    if let Some(reconnecting) = lock(&shared.reconnecting).clone() {
        made.when_reconnecting(reconnecting);
    }
    let dealt = shared.dealt.load(Ordering::Relaxed);
    if dealt > 0 {
        made.already_dealt(dealt);
    }
    let made: Arc<dyn SessionHost> = Arc::from(made);
    Ok(Arc::clone(
        lock(&shared.pools).entry(pool.to_owned()).or_insert(made),
    ))
}

fn fetch_back_all(shared: &Arc<Shared>, on_pools: &OnPools, to_a_pool: &ToAPool) -> Vec<String> {
    let mut failed = Vec::new();
    for (pool, workspaces) in on_pools() {
        let host = match pool_of(shared, to_a_pool, &pool) {
            Ok(host) => host,
            Err(why) => {
                failed.push(why);
                continue;
            }
        };
        for workspace in workspaces {
            failed.extend(host.fetch_back(&workspace, &pool));
        }
    }
    failed
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
        self.here.open(wanted, opening, announce)
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
        let host = self.pool(&pool)?;
        // This machine's host holds the project, and keeps its board, before a chat starts
        // beside it on a runner; and it deals the chat's number.
        self.here.hold_here();
        let number = wanted
            .filter(|wanted| *wanted > 0)
            .unwrap_or_else(|| self.here.deal());
        self.shared.dealt.fetch_max(number, Ordering::Relaxed);
        let composed = host.open_named(Some(number), named, hearing, announce)?;
        lock(&self.shared.chats).insert(composed.number, pool);
        Ok(composed)
    }

    fn composes_its_harness_chats(&self) -> bool {
        self.here.composes_its_harness_chats()
    }

    fn composes_its_harness_chats_in(&self, workspace: Option<&str>) -> bool {
        match self.route(workspace) {
            // A start whose pool is not kept is refused as it opens, in its own words.
            Ok(Some(_)) | Err(_) => true,
            Ok(None) => self.here.composes_its_harness_chats_in(workspace),
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
                let (to_a_pool, on_pools, root) =
                    (Arc::clone(&to_a_pool), Arc::clone(&on_pools), root.clone());
                // Off the thread that tells: a fetch crosses the link.
                std::thread::spawn(move || {
                    let workspaces = on_pools().remove(&pool).unwrap_or_default();
                    let Ok(host) = pool_of(&shared, &to_a_pool, &pool) else {
                        return;
                    };
                    for workspace in workspaces {
                        for why in host.fetch_back(&workspace, &pool) {
                            tracing::warn!("purlis: in {}: {why}", root.display());
                        }
                    }
                });
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

    /// A confinement the window planned is this machine's: a runner composes its own.
    fn confine(
        &self,
        plan: &purlis_core::sandbox::plan::Plan,
        hearing: purlis_core::sandbox::plan::Hearing,
    ) -> Result<purlis_core::sandbox::Confinement, String> {
        self.here.confine(plan, hearing)
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

    fn hear_through(&self, relayed: Arc<crate::hooks::Relayed>) {
        *lock(&self.shared.relayed) = Some(Arc::clone(&relayed));
        for pool in self.pools() {
            pool.hear_through(Arc::clone(&relayed));
        }
        self.here.hear_through(relayed);
    }

    fn hold_here(&self) {
        self.here.hold_here();
    }

    /// Asked of this machine's host, then of each pool a workspace of the project runs its
    /// chats on; one left running on a pool is kept as that pool's.
    fn left_running(&self, number: u32) -> bool {
        if self.here.left_running(number) {
            return true;
        }
        for pool in (self.on_pools)().into_keys() {
            let Ok(host) = self.pool(&pool) else {
                continue;
            };
            if host.left_running(number) {
                lock(&self.shared.chats).insert(number, pool);
                return true;
            }
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

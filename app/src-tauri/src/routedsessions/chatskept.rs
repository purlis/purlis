// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Where each runner chat runs, kept past this window** (#4590, ADR 0078 as amended
//! 2026-11-02): each record goes in this machine's store before the chat starts and leaves as it
//! ends, so a window started again asks that one pool for it, and its branches come back from
//! there whatever the settings say since.
//!
//! One thread writes them, in the order they happened: a chat's close is on the window's thread,
//! which never waits for the store (`INPUT_ORDER` in `costs.rs`), and an end that follows a start
//! at once never reaches the store first. A read waits for what was written before it.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::time::Duration;

use purlis_core::machine::ChatOnPool;

/// How long a read waits for the writes before it.
const SETTLED_WITHIN: Duration = Duration::from_secs(5);

/// What keeps the records: this machine's store, or a test's.
pub trait KeptPlaces: Send + Sync {
    fn keep(&self, number: u32, pool: &str, workspace: &str);
    /// A restart found it on one of the pool's runners, and nothing says which (#5393).
    fn keep_found(&self, number: u32, pool: &str, workspace: &str) {
        self.keep(number, pool, workspace);
    }
    /// It ran there and no longer does: never asked for again, its branches still to come back.
    fn leave(&self, number: u32);
    /// It never started there.
    fn forget(&self, number: u32);
    /// `workspace`'s branches of `pool` came back with no failure.
    fn fetched(&self, pool: &str, workspace: &str);
    fn chats(&self) -> BTreeMap<u32, ChatOnPool>;
    fn owed(&self) -> BTreeMap<String, BTreeSet<String>>;
    /// **Whether every chat the project starts on a pool here has a record by now** (ruled
    /// 2026-11-08): a restart has already taken back the chats from before records, so one with
    /// none is never taken back on a pool's word.
    fn all_recorded(&self) -> bool;
    /// A restart's take-back is done: [`Self::all_recorded`] from now on.
    fn mark_all_recorded(&self);
}

/// The records of the project at `plane`, in `config`'s store.
pub struct OnThisMachine {
    pub config: PathBuf,
    pub plane: PathBuf,
}

impl KeptPlaces for OnThisMachine {
    fn keep(&self, number: u32, pool: &str, workspace: &str) {
        let kept = purlis_core::pools::keep_chat_on_pool(
            &self.config,
            &self.plane,
            number,
            pool,
            workspace,
        );
        if let Err(why) = kept {
            tracing::warn!("purlis: where chat {number} runs was not kept: {why}");
        }
    }

    fn keep_found(&self, number: u32, pool: &str, workspace: &str) {
        let kept = purlis_core::pools::keep_chat_found_on_pool(
            &self.config,
            &self.plane,
            number,
            pool,
            workspace,
        );
        if let Err(why) = kept {
            tracing::warn!("purlis: where chat {number} runs was not kept: {why}");
        }
    }

    fn leave(&self, number: u32) {
        let left = purlis_core::pools::leave_chat_on_pool(&self.config, &self.plane, number);
        if let Err(why) = left {
            tracing::warn!("purlis: where chat {number} ran stays kept: {why}");
        }
    }

    fn forget(&self, number: u32) {
        let forgotten = purlis_core::pools::forget_chat_on_pool(&self.config, &self.plane, number);
        if let Err(why) = forgotten {
            tracing::warn!("purlis: where chat {number} ran stays kept: {why}");
        }
    }

    fn fetched(&self, pool: &str, workspace: &str) {
        let fetched =
            purlis_core::pools::branches_fetched(&self.config, &self.plane, pool, workspace);
        if let Err(why) = fetched {
            tracing::warn!("purlis: {workspace}'s branches of {pool} stay owed: {why}");
        }
    }

    fn chats(&self) -> BTreeMap<u32, ChatOnPool> {
        purlis_core::pools::chats_on_pools(&self.config, &self.plane)
    }

    fn owed(&self) -> BTreeMap<String, BTreeSet<String>> {
        purlis_core::pools::branches_to_fetch(&self.config, &self.plane)
    }

    fn all_recorded(&self) -> bool {
        purlis_core::pools::pool_chats_recorded(&self.config, &self.plane)
    }

    fn mark_all_recorded(&self) {
        let marked = purlis_core::pools::mark_pool_chats_recorded(&self.config, &self.plane);
        if let Err(why) = marked {
            tracing::warn!("purlis: that every runner chat has a record was not kept: {why}");
        }
    }
}

enum Step {
    Keep {
        number: u32,
        pool: String,
        workspace: String,
    },
    KeepFound {
        number: u32,
        pool: String,
        workspace: String,
    },
    Leave(u32),
    Forget(u32),
    Fetched {
        pool: String,
        workspace: String,
    },
    AllRecorded,
    Settled(Sender<()>),
}

/// The project's records, or none for a router with no store.
#[derive(Clone, Default)]
pub struct ChatsKept(Option<Keeping>);

#[derive(Clone)]
struct Keeping {
    places: Arc<dyn KeptPlaces>,
    steps: Sender<Step>,
    /// How many chats have left their records so far, for what a fetch settles.
    leaves: Arc<AtomicU64>,
}

impl ChatsKept {
    /// The records `places` keeps, written on a thread of their own.
    pub fn in_places(places: Arc<dyn KeptPlaces>) -> ChatsKept {
        let (steps, heard) = mpsc::channel();
        let writes = Arc::clone(&places);
        let started = std::thread::Builder::new()
            .name("purlis-chats-kept".into())
            .spawn(move || {
                while let Ok(step) = heard.recv() {
                    match step {
                        Step::Keep {
                            number,
                            pool,
                            workspace,
                        } => writes.keep(number, &pool, &workspace),
                        Step::KeepFound {
                            number,
                            pool,
                            workspace,
                        } => writes.keep_found(number, &pool, &workspace),
                        Step::Leave(number) => writes.leave(number),
                        Step::Forget(number) => writes.forget(number),
                        Step::Fetched { pool, workspace } => writes.fetched(&pool, &workspace),
                        Step::AllRecorded => writes.mark_all_recorded(),
                        Step::Settled(said) => {
                            let _ = said.send(());
                        }
                    }
                }
            });
        if let Err(why) = started {
            tracing::warn!("purlis: where each runner chat runs will not be kept: {why}");
            return ChatsKept::default();
        }
        ChatsKept(Some(Keeping {
            places,
            steps,
            leaves: Arc::default(),
        }))
    }

    fn step(&self, step: Step) {
        if let Some(keeping) = &self.0 {
            let _ = keeping.steps.send(step);
        }
    }

    pub fn keep(&self, number: u32, pool: &str, workspace: &str) {
        self.step(Step::Keep {
            number,
            pool: pool.to_owned(),
            workspace: workspace.to_owned(),
        });
    }

    pub fn keep_found(&self, number: u32, pool: &str, workspace: &str) {
        self.step(Step::KeepFound {
            number,
            pool: pool.to_owned(),
            workspace: workspace.to_owned(),
        });
    }

    pub fn leave(&self, number: u32) {
        if let Some(keeping) = &self.0 {
            keeping.leaves.fetch_add(1, Ordering::SeqCst);
        }
        self.step(Step::Leave(number));
    }

    /// How many chats have left their records so far (#5393).
    pub fn leaves(&self) -> u64 {
        self.0
            .as_ref()
            .map_or(0, |keeping| keeping.leaves.load(Ordering::SeqCst))
    }

    pub fn forget(&self, number: u32) {
        self.step(Step::Forget(number));
    }

    pub fn fetched(&self, pool: &str, workspace: &str) {
        self.step(Step::Fetched {
            pool: pool.to_owned(),
            workspace: workspace.to_owned(),
        });
    }

    /// A restart's take-back is done ([`KeptPlaces::mark_all_recorded`]).
    pub fn mark_all_recorded(&self) {
        self.step(Step::AllRecorded);
    }

    /// Whether every chat on a pool has a record now ([`KeptPlaces::all_recorded`]): `false`
    /// for a router with no store, which keeps none.
    pub fn all_recorded(&self) -> bool {
        self.settled();
        self.0
            .as_ref()
            .is_some_and(|keeping| keeping.places.all_recorded())
    }

    /// Once what was asked before this is in the store.
    pub fn settled(&self) {
        if self.0.is_some() {
            let (said, heard) = mpsc::channel();
            self.step(Step::Settled(said));
            let _ = heard.recv_timeout(SETTLED_WITHIN);
        }
    }

    /// Every chat kept on a pool now, by its number.
    pub fn chats(&self) -> BTreeMap<u32, ChatOnPool> {
        self.settled();
        self.0
            .as_ref()
            .map(|keeping| keeping.places.chats())
            .unwrap_or_default()
    }

    pub fn of(&self, number: u32) -> Option<ChatOnPool> {
        self.chats().remove(&number)
    }

    /// The workspaces with branches to bring back from each pool: those owed, and those of the
    /// chats that run there now.
    pub fn to_fetch(&self) -> BTreeMap<String, BTreeSet<String>> {
        let chats = self.chats();
        let mut all = self
            .0
            .as_ref()
            .map(|keeping| keeping.places.owed())
            .unwrap_or_default();
        for chat in chats.into_values() {
            all.entry(chat.pool).or_default().insert(chat.workspace);
        }
        all
    }
}

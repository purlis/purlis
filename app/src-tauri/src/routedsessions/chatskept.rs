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
use std::sync::mpsc::{self, Sender};
use std::time::Duration;

use purlis_core::machine::ChatOnPool;

/// How long a read waits for the writes before it.
const SETTLED_WITHIN: Duration = Duration::from_secs(5);

/// What keeps the records: this machine's store, or a test's.
pub trait KeptPlaces: Send + Sync {
    fn keep(&self, number: u32, pool: &str, workspace: &str);
    /// It ran there and no longer does: never asked for again, its branches still to come back.
    fn leave(&self, number: u32);
    /// It never started there.
    fn forget(&self, number: u32);
    /// `workspace`'s branches of `pool` came back with no failure.
    fn fetched(&self, pool: &str, workspace: &str);
    fn chats(&self) -> BTreeMap<u32, ChatOnPool>;
    fn owed(&self) -> BTreeMap<String, BTreeSet<String>>;
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
}

enum Step {
    Keep {
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
    Settled(Sender<()>),
}

/// The project's records, or none for a router with no store.
#[derive(Clone, Default)]
pub struct ChatsKept(Option<Keeping>);

#[derive(Clone)]
struct Keeping {
    places: Arc<dyn KeptPlaces>,
    steps: Sender<Step>,
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
                        Step::Leave(number) => writes.leave(number),
                        Step::Forget(number) => writes.forget(number),
                        Step::Fetched { pool, workspace } => writes.fetched(&pool, &workspace),
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
        ChatsKept(Some(Keeping { places, steps }))
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

    pub fn leave(&self, number: u32) {
        self.step(Step::Leave(number));
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

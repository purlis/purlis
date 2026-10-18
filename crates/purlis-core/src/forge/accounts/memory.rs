// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **This process's memory of its forge sign-ins: each item is asked about once per process**
//! (#2656, ADR 0070 as amended 2026-10-18).
//!
//! On macOS every call to the Keychain about an item this build did not make can ask the person
//! for their password: a read of the item's value, and a search for whether it is there. Each
//! forge caller used to make its own (every listing searched, every request read), so a start
//! with a few projects asked several times for one item. [`Remembering`] sits in front of the
//! store [`super::keyring_at`] hands out and keeps, for the life of this process, **the value
//! of each item it read or wrote** (for a browser sign-in, the access token, the refresh token
//! and the expiry), wiped when it is dropped. One call answers both questions:
//!
//! - **A read is answered from memory** once the value is held.
//! - **Whether an item is there is answered by a read**, where the store would otherwise search
//!   for it (macOS, and the fenced stub that stands in for it): the value it reads is kept, so
//!   the listing and the request that follows it ask the Keychain once between them. Elsewhere
//!   the store says it cannot tell, as before, and nothing is read for it.
//!
//! **Only this process holds it.** It is never written anywhere, never logged, and never handed
//! to another process: a second process (the session host, git's credential helper) keeps its
//! own and asks once itself. Whatever purlis writes or deletes through this store updates it
//! first-hand. That an item is gone is never remembered: a read that finds nothing is asked
//! again next time, and so is one that failed.
//!
//! **One call at a time.** The memory's lock is held across the store's own call, so a read
//! racing a delete or a write never puts back a value that is gone or replaced, and two first
//! reads at once make one call, not two questions.
//!
//! **What it does not see** is a change made outside this process while it runs: an item
//! deleted in Keychain Access is still used until the process ends (the forge still checks the
//! token). Callers that must see another process's write read afresh ([`Store::forget`], then
//! a read): a renewal and a sign-out do, and a token the forge refuses is forgotten.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use zeroize::Zeroizing;

use crate::secrets::VaultError;
use crate::secrets::keyring::{Held, Presence, Secret, Store};

/// Each item's value this process read or wrote, by `(service, account)`.
type Items = BTreeMap<(String, String), Zeroizing<String>>;

/// A store, and what this process learnt from it. Cheap to clone: every clone shares the memory.
#[derive(Clone)]
pub(super) struct Remembering {
    inner: Arc<dyn Store>,
    items: Arc<Mutex<Items>>,
    /// Whether "is it there" is answered by a read rather than the store's own search.
    reads_for_presence: bool,
}

impl std::fmt::Debug for Remembering {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Remembering(***)")
    }
}

/// Every store this process remembers for, by the place it keeps its items: one for the
/// system's keyring, and in a fenced build one per stub.
static PROCESS: OnceLock<Mutex<BTreeMap<PathBuf, Remembering>>> = OnceLock::new();

fn key(service: &str, account: &str) -> (String, String) {
    (service.to_owned(), account.to_owned())
}

impl Remembering {
    /// This process's memory of the store kept at `place`, made with `inner` the first time.
    /// `reads_for_presence` where the store's own answer to "is it there" is a question of its
    /// own the person may be asked.
    pub(super) fn of(
        place: &Path,
        reads_for_presence: bool,
        inner: impl FnOnce() -> Box<dyn Store>,
    ) -> Remembering {
        let all = PROCESS.get_or_init(Mutex::default);
        let mut all = all.lock().unwrap_or_else(|e| e.into_inner());
        all.entry(place.to_path_buf())
            .or_insert_with(|| Remembering::over(Arc::from(inner()), reads_for_presence))
            .clone()
    }

    /// A memory of its own over `inner`: what [`Remembering::of`] makes once per place.
    pub(super) fn over(inner: Arc<dyn Store>, reads_for_presence: bool) -> Remembering {
        Remembering {
            inner,
            items: Arc::default(),
            reads_for_presence,
        }
    }

    fn items(&self) -> MutexGuard<'_, Items> {
        self.items.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The value, from memory or else read, with the lock already held.
    fn read(
        &self,
        items: &mut Items,
        service: &str,
        account: &str,
    ) -> Result<Option<Secret>, VaultError> {
        if let Some(value) = items.get(&key(service, account)) {
            return Ok(Some(Secret::new(String::clone(value))));
        }
        let Some(secret) = self.inner.get(service, account)? else {
            return Ok(None);
        };
        let value = secret.into_inner();
        items.insert(key(service, account), Zeroizing::new(value.clone()));
        Ok(Some(Secret::new(value)))
    }

    /// One write through `write`, remembered when it took and forgotten when it did not.
    fn written(
        &self,
        service: &str,
        account: &str,
        value: &str,
        write: impl FnOnce(&dyn Store) -> Result<Held, VaultError>,
    ) -> Result<Held, VaultError> {
        let mut items = self.items();
        let held = write(self.inner.as_ref());
        if held.is_ok() {
            items.insert(key(service, account), Zeroizing::new(value.to_owned()));
        } else {
            items.remove(&key(service, account));
        }
        held
    }
}

impl Store for Remembering {
    fn get(&self, service: &str, account: &str) -> Result<Option<Secret>, VaultError> {
        let mut items = self.items();
        self.read(&mut items, service, account)
    }

    fn set(&self, service: &str, account: &str, value: &str) -> Result<Held, VaultError> {
        self.written(service, account, value, |store| {
            store.set(service, account, value)
        })
    }

    fn delete(&self, service: &str, account: &str) -> Result<bool, VaultError> {
        let mut items = self.items();
        items.remove(&key(service, account));
        self.inner.delete(service, account)
    }

    fn exists(&self, service: &str, account: &str) -> Presence {
        let mut items = self.items();
        if items.contains_key(&key(service, account)) {
            return Presence::Present;
        }
        if !self.reads_for_presence {
            return self.inner.exists(service, account);
        }
        match self.read(&mut items, service, account) {
            Ok(Some(_)) => Presence::Present,
            Ok(None) => Presence::Absent,
            Err(_) => Presence::Unknown,
        }
    }

    fn holds(&self) -> bool {
        self.inner.holds()
    }

    fn rehold(&self, service: &str, account: &str, value: &str) -> Result<Held, VaultError> {
        self.written(service, account, value, |store| {
            store.rehold(service, account, value)
        })
    }

    fn make_fresh(&self, service: &str, account: &str, value: &str) -> Result<Held, VaultError> {
        self.written(service, account, value, |store| {
            store.make_fresh(service, account, value)
        })
    }

    fn make_own(&self, service: &str, account: &str, value: &str) -> Result<Held, VaultError> {
        self.written(service, account, value, |store| {
            store.make_own(service, account, value)
        })
    }

    fn forget(&self, service: &str, account: &str) {
        self.items().remove(&key(service, account));
    }
}

#[cfg(test)]
mod tests;

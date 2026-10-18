// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **This process's memory of its forge sign-ins: each item is asked about once per process**
//! (#2656, ADR 0070 as amended 2026-10-18).
//!
//! On macOS every call to the Keychain about an item this build did not make can ask the person
//! for their password: a read of the item's value, and a search for whether it is there. Each
//! forge caller used to make its own (every listing searched, every request read), so a start
//! with a few projects asked several times for one item. [`Remembering`] sits in front of the
//! store [`super::keyring_at`] hands out and keeps, for the life of this process:
//!
//! - **a value it read or wrote**, wiped when it is dropped, so the next read is answered here;
//! - **that an item is there**, once a search found it or a value is held, so a listing asks
//!   once.
//!
//! **Only this process holds it.** It is never written anywhere, never logged, and never handed
//! to another process: a second process (the session host, git's credential helper) keeps its
//! own and asks once itself. Whatever purlis writes or deletes through this store updates it
//! first-hand. That an item is gone is never remembered: a search or read that finds nothing is
//! asked again next time, and so is one that failed.
//!
//! **What it does not see** is a change made outside purlis while the process runs: an item
//! deleted in Keychain Access is still used until the process ends (the forge still checks the
//! token), and another process's renewal is seen when the remembered token is due for renewal,
//! since [`super::super::refresh::Refresher`] then forgets it and reads afresh before it renews.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use zeroize::Zeroizing;

use crate::secrets::VaultError;
use crate::secrets::keyring::{Held, Presence, Secret, Store};

/// What is known of one item.
enum Known {
    /// Its value, read or written by this process.
    Value(Zeroizing<String>),
    /// That it is there, from a search that read no value.
    There,
}

type Items = BTreeMap<(String, String), Known>;

/// A store, and what this process learnt from it. Cheap to clone: every clone shares the memory.
#[derive(Clone)]
pub(super) struct Remembering {
    inner: Arc<dyn Store>,
    items: Arc<Mutex<Items>>,
}

impl std::fmt::Debug for Remembering {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Remembering(***)")
    }
}

/// Every store this process remembers for, by the place it keeps its items: one for the
/// system's keyring, and in a fenced build one per stub.
static PROCESS: OnceLock<Mutex<BTreeMap<PathBuf, Remembering>>> = OnceLock::new();

impl Remembering {
    /// This process's memory of the store kept at `place`, made with `inner` the first time.
    pub(super) fn of(place: &Path, inner: impl FnOnce() -> Box<dyn Store>) -> Remembering {
        let all = PROCESS.get_or_init(Mutex::default);
        let mut all = all.lock().unwrap_or_else(|e| e.into_inner());
        all.entry(place.to_path_buf())
            .or_insert_with(|| Remembering {
                inner: Arc::from(inner()),
                items: Arc::default(),
            })
            .clone()
    }

    fn items(&self) -> std::sync::MutexGuard<'_, Items> {
        self.items.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn key(service: &str, account: &str) -> (String, String) {
        (service.to_owned(), account.to_owned())
    }

    fn keep(&self, service: &str, account: &str, value: &str) {
        self.items().insert(
            Self::key(service, account),
            Known::Value(Zeroizing::new(value.to_owned())),
        );
    }

    fn written(&self, service: &str, account: &str, value: &str, held: &Result<Held, VaultError>) {
        if held.is_ok() {
            self.keep(service, account, value);
        } else {
            self.forget(service, account);
        }
    }
}

impl Store for Remembering {
    fn get(&self, service: &str, account: &str) -> Result<Option<Secret>, VaultError> {
        if let Some(Known::Value(value)) = self.items().get(&Self::key(service, account)) {
            return Ok(Some(Secret::new(String::clone(value))));
        }
        let Some(secret) = self.inner.get(service, account)? else {
            self.forget(service, account);
            return Ok(None);
        };
        let value = secret.into_inner();
        self.keep(service, account, &value);
        Ok(Some(Secret::new(value)))
    }

    fn set(&self, service: &str, account: &str, value: &str) -> Result<Held, VaultError> {
        let held = self.inner.set(service, account, value);
        self.written(service, account, value, &held);
        held
    }

    fn delete(&self, service: &str, account: &str) -> Result<bool, VaultError> {
        self.forget(service, account);
        self.inner.delete(service, account)
    }

    fn exists(&self, service: &str, account: &str) -> Presence {
        if self.items().contains_key(&Self::key(service, account)) {
            return Presence::Present;
        }
        let presence = self.inner.exists(service, account);
        if presence == Presence::Present {
            self.items()
                .entry(Self::key(service, account))
                .or_insert(Known::There);
        }
        presence
    }

    fn holds(&self) -> bool {
        self.inner.holds()
    }

    fn rehold(&self, service: &str, account: &str, value: &str) -> Result<Held, VaultError> {
        let held = self.inner.rehold(service, account, value);
        self.written(service, account, value, &held);
        held
    }

    fn make_fresh(&self, service: &str, account: &str, value: &str) -> Result<Held, VaultError> {
        let held = self.inner.make_fresh(service, account, value);
        self.written(service, account, value, &held);
        held
    }

    fn make_own(&self, service: &str, account: &str, value: &str) -> Result<Held, VaultError> {
        let held = self.inner.make_own(service, account, value);
        self.written(service, account, value, &held);
        held
    }

    fn forget(&self, service: &str, account: &str) {
        self.items().remove(&Self::key(service, account));
    }
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Driving a browser's pages: the agent's view of a page and its actions (#2010).

mod blank;
mod keys;
mod ledger;
mod settle;
mod tree;

use std::sync::{Mutex, MutexGuard};

/// The id a call carries, so that a retry after a lost answer never acts twice.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CallId(String);

/// Why a call's id was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("a call's id is between 1 and {MAX_CALL_ID} characters")]
pub struct NotCallId;

/// The longest call id, in characters.
pub const MAX_CALL_ID: usize = 128;

impl CallId {
    /// `id`, when it is between 1 and [`MAX_CALL_ID`] characters.
    pub fn new(id: impl Into<String>) -> Result<Self, NotCallId> {
        let id = id.into();
        if id.is_empty() || id.chars().count() > MAX_CALL_ID {
            return Err(NotCallId);
        }
        Ok(Self(id))
    }

    /// The id as given.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn lock<T: ?Sized>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

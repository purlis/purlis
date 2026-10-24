// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A test's way into [`super::move_between`] at the instants a swap would land: after the look at
//! the leaf and before its rename (the put-back's rename too), and after the rename and before
//! the look that proves it.
//!
//! Thread-local, so it fires only in the test that set it: `cargo test` runs tests on a pool.

use std::cell::RefCell;
use std::path::Path;

/// When the hook runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Moment {
    /// Before a rename, the move's or its put-back's, of `from` to `to`.
    BeforeRename,
    /// After the move's rename of `from` to `to`, before the look at `to`.
    AfterRename,
}

type Hook = Box<dyn FnMut(Moment, &Path, &Path)>;

thread_local! {
    static HOOK: RefCell<Option<Hook>> = const { RefCell::new(None) };
}

/// Run `f(moment, from, to)` at each [`Moment`] of every [`super::move_between`] on this thread
/// until the returned guard drops.
pub(crate) fn set(f: impl FnMut(Moment, &Path, &Path) + 'static) -> Unset {
    HOOK.with(|h| *h.borrow_mut() = Some(Box::new(f)));
    Unset
}

pub(crate) struct Unset;

impl Drop for Unset {
    fn drop(&mut self) {
        HOOK.with(|h| *h.borrow_mut() = None);
    }
}

pub(crate) fn at(moment: Moment, from: &Path, to: &Path) {
    let taken = HOOK.with(|h| h.borrow_mut().take());
    let Some(mut f) = taken else {
        return;
    };
    f(moment, from, to);
    HOOK.with(|h| {
        let mut slot = h.borrow_mut();
        if slot.is_none() {
            *slot = Some(f);
        }
    });
}

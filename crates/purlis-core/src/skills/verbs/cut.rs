// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A test's way into a delete by git at the instants a crash would cut it short (SK-35, #3013):
//! after its `git rm`, after the removal is taken out of the index again, and after its commit,
//! each before the note that it runs is taken away.
//!
//! Thread-local, so it fires only in the test that set it: `cargo test` runs tests on a pool.

use std::cell::RefCell;

/// Where a delete by git is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::skills) enum Step {
    /// The skill's files are removed and the removal staged.
    Removed,
    /// The removal is taken out of the index again, for the save to stage.
    Unstaged,
    /// The save ran: committed, or put back where it could not commit.
    Committed,
}

type Hook = Box<dyn FnMut(Step)>;

thread_local! {
    static HOOK: RefCell<Option<Hook>> = const { RefCell::new(None) };
}

/// Run `f(step)` at each [`Step`] of every delete by git on this thread until the returned
/// guard drops. A panic in `f` is the crash: nothing after it in the delete runs.
pub(in crate::skills) fn set(f: impl FnMut(Step) + 'static) -> Unset {
    HOOK.with(|h| *h.borrow_mut() = Some(Box::new(f)));
    Unset
}

pub(in crate::skills) struct Unset;

impl Drop for Unset {
    fn drop(&mut self) {
        HOOK.with(|h| *h.borrow_mut() = None);
    }
}

pub(in crate::skills) fn at(step: Step) {
    let taken = HOOK.with(|h| h.borrow_mut().take());
    let Some(mut f) = taken else {
        return;
    };
    f(step);
    HOOK.with(|h| {
        let mut slot = h.borrow_mut();
        if slot.is_none() {
            *slot = Some(f);
        }
    });
}

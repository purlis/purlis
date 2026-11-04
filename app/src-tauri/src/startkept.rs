// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The new-chat picker's two answers, kept per project** (#4737).
//!
//! The dialog opens on its first answer and fills its rows in from its second (#4736), and the
//! second costs a sandbox compile per harness and a check of each approved profile's program.
//! Both are kept once read, so the dialog opens again with neither, and they are dropped on
//! what moves them:
//!
//! - **the plane watch**, on a change to the profiles, the personas, the project's sandbox or
//!   the harness settings ([`purlis_core::planechange::moves_a_new_chat`]), on a change it
//!   cannot place, and on a watch that lapsed and is whole again;
//! - **the stamp they were kept with**, for what they were read from on this machine and not in
//!   the plane's watched folders: the sandbox policy, the profile store, the approvals, the
//!   installed harnesses ([`purlis_core::pickerstamp`]).
//!
//! A project whose plane is not watched, or not wholly, keeps nothing: each ask reads both
//! answers fresh, as the picker did before.
//!
//! A read that began before a drop is answered but not kept: it may have read what the drop was
//! about from before it moved.

use std::sync::{Mutex, PoisonError};

use purlis_core::pickerstamp::Stamp;

/// One answer, kept with the stamp it was read under.
pub struct Kept<T> {
    slot: Mutex<Slot<T>>,
}

struct Slot<T> {
    /// How many times the answer was dropped: a read that began under another generation is
    /// not kept.
    generation: u64,
    kept: Option<(Stamp, T)>,
}

impl<T> Default for Kept<T> {
    fn default() -> Self {
        Self {
            slot: Mutex::new(Slot {
                generation: 0,
                kept: None,
            }),
        }
    }
}

impl<T: Clone> Kept<T> {
    /// Drops the kept answer, and any read under way.
    pub fn moved(&self) {
        let mut slot = self.lock();
        slot.generation = slot.generation.wrapping_add(1);
        slot.kept = None;
    }

    /// The kept answer where `trusted` and its stamp holds; else what `read` answers, kept where
    /// `trusted` and nothing dropped it while it was read. `read` runs with no lock held.
    pub fn served<E>(
        &self,
        trusted: bool,
        read: impl FnOnce() -> Result<(T, Stamp), E>,
    ) -> Result<T, E> {
        let began = {
            let mut slot = self.lock();
            if !trusted {
                slot.kept = None;
            } else if let Some((stamp, answer)) = &slot.kept {
                if stamp.holds() {
                    return Ok(answer.clone());
                }
                slot.kept = None;
            }
            slot.generation
        };
        let (answer, stamp) = read()?;
        if trusted {
            let mut slot = self.lock();
            if slot.generation == began {
                slot.kept = Some((stamp, answer.clone()));
            }
        }
        Ok(answer)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Slot<T>> {
        self.slot.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Both of a project's picker answers.
#[derive(Default)]
pub struct StartAnswers {
    /// The first answer ([`crate::start_options_first`]), without the next chat's number, which
    /// the plane's chats say on each ask.
    pub first: Kept<crate::StartOptions>,
    /// The second ([`crate::start_details`]).
    pub details: Kept<Vec<crate::RowDetails>>,
}

impl StartAnswers {
    /// Drops both.
    pub fn moved(&self) {
        self.first.moved();
        self.details.moved();
    }
}

#[cfg(test)]
mod tests;

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What a runner chat asks to write in the project arrives as a proposal** (#2046, ADR 0078
//! §7, RR-17): persona, workspace and memory changes made on a pool's runner reach the project
//! on this machine only once the person applies them.
//!
//! A chat on a runner writes the project's own files the way a sandboxed chat here does: it asks
//! the window (a brokered write, `crate::brokered`), and its line comes back over the link its
//! host relays. **Here such a write is held, never made**: the person sees what it would do, in
//! words ([`purlis_core::runnerwrite::said`]), in the chat's tab and the Inbox, with **Apply**
//! and **Dismiss**. Apply makes exactly the write held, as the window would have made it for a
//! chat here, for the chat as the app records it (its workspace and persona, never the line's);
//! Dismiss makes nothing. The chat is told which on its next turn. There is no "always apply".
//!
//! In memory only, as proposed changes are: a relaunch starts with none, and a chat's held
//! writes end with it.

use std::sync::{Mutex, PoisonError};

use purlis_core::brokered::{Asker, Write};
use purlis_core::dispatchtalk::ProposalWord;
use purlis_core::hookwire::Answer;

use crate::planes::{Held, PlaneId, Planes};

/// The most writes one chat has held at once: one more is refused, in words.
pub const AT_MOST_PER_CHAT: usize = 16;

/// What the chat is told once its write is held.
pub const HELD_SAID: &str = "this chat runs on a pool's runner, so its change to the project \
     was shown to the person as a proposal: only they apply it, in the window. Wait for their \
     answer, which comes back to this chat as applied or dismissed.";

/// Why an answer found nothing to answer.
const WAITS_NO_MORE: &str = "That proposal waits no more: it was answered already, or its chat \
     closed. Nothing was written.";

/// One write held for the person.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeldWrite {
    pub id: u32,
    pub session: u32,
    /// The pool the chat runs on.
    pub pool: String,
    /// The chat as the app records it, which the write is made for.
    pub asker: Asker,
    pub write: Write,
    /// What it would do, in words.
    pub said: String,
    /// When it began to wait, in seconds since 1970.
    pub since: u64,
}

/// A project's held writes, until the person answers.
#[derive(Debug, Default)]
pub struct RunnerWrites {
    held: Mutex<(u32, Vec<HeldWrite>)>,
}

impl RunnerWrites {
    fn lock(&self) -> std::sync::MutexGuard<'_, (u32, Vec<HeldWrite>)> {
        self.held.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Holds `write` of chat `asker.chat`, on `pool`: its id, or none where the chat has
    /// [`AT_MOST_PER_CHAT`] held already.
    pub fn note(&self, pool: &str, asker: Asker, write: Write, now: u64) -> Option<u32> {
        let mut held = self.lock();
        let (next, list) = &mut *held;
        if list.iter().filter(|one| one.session == asker.chat).count() >= AT_MOST_PER_CHAT {
            return None;
        }
        *next = next.wrapping_add(1).max(1);
        let id = *next;
        list.push(HeldWrite {
            id,
            session: asker.chat,
            pool: pool.to_owned(),
            said: purlis_core::runnerwrite::said(&write),
            asker,
            write,
            since: now,
        });
        Some(id)
    }

    /// Every write held, oldest first.
    pub fn every(&self) -> Vec<HeldWrite> {
        self.lock().1.clone()
    }

    /// Takes write `id` out: it waits no more.
    pub fn take(&self, id: u32) -> Option<HeldWrite> {
        let mut held = self.lock();
        let at = held.1.iter().position(|one| one.id == id)?;
        Some(held.1.remove(at))
    }

    /// Chat `session` has closed: its held writes go with it.
    pub fn forget(&self, session: u32) {
        self.lock().1.retain(|one| one.session != session);
    }
}

/// **Holds `write` of the runner chat `asker` for the person**, and answers what the chat is
/// told. Nothing is written.
pub fn hold(held: &Held, pool: &str, asker: Asker, write: Write) -> Answer {
    if let Err(why) = write.check() {
        return Answer::No { why };
    }
    match held
        .runner_writes()
        .note(pool, asker, write, crate::sandboxing::now_secs())
    {
        Some(_) => {
            held.chats().asks_may_have_moved();
            Answer::Said {
                lines: vec![purlis_core::repocmd::Say::Done(HELD_SAID.to_owned())],
                code: 0,
            }
        }
        None => Answer::No {
            why: format!(
                "this chat has {AT_MOST_PER_CHAT} changes waiting for the person already: wait \
                 for their answers before asking for another"
            ),
        },
    }
}

/// **Apply** on a held write: made now, exactly as held, for the chat as the app records it.
/// Window-only.
#[tauri::command]
#[specta::specta]
pub async fn apply_runner_write(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    id: u32,
) -> Result<String, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("applying a runner chat's change", move || {
        applied(&held, id)
    })
    .await
}

/// [`apply_runner_write`]'s work, for project `held`.
pub fn applied(held: &Held, id: u32) -> Result<String, String> {
    let made = apply_in(
        held.root(),
        held.runner_writes(),
        id,
        chrono::Local::now().naive_local(),
    );
    held.chats().asks_may_have_moved();
    let one = made?;
    let said = format!(
        "Applied the change of chat {} on pool {}.",
        one.session, one.pool
    );
    tell(held, one.session, ProposalWord::Applied { path: one.said });
    Ok(said)
}

/// **Makes held write `id` of `writes`** in the project at `root`, at `now`: taken out, and
/// made as the window makes a chat's brokered write here, for the chat as the app recorded it.
/// The write made, or why not; one that was refused waits no more.
pub fn apply_in(
    root: &std::path::Path,
    writes: &RunnerWrites,
    id: u32,
    now: chrono::NaiveDateTime,
) -> Result<HeldWrite, String> {
    let one = writes.take(id).ok_or(WAITS_NO_MORE)?;
    match crate::brokered::answered(root, &one.asker, &one.write, now) {
        Answer::No { why } => Err(why),
        _ => Ok(one),
    }
}

/// **Dismiss** on a held write: nothing is made. Window-only.
#[tauri::command]
#[specta::specta]
pub async fn dismiss_runner_write(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    id: u32,
) -> Result<(), String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("dismissing a runner chat's change", move || {
        dismissed(&held, id)
    })
    .await
}

/// [`dismiss_runner_write`]'s work, for project `held`.
pub fn dismissed(held: &Held, id: u32) -> Result<(), String> {
    let one = held.runner_writes().take(id).ok_or(WAITS_NO_MORE)?;
    tell(
        held,
        one.session,
        ProposalWord::Dismissed { path: one.said },
    );
    held.chats().asks_may_have_moved();
    Ok(())
}

/// Tells chat `session` `word` on its next turn.
fn tell(held: &Held, session: u32, word: ProposalWord) {
    held.tasks().ledger().talk.proposal_word(session, word);
}

#[cfg(test)]
mod tests;

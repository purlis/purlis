// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What a runner chat asks to write in the project arrives as a proposal** (#2046, ADR 0078
//! §7, RR-17): persona, workspace and memory changes made on a pool's runner reach the project
//! on this machine only once the person applies them.
//!
//! A chat on a runner writes the project's own files the way a sandboxed chat here does: it asks
//! the window (a brokered write, `crate::brokered`), and its line comes back over the link its
//! host relays. **Here such a write is held, never made**: the person sees what it would do, in
//! words ([`purlis_core::runnerwrite::said`]), in the chat's tab and the Inbox. **Apply** is
//! offered there only where that line is the whole write, as it would be written; otherwise
//! **Open** shows it whole in a tab of its own, with Apply there where every text it carries is
//! drawn as it is. **Dismiss** is always offered. Apply makes exactly the write held (its texts
//! trimmed, as they are shown), as the window would have made it for a chat here, for the chat
//! as the app records it (its workspace and persona, never the line's); Dismiss makes nothing.
//! The chat is told which on its next turn, and told where an Apply could not be made. There is
//! no "always apply". A write that carries what looks like a credential is never held.
//!
//! In memory only, as proposed changes are: a relaunch starts with none, which the chat is told
//! as its write is held ([`HELD_SAID`]). A chat's held writes end with it.

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
     answer, which comes back to this chat as applied or dismissed. A restart of the window \
     drops it, and nothing of it is written then: ask again for what is still wanted.";

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
    /// The write as it is held: its texts trimmed, as they are shown.
    pub write: Write,
    /// What it would do, in words, each text held to its most.
    pub said: String,
    /// What it would do, every text whole: what its own tab shows.
    pub whole: String,
    /// What it is, in purlis's words alone: what the chat is told of it.
    pub subject: String,
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
            whole: purlis_core::runnerwrite::said_whole(&write),
            subject: purlis_core::runnerwrite::subject(&write),
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
    let write = purlis_core::runnerwrite::held_as(write);
    if let Err(why) = write.check() {
        return Answer::No { why };
    }
    if let Some(why) = purlis_core::runnerwrite::refusal(&write) {
        return Answer::No {
            why: why.to_owned(),
        };
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

/// **Apply** on a held write, pressed `from` a row or its own tab: made now, exactly as held,
/// for the chat as the app records it, only where it was shown whole there. Window-only.
#[tauri::command]
#[specta::specta]
pub async fn apply_runner_write(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    id: u32,
    from: crate::proposals::From,
) -> Result<String, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("applying a runner chat's change", move || {
        applied(&held, id, from)
    })
    .await
}

/// What Apply says where it was pressed where the write is not shown whole.
pub const NOT_SHOWN_WHOLE: &str = "That change is not shown whole there, so it was not applied: \
     open it to see it whole.";

/// Whether held write `one` is shown whole where Apply was pressed `from`: in a row, its one
/// line is the whole write ([`crate::asking::runner_write_shown_whole`]); in its own tab, every
/// text it carries is drawn as it is.
pub fn shown_whole(one: &HeldWrite, from: crate::proposals::From) -> bool {
    match from {
        crate::proposals::From::Row => crate::asking::runner_write_shown_whole(one),
        crate::proposals::From::Tab => purlis_core::runnerwrite::drawn_as_it_is(&one.write),
    }
}

/// [`apply_runner_write`]'s work, for project `held`.
pub fn applied(held: &Held, id: u32, from: crate::proposals::From) -> Result<String, String> {
    let made = apply_in(
        held.root(),
        held.runner_writes(),
        id,
        from,
        chrono::Local::now().naive_local(),
    );
    held.chats().asks_may_have_moved();
    match made {
        Ok(one) => {
            let said = format!(
                "Applied the change of chat {} on pool {}.",
                one.session, one.pool
            );
            tell(
                held,
                one.session,
                ProposalWord::Applied { path: one.subject },
            );
            Ok(said)
        }
        Err(NotMade::Kept(why)) => Err(why),
        Err(NotMade::Refused(one, why)) => {
            tell(
                held,
                one.session,
                ProposalWord::NotMade {
                    path: one.subject,
                    why: why.clone(),
                },
            );
            Err(why)
        }
    }
}

/// Why Apply made nothing.
#[derive(Debug)]
pub enum NotMade {
    /// It waits on, or waits no more and was answered already: the chat is told nothing new.
    Kept(String),
    /// The write was refused as it was made: it waits no more, and its chat is told why.
    Refused(Box<HeldWrite>, String),
}

/// **Makes held write `id` of `writes`** in the project at `root`, at `now`, where it was shown
/// whole `from` where Apply was pressed: taken out, and made as the window makes a chat's
/// brokered write here, for the chat as the app recorded it. The write made, or why not.
pub fn apply_in(
    root: &std::path::Path,
    writes: &RunnerWrites,
    id: u32,
    from: crate::proposals::From,
    now: chrono::NaiveDateTime,
) -> Result<HeldWrite, NotMade> {
    let shown = writes
        .every()
        .into_iter()
        .find(|one| one.id == id)
        .ok_or_else(|| NotMade::Kept(WAITS_NO_MORE.to_owned()))?;
    if !shown_whole(&shown, from) {
        return Err(NotMade::Kept(NOT_SHOWN_WHOLE.to_owned()));
    }
    let one = writes
        .take(id)
        .ok_or_else(|| NotMade::Kept(WAITS_NO_MORE.to_owned()))?;
    match crate::brokered::answered(root, &one.asker, &one.write, now) {
        Answer::No { why } => Err(NotMade::Refused(Box::new(one), why)),
        // A memory verb that failed says why in its lines, and wrote nothing.
        Answer::Said { lines, code } if code != 0 => {
            let why = lines
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(" ");
            Err(NotMade::Refused(Box::new(one), why))
        }
        _ => Ok(one),
    }
}

/// **A held write whole, as its own tab draws it**: what it would do with every text whole, and
/// whether Apply is offered there, or why not.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct RunnerWriteWhole {
    /// The chat that asked for it.
    pub session: u32,
    /// The pool it runs on.
    pub pool: String,
    /// What it would do, every text whole: as it would be written where it [`Self::applies`],
    /// and drawn safely, its escapes written out, where it does not.
    pub said: String,
    /// Whether Apply is offered: every text it carries is drawn as it is.
    pub applies: bool,
    /// Why Apply is not offered, in a sentence.
    pub why: Option<String>,
}

/// What a held write's tab says where a text it carries draws as something else.
pub const DRAWN_OTHERWISE: &str = "It carries a character that draws as nothing or as something \
     else, so what is shown is not what would be written: it is not applied from here. Dismiss \
     it, and ask the chat to write it plainly.";

/// `one` as its own tab draws it.
pub fn whole_of(one: &HeldWrite) -> RunnerWriteWhole {
    let applies = purlis_core::runnerwrite::drawn_as_it_is(&one.write);
    RunnerWriteWhole {
        session: one.session,
        pool: purlis_core::dispatchgrant::inert(&one.pool),
        // Unescaped only where every character draws as itself, so a backslash reads as the one
        // backslash that is written.
        said: if applies {
            one.whole.clone()
        } else {
            purlis_core::dispatchgrant::inert(&one.whole)
        },
        applies,
        why: (!applies).then(|| DRAWN_OTHERWISE.to_owned()),
    }
}

/// **Held write `id` whole, for its own tab**. Window-only: what a chat asks to write is the
/// person's to read.
#[tauri::command]
#[specta::specta]
pub async fn runner_write(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    id: u32,
) -> Result<RunnerWriteWhole, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("reading a runner chat's change", move || {
        held.runner_writes()
            .every()
            .into_iter()
            .find(|one| one.id == id)
            .map(|one| whole_of(&one))
            .ok_or_else(|| WAITS_NO_MORE.to_owned())
    })
    .await
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
        ProposalWord::Dismissed { path: one.subject },
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

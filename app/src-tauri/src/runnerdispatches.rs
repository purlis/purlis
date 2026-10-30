// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat's open or dispatch heard from a pool waits for the person**: a handoff or a task a
//! chat on a pool's runner asks this app for is held, never started, until the person answers it
//! in the window. Whatever persona it names, the chat's own included, and whatever kind of pool
//! it came from. A dispatch grant does not carry it past the hold: grants are the person's answer
//! for chats of this machine.
//!
//! Held the way a runner chat's write is held (`crate::runnerwrites`), and decided by the same
//! thing: the link the line came over (`crate::hooks::heard_from_a_pool`), and where it came
//! over this machine's own link, the router's record of the chat on a pool. **Approve** decides
//! it then as the dispatch of a chat of this machine is decided, against the limits, grants and
//! chats as they stand ([`crate::handoff::start_approved`]), so a dispatch to another persona
//! still needs its grant. **Refuse** starts nothing, and the chat is told on its next turn, in
//! words. There is no "always approve".
//!
//! In memory only: a relaunch starts with none, and a chat's held dispatches end with it.

use std::sync::{Mutex, PoisonError};

use purlis_core::hookwire::Answer;

use crate::handoff::{Arrived, Wanted};
use crate::planes::{Held, PlaneId, Planes};

/// The most dispatches one chat has held at once: one more is refused, in words.
pub const AT_MOST_PER_CHAT: usize = 8;

/// What a refused dispatch's chat is told, after the task's name.
pub fn refused_said(pool: &str) -> String {
    format!(
        "the person refused it: this chat runs on pool {pool}'s runner, and a chat there starts \
         a chat on the person's machine only with their Approve. Nothing was started. Do not \
         ask again for it; do the work on the runner, or tell the person it waits."
    )
}

/// Why an answer found nothing to answer.
const WAITS_NO_MORE: &str = "That dispatch waits no more: it was answered already, or its chat \
     closed. Nothing was started.";

/// One open or dispatch held for the person.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeldDispatch {
    pub id: u32,
    pub session: u32,
    /// The pool the chat runs on.
    pub pool: String,
    /// The persona the chat runs as, by the app's own record of it.
    pub from: Option<String>,
    /// The ask as it arrived, which Approve decides.
    pub wanted: Wanted,
    /// When it began to wait, in seconds since 1970.
    pub since: u64,
}

impl HeldDispatch {
    /// The persona it is for: the one it names, else the chat's own.
    pub fn to(&self) -> Option<&str> {
        self.wanted.to.as_deref().or(self.from.as_deref())
    }

    /// What it would start, in words: its row's line. The task's name and brief are the chat's
    /// text, which the row draws inertly.
    pub fn said(&self) -> String {
        let as_whom = self.to().map_or_else(String::new, |to| format!(" as {to}"));
        let what = match &self.wanted.moved {
            Some(moved) => format!(
                "to hand its work off to a chat{as_whom} in workspace {}",
                moved.workspace
            ),
            None => format!(
                "to start the task '{}'{as_whom} on this machine",
                self.wanted.name
            ),
        };
        let brief = purlis_core::shown::one_line(&self.wanted.brief, 240);
        format!("Asks, from pool {}, {what}: {brief}", self.pool)
    }
}

/// A project's held opens and dispatches, until the person answers.
#[derive(Debug, Default)]
pub struct RunnerDispatches {
    held: Mutex<(u32, Vec<HeldDispatch>)>,
}

impl RunnerDispatches {
    fn lock(&self) -> std::sync::MutexGuard<'_, (u32, Vec<HeldDispatch>)> {
        self.held.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Holds `wanted`, of a chat running as `from` on `pool`: its id, or none where the chat
    /// has [`AT_MOST_PER_CHAT`] held already.
    pub fn note(&self, pool: &str, from: Option<String>, wanted: Wanted, now: u64) -> Option<u32> {
        let mut held = self.lock();
        let (next, list) = &mut *held;
        if list.iter().filter(|one| one.session == wanted.chat).count() >= AT_MOST_PER_CHAT {
            return None;
        }
        *next = next.wrapping_add(1).max(1);
        let id = *next;
        list.push(HeldDispatch {
            id,
            session: wanted.chat,
            pool: pool.to_owned(),
            from,
            wanted,
            since: now,
        });
        Some(id)
    }

    /// Every dispatch held, oldest first.
    pub fn every(&self) -> Vec<HeldDispatch> {
        self.lock().1.clone()
    }

    /// Takes dispatch `id` out: it waits no more.
    pub fn take(&self, id: u32) -> Option<HeldDispatch> {
        let mut held = self.lock();
        let at = held.1.iter().position(|one| one.id == id)?;
        Some(held.1.remove(at))
    }

    /// Chat `session` has closed: its held dispatches go with it.
    pub fn forget(&self, session: u32) {
        self.lock().1.retain(|one| one.session != session);
    }
}

/// **The pool an open or dispatch of chat `chat` is heard from**, where it is: the link the line
/// came over, else the router's record of the chat on a pool. `None` for a chat of this machine.
pub fn heard_from_a_pool(held: &Held, chat: u32) -> Option<String> {
    crate::hooks::heard_from_a_pool().or_else(|| held.chats().pool_of(chat))
}

/// **Holds `wanted`, heard from `pool`, for the person**, and answers what the chat is told:
/// held, as a dispatch waiting on the person is ([`Answer::NeedsGrant`]). Nothing starts.
pub fn hold(held: &Held, pool: &str, wanted: Wanted) -> Answer {
    let from = held
        .chats()
        .recorded_chat(wanted.chat)
        .and_then(|chat| chat.persona);
    let to = wanted
        .to
        .clone()
        .or_else(|| from.clone())
        .unwrap_or_else(|| "a chat".to_owned());
    match held
        .runner_dispatches()
        .note(pool, from.clone(), wanted, crate::sandboxing::now_secs())
    {
        Some(_) => {
            held.chats().asks_may_have_moved();
            Answer::NeedsGrant {
                from,
                to,
                waiting: None,
            }
        }
        None => Answer::No {
            why: format!(
                "this chat has {AT_MOST_PER_CHAT} dispatches waiting for the person already: wait \
                 for their answers before asking for another"
            ),
        },
    }
}

/// **A held open or dispatch whole, as its own tab draws it**: everything Approve would start,
/// the brief whole, and what the chat could do with it after. Approve is pressed only there.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct RunnerDispatchWhole {
    /// The chat that asked, by its number and as its tab names it.
    pub session: u32,
    pub chat: String,
    /// The pool it runs on.
    pub pool: String,
    /// The persona the new chat would run as, none for a chat on no persona.
    pub persona: Option<String>,
    /// Whether it hands the chat's work off, rather than asking for a task that reports back.
    pub handoff: bool,
    /// The task's name, or for a handoff the workspace its work moves to.
    pub name: String,
    /// Where the new chat would work, as the ask says it: `--in`, or a handoff's workspace.
    pub place: Option<String>,
    /// Whether a handoff would create the workspace it moves to.
    pub creates: bool,
    /// The profile it would start on, as this app would choose it now.
    pub profile: Option<String>,
    /// Whether it asks for a long-running task.
    pub long: bool,
    /// The brief whole, drawn inertly: escapes written out for what would draw otherwise.
    pub brief: String,
}

/// `one` as its own tab draws it, in project `held`.
pub fn whole_of(held: &Held, one: &HeldDispatch) -> RunnerDispatchWhole {
    let inert = purlis_core::dispatchgrant::inert;
    let asking = held.chats().recorded_chat(one.session);
    let profile = one.to().and_then(|to| {
        crate::handoff::profile_now(
            held.root(),
            asking.as_ref().and_then(|chat| chat.profile.as_deref()),
            to,
            one.wanted.profile.as_deref(),
        )
    });
    let moved = one.wanted.moved.as_ref();
    RunnerDispatchWhole {
        session: one.session,
        chat: inert(
            &held
                .chats()
                .shown_name(one.session)
                .unwrap_or_else(|| one.session.to_string()),
        ),
        pool: inert(&one.pool),
        persona: one.to().map(inert),
        handoff: moved.is_some(),
        name: inert(&one.wanted.name),
        place: moved
            .map(|moved| moved.workspace.clone())
            .or_else(|| one.wanted.place.clone())
            .map(|place| inert(&place)),
        creates: moved.is_some_and(|moved| moved.create_vision.is_some()),
        profile: profile.map(|profile| inert(&profile)),
        long: one.wanted.long,
        brief: inert(&one.wanted.brief),
    }
}

/// **Held dispatch `id` whole, for its own tab**. Window-only: what a runner chat asks to start
/// here is the person's to read.
#[tauri::command]
#[specta::specta]
pub async fn runner_dispatch(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    id: u32,
) -> Result<RunnerDispatchWhole, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("reading a runner chat's dispatch", move || {
        held.runner_dispatches()
            .every()
            .into_iter()
            .find(|one| one.id == id)
            .map(|one| whole_of(&held, &one))
            .ok_or_else(|| WAITS_NO_MORE.to_owned())
    })
    .await
}

/// **Approve** on a held dispatch, pressed in its own tab. Window-only.
#[tauri::command]
#[specta::specta]
pub async fn approve_runner_dispatch(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    id: u32,
) -> Result<String, String> {
    let held = planes.held(&plane)?;
    let (said, arrivals) = crate::off_the_window("starting a runner chat's dispatch", move || {
        let arrivals = Mutex::new(Vec::new());
        let said = approved(&held, &plane, id, &|it| {
            arrivals
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(it);
        });
        let arrivals = arrivals
            .into_inner()
            .unwrap_or_else(PoisonError::into_inner);
        said.map(|said| (said, arrivals))
    })
    .await?;
    for it in arrivals {
        planes.arrived(it);
    }
    Ok(said)
}

/// [`approve_runner_dispatch`]'s work, for project `held`: taken out, and decided as a chat of
/// this machine's dispatch is decided. What the window says of it.
pub fn approved(
    held: &Held,
    plane: &PlaneId,
    id: u32,
    arrived: &(dyn Fn(Arrived) + Send + Sync),
) -> Result<String, String> {
    let one = held
        .runner_dispatches()
        .take(id)
        .ok_or_else(|| WAITS_NO_MORE.to_owned())?;
    held.chats().asks_may_have_moved();
    let to = one.to().map(str::to_owned);
    Ok(crate::handoff::start_approved(
        held,
        plane,
        (&one.wanted, to),
        arrived,
    ))
}

/// **Refuse** on a held dispatch: nothing starts. Window-only.
#[tauri::command]
#[specta::specta]
pub async fn refuse_runner_dispatch(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    id: u32,
) -> Result<(), String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("refusing a runner chat's dispatch", move || {
        refused(&held, id)
    })
    .await
}

/// [`refuse_runner_dispatch`]'s work, for project `held`: the chat is told on its next turn.
pub fn refused(held: &Held, id: u32) -> Result<(), String> {
    let one = held
        .runner_dispatches()
        .take(id)
        .ok_or_else(|| WAITS_NO_MORE.to_owned())?;
    crate::handoff::tell_refused(held, &one.wanted, &refused_said(&one.pool));
    // Listed on the Dispatches tab with the ones kept blocked, as a refused grant is (#1456).
    held.held_dispatches().kept_blocked(one.wanted);
    held.chats().asks_may_have_moved();
    Ok(())
}

#[cfg(test)]
mod tests;

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
use purlis_core::machine::StartChoice;

use crate::handoff::{Arrived, Wanted};
use crate::planes::{Held, PlaneId, Planes};

/// The most dispatches one chat has held at once: one more is refused, in words.
pub const AT_MOST_PER_CHAT: usize = 8;

/// What a refused dispatch's chat is told, after the task's name.
pub fn refused_said(one: &HeldDispatch) -> String {
    let Some(pool) = &one.pool else {
        let place = moves_said(&one.wanted);
        return format!(
            "the person refused it: it asked to run {place}, which is not where its workspace \
             runs its chats, and that waits for the person's Approve. Nothing was started. This \
             chat's dispatches to run {place} are refused from now on without asking the \
             person. Dispatch it without naming where it runs, or tell the person why it needs \
             to run there."
        );
    };
    format!(
        "the person refused it: this chat runs on pool {pool}'s runner, and a chat there starts \
         a chat on the person's machine only with their Approve. Nothing was started. Do not \
         ask again for it; do the work on the runner, or tell the person it waits."
    )
}

/// What a chat is told where the person refused its dispatch to `runs_on` before (#4862): this
/// one is refused without asking them again.
fn refused_before_said(runs_on: &StartChoice) -> String {
    let place = place_said(Some(runs_on));
    format!(
        "the person refused this chat's dispatch to run {place} before, so this one is refused \
         without asking them again. Nothing was started. Dispatch it without naming where it \
         runs, or tell the person why it needs to run there."
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
    /// The pool the asking chat runs on, where that is why it is held. `None` where it is held
    /// because it names a place to run other than its target's own levels (#4536).
    pub pool: Option<String>,
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
            // Where it asks to run, where it named a place (#4536): the levels' answer is the
            // tab's to say, as it is read when the person opens it.
            None => format!(
                "to start the task '{}'{as_whom}{}",
                self.wanted.name,
                self.wanted
                    .runs_on
                    .as_ref()
                    .map_or_else(String::new, |_| format!(" {}", moves_said(&self.wanted)))
            ),
        };
        let brief = purlis_core::shown::one_line(&self.wanted.brief, 240);
        match &self.pool {
            Some(pool) => format!("Asks, from pool {pool}, {what}: {brief}"),
            None => format!("Asks {what}: {brief}"),
        }
    }
}

/// A project's held opens and dispatches, until the person answers.
#[derive(Debug, Default)]
pub struct RunnerDispatches {
    held: Mutex<Inner>,
}

/// What [`RunnerDispatches`] keeps, under one lock, so a refusal and a new ask of the same chat
/// are decided one after the other, never between each other's steps.
#[derive(Debug, Default)]
struct Inner {
    /// The last id given.
    next: u32,
    held: Vec<HeldDispatch>,
    /// The places each chat asked to run its work and the person refused (#4862): a later
    /// dispatch of that chat to one of them is refused without asking, until the chat closes.
    refused_moves: Vec<(u32, StartChoice)>,
}

impl Inner {
    fn refused_before(&self, session: u32, runs_on: &StartChoice) -> bool {
        self.refused_moves
            .iter()
            .any(|(chat, place)| *chat == session && place == runs_on)
    }
}

/// Why [`RunnerDispatches::note`] held nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotHeld {
    /// The chat has [`AT_MOST_PER_CHAT`] held already.
    Full,
    /// The person refused this chat's dispatch to run at this place before (#4862).
    RefusedBefore(StartChoice),
}

/// What the person's Refuse took out: the dispatch refused, and the same chat's other moves to
/// the same place that still waited, refused with it (#4862).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused {
    pub one: HeldDispatch,
    pub with_it: Vec<HeldDispatch>,
}

impl RunnerDispatches {
    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.held.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Holds `wanted`, of a chat running as `from` on `pool`: its id, or why not. A move (no
    /// pool, a place named) to a place the person refused this chat before is not held: the
    /// person isn't asked again ([`NotHeld::RefusedBefore`]).
    pub fn note(
        &self,
        pool: Option<&str>,
        from: Option<String>,
        wanted: Wanted,
        now: u64,
    ) -> Result<u32, NotHeld> {
        let mut inner = self.lock();
        if pool.is_none()
            && let Some(runs_on) = &wanted.runs_on
            && inner.refused_before(wanted.chat, runs_on)
        {
            return Err(NotHeld::RefusedBefore(runs_on.clone()));
        }
        if inner
            .held
            .iter()
            .filter(|one| one.session == wanted.chat)
            .count()
            >= AT_MOST_PER_CHAT
        {
            return Err(NotHeld::Full);
        }
        inner.next = inner.next.wrapping_add(1).max(1);
        let id = inner.next;
        inner.held.push(HeldDispatch {
            id,
            session: wanted.chat,
            pool: pool.map(str::to_owned),
            from,
            wanted,
            since: now,
        });
        Ok(id)
    }

    /// Every dispatch held, oldest first.
    pub fn every(&self) -> Vec<HeldDispatch> {
        self.lock().held.clone()
    }

    /// Takes dispatch `id` out: it waits no more.
    pub fn take(&self, id: u32) -> Option<HeldDispatch> {
        let mut inner = self.lock();
        let at = inner.held.iter().position(|one| one.id == id)?;
        Some(inner.held.remove(at))
    }

    /// Takes dispatch `id` out because the person refused it. Where it was held because it
    /// moves work, its chat's later dispatches to the same place are refused without asking
    /// ([`Self::refused_before`]), and the ones to that place still waiting go with it.
    pub fn refuse(&self, id: u32) -> Option<Refused> {
        let mut inner = self.lock();
        let at = inner.held.iter().position(|one| one.id == id)?;
        let one = inner.held.remove(at);
        let mut with_it = Vec::new();
        if one.pool.is_none()
            && let Some(runs_on) = &one.wanted.runs_on
        {
            let (same, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut inner.held)
                .into_iter()
                .partition(|other| {
                    other.session == one.session
                        && other.pool.is_none()
                        && other.wanted.runs_on.as_ref() == Some(runs_on)
                });
            inner.held = rest;
            with_it = same;
            if !inner.refused_before(one.session, runs_on) {
                inner.refused_moves.push((one.session, runs_on.clone()));
            }
        }
        Some(Refused { one, with_it })
    }

    /// Whether the person refused chat `session`'s dispatch to run at `runs_on` while it runs.
    pub fn refused_before(&self, session: u32, runs_on: &StartChoice) -> bool {
        self.lock().refused_before(session, runs_on)
    }

    /// Chat `session` is now `started`, the same chat started again under a new number: the
    /// places the person refused it follow it, so a restart doesn't ask them again.
    pub fn followed(&self, session: u32, started: u32) {
        let mut inner = self.lock();
        let carried: Vec<StartChoice> = inner
            .refused_moves
            .iter()
            .filter(|(chat, _)| *chat == session)
            .map(|(_, place)| place.clone())
            .collect();
        for place in carried {
            if !inner.refused_before(started, &place) {
                inner.refused_moves.push((started, place));
            }
        }
    }

    /// Chat `session` has closed: its held dispatches, and the places it was refused, go with it.
    pub fn forget(&self, session: u32) {
        let mut inner = self.lock();
        inner.held.retain(|one| one.session != session);
        inner.refused_moves.retain(|(chat, _)| *chat != session);
    }
}

/// **The pool an open or dispatch of chat `chat` is heard from**, where it is: the link the line
/// came over, else the router's record of the chat on a pool. `None` for a chat of this machine.
pub fn heard_from_a_pool(held: &Held, chat: u32) -> Option<String> {
    crate::hooks::heard_from_a_pool().or_else(|| held.chats().pool_of(chat))
}

/// **Holds `wanted`, heard from `pool`, for the person**, and answers what the chat is told:
/// held, as a dispatch waiting on the person is ([`Answer::NeedsGrant`]). Nothing starts.
pub fn hold(held: &Held, pool: Option<&str>, wanted: Wanted) -> Answer {
    let from = held
        .chats()
        .recorded_chat(wanted.chat)
        .and_then(|chat| chat.persona);
    let to = wanted
        .to
        .clone()
        .or_else(|| from.clone())
        .unwrap_or_else(|| "a chat".to_owned());
    let runs_on = wanted.runs_on.as_ref().map(StartChoice::word);
    match held
        .runner_dispatches()
        .note(pool, from.clone(), wanted, crate::sandboxing::now_secs())
    {
        Ok(_) => {
            held.chats().asks_may_have_moved();
            match (pool, runs_on) {
                (None, Some(runs_on)) => Answer::HeldToMove { to, runs_on },
                (pool, _) => Answer::NeedsGrant {
                    from,
                    to,
                    waiting: None,
                    held_by: pool.map(str::to_owned),
                },
            }
        }
        // **A place the person refused this chat is not asked about again** (#4862), while the
        // chat runs. Only a refusal: nothing here ever starts a dispatch or stands in for an
        // Approve.
        Err(NotHeld::RefusedBefore(place)) => Answer::No {
            why: refused_before_said(&place),
        },
        Err(NotHeld::Full) => Answer::No {
            why: format!(
                "this chat has {AT_MOST_PER_CHAT} dispatches waiting for the person already: wait \
                 for their answers before asking for another"
            ),
        },
    }
}

/// **A held open or dispatch whole, as its own tab draws it**: what Approve would start, read
/// as the decision reads it ([`crate::handoff::would_start`]), the brief whole, and what the chat
/// could do with it after. Approve is pressed only there.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct RunnerDispatchWhole {
    /// The chat that asked, by its number and as its tab names it.
    pub session: u32,
    pub chat: String,
    /// The pool the asking chat runs on, where that is why it is held; null where it is held
    /// because it names a place to run other than its workspace's own (#4536).
    pub pool: Option<String>,
    /// The persona the new chat would run as, none for a chat on no persona.
    pub persona: Option<String>,
    /// Whether it hands the chat's work off, rather than asking for a task that reports back.
    pub handoff: bool,
    /// The task's name; a handoff's may be empty.
    pub name: String,
    /// Where the new chat would work, in words.
    pub works: String,
    /// The profile it would start on, none where none is offered.
    pub profile: Option<String>,
    /// Whether it asks for a long-running task.
    pub long: bool,
    /// Where it would run (#4536): this machine or a pool, as the chat asked, else where its
    /// workspace runs its chats.
    pub runs_on: String,
    /// The brief whole, drawn inertly: escapes written out for what would draw otherwise.
    pub brief: String,
}

/// `one` as its own tab draws it, in project `held`, or why it would be refused now.
pub fn whole_of(held: &Held, one: &HeldDispatch) -> Result<RunnerDispatchWhole, String> {
    let inert = purlis_core::dispatchgrant::inert;
    let would = crate::handoff::would_start(held, &one.wanted)?;
    Ok(RunnerDispatchWhole {
        session: one.session,
        chat: inert(
            &held
                .chats()
                .shown_name(one.session)
                .unwrap_or_else(|| one.session.to_string()),
        ),
        pool: one.pool.as_deref().map(inert),
        persona: would.to.as_deref().map(inert),
        handoff: one.wanted.moved.is_some(),
        name: inert(&one.wanted.name),
        works: inert(&would.works),
        profile: would.profile.as_deref().map(inert),
        long: one.wanted.long,
        runs_on: inert(&runs_on_said(
            held,
            one.wanted.runs_on.as_ref(),
            would.workspace.as_deref(),
        )),
        brief: inert(&one.wanted.brief),
    })
}

/// **Where a held dispatch would run, in words** (#4536): what the person reads before Approve.
/// The chat's own choice, else where its target workspace's levels put it now.
pub(crate) fn runs_on_said(
    held: &Held,
    runs_on: Option<&StartChoice>,
    workspace: Option<&str>,
) -> String {
    use purlis_core::machine::PlaceLevel;
    match runs_on {
        Some(StartChoice::ThisMachine) => return "This machine, as the chat asks".to_owned(),
        Some(StartChoice::Pool(pool)) => return format!("Pool {pool}, as the chat asks"),
        None => {}
    }
    let Some(config) = held.chats().pools_in() else {
        return "This machine".to_owned();
    };
    let place = purlis_core::pools::chat_place(&config, held.root(), workspace, None).ok();
    // A level naming a pool this machine no longer keeps: said as that, since the start refuses it.
    if place.is_none()
        && let Some(gone) = workspace.and_then(|workspace| {
            purlis_core::machine::read(&config)
                .store
                .chat_place(held.root(), workspace, None)
                .pool
        })
    {
        return format!("Pool {gone}, which this machine doesn't keep, so it can't start there");
    }
    match place {
        Some(place) => match (place.pool, place.level) {
            (Some(pool), PlaceLevel::Project) => format!("Pool {pool}, as its project's chats run"),
            (Some(pool), _) => format!("Pool {pool}, as its workspace's chats run"),
            (None, PlaceLevel::Workspace) => {
                "This machine, as its workspace's chats run".to_owned()
            }
            (None, _) => "This machine".to_owned(),
        },
        None => "Where its workspace runs its chats, which can't be read now".to_owned(),
    }
}

/// What a dispatch held because it moves work asks, in words: where it asks to run.
fn moves_said(wanted: &Wanted) -> String {
    place_said(wanted.runs_on.as_ref())
}

/// Where a chat asked to run, in words: "on pool <name>" or "on this machine".
fn place_said(runs_on: Option<&StartChoice>) -> String {
    match runs_on {
        Some(StartChoice::Pool(pool)) => format!("on pool {pool}"),
        Some(StartChoice::ThisMachine) | None => "on this machine".to_owned(),
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
            .ok_or_else(|| WAITS_NO_MORE.to_owned())
            .and_then(|one| whole_of(&held, &one))
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
    // The persona as the decision resolves it, for a failed row.
    let to = crate::handoff::would_start(held, &one.wanted)
        .ok()
        .and_then(|would| would.to)
        .or_else(|| one.to().map(str::to_owned));
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
    let Refused { one, with_it } = held
        .runner_dispatches()
        .refuse(id)
        .ok_or_else(|| WAITS_NO_MORE.to_owned())?;
    crate::handoff::tell_refused(held, &one.wanted, &refused_said(&one));
    // Its chat's other asks to the same place go with it, each told so (#4862).
    for other in with_it {
        if let Some(place) = &other.wanted.runs_on {
            crate::handoff::tell_refused(held, &other.wanted, &refused_before_said(place));
        }
        held.held_dispatches().kept_blocked(other.wanted);
    }
    // Listed on the Dispatches tab with the ones kept blocked, as a refused grant is (#1456).
    held.held_dispatches().kept_blocked(one.wanted);
    held.chats().asks_may_have_moved();
    Ok(())
}

#[cfg(test)]
mod tests;

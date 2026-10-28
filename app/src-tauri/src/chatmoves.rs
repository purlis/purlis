// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Move to runner and Move here** (#2048, spec #2041): a chat moves between this machine and a
//! pool's runner with one action, its branch, its uncommitted work, its profile and its
//! conversation going with it.
//!
//! **Prepared in the background, switched at a pause.** Asking for a move starts its
//! preparation at once, off the window's thread, while the chat keeps working where it is: for a
//! runner, the project held there and each clone's branch sent ahead. The switch itself waits for
//! the chat's next pause between turns, which the window drives as it drives a restart owed
//! ([`Moves::due`]): the run ends where it is and a new one starts there, on the same
//! conversation, in its place. A move is never made mid-turn: the core asks the board itself,
//! whoever asked.
//!
//! **A move that fails leaves the chat where it was**, with the reason, which its tab shows
//! until it is put away ([`Moves::put_away`]). A preparation that fails changes nothing of the
//! chat; a switch that fails ends nothing, since the chat's run here ends only once the new one
//! has started.
//!
//! **A move is the workspace's** (D-2048-1): a workspace's chats share its clones, so a chat
//! moves with its workspace, whose chats then run where it went (`chatsRunOn`, the setting the
//! person can change themselves), and a move is refused while another chat of the workspace
//! runs where it is. That is also what finds a moved chat again after this app restarts.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

/// Where a chat runs: a pool's runner by the pool's name, or this machine.
pub(crate) type Place = Option<String>;

/// How far a move has come.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Stage {
    /// Its preparation runs; the chat keeps working where it is.
    Preparing,
    /// Prepared: it switches at the chat's next pause.
    Ready,
    /// It did not happen, and why: the chat is where it was.
    Failed(String),
}

/// One chat's move.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Move {
    pub(crate) from: Place,
    pub(crate) to: Place,
    pub(crate) stage: Stage,
}

/// **A chat's move as the window draws it** (#2048): what `chat-move` tells, and each row of
/// `chat_moves`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub(crate) struct ChatMove {
    pub(crate) session: u32,
    /// The pool it moves to, or `null` for this machine.
    pub(crate) to: Option<String>,
    /// `preparing`, `ready` or `failed`.
    pub(crate) stage: String,
    /// Why it failed, in a sentence: where it is `failed`.
    pub(crate) why: Option<String>,
}

impl ChatMove {
    fn of(session: u32, one: &Move) -> ChatMove {
        let (stage, why) = match &one.stage {
            Stage::Preparing => ("preparing", None),
            Stage::Ready => ("ready", None),
            Stage::Failed(why) => ("failed", Some(why.clone())),
        };
        ChatMove {
            session,
            to: one.to.clone(),
            stage: stage.to_owned(),
            why,
        }
    }
}

/// **A move's stage as the window hears it** (#2048): `chat-move`'s payload.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub(crate) struct MoveTold {
    pub(crate) plane: crate::planes::PlaneId,
    pub(crate) moving: ChatMove,
}

/// The event the window hears a move's stage by.
pub(crate) const EVENT: &str = "chat-move";

/// Told each move as its stage changes.
pub(crate) type Told = Arc<dyn Fn(ChatMove) + Send + Sync + 'static>;

/// What the window is told when a move is not due yet.
pub(crate) const NOT_AT_A_PAUSE: &str =
    "It moves at its next pause between turns, so its turn is never cut off.";

/// What a chat's move is now, at the window's ask to switch it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Due {
    /// The chat is not moving.
    NotMoving,
    /// Not yet, and why, in a sentence.
    NotYet(String),
    /// It switches now.
    Now(Move),
}

/// **Every chat's move in one project**, while it is asked for and until it is put away.
#[derive(Default)]
pub(crate) struct Moves {
    by_chat: Mutex<HashMap<u32, Move>>,
    told: Mutex<Option<Told>>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Moves {
    /// Who is told each move's stage.
    pub(crate) fn telling(&self, told: Told) {
        *lock(&self.told) = Some(told);
    }

    fn tell(&self, session: u32, one: &Move) {
        let told = lock(&self.told).clone();
        if let Some(told) = told {
            told(ChatMove::of(session, one));
        }
    }

    /// **Starts chat `session`'s move from `from` to `to`**: `prepare` runs off this thread,
    /// and the move is ready to switch once it has, or failed with its words. Refused while the
    /// chat is moving already.
    pub(crate) fn begin(
        self: &Arc<Self>,
        session: u32,
        from: Place,
        to: Place,
        prepare: impl FnOnce() -> Result<(), String> + Send + 'static,
    ) -> Result<(), String> {
        let one = {
            let mut all = lock(&self.by_chat);
            if all
                .get(&session)
                .is_some_and(|one| !matches!(one.stage, Stage::Failed(_)))
            {
                return Err("This chat is moving already.".to_owned());
            }
            let one = Move {
                from,
                to,
                stage: Stage::Preparing,
            };
            all.insert(session, one.clone());
            one
        };
        self.tell(session, &one);
        let moves = Arc::clone(self);
        let spawned = std::thread::Builder::new()
            .name("purlis-chat-move".into())
            .spawn(move || {
                let stage = match prepare() {
                    Ok(()) => Stage::Ready,
                    Err(why) => Stage::Failed(why),
                };
                moves.reached(session, stage);
            });
        if let Err(why) = spawned {
            self.reached(
                session,
                Stage::Failed(format!("purlis could not prepare the move: {why}")),
            );
        }
        Ok(())
    }

    /// Chat `session`'s move has reached `stage`, unless it was put away meanwhile.
    fn reached(&self, session: u32, stage: Stage) {
        let one = {
            let mut all = lock(&self.by_chat);
            let Some(one) = all.get_mut(&session) else {
                return;
            };
            one.stage = stage;
            one.clone()
        };
        self.tell(session, &one);
    }

    /// **Whether chat `session`'s move switches now**: only once it is prepared, and only at a
    /// pause between its turns (`at_a_pause`), which the caller reads from the board.
    pub(crate) fn due(&self, session: u32, at_a_pause: bool) -> Due {
        let all = lock(&self.by_chat);
        let Some(one) = all.get(&session) else {
            return Due::NotMoving;
        };
        match &one.stage {
            Stage::Preparing => Due::NotYet("Its move is still being prepared.".to_owned()),
            Stage::Failed(_) => Due::NotMoving,
            Stage::Ready if !at_a_pause => Due::NotYet(NOT_AT_A_PAUSE.to_owned()),
            Stage::Ready => Due::Now(one.clone()),
        }
    }

    /// Chat `session` moved: its move is over.
    pub(crate) fn moved(&self, session: u32) {
        lock(&self.by_chat).remove(&session);
    }

    /// Chat `session`'s switch failed, and why: it stays where it was.
    pub(crate) fn failed(&self, session: u32, why: String) {
        self.reached(session, Stage::Failed(why));
    }

    /// **Puts chat `session`'s move away**: one being prepared is forgotten (its preparation
    /// changes nothing of the chat), and a failed one's reason is put away.
    pub(crate) fn put_away(&self, session: u32) {
        lock(&self.by_chat).remove(&session);
    }

    /// Every move, as the window draws them.
    pub(crate) fn shown(&self) -> Vec<ChatMove> {
        let mut shown: Vec<ChatMove> = lock(&self.by_chat)
            .iter()
            .map(|(session, one)| ChatMove::of(*session, one))
            .collect();
        shown.sort_by_key(|one| one.session);
        shown
    }
}

/// **Whether a chat's work may come back from pool `pool`'s runner onto this machine's clones**
/// (the person's ruling of 2026-10-27): only from a runner shown to be the person's own, by the
/// one check browser logins and tools use (`read`, `poollogins::Bound::read`): until a pool can
/// be marked as yours (#2561), a Docker pool on this machine's own engine. Refused otherwise, in
/// a sentence that says why.
pub(crate) fn brings_work_back(
    pool: &str,
    read: impl FnOnce() -> Result<purlis_core::poollogins::Bound, String>,
) -> Result<(), String> {
    read().map(|_| ()).map_err(|_| {
        format!(
            "This chat's work does not come back from pool {pool}: its runner is not known to be \
             yours, and a runner's work comes onto this machine's clones only from a Docker pool \
             on this machine's own engine, until a pool can be marked as yours."
        )
    })
}

/// **Moves `chat` to pool `pool`'s runner** (#2048, #2049): its profile kept on the runner
/// first (`ready`, `poolprofiles::keep_on` through the start's own readiness check), so its
/// conversation is written into a folder of a profile the runner keeps, and only then its work
/// and conversation (`move_in`). Nothing moves where the profile is not kept or not signed in
/// there.
pub(crate) fn to_a_runner(
    pool: &str,
    chat: &crate::host::Moving,
    ready: &dyn Fn(&str, &str) -> Result<(), String>,
    move_in: &dyn Fn() -> Result<(), String>,
) -> Result<(), String> {
    ready(pool, &chat.profile)?;
    move_in()
}

/// **What a chat's tab says of what came back with it** (the person's ruling of 2026-10-27):
/// every file a tool here runs or reads as its settings, every one a save would refuse, and
/// every link, by its path in the project. None where nothing such came.
pub(crate) fn came_back_said(from: Option<&str>, came: &[String]) -> Option<String> {
    if came.is_empty() {
        return None;
    }
    let from = from.map_or_else(|| "a runner".to_owned(), |pool| format!("pool {pool}"));
    Some(format!(
        "Its work came back from {from} with files a tool on this machine runs or reads, or \
         links that may lead out of the project. Look at them before you run anything here: {}.",
        came.join(", ")
    ))
}

/// What a chat is, for its move: what its workspace, profile and conversation are, and where it
/// runs now.
struct Mover {
    chat: crate::host::Moving,
    from: Place,
}

/// What the window is told of a move it asked for to switch now.
#[derive(serde::Serialize, specta::Type)]
pub(crate) struct MoveNow {
    /// The chat moved, as the window draws it in its place; `null` where it did not move now.
    pub(crate) chat: Option<crate::OpenChat>,
    /// Why it did not move yet, in a sentence: it moves at a later pause.
    pub(crate) not_yet: Option<String>,
    /// What its tab says after the move: what came back that the person is to look at.
    pub(crate) notices: Vec<String>,
}

impl crate::planes::Held {
    /// What chat `session` is for a move to `to`, or why it does not move, in a sentence.
    fn mover(&self, session: u32, to: &Place) -> Result<Mover, String> {
        let chats = self.chats();
        let open = chats
            .open_now()
            .into_iter()
            .find(|open| open.session == session)
            .ok_or_else(|| "This chat is not open, so it was not moved.".to_owned())?;
        if open.harness.is_none() {
            return Err("A shell runs where its window composed it, so it does not move.".into());
        }
        let workspace = open.workspace.clone().ok_or_else(|| {
            "A chat at the project's root works on the project itself, which stays on this \
             machine, so it does not move."
                .to_owned()
        })?;
        let profile = open
            .profile
            .clone()
            .ok_or_else(|| "This chat runs on no profile, so it does not move.".to_owned())?;
        let conversation = chats
            .recorded_chat(session)
            .and_then(|chat| chat.resume)
            .map(|id| id.as_str().to_owned())
            .ok_or_else(|| {
                "This chat has no conversation purlis knows yet, so there is nothing to move: \
                 try again once it has answered."
                    .to_owned()
            })?;
        let from = chats.pool_of(session);
        if &from == to {
            return Err(match to {
                Some(pool) => format!("This chat runs on pool {pool} already."),
                None => "This chat runs on this machine already.".to_owned(),
            });
        }
        if to.is_some() && chats.started_unsandboxed(session) {
            return Err(
                "This chat runs without the sandbox, and a runner runs every chat in it: a move \
                 asks again on the runner, and that is not offered yet, so it stays here."
                    .to_owned(),
            );
        }
        // A workspace's chats share its clones: they move together, or not at all.
        let others: Vec<String> = chats
            .open_now()
            .into_iter()
            .filter(|other| other.session != session)
            .filter(|other| other.workspace.as_deref() == Some(workspace.as_str()))
            .filter(|other| chats.pool_of(other.session) == from)
            .map(|other| other.label.unwrap_or(other.name))
            .collect();
        if !others.is_empty() {
            return Err(format!(
                "{} also works in workspace {workspace} where this chat runs, and the chats of a \
                 workspace share its clones, so this one was not moved: close or move {} first.",
                others.join(", "),
                if others.len() == 1 { "it" } else { "them" }
            ));
        }
        Ok(Mover {
            chat: crate::host::Moving {
                workspace,
                profile,
                conversation,
            },
            from,
        })
    }

    /// **Starts chat `session`'s move to `to`** (#2048): checked now, then prepared in the
    /// background while the chat keeps working; it switches at the chat's next pause. Refused,
    /// in a sentence, for a chat that does not move.
    pub(crate) fn move_chat(self: &Arc<Self>, session: u32, to: Place) -> Result<(), String> {
        let mover = self.mover(session, &to)?;
        if let Some(pool) = &to {
            let config = self.chats().pools_in().ok_or_else(|| {
                "This machine keeps no pool, so the chat was not moved.".to_owned()
            })?;
            purlis_core::runners::runner_for(&config, pool).map_err(|why| format!("{why}."))?;
            // Its conversation is found before anything moves: a harness whose conversations do
            // not move says so now.
            purlis_core::conversationmove::OfAChat {
                root: self.root(),
                workspace: &mover.chat.workspace,
                profile: &mover.chat.profile,
                conversation: &mover.chat.conversation,
            }
            .find()
            .map_err(|why| format!("This chat was not moved: {why}."))?;
        }
        if let (Some(pool), None) = (&mover.from, &to) {
            let config = self.chats().pools_in().ok_or_else(|| {
                "This machine keeps no pool, so the chat was not moved.".to_owned()
            })?;
            brings_work_back(pool, || purlis_core::poollogins::Bound::read(&config, pool))?;
        }
        let prepare: Box<dyn FnOnce() -> Result<(), String> + Send> = match &to {
            Some(pool) => {
                // Weak: a project let go of meanwhile is prepared for no more.
                let held = Arc::downgrade(self);
                let (pool, workspace) = (pool.clone(), mover.chat.workspace.clone());
                let profile = mover.chat.profile.clone();
                Box::new(move || {
                    let held = held
                        .upgrade()
                        .ok_or_else(|| "The project was closed, so nothing moved.".to_owned())?;
                    // Its profile is kept on the runner first (#2049), so a profile the runner
                    // cannot keep, or one not signed in there, is said before anything moves.
                    held.chats().ready_profile_on(&pool, &profile)?;
                    held.chats().sessions().move_ahead(&pool, &workspace)
                })
            }
            None => Box::new(|| Ok(())),
        };
        self.moves().begin(session, mover.from, to, prepare)
    }

    /// **Moves chat `session` now, where its move is prepared and it is at a pause between
    /// turns** (#2048): its work and conversation go where it moves, its workspace's chats run
    /// there from now on, and a new run starts there on its conversation in its place; the run
    /// it leaves then ends. Not yet, with why, mid-turn, while it waits on the person, while
    /// the person stops it, or while its move is prepared. A switch that fails leaves it
    /// running where it was, and its move says why.
    pub(crate) fn move_chat_now(
        &self,
        session: u32,
        size: purlis_core::engine::Size,
    ) -> Result<Option<u32>, String> {
        let at_a_pause = self.board().glance(session).state == purlis_core::state::State::Waiting
            && !self.asks().asks.iter().any(|ask| ask.session == session)
            && !self.stopping().is_stopping(session);
        let one = match self.moves().due(session, at_a_pause) {
            Due::NotMoving => return Err("This chat is not moving.".to_owned()),
            Due::NotYet(_) => return Ok(None),
            Due::Now(one) => one,
        };
        match self.switch(session, &one, size) {
            Ok(started) => {
                self.moves().moved(session);
                Ok(Some(started))
            }
            Err(why) => {
                self.moves().failed(session, why.clone());
                Err(why)
            }
        }
    }

    fn switch(
        &self,
        session: u32,
        one: &Move,
        size: purlis_core::engine::Size,
    ) -> Result<u32, String> {
        let mover = self.mover(session, &one.to)?;
        if mover.from != one.from {
            return Err("This chat moved since its move was asked for, so it stays.".to_owned());
        }
        let config = self
            .chats()
            .pools_in()
            .ok_or_else(|| "This machine keeps no pool, so the chat was not moved.".to_owned())?;
        let sessions = self.chats().sessions();
        let came = match (&one.from, &one.to) {
            (None, Some(pool)) => {
                to_a_runner(pool, &mover.chat, &|pool, profile| {
                    self.chats().ready_profile_on(pool, profile)
                }, &|| sessions.move_in(pool, &mover.chat))?;
                Vec::new()
            }
            (Some(pool), None) => {
                // Asked again at the switch: a pool may have changed since the move was asked.
                brings_work_back(pool, || purlis_core::poollogins::Bound::read(&config, pool))?;
                sessions.move_out(pool, &mover.chat)?
            }
            _ => return Err("A chat moves between this machine and a pool's runner.".to_owned()),
        };
        let runs_on = |place: &Place| {
            crate::dockerpools::run_chats_on(&config, self.root(), &mover.chat.workspace, place)
        };
        runs_on(&one.to)?;
        match self.chats().start_again_where_it_runs(session, size) {
            Ok(started) => {
                if let Some(said) = came_back_said(one.from.as_deref(), &came) {
                    self.chats().note_on_its_tab(started, said);
                }
                self.in_its_place_after_a_move(session, started);
                Ok(started)
            }
            Err(why) => {
                if let Err(back) = runs_on(&one.from) {
                    tracing::warn!(
                        "purlis: where the workspace's chats run was not put back: {back}"
                    );
                }
                Err(format!(
                    "It did not start where it moved, so it stays: {why}"
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests;

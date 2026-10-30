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

use purlis_core::poollogins::Unbound;

/// Where a chat runs: a pool's runner by the pool's name, or this machine.
pub(crate) type Place = Option<String>;

/// How far a move has come.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Stage {
    /// Its preparation runs; the chat keeps working where it is.
    Preparing,
    /// Prepared: it switches at the chat's next pause.
    Ready,
    /// Its switch is being made, for every chat of its workspace at once (#3453).
    Switching,
    /// It did not happen, and why: the chat is where it was.
    Failed(String),
}

/// A chat in a move, by its number and its name as its tab shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Member {
    pub(crate) session: u32,
    pub(crate) name: String,
}

/// **One move of a workspace's chats** (#3453): every chat of the workspace running where it
/// runs moves with it, since they share its clones, each held under this same move.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Move {
    pub(crate) from: Place,
    pub(crate) to: Place,
    pub(crate) stage: Stage,
    /// Every chat that moves together, this one among them.
    pub(crate) group: Vec<Member>,
}

impl Move {
    fn sessions(&self) -> impl Iterator<Item = u32> + '_ {
        self.group.iter().map(|one| one.session)
    }
}

/// **A chat's move as the window draws it** (#2048): what `chat-move` tells, and each row of
/// `chat_moves`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub(crate) struct ChatMove {
    pub(crate) session: u32,
    /// The pool it runs on as it moves, or `null` for this machine (#3646): where it still runs
    /// where the move did not happen.
    pub(crate) from: Option<String>,
    /// The pool it moves to, or `null` for this machine.
    pub(crate) to: Option<String>,
    /// `preparing`, `ready`, `switching` (being made now, past cancelling), `failed`, or
    /// `over`: it moved or was put away, and the window forgets it (#3814).
    pub(crate) stage: String,
    /// Why it failed, in a sentence: where it is `failed`.
    pub(crate) why: Option<String>,
    /// The other chats of its workspace that move with it, by name (#3453).
    pub(crate) with: Vec<String>,
}

impl ChatMove {
    fn of(session: u32, one: &Move) -> ChatMove {
        let (stage, why) = match &one.stage {
            Stage::Preparing => ("preparing", None),
            Stage::Ready => ("ready", None),
            Stage::Switching => ("switching", None),
            Stage::Failed(why) => ("failed", Some(why.clone())),
        };
        ChatMove {
            session,
            from: one.from.clone(),
            to: one.to.clone(),
            stage: stage.to_owned(),
            why,
            with: one
                .group
                .iter()
                .filter(|other| other.session != session)
                .map(|other| other.name.clone())
                .collect(),
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
    /// It switches now, with every chat of its move.
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

/// What a Cancel of a move being made now is told (#3814).
pub(crate) const SWITCHING_NOT_CANCELLED: &str =
    "Its move is being made now, so it is no longer cancelled.";

/// The chats of `group` not at a pause between turns, by name.
fn busy_in(group: &[Member], at_a_pause: &impl Fn(u32) -> bool) -> Vec<String> {
    group
        .iter()
        .filter(|one| !at_a_pause(one.session))
        .map(|one| one.name.clone())
        .collect()
}

/// **Whether every chat of `group` is still at a pause** as a move takes their places
/// (#3814): one that began a turn while the move was made would have it cut off, so none takes
/// its place, and why, in a sentence.
pub(crate) fn still_at_a_pause(
    group: &[Member],
    at_a_pause: &impl Fn(u32) -> bool,
) -> Result<(), String> {
    let busy = busy_in(group, at_a_pause);
    if busy.is_empty() {
        return Ok(());
    }
    Err(format!(
        "{} began a turn while the move was made, so none moved and its turn was not cut off: \
         ask for the move again.",
        listed(&busy)
    ))
}

/// Names in a sentence: `a`, `a and b`, `a, b and c`.
pub(crate) fn listed(names: &[String]) -> String {
    match names {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

impl Moves {
    /// Who is told each move's stage.
    pub(crate) fn telling(&self, told: Told) {
        *lock(&self.told) = Some(told);
    }

    fn tell(&self, one: &Move) {
        let told = lock(&self.told).clone();
        if let Some(told) = told {
            for session in one.sessions() {
                told(ChatMove::of(session, one));
            }
        }
    }

    /// Tells every chat of `one` that its move is over: it moved, or it was put away, so each
    /// tab of it forgets it, not only the one that asked (#3814).
    fn tell_over(&self, one: &Move) {
        let told = lock(&self.told).clone();
        if let Some(told) = told {
            for session in one.sessions() {
                told(ChatMove {
                    stage: "over".to_owned(),
                    why: None,
                    ..ChatMove::of(session, one)
                });
            }
        }
    }

    /// **Starts the move of `group`, a workspace's chats, from `from` to `to`** (#3453):
    /// `prepare` runs once, off this thread, and the move is ready to switch once it has, or
    /// failed with its words, for every one of them. Refused while any of them is moving
    /// already.
    pub(crate) fn begin(
        self: &Arc<Self>,
        group: Vec<Member>,
        from: Place,
        to: Place,
        prepare: impl FnOnce() -> Result<(), String> + Send + 'static,
    ) -> Result<(), String> {
        let Some(first) = group.first().map(|one| one.session) else {
            return Err("No chat was asked to move.".to_owned());
        };
        let one = {
            let mut all = lock(&self.by_chat);
            let moving: Vec<String> = group
                .iter()
                .filter(|member| {
                    all.get(&member.session)
                        .is_some_and(|one| !matches!(one.stage, Stage::Failed(_)))
                })
                .map(|member| member.name.clone())
                .collect();
            if !moving.is_empty() {
                return Err(if group.len() == 1 {
                    "This chat is moving already.".to_owned()
                } else {
                    format!(
                        "{} of its workspace {} moving already.",
                        listed(&moving),
                        if moving.len() == 1 { "is" } else { "are" }
                    )
                });
            }
            let one = Move {
                from,
                to,
                stage: Stage::Preparing,
                group,
            };
            for session in one.sessions() {
                all.insert(session, one.clone());
            }
            one
        };
        self.tell(&one);
        let moves = Arc::clone(self);
        let group = one.group.clone();
        let spawned = std::thread::Builder::new()
            .name("purlis-chat-move".into())
            .spawn(move || {
                let stage = match prepare() {
                    Ok(()) => Stage::Ready,
                    Err(why) => Stage::Failed(why),
                };
                moves.reached_by(first, &group, stage);
            });
        if let Err(why) = spawned {
            self.reached(
                first,
                Stage::Failed(format!("purlis could not prepare the move: {why}")),
            );
        }
        Ok(())
    }

    /// Chat `session`'s move has reached `stage`, for every chat of it, unless it was put away
    /// meanwhile.
    fn reached(&self, session: u32, stage: Stage) {
        let group = match lock(&self.by_chat).get(&session) {
            Some(one) => one.group.clone(),
            None => return,
        };
        self.reached_by(session, &group, stage);
    }

    /// [`Self::reached`], for the move of `group`: a chat of it held under another move since
    /// is left alone.
    fn reached_by(&self, session: u32, group: &[Member], stage: Stage) {
        let one = {
            let mut all = lock(&self.by_chat);
            let Some(mut one) = all.get(&session).cloned().filter(|one| one.group == group) else {
                return;
            };
            one.stage = stage;
            for member in group {
                if all
                    .get(&member.session)
                    .is_some_and(|held| held.group == group)
                {
                    all.insert(member.session, one.clone());
                }
            }
            one
        };
        self.tell(&one);
    }

    /// **Whether chat `session`'s move switches now**: only once it is prepared, and only once
    /// every chat of it is at a pause between its turns (`at_a_pause`, which the caller reads
    /// from the board), so no chat's turn is cut off and no chat works on a clone while it moves
    /// (#3453). Answered `Now` once: the move is then being made.
    pub(crate) fn due(&self, session: u32, at_a_pause: impl Fn(u32) -> bool) -> Due {
        let mut all = lock(&self.by_chat);
        let Some(one) = all.get(&session).cloned() else {
            return Due::NotMoving;
        };
        match &one.stage {
            Stage::Preparing => Due::NotYet("Its move is still being prepared.".to_owned()),
            Stage::Switching => Due::NotYet("Its move is being made.".to_owned()),
            Stage::Failed(_) => Due::NotMoving,
            Stage::Ready if !at_a_pause(session) => Due::NotYet(NOT_AT_A_PAUSE.to_owned()),
            Stage::Ready => {
                let busy = busy_in(&one.group, &at_a_pause);
                if !busy.is_empty() {
                    return Due::NotYet(format!(
                        "It moves with {} once each of them is at a pause too.",
                        listed(&busy)
                    ));
                }
                let switching = Move {
                    stage: Stage::Switching,
                    ..one
                };
                for member in &switching.group {
                    all.insert(member.session, switching.clone());
                }
                drop(all);
                // Every tab of it hears it is being made, so none offers Cancel now.
                self.tell(&switching);
                Due::Now(switching)
            }
        }
    }

    /// Chat `session` moved, with every chat of its move: the move is over, and every tab of
    /// it is told so, a chat that did not take its place among them (#3814).
    pub(crate) fn moved(&self, session: u32) {
        let one = {
            let mut all = lock(&self.by_chat);
            let Some(one) = all.get(&session).cloned() else {
                return;
            };
            for member in one.sessions() {
                all.remove(&member);
            }
            one
        };
        self.tell_over(&one);
    }

    /// Chat `session`'s switch failed, and why: it stays where it was, with every chat of its
    /// move.
    pub(crate) fn failed(&self, session: u32, why: String) {
        self.reached(session, Stage::Failed(why));
    }

    /// **Puts chat `session`'s move away**: one being prepared or ready is forgotten for every
    /// chat of it, since they move together (its preparation changes nothing of them), and
    /// every tab of it is told (#3814); a failed one's reason is put away for this chat. One
    /// being made is past cancelling, and refused in a sentence.
    pub(crate) fn put_away(&self, session: u32) -> Result<(), String> {
        let over = {
            let mut all = lock(&self.by_chat);
            let Some(one) = all.get(&session).cloned() else {
                return Ok(());
            };
            match one.stage {
                Stage::Switching => {
                    return Err(SWITCHING_NOT_CANCELLED.to_owned());
                }
                Stage::Failed(_) => {
                    all.remove(&session);
                    None
                }
                Stage::Preparing | Stage::Ready => {
                    for member in one.sessions() {
                        all.remove(&member);
                    }
                    Some(one)
                }
            }
        };
        if let Some(one) = over {
            self.tell_over(&one);
        }
        Ok(())
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
/// a sentence that says the pool's own reason (#3646).
pub(crate) fn brings_work_back(
    pool: &str,
    read: impl FnOnce() -> Result<purlis_core::poollogins::Bound, Unbound>,
) -> Result<(), String> {
    read().map(|_| ()).map_err(|why| {
        let not_yours = "its runner is not known to be yours, and a runner's work comes onto \
                         this machine's clones only from a Docker pool on this machine's own \
                         engine, until a pool can be marked as yours.";
        format!(
            "This chat's work does not come back from pool {pool}: {}",
            not_ready_said(&why, not_yours)
        )
    })
}

/// **Why a pool is not ready for a move, in words** (#3646): the pool's own reason, and
/// `not_yours` where its runners are not shown to be the person's own.
fn not_ready_said(why: &Unbound, not_yours: &str) -> String {
    match why {
        Unbound::NotYours(_) => not_yours.to_owned(),
        Unbound::NotHere => "no pool of that name is on this machine.".to_owned(),
        Unbound::NoRunnerOfItsImage => {
            "the pool has no runner of its image, so remove the pool and add it again.".to_owned()
        }
        Unbound::Draining => "its runner is draining and the pool has no new runner yet, so \
                              ask for the move again once it has one."
            .to_owned(),
        Unbound::Unread(why) => format!("this machine's pools could not be read: {why}."),
    }
}

/// **Readies pool `pool`'s runner for the chats of `workspace` on `profiles` to move there**
/// (#2048, #2049, #3453): each profile kept on the runner, signed in there, first, then the
/// project held there and each clone's branch sent ahead, once for them all. Nothing is sent
/// ahead where a profile is not kept.
pub(crate) fn prepare_on_a_runner(
    chats: &crate::chats::Chats,
    pool: &str,
    workspace: &str,
    profiles: &[String],
) -> Result<(), String> {
    for profile in profiles {
        chats.ready_profile_on(pool, profile)?;
    }
    chats.sessions().move_ahead(pool, workspace)
}

/// **Moves `moving`, the chats of one workspace, to pool `pool`'s runner** (#2048, #2049,
/// #3453), at their pause: each one's profile kept on the runner again first, so each
/// conversation is written into a folder of a profile the runner keeps, and only then the
/// workspace's work, once, and each conversation. Nothing moves where a profile is not kept or
/// not signed in there.
pub(crate) fn move_to_a_runner(
    chats: &crate::chats::Chats,
    pool: &str,
    moving: &[crate::host::Moving],
) -> Result<(), String> {
    for profile in profiles_of(moving) {
        chats.ready_profile_on(pool, &profile)?;
    }
    chats.sessions().move_in(pool, moving)
}

/// The profiles `moving` run on, each once.
fn profiles_of(moving: &[crate::host::Moving]) -> Vec<String> {
    let mut profiles: Vec<String> = moving.iter().map(|one| one.profile.clone()).collect();
    profiles.sort();
    profiles.dedup();
    profiles
}

/// **Whether a chat may move to pool `pool`'s runner** (D-2048-13, the steward's decision of
/// 2026-10-28): only to a runner shown to be the person's own, by the check Move here is held to
/// (the person's ruling of 2026-10-27) and browser logins and tools use (`read`,
/// `poollogins::Bound::read`): until a pool can be marked as yours (#2561), a Docker pool on this
/// machine's own engine. A chat's conversation and work go nowhere else. Refused, in a sentence
/// that says the pool's own reason (#3646).
pub(crate) fn goes_to_a_runner_of_yours(
    pool: &str,
    read: impl FnOnce() -> Result<purlis_core::poollogins::Bound, Unbound>,
) -> Result<(), String> {
    read().map(|_| ()).map_err(|why| {
        let not_yours = "its runner is not known to be yours, and a chat's conversation and \
                         work go only to a Docker pool on this machine's own engine, until a \
                         pool can be marked as yours.";
        format!(
            "This chat does not move to pool {pool}: {}",
            not_ready_said(&why, not_yours)
        )
    })
}

/// **Why a move did not start where it went** (#2048): why, in a sentence, for the chat's pane,
/// which says where it still runs itself (#3646). Where the workspace's setting could not be put
/// back as well (`put_back`), the setting names the other place while the chat runs where it
/// was, so the sentence says how to set the workspace back, so its next chat starts with it.
pub(crate) fn did_not_start_said(
    why: &str,
    put_back: Result<(), String>,
    from: &Place,
    to: &Place,
    workspace: &str,
) -> String {
    let place = |place: &Place| match place {
        Some(pool) => format!("pool {pool}"),
        None => "this machine".to_owned(),
    };
    let (from, to) = (place(from), place(to));
    match put_back {
        Ok(()) => format!("It did not start on {to}: {why}"),
        Err(back) => format!(
            "It did not start on {to}: {why} And workspace {workspace}'s chats are still set to \
             run on {to}, since that could not be set back ({back}): set Runs on to {from} in the \
             workspace's settings, so its next chat starts with the others."
        ),
    }
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

/// What the chats of a move are, for it: each one's workspace, profile and conversation, and
/// where they run now.
struct Movers {
    chats: Vec<(Member, crate::host::Moving)>,
    from: Place,
}

impl Movers {
    fn members(&self) -> Vec<Member> {
        self.chats
            .iter()
            .map(|(member, _)| member.clone())
            .collect()
    }

    fn moving(&self) -> Vec<crate::host::Moving> {
        self.chats
            .iter()
            .map(|(_, moving)| moving.clone())
            .collect()
    }

    fn workspace(&self) -> &str {
        self.chats
            .first()
            .map_or("", |(_, moving)| moving.workspace.as_str())
    }
}

/// What the window is told of a move it asked for to switch now.
#[derive(serde::Serialize, specta::Type)]
pub(crate) struct MoveNow {
    /// Each chat that moved, as the window draws it in its place (#3453): none where they did
    /// not move now.
    pub(crate) moved: Vec<MovedChat>,
    /// Why it did not move yet, in a sentence: it moves at a later pause.
    pub(crate) not_yet: Option<String>,
}

/// One chat a move started where it went, in the place of the run it left.
#[derive(serde::Serialize, specta::Type)]
pub(crate) struct MovedChat {
    /// The run it left.
    pub(crate) was: u32,
    pub(crate) chat: crate::OpenChat,
    /// What its tab says after the move: what came back that the person is to look at.
    pub(crate) notices: Vec<String>,
}

impl crate::planes::Held {
    /// What `open` is for a move to `to`, or why it does not move, in a sentence.
    fn mover(
        &self,
        open: &crate::chats::Open,
        to: &Place,
    ) -> Result<(Member, crate::host::Moving), String> {
        let chats = self.chats();
        let session = open.session;
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
        if to.is_some() && chats.started_unsandboxed(session) {
            return Err(
                "This chat runs without the sandbox, and a runner runs every chat in it: a move \
                 asks again on the runner, and that is not offered yet, so it stays here."
                    .to_owned(),
            );
        }
        Ok((
            Member {
                session,
                name: open.label.clone().unwrap_or_else(|| open.name.clone()),
            },
            crate::host::Moving {
                workspace,
                profile,
                conversation,
            },
        ))
    }

    /// **What chat `session` and the chats that move with it are, for a move to `to`**, or why
    /// they do not move, in a sentence. A workspace's chats share its clones, so every chat of
    /// the chat's workspace running where it runs moves with it, or none does (#3453).
    fn movers(&self, session: u32, to: &Place) -> Result<Movers, String> {
        let chats = self.chats();
        let open = chats.open_now();
        let asked = open
            .iter()
            .find(|open| open.session == session)
            .ok_or_else(|| "This chat is not open, so it was not moved.".to_owned())?;
        let from = chats.pool_of(session);
        if &from == to {
            return Err("This chat is there already.".to_owned());
        }
        let first = self.mover(asked, to)?;
        let workspace = first.1.workspace.clone();
        let mut group = vec![first];
        for other in open
            .iter()
            .filter(|other| other.session != session)
            .filter(|other| other.workspace.as_deref() == Some(workspace.as_str()))
            .filter(|other| chats.pool_of(other.session) == from)
        {
            let one = self.mover(other, to).map_err(|why| {
                let name = other.label.clone().unwrap_or_else(|| other.name.clone());
                format!(
                    "{name} also works in workspace {workspace} where this chat runs, and the \
                     chats of a workspace share its clones, so they move together or not at \
                     all, and none was moved. {name}: {why}"
                )
            })?;
            group.push(one);
        }
        Ok(Movers { chats: group, from })
    }

    /// **Starts the move of chat `session`, with every chat of its workspace running where it
    /// runs, to `to`** (#2048, #3453): checked now, then prepared once in the background while
    /// they keep working; they switch together once each is at a pause. Refused, in a sentence,
    /// where any of them does not move.
    pub(crate) fn move_chat(self: &Arc<Self>, session: u32, to: Place) -> Result<(), String> {
        let movers = self.movers(session, &to)?;
        if let Some(pool) = &to {
            let config = self.chats().pools_in().ok_or_else(|| {
                "This machine keeps no pool, so the chat was not moved.".to_owned()
            })?;
            goes_to_a_runner_of_yours(pool, || {
                purlis_core::poollogins::Bound::read(&config, pool)
            })?;
            // Each conversation is found before anything moves: a harness whose conversations
            // do not move says so now.
            for (member, chat) in &movers.chats {
                purlis_core::conversationmove::OfAChat {
                    root: self.root(),
                    workspace: &chat.workspace,
                    profile: &chat.profile,
                    conversation: &chat.conversation,
                }
                .find()
                .map_err(|why| {
                    if member.session == session {
                        format!("This chat was not moved: {why}.")
                    } else {
                        format!(
                            "{} moves with this chat, and was not moved, so neither was this \
                             one: {why}.",
                            member.name
                        )
                    }
                })?;
            }
        }
        if let (Some(pool), None) = (&movers.from, &to) {
            let config = self.chats().pools_in().ok_or_else(|| {
                "This machine keeps no pool, so the chat was not moved.".to_owned()
            })?;
            brings_work_back(pool, || purlis_core::poollogins::Bound::read(&config, pool))?;
        }
        let prepare: Box<dyn FnOnce() -> Result<(), String> + Send> = match &to {
            Some(pool) => {
                // Weak: a project let go of meanwhile is prepared for no more.
                let held = Arc::downgrade(self);
                let (pool, workspace) = (pool.clone(), movers.workspace().to_owned());
                let profiles = profiles_of(&movers.moving());
                Box::new(move || {
                    let held = held
                        .upgrade()
                        .ok_or_else(|| "The project was closed, so nothing moved.".to_owned())?;
                    prepare_on_a_runner(held.chats(), &pool, &workspace, &profiles)
                })
            }
            None => Box::new(|| Ok(())),
        };
        self.moves()
            .begin(movers.members(), movers.from.clone(), to, prepare)
    }

    /// **Moves chat `session` now, with every chat of its move, where it is prepared and each
    /// is at a pause between turns** (#2048, #3453): their work and conversations go where they
    /// move, their workspace's chats run there from now on, and a new run of each starts there
    /// on its conversation in its place; the runs they leave then end. Not yet (`None`)
    /// mid-turn (waiting on work of its own is mid-turn, #3887), while one waits on the person,
    /// while the person stops one, or while the move is prepared. A switch that fails leaves every one of them running where it was, and its move
    /// says why. Answers each run left and the run that took its place.
    pub(crate) fn move_chat_now(
        &self,
        session: u32,
        size: purlis_core::engine::Size,
    ) -> Result<Option<Vec<(u32, u32)>>, String> {
        let asks = self.asks();
        // A chat waiting on its own work is in its turn (#3887): a move would cut it off.
        let at_a_pause = |one: u32| {
            let glance = self.board().glance(one);
            glance.state == purlis_core::state::State::Waiting
                && !glance.in_its_turn()
                && !asks.asks.iter().any(|ask| ask.session == one)
                && !self.stopping().is_stopping(one)
        };
        let one = match self.moves().due(session, at_a_pause) {
            Due::NotMoving => return Err("This chat is not moving.".to_owned()),
            Due::NotYet(_) => return Ok(None),
            Due::Now(one) => one,
        };
        match self.switch(session, &one, size, &at_a_pause) {
            Ok(moved) => {
                self.moves().moved(session);
                Ok(Some(moved))
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
        at_a_pause: &impl Fn(u32) -> bool,
    ) -> Result<Vec<(u32, u32)>, String> {
        let movers = self.movers(session, &one.to)?;
        if movers.from != one.from {
            return Err("This chat moved since its move was asked for, so it stays.".to_owned());
        }
        let sessions = |members: &[Member]| {
            let mut sessions: Vec<u32> = members.iter().map(|one| one.session).collect();
            sessions.sort_unstable();
            sessions
        };
        if sessions(movers.members().as_slice()) != sessions(one.group.as_slice()) {
            return Err(format!(
                "The chats of workspace {} changed since the move was asked for, so none was \
                 moved: ask for it again.",
                movers.workspace()
            ));
        }
        let config = self
            .chats()
            .pools_in()
            .ok_or_else(|| "This machine keeps no pool, so the chat was not moved.".to_owned())?;
        let moving = movers.moving();
        let came = match (&one.from, &one.to) {
            (None, Some(pool)) => {
                // Asked again at the switch: a pool may have changed since the move was asked.
                goes_to_a_runner_of_yours(pool, || {
                    purlis_core::poollogins::Bound::read(&config, pool)
                })?;
                move_to_a_runner(self.chats(), pool, &moving)?;
                Vec::new()
            }
            (Some(pool), None) => {
                // Asked again at the switch: a pool may have changed since the move was asked.
                brings_work_back(pool, || purlis_core::poollogins::Bound::read(&config, pool))?;
                self.chats().sessions().move_out(pool, &moving)?
            }
            _ => return Err("A chat moves between this machine and a pool's runner.".to_owned()),
        };
        let runs_on = |place: &Place| {
            crate::dockerpools::run_chats_on(&config, self.root(), movers.workspace(), place)
        };
        runs_on(&one.to)?;
        // Every chat starts where it moved before any run where it was ends, so one that does
        // not start leaves them all where they were (#3453).
        let mut started: Vec<(u32, u32)> = Vec::new();
        for (member, _) in &movers.chats {
            match self.chats().start_again_held(member.session, size) {
                Ok(new) => started.push((member.session, new)),
                Err(why) => {
                    for (was, new) in &started {
                        self.chats().let_go_of_start(*was, *new);
                    }
                    let why = if movers.chats.len() == 1 {
                        why
                    } else {
                        format!("{} did not start there, so none moved: {why}", member.name)
                    };
                    return Err(did_not_start_said(
                        &why,
                        runs_on(&one.from),
                        &one.from,
                        &one.to,
                        movers.workspace(),
                    ));
                }
            }
        }
        // Asked again before any takes its place: a chat that began a turn while the work and
        // conversations moved would have it cut off (#3814). Every start is let go of, and every
        // chat runs where it was.
        if let Err(why) = still_at_a_pause(&one.group, at_a_pause) {
            for (was, new) in &started {
                self.chats().let_go_of_start(*was, *new);
            }
            return Err(did_not_start_said(
                &why,
                runs_on(&one.from),
                &one.from,
                &one.to,
                movers.workspace(),
            ));
        }
        let said = came_back_said(one.from.as_deref(), &came);
        let mut moved = Vec::new();
        for (was, new) in started {
            match self.chats().took_over(was, new) {
                Ok(new) => {
                    if was == session
                        && let Some(said) = &said
                    {
                        self.chats().note_on_its_tab(new, said.clone());
                    }
                    self.in_its_place_after_a_move(was, new);
                    moved.push((was, new));
                }
                Err(why) => tracing::warn!("purlis: chat {was} did not take its move ({why})"),
            }
        }
        Ok(moved)
    }
}

#[cfg(test)]
mod tests;

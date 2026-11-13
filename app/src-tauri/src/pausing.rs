// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Budgets and loops pause into Blocking asks** (#6015, ADR 0121 decision 20, ADR 0076 N4 and
//! N8): a chat that reaches its budget, or is found looping, is paused, and the person is asked.
//!
//! - **Paused means held still** (ADR 0076 §4): the chat's program, and its tasks at work below
//!   it, each have their whole process group stopped by their host ([`Chats::hold`]) until the
//!   person answers. A host before protocol 1.85 cannot hold one: its chat is asked about all
//!   the same, and goes on running meanwhile.
//! - **A budget** is a dispatch limit the person set and a task reached (`crate::overlimit`):
//!   `minutes-per-task`, or its session's `tokens-per-session`. Asked: Continue, which lets it
//!   go on past that limit; Raise the budget, which the window answers by opening Settings where
//!   the limit is, the ask answered and the chat going on once a look finds it within the limit
//!   again ([`let_go_of_budgets`]); or Stop, the stop the limit made before (asked for its report,
//!   then ended).
//! - **A loop** is `purlis_core::looping`'s: the same command failing again and again, its
//!   asks refused again and again, or blocks on the same target (R-206), heard from its hook
//!   lines ([`called`]) and from the stops the app writes ([`stopped`]), the person's Allow
//!   starting the block count again ([`allowed`]). Asked: It's fine, continue, which starts its
//!   counts again; or Stop, the window's Stop.
//!
//! **What moves a pause is a named cause** (ADR 0076): the app's own clock and counts, from the
//! chat's hooks and the person's answers, and the person's answer to the ask. Nothing a chat
//! prints. In memory: a relaunch starts every chat unpaused.
//!
//! [`Chats::hold`]: crate::chats::Chats::hold

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use purlis_core::dispatchlimits::Reached;
use purlis_core::eventlog::stops::{AskGroup, LoopPause, LoopRule, Stop};
use purlis_core::looping::{self, Loops};

use crate::asking::pauses::{CONTINUE, FINE, RAISE, STOP};
use crate::planes::{Held, PlaneId, Planes};

/// Why a chat is paused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Why {
    /// It reached a budget the person set.
    Budget(Reached),
    /// It was found looping.
    Looping(looping::Why),
}

/// One chat paused, as its ask shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paused {
    pub why: Why,
    /// When it was paused, in seconds since 1970.
    pub since: u32,
    /// The chats held still for it: itself, and its tasks at work below it then.
    pub held: Vec<u32>,
}

/// What one project holds.
#[derive(Debug, Default)]
struct Inner {
    /// By the chat asked about.
    paused: BTreeMap<u32, Paused>,
    loops: Loops,
    /// The budget each chat was let go on past with Continue: it is not paused for that limit
    /// again.
    continued: HashMap<u32, Reached>,
}

/// **A project's paused chats**, and the counts that may pause one.
#[derive(Debug, Default)]
pub struct Pauses {
    inner: Mutex<Inner>,
}

/// Whether `reached` is the same limit as `before`: the figure under it moves, the limit not.
fn same_limit(before: Reached, reached: Reached) -> bool {
    match (before, reached) {
        (
            Reached::Time { limit: a, .. } | Reached::Above { limit: a },
            Reached::Time { limit: b, .. } | Reached::Above { limit: b },
        )
        | (Reached::Tokens { limit: a, .. }, Reached::Tokens { limit: b, .. }) => a == b,
        _ => false,
    }
}

impl Pauses {
    fn inner(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Every chat paused now, by the chat asked about.
    pub fn now(&self) -> Vec<(u32, Paused)> {
        self.inner()
            .paused
            .iter()
            .map(|(chat, paused)| (*chat, paused.clone()))
            .collect()
    }

    /// Whether chat `chat` is held still by a pause: its own, or one above it.
    pub fn holds(&self, chat: u32) -> bool {
        self.inner()
            .paused
            .values()
            .any(|paused| paused.held.contains(&chat))
    }

    /// The chats paused at a budget, with the budget each reached.
    pub fn at_budgets(&self) -> Vec<(u32, Reached)> {
        self.inner()
            .paused
            .iter()
            .filter_map(|(chat, paused)| match paused.why {
                Why::Budget(reached) => Some((*chat, reached)),
                Why::Looping(_) => None,
            })
            .collect()
    }

    /// Chat `chat` is gone: nothing is kept of it, and no pause holds it.
    pub fn forget(&self, chat: u32) {
        let mut inner = self.inner();
        inner.paused.remove(&chat);
        for paused in inner.paused.values_mut() {
            paused.held.retain(|one| *one != chat);
        }
        inner.loops.forget(chat);
        inner.continued.remove(&chat);
    }

    /// **Chat `chat` is paused for `why` at `now`**, with `below`, its tasks at work, held with
    /// it: answers the chats to hold still. Nothing where a pause holds it already, or at a
    /// budget the person let it go on past.
    pub fn pause(&self, chat: u32, why: Why, now: i64, below: Vec<u32>) -> Option<Vec<u32>> {
        let mut inner = self.inner();
        let already = inner
            .paused
            .values()
            .any(|paused| paused.held.contains(&chat));
        let continued = match why {
            Why::Budget(reached) => inner
                .continued
                .get(&chat)
                .is_some_and(|before| same_limit(*before, reached)),
            Why::Looping(_) => false,
        };
        if already || continued {
            return None;
        }
        let chats: Vec<u32> = std::iter::once(chat).chain(below).collect();
        inner.paused.insert(
            chat,
            Paused {
                why,
                since: u32::try_from(now).unwrap_or(0),
                held: chats.clone(),
            },
        );
        Some(chats)
    }

    /// **Chat `chat`'s pause ends**: answers it, whose chats go on.
    pub fn let_go(&self, chat: u32) -> Option<Paused> {
        self.inner().paused.remove(&chat)
    }

    /// Chat `chat` ran `command`, which `failed` or not: the loop it makes, held to `limits`.
    pub fn ran(
        &self,
        chat: u32,
        command: &str,
        failed: bool,
        limits: &looping::Limits,
    ) -> Option<looping::Why> {
        self.inner().loops.ran(chat, command, failed, limits)
    }

    /// The person answered one of chat `chat`'s asks, `refused` or not: the loop it makes.
    pub fn answered(
        &self,
        chat: u32,
        refused: bool,
        limits: &looping::Limits,
    ) -> Option<looping::Why> {
        self.inner().loops.answered(chat, refused, limits)
    }

    /// Chat `chat` had `stop` written, under `call` where it was held under one: the block loop
    /// it makes (R-206), a call counted once (Q253).
    pub fn stopped(
        &self,
        chat: u32,
        stop: &purlis_core::eventlog::stops::Stop,
        call: Option<&str>,
    ) -> Option<looping::Why> {
        self.inner().loops.stopped_under(chat, stop, call)
    }

    /// The person allowed a sandbox ask of chat `chat`'s: its block count starts again (R-211).
    pub fn allowed(&self, chat: u32) {
        self.inner().loops.allowed(chat);
    }

    /// Chat `call.chat`'s hook line said it made or ended `call`, reported from outside its
    /// reach where `verified`: the block loop it makes (R-206, R-210).
    pub fn called(
        &self,
        call: &purlis_core::hookwire::ToolCall,
        verified: bool,
    ) -> Option<looping::Why> {
        self.inner().loops.called(call.chat, call, verified)
    }

    /// **The person's answer `option` to chat `chat`'s pause**: its pause ends, and what is
    /// left to do is answered. Continue keeps that budget from pausing it again; It's fine and
    /// Stop on a loop start its counts again.
    pub fn answer(&self, chat: u32, option: &str) -> Result<Answered, String> {
        let mut inner = self.inner();
        let Some(paused) = inner.paused.get(&chat).cloned() else {
            return Err("This chat is no longer paused.".to_owned());
        };
        let then = match (paused.why, option) {
            (Why::Budget(reached), CONTINUE) => {
                inner.continued.insert(chat, reached);
                Then::GoOn
            }
            (Why::Budget(reached), STOP) => Then::StopAt(reached),
            // Answered, and still held: it goes on once a look finds it within the limit the
            // person raises ([`let_go_of_budgets`]).
            (Why::Budget(_), RAISE) => {
                return Ok(Answered {
                    held: Vec::new(),
                    then: Then::Raise,
                });
            }
            (Why::Looping(_), FINE) => {
                inner.loops.fine(chat);
                Then::GoOn
            }
            (Why::Looping(_), STOP) => {
                inner.loops.fine(chat);
                Then::Stop
            }
            _ => return Err(format!("{option:?} is no answer to this ask.")),
        };
        inner.paused.remove(&chat);
        Ok(Answered {
            held: paused.held,
            then,
        })
    }
}

/// What the person's answer to a pause leaves to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answered {
    /// The chats it held still, which go on.
    pub held: Vec<u32>,
    pub then: Then,
}

/// What follows a pause's end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Then {
    /// Nothing: it goes on.
    GoOn,
    /// Nothing yet: the person raises the budget in Settings, and it stays held until a look
    /// finds it within the new limit ([`let_go_of_budgets`]).
    Raise,
    /// It is stopped as the budget's limit stops it: asked for its report, then ended.
    StopAt(Reached),
    /// It is stopped as the window's Stop stops it.
    Stop,
}

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

/// Holds `chats` still, or lets them go on, and tells the window its asks moved.
fn hold(held: &Held, chats: &[u32], still: bool) {
    for one in chats {
        held.chats().hold(*one, still);
    }
    held.chats().asks_may_have_moved();
}

/// The group of ask a pause for `why` is (R-229).
fn group_of(why: Why) -> AskGroup {
    match why {
        Why::Budget(_) => AskGroup::Budget,
        Why::Looping(_) => AskGroup::LoopPause,
    }
}

/// **A loop pause as the event log writes it** (R-232): the rule, and at how many times.
fn loop_pause_of(why: looping::Why) -> LoopPause {
    let (rule, times) = match why {
        looping::Why::Failing { times } => (LoopRule::Failing, times),
        looping::Why::Refused { times } => (LoopRule::Refused, times),
        looping::Why::Blocked { times } => (LoopRule::Blocked, times),
    };
    LoopPause { rule, times }
}

/// **Pauses chat `chat` for `why`, at `now`**, and asks the person: it and its tasks at work
/// below it are held still. Answers whether it was paused ([`Pauses::pause`]). A pause for a
/// loop is written as one (R-232), and its ask as raised (R-229), for the chat's Chat log.
pub(crate) fn pause(held: &Held, chat: u32, why: Why, now: i64) -> bool {
    let below = crate::handoff::at_work_below(held, chat);
    let Some(chats) = held.pauses().pause(chat, why, now, below) else {
        return false;
    };
    if let Why::Looping(found) = why {
        crate::chatlog::record(held.hooks(), chat, Stop::LoopPause(loop_pause_of(found)));
    }
    crate::chatlog::raised(held.hooks(), chat, group_of(why));
    hold(held, &chats, true);
    true
}

/// **Lets go of every chat paused at a budget that `still` says it is no longer at** (#6015):
/// the person raised the limit, or took it away. The clock's look asks, from what it found.
pub(crate) fn let_go_of_budgets(held: &Held, still: impl Fn(u32, Reached) -> bool) {
    for (chat, reached) in held.pauses().at_budgets() {
        if !still(chat, reached)
            && let Some(paused) = held.pauses().let_go(chat)
        {
            hold(held, &paused.held, false);
        }
    }
}

/// **Chat `chat` ran `command`, which `failed` or not**, as its hooks said: a loop of the same
/// failing command pauses it. Held to the thresholds Stuck? read as its turn began.
pub(crate) fn ran(held: &Held, chat: u32, command: &str, failed: bool) {
    let limits = looping::Limits::of(&held.noticed().limits_now(chat));
    if let Some(why) = held.pauses().ran(chat, command, failed, &limits) {
        pause(held, chat, Why::Looping(why), now());
    }
}

/// **The person answered one of chat `chat`'s asks, and `refused` it or not**: refusals piling
/// up with no allow between pause it.
pub(crate) fn answered(held: &Held, chat: u32, refused: bool) {
    // Read now: an answer is rare, and a chat that asks is not working, so Stuck? holds none.
    let limits = looping::Limits::of(&crate::stuck::limits_of(held, chat));
    if let Some(why) = held.pauses().answered(chat, refused, &limits) {
        pause(held, chat, Why::Looping(why), now());
    }
}

/// **Chat `chat` had `stop` written** by the app (a held connection's block, a merge's hold),
/// under `call`, the harness's id for the call it was held under, where one was heard: three
/// blocks on the same target pause it (R-206), each call counted once (Q253); a hold never
/// counts (R-215).
pub(crate) fn stopped(
    held: &Held,
    chat: u32,
    stop: &purlis_core::eventlog::stops::Stop,
    call: Option<&str>,
) {
    if let Some(why) = held.pauses().stopped(chat, stop, call) {
        pause(held, chat, Why::Looping(why), now());
    }
}

/// **The person allowed a sandbox ask of chat `chat`'s** (R-211, Q253), on any of its paths: a
/// held connection's Notice, the question for several tasks, the chooser, an ask answered late.
/// Its block count starts again, as an Allow of a hook's ask starts it ([`answered`]).
pub(crate) fn allowed(held: &Held, chat: u32) {
    held.pauses().allowed(chat);
}

/// **Chat `call.chat`'s hook line said it made or ended `call`**: a guard's deny is a block
/// toward the loop, counted only where its chat's hooks report from outside its reach (R-209,
/// Q235), the harness's own check's never (R-208), and a different call that came back resets
/// the count (R-210).
pub(crate) fn called(held: &Held, call: &purlis_core::hookwire::ToolCall) {
    let verified = held.chats().reports_from_outside_its_reach(call.chat);
    if let Some(why) = held.pauses().called(call, verified) {
        pause(held, call.chat, Why::Looping(why), now());
    }
}

/// **The person's answer to chat `chat`'s pause**, `option` one of its ask's: Continue or It's
/// fine, continue let it go on; Stop lets it go on only to stop it, as the budget's limit or
/// the window's Stop stops it. Raise the budget answers it and leaves it held, as the window
/// opens Settings where the limit is (Q261).
pub(crate) fn answer(held: &Arc<Held>, chat: u32, option: &str) -> Result<(), String> {
    let group = held
        .pauses()
        .now()
        .into_iter()
        .find(|(paused, _)| *paused == chat)
        .map(|(_, paused)| group_of(paused.why));
    let answered = held.pauses().answer(chat, option)?;
    // R-229: the person's answer, from the window (`answer_pause` is the window's alone).
    if let Some(ask) = group {
        let allowed = matches!(answered.then, Then::GoOn | Then::Raise);
        crate::chatlog::answered_in_the_window(held.hooks(), chat, ask, allowed);
    }
    hold(held, &answered.held, false);
    match answered.then {
        Then::GoOn | Then::Raise => Ok(()),
        // A task is asked for its report, then ended, as the limit stopped it before.
        Then::StopAt(reached) if crate::stopping::stop_at_a_limit(held, chat, reached) > 0 => {
            Ok(())
        }
        Then::StopAt(_) | Then::Stop => crate::stopping::stop_as_asked(held, chat),
    }
}

/// **Answers chat `session`'s pause with `option`** (#6015): Continue, Raise the budget, It's
/// fine, continue, or Stop, as its ask in Asks offers them.
// Its plane is a `PlaneId` the registry vouches for. Not a doc comment, because the generated
// bindings carry those. **The window's alone, over Tauri's IPC**
// (`purlis_session_protocol::ui::WINDOW_ONLY`): no link serves it and no hook line reaches it.
#[tauri::command]
#[specta::specta]
pub fn answer_pause(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    session: u32,
    option: String,
) -> Result<(), String> {
    answer(&planes.held(&plane)?, session, &option)
}

#[cfg(test)]
mod tests;

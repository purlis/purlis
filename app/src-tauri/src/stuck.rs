// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Stuck? and work left behind** (ADR 0121 decisions 17 and 19, #6014): the app's half of two
//! facts a chat's mark reads. The rules are the core's (`purlis_core::stuck`,
//! `purlis_core::leftwork`); this feeds them what the app already hears, and tells the window
//! what they answer ([`ChatNoticed`], as a `chat-events` telling).
//!
//! - **A state change** ([`Noticed::moved`]): every `Moved` the board tells, a program's exit
//!   included. Into working, the chat's thresholds are read once and **a timer for that turn**
//!   sleeps until the next rule of time can come true (`Watch::due`), so nothing is polled for
//!   a chat at rest. Out of working, its folder is asked once whether it holds uncommitted
//!   work, off every lock, and the answer is kept until the next change.
//! - **Activity** ([`heard`], [`ran`]): a tool beginning or coming back, a turn's output, a
//!   command that ran and whether it failed, as its hooks said. Never its terminal's bytes.
//! - **"It's fine"** ([`stuck_fine`]): clears Stuck? until the chat changes state.
//! - **A mode purlis didn't set** ([`attendance_heard`], T1-44): what the one attendance fact
//!   the app holds for a Claude Code chat heard from its armed hook, a standing fault its pane
//!   says (R-243).
//!
//! **No notification is sent from here** (#6005 decides that). In memory only.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError, Weak};
use std::time::Duration;

use purlis_core::stuck::{Limits, Watch};

use crate::hooks::Moved;
use crate::planes::{Held, PlaneId, Planes};

/// What the window is told of one chat, each time either fact changes.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct ChatNoticed {
    pub plane: PlaneId,
    pub session: u32,
    /// Why it reads Stuck?, in purlis's words, or none.
    pub stuck: Option<String>,
    /// Whether its folder held uncommitted work when it last stopped working; none where that
    /// is not known.
    pub left_work: Option<bool>,
    /// The mode purlis didn't set that it reported, once one made it unattended (R-243,
    /// T1-44): a standing fault its pane says in a Notice, never an ask. None on every harness
    /// but Claude Code.
    pub mode: Option<ModeDiffers>,
}

/// **A Claude Code chat reported a mode purlis didn't set** (R-243): each mode as Claude Code
/// names it (`default`, `acceptEdits`, `plan`, `auto`, `bypassPermissions`), or `unknown` for a
/// string nobody measured ([`purlis_core::attendance::ModeDiffers`]).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct ModeDiffers {
    /// The mode purlis started it in.
    pub set: String,
    /// The mode its armed hook reported.
    pub reported: String,
}

impl From<purlis_core::attendance::ModeDiffers> for ModeDiffers {
    fn from(differs: purlis_core::attendance::ModeDiffers) -> Self {
        Self {
            set: differs.set.as_str().to_owned(),
            reported: differs.reported.as_str().to_owned(),
        }
    }
}

/// Told each [`ChatNoticed`]. The telling carries its project.
pub type Teller = Arc<dyn Fn(ChatNoticed) + Send + Sync + 'static>;

/// The two facts as last told, per chat.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Said {
    stuck: Option<String>,
    left_work: Option<bool>,
    mode: Option<purlis_core::attendance::ModeDiffers>,
}

/// How long a turn's timer sleeps at most before it looks at the clock again: the machine's
/// own clock, so a rule that came true while it slept is told as it wakes.
const LOOKS_AT_THE_CLOCK_EVERY: Duration = Duration::from_secs(30);

/// What one project holds.
pub struct Noticed {
    plane: PlaneId,
    /// The project, once it is held: what a state change reads the chat's place from.
    held: OnceLock<Weak<Held>>,
    tell: OnceLock<Teller>,
    watch: Mutex<Watch>,
    /// Each working chat's thresholds, read once as its turn began.
    limits: Mutex<HashMap<u32, Limits>>,
    said: Mutex<HashMap<u32, Said>>,
}

fn held_lock<T>(lock: &Mutex<T>) -> MutexGuard<'_, T> {
    lock.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The thresholds where none was read: each limit's default.
fn unset() -> Limits {
    use purlis_core::dispatchlimits::Limit;
    let of = |limit: Limit| limit.when_unset().unwrap_or(1);
    Limits {
        silent_minutes: of(Limit::StuckSilentMinutes),
        failing_times: of(Limit::StuckFailingTimes),
        long_minutes: of(Limit::StuckLongMinutes),
    }
}

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

impl Noticed {
    pub fn new(plane: PlaneId) -> Self {
        Self {
            plane,
            held: OnceLock::new(),
            tell: OnceLock::new(),
            watch: Mutex::new(Watch::default()),
            limits: Mutex::new(HashMap::new()),
            said: Mutex::new(HashMap::new()),
        }
    }

    /// The project is held, and its tellings go to `tell`.
    pub fn held_as(&self, held: Weak<Held>, tell: Teller) {
        let _ = self.held.set(held);
        let _ = self.tell.set(tell);
    }

    /// **The board told `moved`**: a change into or out of working starts that chat afresh,
    /// and what it owes is done off this thread, which may be the board's.
    pub fn moved(self: &Arc<Self>, moved: &Moved) {
        let working = moved.state == "running" && !moved.needs_you && moved.asking != Some(true);
        let chat = moved.session;
        let since = {
            let mut watch = held_lock(&self.watch);
            if !watch.state(chat, working, now()) {
                return;
            }
            watch.working_since(chat)
        };
        let noticed = Arc::clone(self);
        let spawned = std::thread::Builder::new()
            .name("purlis-stuck".to_owned())
            .spawn(move || match since {
                Some(since) => noticed.watch_turn(chat, since),
                None => noticed.stopped_working(chat),
            });
        if let Err(why) = spawned {
            tracing::warn!("purlis: chat {chat}'s Stuck? watch had no thread ({why})");
        }
    }

    /// Chat `chat` began the turn that began at `since`: its thresholds are read, and it is
    /// looked at each time a rule of time can come true, until that turn ends.
    fn watch_turn(&self, chat: u32, since: i64) {
        let limits = self
            .held
            .get()
            .and_then(Weak::upgrade)
            .map_or_else(unset, |held| limits_of(&held, chat));
        held_lock(&self.limits).insert(chat, limits);
        loop {
            let now = now();
            let due = {
                let watch = held_lock(&self.watch);
                if watch.working_since(chat) != Some(since) {
                    return;
                }
                watch.due(chat, now, &limits)
            };
            self.look(chat);
            let Some(due) = due else {
                // Nothing of time can come true again this turn: only activity or "It's
                // fine" changes it, and each looks for itself.
                return;
            };
            let left = Duration::from_secs(u64::try_from(due - now).unwrap_or(0));
            std::thread::sleep(left.min(LOOKS_AT_THE_CLOCK_EVERY));
        }
    }

    /// Chat `chat` stopped working: it is not stuck, and its folder is asked whether it left
    /// work behind.
    fn stopped_working(&self, chat: u32) {
        held_lock(&self.limits).remove(&chat);
        let Some(held) = self.held.get().and_then(Weak::upgrade) else {
            return;
        };
        let Some(open) = held
            .chats()
            .open_now()
            .into_iter()
            .find(|open| open.session == chat)
        else {
            // Closed: nothing is kept of it.
            self.forget(chat);
            return;
        };
        let left_work = open.cwd.as_deref().and_then(purlis_core::leftwork::left_in);
        self.tell_if_changed(chat, |said| said.left_work = left_work);
        self.look(chat);
    }

    /// **What the window was last told of chat `chat`** (#6235): whether it reads Stuck?, and
    /// whether its folder holds work left behind, where that is known. A read.
    pub fn told_of(&self, chat: u32) -> (bool, Option<bool>) {
        held_lock(&self.said)
            .get(&chat)
            .map_or((false, None), |said| (said.stuck.is_some(), said.left_work))
    }

    /// Chat `chat` began or got back a command, or its turn put something out.
    pub fn heard(&self, chat: u32) {
        held_lock(&self.watch).heard(chat, now());
        self.look(chat);
    }

    /// Chat `chat` ran `command`, which `failed` or not.
    pub fn ran(&self, chat: u32, command: &str, failed: bool) {
        held_lock(&self.watch).ran(chat, command, failed, now());
        self.look(chat);
    }

    /// Chat `chat`'s attendance fact names `differs`, the mode purlis didn't set that it
    /// reported (R-243): the window is told where that changed.
    pub fn mode_differs(&self, chat: u32, differs: purlis_core::attendance::ModeDiffers) {
        self.tell_if_changed(chat, |said| said.mode = Some(differs));
    }

    /// **"It's fine"**: chat `chat` is not stuck until it changes state.
    pub fn fine(&self, chat: u32) {
        held_lock(&self.watch).fine(chat);
        self.look(chat);
    }

    /// Chat `chat` has closed: nothing is kept of it.
    pub fn forget(&self, chat: u32) {
        held_lock(&self.watch).forget(chat);
        held_lock(&self.limits).remove(&chat);
        held_lock(&self.said).remove(&chat);
    }

    /// The thresholds chat `chat` is held to now: those read as its turn began, or each
    /// limit's default where it is not working.
    pub fn limits_now(&self, chat: u32) -> Limits {
        held_lock(&self.limits)
            .get(&chat)
            .copied()
            .unwrap_or_else(unset)
    }

    /// What every chat of the project last told, for a window that opens.
    pub fn all(&self) -> Vec<ChatNoticed> {
        held_lock(&self.said)
            .iter()
            .map(|(session, said)| self.noticed(*session, said))
            .collect()
    }

    fn noticed(&self, session: u32, said: &Said) -> ChatNoticed {
        ChatNoticed {
            plane: self.plane.clone(),
            session,
            stuck: said.stuck.clone(),
            left_work: said.left_work,
            mode: said.mode.map(ModeDiffers::from),
        }
    }

    /// Asks the watch whether chat `chat` reads Stuck? now, and tells the window if that
    /// changed.
    fn look(&self, chat: u32) {
        let limits = self.limits_now(chat);
        // A chat paused into an ask is held still, and asked about already (#6015).
        let paused = self
            .held
            .get()
            .and_then(Weak::upgrade)
            .is_some_and(|held| held.pauses().holds(chat));
        let stuck = held_lock(&self.watch)
            .stuck(chat, now(), &limits)
            .filter(|_| !paused)
            .map(|why| why.said());
        self.tell_if_changed(chat, |said| said.stuck = stuck);
    }

    fn tell_if_changed(&self, chat: u32, change: impl FnOnce(&mut Said)) {
        let told = {
            let mut all = held_lock(&self.said);
            let said = all.entry(chat).or_default();
            let was = said.clone();
            change(said);
            (*said != was).then(|| self.noticed(chat, said))
        };
        if let (Some(told), Some(tell)) = (told, self.tell.get()) {
            tell(told);
        }
    }
}

/// The thresholds in force for chat `chat`: the project's, its workspace's or its persona's
/// (`purlis_core::dispatchlimits`).
pub(crate) fn limits_of(held: &Held, chat: u32) -> Limits {
    let open = held.chats().open_now();
    let open = open.iter().find(|open| open.session == chat);
    Limits::of(&purlis_core::dispatchlimits::of(
        held.root(),
        open.and_then(|open| open.workspace.as_deref()),
        open.and_then(|open| open.persona.as_deref()),
        None,
    ))
}

/// Chat `chat` began or got back a command, or its turn put something out.
pub fn heard(held: &Held, chat: u32) {
    held.noticed().heard(chat);
}

/// **A hook report of chat `report.chat`, after the board took it, heard by its attendance
/// fact** (T1-18, T1-44): the one fact `chats` holds for the chat, which A4 reads
/// ([`crate::chats::Chats::heard_attendance`]), and `noticed` told the mode purlis didn't set
/// that it reported, read from that same fact (R-243). Only Claude Code's armed hook is read
/// for its mode, so every other harness's chat is told none.
pub fn attendance_heard(
    chats: &crate::chats::Chats,
    noticed: &Noticed,
    report: &purlis_core::hookwire::Report,
) {
    chats.heard_attendance(report);
    if let Some(differs) = chats
        .attendance_of(report.chat)
        .and_then(purlis_core::attendance::Fact::mode_differs)
    {
        noticed.mode_differs(report.chat, differs);
    }
}

/// Chat `chat` ran `command`, which `failed` or not.
pub fn ran(held: &Held, chat: u32, command: &str, failed: bool) {
    held.noticed().ran(chat, command, failed);
    // And toward a loop, which pauses it (#6015).
    crate::pausing::ran(held, chat, command, failed);
}

/// What each chat of a project last told: Stuck? and work left behind.
#[tauri::command]
#[specta::specta]
pub fn chats_noticed(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
) -> Result<Vec<ChatNoticed>, String> {
    Ok(planes.held(&plane)?.noticed().all())
}

/// **"It's fine"**: chat `session` is not stuck until it changes state.
#[tauri::command]
#[specta::specta]
pub fn stuck_fine(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    session: u32,
) -> Result<(), String> {
    planes.held(&plane)?.noticed().fine(session);
    Ok(())
}

#[cfg(test)]
mod tests;

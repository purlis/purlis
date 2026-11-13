// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **System notifications: for asks, and for two updates** (#1694, spec #1688, I-7; #1989, Q66;
//! #6005).
//!
//! What the person is interrupted for is what blocks work: an **ask** (a permission prompt, a
//! dispatch grant, a sandbox host, a prompt in a harness's terminal, a chat whose turn ended
//! on them). An **update** is information, and the registry does not list it
//! (`asking::only_updates`, the registry's own split), so every item it lists is an ask.
//!
//! **Should know notifies twice only** (#6005, ADR 0121 decision 3; [`Rules::ended`]): a task
//! the person asked for that **fails**, named with its project and its report's first line, and
//! one that **finishes after more than five minutes of work** (`notifications.finishedAfter`,
//! [`FINISHED_AFTER`]) while the person is away from every window. Each is read off the task's
//! dispatch record ([`Finish`], `finished::finishes`) and told once; what had ended before the
//! project was first read is never told. A task a chat dispatched reports to that chat (ADR
//! 0107, #6036) and tells the person nothing. Every other update stays silent in Updates, where
//! the bell counts it. While Quiet these are held, and counted in the one notification after it
//! ("While you were quiet: 2 finished, 1 failed"). A click opens the task's line in Updates in
//! place ([`Landing::update`], #5651).
//!
//! **What is asked is what the ✋ counts** (`asking::counted`): the registry's Blocking asks
//! (ADR 0121 decision 2), with what Snooze all put away left out (decision 14). It is read
//! again whenever one of its sources may have moved ([`poke`]): a chat moving on the board,
//! its hooks' asks, a dispatch held, a block heard — the events the window reads its own list
//! again on. An ask is considered once, the first time it is read ([`Rules`]); the notifier
//! keeps no store of its own past the keys it has seen and the notifications standing, so it
//! agrees with the ✋ by construction. A chat that waits on its background agents is not in
//! the queue (#1626), so it is no ask and sends nothing. A chat's own wait is keyed by the
//! chat, so the board's edge into the queue renews it ([`moved`]).
//!
//! **One notification per chat** (#6003, ADR 0121 decision 12): it names the chat (`steward
//! 12 › #3046 drill`), its project and what kind of ask it is ("smart-ide: Asks your
//! permission, and 2 more"), never the ask's words ([`kind_said`]). The chat's next ask updates
//! it in place, and it is taken away once every ask of the chat is answered. A name is the
//! chat's, so it is data: plain text, capped ([`plain`]).
//!
//! **A burst of chats is one notification** ([`TOGETHER`], [`MANY`]; settles #2108): the
//! first chat to ask is told at once, and each that follows within the window of the one
//! before waits for the window to close. Then, when three chats or more of the burst still
//! wait, one notification counts them all, across projects ("3 chats in 2 projects need you"),
//! and takes over from the first's own; fewer are each told with their own. Three or more in
//! one read are told together at once.
//!
//! **The platform's notifications are one layer under these rules** (`asknotify_center.rs`,
//! #6059). macOS replaces a notification in place and takes one away through its own
//! notification center (`asknotify_macos.rs`); Linux and the BSDs through the notification
//! plugin, an update being the old one taken away and the new one shown. **Where a
//! notification cannot be replaced** ([`shown_here`]) — Windows, which purlis does not ship
//! for, and a macOS build that is no app bundle (`tauri dev`) — a notification shown stays
//! until the person clears it: an update that tells of no new chat is not shown, so a chat is
//! not told twice, and a removal does nothing. The one standing still lands on what waits, or
//! on nothing once all of it was answered.
//!
//! **No notification has an Allow or Deny of its own**: it is shown with no action type, since
//! answering from it would bypass the settle guard (ADR 0121 decision 12).
//!
//! **No sound, but the Expiring one** (#6110, decision 12; [`Rules::expiring`]): with the
//! person's "Sound for an expiring ask" on (`notifications.expiringSound`, off by default), an
//! Expiring ask within [`SOUND_BEFORE`] of its end shows its chat's notification again with the
//! system's own notification sound, once per ask, never while Quiet or while the person looks
//! at its chat ([`should_sound`]).
//!
//! **Nothing is sent while it is Quiet** (#6017, ADR 0121 decision 13): in the person's quiet
//! hours ([`QuietHours`], Settings › You › Notifications, kept in the layout file), while
//! "Quiet now" is on, and while they type in a chat, until [`TYPING_QUIET`] after their last key
//! ([`QuietSources`]). An ask asked then waits in Asks, and the ✋ still counts it. When Quiet
//! ends, every window is told what it held ([`QuietEnded`], [`QUIET_CHANGED`]) for the summary at
//! the top of Asks, and one notification names what still waits unseen if the person is away
//! from every window ([`Rules::quiet`]). macOS's own Focus holds purlis's notifications as it
//! holds every app's; whether purlis may show notifications at all is the system's own
//! notification settings' to say.
//!
//! **Nothing is sent while the person is looking** ([`Looking`], decision 11): the window
//! holding the ask's project is on screen, focused and has that project in front, and it
//! shows either the chat itself (`Chats::looks_at`, #1486, #1489) or that project's Asks
//! ([`InboxOpen`], which the window says). Every question unanswered reads as "not looking",
//! so a notification is sent rather than held back: one not needed costs a glance, one needed
//! and not sent costs a chat sitting unanswered.
//!
//! **A click brings the chat forward with its ask's row open.** The app is not told of a click
//! but that it is brought forward, so each window holding a project the notification counts is
//! sent [`NOTIFIED`] with its chats ([`landings`]), and lands on the first still waiting the
//! next time it comes to the front (`askNotices.ts`). A click with every window hidden (the
//! close button hides the main window, ADR 0025) brings none forward by itself: macOS says it
//! as a reopen, and the window holding the project the latest notification was about is shown
//! ([`reopened`]), where the focus it gets lands it.

use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use tauri::Manager;

use crate::asking::{AskSource, Shown};
use crate::planes::{PlaneId, Planes, Showing};

/// The event the window holding a project is sent when a notification about one of its chats
/// went out: [`Landing`].
pub const NOTIFIED: &str = "asks-notified";

/// How close together chats come to wait to be one burst: each within this of the one before.
pub const TOGETHER: Duration = Duration::from_secs(5);

/// The most characters of a chat's name, or of a title, a notification says.
pub const MOST_NAME: usize = 60;

/// The most chats a notification names; the rest it counts.
const MOST_NAMED: usize = 3;

/// How long a burst of moves is let settle before the registry is read, as the window does.
const SETTLE: Duration = Duration::from_millis(150);

/// A chat, as a notification counts it: its project and its number there. Every project
/// numbers its chats from one, so the number alone is not a chat.
pub type Chat = (PlaneId, u32);

/// **What the notifier does to the system's notifications**: one shown, or replaced in place,
/// or one taken away.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Act {
    /// Show `Notice`; one with an id already shown replaces it ([`Notice::replaces`]).
    Show(Notice),
    /// Take notification `id` away: every chat it was about was answered, or another took
    /// over saying it.
    Remove(i32),
}

/// One notification to show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    /// Its id, the same for as long as it stands: an update of it carries the same one.
    pub id: i32,
    /// Whether it updates the notification of that id, already shown.
    pub replaces: bool,
    /// Whether it tells of a chat no notification standing told of. Where the system cannot
    /// replace a notification ([`shown_here`]), an update that tells of none is not shown.
    pub news: bool,
    /// Every chat it counts, the longest waiting first: a click lands on the first still
    /// waiting.
    pub chats: Vec<Chat>,
    /// Who is asking, as its chain (I-9), or how many chats are.
    pub title: String,
    /// Its project, and what kind of ask it is ([`kind_said`]) and how many more; or the chats
    /// it counts, by name.
    pub body: String,
    /// The update it tells of, by its key in Updates (`task:<dispatch id>`), where it tells of
    /// a task that ended ([`Rules::ended`]) and not of asks: a click opens that line in place.
    pub update: Option<String>,
    /// Whether it plays the system's own notification sound: only where it is shown again
    /// because one of its chat's Expiring asks is nearly out ([`Rules::expiring`], #6110).
    pub sound: bool,
}

/// Where a click on a notification lands, as the window holding `plane` is sent it.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Landing {
    pub plane: PlaneId,
    /// The first chat it counted in this project.
    pub session: u32,
    /// Every chat it counted in this project, in order: a window lands on the first still
    /// waiting.
    pub sessions: Vec<u32>,
    /// The update it told of, by its key in Updates, where it told of a task that ended: the
    /// window opens that line in place (#5651) rather than an ask.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub update: Option<String>,
}

/// One read of a project's asks, as [`Rules::heard`] takes it.
pub struct Heard<'a> {
    pub plane: &'a PlaneId,
    /// What the ✋ counts of it: the registry's Blocking asks, snoozed ones put apart
    /// (`asking::counted`).
    pub asks: &'a [Shown],
    /// Whether the person is looking at a chat's asks now ([`Looking`]).
    pub looking: &'a dyn Fn(u32) -> bool,
    /// Whether it is the person's quiet hours now: asked only when there is something to send.
    pub quiet: &'a dyn Fn() -> bool,
    pub now: Instant,
}

/// **A task that ended**, as [`Rules::ended`] reads it off its dispatch record (#6005).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finish {
    /// Its dispatch record's id: its line in Updates is `task:<id>` (`inboxUpdates.ts`).
    pub id: String,
    /// The chat that asked for it, by its number in the project: what a click brings forward.
    pub session: u32,
    /// The task's name, as its row had it.
    pub name: String,
    /// Whether it came to nothing: failed, blocked, ended without a report, did not start
    /// (`taskBuckets.ts`, `CAME_TO_NOTHING`).
    pub failed: bool,
    /// Whether the person asked for it themselves, from a chat's tab. One a chat dispatched
    /// reports to that chat (ADR 0107, #6036), and is the person's to hear of only there.
    pub by_person: bool,
    /// How long it worked: from its start to its end.
    pub worked: Duration,
    /// Its report, as written: a failure's notification says its first line alone.
    pub report: String,
}

/// One read of the tasks of a project that stand ended, as [`Rules::ended`] takes it.
pub struct Ended<'a> {
    pub plane: &'a PlaneId,
    /// Every task of it that stands ended now, in any order.
    pub finishes: &'a [Finish],
    /// How long a task works before its finish is told while the person is away: the setting,
    /// five minutes unless they changed it ([`FINISHED_AFTER`]).
    pub long: Duration,
    /// Whether the person is away from every window of the app as it is read.
    pub away: bool,
    /// Whether the person is looking at a chat now ([`Looking`]).
    pub looking: &'a dyn Fn(u32) -> bool,
    /// Whether it is Quiet now.
    pub quiet: bool,
    /// Which of the two the person lets notify ([`Sources`]).
    pub sources: Sources,
}

/// **Which Should-know notifications are on** (#6019, ADR 0121 decision 22): a failure of a task
/// the person asked for, and a long finish while they are away, each turned off on its own in
/// Settings › You › Attention. A Blocking ask is no source here: it always notifies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sources {
    /// A task that failed notifies: the update kind `task-failed`.
    pub failed: bool,
    /// A long finish while away notifies: the update kind `task-done`.
    pub finished: bool,
}

impl Default for Sources {
    fn default() -> Self {
        Self {
            failed: true,
            finished: true,
        }
    }
}

impl Sources {
    /// **Which are on, as a layout document keeps it**: `notifications.off`, the update kinds the
    /// person turned off (`app/src/notificationPrefs.ts`). A kind it does not know is no source,
    /// and anything but a list turns none off.
    pub fn of_layout(document: &serde_json::Value) -> Self {
        let Some(off) = document
            .get("notifications")
            .and_then(|held| held.get("off"))
            .and_then(serde_json::Value::as_array)
        else {
            return Self::default();
        };
        let turned_off = |kind: &str| off.iter().any(|one| one.as_str() == Some(kind));
        Self {
            failed: !turned_off("task-failed"),
            finished: !turned_off("task-done"),
        }
    }
}

/// How long a task works before its finish notifies while the person is away, until they say
/// otherwise (`notifications.finishedAfter`, in minutes; ADR 0121 decisions 3 and 22).
pub const FINISHED_AFTER: Duration = Duration::from_secs(5 * 60);

/// **How near its end an Expiring ask sounds**, in seconds (#6110, ADR 0121 decision 12): its
/// last 15. The shortest countdown a person can still beat is a permission prompt's, about 58
/// seconds, and its row counts in seconds below a minute; 15 is time enough to reach the
/// window and answer. An ask first heard with less left than this sounds as it is heard.
pub const SOUND_BEFORE: u32 = 15;

/// **Whether an Expiring ask sounds now** (#6110, ADR 0121 decisions 11, 12 and 13): only with
/// the sound `on` (Settings › You › Attention, off unless the person turns it on), never while
/// it is `quiet` or the person is `looking` at its chat, and once per ask; then only while it
/// is Expiring (`Shown::expires`, still ahead at `now`, in seconds since 1970) and within
/// [`SOUND_BEFORE`] of its end. An ask Quiet holds open has no end said, so it never sounds.
pub fn should_sound(
    ask: &Shown,
    now: u32,
    on: bool,
    quiet: bool,
    looking: bool,
    already_sounded: bool,
) -> bool {
    on && !quiet
        && !looking
        && !already_sounded
        && ask
            .expires
            .is_some_and(|end| end > now && end - now <= SOUND_BEFORE)
}

/// One read of a project's asks, as [`Rules::expiring`] takes it.
pub struct Expiring<'a> {
    pub plane: &'a PlaneId,
    /// What the ✋ counts of it, as [`Heard::asks`].
    pub asks: &'a [Shown],
    /// Now, in seconds since 1970, as an ask's end is said.
    pub wall: u32,
    /// Now, as the notifier wakes by it.
    pub now: Instant,
    /// Whether the person turned the Expiring sound on ([`expiring_sound_of_layout`]).
    pub on: bool,
    /// Whether it is Quiet now.
    pub quiet: bool,
    /// Whether the person is looking at a chat now ([`Looking`]).
    pub looking: &'a dyn Fn(u32) -> bool,
}

/// **Whether an Expiring ask nearly out plays a sound**, as a layout document keeps it:
/// `notifications.expiringSound`, `true` only (`app/src/notificationPrefs.ts`). Anything else is
/// off, the default.
pub fn expiring_sound_of_layout(document: &serde_json::Value) -> bool {
    document
        .get("notifications")
        .and_then(|held| held.get("expiringSound"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

/// **What has been told, and what waits to be**: the whole of the notifier's memory.
#[derive(Default)]
pub struct Rules {
    /// The asks of each project read last, by key, with the chat that asks and whether the
    /// ask is the chat's own wait ([`own_wait`]): an ask in here was already considered.
    known: HashMap<PlaneId, HashMap<String, (u32, bool)>>,
    /// What each chat waiting was last read as: its name and its asks, the first first.
    waits: HashMap<Chat, Waits>,
    /// Each chat's own notification standing, by chat.
    own: HashMap<Chat, i32>,
    /// The one notification for a burst of chats, while it stands.
    many: Option<Many>,
    /// The burst open now ([`TOGETHER`]).
    burst: Option<Burst>,
    /// The chats that came to wait while Quiet: when it ends, what still waits is told, if the
    /// person is away ([`Self::quiet`]).
    held: Vec<Chat>,
    /// When Quiet began, in seconds since 1970, while it holds.
    quiet_since: Option<u32>,
    /// Every ask that came in while Quiet that the person was not looking at, in the order
    /// they came: what its summary counts, where they still wait when it ends.
    quiet_asks: Vec<EndedAsk>,
    /// Every failure and long finish that came in while Quiet, in the order they came: held
    /// until it ends ([`Self::quiet`]).
    quiet_finishes: Vec<HeldFinish>,
    /// The tasks of each project read as ended, by dispatch id: one in here was considered. A
    /// project's first read only fills it, so what ended before is never told.
    finished: HashMap<PlaneId, HashSet<String>>,
    /// The id the next notification takes.
    last_id: i32,
    /// **When each project's first snoozed chat comes back by itself** (#6018, ADR 0121
    /// decision 14): the notifier reads the project again then, so its asks are heard anew.
    snoozes_end: HashMap<PlaneId, Instant>,
    /// **The Expiring asks of each project that sounded** (#6110), by key: each sounds once.
    /// An ask no longer listed is let go of.
    sounded: HashMap<PlaneId, HashSet<String>>,
    /// **When each project's next Expiring ask comes [`SOUND_BEFORE`] its end** (#6110), while
    /// the sound is on: the notifier reads the project again then, and the ask sounds.
    sounds_due: HashMap<PlaneId, Instant>,
}

/// A chat waiting, as it was read last.
#[derive(Clone)]
struct Waits {
    name: String,
    /// Its asks' kinds, its newest-told first, then the rest in the registry's order.
    kinds: Vec<&'static str>,
}

/// **The notification for a burst**, and the chats it counts.
struct Many {
    id: i32,
    chats: Vec<Chat>,
    /// Whether it tells what waits after Quiet ([`Rules::quiet`]), which its title says.
    quiet: bool,
    /// The tasks Quiet held that it counts as well: how many finished, and how many failed.
    ended: (usize, usize),
}

/// **A burst of chats coming to wait**: the first is told at once; each chat that follows
/// within [`TOGETHER`] of the one before waits for it to close.
struct Burst {
    /// When the latest chat joined it.
    at: Instant,
    /// Every chat in it, the first first.
    chats: Vec<Chat>,
}

/// How many chats asking in one burst share one notification.
pub const MANY: usize = 3;

impl Rules {
    /// What one read of a project's asks does to the notifications.
    ///
    /// - A chat with a notification standing whose asks moved has it updated in place, and
    ///   one with none left has it taken away.
    /// - A chat newly waiting, that the person is not looking at, is told at once if no burst
    ///   is open, and joins the burst otherwise ([`Self::woke`] tells it). [`MANY`] or more
    ///   newly waiting in one read are told together at once.
    /// - While Quiet nothing new is told: the chat is held until it ends, and each ask the
    ///   person is not looking at is kept for its summary.
    pub fn heard(&mut self, heard: &Heard<'_>) -> Vec<Act> {
        let plane = heard.plane;
        let known = self.known.entry(plane.clone()).or_default();
        let fresh: Vec<&Shown> = heard
            .asks
            .iter()
            .filter(|ask| !known.contains_key(&ask.ask))
            .collect();
        *known = heard
            .asks
            .iter()
            .map(|ask| (ask.ask.clone(), (ask.session, own_wait(ask))))
            .collect();
        // Every chat waiting now, in the registry's order (the longest waiting first).
        let mut order: Vec<u32> = Vec::new();
        for ask in heard.asks {
            if !order.contains(&ask.session) {
                order.push(ask.session);
            }
        }
        self.waits
            .retain(|(of, session), _| of != plane || order.contains(session));
        let mut renewed: Vec<u32> = Vec::new();
        for session in &order {
            let mine: Vec<&Shown> = heard
                .asks
                .iter()
                .filter(|ask| ask.session == *session)
                .collect();
            let new: Vec<&Shown> = fresh
                .iter()
                .copied()
                .filter(|ask| ask.session == *session)
                .collect();
            let mut kinds: Vec<&'static str> = new.iter().map(|ask| kind_said(ask)).collect();
            for ask in &mine {
                if !new.iter().any(|one| one.ask == ask.ask) {
                    kinds.push(kind_said(ask));
                }
            }
            let chat = (plane.clone(), *session);
            let was = self.waits.insert(
                chat,
                Waits {
                    name: chain_of(mine[0], *session),
                    kinds,
                },
            );
            let moved = !new.is_empty() || was.is_none_or(|was| was.kinds.len() != mine.len());
            if moved {
                renewed.push(*session);
            }
        }
        // Asked only when something came in: reading the schedule is a file read.
        let quiet = !fresh.is_empty() && (heard.quiet)();
        if quiet {
            self.quiet_asks.extend(
                fresh
                    .iter()
                    .filter(|ask| !(heard.looking)(ask.session))
                    .map(|ask| EndedAsk {
                        plane: plane.clone(),
                        key: ask.ask.clone(),
                    }),
            );
        }
        let mut acts = self.let_go_of_answered();
        // A chat's own notification standing, updated in place.
        for session in &renewed {
            let chat = (plane.clone(), *session);
            if let Some(&id) = self.own.get(&chat) {
                acts.push(Act::Show(self.own_notice(id, &chat, true, false)));
            }
        }
        // The chats newly waiting that no notification tells of.
        let new: Vec<Chat> = order
            .iter()
            .filter(|session| fresh.iter().any(|ask| ask.session == **session))
            .filter(|session| !(heard.looking)(**session))
            .map(|session| (plane.clone(), *session))
            .filter(|chat| !self.accounted(chat))
            .collect();
        if new.is_empty() {
            return acts;
        }
        if quiet {
            self.held.extend(new);
            return acts;
        }
        match &mut self.burst {
            Some(burst) if heard.now.saturating_duration_since(burst.at) < TOGETHER => {
                burst.at = heard.now;
                burst.chats.extend(new);
            }
            _ => {
                self.burst = Some(Burst {
                    at: heard.now,
                    chats: new.clone(),
                });
                acts.extend(self.tell(new));
            }
        }
        acts
    }

    /// **What the notifier does when it wakes by itself** ([`Self::next_due`]): a burst that
    /// closed with chats waiting in it tells them, one notification for all of it when it
    /// counts [`MANY`] or more. A chat the person is looking at by then, or that no longer
    /// waits, is not.
    pub fn woke(&mut self, now: Instant, looking: &dyn Fn(&Chat) -> bool) -> Vec<Act> {
        let mut acts = Vec::new();
        let closed = self
            .burst
            .as_ref()
            .is_some_and(|burst| now.saturating_duration_since(burst.at) >= TOGETHER);
        if closed && let Some(burst) = self.burst.take() {
            let waiting: Vec<Chat> = burst
                .chats
                .into_iter()
                .filter(|chat| self.waits.contains_key(chat))
                .filter(|chat| self.own.contains_key(chat) || !looking(chat))
                .collect();
            if waiting.iter().any(|chat| !self.told(chat)) {
                acts.extend(self.tell(waiting));
            }
        }
        acts
    }

    /// **Quiet is `quiet` at `at`**, in seconds since 1970 (ADR 0121 decision 13), as the
    /// notifier finds it before each read. As it begins, it starts a summary of its own. As it
    /// ends, what it held is let go: **one notification**, naming every chat still waiting
    /// unseen, if the person is `away` from every window, and none if they are at one, where
    /// the summary heads Asks ([`QuietEnded`]). The summary is answered either way.
    pub fn quiet(
        &mut self,
        quiet: bool,
        at: u32,
        away: bool,
        looking: &dyn Fn(&Chat) -> bool,
    ) -> (Vec<Act>, Option<QuietEnded>) {
        match (quiet, self.quiet_since) {
            (true, None) => {
                self.quiet_since = Some(at);
                self.quiet_asks.clear();
                self.quiet_finishes.clear();
                (Vec::new(), None)
            }
            (false, Some(from)) => {
                self.quiet_since = None;
                let known = &self.known;
                let asks: Vec<EndedAsk> = std::mem::take(&mut self.quiet_asks)
                    .into_iter()
                    .filter(|one| {
                        known
                            .get(&one.plane)
                            .is_some_and(|asks| asks.contains_key(&one.key))
                    })
                    .collect();
                let held: Vec<Chat> = std::mem::take(&mut self.held)
                    .into_iter()
                    .filter(|chat| self.waits.contains_key(chat) && !looking(chat))
                    .filter(|chat| !self.told(chat))
                    .collect();
                let finishes = std::mem::take(&mut self.quiet_finishes);
                let failed = finishes.iter().filter(|one| one.failed).count();
                let counts = (finishes.len() - failed, failed);
                let acts = match (away, held.is_empty(), finishes.first()) {
                    (false, _, _) | (true, true, None) => Vec::new(),
                    (true, false, _) => self.tell_after_quiet(held, counts),
                    (true, true, Some(first)) => {
                        let id = self.next_id();
                        let key = first.key.clone();
                        vec![Act::Show(finishes_after_quiet(id, &finishes, counts, key))]
                    }
                };
                (acts, Some(QuietEnded { from, to: at, asks }))
            }
            _ => (Vec::new(), None),
        }
    }

    /// **What one read of a project's ended tasks does to the notifications** (#6005, ADR 0121
    /// decision 3): Should know notifies for two things only, each the first time it is read.
    ///
    /// - **A failure**: a task the person asked for that came to nothing, named with its
    ///   project and the first line of its report.
    /// - **A long finish while away**: one that finished after working longer than `long`,
    ///   with the person away from every window.
    ///
    /// Every other update stays in Updates, counted by the bell. A task a chat dispatched
    /// reports to that chat (#6036), and tells the person nothing here.
    pub fn ended(&mut self, ended: &Ended<'_>) -> Vec<Act> {
        let first = !self.finished.contains_key(ended.plane);
        let known = self.finished.entry(ended.plane.clone()).or_default();
        let fresh: Vec<&Finish> = ended
            .finishes
            .iter()
            .filter(|finish| known.insert(finish.id.clone()))
            .collect();
        if first {
            return Vec::new();
        }
        let mut acts = Vec::new();
        for finish in fresh {
            let long = !finish.failed && ended.away && finish.worked > ended.long;
            let off = if finish.failed {
                !ended.sources.failed
            } else {
                !ended.sources.finished
            };
            if off {
                continue;
            }
            if !finish.by_person || !(finish.failed || long) || (ended.looking)(finish.session) {
                continue;
            }
            if ended.quiet {
                self.quiet_finishes.push(HeldFinish {
                    chat: (ended.plane.clone(), finish.session),
                    name: plain(&finish.name),
                    failed: finish.failed,
                    key: update_key(finish),
                });
                continue;
            }
            let id = self.next_id();
            acts.push(Act::Show(finish_notice(id, ended.plane, finish)));
        }
        acts
    }

    /// **What one read of a project's asks sounds** (#6110, ADR 0121 decision 12): each
    /// Expiring ask that is nearly out ([`should_sound`]) sounds once, as its chat's notification
    /// shown again with the system's own sound; one per chat, however many of its asks are. A
    /// chat no notification stands for is told now, with the sound. The next ask to come
    /// [`SOUND_BEFORE`] its end is read again then ([`Self::next_due`]).
    pub fn expiring(&mut self, expiring: &Expiring<'_>) -> Vec<Act> {
        let plane = expiring.plane;
        let sounded = self.sounded.entry(plane.clone()).or_default();
        sounded.retain(|key| expiring.asks.iter().any(|ask| &ask.ask == key));
        let mut chats: Vec<u32> = Vec::new();
        let mut next: Option<u32> = None;
        for ask in expiring.asks {
            let already = sounded.contains(&ask.ask);
            if should_sound(
                ask,
                expiring.wall,
                expiring.on,
                expiring.quiet,
                (expiring.looking)(ask.session),
                already,
            ) {
                sounded.insert(ask.ask.clone());
                if !chats.contains(&ask.session) {
                    chats.push(ask.session);
                }
            } else if expiring.on && !already {
                let at = ask
                    .expires
                    .map(|end| end.saturating_sub(SOUND_BEFORE))
                    .filter(|at| *at > expiring.wall);
                if let Some(at) = at {
                    next = Some(next.map_or(at, |first| first.min(at)));
                }
            }
        }
        match next {
            Some(at) => self.sounds_due.insert(
                plane.clone(),
                expiring.now + Duration::from_secs(u64::from(at - expiring.wall)),
            ),
            None => self.sounds_due.remove(plane),
        };
        chats
            .into_iter()
            .map(|session| Act::Show(self.sounding(&(plane.clone(), session))))
            .collect()
    }

    /// **`chat`'s notification, shown again with the sound**: its own, or the burst's that
    /// counts it; or, where none stands for it, its own told now.
    fn sounding(&mut self, chat: &Chat) -> Notice {
        let mut notice = if let Some(&id) = self.own.get(chat) {
            self.own_notice(id, chat, true, true)
        } else if let Some(many) = self.many.as_ref().filter(|many| many.chats.contains(chat)) {
            let (id, chats) = (many.id, many.chats.clone());
            self.many_notice(id, chats, true, true)
        } else {
            let id = self.next_id();
            self.own.insert(chat.clone(), id);
            self.own_notice(id, chat, false, true)
        };
        notice.sound = true;
        notice
    }

    /// **When the notifier should wake by itself next**: when the burst open closes with
    /// chats in it not told yet, when a project's snooze runs out (#6018), or when an Expiring
    /// ask is nearly out (#6110).
    pub fn next_due(&self) -> Option<Instant> {
        let burst = self.burst.as_ref().and_then(|burst| {
            burst
                .chats
                .iter()
                .any(|chat| !self.told(chat))
                .then_some(burst.at + TOGETHER)
        });
        burst
            .into_iter()
            .chain(self.snoozes_end.values().copied())
            .chain(self.sounds_due.values().copied())
            .min()
    }

    /// **The projects whose snooze has run out by `now`** (#6018), or with an Expiring ask
    /// nearly out (#6110): the notifier reads them again, so the asks of a chat back by itself
    /// are heard anew, and the ask sounds.
    pub fn due(&self, now: Instant) -> Vec<PlaneId> {
        let mut due: Vec<PlaneId> = Vec::new();
        for (plane, at) in self.snoozes_end.iter().chain(&self.sounds_due) {
            if *at <= now && !due.contains(plane) {
                due.push(plane.clone());
            }
        }
        due
    }

    /// **When `plane`'s first snoozed chat comes back by itself**, as its last read said, or
    /// none (#6018): the notifier reads it again then ([`Self::next_due`]).
    pub fn snoozed_until(&mut self, plane: &PlaneId, end: Option<Instant>) {
        match end {
            Some(end) => self.snoozes_end.insert(plane.clone(), end),
            None => self.snoozes_end.remove(plane),
        };
    }

    /// Whether quiet hours hold chats back: the notifier looks again every so often while they
    /// do.
    pub fn holds_in_quiet_hours(&self) -> bool {
        !self.held.is_empty()
    }

    /// **Chat `session` of `plane` waits on the person anew**, as the board's own edge says
    /// (`Moved::interrupts`): its own wait, a reply or a prompt in its terminal, is keyed by
    /// the chat and not by the wait, so a turn that ended on the person again between two
    /// reads would read as the same ask. It is fresh at the next read. A decision it holds,
    /// keyed by itself, is untouched.
    pub fn renew(&mut self, plane: &PlaneId, session: u32) {
        if let Some(known) = self.known.get_mut(plane) {
            known.retain(|_, (asker, own)| !(*own && *asker == session));
        }
    }

    /// A project let go of, or no longer held: nothing of it is remembered, and what was told
    /// of it is taken away.
    pub fn forget(&mut self, plane: &PlaneId) -> Vec<Act> {
        self.known.remove(plane);
        // Held again, its first read of the tasks that ended is the baseline anew.
        self.finished.remove(plane);
        self.quiet_finishes.retain(|one| &one.chat.0 != plane);
        self.waits.retain(|(of, _), _| of != plane);
        self.held.retain(|(of, _)| of != plane);
        self.snoozes_end.remove(plane);
        self.sounded.remove(plane);
        self.sounds_due.remove(plane);
        if let Some(burst) = &mut self.burst {
            burst.chats.retain(|(of, _)| of != plane);
        }
        self.let_go_of_answered()
    }

    /// Whether `chat` is told of by a notification standing.
    fn told(&self, chat: &Chat) -> bool {
        self.own.contains_key(chat)
            || self
                .many
                .as_ref()
                .is_some_and(|many| many.chats.contains(chat))
    }

    /// Whether `chat` is told of, or waits to be: in the burst open or in quiet hours.
    fn accounted(&self, chat: &Chat) -> bool {
        self.told(chat)
            || self.held.contains(chat)
            || self
                .burst
                .as_ref()
                .is_some_and(|burst| burst.chats.contains(chat))
    }

    /// Takes away each notification whose chats no longer wait, and updates the one for a
    /// burst some of whose chats were answered.
    fn let_go_of_answered(&mut self) -> Vec<Act> {
        let mut acts = Vec::new();
        let waits = &self.waits;
        let mut gone: Vec<i32> = Vec::new();
        self.own.retain(|chat, id| {
            let keep = waits.contains_key(chat);
            if !keep {
                gone.push(*id);
            }
            keep
        });
        gone.sort_unstable();
        acts.extend(gone.into_iter().map(Act::Remove));
        if let Some(many) = &mut self.many {
            let before = many.chats.len();
            many.chats.retain(|chat| waits.contains_key(chat));
            if many.chats.is_empty() {
                acts.push(Act::Remove(many.id));
                self.many = None;
            } else if many.chats.len() != before {
                let (id, chats) = (many.id, many.chats.clone());
                acts.push(Act::Show(self.many_notice(id, chats, true, false)));
            }
        }
        if let Some(burst) = &mut self.burst {
            burst.chats.retain(|chat| waits.contains_key(chat));
        }
        acts
    }

    /// **Tells `chats`**, a burst's: fewer than [`MANY`] each with a notification of its own;
    /// [`MANY`] or more with one for all of them, which takes over from their own, and which
    /// a burst's notification still standing grows into.
    fn tell(&mut self, chats: Vec<Chat>) -> Vec<Act> {
        if chats.len() < MANY {
            let mut acts = Vec::new();
            for chat in chats {
                if self.told(&chat) {
                    continue;
                }
                let id = self.next_id();
                self.own.insert(chat.clone(), id);
                acts.push(Act::Show(self.own_notice(id, &chat, false, true)));
            }
            return acts;
        }
        let mut all: Vec<Chat> = self
            .many
            .as_ref()
            .map(|many| many.chats.clone())
            .unwrap_or_default();
        for chat in chats {
            if !all.contains(&chat) {
                all.push(chat);
            }
        }
        let mut acts = Vec::new();
        let mut taken: Vec<i32> = all
            .iter()
            .filter_map(|chat| self.own.remove(chat))
            .collect();
        taken.sort_unstable();
        acts.extend(taken.into_iter().map(Act::Remove));
        let (id, replaces) = match &self.many {
            Some(many) => (many.id, true),
            None => (self.next_id(), false),
        };
        self.many = Some(Many {
            id,
            chats: all.clone(),
            quiet: false,
            ended: (0, 0),
        });
        acts.push(Act::Show(self.many_notice(id, all, replaces, true)));
        acts
    }

    /// **Tells `chats`, held while Quiet, as one notification** however many they are, saying
    /// it was quiet; it takes over from a burst's notification still standing.
    fn tell_after_quiet(&mut self, chats: Vec<Chat>, ended: (usize, usize)) -> Vec<Act> {
        let mut acts = Vec::new();
        if let Some(many) = self.many.take() {
            acts.push(Act::Remove(many.id));
        }
        let id = self.next_id();
        self.many = Some(Many {
            id,
            chats: chats.clone(),
            quiet: true,
            ended,
        });
        acts.push(Act::Show(self.many_notice(id, chats, false, true)));
        acts
    }

    fn next_id(&mut self) -> i32 {
        self.last_id += 1;
        self.last_id
    }

    /// Chat `chat`'s own notification: who asks, its project, and what kind of ask.
    fn own_notice(&self, id: i32, chat: &Chat, replaces: bool, news: bool) -> Notice {
        let waits = self.waits.get(chat);
        let title = waits.map_or_else(|| format!("chat {}", chat.1), |waits| waits.name.clone());
        let kinds = waits
            .map(|waits| waits.kinds.as_slice())
            .unwrap_or_default();
        let said = match kinds {
            [] => String::new(),
            [only] => (*only).to_owned(),
            [first, rest @ ..] => format!("{first}, and {} more", rest.len()),
        };
        Notice {
            id,
            replaces,
            news,
            chats: vec![chat.clone()],
            title,
            body: format!("{}: {said}", project_name(&chat.0)),
            update: None,
            sound: false,
        }
    }

    /// The notification for a burst of `chats`: how many, in how many projects, and their
    /// names.
    fn many_notice(&self, id: i32, chats: Vec<Chat>, replaces: bool, news: bool) -> Notice {
        let mut projects: Vec<&PlaneId> = Vec::new();
        for (plane, _) in &chats {
            if !projects.contains(&plane) {
                projects.push(plane);
            }
        }
        let after_quiet = self
            .many
            .as_ref()
            .is_some_and(|many| many.id == id && many.quiet);
        let ended = self.many.as_ref().map_or((0, 0), |many| many.ended);
        let title = match (after_quiet, projects.as_slice()) {
            (true, _) => quiet_title(chats.len(), ended),
            (false, [_]) => format!("{} chats need you", chats.len()),
            (false, many) => format!("{} chats in {} projects need you", chats.len(), many.len()),
        };
        let names: Vec<String> = chats
            .iter()
            .take(MOST_NAMED)
            .map(|chat| {
                let name = self.waits.get(chat).map_or_else(
                    || format!("chat {}", chat.1),
                    |waits| first_name(&waits.name, chat.1),
                );
                match projects.as_slice() {
                    [_] => name,
                    _ => format!("{name} ({})", project_name(&chat.0)),
                }
            })
            .collect();
        let named = and_more(&names, chats.len());
        let body = match projects.as_slice() {
            [only] => format!("{}: {named}", project_name(only)),
            _ => named,
        };
        Notice {
            id,
            replaces,
            news,
            chats,
            title,
            body,
            update: None,
            sound: false,
        }
    }
}

/// **Whether `notice` is shown on a system that `replaces` notifications or not.** macOS (from
/// an app bundle), Linux and the BSDs replace a notification by its id. Windows cannot take one
/// away or change it once shown (the notification plugin's `remove_active` does nothing
/// there): an update that tells of no chat new is not shown, so a chat is not told twice; the
/// one standing still lands on what waits.
pub fn shown_here(notice: &Notice, replaces: bool) -> bool {
    replaces || !notice.replaces || notice.news
}

/// Whether the notification plugin takes a notification away, or replaces it, by its id here.
/// On macOS purlis does both through the system's own center instead (`asknotify_macos.rs`).
pub const REPLACES_HERE: bool = cfg!(all(unix, not(target_os = "macos")));

/// A failure or long finish held while Quiet: told, counted, when it ends.
struct HeldFinish {
    chat: Chat,
    name: String,
    failed: bool,
    /// Its line in Updates ([`update_key`]).
    key: String,
}

/// A task's line in Updates, by its key there (`inboxUpdates.ts`, `taskUpdates`).
fn update_key(finish: &Finish) -> String {
    format!("task:{}", finish.id)
}

/// The title of what Quiet held, told as it ends: "While you were quiet: 1 chat needs you, 2
/// finished, 1 failed", each part left out where it counts none.
fn quiet_title(waiting: usize, (finished, failed): (usize, usize)) -> String {
    let mut parts: Vec<String> = Vec::new();
    match waiting {
        0 => {}
        1 => parts.push("1 chat needs you".to_owned()),
        count => parts.push(format!("{count} chats need you")),
    }
    if finished > 0 {
        parts.push(format!("{finished} finished"));
    }
    if failed > 0 {
        parts.push(format!("{failed} failed"));
    }
    format!("While you were quiet: {}", parts.join(", "))
}

/// `names`, the first of `all` things, as a list: "a, b and c", or "a, b, c and 2 more".
fn and_more(names: &[String], all: usize) -> String {
    match (all - names.len(), names.split_last()) {
        (0, Some((last, rest))) if !rest.is_empty() => format!("{} and {last}", rest.join(", ")),
        (0, _) => names.join(""),
        (more, _) => format!("{} and {more} more", names.join(", ")),
    }
}

/// **The one notification for the tasks Quiet held**, where no chat still waits: what they
/// count, and their names. A click opens the first's line in Updates.
fn finishes_after_quiet(
    id: i32,
    finishes: &[HeldFinish],
    counts: (usize, usize),
    first: String,
) -> Notice {
    let mut chats: Vec<Chat> = Vec::new();
    let mut projects: Vec<&PlaneId> = Vec::new();
    for one in finishes {
        if !chats.contains(&one.chat) {
            chats.push(one.chat.clone());
        }
        if !projects.contains(&&one.chat.0) {
            projects.push(&one.chat.0);
        }
    }
    let names: Vec<String> = finishes
        .iter()
        .take(MOST_NAMED)
        .map(|one| match projects.as_slice() {
            [_] => one.name.clone(),
            _ => format!("{} ({})", one.name, project_name(&one.chat.0)),
        })
        .collect();
    let named = and_more(&names, finishes.len());
    Notice {
        id,
        replaces: false,
        news: true,
        title: quiet_title(0, counts),
        body: match projects.as_slice() {
            [only] => format!("{}: {named}", project_name(only)),
            _ => named,
        },
        chats,
        update: Some(first),
        sound: false,
    }
}

/// **The notification for a task that ended**: its name, its project, and that it failed with
/// its report's first line, never more of it; or how long it worked.
fn finish_notice(id: i32, plane: &PlaneId, finish: &Finish) -> Notice {
    let line = finish.report.lines().next().unwrap_or_default();
    Notice {
        id,
        replaces: false,
        news: true,
        chats: vec![(plane.clone(), finish.session)],
        title: plain(&finish.name),
        body: if finish.failed {
            format!("{}: Failed: {}", project_name(plane), plain(line))
        } else {
            format!(
                "{}: Finished after {}",
                project_name(plane),
                minutes_said(finish.worked)
            )
        },
        update: Some(update_key(finish)),
        sound: false,
    }
}

/// How long a task worked, in whole minutes: "12 minutes", "1 minute".
fn minutes_said(worked: Duration) -> String {
    match worked.as_secs() / 60 {
        1 => "1 minute".to_owned(),
        minutes => format!("{minutes} minutes"),
    }
}

/// The chain of who asks `ask`, as a title: `steward 12 › #3046 drill`, or the chat's number.
fn chain_of(ask: &Shown, session: u32) -> String {
    if ask.chain.is_empty() {
        return format!("chat {session}");
    }
    capped(
        &ask.chain
            .iter()
            .map(|name| plain(name))
            .collect::<Vec<_>>()
            .join(" › "),
        MOST_NAME,
    )
}

/// The first name of a chain said by [`chain_of`]: the session's.
fn first_name(chain: &str, session: u32) -> String {
    let first = chain.split(" › ").next().unwrap_or_default();
    if first.is_empty() {
        format!("chat {session}")
    } else {
        first.to_owned()
    }
}

/// A project's name: the last segment of its directory, as its tab on the title bar says it
/// (`projectName.ts`).
pub fn project_name(plane: &PlaneId) -> String {
    plane.root().file_name().map_or_else(
        || plane.as_str().to_owned(),
        |name| plain(&name.to_string_lossy()),
    )
}

/// **What a notification says an ask is**: its kind, never its words. A command line, a host
/// or a path is the window's to show; a notification is drawn where the window is not, on a
/// locked screen and in the system's own list of them, and kept there.
fn kind_said(ask: &Shown) -> &'static str {
    match ask.source {
        AskSource::Permission => "Asks your permission",
        AskSource::Dispatch => "Asks to hand a task on",
        AskSource::SandboxHost => "Asks to reach a host",
        AskSource::SandboxWrite => "Asks to write in a folder",
        AskSource::SandboxLocal => "Asks to reach a service on this machine",
        AskSource::WorkspaceRemoval => "Asks you to remove a workspace",
        AskSource::ProposedChange => "Proposes a change for you to apply",
        AskSource::RunnerWrite => "Asks, from a pool's runner, to change this project",
        AskSource::RunnerDispatch => "Asks, from a pool's runner, to start a chat here",
        AskSource::Terminal => "Waiting in its terminal",
        AskSource::Question => "Waiting on your reply",
        AskSource::TaskQuestion => "A task asks you a question",
        AskSource::SignIn => "Asks you to sign a profile in",
        AskSource::SandboxOnly => "Asks you to make a sandbox-only profile",
        AskSource::OldHarnessPlugins => "Old harness plugin settings, ignored now",
        AskSource::Signing => "Lanes wait on signed commits",
        AskSource::Browser => "Asks what its browser may reach",
        AskSource::BrowserNeedsYou => "Needs you in its browser",
        AskSource::BrowserSite => "Asks to let its browser reach a new site",
        AskSource::PoolLogin => "Asks to log its browser in on a pool's runner",
        AskSource::PoolTool => "Asks to send a tool its vault values for a run on a pool's runner",
        AskSource::BrowserLogin => "Asks to log its browser in to a site with vault credentials",
        AskSource::Secret => "Asks you for a secret its vault doesn't hold",
        AskSource::RunGrant => "Asks you to allow a program to use a vault secret",
        AskSource::SecretFill => "Asks to put a vault secret into a field of its browser",
        AskSource::Vault => "Refused a vault its persona isn't tagged for",
        AskSource::SandboxExpects => "Asks you to allow what this project expects",
        AskSource::Confirmation => "A command waits on your confirmation",
        AskSource::Budget => "Paused at its budget",
        AskSource::Looping => "Paused, as it may be looping",
        AskSource::MergeHold => "Asks you to merge a request",
    }
}

/// Whether `ask` is a chat's own wait, keyed by the chat: a reply it waits on, or a prompt in
/// its terminal.
fn own_wait(ask: &Shown) -> bool {
    matches!(ask.source, AskSource::Question | AskSource::Terminal)
}

/// **A name a chat chose, as plain text** for a notification: every control and direction
/// mark dropped, each run of white space one space, and at most [`MOST_NAME`] characters.
pub fn plain(name: &str) -> String {
    plain_within(name, MOST_NAME)
}

/// [`plain`], at most `most` characters: for words a host or a runner wrote, which the window
/// shows as text (#2846).
pub fn plain_within(name: &str, most: usize) -> String {
    let mut out = String::new();
    let mut gap = false;
    for one in name.chars() {
        if one.is_whitespace() {
            gap = !out.is_empty();
            continue;
        }
        // The marks that turn the direction of the text after them: purlis's generated
        // Bidi_Control table (SK-49).
        if one.is_control() || purlis_unicode::bidi_control(one) {
            continue;
        }
        if gap {
            out.push(' ');
            gap = false;
        }
        out.push(one);
    }
    capped(&out, most)
}

/// `text` at most `most` characters, ending in `…` where it was cut.
fn capped(text: &str, most: usize) -> String {
    if text.chars().count() <= most {
        return text.to_owned();
    }
    let mut cut: String = text.chars().take(most.saturating_sub(1)).collect();
    cut.push('…');
    cut
}

/// The event every window is sent when Quiet starts or ends: [`QuietNow`].
pub const QUIET_CHANGED: &str = "quiet-changed";

/// **How long Quiet holds after the last key typed into a chat's pane** (ADR 0121 decision 13):
/// the person typing is the person busy, and a pause this long is them done.
pub const TYPING_QUIET: Duration = Duration::from_secs(30);

/// **What makes it Quiet, besides the schedule** (#6017): "Quiet now", by hand, until it is
/// turned off, and the person typing in a chat, until [`TYPING_QUIET`] after their last key.
/// Neither is kept past the app's run: a relaunch starts with neither.
#[derive(Debug, Default)]
pub struct QuietSources {
    now: bool,
    typed_at: Option<Instant>,
}

impl QuietSources {
    /// "Quiet now" turned on or off.
    pub fn set_now(&mut self, on: bool) {
        self.now = on;
    }

    /// Whether "Quiet now" is on.
    pub fn now_on(&self) -> bool {
        self.now
    }

    /// The person typed into a chat's pane `at`: whether that started Quiet by typing, which
    /// the notifier is woken for.
    pub fn typed(&mut self, at: Instant) -> bool {
        let started = self.typing_ends(at).is_none();
        self.typed_at = Some(at);
        started
    }

    /// When Quiet by typing ends, while the person is typing `at`.
    pub fn typing_ends(&self, at: Instant) -> Option<Instant> {
        self.typed_at
            .map(|typed| typed + TYPING_QUIET)
            .filter(|ends| *ends > at)
    }

    /// Whether it is Quiet `at`, where the schedule says `scheduled`.
    pub fn is_quiet(&self, scheduled: bool, at: Instant) -> bool {
        scheduled || self.now || self.typing_ends(at).is_some()
    }
}

/// Whether `bytes` sent from a chat's pane are the person typing: anything but the terminal
/// answering its program by itself (`curation::only_the_terminal_answering`).
pub fn is_typing(bytes: &[u8]) -> bool {
    !crate::curation::only_the_terminal_answering(bytes)
}

/// **What Quiet held, told when it ends** (ADR 0121 decision 13): when it was, and the asks
/// that came in it that still wait. The window counts with them the updates of the same span
/// ("While you were quiet: 3 asks, 2 finished, 1 failed"), and drops the row once all of it is
/// seen (#6002).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct QuietEnded {
    /// When it began, in seconds since 1970.
    pub from: u32,
    /// When it ended, in seconds since 1970.
    pub to: u32,
    /// The asks that came in it that the person was not looking at, still waiting as it ended.
    pub asks: Vec<EndedAsk>,
}

/// One ask a Quiet's summary counts: its project, and its key in the registry.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct EndedAsk {
    pub plane: PlaneId,
    pub key: String,
}

/// **Whether it is Quiet now, and what Quiet ended last**, as a window asks for it and is sent
/// it ([`QUIET_CHANGED`]).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct QuietNow {
    pub quiet: bool,
    /// Whether "Quiet now" is on: the toggle's state, whatever else makes it Quiet.
    pub by_hand: bool,
    /// The summary of the latest Quiet that ended in this run of the app, if one did.
    pub ended: Option<QuietEnded>,
}

/// **The person's quiet hours** (#1989): from a time of day until another, in minutes since
/// midnight, local time. A span that ends before it starts runs across midnight; one that ends
/// where it starts covers nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuietHours {
    pub from: u16,
    pub to: u16,
}

impl QuietHours {
    /// The quiet hours a layout document keeps, under `notifications.quiet` (`{"on": true,
    /// "from": "22:00", "to": "07:00"}`, `app/src/notificationPrefs.ts`), while they are on.
    /// Anything else is none: the window says what it put right.
    pub fn of_layout(document: &serde_json::Value) -> Option<Self> {
        let quiet = document.get("notifications")?.get("quiet")?;
        if quiet.get("on")?.as_bool() != Some(true) {
            return None;
        }
        let at = |key: &str| quiet.get(key)?.as_str().and_then(minute_of);
        Some(Self {
            from: at("from")?,
            to: at("to")?,
        })
    }

    /// Whether `minute` of the day is in them.
    pub fn covers(self, minute: u16) -> bool {
        match self.from.cmp(&self.to) {
            std::cmp::Ordering::Less => (self.from..self.to).contains(&minute),
            std::cmp::Ordering::Greater => minute >= self.from || minute < self.to,
            std::cmp::Ordering::Equal => false,
        }
    }
}

/// The minute of the day `text` names, written `HH:MM` on a 24-hour clock.
pub fn minute_of(text: &str) -> Option<u16> {
    let (hours, minutes) = text.split_once(':')?;
    let two = |part: &str| {
        (part.len() == 2 && part.bytes().all(|one| one.is_ascii_digit()))
            .then(|| part.parse::<u16>().ok())
            .flatten()
    };
    let (hours, minutes) = (two(hours)?, two(minutes)?);
    (hours < 24 && minutes < 60).then_some(hours * 60 + minutes)
}

/// **How long a task works before its finish notifies** while the person is away, as a layout
/// document keeps it: `notifications.finishedAfter`, a whole number of minutes above none
/// (`app/src/notificationPrefs.ts`). Anything else is [`FINISHED_AFTER`].
pub fn finished_after_of_layout(document: &serde_json::Value) -> Duration {
    document
        .get("notifications")
        .and_then(|held| held.get("finishedAfter"))
        .and_then(serde_json::Value::as_u64)
        .filter(|minutes| *minutes > 0)
        .map_or(FINISHED_AFTER, |minutes| {
            Duration::from_secs(minutes.saturating_mul(60))
        })
}

/// The person's quiet hours on this machine, as the layout file says, while they are on. A file
/// that cannot be read says none.
fn scheduled() -> Option<QuietHours> {
    QuietHours::of_layout(&layout()?)
}

/// The layout file on this machine, as a document, where it can be read.
fn layout() -> Option<serde_json::Value> {
    let root = purlis_core::machine::config_root()?;
    purlis_core::windowprefs::read_layout(&root).document
}

/// Whether `hours` cover this minute, on this machine's clock.
fn covers_now(hours: QuietHours) -> bool {
    use chrono::Timelike;
    let now = chrono::Local::now();
    let minute = u16::try_from(now.hour() * 60 + now.minute()).unwrap_or_default();
    hours.covers(minute)
}

/// Seconds since 1970, now.
fn wall_now() -> u32 {
    u32::try_from(crate::sandboxing::now_secs()).unwrap_or(u32::MAX)
}

/// **Whether the person is looking at a chat's asks**: all of the window's questions, each
/// answered by the window holding the chat's project.
#[derive(Debug, Clone, Copy)]
pub struct Looking {
    /// That window is on screen, has the keyboard and has the project in front.
    pub window_in_front: bool,
    /// It has the project's Inbox open.
    pub inbox_open: bool,
    /// It has the chat on screen in the tab in front.
    pub chat_on_screen: bool,
}

impl Looking {
    pub fn is_looking(self) -> bool {
        self.window_in_front && (self.inbox_open || self.chat_on_screen)
    }
}

/// **Which project's Inbox each window has open**, as the window says it (`inbox_shown`).
/// A window that never said has none open.
#[derive(Default)]
pub struct InboxOpen {
    open: Mutex<HashSet<(String, PlaneId)>>,
}

impl InboxOpen {
    fn open(&self) -> MutexGuard<'_, HashSet<(String, PlaneId)>> {
        self.open.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// `window` says whether it has `plane`'s Inbox open.
    pub fn shown(&self, window: &str, plane: &PlaneId, open: bool) {
        let key = (window.to_owned(), plane.clone());
        if open {
            self.open().insert(key);
        } else {
            self.open().remove(&key);
        }
    }

    pub fn is_open(&self, window: &str, plane: &PlaneId) -> bool {
        self.open().contains(&(window.to_owned(), plane.clone()))
    }

    /// A window gone has no Inbox open.
    pub fn forget_window(&self, window: &str) {
        self.open().retain(|(label, _)| label != window);
    }
}

/// **The project the latest notification was about**: where a reopen with every window
/// hidden goes ([`brought_back`]).
#[derive(Default)]
pub struct LatestSent {
    plane: Mutex<Option<PlaneId>>,
}

impl LatestSent {
    fn held(&self) -> MutexGuard<'_, Option<PlaneId>> {
        self.plane.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// A notification about one of `plane`'s chats went out.
    pub fn sent(&self, plane: &PlaneId) {
        *self.held() = Some(plane.clone());
    }

    /// The project the latest notification was about, if one went out.
    #[cfg(any(target_os = "macos", test))]
    pub fn plane(&self) -> Option<PlaneId> {
        self.held().clone()
    }
}

/// **The window a reopen with every window hidden brings back**: the one holding the project
/// the latest notification was about (`holder`), where its landing waits; else the main
/// window, as a click on the dock icon would.
#[cfg(any(target_os = "macos", test))]
pub fn brought_back(
    latest: Option<&PlaneId>,
    holder: impl Fn(&PlaneId) -> Option<String>,
) -> String {
    latest
        .and_then(holder)
        .unwrap_or_else(|| crate::windows::MAIN.to_owned())
}

/// **The notifier**: where the registry's sources say a project's asks may have moved, and
/// the one thread that reads them and sends.
pub struct Notifier {
    poked: Mutex<mpsc::Sender<Poke>>,
    inbox: InboxOpen,
    latest: LatestSent,
    /// "Quiet now" and the person typing ([`QuietSources`]).
    quiet: Mutex<QuietSources>,
    /// What a window is told of Quiet: as it was found last, and what it ended last.
    told: Mutex<QuietNow>,
}

impl Notifier {
    /// Starts the thread that reads and sends, for `app`.
    pub fn start(app: &tauri::AppHandle) -> Self {
        let (poked, heard) = mpsc::channel();
        let app = app.clone();
        let _ = std::thread::Builder::new()
            .name("purlis-ask-notifications".to_owned())
            .spawn(move || listen(&app, &heard));
        Self {
            poked: Mutex::new(poked),
            inbox: InboxOpen::default(),
            latest: LatestSent::default(),
            quiet: Mutex::new(QuietSources::default()),
            told: Mutex::new(QuietNow {
                quiet: false,
                by_hand: false,
                ended: None,
            }),
        }
    }

    fn sources(&self) -> MutexGuard<'_, QuietSources> {
        self.quiet.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn told(&self) -> MutexGuard<'_, QuietNow> {
        self.told.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// What the notifier is told: a project whose asks may have moved, and the chat that waits on
/// the person anew, where the board said one does; or, with no project, that Quiet may have
/// moved.
type Poke = Option<(PlaneId, Option<u32>)>;

/// `plane`'s asks may have moved: they are read again once the burst settles.
pub fn poke(app: &tauri::AppHandle, plane: &PlaneId) {
    tell(app, Some((plane.clone(), None)));
}

/// **The person typed `text` into a chat's pane** (#6017), as the pane says, at most every few
/// seconds: Quiet holds until [`TYPING_QUIET`] after their last key. The terminal answering by
/// itself is not them ([`is_typing`]). Answers whether it counted, so the pane says the next
/// key that does.
#[tauri::command]
#[specta::specta]
pub fn quiet_typed(
    app: tauri::AppHandle,
    notifier: tauri::State<'_, Notifier>,
    text: String,
) -> bool {
    if !is_typing(text.as_bytes()) {
        return false;
    }
    if notifier.sources().typed(Instant::now()) {
        tell(&app, None);
    }
    true
}

/// Whether it is Quiet now, and what the latest Quiet that ended held (#6017).
#[tauri::command]
#[specta::specta]
pub fn quiet_now(notifier: tauri::State<'_, Notifier>) -> QuietNow {
    notifier.told().clone()
}

/// **"Quiet now"** turned on or off by the person (#6017): from the Asks view's toggle or the
/// palette. It holds until they turn it off, or the app ends.
#[tauri::command]
#[specta::specta]
pub fn set_quiet_now(app: tauri::AppHandle, notifier: tauri::State<'_, Notifier>, on: bool) {
    notifier.sources().set_now(on);
    tell(&app, None);
}

/// Chat `moved` moved: its project's asks are read again, and a chat that came to wait on the
/// person by its own move (`Moved::interrupts`) waits anew ([`Rules::renew`]).
pub fn moved(app: &tauri::AppHandle, moved: &crate::hooks::Moved) {
    tell(
        app,
        Some((
            moved.plane.clone(),
            moved.interrupts().then_some(moved.session),
        )),
    );
}

fn tell(app: &tauri::AppHandle, poke: Poke) {
    if let Some(notifier) = app.try_state::<Notifier>() {
        let poked = notifier
            .poked
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let _ = poked.send(poke);
    }
}

/// A window gone: it has no Inbox open.
pub fn window_gone<R: tauri::Runtime>(app: &impl Manager<R>, window: &str) {
    if let Some(notifier) = app.try_state::<Notifier>() {
        notifier.inbox.forget_window(window);
    }
}

/// **The app was reopened** (macOS: the dock icon, or a notification clicked) with
/// `has_visible_windows` or not. With every window hidden, nothing comes forward by itself, so
/// the window [`brought_back`] says is shown and given the focus; its landing, if a
/// notification left one, opens the Inbox at that chat's group (`askNotices.ts`).
#[cfg(target_os = "macos")]
pub fn reopened(app: &tauri::AppHandle, has_visible_windows: bool) {
    if has_visible_windows {
        return;
    }
    let latest = app
        .try_state::<Notifier>()
        .and_then(|notifier| notifier.latest.plane());
    let showing = app.try_state::<Showing>();
    let label = brought_back(latest.as_ref(), |plane| {
        showing.as_ref().and_then(|showing| showing.holder(plane))
    });
    crate::windows::raise(app, &label);
}

/// The window says whether it has `plane`'s Inbox open (#1694): while it is, and the window
/// is focused with the project in front, no notification is sent about that project's asks.
#[tauri::command]
#[specta::specta]
pub fn inbox_shown(
    window: tauri::Window,
    notifier: tauri::State<'_, Notifier>,
    plane: PlaneId,
    open: bool,
) {
    notifier.inbox.shown(window.label(), &plane, open);
}

/// The notifier's thread: each poke, then whatever follows it while it settles, read once.
fn listen(app: &tauri::AppHandle, heard: &mpsc::Receiver<Poke>) {
    let mut rules = Rules::default();
    // Whether quiet hours are on, as last read: they are looked at every so often while on.
    let mut scheduling = scheduled().is_some();
    loop {
        // Woken by a poke, or by itself: when a burst closes with chats in it not told yet,
        // when typing stops being Quiet, and every so often while Quiet or quiet hours are on.
        let wake = woken_at(&rules, app, scheduling, Instant::now());
        let first = match wake {
            None => match heard.recv() {
                Ok(poke) => Some(poke),
                Err(_) => return,
            },
            Some(at) => match heard.recv_timeout(at.saturating_duration_since(Instant::now())) {
                Ok(poke) => Some(poke),
                Err(RecvTimeoutError::Timeout) => None,
                Err(RecvTimeoutError::Disconnected) => return,
            },
        };
        let mut pokes: Vec<Poke> = first.into_iter().collect();
        if !pokes.is_empty() {
            let until = Instant::now() + SETTLE;
            loop {
                let left = until.saturating_duration_since(Instant::now());
                if left.is_zero() {
                    break;
                }
                match heard.recv_timeout(left) {
                    Ok(poke) => pokes.push(poke),
                    Err(RecvTimeoutError::Timeout) => break,
                    Err(RecvTimeoutError::Disconnected) => return,
                }
            }
        }
        // **Quiet, found once for the whole pass**, so what is held and what its summary
        // counts agree.
        let document = layout();
        let hours = document.as_ref().and_then(QuietHours::of_layout);
        let long = document
            .as_ref()
            .map_or(FINISHED_AFTER, finished_after_of_layout);
        let sources = document
            .as_ref()
            .map_or_else(Sources::default, Sources::of_layout);
        let sound = document.as_ref().is_some_and(expiring_sound_of_layout);
        scheduling = hours.is_some();
        let quiet = app.try_state::<Notifier>().is_some_and(|notifier| {
            notifier
                .sources()
                .is_quiet(hours.is_some_and(covers_now), Instant::now())
        });
        let is_looking = |(plane, session): &Chat| {
            app.try_state::<Planes>()
                .and_then(|planes| planes.held(plane).ok())
                .is_some_and(|held| looking(app, plane, *session, &held).is_looking())
        };
        let (acts, ended) = rules.quiet(quiet, wall_now(), away(app), &is_looking);
        for act in acts {
            send(app, act);
        }
        tell_windows(app, quiet, ended);
        let mut planes: Vec<PlaneId> = Vec::new();
        for (plane, anew) in pokes.into_iter().flatten() {
            if let Some(session) = anew {
                rules.renew(&plane, session);
            }
            if !planes.contains(&plane) {
                planes.push(plane);
            }
        }
        // A project whose snooze ran out is read again by itself (#6018).
        for plane in rules.due(Instant::now()) {
            if !planes.contains(&plane) {
                planes.push(plane);
            }
        }
        for plane in planes {
            read(app, &mut rules, &plane, quiet, long, sources, sound);
        }
        for act in rules.woke(Instant::now(), &is_looking) {
            send(app, act);
        }
    }
}

/// Whether the person is away from every window of the app: none has the keyboard.
fn away(app: &tauri::AppHandle) -> bool {
    !app.webview_windows()
        .values()
        .any(|window| window.is_focused().unwrap_or(false))
}

/// **Whether the person is Quiet, as every chat's proxy holds its live asks by it** (ADR 0121
/// decision 13, #6007): set as Quiet moves, and read by a chat as it starts. The person's, so
/// the app's whole, as Quiet is.
static HOLDS_ASKS_OPEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Whether a chat's proxy holds its live asks open past their minute now (#6007).
pub fn holds_asks_open() -> bool {
    HOLDS_ASKS_OPEN.load(std::sync::atomic::Ordering::SeqCst)
}

/// **Quiet began or ended** (#6007): every open chat in every project holds its live asks open,
/// or no more, where purlis owns the timer (`chats::Chats::hold_asks_open`).
fn hold_asks_open(app: &tauri::AppHandle, quiet: bool) {
    if HOLDS_ASKS_OPEN.swap(quiet, std::sync::atomic::Ordering::SeqCst) == quiet {
        return;
    }
    if let Some(planes) = app.try_state::<crate::planes::Planes>() {
        for held in planes.every_held() {
            held.chats().hold_asks_open(quiet);
        }
    }
}

/// Tells every window whether it is Quiet, and the summary of a Quiet that just `ended`, when
/// either moved.
fn tell_windows(app: &tauri::AppHandle, quiet: bool, ended: Option<QuietEnded>) {
    hold_asks_open(app, quiet);
    use tauri::Emitter;
    let Some(notifier) = app.try_state::<Notifier>() else {
        return;
    };
    let by_hand = notifier.sources().now_on();
    let now = {
        let mut told = notifier.told();
        if told.quiet == quiet && told.by_hand == by_hand && ended.is_none() {
            return;
        }
        told.quiet = quiet;
        told.by_hand = by_hand;
        if ended.is_some() {
            told.ended = ended;
        }
        told.clone()
    };
    let _ = app.emit(QUIET_CHANGED, now);
}

/// How often the notifier looks whether Quiet began or ended by the clock.
const QUIET_LOOK: Duration = Duration::from_secs(60);

/// When the notifier wakes by itself next, if it does: when a burst closes, when typing stops
/// being Quiet, and every [`QUIET_LOOK`] while quiet hours are on or hold chats back. "Quiet
/// now" moving pokes it.
fn woken_at(
    rules: &Rules,
    app: &tauri::AppHandle,
    scheduling: bool,
    now: Instant,
) -> Option<Instant> {
    let typing = app
        .try_state::<Notifier>()
        .and_then(|notifier| notifier.sources().typing_ends(now));
    let look = (scheduling || rules.holds_in_quiet_hours()).then(|| now + QUIET_LOOK);
    [rules.next_due(), typing, look].into_iter().flatten().min()
}

/// Reads `plane`'s asks and its tasks that ended, and does what [`Rules`] says to, while it is
/// `quiet` or not, with `long` as a long finish, and with the Expiring `sound` on or not.
fn read(
    app: &tauri::AppHandle,
    rules: &mut Rules,
    plane: &PlaneId,
    quiet: bool,
    long: Duration,
    sources: Sources,
    sound: bool,
) {
    let Some(held) = app
        .try_state::<Planes>()
        .and_then(|planes| planes.held(plane).ok())
    else {
        for act in rules.forget(plane) {
            send(app, act);
        }
        return;
    };
    // What the ✋ counts (ADR 0121 decision 3): the registry's Blocking asks, its own split
    // (#1693, #1694), with what Snooze all put away left out. An update is not listed, so
    // never notifies as an ask. A snoozed chat's asks are heard anew when its snooze runs
    // out, read again then (#6018, ADR 0121 decision 14).
    let asking = crate::asking::counted(&held);
    let now = Instant::now();
    let end = asking.snoozed_until.map(|until| {
        now + Duration::from_secs(u64::from(until).saturating_sub(crate::sandboxing::now_secs()))
    });
    rules.snoozed_until(plane, end);
    let is_looking = |session| looking(app, plane, session, &held).is_looking();
    let mut acts = rules.heard(&Heard {
        plane,
        asks: &asking.asks,
        looking: &is_looking,
        quiet: &|| quiet,
        now,
    });
    // The Expiring sound (#6110): an ask nearly out sounds once, read as the ✋ counts it.
    acts.extend(rules.expiring(&Expiring {
        plane,
        asks: &asking.asks,
        wall: wall_now(),
        now,
        on: sound,
        quiet,
        looking: &is_looking,
    }));
    // Should know (#6005): the tasks that ended, read off their records as the window reads its
    // finished rows on every move of a chat. Only a failure or a long finish while away is told.
    let finishes = crate::finished::finishes(&held);
    acts.extend(rules.ended(&Ended {
        plane,
        finishes: &finishes,
        long,
        away: away(app),
        looking: &is_looking,
        quiet,
        sources,
    }));
    for act in acts {
        send(app, act);
    }
}

/// What the window showing chat `session` answers about it ([`Looking`]): the tab window whose
/// front tab shows it (#4768), else the window holding `plane`.
///
/// **The window asked is the one holding the chat's project** (charter#126): with a project
/// split into a window of its own, asking the main window would hold a notification back
/// because the person was looking at a different window. **And that window must have the
/// project in front** (#111): every project numbers its chats from one, so "is chat 3 on
/// screen" has as many answers as there are projects open, and only the window knows which one
/// it draws. A task shown inside its session's tab is on screen; a session's own chat hidden
/// behind a task is not (`Chats::looked_at`).
fn looking(
    app: &tauri::AppHandle,
    plane: &PlaneId,
    session: u32,
    held: &crate::planes::Held,
) -> Looking {
    let showing = app.try_state::<Showing>();
    // **Asked of the window that shows the chat** (#4768): a tab window, where the chat is on
    // screen there, and the window holding the project otherwise.
    let window = showing
        .as_ref()
        .and_then(|showing| showing.asked_of(plane, held.chats().window_showing(session)))
        .and_then(|label| app.get_window(&label));
    let Some(window) = window else {
        return Looking {
            window_in_front: false,
            inbox_open: false,
            chat_on_screen: false,
        };
    };
    // A tab window draws its one project, so it has it in front whenever it is drawn.
    let window_in_front = window.is_visible().unwrap_or(false)
        && window.is_focused().unwrap_or(false)
        && showing.is_some_and(|showing| {
            showing.tab_window(window.label()).is_some()
                || showing.is_showing(window.label(), plane)
        });
    Looking {
        window_in_front,
        inbox_open: app
            .try_state::<Notifier>()
            .is_some_and(|notifier| notifier.inbox.is_open(window.label(), plane)),
        chat_on_screen: held.chats().looks_at(session),
    }
}

/// **Where a click on `notice` lands**, for each project it counts chats of, the project of
/// its first chat first.
pub fn landings(notice: &Notice) -> Vec<Landing> {
    let mut landings: Vec<Landing> = Vec::new();
    for (plane, session) in &notice.chats {
        match landings.iter_mut().find(|landing| &landing.plane == plane) {
            Some(landing) => landing.sessions.push(*session),
            None => landings.push(Landing {
                plane: plane.clone(),
                session: *session,
                sessions: vec![*session],
                update: notice.update.clone(),
            }),
        }
    }
    landings
}

/// Does `act` to the system's notifications, and tells each window holding a project it
/// counts where a click lands.
fn send(app: &tauri::AppHandle, act: Act) {
    let Some(notice) = center::act(&*center::here(app), act) else {
        return;
    };
    let landings = landings(&notice);
    if let (Some(notifier), Some(first)) = (app.try_state::<Notifier>(), landings.first()) {
        notifier.latest.sent(&first.plane);
    }
    for landing in landings {
        crate::windows::emit_for_plane(app, &landing.plane.clone(), NOTIFIED, &landing);
    }
}

#[path = "asknotify_center.rs"]
mod center;

#[cfg(target_os = "macos")]
#[path = "asknotify_macos.rs"]
mod macos;

#[cfg(test)]
#[path = "asknotify_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "asknotify_sound_tests.rs"]
mod sound_tests;

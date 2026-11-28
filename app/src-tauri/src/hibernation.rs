// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat the person hibernated by hand, and the Resume that wakes it** (SC-4a, #7630, spec
//! #1391, ADR 0076 as amended 2026-11-25: the run's `hibernated | operator` row).
//!
//! Hibernating ends a chat's program at a turn boundary and drops its engine in the session host
//! ([`purlis_session_protocol::session::Command::Hibernate`]); its conversation is the harness's,
//! resumed by id. **The decision and the act are the app's.** Nothing here reads a harness's
//! output: whether a chat may be hibernated is read from the board, the harness's declared
//! abilities and the app's own records ([`facts_of`]), and answered by the core's
//! [`purlis_core::rested::by_hand`], whose reasons the automatic path reads too (SC-4c, #7632).
//!
//! This is the tracer: one chat, by hand. **Nothing is automatic.** The flag survives a restart
//! as its chat's mark in the record of open chats (SC-4e, #7634): written with the act's intent,
//! before the program ends, taken back where the act aborts, and read at the next launch, which
//! gives a chat it marks no reopen ([`Hibernated::asleep`]). What a ruling on the spec could
//! change sits in two places: the core's [`purlis_core::rested`], the one pure answer to *may
//! it be hibernated*, and [`Hibernated`], the one place that says which chats are.
//!
//! **What it showed is kept** (SC-4b, #7631): as it is hibernated, its screen and scrollback are
//! taken from the session host, fitted to 1 MiB, masked and kept
//! ([`purlis_core::hibernatedscreens`]), for its pane to show read-only with Resume. A screen
//! that cannot be taken, masked or written never stops the hibernation ([`keep_screen`]).
//!
//! **The five-word board `State` gains no value.** A hibernated chat is a per-chat flag read
//! where the board is read, as [`crate::hostlost::Lost`] is.

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

use purlis_core::rested::{AsATask, Facts, NotRested, OwnWorkSeen, Task};
use purlis_core::state::State;
use purlis_core::state::run::{Reason, RunState};

use crate::planes::{PlaneId, Planes};

/// The event the window is sent when a chat is hibernated or woken.
pub const CHANGED: &str = "chat-hibernation-changed";

/// Why a Hibernate pressed while one is already under way does nothing: the first acts.
pub const ALREADY_HIBERNATING: &str =
    "Not hibernated again: purlis is already hibernating this chat.";

/// Why a Resume pressed while the chat is still being hibernated starts nothing: a second
/// program would start beside the one ending.
pub const STILL_HIBERNATING: &str =
    "Not resumed: purlis is still hibernating this chat. Resume it once it is hibernated.";

/// Why the app's own hibernation of a rested chat did not take: the device is no longer over
/// its hot target (SC-4g, #7636). Logged, never said: the person asked for nothing.
pub const AT_THE_TARGET: &str =
    "Not hibernated: the device is no longer over its target of hot chats.";

/// Why a hibernation did not take: the chat's program ended another way as it ended.
pub const ENDED_ANOTHER_WAY: &str =
    "Not hibernated: this chat ended another way while purlis hibernated it.";

/// **Why a chat was not hibernated**, as the window is told it: the core's
/// [`NotRested`], one value for one reason, whose words are the window's
/// (`app/src/hibernationWords.ts`, `docs/ui-copy.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum Why {
    Working,
    Asking,
    Paused,
    Hibernated,
    Ended,
    CannotResume,
    CannotSeeItsOwnWork,
    OwnWork,
    LineComing,
    TaskOpen,
    ReportOwed,
    Observed,
    OnARunner,
    ChildRun,
    LookedAt,
}

impl From<NotRested> for Why {
    fn from(why: NotRested) -> Self {
        match why {
            NotRested::Working => Self::Working,
            NotRested::Asking => Self::Asking,
            NotRested::Paused => Self::Paused,
            NotRested::Hibernated => Self::Hibernated,
            NotRested::Ended => Self::Ended,
            NotRested::CannotResume => Self::CannotResume,
            NotRested::CannotSeeItsOwnWork => Self::CannotSeeItsOwnWork,
            NotRested::OwnWork => Self::OwnWork,
            NotRested::LineComing => Self::LineComing,
            NotRested::TaskOpen => Self::TaskOpen,
            NotRested::ReportOwed => Self::ReportOwed,
            NotRested::Observed => Self::Observed,
            NotRested::OnARunner => Self::OnARunner,
            NotRested::ChildRun => Self::ChildRun,
            NotRested::LookedAt => Self::LookedAt,
        }
    }
}

/// Why a Hibernate pressed on a row did not hibernate its chat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    /// The chat may not be hibernated now ([`by_hand`]).
    Not(NotRested),
    /// The act could not be done: it is under way already, the chat is stopping, or the host
    /// said no. In the window's words.
    Said(String),
}

impl From<String> for Refused {
    fn from(said: String) -> Self {
        Self::Said(said)
    }
}

/// What the window is answered when it asks for a chat's hibernation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Hibernation {
    /// Its program ended as a hibernation.
    Hibernated,
    /// It may not be hibernated now, for this reason.
    Refused { why: Why },
}

/// How a chat that is hibernated was put to rest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum By {
    /// The person did, from its row.
    Operator,
    /// The app did, by itself, for a rested chat while the device was over its hot target
    /// (SC-4g, #7636), or read back from the record at a restart (SC-4e, #7634).
    Idle,
    /// Its session host ended while it rested at a turn's end (#2278), read back from the
    /// record at a restart (SC-4e, #7634): it has no kept screen.
    HostCrash,
}

impl From<purlis_core::reopen::HibernatedBy> for By {
    fn from(by: purlis_core::reopen::HibernatedBy) -> Self {
        use purlis_core::reopen::HibernatedBy;
        match by {
            HibernatedBy::Idle => Self::Idle,
            HibernatedBy::Operator => Self::Operator,
            HibernatedBy::HostCrash => Self::HostCrash,
        }
    }
}

impl From<By> for purlis_core::reopen::HibernatedBy {
    fn from(by: By) -> Self {
        match by {
            By::Idle => Self::Idle,
            By::Operator => Self::Operator,
            By::HostCrash => Self::HostCrash,
        }
    }
}

/// **The mark the record of open chats keeps for a chat hibernated by `by` now** (SC-4e,
/// #7634): what a restart reads it back hibernated by.
pub fn mark_of(by: By) -> purlis_core::reopen::Hibernation {
    purlis_core::reopen::Hibernation {
        by: by.into(),
        at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since| since.as_secs()),
    }
}

/// One chat's hibernation, from the person's press until it is woken or ended another way.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Held {
    by: By,
    /// The act's intent is recorded (spec #1391, *The act, and its latch*, step 4): its
    /// program's end from now on is the hibernation. Before it, the chat is only latched: a
    /// second caller does nothing, and an end of its program is its own.
    intended: bool,
    /// The host has answered that its program ended as a hibernation. Until then it is in
    /// flight: neither Resume nor a second Hibernate is taken.
    settled: bool,
    /// Its program's end was heard after the intent, and read as the hibernation.
    ended: bool,
    /// Its program ended by itself before the intent: the act aborts, and its run takes its own
    /// cause.
    exited: bool,
    /// What the pane typed into it once it was hibernated, kept, never sent to a program that is
    /// gone: the chat's unsent text (SC-4d, #7633), delivered to its woken run once that run is
    /// heard from (SC-4j, #7639).
    unsent: Vec<u8>,
    /// Why the last wake asked of it on its own was refused (SC-4j, #7639): said beside its
    /// unsent text until a wake takes.
    not_woken: Option<String>,
    /// The own work its hibernation ends, as its record held it at the intent: what the wake
    /// line names (SC-4d, #7633), never the record as it reads at the wake.
    own: crate::chats::Own,
}

/// **The most bytes a hibernated chat holds of what its pane typed** (SC-4d, #7633): past it, a
/// send is refused and said on the pane ([`HOLDS_NO_MORE`]), never dropped unsaid.
pub const MOST_UNSENT_BYTES: usize = 64 * 1024;

/// Why what the pane typed into a hibernated chat was not kept: it holds
/// [`MOST_UNSENT_BYTES`] already. Said after "purlis could not send this chat what you typed,
/// or anything after it:".
pub const HOLDS_NO_MORE: &str = "it is hibernated, and already holds 64 KiB of what you typed";

/// **What becomes of what a pane typed into a chat** ([`Hibernated::takes_input`], SC-4d, #7633).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Typed {
    /// The chat has a program to read it: it is sent.
    Send,
    /// The chat is hibernated, or its program is ending: kept as its unsent text. `shown` where
    /// it is hibernated, so its pane shows the text, and is told to read it again.
    Held { shown: bool },
    /// The terminal's own answer to a program that is gone or going: nothing to keep.
    Dropped,
    /// It already holds [`MOST_UNSENT_BYTES`]: not kept, and the pane says so.
    Full,
    /// The chat was woken as this run, which has been handed what was held: sent to it
    /// (SC-4j, #7639). What a pane still showing the hibernated run types after the wake.
    Woken(u32),
}

/// **The chats of a project that are hibernated, or being hibernated, by session**, in memory;
/// the record of open chats carries each one's mark across a restart (SC-4e, #7634). The one
/// answer to *is it hibernated*: the program-end path, the pane, the row and the wake read it
/// here.
#[derive(Debug, Default)]
pub struct Hibernated {
    held: Mutex<HashMap<u32, Held>>,
    /// The chats woken whose new run has not drawn yet, by its session, each with the ULID its
    /// kept screen is under: that screen goes once the run draws ([`Self::drawn`]).
    woken: Mutex<HashMap<u32, String>>,
    /// **What the panes of chats woken typed while they were hibernated**, by the woken run's
    /// session, until that run is first heard from ([`Self::heard_from`], SC-4j, #7639): then it
    /// is typed into it, after its wake line, in the order it was typed.
    to_deliver: Mutex<HashMap<u32, Vec<u8>>>,
    /// Each chat woken, by its hibernated run's session: the run it woke as. A second caller of
    /// the same wake ([`crate::planes::Held::wake`]) is answered this run, and a key a pane
    /// still showing the hibernated run sends goes to it.
    woken_as: Mutex<HashMap<u32, u32>>,
    /// What woken runs never read, handed back to the person ([`Self::hand_back`]).
    returned: Mutex<Vec<Returned>>,
}

/// What a woken run never read, by the sessions whose pane may show it: the woken run's, and
/// the hibernated run's it woke from, where that is known.
#[derive(Debug)]
struct Returned {
    sessions: [Option<u32>; 2],
    back: HandedBack,
}

/// **What a woken chat's pane typed that its woken run never read** (SC-4j, #7639), handed back
/// to the person with why.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct HandedBack {
    /// What was typed, as typed.
    pub text: String,
    /// Why it was not sent, as a sentence.
    pub why: String,
}

/// Why what a woken chat's pane typed was handed back: its woken run ended first.
pub const WOKEN_RUN_ENDED: &str = "Its new run ended before it was ready to read it.";

/// **Joins `bytes` to held text `unsent`**, a Backspace (DEL or BS) taking back the character
/// held before it: false, and nothing joined, where that would hold more than
/// [`MOST_UNSENT_BYTES`].
fn hold(unsent: &mut Vec<u8>, bytes: &[u8]) -> bool {
    let mut next = unsent.clone();
    for &byte in bytes {
        if byte == 0x7f || byte == 0x08 {
            // Back past a character's continuation bytes, to the byte it began with.
            while let Some(last) = next.pop() {
                if last & 0xc0 != 0x80 {
                    break;
                }
            }
        } else {
            next.push(byte);
        }
    }
    if next.len() > MOST_UNSENT_BYTES {
        return false;
    }
    *unsent = next;
    true
}

impl Hibernated {
    fn held(&self) -> std::sync::MutexGuard<'_, HashMap<u32, Held>> {
        self.held.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn woken(&self) -> std::sync::MutexGuard<'_, HashMap<u32, String>> {
        self.woken.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Chat `id` was woken as session `session`: its kept screen stays until that run has drawn.
    pub fn woke(&self, session: u32, id: String) {
        self.woken().insert(session, id);
    }

    /// Session `session`'s pane drew: where it is a woken chat's new run, the ULID of the screen
    /// that is no longer needed, once. `None` for every other session.
    pub fn drawn(&self, session: u32) -> Option<String> {
        let mut woken = self.woken();
        if woken.is_empty() {
            return None;
        }
        woken.remove(&session)
    }

    /// **Takes chat `session`'s hibernating latch** for `by` (spec #1391, step 1). False, with
    /// nothing changed, where it is already hibernated or being hibernated: the first caller is
    /// the one that acts, and a second does nothing. Until [`Self::intend`] an end of its
    /// program is its own, and nothing is held.
    pub fn latch(&self, session: u32, by: By) -> bool {
        let mut held = self.held();
        if held.contains_key(&session) {
            return false;
        }
        // A woken run hibernated before it drew has no screen of its earlier run to let go of:
        // the screen kept now is its own (SC-4b, #7631).
        self.woken().remove(&session);
        held.insert(
            session,
            Held {
                by,
                intended: false,
                settled: false,
                ended: false,
                exited: false,
                unsent: Vec::new(),
                not_woken: None,
                own: crate::chats::Own::default(),
            },
        );
        true
    }

    /// **The host is about to end latched chat `session`'s program as a hibernation**: the
    /// intent, written first, so the exit that follows is read as one (spec #1391, step 4),
    /// with `own`, the own work it ends as its record holds it now: what the wake line names
    /// (SC-4d, #7633), whatever the record says after. False where the chat is not latched, or
    /// its program already ended by itself: the act aborts.
    pub fn intend(&self, session: u32, own: crate::chats::Own) -> bool {
        self.held().get_mut(&session).is_some_and(|held| {
            if held.exited || held.intended {
                return false;
            }
            held.intended = true;
            held.own = own;
            true
        })
    }

    /// The own work hibernated chat `session`'s hibernation ended, as its record held it at the
    /// intent ([`Self::intend`]): what its wake tells it. Nothing for a chat not hibernated.
    pub fn ended_of_its_own(&self, session: u32) -> crate::chats::Own {
        self.held()
            .get(&session)
            .filter(|held| held.settled)
            .map(|held| held.own.clone())
            .unwrap_or_default()
    }

    /// **Chat `session` comes back hibernated by `by`** (SC-4e, #7634): a restart read its
    /// record's mark and found no program of it running, so it is hibernated as one the act
    /// settled, its program gone, and `own` what its hibernation ended of its own work, as its
    /// record held it: what its wake tells it.
    pub fn asleep(&self, session: u32, by: By, own: crate::chats::Own) {
        self.held().insert(
            session,
            Held {
                by,
                intended: true,
                settled: true,
                ended: true,
                exited: false,
                unsent: Vec::new(),
                not_woken: None,
                own,
            },
        );
    }

    /// [`Self::latch`] then [`Self::intend`]: a test's chat whose hibernation is in flight.
    #[cfg(test)]
    pub fn latch_and_intend(&self, session: u32, by: By) -> bool {
        self.latch(session, by) && self.intend(session, crate::chats::Own::default())
    }

    /// **What becomes of `bytes` the pane typed into chat `session`** (SC-4d, #7633): where it is
    /// hibernated, or its intent is recorded, they join its unsent text, up to
    /// [`MOST_UNSENT_BYTES`], and are never sent to a program that is gone. `answering` where
    /// they are only the terminal's answer to its program (a focus report, a cursor or device
    /// reply, a mouse report: [`crate::curation::only_the_terminal_answering`]): nothing the
    /// person typed, so dropped there, never held. Every other chat's are sent.
    ///
    /// The act holds the chat's lane from its latch to its end, so what a pane sends meanwhile
    /// waits behind it and comes here only once the act is done: held by a hibernated chat, or
    /// sent to the program an aborted act left running.
    ///
    /// **A chat already woken** (SC-4j, #7639) takes what its hibernated run's pane types as its
    /// woken run's: held behind what it was typed while hibernated until that run is heard from,
    /// and sent to it after.
    ///
    /// **So does the woken run's own pane** until that run is heard from: the window draws the
    /// woken run in the hibernated one's place as soon as it is told of the wake, and a key typed
    /// there must not reach the harness before what was held, or `fix the bug⏎` could arrive as
    /// `ug⏎` then `fix the b`.
    ///
    /// A Backspace takes back the character held before it, as the harness's own line would.
    pub fn takes_input(&self, session: u32, bytes: &[u8], answering: bool) -> Typed {
        let woken = self.woken_as(session);
        if let Some(waiting) = self.delivering().get_mut(&woken.unwrap_or(session)) {
            if answering {
                return Typed::Dropped;
            }
            return if hold(waiting, bytes) {
                Typed::Held { shown: false }
            } else {
                Typed::Full
            };
        }
        if let Some(woken) = woken {
            return Typed::Woken(woken);
        }
        let mut all = self.held();
        let Some(held) = all.get_mut(&session).filter(|held| held.intended) else {
            return Typed::Send;
        };
        if answering {
            return Typed::Dropped;
        }
        if !hold(&mut held.unsent, bytes) {
            return Typed::Full;
        }
        Typed::Held {
            shown: held.settled,
        }
    }

    /// What the pane typed into hibernated chat `session` that its program never read, where
    /// it typed anything.
    pub fn unsent(&self, session: u32) -> Option<String> {
        self.held()
            .get(&session)
            .filter(|held| held.settled && !held.unsent.is_empty())
            .map(|held| String::from_utf8_lossy(&held.unsent).into_owned())
    }

    /// A wake of hibernated chat `session` asked on its own was refused, for `why`: said on its
    /// pane beside what it holds (SC-4j, #7639).
    pub fn not_woken(&self, session: u32, why: &str) {
        if let Some(held) = self.held().get_mut(&session) {
            held.not_woken = Some(why.to_owned());
        }
    }

    /// Why the last wake of hibernated chat `session` asked on its own was refused, where one
    /// was.
    pub fn why_not_woken(&self, session: u32) -> Option<String> {
        self.held()
            .get(&session)
            .filter(|held| held.settled)
            .and_then(|held| held.not_woken.clone())
    }

    /// The host answered that chat `session`'s program ended as a hibernation: it is
    /// hibernated from now on. False where nothing was in flight for it any more: another end
    /// won, and the chat is not hibernated.
    pub fn settle(&self, session: u32) -> bool {
        self.held().get_mut(&session).is_some_and(|held| {
            held.settled = true;
            true
        })
    }

    /// The act on chat `session` failed, or aborted: its latch, or its intent, goes. Where its
    /// program's end was already heard as the hibernation it is hibernated all the same, and
    /// true; otherwise it is not, and false.
    pub fn failed(&self, session: u32) -> bool {
        let mut held = self.held();
        match held.get_mut(&session) {
            Some(one) if one.ended => {
                one.settled = true;
                true
            }
            Some(_) => {
                held.remove(&session);
                false
            }
            None => false,
        }
    }

    /// Chat `session`'s program ended. True where that end is a hibernation, its intent
    /// recorded or already answered: the end the program-end path reads as no end of the chat.
    /// Before the intent, its program ended by itself: not a hibernation, and the act aborts.
    pub fn ends_as_one(&self, session: u32) -> bool {
        self.held().get_mut(&session).is_some_and(|held| {
            if !held.intended {
                held.exited = true;
                return false;
            }
            held.ended = true;
            true
        })
    }

    /// Who latched chat `session`'s hibernation, settled or not: the cause its run moves to
    /// `hibernated` for (SC-4g, #7636).
    pub fn latched_by(&self, session: u32) -> Option<By> {
        self.held().get(&session).map(|held| held.by)
    }

    /// How chat `session` is hibernated, once the host has answered.
    pub fn of(&self, session: u32) -> Option<By> {
        self.held()
            .get(&session)
            .filter(|held| held.settled)
            .map(|held| held.by)
    }

    /// Whether chat `session` is being hibernated, and the host has not answered yet.
    pub fn in_flight(&self, session: u32) -> bool {
        self.held().get(&session).is_some_and(|held| !held.settled)
    }

    /// The chats that are hibernated, in the order of their numbers.
    pub fn now(&self) -> Vec<u32> {
        let mut now: Vec<u32> = self
            .held()
            .iter()
            .filter(|(_, held)| held.settled)
            .map(|(session, _)| *session)
            .collect();
        now.sort_unstable();
        now
    }

    /// It was ended another way, or is closed. **A woken run that ends before it is heard from**
    /// (SC-4j, #7639) never read what its pane held: that is handed back to the person
    /// ([`Self::hand_back`], [`WOKEN_RUN_ENDED`]), and this answers true, for the window to be
    /// told.
    pub fn forget(&self, session: u32) -> bool {
        self.held().remove(&session);
        let handed = self.ended_unheard(session);
        // The hibernated run is closed in its woken run's place as it wakes: what it woke as is
        // kept until that run closes.
        self.woken_as_now().retain(|_, woken| *woken != session);
        handed
    }

    /// **Hibernated chat `session` is awake as `woken`**: forgotten, and what its pane typed
    /// while it was hibernated is kept for the woken run, to be typed into it once that run is
    /// heard from ([`Self::heard_from`], SC-4j, #7639).
    pub fn awake(&self, session: u32, woken: u32) {
        let unsent = self
            .held()
            .remove(&session)
            .map(|held| held.unsent)
            .unwrap_or_default();
        if !unsent.is_empty() {
            self.delivering().insert(woken, unsent);
        }
        self.woken_as_now().insert(session, woken);
    }

    /// The run hibernated chat `session` was woken as, where it was.
    pub fn woken_as(&self, session: u32) -> Option<u32> {
        self.woken_as_now().get(&session).copied()
    }

    /// **Run `session`'s program ended**: where it is a woken run not yet heard from, what its
    /// pane held is handed back to the person ([`Self::hand_back`], [`WOKEN_RUN_ENDED`]). True
    /// where it was, for the window to be told.
    pub fn ended_unheard(&self, session: u32) -> bool {
        let undelivered = self.delivering().remove(&session);
        undelivered.is_some_and(|bytes| self.hand_back(session, bytes, WOKEN_RUN_ENDED))
    }

    /// Whether woken run `woken` holds what its pane typed, still to be handed to it.
    pub fn holds_for(&self, woken: u32) -> bool {
        self.delivering().contains_key(&woken)
    }

    /// **Woken run `woken` was heard from** (SC-4j, #7639): its harness has its conversation
    /// and reads its input, so what its pane typed while it was hibernated, and after, is
    /// handed over now, once. `None` for every other run. Taken in the run's turn in its lane
    /// ([`crate::planes::Held::deliver_held`]), so a key queued before it joins it and one
    /// queued after follows it.
    pub fn heard_from(&self, woken: u32) -> Option<Vec<u8>> {
        let mut delivering = self.delivering();
        if delivering.is_empty() {
            return None;
        }
        delivering.remove(&woken)
    }

    /// **What woken run `woken`'s pane held was never read** (SC-4j, #7639), for `why`: its run
    /// ended before it was heard from, or typing it in was refused. Handed back to the person,
    /// unsent, on the woken run's pane and on the hibernated run's, whichever the window still
    /// draws, until the window takes it ([`Self::take_handed_back`]), as a refused wake keeps it
    /// on the band. False where it held nothing a person typed.
    ///
    /// Kept in memory only, as what a hibernated chat holds is: an app that quits between the
    /// wake and the woken run's first hook loses it.
    pub fn hand_back(&self, woken: u32, bytes: Vec<u8>, why: &str) -> bool {
        let text = String::from_utf8_lossy(&bytes).into_owned();
        if text.trim().is_empty() {
            return false;
        }
        let old = self
            .woken_as_now()
            .iter()
            .find(|(_, was)| **was == woken)
            .map(|(old, _)| *old);
        let mut returned = self.returned();
        returned.retain(|back| !back.sessions.contains(&Some(woken)));
        returned.push(Returned {
            sessions: [Some(woken), old],
            back: HandedBack {
                text,
                why: why.to_owned(),
            },
        });
        true
    }

    /// What was handed back to chat `session`'s pane, unsent, once: the window keeps it on that
    /// pane until it is put away.
    pub fn take_handed_back(&self, session: u32) -> Option<HandedBack> {
        let mut returned = self.returned();
        let at = returned
            .iter()
            .position(|back| back.sessions.contains(&Some(session)))?;
        Some(returned.remove(at).back)
    }

    fn returned(&self) -> std::sync::MutexGuard<'_, Vec<Returned>> {
        self.returned.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn delivering(&self) -> std::sync::MutexGuard<'_, HashMap<u32, Vec<u8>>> {
        self.to_deliver
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    fn woken_as_now(&self) -> std::sync::MutexGuard<'_, HashMap<u32, u32>> {
        self.woken_as.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// **Keeps what chat `session`, whose id is `id`, shows as it is hibernated** (SC-4b, #7631):
/// its screen and scrollback taken from the session host while its program is still there,
/// fitted to [`purlis_core::hibernatedscreens::MOST_BYTES`] by taking fewer lines of history
/// (`skillgrade::screen_within`), each look masked with every secret purlis handed it, as a
/// task's last screen is (#6309). **Nothing here stops the hibernation**: a screen that cannot
/// be taken, masked or written is said in the log and leaves no file, the one an earlier
/// hibernation kept included, and the chat is hibernated with none.
pub(crate) fn keep_screen(
    root: &std::path::Path,
    host: &dyn crate::host::SessionHost,
    session: u32,
    id: &str,
) {
    keep_screen_taken(root, session, id, |history| {
        crate::skillgrade::screen_within(host, session, history)
    });
}

/// [`keep_screen`], the screen taken by `take` with at most so many lines of history (`None`
/// for all of it).
fn keep_screen_taken(
    root: &std::path::Path,
    session: u32,
    id: &str,
    mut take: impl FnMut(Option<u32>) -> Option<String>,
) {
    use purlis_core::hibernatedscreens;
    let mut unmasked = false;
    let fitted = hibernatedscreens::fitted(|history| {
        let screen = take(history)?;
        let masked = purlis_core::secrets::handedto::masked_for(root, session, id, &screen);
        unmasked |= masked.is_none();
        masked
    });
    let Some(screen) = fitted else {
        // Never an older hibernation's screen in its place.
        hibernatedscreens::forget(root, id);
        if unmasked {
            tracing::info!(
                "purlis: hibernated chat {session}'s screen was not kept: what it was handed \
                 could not be known to mask"
            );
        } else {
            tracing::info!(
                "purlis: hibernated chat {session}'s screen was not kept: it could not be read \
                 within 1 MiB"
            );
        }
        return;
    };
    if let Err(why) = hibernatedscreens::keep(root, id, &screen) {
        tracing::info!("purlis: hibernated chat {session}'s screen was not kept ({why})");
    }
}

/// **What is read of chat `session` to say whether it is rested** (SC-4c, #7632): the board, its
/// harness, its pauses, its pool, the app's records of what it dispatched and who dispatched
/// it, and the dispatch ledger's word on each of its tasks. Nothing a harness printed, and
/// **nothing that acts**: it never calls `Ledger::turn_ended_owing`, which asks for reports
/// (`gathering_never_asks_a_task_for_its_report`).
///
/// The run's state is drawn from the board as [`crate::hostlost::after_the_host`] draws it.
/// Observed chats are not yet a thing the app keeps for a local chat, and a chat's row is never
/// a child run's, so both are read as false until a source for them exists. `looked_at` is the
/// looked-at clock's answer (SC-4g, #7636), which the operator's act does not read.
///
/// **Own work counts while it is out, lapsed or not** (spec #1391): a wait on it that passed its
/// limit leaves the work running, and its wake would find the program gone
/// ([`purlis_core::state::Chat::wakes_on_its_own_work`]).
pub fn facts_of(held: &crate::planes::Held, session: u32, looked_at: bool) -> Facts {
    let board = held.board();
    let glance = board.glance(session);
    let harness = held.chats().harness(session);
    let run = if held.hibernated().of(session).is_some() {
        RunState::Hibernated
    } else if held.pauses().holds(session) {
        RunState::Paused
    } else {
        match glance.state {
            State::Unknown => RunState::Starting,
            State::Running => RunState::Working,
            State::Waiting if glance.asking => RunState::InputRequired(Reason::Asked),
            State::Waiting => RunState::InputRequired(Reason::TurnEnded),
            State::Done => RunState::Completed,
            State::Failed => RunState::Failed,
        }
    };
    // What it waits on of its own, from the hooks' board; where the window's board says it has
    // work out that that one does not detail, it is out all the same.
    let in_flight = held.hooks().board().own_work(session);
    let own_work = match in_flight {
        Some(work) => OwnWorkSeen::InFlight(work),
        None if glance.own_work || glance.wakes_on_its_own_work => OwnWorkSeen::OutAfterItsWait,
        None => OwnWorkSeen::Nothing,
    };
    let as_a_task = held
        .chats()
        .recorded_chat(session)
        .and_then(|chat| chat.from)
        .filter(|from| from.mode == purlis_core::reopen::Mode::Task)
        .map(|from| AsATask::of(&from));
    // Each task's sight first, then the ledger's word on it, so its lock is not held while the
    // board and the chats are read.
    let seen: Vec<_> = held
        .chats()
        .tasks_of(session)
        .into_iter()
        .map(|(task, from)| (task, from, crate::dispatched::seen_now(held, task)))
        .collect();
    let seen_itself = crate::dispatched::seen_now(held, session);
    let (tasks, line_coming) = {
        let ledger = held.tasks().ledger();
        let tasks = seen
            .into_iter()
            .map(|(task, from, seen)| Task {
                state: ledger.state(task, &from, seen),
                hibernated: held.hibernated().of(task).is_some(),
            })
            .collect();
        // A task's report, or its *ended without a report*, is a line typed into this chat:
        // read over at once by `state`, it starts a turn here that has not begun yet.
        (tasks, ledger.line_coming(session, seen_itself, false))
    };
    Facts {
        run,
        resumes_by_id: harness.is_some_and(purlis_core::harness::Harness::resumes_by_id)
            && board.conversation(session).is_some(),
        reports_its_own_work: harness
            .is_some_and(purlis_core::harness::Harness::reports_its_own_work),
        own_work,
        line_coming,
        tasks,
        as_a_task,
        observed: false,
        on_a_runner: on_a_runner(held, session),
        child_run: false,
        looked_at,
    }
}

/// Whether chat `session` runs on a runner: its own fresh one, or a pool's (local or remote).
/// Such a chat's host is reached through a `remote-link` link, which a runner refuses
/// `hibernate` from (it lacks `Power::Reattach`, #7877).
fn on_a_runner(held: &crate::planes::Held, session: u32) -> bool {
    held.chats().runner_of(session).is_some() || held.chats().pool_of(session).is_some()
}

/// **What chat `session`'s row offers of Hibernate** ([`offered`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Offer {
    /// No row: its harness cannot pick a conversation up by its id, or its session host cannot
    /// hibernate it (one before protocol 1.96, or the in-process host).
    None,
    /// A Hibernate row the person may press.
    Hibernate,
    /// A Hibernate row greyed, with the core's reason it may never be hibernated where it runs
    /// (#7877): a runner's chat, whose link refuses the act.
    Greyed(NotRested),
}

/// **Whether chat `session`'s row offers Hibernate** (#7630): its harness picks a conversation
/// up by its id, and its session host can hibernate it (protocol 1.96; never the in-process
/// host). A chat on a runner is offered it greyed, with [`NotRested::OnARunner`] (#7877): no
/// grant of a runner's link is widened for it. Whether it may be hibernated now is
/// [`purlis_core::rested::by_hand`]'s answer, said when it is pressed.
pub fn offered(held: &crate::planes::Held, session: u32) -> Offer {
    if !held
        .chats()
        .harness(session)
        .is_some_and(purlis_core::harness::Harness::resumes_by_id)
    {
        return Offer::None;
    }
    if on_a_runner(held, session) {
        return Offer::Greyed(NotRested::OnARunner);
    }
    if held.chats().sessions().can_hibernate(session) {
        Offer::Hibernate
    } else {
        Offer::None
    }
}

/// The chats of `plane` that are hibernated: what the window reads its rows by as it opens.
#[tauri::command]
#[specta::specta]
pub fn hibernated_chats(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
) -> Result<Vec<u32>, String> {
    Ok(planes.held(&plane)?.hibernated().now())
}

/// Chat `session` as the window reads its hibernation.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct ChatHibernation {
    pub by: By,
    /// What the pane typed into it once its program was ending, which no program read: its
    /// unsent text, held in memory, never lost (SC-4d, #7633). Absent where it typed nothing.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional)]
    pub unsent: Option<String>,
    /// Why purlis could not wake it when its pane typed into it or a tell reached it, in the
    /// core's words (SC-4j, #7639): what it holds stays unsent. Absent where nothing was refused.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional)]
    pub not_woken: Option<String>,
}

/// Chat `session`, where it is hibernated.
#[tauri::command]
#[specta::specta]
pub fn chat_hibernated(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    session: u32,
) -> Result<Option<ChatHibernation>, String> {
    let held = planes.held(&plane)?;
    let hibernated = held.hibernated();
    Ok(hibernated.of(session).map(|by| ChatHibernation {
        by,
        unsent: hibernated.unsent(session),
        not_woken: hibernated.why_not_woken(session),
    }))
}

/// **What chat `session`'s pane typed that its woken run never read** (SC-4j, #7639), handed
/// to the window once, which keeps it on the pane until it is put away. None where nothing was
/// handed back.
#[tauri::command]
#[specta::specta]
pub fn chat_unsent_handed_back(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    session: u32,
) -> Result<Option<HandedBack>, String> {
    Ok(planes.held(&plane)?.hibernated().take_handed_back(session))
}

/// **What hibernated chat `session` showed as it was hibernated** (SC-4b, #7631): what its pane
/// draws read-only with Resume. Null where it is not hibernated, has no id, or nothing was
/// kept: a missing, corrupt or oversize file reads as none kept.
#[tauri::command]
#[specta::specta]
pub async fn hibernated_chat_screen(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    session: u32,
) -> Result<Option<String>, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("reading a hibernated chat's screen", move || {
        Ok(screen_of(&held, session))
    })
    .await
}

/// [`hibernated_chat_screen`], of a plane held.
pub(crate) fn screen_of(held: &crate::planes::Held, session: u32) -> Option<String> {
    held.hibernated().of(session)?;
    let id = held.chats().recorded_chat(session)?.identity.id?;
    purlis_core::hibernatedscreens::kept(held.root(), &id)
}

/// Hibernate on a chat's row (SC-4a, #7630): the person's act. Where the chat may not be
/// hibernated now it is answered why, as a typed reason the window words (SC-4c, #7632);
/// where the act cannot be done, in a sentence. Otherwise its program ends and the window is
/// told. `own_work_named` is the window's word that the person was asked about the chat's own
/// work and said yes, with the id of each piece the ask named: only then is a chat with own
/// work hibernated, and only where it has no piece the ask did not name (SC-4d, #7633). The
/// window's alone (`WINDOW_ONLY`).
#[tauri::command]
#[specta::specta]
pub async fn hibernate_chat(
    app: tauri::AppHandle,
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    session: u32,
    own_work_named: Option<Vec<String>>,
) -> Result<Hibernation, String> {
    use tauri::Emitter;
    let held = planes.held(&plane)?;
    let done = tauri::async_runtime::spawn_blocking(move || match &own_work_named {
        Some(named) => held.hibernate_chat_ending_its_own_work(session, named),
        None => held.hibernate_chat(session),
    })
    .await
    .map_err(|err| format!("purlis could not hibernate the chat: {err}"))?;
    match done {
        Ok(()) => {}
        Err(Refused::Not(why)) => return Ok(Hibernation::Refused { why: why.into() }),
        Err(Refused::Said(said)) => return Err(said),
    }
    let _ = app.emit(
        CHANGED,
        Changed {
            plane,
            session,
            hibernated: true,
            woken: None,
        },
    );
    Ok(Hibernation::Hibernated)
}

/// Told each [`Changed`] the core says on its own: a hibernated chat's unsent text moved
/// (SC-4d, #7633).
pub type Teller = std::sync::Arc<dyn Fn(Changed) + Send + Sync + 'static>;

/// What the window is told when a chat is hibernated or woken, or its unsent text changes.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct Changed {
    pub plane: PlaneId,
    pub session: u32,
    /// Whether it is hibernated now.
    pub hibernated: bool,
    /// **The run it was woken as**, where purlis woke it on its own: a message typed in its
    /// pane, or a parent's tell (SC-4j, #7639). The window draws that run in its place, as
    /// after a Resume. Absent otherwise.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional)]
    pub woken: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hibernated_chat_is_kept_until_it_is_forgotten() {
        let hibernated = Hibernated::default();
        assert!(hibernated.latch_and_intend(3, By::Operator));
        assert!(hibernated.settle(3));
        assert_eq!(hibernated.of(3), Some(By::Operator));
        assert_eq!(hibernated.of(4), None);
        assert_eq!(hibernated.now(), [3]);
        hibernated.forget(3);
        assert_eq!(hibernated.of(3), None);
    }

    #[test]
    fn a_chat_is_hibernated_only_once_the_host_has_answered() {
        let hibernated = Hibernated::default();
        assert!(hibernated.latch_and_intend(3, By::Operator));
        assert!(hibernated.in_flight(3));
        assert_eq!(hibernated.of(3), None, "no Resume while it is in flight");
        assert_eq!(hibernated.now(), Vec::<u32>::new());
        assert!(
            hibernated.ends_as_one(3),
            "its end is read as the hibernation"
        );
        assert!(hibernated.settle(3));
        assert!(!hibernated.in_flight(3));
        assert_eq!(hibernated.of(3), Some(By::Operator));
    }

    #[test]
    fn what_a_pane_types_is_sent_until_the_intent_then_held_and_shown_once_hibernated() {
        let hibernated = Hibernated::default();
        assert_eq!(hibernated.takes_input(3, b"a", false), Typed::Send);
        assert!(hibernated.latch(3, By::Operator));
        assert_eq!(
            hibernated.takes_input(3, b"b", false),
            Typed::Send,
            "latched, not intended"
        );
        assert!(hibernated.intend(3, crate::chats::Own::default()));
        assert_eq!(
            hibernated.takes_input(3, b"c", false),
            Typed::Held { shown: false }
        );
        assert!(hibernated.settle(3));
        assert_eq!(
            hibernated.takes_input(3, b"d", false),
            Typed::Held { shown: true }
        );
        assert_eq!(hibernated.unsent(3).as_deref(), Some("cd"));
        assert_eq!(hibernated.takes_input(4, b"e", false), Typed::Send);
    }

    #[test]
    fn the_terminals_answer_is_dropped_by_a_hibernating_chat_and_never_held() {
        let hibernated = Hibernated::default();
        assert_eq!(hibernated.takes_input(3, b"\x1b[I", true), Typed::Send);
        assert!(hibernated.latch_and_intend(3, By::Operator));
        assert_eq!(hibernated.takes_input(3, b"\x1b[I", true), Typed::Dropped);
        assert!(hibernated.settle(3));
        assert_eq!(hibernated.unsent(3), None);
    }

    #[test]
    fn a_hibernated_chat_holds_no_more_than_its_bound() {
        let hibernated = Hibernated::default();
        assert!(hibernated.latch_and_intend(3, By::Operator));
        assert!(hibernated.settle(3));
        let most = vec![b'x'; MOST_UNSENT_BYTES];
        assert_eq!(
            hibernated.takes_input(3, &most, false),
            Typed::Held { shown: true }
        );
        assert_eq!(hibernated.takes_input(3, b"y", false), Typed::Full);
        assert_eq!(
            hibernated.unsent(3).map(|unsent| unsent.len()),
            Some(MOST_UNSENT_BYTES),
            "what was held stays"
        );
    }

    #[test]
    fn what_a_woken_run_holds_before_it_is_heard_from_stops_at_the_most_a_chat_holds() {
        // SC-4j review: keys held for a woken run, under either number, are bounded as a
        // hibernated chat's are.
        let hibernated = Hibernated::default();
        assert!(hibernated.latch_and_intend(3, By::Operator));
        assert!(hibernated.settle(3));
        assert_eq!(
            hibernated.takes_input(3, b"go", false),
            Typed::Held { shown: true }
        );
        hibernated.awake(3, 7);
        let most = vec![b'a'; MOST_UNSENT_BYTES - 2];
        assert_eq!(
            hibernated.takes_input(7, &most, false),
            Typed::Held { shown: false }
        );
        assert_eq!(hibernated.takes_input(7, b"!", false), Typed::Full);
        assert_eq!(hibernated.takes_input(3, b"!", false), Typed::Full);
        assert_eq!(
            hibernated.takes_input(7, b"\x1b[I", true),
            Typed::Dropped,
            "the terminal's answer is no one's typing"
        );
        assert_eq!(
            hibernated.heard_from(7).map(|held| held.len()),
            Some(MOST_UNSENT_BYTES)
        );
    }

    #[test]
    fn a_backspace_takes_back_a_whole_character_and_nothing_before_what_is_held() {
        let mut held = "café".as_bytes().to_vec();
        assert!(hold(&mut held, b"\x7f"));
        assert_eq!(held, b"caf");
        assert!(hold(&mut held, b"\x08\x7f\x7f\x7fx"));
        assert_eq!(held, b"x");
    }

    #[test]
    fn a_woken_run_is_handed_what_was_typed_once_it_is_heard_from_and_what_follows_after() {
        let hibernated = Hibernated::default();
        assert!(hibernated.latch_and_intend(3, By::Operator));
        assert!(hibernated.settle(3));
        assert_eq!(
            hibernated.takes_input(3, b"hello", false),
            Typed::Held { shown: true }
        );
        hibernated.awake(3, 7);
        assert_eq!(hibernated.of(3), None, "it is awake");
        assert_eq!(hibernated.woken_as(3), Some(7));
        // Its old pane types on before the woken run is heard from: behind what it held.
        assert_eq!(
            hibernated.takes_input(3, b" there", false),
            Typed::Held { shown: false }
        );
        assert_eq!(hibernated.heard_from(3), None, "not the woken run");
        assert_eq!(
            hibernated.heard_from(7).as_deref(),
            Some(&b"hello there"[..])
        );
        assert_eq!(hibernated.heard_from(7), None, "handed over once");
        assert_eq!(hibernated.takes_input(3, b"!", false), Typed::Woken(7));

        assert!(hibernated.latch_and_intend(4, By::Operator));
        assert!(hibernated.settle(4));
        hibernated.awake(4, 8);
        assert_eq!(
            hibernated.heard_from(8),
            None,
            "nothing typed, nothing handed"
        );
        hibernated.forget(8);
        assert_eq!(hibernated.woken_as(4), None, "the woken run is closed");
    }

    #[test]
    fn a_second_intent_leaves_the_first_in_flight() {
        let hibernated = Hibernated::default();
        assert!(hibernated.latch_and_intend(3, By::Operator));
        assert!(
            !hibernated.latch_and_intend(3, By::Operator),
            "the first press acts"
        );
        assert!(hibernated.in_flight(3));
    }

    #[test]
    fn a_failed_act_is_forgotten_unless_its_program_already_ended_as_one() {
        let hibernated = Hibernated::default();
        assert!(hibernated.latch_and_intend(3, By::Operator));
        assert!(!hibernated.failed(3));
        assert!(!hibernated.in_flight(3));
        assert_eq!(hibernated.of(3), None);

        assert!(hibernated.latch_and_intend(4, By::Operator));
        assert!(hibernated.ends_as_one(4));
        assert!(hibernated.failed(4), "its program ended as a hibernation");
        assert_eq!(hibernated.of(4), Some(By::Operator));
    }

    const CHAT: &str = "01K6H1BERNATED00000000000B";

    fn project() -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().expect("a directory");
        let root = std::fs::canonicalize(dir.path()).expect("canonical");
        (dir, root)
    }

    #[test]
    fn a_handed_secret_on_the_screen_is_masked_in_the_kept_screen() {
        // An obvious fake: no credential is or was this.
        let (_dir, root) = project();
        purlis_core::secrets::handedto::remember(
            &root,
            31,
            Some(CHAT),
            &purlis_core::secrets::handedto::Entry::new("team", "github-token"),
            "FAKE-VAULT-VALUE-7631",
        )
        .expect("named");

        keep_screen_taken(&root, 31, CHAT, |_| {
            Some("token: FAKE-VAULT-VALUE-7631\r\n".to_owned())
        });

        assert_eq!(
            purlis_core::hibernatedscreens::kept(&root, CHAT).as_deref(),
            Some("token: ‹secret: github-token›\r\n")
        );
    }

    #[test]
    fn a_screen_that_cannot_be_masked_is_not_kept_and_takes_an_older_one_with_it() {
        let (_dir, root) = project();
        purlis_core::hibernatedscreens::keep(&root, CHAT, "an older hibernation's").unwrap();
        // A line naming what chat 32 was handed that purlis did not seal: what it was handed
        // cannot be known, so nothing can be masked.
        let named = purlis_core::secrets::handedto::dir(&root);
        std::fs::create_dir_all(&named).expect("made");
        std::fs::write(
            named.join("32.names"),
            format!(
                "{{\"vault\":\"team\",\"key\":\"k\",\"shown\":[],\"seal\":\"{}\"}}\n",
                "0".repeat(64)
            ),
        )
        .expect("named");

        keep_screen_taken(&root, 32, CHAT, |_| Some("token: whatever\r\n".to_owned()));

        assert_eq!(purlis_core::hibernatedscreens::kept(&root, CHAT), None);
        assert!(
            !purlis_core::hibernatedscreens::dir(&root)
                .join(format!("{CHAT}.screen"))
                .exists()
        );
    }

    #[test]
    fn a_scrollback_past_a_mebibyte_is_taken_again_with_fewer_lines_and_kept_within_it() {
        let (_dir, root) = project();
        let line = format!("\x1b[32m{}\x1b[0m\r\n", "x".repeat(200));
        let mut asked = Vec::new();
        keep_screen_taken(&root, 33, CHAT, |history| {
            asked.push(history);
            let lines = history.map_or(20_000, |history| history.min(20_000)) + 24;
            Some(line.repeat(lines as usize))
        });
        let kept = purlis_core::hibernatedscreens::kept(&root, CHAT).expect("kept");
        assert!(kept.len() <= purlis_core::hibernatedscreens::MOST_BYTES);
        assert_eq!(asked[0], None, "the whole of it first");
        assert!(asked.len() > 1);
    }

    #[test]
    fn a_woken_chat_s_screen_is_let_go_of_once_its_new_run_draws_and_only_then() {
        let hibernated = Hibernated::default();
        assert_eq!(hibernated.drawn(9), None);
        hibernated.woke(9, CHAT.to_owned());
        assert_eq!(hibernated.drawn(8), None, "another chat's pane");
        assert_eq!(hibernated.drawn(9).as_deref(), Some(CHAT));
        assert_eq!(hibernated.drawn(9), None, "once");
    }

    #[test]
    fn a_woken_chat_hibernated_again_before_it_drew_has_no_earlier_screen_to_let_go_of() {
        let hibernated = Hibernated::default();
        hibernated.woke(9, CHAT.to_owned());
        assert!(hibernated.latch_and_intend(9, By::Operator));
        assert!(hibernated.settle(9));
        assert_eq!(hibernated.drawn(9), None, "the screen kept now is its own");
    }

    #[test]
    fn gathering_never_asks_a_task_for_its_report() {
        // SC-4c (#7632): `turn_ended_owing` is an act, which asks a task for its report; what
        // is read to say whether a chat is rested only reads.
        let source = include_str!("hibernation.rs");
        let gathering = &source[source
            .find("pub fn facts_of(")
            .expect("the gathering is here")..];
        let gathering = &gathering[..gathering.find("\n}\n").expect("its end")];
        assert!(gathering.contains("ledger.state("), "it reads the ledger");
        assert!(!gathering.contains(concat!("turn_ended", "_owing")));
    }

    #[test]
    fn every_reason_reaches_the_window_as_its_own_value() {
        use NotRested::*;
        let all = [
            Working,
            Asking,
            Paused,
            Hibernated,
            Ended,
            CannotResume,
            CannotSeeItsOwnWork,
            OwnWork,
            LineComing,
            TaskOpen,
            ReportOwed,
            Observed,
            OnARunner,
            ChildRun,
            LookedAt,
        ];
        let sent: std::collections::HashSet<String> = all
            .into_iter()
            .map(|why| serde_json::to_string(&Why::from(why)).expect("it is sent"))
            .collect();
        assert_eq!(sent.len(), all.len(), "no two reasons share a value");
        for why in all {
            // The core's own spelling: one name for one reason, wherever it is read.
            assert_eq!(
                serde_json::to_string(&Why::from(why)).expect("sent"),
                serde_json::to_string(&why).expect("the core's")
            );
        }
    }

    #[test]
    fn an_end_that_won_leaves_nothing_to_settle() {
        let hibernated = Hibernated::default();
        assert!(hibernated.latch_and_intend(3, By::Operator));
        hibernated.forget(3);
        assert!(!hibernated.settle(3));
        assert_eq!(hibernated.of(3), None);
    }
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The skill run open in each chat** (SK-3, #2766), told to the window for the chip on its
//! tab: the skill, the run's id and the tool calls made inside it so far.
//!
//! It reads the hook lines with the same code the event log's writer records runs with
//! ([`purlis_core::skillspan::Tracker`]), keyed by the chat's number, so the chip and the
//! recorded run never disagree. **In memory only**: the run itself is recorded by the event log
//! and the plane's skill log, and this goes with the chat.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use purlis_core::skills::runs::Ended;
use purlis_core::skillspan::Tracker;

use crate::planes::PlaneId;

/// The event the window is sent when a chat's open skill run changes.
pub const EVENT: &str = "chat-skill-run";

/// A skill run open in a chat, as its tab's chip shows it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct OpenRun {
    /// The skill, by the name its harness started it by.
    pub skill: String,
    /// The skill it is a run of, by its own name, as the core counts it
    /// ([`purlis_core::skilluse::own_name`]): `secrets` for opencode's `purlis-secrets`. The
    /// skill whose tab its chip opens.
    pub name: String,
    /// The run's id, as the skill's Runs panel lists it.
    pub id: String,
    /// Tool calls made inside it so far, each once.
    pub tool_calls: u32,
    /// The tokens the chat has spent since it opened, as its harness last said them (SK-26,
    /// #3694): what the eventlog records the run's tokens from, live. `None` where the harness
    /// says none, so the chip shows no zero. A `u32`, as the window reads it whole: a run past
    /// four billion tokens says the most it can.
    pub tokens: Option<u32>,
    /// The level of the copy it is of, as a person reads it (`persona`, `local workspace`), where
    /// its start knew it: the scope its chip opens the Skill studio at (#3694).
    pub level: Option<String>,
}

/// What `chat-skill-run` carries, and what `chat_skill_runs` answers a list of: one chat's open
/// run, or that it has none.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct ChatSkillRun {
    pub plane: PlaneId,
    pub session: u32,
    /// The innermost run open, or null: none is, or the turn it was in ended.
    pub run: Option<OpenRun>,
}

/// Told each change of a chat's open run.
pub type Teller = Arc<dyn Fn(ChatSkillRun) + Send + Sync + 'static>;

/// **The tokens chat `chat` has spent**, as its harness last said them
/// ([`purlis_core::usage::spent`]), or `None` where it said none: what a run's tokens are counted
/// from. Handed in, so a test reads no machine's files.
pub type Tokens = Arc<dyn Fn(u32) -> Option<u64> + Send + Sync + 'static>;

/// **What a watched chat did** (SK-16): what a grade waits for of the chat it asked purlis for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Watched {
    /// A run of this skill started in it, by the name its harness started it by.
    Started(String),
    /// Its turn ended.
    TurnEnded,
    /// It asked the person something: a permission, or a notice that it waits on them.
    Asked,
    /// Its program got as far as its session's start.
    Began,
    /// It closed, or its session ended.
    Closed,
    /// **What its turn ended saying** (SK-55): the harness's last message, as its `Stop` hook
    /// handed it (#1981), heard just before the turn's end. What an outcome eval's judge reads,
    /// and what a judge answers with.
    Said(String),
    /// **Its read guard refused a call of it** (SK-55): in a judge, a reach past what it may read.
    /// Told as it happens.
    ReadPast,
    /// **A call of it was refused by anything else**: in a judge, a fault of purlis's, such as a
    /// guard that crashed or did not answer, which says nothing of the judge.
    RefusedOtherwise,
}

/// The open skill runs of one project's chats.
pub struct SkillRuns {
    plane: PlaneId,
    runs: Mutex<Tracker>,
    teller: Mutex<Option<Teller>>,
    /// How a chat's tokens are read ([`Self::count_tokens_by`]): none until it is told.
    tokens: Mutex<Option<Tokens>>,
    /// The tokens each open run's chat had spent as the run was first seen open with a figure,
    /// by chat and run id: what its tokens so far are counted from. A chat's go once it has no
    /// run open.
    started_at: Mutex<std::collections::HashMap<(u32, String), u64>>,
    /// The chats a grade watches, by number, each told what it did ([`Self::watch`]).
    watched: Mutex<std::collections::HashMap<u32, std::sync::mpsc::Sender<Watched>>>,
}

impl SkillRuns {
    /// No run open in any chat of `plane`.
    pub fn new(plane: PlaneId) -> Arc<Self> {
        Arc::new(Self {
            plane,
            runs: Mutex::new(Tracker::default()),
            teller: Mutex::new(None),
            tokens: Mutex::new(None),
            started_at: Mutex::new(std::collections::HashMap::new()),
            watched: Mutex::new(std::collections::HashMap::new()),
        })
    }

    /// **Watches chat `chat`** for a grade (SK-16): each skill run that starts in it, the end of
    /// its turn and its close are told to what this answers, until [`Self::unwatch`]. Asked
    /// before the chat starts, so nothing it does is missed.
    pub fn watch(&self, chat: u32) -> std::sync::mpsc::Receiver<Watched> {
        let (tell, told) = std::sync::mpsc::channel();
        self.watched().insert(chat, tell);
        told
    }

    /// Stops watching chat `chat`.
    pub fn unwatch(&self, chat: u32) {
        self.watched().remove(&chat);
    }

    fn watched(
        &self,
    ) -> MutexGuard<'_, std::collections::HashMap<u32, std::sync::mpsc::Sender<Watched>>> {
        self.watched.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Tells the grade watching chat `chat`, where one does.
    fn tell_watching(&self, chat: u32, what: Watched) {
        if let Some(tell) = self.watched().get(&chat) {
            let _ = tell.send(what);
        }
    }

    /// **What chat `chat`'s turn ended saying, `text`** (SK-55), told to the grade watching it,
    /// where one does: no other chat's last message goes anywhere from here.
    pub fn said(&self, chat: u32, text: String) {
        self.tell_watching(chat, Watched::Said(text));
    }

    /// Who is told each change from now on.
    pub fn tell_to(&self, teller: Teller) {
        *self.teller.lock().unwrap_or_else(PoisonError::into_inner) = Some(teller);
    }

    /// How each chat's tokens are read from now on, for the tokens of its open run (SK-26).
    pub fn count_tokens_by(&self, tokens: Tokens) {
        *self.tokens.lock().unwrap_or_else(PoisonError::into_inner) = Some(tokens);
    }

    /// **Run `run` of chat `chat` with its tokens so far** (SK-26): what the chat has spent since
    /// the run was first seen open with a figure. Read here, outside every lock this holds, and
    /// only for a run that is open: a chat with none reads nothing. A run first seen before its
    /// harness said any usage is counted from the first figure it says.
    ///
    /// The read takes a moment, in which another hook of the chat may have moved its run on: the
    /// run as it is once the figure is read is what is told, so a run the turn's end closed
    /// meanwhile is never told open again.
    fn with_tokens(&self, chat: u32, run: Option<OpenRun>) -> Option<OpenRun> {
        if run.is_none() {
            lock(&self.started_at).retain(|(of, _), _| *of != chat);
            return None;
        }
        let reader = lock(&self.tokens).clone();
        let now = reader.and_then(|tokens| tokens(chat));
        let Some(mut run) = open_of(&self.runs(), &chat.to_string()) else {
            lock(&self.started_at).retain(|(of, _), _| *of != chat);
            return None;
        };
        let mut started = lock(&self.started_at);
        let start = match now {
            Some(now) => Some(*started.entry((chat, run.id.clone())).or_insert(now)),
            None => started.get(&(chat, run.id.clone())).copied(),
        };
        run.tokens = now
            .zip(start)
            .map(|(now, then)| u32::try_from(now.saturating_sub(then)).unwrap_or(u32::MAX));
        Some(run)
    }

    /// A tool hook of chat `call.chat`: one that starts a skill opens its run, and every one
    /// inside a run counts toward it ([`Tracker::call`]).
    pub fn heard(&self, call: &purlis_core::hookwire::ToolCall) {
        // A call put to the person is an ask, which a grade's chat never waits on (SK-16).
        if call.decision == purlis_core::hookwire::Decision::Ask {
            self.tell_watching(call.chat, Watched::Asked);
        }
        if call.decision == purlis_core::hookwire::Decision::Deny {
            let judges_own = call.rule.as_deref() == Some(purlis_core::toolhooks::JUDGE_READ_RULE);
            self.tell_watching(
                call.chat,
                if judges_own {
                    Watched::ReadPast
                } else {
                    Watched::RefusedOtherwise
                },
            );
        }
        let key = call.chat.to_string();
        let (told, opened) = {
            let mut runs = self.runs();
            let was = open_of(&runs, &key);
            runs.call(&key, call, now_ms(), || None);
            let now = open_of(&runs, &key);
            if now == was {
                return;
            }
            // A call only opens runs: an innermost run of another id is one that opened.
            let opened = now
                .as_ref()
                .filter(|now| was.as_ref().is_none_or(|was| was.id != now.id))
                .map(|now| now.skill.clone());
            (now, opened)
        };
        if let Some(skill) = opened {
            self.tell_watching(call.chat, Watched::Started(skill));
        }
        let told = self.with_tokens(call.chat, told);
        self.tell(call.chat, told);
    }

    /// A state hook of chat `report.chat`: the turn's end, a new prompt and the session's end
    /// close every run open in it, and a new prompt opens the run of a skill Use skill… typed
    /// into it ([`Tracker::report`]).
    pub fn reported(&self, report: &purlis_core::hookwire::Report) {
        let key = report.chat.to_string();
        let told = {
            let mut runs = self.runs();
            let reported = runs.report(&key, report, now_ms(), || None);
            if report.agent.is_none() {
                match report.event {
                    purlis_core::state::Event::Stop => {
                        self.tell_watching(report.chat, Watched::TurnEnded);
                    }
                    purlis_core::state::Event::SessionStart => {
                        self.tell_watching(report.chat, Watched::Began);
                    }
                    purlis_core::state::Event::SessionEnd => {
                        self.tell_watching(report.chat, Watched::Closed);
                    }
                    // A chat sitting idle at its prompt is nudged, which asks nothing.
                    purlis_core::state::Event::Notification if !report.detail.idle => {
                        self.tell_watching(report.chat, Watched::Asked);
                    }
                    _ => {}
                }
            }
            for opened in &reported.opened {
                self.tell_watching(report.chat, Watched::Started(opened.skill.clone()));
            }
            if reported.closed.is_empty() && reported.opened.is_empty() {
                return;
            }
            open_of(&runs, &key)
        };
        let told = self.with_tokens(report.chat, told);
        self.tell(report.chat, told);
    }

    /// **Use skill… typed `typed` into chat `chat`**, unsent (SK-12): its run opens, and the chip
    /// shows it, as the chat's next prompt does.
    pub fn typed(&self, chat: u32, typed: purlis_core::skillspan::Typed) {
        self.runs().typed(&chat.to_string(), typed, now_ms());
    }

    /// **A curation action put prompt `prompted` in chat `chat`**, unsent (SK-20): the runs the
    /// turn it is sent in opens are the action's.
    pub fn prompted(&self, chat: u32, prompted: purlis_core::skillspan::Prompted) {
        self.runs().prompted(&chat.to_string(), prompted);
    }

    /// Chat `chat` is closed: a run open in it goes with it, and the window is told.
    pub fn closed(&self, chat: u32) {
        self.tell_watching(chat, Watched::Closed);
        let closed = self
            .runs()
            .close(&chat.to_string(), Ended::Session, now_ms());
        if !closed.is_empty() {
            let told = self.with_tokens(chat, None);
            self.tell(chat, told);
        }
    }

    /// The open run of every chat that has one, for a window that has just opened.
    pub fn now(&self, chats: impl IntoIterator<Item = u32>) -> Vec<ChatSkillRun> {
        let open: Vec<(u32, OpenRun)> = {
            let runs = self.runs();
            chats
                .into_iter()
                .filter_map(|chat| open_of(&runs, &chat.to_string()).map(|run| (chat, run)))
                .collect()
        };
        // Their tokens read once the runs' lock is let go, as a change's are.
        open.into_iter()
            .map(|(chat, run)| ChatSkillRun {
                plane: self.plane.clone(),
                session: chat,
                run: self.with_tokens(chat, Some(run)),
            })
            .collect()
    }

    fn runs(&self) -> MutexGuard<'_, Tracker> {
        self.runs.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn tell(&self, chat: u32, run: Option<OpenRun>) {
        // Taken out of the lock before it runs, as an answer is.
        let teller = self
            .teller
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        if let Some(teller) = teller {
            teller(ChatSkillRun {
                plane: self.plane.clone(),
                session: chat,
                run,
            });
        }
    }
}

/// Now, in milliseconds since 1970.
fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|since| u64::try_from(since.as_millis()).ok())
        .unwrap_or_default()
}

/// The innermost run open in chat `key`, as the chip shows it.
fn open_of(runs: &Tracker, key: &str) -> Option<OpenRun> {
    runs.inside(key).map(|open| OpenRun {
        skill: open.skill.clone(),
        name: purlis_core::skilluse::own_name(&open.skill, open.whose.level).to_owned(),
        id: open.id.clone(),
        tool_calls: open.signals.tool_calls,
        // Counted by the caller, outside the runs' lock ([`SkillRuns::with_tokens`]).
        tokens: None,
        level: open.whose.level.map(|level| level.word()),
    })
}

/// A lock, through poisoning too: a panic elsewhere must not cost a chip.
fn lock<T>(held: &Mutex<T>) -> MutexGuard<'_, T> {
    held.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests;

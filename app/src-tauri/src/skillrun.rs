// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The skill run open in each chat** (SK-3, #2766), told to the window for the chip on its
//! tab: the skill, the run's id and the tool calls made inside it so far.
//!
//! It follows the same lines the event log's writer records a run from: a tool call that
//! invokes a skill opens one ([`purlis_core::hookwire::ToolCall::skill`]), every tool call
//! counts toward it, and the turn's end, a new prompt or the session's end closes it
//! ([`purlis_core::skills::runs::Spans`]). **In memory only**: the run itself is recorded by the event
//! log and the plane's skill log, and this goes with the chat.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use purlis_core::skills::runs::{Ended, Heard, Invoked, Spans, StartedBy};

use crate::planes::PlaneId;

/// The event the window is sent when a chat's open skill run changes.
pub const EVENT: &str = "chat-skill-run";

/// A skill run open in a chat, as its tab's chip shows it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct OpenRun {
    /// The skill, by the name it was invoked by.
    pub skill: String,
    /// The run's id, as the skill's Runs panel lists it.
    pub id: String,
    /// Tool calls made inside it so far, each once.
    pub tool_calls: u32,
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

/// The open skill runs of one project's chats.
pub struct SkillRuns {
    plane: PlaneId,
    spans: Mutex<Spans>,
    teller: Mutex<Option<Teller>>,
}

impl SkillRuns {
    /// No run open in any chat of `plane`.
    pub fn new(plane: PlaneId) -> Arc<Self> {
        Arc::new(Self {
            plane,
            spans: Mutex::new(Spans::default()),
            teller: Mutex::new(None),
        })
    }

    /// Who is told each change from now on.
    pub fn tell_to(&self, teller: Teller) {
        *self.teller.lock().unwrap_or_else(PoisonError::into_inner) = Some(teller);
    }

    /// A tool hook of chat `call.chat`: one that invokes a skill opens its run, and every one
    /// inside a run counts toward it. A call a guard refused invokes nothing.
    pub fn heard(&self, call: &purlis_core::hookwire::ToolCall) {
        use purlis_core::hookwire::Decision;
        let key = call.chat.to_string();
        let told = {
            let mut spans = self.spans();
            if call.decision != Decision::Deny
                && let Some(invoked) = &call.skill
                && let Some(started_by) = StartedBy::parse(&invoked.started_by)
                && !invoked.skill.trim().is_empty()
            {
                let skill = purlis_core::shown::readable(
                    invoked.skill.trim(),
                    purlis_core::shown::DISPLAY_LIMIT,
                );
                spans.open(
                    &key,
                    Invoked {
                        skill: &skill,
                        id: &invoked.run,
                        started_by,
                        at_ms: 0,
                        tokens: None,
                    },
                );
            }
            if !spans.any(&key) {
                return;
            }
            spans.heard(
                &key,
                Heard {
                    call: Some(call.call.as_deref()),
                    ..Heard::default()
                },
            );
            open_of(&spans, &key)
        };
        self.tell(call.chat, told);
    }

    /// A state hook of chat `report.chat`: the turn's end, a new prompt and the session's end
    /// close every run open in it. A sub-agent's stop closes nothing.
    pub fn reported(&self, report: &purlis_core::hookwire::Report) {
        use purlis_core::state::Event;
        if report.agent.is_some() {
            return;
        }
        let ended = match report.event {
            Event::Stop => Ended::Turn,
            Event::UserPromptSubmit => Ended::Interrupted,
            Event::SessionEnd => Ended::Session,
            _ => return,
        };
        let closed = self.spans().close(&report.chat.to_string(), ended, 0);
        if !closed.is_empty() {
            self.tell(report.chat, None);
        }
    }

    /// Chat `chat` is closed: a run open in it goes with it, and the window is told.
    pub fn closed(&self, chat: u32) {
        let closed = self.spans().close(&chat.to_string(), Ended::Session, 0);
        if !closed.is_empty() {
            self.tell(chat, None);
        }
    }

    /// The open run of every chat that has one, for a window that has just opened.
    pub fn now(&self, chats: impl IntoIterator<Item = u32>) -> Vec<ChatSkillRun> {
        let spans = self.spans();
        chats
            .into_iter()
            .filter_map(|chat| {
                open_of(&spans, &chat.to_string()).map(|run| ChatSkillRun {
                    plane: self.plane.clone(),
                    session: chat,
                    run: Some(run),
                })
            })
            .collect()
    }

    fn spans(&self) -> MutexGuard<'_, Spans> {
        self.spans.lock().unwrap_or_else(PoisonError::into_inner)
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

/// The innermost run open in chat `key`, as the chip shows it.
fn open_of(spans: &Spans, key: &str) -> Option<OpenRun> {
    spans.inside(key).map(|open| OpenRun {
        skill: open.skill.clone(),
        id: open.id.clone(),
        tool_calls: open.signals.tool_calls,
    })
}

#[cfg(test)]
mod tests;

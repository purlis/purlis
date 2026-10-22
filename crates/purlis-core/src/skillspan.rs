// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **How the hook lines move a chat's skill runs** (SK-3, #2766): the one reading of them, which
//! the event log's writer ([`crate::eventlog::Recorder`]) records runs from and the window draws
//! a chat's open run from, so the two can never disagree.
//!
//! A tool call whose hook found a skill starting ([`crate::hookwire::ToolCall::skill`]) opens a
//! run of it, once per turn, where the claim checks; every tool call is counted onto the runs
//! open; a question put to the person is counted as an ask; and the turn's end, a new prompt or
//! the session's end closes them ([`purlis_skills::runs::Spans`]). A sub-agent's own stop closes
//! nothing. Runs are kept by a key the caller chooses: the event log's writer keys them by the
//! chat's run, the window by the chat's number.
//!
//! **A skill Use skill… typed** (SK-12, #2775) is held for its chat ([`Tracker::typed`]) and its
//! run opens as the chat's next prompt does, started by [`StartedBy::UseSkill`]: what the person
//! sends is the prompt the skill was typed into. The window types it without sending it, so a
//! prompt is the only moment it can start; a typed skill no prompt follows goes with its chat.

use std::collections::HashMap;

use purlis_skills::runs::{Call, Closed, Ended, Heard, Open, Spans, Start, StartedBy, Whose};

/// The hook words of a call that failed: `posttoolusefailure` and its kin.
const FAILED: &str = "posttoolusefailure";

/// Every skill run open, by the caller's key for the chat it is in.
#[derive(Debug, Default)]
pub struct Tracker {
    spans: Spans,
    /// The skill Use skill… typed into each chat, by the same key, until its prompt is sent.
    typed: HashMap<String, Typed>,
}

/// A skill Use skill… typed into a chat, unsent: the run its prompt opens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Typed {
    /// The skill, by the name the chat's harness knows it by.
    pub skill: String,
    /// The run's id: a ULID the window minted as it typed it, so the event log and the window
    /// name one run.
    pub run: String,
}

/// What a state hook did to a chat's skill runs ([`Tracker::report`]).
#[derive(Debug, Default)]
pub struct Reported {
    /// Every run it closed, innermost first.
    pub closed: Vec<Closed>,
    /// The run a prompt opened, for the skill Use skill… typed into it.
    pub opened: Option<Open>,
}

impl Tracker {
    /// **Tool call `call` was heard** in the chat keyed `key`, at `at_ms`. A call that starts a
    /// skill, and that no guard refused, opens its run where the claim checks and the turn has
    /// not started that skill already, and answers it; `tokens` is asked only then. The call is
    /// then counted onto every run open, the one it opened included.
    pub fn call(
        &mut self,
        key: &str,
        call: &crate::hookwire::ToolCall,
        at_ms: u64,
        tokens: impl FnOnce() -> Option<u64>,
    ) -> Option<Open> {
        use crate::hookwire::Decision;
        let opened = match &call.skill {
            Some(started) if call.decision != Decision::Deny => {
                self.open(key, started, at_ms, tokens)
            }
            _ => None,
        };
        self.spans.heard(
            key,
            Heard {
                call: Some(call.call.as_deref().map_or(Call::Unnamed, Call::Id)),
                failed: call.tool_hook.starts_with(FAILED),
                refused: call.decision == Decision::Deny,
                asked: call.decision == Decision::Ask,
            },
        );
        opened
    }

    /// **Use skill… typed `typed` into the chat keyed `key`**, unsent: its run opens at the chat's
    /// next prompt ([`Self::report`]). A second typing before that prompt replaces the first,
    /// as its text did; one with no name, or an id that is no ULID, holds nothing.
    pub fn typed(&mut self, key: &str, typed: Typed) {
        let named = typed.skill.trim();
        if named.is_empty() || ulid::Ulid::from_string(&typed.run).is_err() {
            return;
        }
        let skill = crate::shown::readable(named, crate::shown::DISPLAY_LIMIT);
        self.typed.insert(
            key.to_owned(),
            Typed {
                skill,
                run: typed.run,
            },
        );
    }

    /// **State hook `report` was heard** in the chat keyed `key`, at `at_ms`: the turn's end, a
    /// new prompt and the session's end close every run open there, innermost first, and are
    /// answered; a question the chat put to the person is counted. A new prompt then opens the
    /// run of the skill Use skill… typed into it, where one was, with `tokens` asked only then;
    /// the session's end drops it. A sub-agent's report moves nothing.
    pub fn report(
        &mut self,
        key: &str,
        report: &crate::hookwire::Report,
        at_ms: u64,
        tokens: impl FnOnce() -> Option<u64>,
    ) -> Reported {
        use crate::harness::model::Said;
        use crate::state::Event;
        if report.agent.is_some() {
            return Reported::default();
        }
        let ended = match report.event {
            Event::Stop => Ended::Turn,
            Event::UserPromptSubmit => Ended::Interrupted,
            Event::SessionEnd => {
                self.typed.remove(key);
                Ended::Session
            }
            _ => {
                if matches!(report.event.said(report.detail), Said::Ask(_)) {
                    self.spans.heard(
                        key,
                        Heard {
                            asked: true,
                            ..Heard::default()
                        },
                    );
                }
                return Reported::default();
            }
        };
        let closed = self.spans.close(key, ended, at_ms);
        let opened = match report.event {
            Event::UserPromptSubmit => self.typed.remove(key).and_then(|typed| {
                self.spans
                    .open(
                        key,
                        Start {
                            skill: &typed.skill,
                            id: &typed.run,
                            started_by: StartedBy::UseSkill,
                            at_ms,
                            tokens: tokens(),
                        },
                    )
                    .cloned()
            }),
            _ => None,
        };
        Reported { closed, opened }
    }

    /// Closes every run open in the chat keyed `key`, as `ended` at `at_ms`: its run or its
    /// window went away under them, and a skill typed into it with them.
    pub fn close(&mut self, key: &str, ended: Ended, at_ms: u64) -> Vec<Closed> {
        self.typed.remove(key);
        self.spans.close(key, ended, at_ms)
    }

    /// The innermost run open in the chat keyed `key`: the skill an entry there is inside.
    pub fn inside(&self, key: &str) -> Option<&Open> {
        self.spans.inside(key)
    }

    /// Opens the run `started` claims, where it checks: a start this purlis knows, a ULID for
    /// its id and a name, contained as every value a chat sends is.
    fn open(
        &mut self,
        key: &str,
        started: &crate::hookwire::SkillStarted,
        at_ms: u64,
        tokens: impl FnOnce() -> Option<u64>,
    ) -> Option<Open> {
        let skill = crate::shown::readable(started.skill.trim(), crate::shown::DISPLAY_LIMIT);
        if skill.is_empty()
            || started.started_by == StartedBy::Unknown
            || ulid::Ulid::from_string(&started.run).is_err()
            || self.spans.holds(key, &skill)
        {
            return None;
        }
        // The workspace is a claim too: one plain, readable name, or the copy is not known.
        let workspace = started.workspace.as_deref().map(str::trim);
        let whose = match workspace {
            Some(ws)
                if ws.is_empty()
                    || ws.contains(['/', '\\'])
                    || crate::shown::readable(ws, crate::shown::DISPLAY_LIMIT) != ws =>
            {
                Whose::default()
            }
            _ => Whose {
                workspace: workspace.map(str::to_owned),
                level: started.level,
            },
        };
        self.spans
            .open(
                key,
                Start {
                    skill: &skill,
                    id: &started.run,
                    started_by: started.started_by,
                    whose: &whose,
                    at_ms,
                    tokens: tokens(),
                },
            )
            .cloned()
    }
}

#[cfg(test)]
mod tests;

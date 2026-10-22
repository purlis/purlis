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

use purlis_skills::runs::{Call, Closed, Ended, Heard, Open, Spans, Start, StartedBy};

/// The hook words of a call that failed: `posttoolusefailure` and its kin.
const FAILED: &str = "posttoolusefailure";

/// Every skill run open, by the caller's key for the chat it is in.
#[derive(Debug, Default)]
pub struct Tracker {
    spans: Spans,
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

    /// **State hook `report` was heard** in the chat keyed `key`, at `at_ms`: the turn's end, a
    /// new prompt and the session's end close every run open there, innermost first, and are
    /// answered; a question the chat put to the person is counted. A sub-agent's report moves
    /// nothing.
    pub fn report(
        &mut self,
        key: &str,
        report: &crate::hookwire::Report,
        at_ms: u64,
    ) -> Vec<Closed> {
        use crate::harness::model::Said;
        use crate::state::Event;
        if report.agent.is_some() {
            return Vec::new();
        }
        let ended = match report.event {
            Event::Stop => Ended::Turn,
            Event::UserPromptSubmit => Ended::Interrupted,
            Event::SessionEnd => Ended::Session,
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
                return Vec::new();
            }
        };
        self.spans.close(key, ended, at_ms)
    }

    /// Closes every run open in the chat keyed `key`, as `ended` at `at_ms`: its run or its
    /// window went away under them.
    pub fn close(&mut self, key: &str, ended: Ended, at_ms: u64) -> Vec<Closed> {
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
        self.spans
            .open(
                key,
                Start {
                    skill: &skill,
                    id: &started.run,
                    started_by: started.started_by,
                    at_ms,
                    tokens: tokens(),
                },
            )
            .cloned()
    }
}

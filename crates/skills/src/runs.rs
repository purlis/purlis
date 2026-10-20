// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Skill runs** (SK-3, #2766; spec #2762): every use of a skill is a span, from the moment the
//! skill is invoked to the end of the turn.
//!
//! This module is the span's bookkeeping, and knows no harness and no log. A host tells it a
//! skill was invoked in a chat's run ([`Spans::open`]), what happened in that run while a span
//! is open ([`Spans::heard`]), and that the turn ended ([`Spans::close`]); it answers which skill
//! an entry is inside ([`Spans::inside`]) and, at the close, each span with its [`Signals`]. A
//! span opened while another is open is its child. A run shows signals only, never a verdict:
//! pass or fail comes from grading alone.
//!
//! How a harness shows an invocation is its adapter's to read; [`invoked_by_a_read`] is the one
//! reading this crate owns, for a harness briefed on its skills (the [`crate::Route::Briefing`]
//! route), whose chat invokes a skill by reading its `SKILL.md`.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::Skill;

/// What started a skill run (decision 60). Its [`StartedBy::word`] is what a run records.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum StartedBy {
    /// The harness's own Skill tool: Claude Code's, and opencode's `skill` tool.
    SkillTool,
    /// The chat read a known `SKILL.md`: how a briefed harness (Codex) invokes a skill.
    Read,
}

impl StartedBy {
    /// The word a run records: `skill-tool` or `read`.
    pub fn word(self) -> &'static str {
        match self {
            Self::SkillTool => "skill-tool",
            Self::Read => "read",
        }
    }

    /// The start `word` names, or none.
    pub fn parse(word: &str) -> Option<Self> {
        Some(match word {
            "skill-tool" => Self::SkillTool,
            "read" => Self::Read,
            _ => return None,
        })
    }
}

/// How a span came to its end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ended {
    /// The turn it was invoked in ended.
    Turn,
    /// A new prompt came before the turn said it ended: the person interrupted it.
    Interrupted,
    /// The chat's session or run ended under it.
    Session,
}

impl Ended {
    /// The word a run records: `turn`, `interrupted` or `session`.
    pub fn word(self) -> &'static str {
        match self {
            Self::Turn => "turn",
            Self::Interrupted => "interrupted",
            Self::Session => "session",
        }
    }
}

/// What a span counted while it was open: signals, never a pass or a fail (decision 24).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Signals {
    /// Tool calls made inside it, each counted once whichever of its hooks were heard.
    pub tool_calls: u32,
    /// Tool calls that failed.
    pub tool_errors: u32,
    /// Tool calls a guard refused.
    pub refusals: u32,
    /// Times the chat asked the person: a tool call put to them, or a question it raised.
    pub asks: u32,
}

/// One thing heard inside a chat's run while a span may be open.
#[derive(Debug, Clone, Copy, Default)]
pub struct Heard<'a> {
    /// A tool call, by the harness's id for it where it gave one.
    pub call: Option<Option<&'a str>>,
    /// The call failed.
    pub failed: bool,
    /// A guard refused the call.
    pub refused: bool,
    /// The person was asked.
    pub asked: bool,
}

/// One open span.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Open {
    /// The skill, by the name it was invoked by.
    pub skill: String,
    /// The run's id.
    pub id: String,
    /// The span it opened inside, where it is a child.
    pub parent: Option<String>,
    pub started_by: StartedBy,
    /// When it opened, in milliseconds since 1970.
    pub at_ms: u64,
    /// The chat's tokens as its harness last said them when it opened, where it said any.
    pub tokens_at_start: Option<u64>,
    pub signals: Signals,
    /// The tool calls counted, by id, so a call's two hooks count once.
    calls: HashSet<String>,
}

/// One span, closed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Closed {
    pub open: Open,
    pub ended: Ended,
    /// How long it was open, in milliseconds.
    pub ms: u64,
}

/// Every open span, by the chat run it is in.
#[derive(Debug, Default)]
pub struct Spans {
    open: HashMap<String, Vec<Open>>,
}

/// What [`Spans::open`] is told of a skill invoked.
#[derive(Debug, Clone, Copy)]
pub struct Invoked<'a> {
    pub skill: &'a str,
    pub id: &'a str,
    pub started_by: StartedBy,
    pub at_ms: u64,
    pub tokens: Option<u64>,
}

impl Spans {
    /// **Opens a span for `invoked` in chat run `run`**, the child of the innermost span open
    /// there, and answers it. `None` where a span of that skill is already open in the run: one
    /// turn invokes a skill once, however many times it is read.
    pub fn open(&mut self, run: &str, invoked: Invoked<'_>) -> Option<&Open> {
        let stack = self.open.entry(run.to_owned()).or_default();
        if stack.iter().any(|open| open.skill == invoked.skill) {
            return None;
        }
        let parent = stack.last().map(|open| open.id.clone());
        stack.push(Open {
            skill: invoked.skill.to_owned(),
            id: invoked.id.to_owned(),
            parent,
            started_by: invoked.started_by,
            at_ms: invoked.at_ms,
            tokens_at_start: invoked.tokens,
            signals: Signals::default(),
            calls: HashSet::new(),
        });
        stack.last()
    }

    /// The innermost span open in chat run `run`: the skill an entry there is inside.
    pub fn inside(&self, run: &str) -> Option<&Open> {
        self.open.get(run).and_then(|stack| stack.last())
    }

    /// Whether any span is open in chat run `run`.
    pub fn any(&self, run: &str) -> bool {
        self.inside(run).is_some()
    }

    /// Counts `heard` onto every span open in chat run `run`: a child's call is its parent's too.
    pub fn heard(&mut self, run: &str, heard: Heard<'_>) {
        let Some(stack) = self.open.get_mut(run) else {
            return;
        };
        for open in stack {
            if let Some(call) = heard.call {
                let first = call.is_none_or(|id| open.calls.insert(id.to_owned()));
                if first {
                    open.signals.tool_calls += 1;
                }
            }
            open.signals.tool_errors += u32::from(heard.failed);
            open.signals.refusals += u32::from(heard.refused);
            open.signals.asks += u32::from(heard.asked);
        }
    }

    /// **Closes every span open in chat run `run`**, innermost first, as `ended` at `at_ms`.
    pub fn close(&mut self, run: &str, ended: Ended, at_ms: u64) -> Vec<Closed> {
        let mut stack = self.open.remove(run).unwrap_or_default();
        stack.reverse();
        stack
            .into_iter()
            .map(|open| Closed {
                ms: at_ms.saturating_sub(open.at_ms),
                open,
                ended,
            })
            .collect()
    }
}

/// **The skill a shell command or a file read invokes by reading its `SKILL.md`**, of `skills`,
/// the chat's set: the first whose `SKILL.md` `read` names, by its whole path or by its path
/// from `cwd`. What a briefed harness's chat does to use a skill (decision 9).
pub fn invoked_by_a_read<'a>(read: &str, cwd: &Path, skills: &'a [Skill]) -> Option<&'a Skill> {
    skills.iter().find(|skill| {
        let whole = skill.path.to_string_lossy();
        named(read, &whole)
            || skill
                .path
                .strip_prefix(cwd)
                .ok()
                .is_some_and(|from_cwd| named(read, &from_cwd.to_string_lossy()))
    })
}

/// Whether `text` names `path` as a word of its own, not as part of a longer path: between the
/// edges of a shell word, or after `./`.
fn named(text: &str, path: &str) -> bool {
    let edge = |c: Option<char>| {
        c.is_none_or(|c| {
            c.is_whitespace()
                || matches!(
                    c,
                    '"' | '\'' | '`' | '=' | ';' | '|' | '&' | '(' | ')' | '<' | '>' | ','
                )
        })
    };
    !path.is_empty()
        && text.match_indices(path).any(|(at, _)| {
            let before = &text[..at];
            (edge(before.chars().next_back())
                || before
                    .strip_suffix("./")
                    .is_some_and(|rest| edge(rest.chars().next_back())))
                && edge(text[at + path.len()..].chars().next())
        })
}

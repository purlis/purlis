// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat's finish rows are a query of the event log** (R-241, R-178, R-179; Q131, Q164): each
//! turn of the chat's that ended, and each run of its own that ended failed, with the blocks of
//! that turn still unresolved at its end. Updates › Mine draws one row for each.
//!
//! **A turn** is what the chat's own run heard between a prompt (`hook.userpromptsubmit`) and
//! its end (`hook.stop`), or the run's end. Its sub-agents' stops end no turn, and their blocks
//! are the chat's. Both hook words are the harness-agnostic ones the recorder writes
//! ([`crate::state::Event::word`]), so this reads every harness that reports them alike.
//!
//! **Unresolved, decided at the turn's end** (R-178, B2): a block that is proof, as the block
//! loop takes one (`crate::looping::counts`: a guard's, the sandbox's, a policy's or a
//! project's, reported plainly), with **no later call on the same target that came back** (an
//! event of the same tool and the same keyed digest of its arguments, under any of the
//! recorder's post-tool words, `hook.posttooluse*`; a failed retry, `hook.posttoolusefailure*`,
//! is none) **and, for a sandbox block, no Allow of a sandbox ask** after it in the turn (Q263).
//! An Allow of another kind of ask resolves nothing. Counted **one a target** (R-179): two
//! blocks of one call are one. A block of an earlier turn is never this turn's.
//!
//! **Accepted limits:** a block that names no arguments (a held connection no call was heard
//! for) is resolved only by a sandbox ask's Allow; a tool no post-tool hook names (Grep, say) is
//! never heard to come back, so a block of it stays unresolved unless allowed. Both err toward
//! saying a block stopped the turn. **A turn that never ends** (no `hook.stop`, or a
//! `hook.stopfailure`) gets no finish, and its blocks are seen in the Chat log only (Q263).
//! Nothing is kept: the finishes are read from the log each time.

use std::io;
use std::path::Path;

use super::stops::{self, Answer, AskAnswered, AskGroup, Block, Source, TARGET_CHARS, Target};
use super::{Event, Phase};

/// The kind a turn's prompt is recorded under.
const PROMPT: &str = "hook.userpromptsubmit";
/// The kind a turn's end is recorded under.
const TURN_ENDED: &str = "hook.stop";
/// The start of a failed call's post-tool words, which are no call that came back.
const FAILED: &str = "posttoolusefailure";

/// **Whether an event of `kind` is a call that came back**: one of the recorder's post-tool
/// kinds, `hook.` and a post-tool word it answers ([`super::phase`]), and not a failed call's.
fn came_back(kind: &str) -> bool {
    kind.strip_prefix("hook.")
        .is_some_and(|word| super::phase(word) == Some(Phase::Post) && !word.starts_with(FAILED))
}

/// How a finish ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum How {
    /// The chat's turn ended (`hook.stop` of its own run).
    Finished,
    /// The chat's own run ended `failed`.
    Failed,
}

/// **One finish of a chat's**: the event that ended it, and its turn's unresolved blocks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finish {
    /// The ending event's number in its device's log.
    pub seq: u64,
    /// The ending event's id: what its row is noted by.
    pub ulid: String,
    /// When it was written, in milliseconds since 1970, where its id says.
    pub at_ms: Option<u64>,
    pub how: How,
    /// Its turn's blocks still unresolved at its end, one a target.
    pub blocked: u32,
}

/// What a block was aimed at: its tool, its arguments' keyed digest, and its target where it
/// names no digest; and who blocked it, since only a sandbox block is resolved by an Allow.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Aim {
    source: Source,
    tool: String,
    args: Option<String>,
    target: Option<Target>,
}

/// **The blocks of the turn so far**, each with its aim, as they stand: a later success on its
/// aim, or, for a sandbox block, a sandbox ask's Allow, takes it out.
#[derive(Debug, Default)]
struct Turn {
    unresolved: Vec<Aim>,
}

impl Turn {
    fn blocked(&mut self, block: Block) {
        let target = block.target.filter(|_| block.args.is_none());
        let aim = Aim {
            source: block.source,
            tool: block.tool,
            args: block.args,
            target,
        };
        if !self.unresolved.contains(&aim) {
            self.unresolved.push(aim);
        }
    }

    fn came_back(&mut self, tool: &str, args: &str) {
        self.unresolved
            .retain(|aim| !(aim.tool == tool && aim.args.as_deref() == Some(args)));
    }

    /// A sandbox ask was allowed: every sandbox block of the turn is resolved, and no other.
    fn sandbox_allowed(&mut self) {
        self.unresolved.retain(|aim| aim.source != Source::Sandbox);
    }

    fn count(&self) -> u32 {
        u32::try_from(self.unresolved.len()).unwrap_or(u32::MAX)
    }
}

/// **The finishes of chat `chat` among `events`**, oldest first, each with its turn's
/// unresolved blocks ([`Finish::blocked`]); a failed run counts none, since its row is a
/// failure already.
pub fn finishes(events: &[Event], chat: &str) -> Vec<Finish> {
    let mut out = Vec::new();
    let mut turn = Turn::default();
    for event in events
        .iter()
        .filter(|event| event.chat.as_deref() == Some(chat))
    {
        let own = event.parent_run.is_none();
        let ended = |how, blocked| Finish {
            seq: event.seq,
            ulid: event.ulid.clone(),
            at_ms: super::written_at_ms(&event.ulid),
            how,
            blocked,
        };
        match event.kind.as_str() {
            PROMPT if own => turn = Turn::default(),
            TURN_ENDED if own => {
                out.push(ended(How::Finished, turn.count()));
                turn = Turn::default();
            }
            "run.ended" if own => {
                if event.body.get("state").and_then(serde_json::Value::as_str) == Some("failed") {
                    out.push(ended(How::Failed, 0));
                }
                turn = Turn::default();
            }
            stops::BLOCK => {
                if let Ok(block) = serde_json::from_value::<Block>(event.body.clone())
                    // The loop rule's own test (Q235, Q237): a block the sidebar would not
                    // count never stops a turn either.
                    && crate::looping::counts(&block)
                {
                    turn.blocked(block);
                }
            }
            stops::ASK_ANSWERED => {
                if let Ok(AskAnswered {
                    ask: AskGroup::Sandbox,
                    answer: Answer::Allowed,
                }) = serde_json::from_value(event.body.clone())
                {
                    turn.sandbox_allowed();
                }
            }
            kind if came_back(kind) => {
                let word = |field: &str| event.body.get(field).and_then(serde_json::Value::as_str);
                if let (Some(tool), Some(args)) = (word("tool"), word("args")) {
                    // As a block writes its tool: one short line (`stops::line_tool`).
                    turn.came_back(&crate::shown::readable(tool, TARGET_CHARS), args);
                }
            }
            _ => {}
        }
    }
    out
}

/// [`finishes`] of the log in `dir`, read through [`super::read`].
pub fn finishes_in(dir: &Path, chat: &str) -> io::Result<Vec<Finish>> {
    Ok(finishes(&super::read(dir)?, chat))
}

#[cfg(test)]
#[path = "finishes/tests.rs"]
mod tests;

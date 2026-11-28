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
//! **Unresolved, decided at the turn's end** (R-178, B2): every block the Blocked marker counts
//! (Q280: any source, the harness's own check's included, and any qualifier, a Can't-tell
//! report's included, each with its qualifier and the chat's harness), so the finish row and
//! the marker never disagree, with **no later call on the same target that came back** (an
//! event of the same tool and the same keyed digest of its arguments, under any of the
//! recorder's post-tool words, `hook.posttooluse*`; a failed retry, `hook.posttoolusefailure*`,
//! is none) **and no Allow after it in the turn that resolves it**: a sandbox ask's, for a
//! sandbox block, and no other (Q263; `crate::looping::allow_resolves`, the block loop's own
//! rule). Counted **one a target** (R-179): two blocks of one call are one. A block of an
//! earlier turn is never this turn's. A `stop.block` that does not read back names nothing to
//! offer a way out of, and is left to the Chat log.
//!
//! **Accepted limits:** a block that names no arguments (a held connection no call was heard
//! for) is resolved only by a sandbox ask's Allow; a tool no post-tool hook names (Grep, say) is
//! never heard to come back, so a block of it stays unresolved unless allowed. Both err toward
//! saying a block stopped the turn. **A turn that never ends** (no `hook.stop`, or a
//! `hook.stopfailure`) gets no finish, and its blocks are seen in the Chat log only (Q263).
//! Nothing is kept on disk: the finishes are a fold of the log's events, which [`Recent`] keeps
//! read in memory, as compact records of each chat's, while the log's files do not change.

use std::io;
use std::path::Path;

mod recent;
pub use recent::Recent;

use super::stops::{
    self, Answer, AskAnswered, AskGroup, Block, Qualifier, Source, TARGET_CHARS, Target,
};
use super::{Event, Phase};

/// The kind a turn's prompt is recorded under.
const PROMPT: &str = "hook.userpromptsubmit";
/// The kind a turn's end is recorded under.
const TURN_ENDED: &str = "hook.stop";
/// The start of a failed call's post-tool words, which are no call that came back.
const FAILED: &str = "posttoolusefailure";

/// **Whether an event of `kind` is a call that came back**: one of the recorder's post-tool
/// kinds, `hook.` and a post-tool word it answers ([`super::phase`]), and neither a failed
/// call's nor one of the post-tool words that say nothing of a call
/// ([`crate::hookreg::NO_OPS`]), so it reads a call coming back as the block loop does (Q263).
pub(super) fn came_back(kind: &str) -> bool {
    kind.strip_prefix("hook.").is_some_and(|word| {
        super::phase(word) == Some(Phase::Post)
            && !word.starts_with(FAILED)
            && !crate::hookreg::NO_OPS.contains(&word)
    })
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
    /// **Those blocks**, oldest first, each by who blocked it and what at (T1-33, R-300): what
    /// its finish row offers ways out of. As many as [`Self::blocked`] counts.
    pub unresolved: Vec<Unresolved>,
}

/// **An unresolved block of a turn** (T1-33, R-300): who blocked it, how much its report is
/// worth, what at where it says, and the chat's harness (Q272): what its finish row's ways out
/// are read from, as its Chat log row's are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unresolved {
    pub source: Source,
    /// Its report's qualifier, as the block wrote it: the row reads it as the Chat log does, so
    /// a Can't-tell block offers Go to chat (Q272).
    pub qualifier: Option<Qualifier>,
    pub target: Option<Target>,
    /// The chat's harness when it was blocked, by the word its latest `run.started` gave, or
    /// `unknown`: whose own check a `harness_check` block was.
    pub harness: String,
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

/// What a finish names of a block beside its aim: its first target, its qualifier and the
/// chat's harness when it was blocked.
#[derive(Debug, Clone)]
struct Shown {
    target: Option<Target>,
    qualifier: Option<Qualifier>,
    harness: String,
}

/// **The blocks of the turn so far**, each with its aim, as they stand: a later success on its
/// aim, or, for a sandbox block, a sandbox ask's Allow, takes it out. Each keeps the first
/// target its aim was blocked at, which its finish row names (T1-33).
#[derive(Debug, Default)]
struct Turn {
    unresolved: Vec<(Aim, Shown)>,
}

impl Turn {
    fn blocked(&mut self, block: Block, harness: &str) {
        let shown = Shown {
            target: block.target.clone(),
            qualifier: block.qualifier,
            harness: harness.to_owned(),
        };
        let target = block.target.filter(|_| block.args.is_none());
        let aim = Aim {
            source: block.source,
            tool: block.tool,
            args: block.args,
            target,
        };
        if !self.unresolved.iter().any(|(held, _)| *held == aim) {
            self.unresolved.push((aim, shown));
        }
    }

    fn came_back(&mut self, tool: &str, args: &str) {
        self.unresolved
            .retain(|(aim, _)| !(aim.tool == tool && aim.args.as_deref() == Some(args)));
    }

    /// An ask of group `ask` was allowed: each block of the turn that Allow resolves is
    /// resolved, by the block loop's own rule (`crate::looping::allow_resolves`), and no other.
    fn allowed(&mut self, ask: AskGroup) {
        self.unresolved
            .retain(|(aim, _)| !crate::looping::allow_resolves(ask, aim.source));
    }

    fn count(&self) -> u32 {
        u32::try_from(self.unresolved.len()).unwrap_or(u32::MAX)
    }

    /// The unresolved blocks, as a finish names them.
    fn named(&self) -> Vec<Unresolved> {
        self.unresolved
            .iter()
            .map(|(aim, shown)| Unresolved {
                source: aim.source,
                qualifier: shown.qualifier,
                target: shown.target.clone(),
                harness: shown.harness.clone(),
            })
            .collect()
    }
}

/// **What one event of a chat's says to the fold**, read off its body once: the event's kind as
/// [`fold`] tells it apart, and only what [`fold`] and [`needs_older`] read of it. An event that
/// says nothing to either is none ([`Record::of`]). What [`Recent`] keeps of a file in place of its
/// events, so no body is kept (FS-C2, #7902).
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Record {
    /// The event's number in its device's log: what a repeat of a sealed event is told by.
    seq: u64,
    step: Step,
}

#[derive(Debug, Clone, PartialEq)]
enum Step {
    /// `run.started`, of the chat's own run or a sub-agent's: the harness its body names, or
    /// `unknown`.
    Started(String),
    /// The chat's own prompt (`hook.userpromptsubmit`).
    Prompt,
    /// The chat's own turn ended (`hook.stop`).
    TurnEnded(Ending),
    /// The chat's own run ended: how, where it ended failed.
    RunEnded(Option<Ending>),
    /// A block, where it reads back.
    Blocked(Option<Box<Block>>),
    /// An ask answered: its group, where it was allowed.
    Answered(Option<AskGroup>),
    /// A call that came back: its tool, as a block writes it, and its arguments, where it names
    /// both.
    CameBack(Option<(String, String)>),
}

/// The event that ended a finish, as the finish names it.
#[derive(Debug, Clone, PartialEq)]
struct Ending {
    ulid: String,
    at_ms: Option<u64>,
}

impl Record {
    /// What `event` says to the fold, told apart in the order [`fold`] always read an event in.
    pub(super) fn of(event: &Event) -> Option<Self> {
        use serde::Deserialize as _;
        let own = event.parent_run.is_none();
        let ending = || Ending {
            ulid: event.ulid.clone(),
            at_ms: super::written_at_ms(&event.ulid),
        };
        let step = match event.kind.as_str() {
            // As the Chat log reads it (`chatlog::rows`): the latest start's word.
            "run.started" => Step::Started(
                event
                    .body
                    .get("harness")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or(super::UNKNOWN)
                    .to_owned(),
            ),
            PROMPT if own => Step::Prompt,
            TURN_ENDED if own => Step::TurnEnded(ending()),
            "run.ended" if own => Step::RunEnded(
                (event.body.get("state").and_then(serde_json::Value::as_str) == Some("failed"))
                    .then(ending),
            ),
            // Every block the marker's tally reads back (`chatlog::Tally`), whoever blocked it
            // and however much its report is worth (Q280).
            stops::BLOCK => Step::Blocked(Block::deserialize(&event.body).ok().map(Box::new)),
            stops::ASK_ANSWERED => Step::Answered(match AskAnswered::deserialize(&event.body) {
                Ok(AskAnswered {
                    ask,
                    answer: Answer::Allowed,
                    ..
                }) => Some(ask),
                _ => None,
            }),
            kind if came_back(kind) => {
                let word = |field: &str| event.body.get(field).and_then(serde_json::Value::as_str);
                Step::CameBack(match (word("tool"), word("args")) {
                    // As a block writes its tool: one short line (`stops::line_tool`).
                    (Some(tool), Some(args)) => {
                        Some((crate::shown::readable(tool, TARGET_CHARS), args.to_owned()))
                    }
                    _ => None,
                })
            }
            _ => return None,
        };
        Some(Self {
            seq: event.seq,
            step,
        })
    }

    /// About how many bytes it holds, its own and what it points to: what [`Recent`]'s cap
    /// counts.
    pub(super) fn bytes(&self) -> usize {
        let held = match &self.step {
            Step::Started(harness) => harness.len(),
            Step::Prompt | Step::Answered(_) | Step::RunEnded(None) => 0,
            Step::TurnEnded(ending) | Step::RunEnded(Some(ending)) => ending.ulid.len(),
            Step::Blocked(None) | Step::CameBack(None) => 0,
            Step::Blocked(Some(block)) => {
                std::mem::size_of::<Block>()
                    + block.tool.len()
                    + block.args.as_ref().map_or(0, String::len)
                    + block.target.as_ref().map_or(0, |_| TARGET_CHARS)
            }
            Step::CameBack(Some((tool, args))) => tool.len() + args.len(),
        };
        std::mem::size_of::<Self>() + held
    }

    pub(super) fn seq(&self) -> u64 {
        self.seq
    }
}

/// **The finishes of chat `chat` among `events`**, oldest first, each with its turn's
/// unresolved blocks ([`Finish::blocked`]); a failed run counts none, since its row is a
/// failure already.
pub fn finishes(events: &[Event], chat: &str) -> Vec<Finish> {
    fold(
        events
            .iter()
            .filter(|event| event.chat.as_deref() == Some(chat))
            .filter_map(Record::of)
            .collect::<Vec<_>>()
            .iter(),
    )
}

/// **The finishes one chat's records say**, in the order they come, which may be several files'
/// one after the other.
fn fold<'a>(records: impl Iterator<Item = &'a Record>) -> Vec<Finish> {
    let mut out = Vec::new();
    let mut turn = Turn::default();
    let mut harness = super::UNKNOWN;
    for record in records {
        let ended = |how, blocked, unresolved, ending: &Ending| Finish {
            seq: record.seq,
            ulid: ending.ulid.clone(),
            at_ms: ending.at_ms,
            how,
            blocked,
            unresolved,
        };
        match &record.step {
            Step::Started(word) => harness = word,
            Step::Prompt => turn = Turn::default(),
            Step::TurnEnded(ending) => {
                out.push(ended(How::Finished, turn.count(), turn.named(), ending));
                turn = Turn::default();
            }
            Step::RunEnded(failed) => {
                if let Some(ending) = failed {
                    out.push(ended(How::Failed, 0, Vec::new(), ending));
                }
                turn = Turn::default();
            }
            Step::Blocked(Some(block)) => turn.blocked((**block).clone(), harness),
            Step::Answered(Some(ask)) => turn.allowed(*ask),
            Step::CameBack(Some((tool, args))) => turn.came_back(tool, args),
            Step::Blocked(None) | Step::Answered(None) | Step::CameBack(None) => {}
        }
    }
    out
}

/// **Whether the turn or the run of the chat that its first `records` belong to began before
/// them**: its first record that depends on what came earlier is not preceded by what settles
/// it. A block, an Allow, a call that came back or a turn's end needs the prompt that began its
/// turn (or the run's end that cleared it); a block needs, besides, the run's start that names
/// the chat's harness. With neither missing, nothing before `records` can change what [`fold`]
/// answers of the chat.
fn needs_older<'a>(records: impl Iterator<Item = &'a Record>) -> bool {
    let (mut started, mut reset) = (false, false);
    for record in records {
        let settled = match record.step {
            Step::Started(_) => {
                started = true;
                true
            }
            Step::Prompt | Step::RunEnded(_) => {
                reset = true;
                true
            }
            Step::Blocked(_) => started && reset,
            Step::TurnEnded(_) | Step::Answered(_) | Step::CameBack(_) => reset,
        };
        if !settled {
            return true;
        }
        if started && reset {
            return false;
        }
    }
    false
}

/// [`finishes`] of the log in `dir`, read through [`super::read`]: what [`Recent`] answers, from the
/// whole log at once.
pub fn finishes_in(dir: &Path, chat: &str) -> io::Result<Vec<Finish>> {
    Ok(finishes(&super::read(dir)?, chat))
}

#[cfg(test)]
#[path = "finishes/tests.rs"]
mod tests;

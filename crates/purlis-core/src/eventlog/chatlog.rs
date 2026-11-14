// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat's timeline and the fleet counts are queries of the event log** (R-223, Q129, Q216):
//! the timeline the Chat log tab draws, and the counts of each kind of stop, per chat and across
//! chats, that the marker and the finish rows read.
//!
//! **No second write path.** Both are read from the one log through its read API
//! ([`super::read`]), and neither keeps anything of its own: an event the log gains shows in
//! both, and one it no longer holds shows in neither. Nothing is migrated: the shipped Activity
//! view ([`crate::activity`]) is still read from the dispatch records, with the chats' own words,
//! and the event log stays metadata-only.
//!
//! **A new kind of story event needs a line in [`is_story`].** Only `trust.*` is taken as a
//! family; `run.started`, `run.ended`, the four kinds of stop ([`stops`]: `stop.block`,
//! `stop.hold`, `stop.loop_pause` and `ask.answered`) and an ask being raised (`ask.raised`,
//! T1-28) are taken by name. Any other kind, a new `stop.*` or `run.*` one included, is left
//! out of the timeline until [`is_story`] names it, and a new kind of stop is counted only once
//! [`Tally`] has a bucket for it. A raised ask is no stop and is never counted.
//!
//! **What heads a Chat log is a query of its own** ([`summary`]): the tool events its allowed
//! counts are read from, and the starts and lifted sandboxes its honesty lines are, are the
//! trace, never the story, so [`timeline`] leaves the tool events out.

use std::collections::BTreeMap;
use std::io;
use std::path::Path;

use super::Event;
use super::stops::{self, AskAnswered, AskGroup, AskRaised, Block, Hold, LoopPause, Qualifier};
use crate::harness::Harness;
use crate::sandbox::protection::{InForce, SANDBOX};

/// **Whether an event of `kind` is part of a chat's story**: its runs starting and ending, its
/// stops ([`stops`]: blocks, holds, loop pauses and answers to asks), and the grants and
/// protection changes the log writes under it (`trust.*`). A tool hook's line and the log's own
/// housekeeping are not: the Chat log is the story, never the trace.
pub fn is_story(kind: &str) -> bool {
    matches!(kind, "run.started" | "run.ended" | stops::ASK_RAISED)
        || is_stop(kind)
        || kind.starts_with("trust.")
}

/// **Chat `chat`'s timeline among `events`**: the events of its story ([`is_story`]), in the
/// order of the log, oldest first.
pub fn timeline(events: &[Event], chat: &str) -> Vec<Event> {
    events
        .iter()
        .filter(|event| event.chat.as_deref() == Some(chat) && is_story(&event.kind))
        .cloned()
        .collect()
}

/// [`timeline`] of the log in `dir`, read through [`super::read`].
pub fn timeline_in(dir: &Path, chat: &str) -> io::Result<Vec<Event>> {
    Ok(timeline(&super::read(dir)?, chat))
}

/// **A row the Chat log draws** of a chat's timeline (R-227, R-228, R-229, R-230, R-231,
/// R-232, R-233). Phase 1 draws a start and an end, the protection changes, each block (T1-29),
/// the asks and their answers, the holds and the loop pauses; a kind of story event with no row
/// here (a grant, phase 3's) is drawn by none yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Row {
    /// A start of the chat's own run that resolved the protection it runs under.
    Started {
        /// When it was written, in milliseconds since 1970, where its id says.
        at_ms: Option<u64>,
        /// Why the run began, as `run.started` says it (`start`, `reopen`, `fresh`, ...).
        cause: String,
        /// The protection it recorded; `None` where the event has none, says `unknown`, or
        /// has one of a shape this version cannot read (T1-06's three shapes).
        protection: Option<InForce>,
    },
    /// The end of the chat's own run, and its result.
    Ended {
        at_ms: Option<u64>,
        /// `completed`, `failed` or `stopped`.
        state: String,
        /// Why: `exited`, `host-crash`, or the stop's word.
        cause: String,
        /// The exit code, where the program left one.
        code: Option<i64>,
    },
    /// **The chat's protection changed** (R-228), through a shipped path in phase 1: its
    /// sandbox lifted for a run (`trust.sandbox.off`) or put back on (`trust.sandbox.on`).
    ProtectionChanged {
        at_ms: Option<u64>,
        /// `off` or `on`.
        change: String,
        /// Who made it, as its trust event names them: `human`, `host` or `agent`.
        actor_kind: String,
        /// The actor's own word under that kind (`operator`, `purlis`, `a chat`, ...).
        actor: String,
    },
    /// **An ask raised** (R-229): what waited on the person, by its group.
    AskRaised { at_ms: Option<u64>, ask: AskGroup },
    /// **An ask answered** (R-229): its group, the answer and who answered.
    AskAnswered {
        at_ms: Option<u64>,
        answered: AskAnswered,
    },
    /// **A hold** (R-231): an action purlis refused to the chat and raised as an ask.
    Held {
        at_ms: Option<u64>,
        hold: Hold,
        /// It was written under a sub-agent's run (one with a `parent_run`): still the chat's
        /// row, marked as its sub-agent's (Q261).
        sub_agent: bool,
    },
    /// **A loop pause** (R-232): the rule that paused the chat, and at how many times.
    LoopPaused {
        at_ms: Option<u64>,
        pause: LoopPause,
    },
    /// **A block** (R-230, T1-29): a call refused, by whom, how much its report is worth and at
    /// what, as its `stop.block` says; a sub-agent's included, since its stops are the chat's.
    Blocked {
        at_ms: Option<u64>,
        /// The block, or `None` for a body that does not read back as one ([`Tally`]'s
        /// `unreadable_blocks`): nothing says who refused it.
        block: Option<Block>,
        /// The harness's id for the refused call, from the tool event the block was written
        /// just after (`Recorder::tool_spooled`): what the app finds a reason it holds by
        /// ([`super::reasons::Reasons`]). None where no tool event of a call came just before.
        call: Option<String>,
        /// The chat's harness, by the word its latest `run.started` gave (`claude`, `codex`,
        /// `opencode`), or `unknown`: whose own check a `harness_check` block was.
        harness: String,
    },
}

/// The call id of `event`, a tool hook's event (`hook.*`) with a `call`.
fn call_of(event: &Event) -> Option<String> {
    if !event.kind.starts_with("hook.") {
        return None;
    }
    event.body.get("call")?.as_str().map(str::to_owned)
}

/// **The rows of chat `chat`'s Chat log** among `events`, oldest first: read from its
/// [`timeline`]'s events, so the log holds nothing a row does not.
///
/// **Only the chat's own runs start and end rows** (T1-27's ruling on T1-03's review). A run
/// with a `parent_run` is a sub-agent's: its start and end are the trace of the chat's work,
/// not its story, and its stops are still the chat's rows. A `/clear`'s run (and a sub-agent's)
/// carries a copy of what the run before it started under, so it is no start of its own; a
/// relaunch (`reopen`, `fresh`) resolved its protection anew, and is. A stop written under a
/// sub-agent's run says so (Q261): in phase 1, a hold.
///
/// **A block finds its call by the event before it**: the writer writes a block just after its
/// call's tool event, under the same run and with nothing between (one line's events, written
/// together), so the chat's event before a block, under its run, is that call's.
pub fn rows(events: &[Event], chat: &str) -> Vec<Row> {
    let mut rows = Vec::new();
    let mut harness = super::UNKNOWN.to_owned();
    let mut before: Option<&Event> = None;
    for event in events
        .iter()
        .filter(|event| event.chat.as_deref() == Some(chat))
    {
        let previous = before.replace(event);
        if !is_story(&event.kind) {
            continue;
        }
        let word = |field: &str| {
            event
                .body
                .get(field)
                .and_then(serde_json::Value::as_str)
                .unwrap_or(super::UNKNOWN)
                .to_owned()
        };
        let at_ms = super::written_at_ms(&event.ulid);
        if event.kind == "run.started" {
            harness = word("harness");
        }
        let own = event.parent_run.is_none();
        let row = match event.kind.as_str() {
            "run.started" if own => {
                let cause = word("cause");
                (!matches!(cause.as_str(), "clear" | "child")).then(|| Row::Started {
                    at_ms,
                    cause,
                    protection: event
                        .body
                        .get("protection")
                        .and_then(|said| serde_json::from_value(said.clone()).ok()),
                })
            }
            "run.ended" if own => Some(Row::Ended {
                at_ms,
                state: word("state"),
                cause: word("cause"),
                code: event.body.get("code").and_then(serde_json::Value::as_i64),
            }),
            // The protection changes the shipped paths write (R-228): the sandbox lifted
            // for a run, or put back on. A grant is no change of protection.
            kind @ (SANDBOX_OFF | SANDBOX_ON) => Some(Row::ProtectionChanged {
                at_ms,
                change: (if kind == SANDBOX_OFF { "off" } else { "on" }).to_owned(),
                actor_kind: word("actor_kind"),
                actor: word("actor"),
            }),
            stops::ASK_RAISED => {
                read::<AskRaised>(event).map(|AskRaised { ask }| Row::AskRaised { at_ms, ask })
            }
            stops::ASK_ANSWERED => read(event).map(|answered| Row::AskAnswered { at_ms, answered }),
            stops::HOLD => read(event).map(|hold| Row::Held {
                at_ms,
                hold,
                sub_agent: !own,
            }),
            stops::LOOP_PAUSE => read(event).map(|pause| Row::LoopPaused { at_ms, pause }),
            stops::BLOCK => Some(Row::Blocked {
                at_ms,
                block: serde_json::from_value(event.body.clone()).ok(),
                call: previous
                    .filter(|previous| previous.run == event.run)
                    .and_then(call_of),
                harness: harness.clone(),
            }),
            _ => None,
        };
        rows.extend(row);
    }
    rows
}

/// A stop's body as its kind writes it; one that does not read back is no row.
fn read<T: serde::de::DeserializeOwned>(event: &Event) -> Option<T> {
    serde_json::from_value(event.body.clone()).ok()
}

/// The kinds a chat's sandbox being lifted for a run, and put back on, are written under
/// ([`crate::sandbox::Change::kind`]).
const SANDBOX_OFF: &str = "trust.sandbox.off";
const SANDBOX_ON: &str = "trust.sandbox.on";

/// **What heads a chat's Chat log** (R-234, R-235): its allowed calls, collapsed into one
/// count with those allowed by the person's rules, and the honesty lines over the whole chat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Summary {
    /// The calls purlis's hook allowed.
    pub allowed: u32,
    /// Of those, the calls an entry in the person's Allowed list allowed.
    pub by_your_rules: u32,
    /// What reports the chat's calls was within its own reach at some start, or nothing says
    /// how it ran, so its reports are not checked.
    pub unverified: bool,
    /// The chat ran with No sandbox at some point: as the person, with all they can reach.
    pub ran_as_you: bool,
    /// The harness its latest start names, where purlis ships it; `None` where no start names
    /// one it knows.
    pub harness: Option<Harness>,
    /// Some start's harness does not report what its own check blocks (R-097, R-100), or
    /// nothing says which harness it ran: its log can't tell of those blocks.
    pub cant_tell_its_checks_blocks: bool,
    /// Some start's harness reports no mode purlis can check (R-099, Q152), or nothing says which
    /// harness it ran: its log can't tell which mode it ran in.
    pub cant_tell_its_mode: bool,
}

/// **What heads chat `chat`'s Chat log** among `events` ([`Summary`]), from its tool events,
/// every start of it, and its sandbox lifted for a run.
///
/// - **Allowed** (R-234, Q261): each call whose tool event before it ran (a `PreToolUse`
///   hook's, [`super::phase`]) says purlis's hook allowed it, `allow`, once per call by its id
///   under its run, so its post hook, or a second pre hook, is never a second call. A call the
///   hook had no say on (`none`), refused (`deny`) or asked about (`ask`) is not counted.
/// - **By your rules**: of those, each allowed by an entry of the person's Allowed list, which
///   its `allow` names as its `rule`.
/// - **Unverified** (R-235, Q261): any start of the chat's says its hooks report from within its
///   reach, as its harness and sandbox say ([`stops::reported_from_outside_its_reach`]), the
///   rule a block's report takes; and a chat with no start in the log is unverified, since
///   verified is an allowlist (Q235) and nothing says how it ran.
/// - **Ran as you** (Q261): any start's sandbox layer was at `off`, No sandbox (where a start
///   did not record its protection, one with no sandbox backend, `none`), or its sandbox was
///   lifted for a run (`trust.sandbox.off`).
/// - **Can't tell** (R-097, R-099, R-100; T1-38): any start's harness reports no block of its
///   own check, or no mode purlis can check ([`Harness::reports_its_checks_blocks`],
///   [`Harness::reports_a_mode_purlis_can_check`]), taken over the whole chat as the honesty
///   lines are; and a chat with no start, or one whose harness purlis does not ship, can't tell
///   either. The harness named is the latest start's.
///
/// **The counts are only as good as the chat's hook lines** (Q235): a chat whose hooks run
/// within its reach can write lines that say `allow` for calls it never made, and so inflate
/// its own counts. The Unverified line says so for such a chat.
pub fn summary(events: &[Event], chat: &str) -> Summary {
    let mut summary = Summary::default();
    let mut started = false;
    let mut calls = std::collections::HashSet::new();
    for event in events
        .iter()
        .filter(|event| event.chat.as_deref() == Some(chat))
    {
        if event.kind == "run.started" {
            let attributes: super::Attributes =
                serde_json::from_value(event.body.clone()).unwrap_or_default();
            started = true;
            summary.unverified |= !stops::reported_from_outside_its_reach(&attributes);
            let harness = attributes.harness.as_deref().and_then(Harness::of_kind);
            summary.harness = harness;
            summary.cant_tell_its_checks_blocks |=
                !harness.is_some_and(Harness::reports_its_checks_blocks);
            summary.cant_tell_its_mode |=
                !harness.is_some_and(Harness::reports_a_mode_purlis_can_check);
            summary.ran_as_you |= match &attributes.protection {
                Some(in_force) => in_force
                    .layers
                    .iter()
                    .any(|layer| layer.layer == SANDBOX && layer.level == "off"),
                None => attributes.sandbox.as_deref() == Some("none"),
            };
            continue;
        }
        if event.kind == SANDBOX_OFF {
            summary.ran_as_you = true;
            continue;
        }
        let Some(hook) = event.kind.strip_prefix("hook.") else {
            continue;
        };
        if super::phase(hook) != Some(super::Phase::Pre) {
            continue;
        }
        let said = |field: &str| event.body.get(field).and_then(serde_json::Value::as_str);
        if said("decision") != Some("allow") {
            continue;
        }
        if let Some(call) = said("call")
            && !calls.insert((event.run.as_deref(), call))
        {
            continue;
        }
        summary.allowed = summary.allowed.saturating_add(1);
        if said("rule").is_some() {
            summary.by_your_rules = summary.by_your_rules.saturating_add(1);
        }
    }
    summary.unverified |= !started;
    summary.cant_tell_its_checks_blocks |= !started;
    summary.cant_tell_its_mode |= !started;
    summary
}

/// **How many of each kind of stop.** A block whose report is qualified, `unverified` or
/// `cant_tell` (Q165, Q235, Q237), is counted apart from a plain one, since such a report never
/// counts on the sidebar, and each qualifier apart from the other. A `stop.block` whose body does
/// not read back as a [`Block`] is counted in a bucket of its own, `unreadable_blocks`: nothing
/// says which kind of block it was.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Tally {
    /// Blocks with no qualifier.
    pub blocks: u32,
    /// Blocks reported `unverified`.
    pub unverified_blocks: u32,
    /// Blocks reported `cant_tell`.
    pub cant_tell_blocks: u32,
    /// Blocks whose body does not read back as a [`Block`].
    pub unreadable_blocks: u32,
    pub holds: u32,
    pub loop_pauses: u32,
    pub asks_answered: u32,
}

impl Tally {
    /// Counts `event` where it is a stop.
    fn add(&mut self, event: &Event) {
        let count = match event.kind.as_str() {
            stops::BLOCK => match serde_json::from_value::<Block>(event.body.clone()) {
                Ok(Block { qualifier, .. }) => match qualifier {
                    None => &mut self.blocks,
                    Some(Qualifier::Unverified) => &mut self.unverified_blocks,
                    Some(Qualifier::CantTell) => &mut self.cant_tell_blocks,
                },
                // A block that does not read back is not trusted to be any of those.
                Err(_) => &mut self.unreadable_blocks,
            },
            stops::HOLD => &mut self.holds,
            stops::LOOP_PAUSE => &mut self.loop_pauses,
            stops::ASK_ANSWERED => &mut self.asks_answered,
            _ => return,
        };
        *count = count.saturating_add(1);
    }
}

/// **The fleet counts**: each kind of stop across chats, and per chat by its id.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Counts {
    /// Every stop in the log, a chat's or of no chat.
    pub all: Tally,
    /// Each chat's, by its id; a chat with no stop is not listed.
    pub per_chat: BTreeMap<String, Tally>,
}

/// The fleet counts of `events`.
pub fn counts(events: &[Event]) -> Counts {
    let mut counts = Counts::default();
    for event in events {
        counts.all.add(event);
        if let Some(chat) = &event.chat
            && is_stop(&event.kind)
        {
            counts.per_chat.entry(chat.clone()).or_default().add(event);
        }
    }
    counts
}

fn is_stop(kind: &str) -> bool {
    matches!(
        kind,
        stops::BLOCK | stops::HOLD | stops::LOOP_PAUSE | stops::ASK_ANSWERED
    )
}

/// [`counts`] of the log in `dir`, read through [`super::read`].
pub fn counts_in(dir: &Path) -> io::Result<Counts> {
    Ok(counts(&super::read(dir)?))
}

/// **What a chat's blocked marker shows** (R-172, R-174, R-299): its blocks after the person's
/// last look, as the fleet counts tally them ([`Tally`]), and the latest block's `seq`, which is
/// the look the window keeps once the chat has been looked at.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Marker {
    /// The verified blocks after the look: `Tally.blocks` alone, the sidebar's count.
    pub blocks: u32,
    /// The unverified ones after it, which never count on the sidebar (R-299): a muted dot.
    pub unverified: u32,
    /// Every block after it, by its kind ([`Tally`]).
    pub tally: Tally,
    /// The `seq` of the chat's latest block, the look's point; `None` for a chat with none.
    /// A `seq`, not its ULID: the log numbers its events in order, and two ULIDs minted in one
    /// millisecond need not sort in the order they were written.
    pub latest: Option<u64>,
}

/// The most blocks of one chat [`Blocks`] holds; the oldest go first. A look is always at the
/// latest, so what goes is long looked at, or counted as "this many or more".
pub const BLOCKS_HELD: usize = 1000;

/// **The blocked markers' running tally** (R-174, T1-03's note): each chat's blocks, by their
/// `seq`s, fed event by event from the log's subscription ([`super::subscribe`]), so a marker's
/// refresh reads only what was written since, never the whole log. "After the last look" is
/// the caller's filter ([`Blocks::since`]); the look itself is kept where the shipped New
/// markers keep theirs (the Inbox's seen record, ADR 0124 decision 9.1), never here.
#[derive(Debug, Default)]
pub struct Blocks {
    per_chat: BTreeMap<String, std::collections::VecDeque<(u64, Tally)>>,
}

impl Blocks {
    /// Takes what the subscription delivered: a block is held under its chat. Lines the log
    /// lost that this held (`Ahead`, after a power loss) let go of everything: the stream
    /// carries on from where the log now ends, so the blocks still in the log before that are
    /// not delivered again, and count only once the log is read whole, at the next start.
    /// Events no longer kept (`Missed`) are only gone from the log.
    pub fn take(&mut self, delivery: super::Delivery) {
        match delivery {
            super::Delivery::Event(event) => {
                let Some(chat) = event.chat.clone() else {
                    return;
                };
                if event.kind != stops::BLOCK {
                    return;
                }
                let mut one = Tally::default();
                one.add(&event);
                let held = self.per_chat.entry(chat).or_default();
                held.push_back((event.seq, one));
                if held.len() > BLOCKS_HELD {
                    held.pop_front();
                }
            }
            super::Delivery::Ahead { .. } => self.per_chat.clear(),
            super::Delivery::Missed { .. } => {}
        }
    }

    /// **Chat `chat`'s marker**: its blocks after `after`, the `seq` of the last block looked at
    /// (`None`: never looked at, so every block it has counts).
    pub fn since(&self, chat: &str, after: Option<u64>) -> Marker {
        let Some(held) = self.per_chat.get(chat) else {
            return Marker::default();
        };
        let mut tally = Tally::default();
        for (seq, one) in held {
            if after.is_some_and(|looked| *seq <= looked) {
                continue;
            }
            tally.blocks = tally.blocks.saturating_add(one.blocks);
            tally.unverified_blocks = tally
                .unverified_blocks
                .saturating_add(one.unverified_blocks);
            tally.cant_tell_blocks = tally.cant_tell_blocks.saturating_add(one.cant_tell_blocks);
            tally.unreadable_blocks = tally
                .unreadable_blocks
                .saturating_add(one.unreadable_blocks);
        }
        Marker {
            blocks: tally.blocks,
            unverified: tally.unverified_blocks,
            tally,
            latest: held.back().map(|(seq, _)| *seq),
        }
    }
}

#[cfg(test)]
#[path = "chatlog/tests.rs"]
mod tests;

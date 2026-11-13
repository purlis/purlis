// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat's Chat log, for its tab** (R-224, R-227, R-233, R-310; ADR 0124 decision 11): the
//! rows of its story, read from this machine's event log through `purlis_core::eventlog::
//! chatlog::rows`, oldest first, and what heads it (`chatlog::summary`): its start and its end,
//! and between them its protection changes, its asks and their answers, its holds and its loop
//! pauses (T1-28); its blocks are T1-29's.
//!
//! **What writes an ask** (R-229, Q261): loop and budget pauses (`crate::pausing`), harness
//! prompts (`crate::hooks`, raised as the hook holds the ask and answered as the registry
//! applies it, a late answer included), sandbox asks (`crate::sandboxing`,
//! `crate::taskblocks`) and a held merge (`crate::mergeholds`). Consent, a hook's give-up and an
//! answer from `purlis inbox` write none yet: a follow-up's.
//!
//! **Pull-only** (R-225): read when the tab opens, told to no window, and nothing here counts
//! or marks a row as new. It holds nothing of its own: the Chat log is a query of the event
//! log, and the event log holds no chat's words.

use purlis_core::eventlog::chatlog::{self, Row, Summary};
use purlis_core::eventlog::stops::{
    Answer, AnsweredBy, AskAnswered, AskGroup, AskRaised, Stop, Target,
};
use purlis_core::sandbox::protection::InForce;

use crate::planes::PlaneId;

/// One layer's protection at a start, as the start row says it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct ProtectionLayer {
    /// The layer: `sandbox`, the one a tier sets in phase 1.
    pub layer: String,
    /// Its level's word: `on`, `floor` or `off`.
    pub level: String,
    /// Who set it: `policy`, `project`, `person`, `unconfirmed`, `chat`, `purlis` or
    /// `unknown`.
    pub tier: String,
}

/// The protection a start recorded (R-227): the four fields of its `run.started`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct StartProtection {
    pub layers: Vec<ProtectionLayer>,
    /// The last commit of `charter.toml`: a commit, `none` or `unknown`.
    pub charter_commit: String,
    /// The working `charter.toml` differed from that commit.
    pub charter_dirty: bool,
    /// How many answers were in force: none until phase 2.
    pub answers_in_force: u32,
}

/// One row of a chat's Chat log.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ChatLogRow {
    /// The chat's run started (`cause`: `start`, `reopen`, `fresh`, ...), with the protection
    /// it recorded, or none where its event has none this version can read.
    Started {
        /// When (UTC, RFC 3339), where the event's id says.
        at: Option<String>,
        cause: String,
        protection: Option<StartProtection>,
    },
    /// The chat's run ended: `completed`, `failed` or `stopped`, why, and its exit code.
    Ended {
        at: Option<String>,
        state: String,
        cause: String,
        code: Option<i32>,
    },
    /// The chat's sandbox was lifted for a run (`off`) or put back on (`on`), and who did it:
    /// `actor_kind` `human`, `host` or `agent`, and the actor's word.
    ProtectionChanged {
        at: Option<String>,
        change: String,
        actor_kind: String,
        actor: String,
    },
    /// The chat raised an ask, by its group's word.
    AskRaised { at: Option<String>, ask: String },
    /// The chat's ask was answered: `allowed` or `refused`, and by whom: `window` or `inbox`.
    AskAnswered {
        at: Option<String>,
        ask: String,
        answer: String,
        by: String,
    },
    /// purlis held an action and asked the person: the tool, and the guard's arm it was held
    /// at, where it names one (`merge-hold`).
    Held {
        at: Option<String>,
        tool: String,
        arm: Option<String>,
        /// It was held under one of the chat's sub-agents.
        sub_agent: bool,
    },
    /// The chat was paused as looping: the rule's word and how many times.
    LoopPaused {
        at: Option<String>,
        rule: String,
        times: u32,
    },
}

/// What heads a chat's Chat log: its allowed calls in one count, with those its person's rules
/// allowed, and its honesty lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, specta::Type)]
pub struct ChatLogSummary {
    pub allowed: u32,
    pub by_your_rules: u32,
    /// What reports its calls is within its own reach.
    pub unverified: bool,
    /// It ran with No sandbox.
    pub ran_as_you: bool,
}

/// A chat's Chat log: whether purlis knows the chat, and its rows.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct ChatLog {
    /// purlis knows the chat by its number: open, or ended here and still kept.
    pub known: bool,
    pub rows: Vec<ChatLogRow>,
    pub summary: ChatLogSummary,
}

fn at(ms: Option<u64>) -> Option<String> {
    let ms = i64::try_from(ms?).ok()?;
    chrono::DateTime::from_timestamp_millis(ms)
        .map(|when| when.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
}

fn protection_of(in_force: InForce) -> StartProtection {
    StartProtection {
        layers: in_force
            .layers
            .into_iter()
            .map(|layer| ProtectionLayer {
                layer: layer.layer,
                level: layer.level,
                tier: serde_json::to_value(layer.tier)
                    .ok()
                    .and_then(|word| word.as_str().map(str::to_owned))
                    .unwrap_or_else(|| purlis_core::eventlog::UNKNOWN.to_owned()),
            })
            .collect(),
        charter_commit: in_force.charter_commit,
        charter_dirty: in_force.charter_dirty,
        answers_in_force: u32::try_from(in_force.answers_in_force.len()).unwrap_or(u32::MAX),
    }
}

/// A closed word's own spelling, as the log writes it.
fn word_of(value: impl serde::Serialize) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|word| word.as_str().map(str::to_owned))
        .unwrap_or_else(|| purlis_core::eventlog::UNKNOWN.to_owned())
}

/// The window's row for the core's `row`.
fn row_of(row: Row) -> ChatLogRow {
    match row {
        Row::Started {
            at_ms,
            cause,
            protection,
        } => ChatLogRow::Started {
            at: at(at_ms),
            cause,
            protection: protection.map(protection_of),
        },
        Row::Ended {
            at_ms,
            state,
            cause,
            code,
        } => ChatLogRow::Ended {
            at: at(at_ms),
            state,
            cause,
            code: code.and_then(|code| i32::try_from(code).ok()),
        },
        Row::ProtectionChanged {
            at_ms,
            change,
            actor_kind,
            actor,
        } => ChatLogRow::ProtectionChanged {
            at: at(at_ms),
            change,
            actor_kind,
            actor,
        },
        Row::AskRaised { at_ms, ask } => ChatLogRow::AskRaised {
            at: at(at_ms),
            ask: word_of(ask),
        },
        Row::AskAnswered { at_ms, answered } => ChatLogRow::AskAnswered {
            at: at(at_ms),
            ask: word_of(answered.ask),
            answer: word_of(answered.answer),
            by: word_of(answered.by),
        },
        Row::Held {
            at_ms,
            hold,
            sub_agent,
        } => ChatLogRow::Held {
            at: at(at_ms),
            arm: match hold.target {
                Some(Target::GuardArm(arm)) => Some(arm.as_str().to_owned()),
                _ => None,
            },
            tool: hold.tool,
            sub_agent,
        },
        Row::LoopPaused { at_ms, pause } => ChatLogRow::LoopPaused {
            at: at(at_ms),
            rule: word_of(pause.rule),
            times: pause.times,
        },
    }
}

/// The window's summary for the core's.
fn summary_of(
    Summary {
        allowed,
        by_your_rules,
        unverified,
        ran_as_you,
    }: Summary,
) -> ChatLogSummary {
    ChatLogSummary {
        allowed,
        by_your_rules,
        unverified,
        ran_as_you,
    }
}

/// The Chat log of the chat with id `chat` (`None`: purlis does not know it) in `events`.
fn chat_log_of(events: &[purlis_core::eventlog::Event], chat: Option<&str>) -> ChatLog {
    match chat {
        None => ChatLog {
            known: false,
            rows: Vec::new(),
            summary: ChatLogSummary::default(),
        },
        Some(chat) => ChatLog {
            known: true,
            rows: chatlog::rows(events, chat)
                .into_iter()
                .map(row_of)
                .collect(),
            summary: summary_of(chatlog::summary(events, chat)),
        },
    }
}

/// **Chat `session`'s Chat log** (R-224): its rows, from this machine's event log, oldest
/// first. Read when its tab opens, off the window's thread, as it reads the log.
#[tauri::command]
#[specta::specta]
pub(crate) async fn chat_log(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: PlaneId,
    session: u32,
) -> Result<ChatLog, String> {
    let held = planes.held(&plane)?;
    let chat = held.chats().logged_id_of(session);
    let dir = match crate::EVENT_LOG.get() {
        Some(Ok(dir)) => dir.clone(),
        Some(Err(refused)) => {
            return Err(format!(
                "purlis keeps no event log on this machine: {}",
                refused.why
            ));
        }
        None => return Err("purlis keeps no event log on this machine.".to_owned()),
    };
    crate::off_the_window("reading a chat's log", move || {
        let events = purlis_core::eventlog::read(&dir).map_err(|why| why.to_string())?;
        Ok(chat_log_of(&events, chat.as_deref()))
    })
    .await
}

/// **Writes chat `chat`'s ask raised or answered, a hold or a loop pause** (R-229, R-232) into
/// the event log through `hooks`, where this machine keeps one: what its Chat log lists. A
/// write that fails is said in the app's log and changes nothing the person sees: the ask is
/// raised, answered or paused all the same.
pub(crate) fn record(hooks: &crate::hooks::Hooks, chat: u32, stop: Stop) {
    let kind = stop.kind();
    if let Err(why) = hooks.write_stop(chat, stop) {
        tracing::warn!("purlis: chat {chat}'s {kind} was not written to its log: {why}");
    }
}

/// **Chat `chat` raised an ask of group `ask`** (R-229), in its Chat log ([`record`]).
pub(crate) fn raised(hooks: &crate::hooks::Hooks, chat: u32, ask: AskGroup) {
    record(hooks, chat, Stop::AskRaised(AskRaised { ask }));
}

/// **The person answered chat `chat`'s ask of group `ask` in the window**, `allowed` it or not
/// (R-229), in its Chat log: an answer the window's own command applied, not the asks registry,
/// so by the window's answerer ([`crate::asking::the_window`]).
pub(crate) fn answered_in_the_window(
    hooks: &crate::hooks::Hooks,
    chat: u32,
    ask: AskGroup,
    allowed: bool,
) {
    record(hooks, chat, Stop::AskAnswered(in_the_window(ask, allowed)));
}

/// The person's answer to an ask of group `ask` in the window, `allowed` it or not.
pub(crate) fn in_the_window(ask: AskGroup, allowed: bool) -> AskAnswered {
    AskAnswered {
        ask,
        answer: if allowed {
            Answer::Allowed
        } else {
            Answer::Refused
        },
        by: AnsweredBy::from(crate::asking::the_window()),
    }
}

#[cfg(test)]
mod tests;

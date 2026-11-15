// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat's Chat log, for its tab** (R-224, R-227, R-233, R-310; ADR 0124 decision 11): the
//! rows of its story, read from this machine's event log through `purlis_core::eventlog::
//! chatlog::rows`, oldest first, and what heads it (`chatlog::summary`): its start and its end,
//! and between them its protection changes, its blocks (T1-29), its asks and their answers, its
//! holds and its loop pauses (T1-28).
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
//!
//! **A block's reason comes from memory, never the log** (R-340, Q185): what a harness's own
//! check said as it refused a call is held by the app as the hook channel hears it
//! ([`HARNESS_REASONS`]) and put on the block's row here, as written. Only a harness check's
//! block carries one (Q259): the reason is the chat's or its harness's words, never a guard's.
//! A restarted app holds none, so the row then has none.

use std::sync::LazyLock;

use purlis_core::eventlog::chatlog::{self, Row, Summary};
use purlis_core::eventlog::reasons::Reasons;
use purlis_core::eventlog::stops::{
    Answer, AnsweredBy, AskAnswered, AskGroup, AskRaised, Block, Source, Stop, Target,
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
    /// The last commit of the plane's own file: a commit, `none` or `unknown`.
    pub charter_commit: String,
    /// The plane's own file that commit is of: `purlis.toml` or `charter.toml`.
    pub charter_file: String,
    /// The working file differed from that commit.
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
    /// A call was blocked: by whom, how much its report is worth, at what, and why it said.
    Blocked {
        at: Option<String>,
        tool: String,
        source: String,
        qualifier: Option<String>,
        target_kind: Option<String>,
        target: Option<String>,
        harness: String,
        reason: Option<String>,
        /// It has since been resolved (Q272): its row offers no Tell the chat….
        resolved: bool,
        /// The labels its Allow next time may offer (T1-34, Q278), as policy keeps them.
        allow_levels: Vec<crate::sandboxing::GrantLevel>,
    },
}

/// What heads a chat's Chat log: its allowed calls in one count, with those its person's rules
/// allowed, its honesty lines, and what purlis can't tell of its harness.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, specta::Type)]
pub struct ChatLogSummary {
    pub allowed: u32,
    pub by_your_rules: u32,
    /// What reports its calls is within its own reach.
    pub unverified: bool,
    /// It ran with No sandbox.
    pub ran_as_you: bool,
    /// Its harness, by the word the plane calls it, where its latest start names one purlis
    /// ships: what the Can't tell lines name.
    pub harness: Option<String>,
    /// Its harness does not report what its own check blocks (R-097, R-100).
    pub cant_tell_its_checks_blocks: bool,
    /// Its harness reports no mode purlis can check (R-099).
    pub cant_tell_its_mode: bool,
}

/// **The reasons the harnesses' own checks gave, for the app's life** (R-340): held as the
/// hook channel hears them (`hooks.rs`), read as a Chat log is, never written.
pub(crate) static HARNESS_REASONS: LazyLock<Reasons> = LazyLock::new(Reasons::default);

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
        charter_file: in_force.charter_file,
        charter_dirty: in_force.charter_dirty,
        answers_in_force: u32::try_from(in_force.answers_in_force.len()).unwrap_or(u32::MAX),
    }
}

/// A closed word's own spelling, as the log writes it.
pub(crate) fn word_of(value: impl serde::Serialize) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|word| word.as_str().map(str::to_owned))
        .unwrap_or_else(|| purlis_core::eventlog::UNKNOWN.to_owned())
}

/// A block's target, as its kind's word and its value.
pub(crate) fn target_of(target: &Target) -> (String, String) {
    match target {
        Target::Host(host) => ("host".to_owned(), host.as_str().to_owned()),
        Target::Folder(folder) => ("folder".to_owned(), folder.as_str().to_owned()),
        Target::Service(service) => ("service".to_owned(), service.as_str().to_owned()),
        Target::GuardArm(arm) => ("guard_arm".to_owned(), arm.as_str().to_owned()),
    }
}

/// The window's row for the core's `row`, a block's with the reason `reason` finds for its
/// call where the block is a harness's own check's (Q259): a guard's or the sandbox's has none.
fn row_of(row: Row, reason: &dyn Fn(&str) -> Option<String>) -> ChatLogRow {
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
        Row::Blocked {
            at_ms,
            block,
            call,
            harness,
            resolved,
        } => {
            let harness_check = matches!(
                &block,
                Some(Block {
                    source: Source::HarnessCheck,
                    ..
                })
            );
            let (tool, source, qualifier, target) = match block {
                Some(Block {
                    tool,
                    source,
                    qualifier,
                    target,
                    ..
                }) => (
                    tool,
                    word_of(source),
                    qualifier.map(word_of),
                    target.as_ref().map(target_of),
                ),
                None => (
                    purlis_core::eventlog::UNKNOWN.to_owned(),
                    purlis_core::eventlog::UNKNOWN.to_owned(),
                    None,
                    None,
                ),
            };
            let (target_kind, target) = target.unzip();
            ChatLogRow::Blocked {
                at: at(at_ms),
                tool,
                source,
                qualifier,
                target_kind,
                target,
                harness,
                // The reason is the chat's or its harness's words (Q259): only a harness's own
                // check's block carries one, never a guard's or the sandbox's.
                reason: call.as_deref().filter(|_| harness_check).and_then(reason),
                resolved,
                // The core says them where the project's policy is read ([`with_levels`]).
                allow_levels: Vec::new(),
            }
        }
    }
}

/// **`log`, each sandbox block's row with the labels its Allow next time may offer under
/// `locks`** (T1-34, Q278; #1906): the core's word, so the window offers only those.
fn with_levels(mut log: ChatLog, locks: &purlis_core::sandbox::policy::Locks) -> ChatLog {
    for row in &mut log.rows {
        if let ChatLogRow::Blocked {
            source,
            qualifier,
            target_kind,
            target,
            resolved: false,
            allow_levels,
            ..
        } = row
        {
            *allow_levels = crate::network::next_time_levels(
                locks,
                source,
                qualifier.as_deref(),
                target_kind.as_deref(),
                target.as_deref(),
            );
        }
    }
    log
}

/// The window's summary for the core's.
fn summary_of(
    Summary {
        allowed,
        by_your_rules,
        unverified,
        ran_as_you,
        harness,
        cant_tell_its_checks_blocks,
        cant_tell_its_mode,
    }: Summary,
) -> ChatLogSummary {
    ChatLogSummary {
        allowed,
        by_your_rules,
        unverified,
        ran_as_you,
        harness: harness.map(|harness| harness.name().to_owned()),
        cant_tell_its_checks_blocks,
        cant_tell_its_mode,
    }
}

/// The Chat log of the chat with id `chat` (`None`: purlis does not know it) in `events`, with
/// no reason for any block.
#[cfg(test)]
fn chat_log_of(events: &[purlis_core::eventlog::Event], chat: Option<&str>) -> ChatLog {
    chat_log_with(events, chat, &|_| None)
}

/// [`chat_log_of`], each block with the reason `reason` finds for its call (R-340).
fn chat_log_with(
    events: &[purlis_core::eventlog::Event],
    chat: Option<&str>,
    reason: &dyn Fn(&str) -> Option<String>,
) -> ChatLog {
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
                .map(|row| row_of(row, reason))
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
        let root = plane.root();
        let log = chat_log_with(&events, chat.as_deref(), &|call| {
            HARNESS_REASONS.of(root, session, call)
        });
        Ok(with_levels(
            log,
            &purlis_core::sandbox::policy::Locks::of(root),
        ))
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

/// One chat the window asks the blocked marker of, and its last look: the `seq` of the latest
/// block the person looked at, read from its key in the seen record (decimal, `None` for
/// never).
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, specta::Type)]
pub struct BlocksAsked {
    pub session: u32,
    pub after: Option<String>,
}

/// **A chat's blocks since the person last looked** (R-172, R-174, R-299): what its marker
/// shows.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct ChatBlocks {
    pub session: u32,
    /// The verified blocks after the look: the marker's count, "2 new blocked".
    pub blocks: u32,
    /// The unverified ones after it, which never count on the sidebar: a muted dot.
    pub unverified: u32,
    /// The seen record's key for a look at the chat's latest block, `blocks:<chat>:<seq>`, the
    /// chat by its logged id, stable across restarts (Q258): what the window keeps once the chat
    /// has been looked at. `None` for a chat with none, or one the log does not know.
    pub key: Option<String>,
}

/// **The markers' running tally** (R-174): fed from the event log's subscription, so a refresh
/// reads only what was written since the last one. One per log; a log opened elsewhere starts
/// it again.
static BLOCKS: std::sync::Mutex<Option<Feed>> = std::sync::Mutex::new(None);

/// The log a tally reads, where it has read it to, and what it has counted.
struct Feed {
    dir: std::path::PathBuf,
    read: purlis_core::eventlog::Subscription,
    blocks: chatlog::Blocks,
}

/// One asked chat: its number, the id the event log knows it by, and its last look.
struct Asked {
    session: u32,
    chat: Option<String>,
    after: Option<u64>,
}

/// Each of `asked`'s markers, after reading on from the log in `dir`.
fn blocks_of(dir: &std::path::Path, asked: &[Asked]) -> Result<Vec<ChatBlocks>, String> {
    let mut held = BLOCKS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if held.as_ref().is_none_or(|feed| feed.dir != dir) {
        *held = Some(Feed {
            dir: dir.to_path_buf(),
            read: purlis_core::eventlog::subscribe(dir, 0),
            blocks: chatlog::Blocks::default(),
        });
    }
    let Some(Feed { read, blocks, .. }) = held.as_mut() else {
        return Ok(Vec::new());
    };
    for delivery in read.poll().map_err(|why| why.to_string())? {
        blocks.take(delivery);
    }
    Ok(asked
        .iter()
        .map(|one| {
            let Some(chat) = one.chat.as_deref() else {
                return ChatBlocks {
                    session: one.session,
                    blocks: 0,
                    unverified: 0,
                    key: None,
                };
            };
            let marker = blocks.since(chat, one.after);
            ChatBlocks {
                session: one.session,
                blocks: marker.blocks,
                unverified: marker.unverified,
                key: marker
                    .latest
                    .map(|seq| format!("{}{chat}:{seq}", purlis_core::inboxupdates::BLOCKS)),
            }
        })
        .collect())
}

/// **Each asked chat's blocked marker** (R-172, R-174, R-299; ADR 0124 decision 9.1): its
/// verified and unverified blocks after the last look the window keeps for it, from this
/// machine's event log. No notification and no announcement is made of them (R-176), and
/// nothing of Quiet or snooze is touched (R-177): it only reads.
#[tauri::command]
#[specta::specta]
pub(crate) async fn chat_blocks(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: PlaneId,
    asked: Vec<BlocksAsked>,
) -> Result<Vec<ChatBlocks>, String> {
    let held = planes.held(&plane)?;
    let asked: Vec<Asked> = asked
        .into_iter()
        .map(|one| Asked {
            session: one.session,
            chat: held.chats().logged_id_of(one.session),
            after: one.after.and_then(|after| after.parse().ok()),
        })
        .collect();
    let dir = match crate::EVENT_LOG.get() {
        Some(Ok(dir)) => dir.clone(),
        // No log on this machine: no block is counted, and no marker is drawn.
        _ => return Ok(Vec::new()),
    };
    crate::off_the_window("counting a chat's blocks", move || blocks_of(&dir, &asked)).await
}

#[cfg(test)]
mod tests;

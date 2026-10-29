// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **One answer for the tasks of one session that hit the same sandbox block** (#1508, V100-57).
//!
//! Several tasks of one session that are blocked on the same host, or on writing the same
//! folder, are asked about in one Notice on the session's tab, and the one answer applies to
//! each task the Notice listed. This is the core's half: it holds the block each open chat is
//! on now ([`Blocks`]), and judges and keeps an answer only against that.
//!
//! # What holds (D-1508-9, fail closed)
//!
//! - **An answer names the exact blocks the person saw**: each task with the block shown for
//!   it ([`SeenBlock`]). The answer is refused whole if any task is not held on that block now
//!   (it was never blocked, it was answered already, it moved on to another block, or it
//!   ended), if the blocks are not one block, or if any task is not recorded below the
//!   session. So a press can grant only a task whose block was on screen, as the dispatch
//!   question's answer is bound to what it showed. Keep blocked is checked the same way.
//! - **Who asked whom is purlis's own record**, the lineage the app wrote when it started each
//!   task ([`purlis_core::reopen::HandedFrom`]). The session itself is never one of its tasks: a
//!   permission given to a session does not reach its tasks, and an answer about its tasks does
//!   not reach the session.
//! - **"For this chat" is each task's own grant**, held under that task's id and judged against
//!   that task's own folder. Every task is judged before anything is kept. If keeping fails part
//!   way (the audit cannot be written), the answer says which tasks it allowed, and only those.
//! - **"For me" and "for the project" are kept once**, audited once more for each other task
//!   listed, and every task listed is owed the restart that takes it.

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

use purlis_core::sandbox::grant::{self, Level, What};

use crate::planes::{PlaneId, Planes};
use crate::sandboxing::{GrantLevel, GrantWhat};

/// How far up a lineage is followed before it is taken to reach nothing: far past any depth a
/// dispatch is allowed, so only a record that loops stops here.
const MOST_DEPTH: usize = 64;

/// The most blocks the app holds for one chat at once, as the window keeps them
/// (`sandboxBlocks.ts`): one per operation and kind, the oldest going first. The hosts of one
/// block count once ([`MOST_HOSTS_PER_BLOCK`]).
const MOST_HELD_PER_CHAT: usize = 5;

/// The most hosts one block of a chat names at once, as the window's one Notice lists them
/// (#1637): the oldest going first. As many as one tool result can be refused.
const MOST_HOSTS_PER_BLOCK: usize = purlis_core::sandboxblock::AT_MOST_PER_RESULT;

/// The most blocks of one chat kept as waiting on the person at once ([`Blocks::heard_waiting`]):
/// as many connections as one chat's proxy holds. Past that the oldest waits no longer, so what
/// is held for one chat stays bounded.
const MOST_WAITING_PER_CHAT: usize = purlis_core::sandbox::asks::MOST_HELD;

/// **The host or folder a block names, as a grant matches it**: a host lowered, without a
/// trailing dot, and without its scheme's default port (`:443`, `:80`), which the grant for the
/// bare host covers; a folder as the core offered it. None for what no answer names.
pub fn normalised(what: GrantWhat, target: &str) -> Option<String> {
    match what {
        GrantWhat::Host => {
            let host = grant::host(target).ok()?;
            let said = host.to_string();
            Some(match host.port() {
                Some(port @ (443 | 80)) => said
                    .strip_suffix(&format!(":{port}"))
                    .unwrap_or(&said)
                    .to_owned(),
                _ => said,
            })
        }
        GrantWhat::Write | GrantWhat::Socket | GrantWhat::Service => {
            (!target.is_empty()).then(|| target.to_owned())
        }
        // A service on this Mac by its one port, as the core judges one (qw384), and a port
        // to listen on likewise (#2019).
        GrantWhat::Port | GrantWhat::Listen => purlis_core::sandbox::localport::port(target)
            .ok()
            .map(|port| port.to_string()),
        // None is ever a block's: each is granted from its own Notice or Ask (#1430, #1362,
        // #2241).
        GrantWhat::Vault
        | GrantWhat::PersonaHosts
        | GrantWhat::Browser
        | GrantWhat::BrowserSite => None,
    }
}

/// One block a chat is held on: its operation and kind, by their words, and what it names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeldBlock {
    pub operation: String,
    pub kind: String,
    pub what: GrantWhat,
    /// [`normalised`]; empty for a host the block did not name, which the person types on the
    /// chat's own Notice ([`HeldBlock::unnamed_host`]).
    pub target: String,
}

impl HeldBlock {
    /// A block on a host its report did not name: only the chat's own Notice answers it, with
    /// the host the person types (#1538). No question for several tasks matches it.
    pub fn unnamed_host(operation: &str, kind: &str) -> Self {
        Self {
            operation: operation.to_owned(),
            kind: kind.to_owned(),
            what: GrantWhat::Host,
            target: String::new(),
        }
    }

    /// A block on a socket or a service its report did not name (#1884): only the chat's own
    /// Notice answers it, once the person has chosen which (`crate::sandboxchoose`). No ask is
    /// listed for it, and no Allow matches it until then.
    pub fn unnamed(operation: &str, kind: &str, what: GrantWhat) -> Self {
        Self {
            operation: operation.to_owned(),
            kind: kind.to_owned(),
            what,
            target: String::new(),
        }
    }

    /// A host its report named: one of several a chat may be held on at once under one
    /// operation and kind (#1637).
    fn names_a_host(&self) -> bool {
        self.what == GrantWhat::Host && !self.target.is_empty()
    }

    /// Whether `other` is asked about in the same Notice: the same operation and kind, and both
    /// naming a host or neither. Hosts named alike are listed together in one Notice; any other
    /// block is one per operation and kind, the newest replacing the one before.
    fn shares_a_notice(&self, other: &Self) -> bool {
        self.operation == other.operation
            && self.kind == other.kind
            && self.names_a_host() == other.names_a_host()
    }
}

/// **The block each open chat is held on now**, as the app heard it: one per operation and
/// kind, the newest replacing the one before as the window's Notice does, except that each host
/// a report named is held beside the others its one Notice lists (#1637). Cleared when an
/// answer takes it, and when the chat ends ([`crate::chats::Chats::close`]). In memory only.
#[derive(Default)]
pub struct Blocks {
    held: Mutex<HashMap<u32, Vec<HeldBlock>>>,
    /// The turn each open chat last had a block Notice raised in, by the count of turns its
    /// board had then (#1663): what the chat that asked for it is told it waits on.
    noticed: Mutex<HashMap<u32, u32>>,
    /// When each block held was first heard, in seconds since 1970 (#1700): what the asks
    /// registry orders the longest waiting first by. Kept beside `held`, only for what it holds.
    since: Mutex<HashMap<u32, Vec<(HeldBlock, u64)>>>,
    /// The blocks of each open chat whose connection is held now, waiting on the person
    /// (#1709): never pushed out by what is heard after them, until they no longer wait.
    waiting: Mutex<HashMap<u32, Vec<HeldBlock>>>,
    /// The blocks of each open chat found in what a tool that ran nothing read back (#1884):
    /// held, so the person may still answer one, and listed by no ask until they take it up
    /// ([`Self::taken_up`]) or a command of the chat is refused the same.
    quiet: Mutex<HashMap<u32, Vec<HeldBlock>>>,
}

impl Blocks {
    fn held(&self) -> std::sync::MutexGuard<'_, HashMap<u32, Vec<HeldBlock>>> {
        self.held.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Chat `session` is held on `block` now: beside the other hosts its Notice lists, where it
    /// names a host (#1637), and otherwise in place of the block of its operation and kind.
    pub fn heard(&self, session: u32, block: HeldBlock) {
        self.heard_at(session, block, crate::sandboxing::now_secs());
    }

    /// [`Self::heard`], at `at`, in seconds since 1970: a block heard again while it is held
    /// keeps the time it was first heard.
    pub fn heard_at(&self, session: u32, block: HeldBlock, at: u64) {
        // Heard of a command the chat ran: no longer the quiet line of a read (#1884).
        self.loud(session, |one| {
            if block.names_a_host() {
                *one == block
            } else {
                one.shares_a_notice(&block)
            }
        });
        let waits = self.waiting().get(&session).cloned().unwrap_or_default();
        let waits = |one: &HeldBlock| waits.contains(one);
        let mut held = self.held();
        let mine = held.entry(session).or_default();
        if block.names_a_host() {
            mine.retain(|one| *one != block);
        } else {
            mine.retain(|one| !one.shares_a_notice(&block));
        }
        mine.push(block.clone());
        let hosts = mine
            .iter()
            .filter(|one| one.shares_a_notice(&block))
            .count();
        // A host whose connection waits on the person is never the one that goes (#1709).
        if hosts > MOST_HOSTS_PER_BLOCK
            && let Some(oldest) = mine
                .iter()
                .position(|one| one.shares_a_notice(&block) && !waits(one))
        {
            mine.remove(oldest);
        }
        // The oldest Notice goes first, with every host it lists, unless one of them waits on
        // the person (#1709).
        loop {
            let mut notices: Vec<&HeldBlock> = Vec::new();
            for one in mine.iter() {
                if !notices.iter().any(|seen| seen.shares_a_notice(one)) {
                    notices.push(one);
                }
            }
            if notices.len() <= MOST_HELD_PER_CHAT {
                break;
            }
            let Some(oldest) = notices
                .iter()
                .find(|notice| {
                    !mine
                        .iter()
                        .any(|one| one.shares_a_notice(notice) && waits(one))
                })
                .map(|notice| (*notice).clone())
            else {
                break;
            };
            mine.retain(|one| !one.shares_a_notice(&oldest));
        }
        let mut since = self.since();
        let was = since.remove(&session).unwrap_or_default();
        let kept: Vec<(HeldBlock, u64)> = mine
            .iter()
            .map(|one| {
                let first = was
                    .iter()
                    .find(|(seen, _)| seen == one)
                    .map_or(at, |(_, first)| *first);
                (one.clone(), first)
            })
            .collect();
        since.insert(session, kept);
    }

    fn quiet(&self) -> std::sync::MutexGuard<'_, HashMap<u32, Vec<HeldBlock>>> {
        self.quiet.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Chat `session`'s quiet blocks that `gone` names are quiet no longer.
    fn loud(&self, session: u32, gone: impl Fn(&HeldBlock) -> bool) {
        let mut quiet = self.quiet();
        if let Some(mine) = quiet.get_mut(&session) {
            mine.retain(|one| !gone(one));
            if mine.is_empty() {
                quiet.remove(&session);
            }
        }
    }

    /// [`Self::heard`], for a block found in what a tool that ran nothing read back (#1884):
    /// held as any block is, so its Notice's answers are checked against it, and quiet, so no
    /// ask lists it ([`Self::every_since`]) until the person takes it up ([`Self::taken_up`]).
    /// A block the chat is already held on by a command it ran stays as it is.
    pub fn heard_quietly(&self, session: u32, block: HeldBlock) {
        if self.holds(session, &block) && !self.is_quiet(session, &block) {
            return;
        }
        self.heard(session, block.clone());
        let held = self.held().get(&session).cloned().unwrap_or_default();
        let mut quiet = self.quiet();
        let mine = quiet.entry(session).or_default();
        mine.push(block);
        // Only what is still held: the bound on what is held bounds this too.
        mine.retain(|one| held.contains(one));
    }

    /// Whether chat `session` is held on `block` quietly.
    pub fn is_quiet(&self, session: u32, block: &HeldBlock) -> bool {
        self.quiet()
            .get(&session)
            .is_some_and(|mine| mine.contains(block))
    }

    /// **The person took `block` of chat `session` up** (#1884): it is quiet no longer, and is
    /// listed as any block is. Whether the chat is held on it now; taking up one that is not
    /// quiet changes nothing.
    pub fn taken_up(&self, session: u32, block: &HeldBlock) -> bool {
        if !self.holds(session, block) {
            return false;
        }
        self.loud(session, |one| one == block);
        true
    }

    fn since(&self) -> std::sync::MutexGuard<'_, HashMap<u32, Vec<(HeldBlock, u64)>>> {
        self.since.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn waiting(&self) -> std::sync::MutexGuard<'_, HashMap<u32, Vec<HeldBlock>>> {
        self.waiting.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// [`Self::heard`], for a block whose connection is held now, waiting on the person
    /// (#1709): a held ask is not dropped while its connection waits, whatever is heard after
    /// it, until it is answered, it no longer waits ([`Self::no_longer_waits`]), or the chat
    /// ends. At most [`MOST_WAITING_PER_CHAT`] wait at once, the oldest going first.
    pub fn heard_waiting(&self, session: u32, block: HeldBlock) {
        {
            let mut waiting = self.waiting();
            let mine = waiting.entry(session).or_default();
            mine.retain(|one| *one != block);
            mine.push(block.clone());
            if mine.len() > MOST_WAITING_PER_CHAT {
                mine.remove(0);
            }
        }
        self.heard(session, block);
    }

    /// `block` of chat `session` waits on the person no longer (its connection gave up): it is
    /// still held, and goes as any other block does.
    pub fn no_longer_waits(&self, session: u32, block: &HeldBlock) {
        let mut waiting = self.waiting();
        if let Some(mine) = waiting.get_mut(&session) {
            mine.retain(|one| one != block);
            if mine.is_empty() {
                waiting.remove(&session);
            }
        }
    }

    /// Whether chat `session` is held on `block` now.
    pub fn holds(&self, session: u32, block: &HeldBlock) -> bool {
        self.held()
            .get(&session)
            .is_some_and(|mine| mine.contains(block))
    }

    /// `block` of chat `session` was answered: it is held no longer.
    pub fn answered(&self, session: u32, block: &HeldBlock) {
        self.no_longer_waits(session, block);
        self.loud(session, |one| one == block);
        self.noticed().remove(&session);
        let mut held = self.held();
        if let Some(mine) = held.get_mut(&session) {
            mine.retain(|one| one != block);
            if mine.is_empty() {
                held.remove(&session);
            }
        }
        let mut since = self.since();
        if let Some(mine) = since.get_mut(&session) {
            mine.retain(|(one, _)| one != block);
            if mine.is_empty() {
                since.remove(&session);
            }
        }
    }

    /// Chat `session` ended: nothing is held for it.
    pub fn ended(&self, session: u32) {
        self.held().remove(&session);
        self.noticed().remove(&session);
        self.since().remove(&session);
        self.waiting().remove(&session);
        self.quiet().remove(&session);
    }

    fn noticed(&self) -> std::sync::MutexGuard<'_, HashMap<u32, u32>> {
        self.noticed.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// A Notice went up for a block of chat `session`, in the turn its board counts as `turn`
    /// (#1663), whatever the Notice offers.
    pub fn raised(&self, session: u32, turn: u32) {
        self.noticed().insert(session, turn);
    }

    /// **Whether chat `session` waits on the person over a block's Notice** (#1663): a Notice
    /// went up in the turn its board counts as `turn` now, and that turn has ended
    /// (`waiting`). A new turn, a start again (whose count begins anew), an answer and an end
    /// each move it on.
    pub fn waits_on_a_notice(&self, session: u32, turn: u32, waiting: bool) -> bool {
        waiting && self.noticed().get(&session) == Some(&turn)
    }
}

/// Holds the block the window is about to be told of, where an answer could name it: a host
/// or a folder to write, named whole, a host the report did not name, which the person
/// types on the chat's own Notice, or a socket or a service the person may choose
/// ([`crate::hooks::ChatBlocked::choose`], #1884). One read back is held quietly
/// ([`Blocks::heard_quietly`]).
pub fn heard(chats: &crate::chats::Chats, told: &crate::hooks::ChatBlocked) {
    let Some(block) = held_of(told) else {
        return;
    };
    if told.quiet.is_some() {
        chats.blocks().heard_quietly(told.session, block);
    } else {
        chats.blocks().heard(told.session, block);
    }
}

/// The block a chat is held on for what the window is `told`, or none where no answer names it.
pub fn held_of(told: &crate::hooks::ChatBlocked) -> Option<HeldBlock> {
    let Some(what) = told.offer.granted() else {
        // Nothing to allow yet, and something the person may choose.
        return told
            .choose
            .map(|what| HeldBlock::unnamed(&told.operation, &told.kind, what));
    };
    match told.target.as_deref() {
        None if what == GrantWhat::Host => {
            Some(HeldBlock::unnamed_host(&told.operation, &told.kind))
        }
        // A port the refusal did not name (qw384, #2019): the person types it, as a host.
        None if matches!(what, GrantWhat::Port | GrantWhat::Listen) => {
            Some(HeldBlock::unnamed(&told.operation, &told.kind, what))
        }
        None => None,
        Some(target) => Some(HeldBlock {
            operation: told.operation.clone(),
            kind: told.kind.clone(),
            what,
            target: normalised(what, target)?,
        }),
    }
}

/// **What one chat's own block Notice answers** (#1538): the block's operation and kind, by the
/// words the window was told, what it offers, and the host or folder Allow names: the one the
/// Notice showed whole, or the host the person typed where the block named none.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct BlockShown {
    pub operation: String,
    pub kind: String,
    pub what: GrantWhat,
    pub target: String,
}

/// **The block one chat's own Notice answers** (#1538, as D-1508-9 binds the question for
/// several tasks): chat `session` held now on the block of that operation and kind, offering
/// `what`, and naming `target` as a grant matches it, or naming no host where the person typed
/// `target`. Anything else (a block never heard, answered already, replaced by a newer one, or
/// of a chat that ended) refuses the Allow whole, before anything is judged or kept.
pub fn shown_one(blocks: &Blocks, session: u32, shown: &BlockShown) -> Result<HeldBlock, String> {
    let BlockShown {
        operation,
        kind,
        what,
        target,
    } = shown;
    let (what, target) = (*what, target.as_str());
    let named = normalised(what, target).map(|target| HeldBlock {
        operation: operation.to_owned(),
        kind: kind.to_owned(),
        what,
        target,
    });
    // A host or a port the person typed, where the block named none (qw384 for a port).
    let typed = matches!(what, GrantWhat::Host | GrantWhat::Port | GrantWhat::Listen)
        .then(|| HeldBlock::unnamed(operation, kind, what));
    named
        .into_iter()
        .chain(typed)
        .find(|block| blocks.holds(session, block))
        .ok_or_else(|| {
            format!(
                "Nothing was allowed: chat {session} is not blocked on what this Notice showed any \
                 more. It was answered already, or the chat moved on or ended."
            )
        })
}

/// The chat that asked for chat `session` as a task, by the app's own record of it; none for a
/// chat no task link records (the person started it, or a handoff did).
pub type AskerOf<'a> = &'a dyn Fn(u32) -> Option<u32>;

/// **Whether `task` is a task below `session`**, at any depth, by `asker_of`: the app's record
/// of who asked whom. A chat is never below itself.
pub fn below(asker_of: AskerOf<'_>, session: u32, task: u32) -> bool {
    let mut at = task;
    for _ in 0..MOST_DEPTH {
        match asker_of(at) {
            Some(asker) if asker == session => return true,
            Some(asker) if asker != at => at = asker,
            _ => return false,
        }
    }
    false
}

/// **One task's block, as the person saw it** on the question: what the window sends back.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct SeenBlock {
    /// The task's number.
    pub task: u32,
    /// The block's operation and kind, by the words the window was told.
    pub operation: String,
    pub kind: String,
    pub what: GrantWhat,
    /// The host or folder shown for this task.
    pub target: String,
}

/// What one answer to several tasks did (#1508).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct TasksAnswered {
    /// The sentence the Notice says.
    pub said: String,
    /// The tasks it answered: each listed one, or, where keeping failed part way, those it
    /// allowed before it failed. The window puts their blocks away and restarts them.
    pub answered: Vec<u32>,
}

/// What [`checked`] and [`answered`] read of the app, so they read no chat store of their own.
pub struct Reading<'a> {
    /// Who asked whom ([`below`]).
    pub asker_of: AskerOf<'a>,
    /// Whether a task is held on a block now ([`Blocks::holds`]).
    pub holds: &'a dyn Fn(u32, &HeldBlock) -> bool,
}

/// **What an answer to several tasks of `session` is checked against before anything is done**
/// (D-1508-9): one block, named the same for every task; each task held on it now; each
/// recorded below `session`, none of them `session`, none twice. Answers the tasks and the one
/// block, or why the whole answer is refused.
pub fn checked(
    session: u32,
    seen: &[SeenBlock],
    reading: &Reading<'_>,
) -> Result<(Vec<u32>, HeldBlock), String> {
    let refused = |why: &str| Err(format!("Nothing was answered: {why}"));
    let [first, ..] = seen else {
        return refused("no task was named.");
    };
    let held_of = |one: &SeenBlock| {
        normalised(one.what, &one.target).map(|target| HeldBlock {
            operation: one.operation.clone(),
            kind: one.kind.clone(),
            what: one.what,
            target,
        })
    };
    let Some(block) = held_of(first) else {
        return refused("the question names nothing that can be answered.");
    };
    let mut tasks = Vec::with_capacity(seen.len());
    for one in seen {
        if held_of(one).as_ref() != Some(&block) {
            return refused(
                "the tasks listed are not on one block, so one answer cannot cover them.",
            );
        }
        if one.task == session {
            return refused(
                "a session is not one of its own tasks. Answer the session's own block on it.",
            );
        }
        if tasks.contains(&one.task) {
            return Err(format!(
                "Nothing was answered: chat {} was named twice.",
                one.task
            ));
        }
        if !below(reading.asker_of, session, one.task) {
            return Err(format!(
                "Nothing was answered: purlis has no record of chat {} as a task of this \
                 session, and one answer covers only the session's own tasks.",
                one.task
            ));
        }
        if !(reading.holds)(one.task, &block) {
            return Err(format!(
                "Nothing was answered: chat {} is not blocked on what the question showed any \
                 more. The question is shown again as it is now.",
                one.task
            ));
        }
        tasks.push(one.task);
    }
    Ok((tasks, block))
}

/// Judges what allowing a block for one task would grant.
pub type Judge<'a> =
    &'a dyn Fn(u32, &HeldBlock) -> Result<(What, Level), crate::sandboxing::NotJudged>;

/// **Why one answer to several tasks kept nothing**: the sentence the question says, and
/// whether it was because a socket is no longer the one the question drew (#1927), which is
/// answered by telling each task's block again as it stands ([`allowed_or_told_again`]). Told
/// by its kind, never read back out of the sentence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotAnswered {
    pub said: String,
    pub socket_changed: bool,
}

impl From<String> for NotAnswered {
    fn from(said: String) -> Self {
        Self {
            said,
            socket_changed: false,
        }
    }
}

/// What [`answered`] is told to do with the app.
pub struct Doing<'a> {
    /// What allowing `block` for one task would grant, judged by the core
    /// ([`crate::sandboxing::judged`]).
    pub judge: Judge<'a>,
    /// Keeps one grant for one task: audited, kept and the task owed its restart
    /// ([`crate::sandboxing::kept`]).
    pub keep: &'a mut dyn FnMut(u32, &What, Level) -> Result<(), String>,
    /// Audits a grant kept once for everyone as reaching one more task.
    pub note: &'a dyn Fn(u32, &What, Level) -> Result<(), String>,
    /// Owes one task a restart that tells it what it was allowed: unless its proxy took a
    /// host live (#1666), which the app reads as it owes it.
    pub owe: &'a dyn Fn(u32, &What, String),
}

/// **The one Allow to the tasks a Notice listed** ([module](self)): [`checked`], every task
/// judged, then kept for each task (once, this chat) or once (for me, for the project) with
/// each task audited and owed its restart. Nothing is kept unless every check passed.
pub fn answered(
    session: u32,
    seen: &[SeenBlock],
    level: GrantLevel,
    reading: &Reading<'_>,
    doing: Doing<'_>,
) -> Result<TasksAnswered, NotAnswered> {
    let (tasks, block) = checked(session, seen, reading)?;
    let mut judged = Vec::with_capacity(tasks.len());
    for task in &tasks {
        let (what, level) = (doing.judge)(*task, &block).map_err(|why| NotAnswered {
            said: format!("Nothing was allowed: for chat {task}, {why}"),
            socket_changed: why == crate::sandboxing::NotJudged::SocketChanged,
        })?;
        judged.push((*task, what, level));
    }
    let count = tasks.len();
    // A one-chat Allow ("Allow once", "Allow for this chat") is kept for each task on its own
    // (#2629): each holds it, is audited for it and owed its restart by its own keep.
    if Level::from(level).one_chat() {
        let mut allowed = Vec::new();
        for (task, what, level) in &judged {
            if let Err(why) = (doing.keep)(*task, what, *level) {
                if allowed.is_empty() {
                    return Err(format!("Nothing was allowed: {why}").into());
                }
                return Ok(TasksAnswered {
                    said: format!(
                        "It was allowed for chat {} only, and not for the rest: {why}",
                        numbers(&allowed)
                    ),
                    answered: allowed,
                });
            }
            allowed.push(*task);
        }
        return Ok(TasksAnswered {
            said: if count == 1 {
                "Allowed for this task alone, not for the chat that asked it. It restarts on \
                 the same conversation once its turn ends, and is told to retry."
                    .to_owned()
            } else {
                format!(
                    "Allowed for each of these {count} tasks on its own, not for the chat that \
                     asked them. Each restarts on the same conversation once its turn ends, and \
                     is told to retry."
                )
            },
            answered: allowed,
        });
    }
    // Kept once: every task was judged to the same grant, or it is not one answer.
    let [(first, what, level), rest @ ..] = judged.as_slice() else {
        return Err("Nothing was allowed: no task was named.".to_owned().into());
    };
    if rest.iter().any(|(_, other, _)| other != what) {
        return Err(
            "Nothing was allowed: these tasks do not want the same thing, so one answer cannot \
             cover them. Answer each on its own."
                .to_owned()
                .into(),
        );
    }
    (doing.keep)(*first, what, *level).map_err(|why| format!("Nothing was allowed: {why}"))?;
    let told = grant::told(what, *level);
    for (task, _, _) in rest {
        if let Err(why) = (doing.note)(*task, what, *level) {
            // The grant stands, audited once; only this task's line of it is missing.
            tracing::warn!("purlis: a sandbox grant reaching chat {task} was not audited ({why})");
        }
        (doing.owe)(*task, what, told.clone());
    }
    Ok(TasksAnswered {
        said: format!(
            "Allowed {}. The {count} {} restart on the same conversation once each one's turn \
             ends, and are told to retry.",
            level.said(),
            if count == 1 { "task" } else { "tasks" }
        ),
        answered: tasks,
    })
}

/// `1, 4 and 5`.
fn numbers(of: &[u32]) -> String {
    let said: Vec<String> = of.iter().map(u32::to_string).collect();
    match said.as_slice() {
        [] => String::new(),
        [one] => one.clone(),
        [init @ .., last] => format!("{} and {last}", init.join(", ")),
    }
}

/// The chat that asked for chat `session` as a task, as `chats` records it while it is open.
fn asker_in(chats: &crate::chats::Chats, session: u32) -> Option<u32> {
    let from = chats.recorded_chat(session)?.from?;
    (from.mode == purlis_core::reopen::Mode::Task).then_some(from.chat)
}

/// Runs `with` on what `chats` is read for: its lineage and the blocks it holds.
fn reading<R>(chats: &crate::chats::Chats, with: impl FnOnce(&Reading<'_>) -> R) -> R {
    with(&Reading {
        asker_of: &|chat| asker_in(chats, chat),
        holds: &|task, block| chats.blocks().holds(task, block),
    })
}

/// **[`answered`], then what follows from it**: the blocks of the tasks it answered are let
/// go; and where nothing was kept because a socket is no longer the one the question drew
/// (#1927), each task still held on that socket is told its block again as it stands now
/// (`tell`), so the question the person reads next names what an answer would keep. The
/// refusal is known by its kind ([`NotAnswered::socket_changed`]).
pub(crate) fn allowed_or_told_again(
    plane: &PlaneId,
    chats: &crate::chats::Chats,
    reading: &Reading<'_>,
    (session, seen, level): (u32, &[SeenBlock], GrantLevel),
    doing: Doing<'_>,
    tell: &dyn Fn(crate::hooks::ChatBlocked),
) -> Result<TasksAnswered, String> {
    match answered(session, seen, level, reading, doing) {
        Ok(done) => {
            let_go(chats, seen, &done.answered);
            Ok(done)
        }
        Err(not) => {
            if not.socket_changed {
                for one in seen.iter().filter(|one| one.what == GrantWhat::Socket) {
                    let was = HeldBlock {
                        operation: one.operation.clone(),
                        kind: one.kind.clone(),
                        what: one.what,
                        target: one.target.clone(),
                    };
                    if chats.blocks().holds(one.task, &was)
                        && let Some(told) =
                            crate::sandboxing::shown_again(plane, chats, one.task, &was)
                    {
                        tell(told);
                    }
                }
            }
            Err(not.said)
        }
    }
}

/// [`allow_sandbox_block_for_tasks`] for the project `plane` on `machine`, with `chats` and
/// `audit` the app's: [`allowed_or_told_again`] with the real judging and keeping.
pub fn allow_for_tasks(
    plane: &PlaneId,
    machine: &purlis_core::sandbox::Machine,
    chats: &crate::chats::Chats,
    asked: (u32, &[SeenBlock], GrantLevel),
    audit: crate::sandboxing::Audit<'_>,
    (at, tell): (u64, &dyn Fn(crate::hooks::ChatBlocked)),
) -> Result<TasksAnswered, String> {
    let root = plane.root();
    let level = asked.2;
    reading(chats, |reading| {
        allowed_or_told_again(
            plane,
            chats,
            reading,
            asked,
            Doing {
                judge: &|task, block| {
                    crate::sandboxing::judged(
                        root,
                        machine,
                        chats.folder_of(task).as_deref(),
                        task,
                        (block.what, &block.target, level),
                    )
                },
                keep: &mut |task, what, level| {
                    crate::sandboxing::kept(root, chats, task, (what, level), audit, at).map(|_| ())
                },
                note: &|task, what, level| {
                    audit(
                        Some(task),
                        &grant::Audited {
                            granted: true,
                            what: what.word(),
                            target: &what.target(),
                            level,
                        },
                    )
                },
                // A host reaches each task whose proxy asks live at once (#1666): only the
                // others are owed the restart that takes it.
                owe: &|task, what, told| {
                    if !(matches!(what, What::Host(_)) && chats.board_of(task).is_some()) {
                        chats.owe_restart(task, told);
                    }
                },
            },
            tell,
        )
    })
}

/// The blocks of `answered` among `seen` are answered: the app holds them no longer.
fn let_go(chats: &crate::chats::Chats, seen: &[SeenBlock], answered: &[u32]) {
    for one in seen.iter().filter(|one| answered.contains(&one.task)) {
        if let Some(target) = normalised(one.what, &one.target) {
            chats.blocks().answered(
                one.task,
                &HeldBlock {
                    operation: one.operation.clone(),
                    kind: one.kind.clone(),
                    what: one.what,
                    target,
                },
            );
        }
    }
}

/// **Allow on the one question for several tasks of chat `session`** (#1508, V100-57): `seen`
/// is each task with the block the question showed for it. The whole answer is refused unless
/// every task is held on that one block now and is a task below `session` by the app's own
/// record. Each is judged on its own, and the answer is kept for each task alone (`chat`), or
/// once for every chat of the project on this machine (`you`) or for everyone in it
/// (`project`), with each task owed a restart. The window then restarts each once its turn has
/// ended (`restart_chat`).
#[tauri::command]
#[specta::specta]
pub async fn allow_sandbox_block_for_tasks(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    session: u32,
    seen: Vec<SeenBlock>,
    level: GrantLevel,
) -> Result<TasksAnswered, String> {
    let held = planes.held(&plane)?;
    let network = planes.network().cloned();
    let tell = planes.block_teller();
    // Off the window's thread (#1949): each task's grant is judged, audited and kept on disk.
    crate::off_the_window("keeping your Allow", move || {
        allow_for_tasks(
            &plane,
            &purlis_core::sandbox::Machine::this(),
            held.chats(),
            (session, &seen, level),
            &held.audit_then_record(network.as_ref()),
            (crate::sandboxing::now_secs(), &|told| tell(told)),
        )
    })
    .await
}

/// **Keep blocked on the one question for several tasks of chat `session`** (#1508): checked
/// as an Allow is ([`checked`]), then each task's block is let go. Nothing is granted.
#[tauri::command]
#[specta::specta]
pub fn keep_sandbox_block_for_tasks(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    session: u32,
    seen: Vec<SeenBlock>,
    reason: Option<String>,
) -> Result<Vec<u32>, String> {
    let held = planes.held(&plane)?;
    let audit = held.audit_then_record(planes.network());
    keep_blocked(held.chats(), session, &seen, reason.as_deref(), &audit)
}

/// [`keep_sandbox_block_for_tasks`] on `chats`.
pub fn keep_blocked(
    chats: &crate::chats::Chats,
    session: u32,
    seen: &[SeenBlock],
    reason: Option<&str>,
    audit: crate::sandboxing::Audit<'_>,
) -> Result<Vec<u32>, String> {
    let (tasks, block) = reading(chats, |reading| checked(session, seen, reading))?;
    let_go(chats, seen, &tasks);
    // What each task's proxy holds on the host is refused now (#1666, #1411), and each task is
    // told in one line, with the reason (#1987).
    if block.what == GrantWhat::Host
        && let Ok(host) = grant::host(&block.target)
    {
        for task in &tasks {
            chats.keep_blocked_live(*task, &host, audit);
        }
    }
    for task in &tasks {
        crate::askoutcome::kept_blocked(chats, *task, (block.what, &block.target), reason);
    }
    Ok(tasks)
}

impl Blocks {
    /// **Every block each open chat is held on now**, by session, lowest first: what the asks
    /// registry lists as sandbox host asks (#1690).
    pub fn every(&self) -> Vec<(u32, HeldBlock)> {
        let held = self.held();
        let mut every: Vec<(u32, HeldBlock)> = held
            .iter()
            .flat_map(|(session, blocks)| blocks.iter().map(|block| (*session, block.clone())))
            .collect();
        every.sort_by_key(|(session, _)| *session);
        every
    }

    /// [`Self::every`], each block with when it was first heard, in seconds since 1970, where
    /// that is kept (#1700). **Never a quiet one** (#1884): a block read back asks nothing of
    /// the person until they take it up.
    pub fn every_since(&self) -> Vec<(u32, HeldBlock, Option<u64>)> {
        let every: Vec<(u32, HeldBlock)> = {
            let quiet = self.quiet();
            self.every()
                .into_iter()
                .filter(|(session, block)| {
                    !quiet.get(session).is_some_and(|mine| mine.contains(block))
                })
                .collect()
        };
        let since = self.since();
        every
            .into_iter()
            .map(|(session, block)| {
                let first = since
                    .get(&session)
                    .and_then(|mine| mine.iter().find(|(one, _)| *one == block))
                    .map(|(_, first)| *first);
                (session, block, first)
            })
            .collect()
    }
}

#[cfg(test)]
#[path = "taskblocks/tests.rs"]
mod tests;

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The board, kept in the session host** (#2194).
//!
//! With chats in the session host, the host keeps each held project's board, and it outlives the
//! window. The window keeps a copy it reads from, so none of its reads waits on the link, and
//! each move the window makes on its copy is told to the host in the order it was made
//! ([`Board::told_to`]). The host makes the same move on its own board ([`Board::make`]), so the
//! two read the same. While no window holds the project, the host moves its board by the lines
//! its hooks write ([`Board::hear`]). A window that holds the project again takes the host's
//! board whole ([`Board::kept`], [`Board::keep`]): every chat's state and Needs you as the host
//! kept them.
//!
//! A board moves the same way from the same moves: nothing on it reads a clock or the machine.
//! Only when a chat last moved ([`Board::moved_at`]) is the process's own count; a board taken
//! whole keeps the host's counts, and this process counts on from the highest.

use std::collections::BTreeMap;
use std::sync::Arc;

use super::{Board, FailedTask, Need, Tracked, Waits};
use crate::hookwire::{Report, Speaker};

/// One move of a chat on the board: what each of [`Board`]'s moving calls is told as, and what
/// [`Board::make`] makes.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case", tag = "move")]
pub enum Move {
    /// [`Board::opened`]: by whether its harness reports its own process.
    Opened {
        reports_its_process: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        conversation: Option<String>,
        /// [`Board::opened_as`]: a grade's chat, never in the queue (#3583).
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        kept_from_the_window: bool,
    },
    /// [`Board::closed`].
    Closed,
    /// [`Board::reported_while`].
    Reported { report: Report, waits: Waits },
    /// [`Board::rested`].
    Rested,
    /// [`Board::own_work_lapsed`].
    OwnWorkLapsed { now: i64, waits: Waits },
    /// [`Board::task_failed`].
    TaskFailed { failed: FailedTask },
    /// [`Board::failures_seen`].
    FailuresSeen,
    /// [`Board::failure_cleared`].
    FailureCleared { id: String },
    /// [`Board::child_heard`].
    ChildHeard { agent: String },
    /// [`Board::ignored`].
    Ignored,
    /// [`Board::answered`].
    Answered,
    /// [`Board::answered_by`].
    AnsweredBy {
        asker: crate::harness::hooked::Asker,
    },
    /// [`Board::tool_said_by`].
    ToolSaidBy {
        speaker: Speaker,
        said: crate::doing::Said,
    },
    /// [`Board::child_tool_said_by`].
    ChildToolSaidBy {
        agent: String,
        speaker: Speaker,
        said: crate::doing::Said,
    },
    /// [`Board::exited`].
    Exited {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        code: Option<i32>,
    },
    /// [`Board::reported_back`].
    ReportedBack { from: String },
    /// [`Board::stopped_below`].
    StoppedBelow { from: String },
    /// [`Board::needs`].
    Needs { need: Need },
    /// [`Board::reported_to_its_asker`].
    ReportedToItsAsker,
    /// [`Board::commit_refused`].
    CommitRefused { what: String },
}

/// What is told each move a board makes, with the chat's number, in the order the board made
/// them: under the hold of whoever holds the board, so it must not reach back into it.
pub type Keeper = Arc<dyn Fn(u32, &Move) + Send + Sync + 'static>;

/// Who a board tells its moves, if anyone.
#[derive(Default, Clone)]
pub(super) struct Told(Option<Keeper>);

impl std::fmt::Debug for Told {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(if self.0.is_some() {
            "Told(kept)"
        } else {
            "Told(none)"
        })
    }
}

/// A whole board, as one process keeps it for another to take ([`Board::kept`]).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Kept {
    chats: BTreeMap<u32, Tracked>,
}

impl Kept {
    /// The chats it holds, by number.
    pub fn chats(&self) -> impl Iterator<Item = u32> + '_ {
        self.chats.keys().copied()
    }
}

impl Board {
    /// Tell `keeper` every move this board makes from now on, in the order it makes them
    /// (#2194): what keeps the session host's board the same as this one.
    pub fn told_to(&mut self, keeper: Keeper) {
        self.told = Told(Some(keeper));
    }

    /// Tells whoever keeps this board of the move `moved` makes of chat `number`.
    pub(super) fn tell(&self, number: u32, moved: impl FnOnce() -> Move) {
        if let Some(keeper) = &self.told.0 {
            keeper(number, &moved());
        }
    }

    /// Makes `moved` of chat `number`, as the call it names would: whether anything a reader
    /// can see changed.
    pub fn make(&mut self, number: u32, moved: Move) -> bool {
        match moved {
            Move::Opened {
                reports_its_process,
                conversation,
                kept_from_the_window,
            } => {
                self.opened_reporting(
                    number,
                    reports_its_process,
                    conversation,
                    kept_from_the_window,
                );
                true
            }
            Move::Closed => {
                self.closed(number);
                true
            }
            // A report names its own chat: the move's number is the one it was told under.
            Move::Reported { report, waits } => {
                number == report.chat && self.reported_while(&report, waits)
            }
            Move::Rested => self.rested(number),
            Move::OwnWorkLapsed { now, waits } => self.own_work_lapsed(number, now, waits),
            Move::TaskFailed { failed } => self.task_failed(number, failed),
            Move::FailuresSeen => self.failures_seen(number),
            Move::FailureCleared { id } => self.failure_cleared(number, &id),
            Move::ChildHeard { agent } => self.child_heard(number, &agent),
            Move::Ignored => self.ignored(number),
            Move::Answered => self.answered(number),
            Move::AnsweredBy { asker } => self.answered_by(number, &asker),
            Move::ToolSaidBy { speaker, said } => self.tool_said_by(number, &speaker, &said),
            Move::ChildToolSaidBy {
                agent,
                speaker,
                said,
            } => self.child_tool_said_by(number, &agent, &speaker, &said),
            Move::Exited { code } => self.exited(number, code),
            Move::ReportedBack { from } => self.reported_back(number, &from),
            Move::StoppedBelow { from } => self.stopped_below(number, &from),
            Move::Needs { need } => self.needs(number, need),
            Move::ReportedToItsAsker => {
                self.reported_to_its_asker(number);
                false
            }
            Move::CommitRefused { what } => self.commit_refused(number, &what),
        }
    }

    /// The whole board, for another process to take.
    pub fn kept(&self) -> Kept {
        Kept {
            chats: self.chats.clone(),
        }
    }

    /// Takes `kept` as this board, whole, in place of what it held: every chat as it was
    /// kept, when it last moved included. This process's count of moves goes on from the
    /// highest it took, so a move made here is later than every one kept.
    pub fn keep(&mut self, kept: Kept) {
        let highest = kept
            .chats
            .values()
            .map(|tracked| tracked.moved_at)
            .max()
            .unwrap_or(0);
        super::MOVES.fetch_max(highest, std::sync::atomic::Ordering::Relaxed);
        self.chats = kept.chats;
    }

    /// What line `heard`, which a held project's hooks wrote, moves on this board where no
    /// window is there to decide it: what the window's own hearing moves for it, with nothing
    /// said of what a chat waits on, so the end of a turn is the person's. Whether anything a
    /// reader can see changed.
    pub fn hear(&mut self, heard: &crate::hookwire::relay::Heard) -> bool {
        use crate::hookwire::relay::Heard;
        match heard {
            Heard::Report(report) => self.reported_while(report, Waits::default()),
            Heard::Refused(refused) => self.commit_refused(refused.chat, &refused.commit_refused),
            Heard::Tool(call) => {
                let mut changed = false;
                if call.refused_a_root_update() {
                    changed |= self.needs(call.chat, Need::RootUpdateRefused);
                }
                if let Some(agent) = &call.agent {
                    changed |= self.child_heard(call.chat, agent);
                }
                changed
            }
            Heard::Doing(doing) => {
                if !doing.doing.goes_on_past_a_prompt() && !doing.doing.starts_a_tool_of_its_own() {
                    return false;
                }
                match &doing.agent {
                    Some(agent) => {
                        self.child_tool_said_by(doing.chat, agent, &doing.speaker, &doing.doing)
                    }
                    None => self.tool_said_by(doing.chat, &doing.speaker, &doing.doing),
                }
            }
            _ => false,
        }
    }

    /// The chats on the board, by number.
    pub fn chats(&self) -> impl Iterator<Item = u32> + '_ {
        self.chats.keys().copied()
    }
}

#[cfg(test)]
mod tests;

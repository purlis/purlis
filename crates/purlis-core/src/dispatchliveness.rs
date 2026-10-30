// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Whether a task is alive**, as its asking chat is told (#3900, from #3897 concern 2).
//!
//! After a restart, a task listed as `reported` or `started before this app did` said nothing
//! about whether it still worked. A coordinator took a live task for a dead one, dispatched a
//! resume chat, and the two wrote into one branch folder. So every task a chat lists
//! (`purlis dispatch list`, the `dispatch_list` tool) and every task its turn's roster names
//! ([`crate::awareness`]) says three facts the app holds, and no guess:
//!
//! - **whether its program is running**: the app's board, not the task's word;
//! - **when it last ran a command or sent a note**, as this run of the app heard it
//!   ([`Heard`]). In memory, so a task brought back says nothing was heard since the app
//!   started until it does something;
//! - **how it stands across the app's restart** ([`Restart`]): started by this run, kept
//!   running in the session host and reattached, or ended by the restart and started again on
//!   its conversation. A task whose dispatch still runs that this app can no longer reach (its
//!   start was refused at the launch, or nothing brought it back) is **orphaned by restart**,
//!   said in those words.
//!
//! Plain Rust with no clock: the app hands it every instant.

use std::time::{Duration, SystemTime};

/// What a task was last heard doing (#3900): only the two things the ticket names, a command
/// it ran and a note it sent its asking chat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Did {
    RanACommand,
    SentANote,
}

impl Did {
    fn said(self) -> &'static str {
        match self {
            Self::RanACommand => "ran a command",
            Self::SentANote => "sent a note",
        }
    }
}

/// How a task stands across the app's last restart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Restart {
    /// Its program ran on through the restart in the session host, and this app reattached it.
    KeptRunning,
    /// The restart ended its program, and this app started it again on its conversation.
    StartedAgain,
    /// Its dispatch still runs, and this app cannot reach it: no program of it runs here.
    Orphaned,
    /// This run of the app started it: there was no restart in between. A word a newer app
    /// says that this one does not know reads as this one (last, as serde asks).
    #[default]
    #[serde(other)]
    StartedHere,
}

/// When a task last ran a command or sent a note, as this run of the app heard it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Heard {
    last: Option<(SystemTime, Did)>,
}

impl Heard {
    /// It did `did` at `at`. An earlier instant than the one kept is not kept over it.
    pub fn did(&mut self, did: Did, at: SystemTime) {
        if self.last.is_none_or(|(kept, _)| at >= kept) {
            self.last = Some((at, did));
        }
    }

    /// Its liveness at `now`: `running` is the app's board, `restart` the app's record.
    pub fn liveness(&self, running: bool, restart: Restart, now: SystemTime) -> Liveness {
        Liveness {
            running,
            heard_secs: self
                .last
                .map(|(at, _)| now.duration_since(at).unwrap_or(Duration::ZERO).as_secs()),
            did: self.last.map(|(_, did)| did),
            restart,
        }
    }
}

/// A task's liveness, on a listed row and in its asking chat's roster.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct Liveness {
    /// Its program is running now, by the app's board.
    pub running: bool,
    /// How long ago it last ran a command or sent a note, in seconds; none where this run of
    /// the app heard neither.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heard_secs: Option<u64>,
    /// Which of the two that was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub did: Option<Did>,
    #[serde(default)]
    pub restart: Restart,
}

/// The words a task this app can no longer reach is marked with, explicitly (#3900).
pub const ORPHANED_BY_RESTART: &str = "orphaned by restart";

impl Liveness {
    /// A task whose dispatch still runs that this app cannot reach.
    pub fn orphaned() -> Self {
        Self {
            running: false,
            heard_secs: None,
            did: None,
            restart: Restart::Orphaned,
        }
    }

    pub fn is_orphaned(&self) -> bool {
        self.restart == Restart::Orphaned
    }

    /// The liveness in one clause, as a list row and a refusal say it.
    pub fn said(&self) -> String {
        if self.is_orphaned() {
            return format!(
                "{ORPHANED_BY_RESTART}: purlis can no longer reach it, and no program of it \
                 runs under this app"
            );
        }
        let mut said = vec![
            if self.running {
                "its program is running"
            } else {
                "its program is not running"
            }
            .to_owned(),
        ];
        said.push(match (self.heard_secs, self.did) {
            (Some(secs), Some(did)) => format!(
                "last {} {} ago",
                did.said(),
                crate::dispatched::age_word(Duration::from_secs(secs))
            ),
            _ => "no command or note heard from it since this app started".to_owned(),
        });
        match self.restart {
            Restart::KeptRunning => said.push("it kept running across purlis's restart".into()),
            Restart::StartedAgain => said
                .push("purlis's restart ended it and started it again on its conversation".into()),
            Restart::Orphaned | Restart::StartedHere => {}
        }
        said.join(", ")
    }
}

#[cfg(test)]
mod tests;

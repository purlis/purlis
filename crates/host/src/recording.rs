// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The event log, and what is kept as a chat's program ends, written in the session host
//! (#2003): the binary's, built on the core, as [`crate::Compose`] is, so this crate links none
//! of it.
//!
//! **The host is the event log's one writer while chats run in it** (ADR 0066, ADR 0068). What
//! the window decides (the run a start begins, a person's grant, a kill switch line) reaches the
//! host as `record` ([`Record::record`]) and is written there; what a held project's hooks say
//! is recorded by the binary's hook channel ([`crate::Hear`]), on the thread that heard it.
//!
//! **A program's end is kept here, window or no window** ([`Record::ended`]). The host is the
//! first to know a program ended and why: whether it ended by itself, or by the host's own
//! act (the kill switch, a quit, a stop, a close). It tells the binary at once, which records
//! the end of the chat's run in the log, drains what its hooks spooled, and, where no window
//! holds the project to decide it, ends the dispatch the chat worked as one that went without
//! its report.

use std::path::Path;

use purlis_session_protocol::session::{Exit, Pushed};

use crate::hostrecord::Started;

/// The host's own act that ended a program, where one did: the highest of them where several
/// did, in this order (ADR 0076 §2, D-1084-1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Stopped {
    /// A close forgot it, and ended it.
    Closed,
    /// A stop ended it, and kept it.
    Operator,
    /// The host ended every program as it stopped: the window quit.
    Quit,
    /// The kill switch.
    Killed,
}

/// A program the host ran has ended.
pub struct Ended<'a> {
    /// Where the project it ran in is on this machine.
    pub project_path: &'a Path,
    /// Its number in the project.
    pub number: u32,
    /// How it ended.
    pub exit: Exit,
    /// The host's own act that ended it, or `None` for a program that ended by itself.
    pub stopped: Option<Stopped>,
    /// Whether a window held the project as it ended, and so decides what its end moves.
    pub watched: bool,
}

/// Writes the event log, and keeps what a program's end leaves: the binary's (#2003).
pub trait Record: Send + Sync + 'static {
    /// Writes `entry`, one write the window decided, in the event log, durably: or why it is
    /// not, in a sentence.
    fn record(&self, entry: serde_json::Value) -> Result<(), String>;

    /// A program the host ran has ended ([`Ended`]): called once, as it ends, off the host's
    /// runtime.
    fn ended(&self, ended: &Ended<'_>);

    /// **A program the last host ran was left running when that host ended without ending it**
    /// (#2007): it was killed outright, or crashed. Its program is gone with that host's
    /// terminal, or has just been ended ([`crate::Run::end_what_was_left`]), and what its end
    /// leaves is kept here, as a crash's (ADR 0076: `host-crash`). Called once for each, before
    /// this host serves anyone, in the order they started. Nothing, for a binary that keeps no
    /// event log.
    fn left_by_its_last_host(&self, left: &Started) {
        let _ = left;
    }

    /// **Where this machine's event log ends now** (1.41, #3393): its device and the `seq` of
    /// its last line, 0 for none; or why there is no log to say it of.
    fn log_end(&self) -> Result<(String, u64), String> {
        Err(NO_LOG.into())
    }

    /// **A reader of this machine's event log from `after`** (1.41, #3393): each of its lines
    /// after that `seq`, then each as it is written, as the session protocol pushes them. Called
    /// off the host's runtime.
    fn read_log(&self, after: u64) -> Result<Box<dyn ReadsLog>, String> {
        let _ = after;
        Err(NO_LOG.into())
    }
}

/// What a host whose binary keeps no event log says when it is asked for its log.
const NO_LOG: &str = "this session host keeps no event log";

/// **One reader of the host's own event log, from its cursor** (1.41, #3393): the binary's.
pub trait ReadsLog: Send + 'static {
    /// What was written since the last read, in order, each line once: each as
    /// [`Pushed::Event`], and a gap as [`Pushed::Missed`]. Never blocks on what is not written
    /// yet: nothing new is an empty answer.
    fn read(&mut self) -> Result<Vec<Pushed>, String>;
}

/// A host whose binary keeps no event log: every `record` is refused with a sentence, and a
/// program's end is kept nowhere.
pub struct RecordsNothing;

impl Record for RecordsNothing {
    fn record(&self, _entry: serde_json::Value) -> Result<(), String> {
        Err("this session host keeps no event log, so nothing was recorded".into())
    }

    fn ended(&self, _ended: &Ended<'_>) {}
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What a chat's end leaves, kept in a session host's event log** (#2003, #2494): the host's
//! [`purlis_host::Record`], one for both binaries' hosts, the window's and `purlis serve`, as
//! their hook channels are one ([`super::HookChannels`]).
//!
//! **A write the window decided** (`record`) is read as an [`Entry`] and written durably.
//!
//! **A program's end is kept here, window or no window.** What its hooks spooled is recorded,
//! and its sub-agents' runs end with its own, in the end state and for the cause the host's own
//! act or its exit gives (ADR 0076 §2, §6). Where no window held its project as it ended by
//! itself, the dispatch it worked ends as one that went without its report, kept for its asking
//! chat ([`crate::dispatchrecord::ended_unwatched`]); a window that holds the project decides
//! that itself, from the board and the asking chat it holds (D-2000-1, #2194).
//!
//! **A chat the last host left running** (#2007) is kept as a crash's: what its hooks spooled is
//! recorded now, and its sub-agents' runs end `failed` for `host-crash` (ADR 0076 §5, §6).

use std::path::Path;
use std::sync::{Mutex, PoisonError};

use purlis_session_protocol::session;

use crate::eventlog::{Entry, Recorder};
use crate::state::run::{Cause, Exit, RunState, StopBy};

use super::{EventLog, drain_while_running, socket_for};

impl purlis_host::Record for EventLog {
    fn record(&self, entry: serde_json::Value) -> Result<(), String> {
        let entry: Entry = serde_json::from_value(entry).map_err(|why| {
            format!("the window asked for a write this host does not read: {why}")
        })?;
        self.write(entry).map_err(|why| why.to_string())
    }

    fn ended(&self, ended: &purlis_host::Ended<'_>) {
        let Some(log) = self.recorder() else { return };
        let (root, number) = (ended.project_path, ended.number);
        what_it_spooled_is_recorded(&log, root, number);
        let exit = match &ended.exit {
            session::Exit::Code(code) => crate::session::Exit::Code(*code),
            session::Exit::Signal(signal) => crate::session::Exit::Signal(signal.clone()),
        };
        let (state, cause) = how_its_run_ended(ended.stopped.map(stop_by), &exit);
        its_children_end(&log, root, number, state, cause);
        if ended.watched || ended.stopped.is_some() {
            return;
        }
        let chat = log
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .chat_of(root, number);
        if let Some(chat) = chat {
            crate::dispatchrecord::ended_unwatched(root, &chat, number, chrono::Utc::now());
        }
    }

    fn left_by_its_last_host(&self, left: &purlis_host::Started) {
        let Some(log) = self.recorder() else { return };
        let root = left.project_path.as_path();
        what_it_spooled_is_recorded(&log, root, left.number);
        its_children_end(&log, root, left.number, RunState::Failed, Cause::HostCrash);
    }
}

/// What chat `number` of the project at `root` spooled, beside its project's hook socket, is
/// recorded in `log`.
fn what_it_spooled_is_recorded(log: &Mutex<Recorder>, root: &Path, number: u32) {
    let socket = socket_for(Some(root)).socket;
    drain_while_running(&crate::hookwire::spool::dir_for(&socket), root, log, number);
}

/// The host's own act that ended a program, as a run's end names it (ADR 0076 §2).
fn stop_by(stopped: purlis_host::Stopped) -> StopBy {
    match stopped {
        purlis_host::Stopped::Closed => StopBy::Closed,
        purlis_host::Stopped::Operator => StopBy::Operator,
        purlis_host::Stopped::Quit => StopBy::Quit,
        purlis_host::Stopped::Killed => StopBy::Killed,
    }
}

/// **The end state and cause of a chat's run whose program ended with `exit`**, stopped by the
/// host for `by` where it was (ADR 0076 §2): a stop the host caused is `stopped` whatever the
/// code says; otherwise code 0 is `completed` and anything else, a signal included, `failed`,
/// for `exited`.
pub fn how_its_run_ended(by: Option<StopBy>, exit: &crate::session::Exit) -> (RunState, Cause) {
    if let Some(by) = by {
        return (RunState::Stopped, Cause::Stop(by));
    }
    let exit = match exit {
        crate::session::Exit::Code(code) => i32::try_from(*code).map_or(Exit::Signal, Exit::Code),
        // A signal leaves no code behind, and it is not a clean end.
        crate::session::Exit::Signal(_) => Exit::Signal,
    };
    let state = if exit == Exit::Code(0) {
        RunState::Completed
    } else {
        RunState::Failed
    };
    (state, Cause::Exited(exit))
}

/// **Every sub-agent of chat `session` still live ends with its run** (ADR 0076 §6, #1084): a
/// `run.ended` under each child run, in `state` for `cause`, written under the event log's
/// lock. Nothing where none is live. A write that fails is warned of, and never holds up the
/// rest of the chat's end.
pub fn its_children_end(
    events: &Mutex<Recorder>,
    plane: &Path,
    session: u32,
    state: RunState,
    cause: Cause,
) {
    let mut log = events.lock().unwrap_or_else(PoisonError::into_inner);
    if let Err(why) = log.end_children_with(plane, session, state, cause) {
        tracing::warn!("purlis: a sub-agent's end was not written to the event log ({why})");
    }
}

#[cfg(test)]
mod tests;

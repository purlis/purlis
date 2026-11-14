// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A runner's event log as the window reads it over the link** (#3393, ADR 0078 §7: *"the
//! desktop subscribes with a cursor per device. The runner's host is the only writer of the
//! runner's event log. The desktop never copies the runner's events into its own"*).
//!
//! Each link the window's host relays to a pool's runner asks where the runner's log ends
//! (`log_end`, 1.41) and subscribes from there with a cursor for the runner's device, so it is
//! pushed what the runner writes from then on, and again from its cursor after it fell behind or
//! the link came back: no line twice, none missed in silence. A runner that said no end as the
//! link came up is asked again once it is back.
//!
//! **What is held is only what the window draws, and small** (#3472 widens it): of each line,
//! a top-level run's end, its chat's id, how it ended and when, each field held to a few bytes,
//! and no other line at all; at most [`MOST_LINES`] of them and [`MOST_BYTES`] in all, the oldest
//! let go first. A runner's line is the runner's own word, so nothing of it is kept that the
//! window does not read, and nothing of it whole. **It is held in this window's memory alone**,
//! never written into this machine's log or anywhere on its disk: a window started again reads
//! on from the log's end then. What the window draws from it is [`RunnerLogNow`], with whether
//! the link is down, so a runner it cannot reach is said so, never shown as one where nothing
//! happened.

use std::collections::VecDeque;

use purlis_session_protocol::session::{Event, Since};

/// The most run ends one link holds.
pub const MOST_LINES: usize = 1024;

/// The most bytes the run ends one link holds come to, its fields' lengths counted.
pub const MOST_BYTES: usize = 128 * 1024;

/// The longest chat id, device id or event id held: a ULID is 26 characters.
const MOST_ID: usize = 32;

/// How a run ended, as the runner's log says it: any other word is not held.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum RunEnded {
    Completed,
    Failed,
    Stopped,
}

impl RunEnded {
    fn of(word: &str) -> Option<RunEnded> {
        match word {
            "completed" => Some(RunEnded::Completed),
            "failed" => Some(RunEnded::Failed),
            "stopped" => Some(RunEnded::Stopped),
            _ => None,
        }
    }
}

/// One run of a chat that ended on the runner, as the window holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunEndLine {
    /// The chat's id (ADR 0066).
    pub chat: String,
    pub ended: RunEnded,
    /// The event's ULID, whose time part says when it was written.
    pub ulid: String,
    /// The device that wrote it.
    pub device: String,
}

impl RunEndLine {
    /// A line of the runner's log as the window holds it: only a top-level run's end, with
    /// every field short enough, and a known word for how it ended.
    fn of(event: &Event) -> Option<RunEndLine> {
        if event.kind != "run.ended" || event.parent_run.is_some() {
            return None;
        }
        // A run replaced at once is no chat finishing (ADR 0076 §2, Q247): the chat goes on.
        if event.body.get("cause").and_then(serde_json::Value::as_str) == Some("superseded") {
            return None;
        }
        let short =
            |text: &str| (!text.is_empty() && text.len() <= MOST_ID).then(|| text.to_owned());
        Some(RunEndLine {
            chat: short(event.chat.as_deref()?)?,
            ended: RunEnded::of(event.body.get("state")?.as_str()?)?,
            ulid: short(&event.ulid)?,
            device: short(&event.device_id)?,
        })
    }

    fn bytes(&self) -> usize {
        self.chat.len() + self.ulid.len() + self.device.len() + 1
    }
}

/// What one link holds of its runner's log.
#[derive(Debug, Default)]
pub struct RunnerLog {
    /// The runner's device, once its host said where its log ends: none for a runner before
    /// 1.41, which says none of its log.
    device: Option<String>,
    /// The last `seq` read of it.
    cursor: u64,
    /// Where its log ended as the link came up: only a run that ended after it is drawn, though
    /// the link reads on from where this window last logged its project's skill runs (#4219).
    began_at: u64,
    lines: VecDeque<RunEndLine>,
    bytes: usize,
    /// Whether some of its lines were not kept for the window to read (#940's retention, or
    /// more than the window holds came).
    missed: bool,
    /// Whether the link to the runner is down now.
    down: bool,
}

/// A runner's log as the window draws it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RunnerLogNow {
    /// The runner's device, where its host said it: none, and no line of its log is read.
    pub device: Option<String>,
    /// The run ends held, oldest first.
    pub lines: Vec<RunEndLine>,
    /// Whether some lines between them were not kept.
    pub missed: bool,
    /// Whether the link to the runner is down now.
    pub down: bool,
}

impl RunnerLogNow {
    /// **Two runners' logs of one pool, read as the pool's** (#4271): the ends of both, oldest
    /// first; what either missed, or a link either has down, is said of the pool.
    pub fn beside(mut self, other: RunnerLogNow) -> RunnerLogNow {
        self.device = self.device.or(other.device);
        self.lines.extend(other.lines);
        // A ULID's order is its time's.
        self.lines.sort_by(|one, next| one.ulid.cmp(&next.ulid));
        self.missed |= other.missed;
        self.down |= other.down;
        self
    }
}

impl RunnerLog {
    /// The runner's host said its log is `device`'s and ends at `seq`: read on from `from`, where
    /// this window last logged a project's skill runs of it (#4219), else from there. A `from`
    /// past the end is a log begun again, read on from its end.
    pub fn starts_at(&mut self, device: String, seq: u64, from: Option<u64>) {
        if device.is_empty() || device.len() > MOST_ID {
            return;
        }
        self.device = Some(device);
        self.cursor = from.filter(|from| *from <= seq).unwrap_or(seq);
        self.began_at = seq;
    }

    /// Where the runner's log ended as the link came up.
    pub fn began_at(&self) -> u64 {
        self.began_at
    }

    /// Whether the runner's host has said where its log ends.
    pub fn started(&self) -> bool {
        self.device.is_some()
    }

    /// What a subscription to the runner asks with: the cursor for its device, where it said
    /// which.
    pub fn since(&self) -> Since {
        self.device
            .iter()
            .map(|device| (device.clone(), self.cursor))
            .collect()
    }

    /// A line of the runner's log came. One of another device, or one held already, is not
    /// read; one the window does not draw moves the cursor and is not kept. Answers whether it
    /// was read, so each line the runner wrote is read once (a skill run's is logged, #3084).
    pub fn heard(&mut self, event: &Event) -> bool {
        if self.device.as_deref() != Some(event.device_id.as_str()) || event.seq <= self.cursor {
            return false;
        }
        self.cursor = event.seq;
        let Some(line) = RunEndLine::of(event).filter(|_| event.seq > self.began_at) else {
            return true;
        };
        self.bytes += line.bytes();
        self.lines.push_back(line);
        while self.lines.len() > MOST_LINES || self.bytes > MOST_BYTES {
            if let Some(first) = self.lines.pop_front() {
                self.bytes -= first.bytes();
            }
            self.missed = true;
        }
        true
    }

    /// The runner's lines after `after` up to `resumes_at` are not there to read.
    pub fn missed(&mut self, device: &str, resumes_at: u64) {
        if self.device.as_deref() != Some(device) {
            return;
        }
        self.missed = true;
        self.cursor = self.cursor.max(resumes_at.saturating_sub(1));
    }

    /// Some of the runner's lines were let go of before anything read them (#4219): said, as a
    /// gap in its log is.
    pub fn let_go(&mut self) {
        self.missed = true;
    }

    /// The link to the runner went down, or came back.
    pub fn link(&mut self, down: bool) {
        self.down = down;
    }

    /// Whether the link to the runner is down, as far as it was told.
    pub fn link_is_down(&self) -> bool {
        self.down
    }

    pub fn now(&self) -> RunnerLogNow {
        RunnerLogNow {
            device: self.device.clone(),
            lines: self.lines.iter().cloned().collect(),
            missed: self.missed,
            down: self.down,
        }
    }
}

#[cfg(test)]
mod tests;

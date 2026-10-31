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
    /// The last `seq` held of it.
    cursor: u64,
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
    /// The runner's host said its log is `device`'s and ends at `seq`: read on from there.
    pub fn starts_at(&mut self, device: String, seq: u64) {
        if device.is_empty() || device.len() > MOST_ID {
            return;
        }
        self.device = Some(device);
        self.cursor = seq;
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
        let Some(line) = RunEndLine::of(event) else {
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

    /// The link to the runner went down, or came back.
    pub fn link(&mut self, down: bool) {
        self.down = down;
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
mod tests {
    use super::*;

    /// **Two runners' logs are one pool's** (#4271): both's ends oldest first, and what either
    /// missed or has down is said.
    #[test]
    fn a_pools_two_runners_read_as_one() {
        let line = |ulid: &str| RunEndLine {
            chat: "chat".into(),
            ended: RunEnded::Completed,
            ulid: ulid.into(),
            device: "device".into(),
        };
        let now = RunnerLogNow {
            device: Some("new".into()),
            lines: vec![line("02")],
            missed: false,
            down: false,
        };
        let draining = RunnerLogNow {
            device: Some("old".into()),
            lines: vec![line("01"), line("03")],
            missed: true,
            down: true,
        };

        let joined = now.beside(draining);

        let ulids: Vec<&str> = joined.lines.iter().map(|line| line.ulid.as_str()).collect();
        assert_eq!(ulids, ["01", "02", "03"]);
        assert_eq!(joined.device.as_deref(), Some("new"));
        assert!(joined.missed && joined.down);
    }

    const RUNNER: &str = "01JB0RUNNER0000000000000000";
    const CHAT: &str = "01K6W0RKER000000000000000B";

    fn line(device: &str, seq: u64) -> Event {
        Event {
            v: 1,
            device_id: device.into(),
            seq,
            ulid: format!("01JB{seq:022}"),
            chat: Some(CHAT.into()),
            run: None,
            parent_run: None,
            kind: "run.ended".into(),
            body: serde_json::json!({ "state": "completed" }),
        }
    }

    fn seqs(log: &RunnerLog) -> Vec<String> {
        log.now()
            .lines
            .iter()
            .map(|line| line.ulid.clone())
            .collect()
    }

    #[test]
    fn a_runner_that_said_no_device_is_asked_for_none_of_its_log_and_keeps_none() {
        let mut log = RunnerLog::default();
        assert!(log.since().is_empty());
        assert!(!log.started());
        log.heard(&line(RUNNER, 1));
        assert!(log.now().lines.is_empty());
    }

    #[test]
    fn lines_are_kept_after_the_cursor_once_each_and_the_cursor_follows_them() {
        let mut log = RunnerLog::default();
        log.starts_at(RUNNER.into(), 4);
        let read: Vec<bool> = [3, 5, 5, 6]
            .into_iter()
            .map(|seq| log.heard(&line(RUNNER, seq)))
            .collect();
        assert_eq!(read, [false, true, false, true], "each line read once");
        assert!(!log.heard(&line("01JA0DESKT0P00000000000000", 7)));
        assert_eq!(seqs(&log), vec![line(RUNNER, 5).ulid, line(RUNNER, 6).ulid]);
        assert_eq!(log.since(), Since::from([(RUNNER.to_owned(), 6)]));
    }

    // Seen red: with every kind kept, the hook line and the sub-agent's end are held too.
    #[test]
    fn only_a_top_level_runs_end_of_a_known_word_is_kept_and_the_rest_moves_the_cursor() {
        let mut log = RunnerLog::default();
        log.starts_at(RUNNER.into(), 0);
        let mut hook = line(RUNNER, 1);
        hook.kind = "hook.stop".into();
        let mut child = line(RUNNER, 2);
        child.parent_run = Some("01K6PARENT0000000000000000".into());
        let mut odd = line(RUNNER, 3);
        odd.body = serde_json::json!({ "state": "completed. All checks passed" });
        let mut long = line(RUNNER, 4);
        long.chat = Some("x".repeat(MOST_ID + 1));
        let mut failed = line(RUNNER, 5);
        failed.body = serde_json::json!({ "state": "failed", "padding": "x".repeat(100_000) });
        for event in [hook, child, odd, long, failed] {
            log.heard(&event);
        }
        let now = log.now();
        assert_eq!(now.lines.len(), 1);
        assert_eq!(now.lines[0].ended, RunEnded::Failed);
        assert_eq!(log.since(), Since::from([(RUNNER.to_owned(), 5)]));
    }

    #[test]
    fn a_gap_is_said_and_read_on_from_where_the_log_resumes() {
        let mut log = RunnerLog::default();
        log.starts_at(RUNNER.into(), 2);
        log.missed(RUNNER, 10);
        assert!(log.now().missed);
        assert_eq!(log.since(), Since::from([(RUNNER.to_owned(), 9)]));
    }

    // Seen red: with the bounds' loop taken out, every line is held.
    #[test]
    fn it_holds_the_last_lines_alone_within_its_bounds_and_says_it_let_the_first_go() {
        let mut log = RunnerLog::default();
        log.starts_at(RUNNER.into(), 0);
        for seq in 1..=(MOST_LINES as u64 + 2) {
            log.heard(&line(RUNNER, seq));
        }
        let now = log.now();
        assert_eq!(now.lines.len(), MOST_LINES);
        assert_eq!(now.lines[0].ulid, line(RUNNER, 3).ulid);
        assert!(now.missed);
        assert!(log.bytes <= MOST_BYTES);
    }

    #[test]
    fn a_link_that_is_down_is_said_so() {
        let mut log = RunnerLog::default();
        log.link(true);
        assert!(log.now().down);
        log.link(false);
        assert!(!log.now().down);
    }
}

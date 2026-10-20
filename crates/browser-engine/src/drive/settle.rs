// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! When a page has settled: no frame of it is loading, no request it made is still open, and
//! nothing of that kind has happened for a quiet while.
//!
//! What counts is read from the engine's own events (`Page.frameStartedLoading` and
//! `frameStoppedLoading`, `Network.requestWillBeSent`, `loadingFinished` and `loadingFailed`).
//! A stream that is meant never to end (a WebSocket, an event source) does not count. A page
//! that has not settled says what it still waits for, in words.
//!
//! **A frame's document may end in another session than it started in** (#2689). The document
//! of a frame from another site is asked for in the session of the page around it, and answered
//! in the frame's own, which the engine gives the frame as it moves it to a process of its own:
//! it is told ended there, under the same id. A document's id is its load's own token, which no
//! other process's requests share, so a document ended in any session of the page ends;
//! another request's id is numbered per process, so it ends only in its own session.

use std::collections::{BTreeMap, HashSet};
use std::time::{Duration, Instant};

use serde_json::Value;

/// One event's part in settling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Activity {
    FrameStarted(String),
    FrameStopped(String),
    FrameGone(String),
    RequestStarted {
        id: String,
        url: String,
        /// Whether it asks for a frame's document.
        document: bool,
    },
    RequestEnded(String),
    /// Something that is not a load but resets the quiet while (a page opening, say).
    Changed,
}

/// Request types that are meant to stay open.
const NEVER_ENDING: &[&str] = &["WebSocket", "EventSource"];

/// How many open requests a reason names.
const NAMED: usize = 3;

/// What `method` with `params` means for settling, if anything.
pub(crate) fn activity(method: &str, params: &Value) -> Option<Activity> {
    let text = |field: &str| params.get(field).and_then(Value::as_str).map(str::to_owned);
    match method {
        "Page.frameStartedLoading" => text("frameId").map(Activity::FrameStarted),
        "Page.frameStoppedLoading" => text("frameId").map(Activity::FrameStopped),
        "Page.frameDetached" => text("frameId").map(Activity::FrameGone),
        "Network.requestWillBeSent" => {
            let kind = params.get("type").and_then(Value::as_str).unwrap_or("");
            let url = params
                .get("request")
                .and_then(|r| r.get("url"))
                .and_then(Value::as_str)
                .unwrap_or("");
            if NEVER_ENDING.contains(&kind) || url.starts_with("data:") {
                return Some(Activity::Changed);
            }
            text("requestId").map(|id| Activity::RequestStarted {
                id,
                url: url.to_owned(),
                document: kind == "Document",
            })
        }
        "Network.loadingFinished" | "Network.loadingFailed" => {
            text("requestId").map(Activity::RequestEnded)
        }
        "Page.frameNavigated" | "Page.navigatedWithinDocument" | "Target.targetCreated" => {
            Some(Activity::Changed)
        }
        _ => None,
    }
}

/// One page's loads, as its events tell them.
#[derive(Debug, Clone)]
pub(crate) struct Settle {
    loading: HashSet<String>,
    /// Open requests by (session, request id), with their addresses and whether each asks for
    /// a frame's document.
    requests: BTreeMap<(String, String), (String, bool)>,
    last_change: Instant,
}

impl Settle {
    /// A page seen first at `now`, with nothing loading yet.
    pub(crate) fn new(now: Instant) -> Self {
        Self {
            loading: HashSet::new(),
            requests: BTreeMap::new(),
            last_change: now,
        }
    }

    /// Takes in what one event from `session` meant, at `now`.
    pub(crate) fn observe(&mut self, session: &str, seen: Activity, now: Instant) {
        self.last_change = now;
        match seen {
            Activity::FrameStarted(frame) => {
                self.loading.insert(frame);
            }
            Activity::FrameStopped(frame) | Activity::FrameGone(frame) => {
                self.loading.remove(&frame);
            }
            Activity::RequestStarted { id, url, document } => {
                self.requests
                    .insert((session.to_owned(), id), (url, document));
            }
            Activity::RequestEnded(id) => {
                if self
                    .requests
                    .remove(&(session.to_owned(), id.clone()))
                    .is_none()
                {
                    // A frame's document, answered in the frame's own session.
                    self.requests
                        .retain(|(_, open), (_, document)| !(*document && *open == id));
                }
            }
            Activity::Changed => {}
        }
    }

    /// Forgets what `session` had open: its frame went away with it.
    pub(crate) fn forget_session(&mut self, session: &str, now: Instant) {
        self.requests.retain(|(s, _), _| s != session);
        self.last_change = now;
    }

    /// What the page still waits for at `now`, in words; empty once it has settled.
    pub(crate) fn waiting_for(&self, now: Instant, quiet: Duration) -> Vec<String> {
        let mut out = Vec::new();
        if !self.loading.is_empty() {
            let n = self.loading.len();
            out.push(format!(
                "{n} frame{} still loading",
                if n == 1 { "" } else { "s" }
            ));
        }
        if !self.requests.is_empty() {
            let n = self.requests.len();
            let named: Vec<&str> = self
                .requests
                .values()
                .take(NAMED)
                .map(|(url, _)| url.as_str())
                .collect();
            let more = if n > NAMED {
                format!(" and {} more", n - NAMED)
            } else {
                String::new()
            };
            out.push(format!(
                "{n} request{} still loading: {}{more}",
                if n == 1 { "" } else { "s" },
                named.join(", ")
            ));
        }
        let since = now.saturating_duration_since(self.last_change);
        if out.is_empty() && since < quiet {
            out.push(format!("the page changed {} ms ago", since.as_millis()));
        }
        out
    }
}

#[cfg(test)]
mod tests;

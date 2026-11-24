// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The proxy's record of what each chat asked and what it was told.
//!
//! A record holds the method, the host, the repo (when the request named an allowed one), the
//! **class** of the request and the verdict. It holds no path, no query, no header and no body: a
//! path may carry a token in its query, and the class says what the path was for.

use std::collections::{HashMap, VecDeque};
use std::sync::{Mutex, PoisonError};

use super::policy::{Allowed, Class, Refusal};
use super::request::RequestHead;
use super::sentinel::ChatId;

/// What the proxy answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Allowed,
    Refused(Refusal),
}

/// One answered request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditRecord {
    pub method: String,
    pub host: String,
    /// The allowed repo the request was for, when it was for one.
    pub repo: Option<String>,
    /// `None` for a request refused before it was read as one of the classes.
    pub class: Option<Class>,
    pub verdict: Verdict,
}

/// Per-chat records in memory, newest last, the oldest dropped past [`AuditLog::CAP`].
#[derive(Default)]
pub struct AuditLog {
    by_chat: Mutex<HashMap<ChatId, VecDeque<AuditRecord>>>,
}

impl AuditLog {
    /// Records kept per chat.
    pub const CAP: usize = 2000;

    pub(crate) fn record(
        &self,
        chat: &ChatId,
        head: &RequestHead,
        allowed: Option<&Allowed>,
        verdict: Verdict,
    ) {
        // The host is what the chat sent; it is cut so a long one cannot fill the log.
        let host: String = head.host.chars().take(80).collect();
        let record = AuditRecord {
            method: head.method.to_ascii_uppercase().chars().take(8).collect(),
            host,
            repo: allowed.map(|a| a.repo.clone()),
            class: allowed.map(|a| a.class.clone()),
            verdict,
        };
        let mut map = self.by_chat.lock().unwrap_or_else(PoisonError::into_inner);
        let log = map.entry(chat.clone()).or_default();
        if log.len() == Self::CAP {
            log.pop_front();
        }
        log.push_back(record);
    }

    pub fn for_chat(&self, chat: &ChatId) -> Vec<AuditRecord> {
        self.by_chat
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(chat)
            .map(|log| log.iter().cloned().collect())
            .unwrap_or_default()
    }
}

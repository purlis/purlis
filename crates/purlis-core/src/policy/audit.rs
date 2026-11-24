// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Audit retention** (M75 slice 36, #7045; spec §3.3 and §5.1; Q313.4).
//!
//! How long the event log and the audit record keep what they hold is three `audit.*` limits an
//! Organisation and a project may set. Each is the stricter of every word said (keeping less
//! loosens, so it waits for the person's press and never goes below an Organisation's number),
//! and each is held to the limits key table's least and most, so a number outside reads as the
//! nearest end, with a Notice when a file says it.
//!
//! **No policy turns the record off.** The `audit` family takes no `off` or `log` (it reads as
//! `enforce`), and nothing here consults the `limits` family's mode: retention is a floor, not a
//! cap that was met, so `limits = "off"` changes none of it. A key no side says reads as the
//! shipped value, and a missing table row as the strictest there is, never as zero.

use super::levels::Loaded;
use super::limits;

/// How long the audit stores keep what they hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Retention {
    /// Days of event log.
    pub event_log_days: i64,
    /// Days of the audit record.
    pub record_days: i64,
    /// Bytes of the audit record.
    pub record_max_bytes: i64,
}

/// The three keys.
pub const EVENT_LOG_DAYS: &str = "audit.event-log-days";
/// Days of the audit record.
pub const RECORD_DAYS: &str = "audit.record-days";
/// Bytes of the audit record.
pub const RECORD_MAX_BYTES: &str = "audit.record-max-bytes";

/// **The retention `number_of` reads**, key by key. A key with no number in force reads as the
/// shipped value, and the result is always held to the key's least and most, so it is never
/// zero.
pub fn retention_by(number_of: impl Fn(&str) -> Option<i64>) -> Retention {
    let read = |key: &str| {
        let spec = limits::spec(key).expect("the limits key table names every audit key");
        let shipped = match spec.shipped {
            limits::Shipped::Number(n) => n,
            limits::Shipped::Unset | limits::Shipped::ByMemory => limits::strictest(spec),
        };
        spec.bounded(number_of(key).unwrap_or(shipped))
    };
    Retention {
        event_log_days: read(EVENT_LOG_DAYS),
        record_days: read(RECORD_DAYS),
        record_max_bytes: read(RECORD_MAX_BYTES),
    }
}

impl Loaded {
    /// **The retention this one loaded policy puts in force**: the stricter of the Organisation's,
    /// the project's as committed, your own file's and the accepted copy's numbers, held to the
    /// least and most. Like [`Loaded::limit`] it knows no older version of the accepted floor;
    /// the app's enforcement is [`super::force::limit`] through [`retention_by`].
    pub fn audit_retention(&self) -> Retention {
        retention_by(|key| self.limit(key))
    }
}

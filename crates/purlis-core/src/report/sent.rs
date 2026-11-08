// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The sent list: every report this machine sent, and its exact bytes** (#5689, spec #5683,
//! Q13). The local viewer ruling O9 asks for: Settings › You › Reports lists each entry, and
//! opens the bytes that left, not a summary of them.
//!
//! **Tier: Machine, device-bound** (ADR 0069): `<config>/reports-sent.json`, beside
//! `reports.json`, through the same contained read and write ([`crate::machine::read_beside`],
//! [`crate::machine::write_beside`]), `0600`. Kept [`DAYS_KEPT`] days: an entry older is dropped
//! the next time one is added. Nothing sends it anywhere.

use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// The most drafts sent unread in one UTC day (#5693, spec #5683, Q2b): the next one falls back
/// to asking.
pub const DRAFTS_A_DAY: usize = 5;

/// The most reports that wait on this machine for the intake to answer (#5693, Q24): one more
/// is not kept.
pub const WAITING_AT_MOST: usize = 50;

/// How long a report waits for the intake: one older is dropped unsent.
pub const DAYS_WAITING: i64 = 14;

/// The file, inside [`crate::machine::DIR`].
pub const FILE: &str = "reports-sent.json";

/// The format's version. Another one reads as an empty list.
pub const VERSION: u32 = 1;

/// How long an entry is kept.
pub const DAYS_KEPT: i64 = 90;

/// The most the file may hold. A larger one is not read.
pub const MAX_BYTES: u64 = 16 * 1024 * 1024;

/// Where a report went.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SentTo {
    /// purlis's private Report intake (ADR 0118).
    Intake,
    /// purlis/purlis on GitHub, filed as the person or added to an issue already there
    /// (#5694).
    #[serde(rename = "github")]
    GitHub,
}

/// One report sent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sent {
    /// When it left, UTC, RFC 3339 to the second.
    pub at: String,
    pub title: String,
    pub to: SentTo,
    /// Where it was kept, as the destination answered.
    pub address: String,
    /// Exactly what was sent.
    pub bytes: String,
    /// What makes this report the same as another (#5693, Q15): for a Crash report, where it
    /// crashed in which purlis version. A report with the fingerprint of one listed is not sent
    /// again: it is counted on that entry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<String>,
    /// How many times the same report was made again after this one.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub repeats: u32,
    /// How many of [`Sent::repeats`] the intake was told of, on a later report's envelope.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub repeats_sent: u32,
    /// A draft sent unread, by Always send (#5695): it counts against [`DRAFTS_A_DAY`]. A Crash
    /// report never does: it cannot fall back to asking, and the fingerprint bounds it instead.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub unread_draft: bool,
    /// Not sent yet: the intake could not be reached, and it waits on this machine (#5693, Q24).
    /// Its `at` is when it was made, and its `address` is empty.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub waiting: bool,
}

/// Marks each count in `told` as told on the entry with its fingerprint.
fn mark_told(kept: &mut [Sent], told: &[(String, u32)]) {
    for one in kept {
        if let Some((_, count)) = told
            .iter()
            .find(|(fingerprint, _)| one.fingerprint.as_ref() == Some(fingerprint))
        {
            one.repeats_sent = *count;
        }
    }
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

#[derive(Serialize, Deserialize)]
struct OnDisk {
    version: u32,
    sent: Vec<Sent>,
}

/// The sent list under a config root.
pub struct SentList;

impl SentList {
    /// Every report kept under `config_root`, oldest first, the ones still waiting to send
    /// among them; empty when there is no file or one purlis will not read. A report that has
    /// waited longer than [`DAYS_WAITING`] is not read, and goes with the next write.
    pub fn read(config_root: &Path) -> Vec<Sent> {
        let oldest_waiting = chrono::Utc::now() - chrono::Duration::days(DAYS_WAITING);
        SentList::read_all(config_root)
            .into_iter()
            .filter(|one| {
                !one.waiting
                    || chrono::DateTime::parse_from_rfc3339(&one.at)
                        .is_ok_and(|at| at >= oldest_waiting)
            })
            .collect()
    }

    fn read_all(config_root: &Path) -> Vec<Sent> {
        crate::machine::read_beside(
            config_root,
            FILE,
            MAX_BYTES,
            "the reports this machine sent",
        )
        .ok()
        .flatten()
        .and_then(|text| serde_json::from_str::<OnDisk>(&text).ok())
        .filter(|on_disk| on_disk.version == VERSION)
        .map(|on_disk| on_disk.sent)
        .unwrap_or_default()
    }

    /// Adds `sent`, dropping every entry older than [`DAYS_KEPT`].
    pub fn add(config_root: &Path, sent: Sent) -> io::Result<()> {
        SentList::change(config_root, |kept| kept.push(sent))
    }

    /// Counts a repeat on the entry with `fingerprint`, and answers how many repeats it has now;
    /// `None` when no entry has it, or the list cannot be written.
    pub(super) fn count_repeat(config_root: &Path, fingerprint: &str) -> Option<u32> {
        SentList::change(config_root, |kept| {
            let one = kept
                .iter_mut()
                .find(|one| one.fingerprint.as_deref() == Some(fingerprint))?;
            one.repeats += 1;
            Some(one.repeats)
        })
        .ok()
        .flatten()
    }

    /// Each fingerprint with repeats the intake has not been told of, and its count.
    pub(super) fn untold_repeats(config_root: &Path) -> Vec<(String, u32)> {
        SentList::read(config_root)
            .into_iter()
            .filter(|one| one.repeats > one.repeats_sent)
            .filter_map(|one| Some((one.fingerprint?, one.repeats)))
            .collect()
    }

    /// Adds `sent`, and marks `told` (as [`SentList::untold_repeats`] answered) as told.
    pub(super) fn add_telling(
        config_root: &Path,
        sent: Sent,
        told: &[(String, u32)],
    ) -> io::Result<()> {
        SentList::change(config_root, |kept| {
            mark_told(kept, told);
            kept.push(sent);
        })
    }

    /// Keeps `sent` to send when the intake answers again, unless [`WAITING_AT_MOST`] already
    /// wait: then it is not kept, and the answer is `false`.
    pub(super) fn wait(config_root: &Path, sent: Sent) -> io::Result<bool> {
        SentList::change(config_root, |kept| {
            let room = kept.iter().filter(|one| one.waiting).count() < WAITING_AT_MOST;
            if room {
                kept.push(Sent {
                    waiting: true,
                    ..sent
                });
            }
            room
        })
    }

    /// The reports waiting to send, oldest first.
    pub(super) fn waiting(config_root: &Path) -> Vec<Sent> {
        SentList::read(config_root)
            .into_iter()
            .filter(|one| one.waiting)
            .collect()
    }

    /// Marks the waiting report whose bytes were `waited` as sent: `left`, with when it left,
    /// where it was kept and the bytes that left, and marks `told` as told.
    pub(super) fn left(
        config_root: &Path,
        waited: &str,
        left: Sent,
        told: &[(String, u32)],
    ) -> io::Result<()> {
        SentList::change(config_root, |kept| {
            mark_told(kept, told);
            if let Some(one) = kept
                .iter_mut()
                .find(|one| one.waiting && one.bytes == waited)
            {
                *one = Sent {
                    waiting: false,
                    repeats: one.repeats,
                    repeats_sent: one.repeats_sent,
                    ..left
                };
            }
        })
    }

    /// Refused as [`super::Refusal::CapReached`] when [`DRAFTS_A_DAY`] drafts were sent unread,
    /// or wait to be, on `now`'s UTC day. #5695 asks it before Always send sends a draft, and
    /// shows the draft instead when it is refused.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "#5695 wires the cap into Always send")
    )]
    pub(super) fn within_the_daily_cap(
        config_root: &Path,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Result<(), super::Refusal> {
        let today = now.date_naive();
        let drafts = SentList::read(config_root)
            .iter()
            .filter(|one| one.unread_draft)
            .filter(|one| {
                chrono::DateTime::parse_from_rfc3339(&one.at)
                    .is_ok_and(|at| at.with_timezone(&chrono::Utc).date_naive() == today)
            })
            .count();
        if drafts < DRAFTS_A_DAY {
            Ok(())
        } else {
            Err(super::Refusal::CapReached)
        }
    }

    /// Writes the list `change` makes of the one kept, after dropping every entry older than
    /// [`DAYS_KEPT`].
    fn change<R>(config_root: &Path, change: impl FnOnce(&mut Vec<Sent>) -> R) -> io::Result<R> {
        crate::machine::supported()?;
        let oldest = chrono::Utc::now() - chrono::Duration::days(DAYS_KEPT);
        let mut kept: Vec<Sent> = SentList::read(config_root)
            .into_iter()
            .filter(|one| {
                chrono::DateTime::parse_from_rfc3339(&one.at).is_ok_and(|at| at >= oldest)
            })
            .collect();
        let said = change(&mut kept);
        let dir = crate::machine::private_dir(config_root)?;
        let text = serde_json::to_string_pretty(&OnDisk {
            version: VERSION,
            sent: kept,
        })
        .map_err(io::Error::other)?;
        crate::machine::write_beside(config_root, &dir.join(FILE), (text + "\n").as_bytes())?;
        Ok(said)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::Refusal;

    fn entry(at: chrono::DateTime<chrono::Utc>, unread_draft: bool) -> Sent {
        Sent {
            at: at.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
            title: "a report".into(),
            to: SentTo::Intake,
            address: "an event".into(),
            bytes: "{}\n".into(),
            fingerprint: None,
            repeats: 0,
            repeats_sent: 0,
            unread_draft,
            waiting: false,
        }
    }

    #[test]
    fn a_sixth_unread_draft_in_a_utc_day_is_refused_as_cap_reached() {
        let home = tempfile::tempdir().unwrap();
        let now = chrono::Utc::now();
        for _ in 0..DRAFTS_A_DAY - 1 {
            SentList::add(home.path(), entry(now, true)).unwrap();
        }
        assert_eq!(SentList::within_the_daily_cap(home.path(), now), Ok(()));

        SentList::add(home.path(), entry(now, true)).unwrap();

        assert_eq!(
            SentList::within_the_daily_cap(home.path(), now),
            Err(Refusal::CapReached)
        );
    }

    #[test]
    fn the_cap_counts_neither_yesterdays_drafts_nor_crash_reports() {
        let home = tempfile::tempdir().unwrap();
        let now = chrono::Utc::now();
        for _ in 0..DRAFTS_A_DAY {
            SentList::add(home.path(), entry(now - chrono::Duration::days(1), true)).unwrap();
            SentList::add(home.path(), entry(now, false)).unwrap();
        }
        SentList::add(home.path(), entry(now, true)).unwrap();

        assert_eq!(SentList::within_the_daily_cap(home.path(), now), Ok(()));
    }
}

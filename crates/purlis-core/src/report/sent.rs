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
}

#[derive(Serialize, Deserialize)]
struct OnDisk {
    version: u32,
    sent: Vec<Sent>,
}

/// The sent list under a config root.
pub struct SentList;

impl SentList {
    /// Every report kept under `config_root`, oldest first; empty when there is no file or one
    /// purlis will not read.
    pub fn read(config_root: &Path) -> Vec<Sent> {
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
        crate::machine::supported()?;
        let oldest = chrono::Utc::now() - chrono::Duration::days(DAYS_KEPT);
        let mut kept: Vec<Sent> = SentList::read(config_root)
            .into_iter()
            .filter(|one| {
                chrono::DateTime::parse_from_rfc3339(&one.at).is_ok_and(|at| at >= oldest)
            })
            .collect();
        kept.push(sent);
        let dir = crate::machine::private_dir(config_root)?;
        let text = serde_json::to_string_pretty(&OnDisk {
            version: VERSION,
            sent: kept,
        })
        .map_err(io::Error::other)?;
        crate::machine::write_beside(config_root, &dir.join(FILE), (text + "\n").as_bytes())
    }
}

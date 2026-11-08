// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What the person said about reports, once, on this machine** (#5688, spec #5683, Q1/Q10).
//!
//! Two streams, each answered on its own: **Crash reports**, which purlis sends with nobody
//! asking, and **Agent drafts**, which an agent or purlis itself writes for the person to read.
//! Neither is ever answered for the person (GDPR): unanswered is no, and so is a file that is
//! missing or that purlis cannot read. [`super::Reports`] holds one and refuses every draft and
//! send of a stream without its yes.
//!
//! **Tier: Machine, device-bound** (ADR 0069), at the You level (Q11): kept in purlis's machine
//! directory (`<config>/reports.json`) beside `onboarding.json`, through the same contained read
//! and write ([`crate::machine::read_beside`], [`crate::machine::write_beside`]). One answer on
//! one machine, for every project on it; a team's committed file can never give it.
//!
//! **Asked once.** [`Consent::asked`] says the first-run question was answered, by both answers
//! or by Not now. The file is new with this version, so a machine that ran an older purlis has
//! none and is asked once too (Q8).

use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// The file, inside [`crate::machine::DIR`].
pub const FILE: &str = "reports.json";

/// The format's version. Another one reads as no answer.
pub const VERSION: u32 = 1;

/// The most the file may hold: two answers. A larger one is not read.
pub const MAX_BYTES: u64 = 4 * 1024;

/// A kind of report the person says yes or no to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Stream {
    /// Machine-made reports with fixed fields, sent with nobody asking.
    CrashReports,
    /// Reports an agent or purlis itself writes, for the person to read.
    AgentDrafts,
}

/// The person's answers. `None` is unanswered, which is no.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Consent {
    /// Crash reports: `Some(true)` is Send.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crash_reports: Option<bool>,
    /// Agent drafts: `Some(true)` is Allow.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_drafts: Option<bool>,
    /// Whether the first-run question was answered, by Save or by Not now.
    #[serde(default)]
    pub asked: bool,
}

#[derive(Serialize, Deserialize)]
struct OnDisk {
    version: u32,
    #[serde(flatten)]
    consent: Consent,
}

impl Consent {
    /// Whether the person said yes to `stream`.
    pub fn allows(&self, stream: Stream) -> bool {
        let said = match stream {
            Stream::CrashReports => self.crash_reports,
            Stream::AgentDrafts => self.agent_drafts,
        };
        said == Some(true)
    }

    /// What this machine keeps, under `config_root`: nothing answered and not yet asked when
    /// there is no file or one purlis will not read. Never an error: no yes is the safe answer.
    pub fn read(config_root: &Path) -> Consent {
        crate::machine::read_beside(config_root, FILE, MAX_BYTES, "your answers about reports")
            .ok()
            .flatten()
            .and_then(|text| serde_json::from_str::<OnDisk>(&text).ok())
            .filter(|on_disk| on_disk.version == VERSION)
            .map(|on_disk| on_disk.consent)
            .unwrap_or_default()
    }

    /// What this machine keeps, or nothing answered where it has no config home.
    pub fn here() -> Consent {
        crate::machine::config_root().map_or_else(Consent::default, |root| Consent::read(&root))
    }

    /// Keeps these answers under `config_root`, replacing what was kept: whole, `0600`, never
    /// through a link.
    pub fn write(&self, config_root: &Path) -> io::Result<()> {
        crate::machine::supported()?;
        let dir = crate::machine::private_dir(config_root)?;
        let text = serde_json::to_string_pretty(&OnDisk {
            version: VERSION,
            consent: *self,
        })
        .expect("two answers are plain data serde can always write");
        crate::machine::write_beside(config_root, &dir.join(FILE), (text + "\n").as_bytes())
    }
}

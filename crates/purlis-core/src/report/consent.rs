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
//!
//! **Asked again once, after a crash** (#5700). [`Consent::crash_notice`] is the one answer
//! purlis writes itself: [`super::Reports`] marks it due when it refuses a crash for someone
//! who chose Not now ([`Consent::wants_the_crash_notice`]), and the window settles it when the
//! person dismisses or answers the Notice.

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
    /// Always send (#5695, Q2b): `Some(true)` is the person's standing yes for drafts to go to
    /// the Report intake unread, held first with Undo. It means nothing without the yes to Agent
    /// drafts, and is asked the first time the person presses Always send on a draft.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub always_send: Option<bool>,
    /// Whether the first-run question was answered, by Save or by Not now.
    #[serde(default)]
    pub asked: bool,
    /// Whether purlis asks again after a crash (#5700): `None` while no crash was caught after
    /// Not now.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crash_notice: Option<CrashNotice>,
}

/// **Asked again, once, after a crash** (#5700, spec #5683, story 8): someone who chose Not now
/// is asked about Crash reports again by one Notice in the Inbox, after the next crash purlis
/// catches, and never after that.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CrashNotice {
    /// A crash was caught while Crash reports were left unanswered by Not now: the next window
    /// raises the Notice.
    Due,
    /// The Notice was dismissed or answered: it is never raised again.
    Settled,
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

    /// Whether drafts go to the Report intake unread (#5695): the yes to Agent drafts and to
    /// Always send, both.
    pub fn always_sends(&self) -> bool {
        self.allows(Stream::AgentDrafts) && self.always_send == Some(true)
    }

    /// Whether a crash caught now should raise the crash Notice (#5700): the first-run question
    /// was answered with Not now, Crash reports are still unanswered, and the Notice was never
    /// raised before.
    pub fn wants_the_crash_notice(&self) -> bool {
        self.asked && self.crash_reports.is_none() && self.crash_notice.is_none()
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
        super::config_home().map_or_else(Consent::default, |root| Consent::read(&root))
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

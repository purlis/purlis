// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Report drafts a chat wrote, held for the person's Inbox** (#5694, spec #5683, Q2).
//!
//! A chat holds no sign-in (ADR 0070), so a report a chat drafts waits here until the person
//! reads it, edits it, files it or dismisses it from the Inbox. Each names the chat it came from
//! and the project it was written in, so the project's window lists it and Go to chat finds it.
//!
//! **Tier: Machine, device-bound** (ADR 0069): `<config>/report-drafts.json`, beside
//! `reports.json` and `reports-sent.json`, through the same contained read and write
//! ([`crate::machine::read_beside`], [`crate::machine::write_beside`]), `0600`. Kept
//! [`DAYS_KEPT`] days and at most [`AT_MOST_KEPT`]: an older or further draft is dropped the next
//! time one is written. Nothing sends it anywhere: only the person's File does.

use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};
use super::Kind;

/// The file, inside [`crate::machine::DIR`].
pub const FILE: &str = "report-drafts.json";

/// The format's version. Another one reads as no drafts.
pub const VERSION: u32 = 1;

/// How long a draft waits for the person.
pub const DAYS_KEPT: i64 = 14;

/// The most drafts kept; the oldest go first.
pub const AT_MOST_KEPT: usize = 50;

/// The most the file may hold. A larger one is not read.
pub const MAX_BYTES: u64 = 8 * 1024 * 1024;

/// The chat a draft came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CameFrom {
    /// The chat's number in the app while it means one, for Go to chat.
    pub session: Option<u32>,
    /// The chat's name as the window shows it (`steward 4`): data, drawn as text.
    pub chat: String,
    /// The project it was written in, as its root's path: whose Inbox lists it.
    pub project: String,
}

/// A draft held for the person.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Held {
    /// Its own name, for the window to act on it by.
    pub id: String,
    /// When it was held, UTC, RFC 3339 to the second.
    pub at: String,
    pub kind: HeldKind,
    pub title: String,
    pub body: String,
    /// The call that failed, its exit and the versions, in one line: the card's sentence.
    pub line: String,
    /// The categories the scrub removed.
    #[serde(default)]
    pub scrubbed: Vec<String>,
    pub from: CameFrom,
    /// The person changed its words in the Inbox: no longer only the agent's.
    #[serde(default)]
    pub edited: bool,
}

/// [`Kind`], as the file keeps it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HeldKind {
    Bug,
    Feature,
}

impl From<Kind> for HeldKind {
    fn from(kind: Kind) -> HeldKind {
        match kind {
            Kind::Bug => HeldKind::Bug,
            Kind::Feature => HeldKind::Feature,
        }
    }
}

impl Held {
    /// Twelve hex characters over the repository, the title and the body, as
    /// [`super::Draft::digest`] has them: what the window's File must name for exactly these
    /// words to be filed.
    pub fn digest(&self) -> String {
        super::Draft {
            kind: self.report_kind(),
            title: self.title.clone(),
            body: self.body.clone(),
            scrubbed: Vec::new(),
        }
        .digest()
    }

    pub(super) fn report_kind(&self) -> Kind {
        match self.kind {
            HeldKind::Bug => Kind::Bug,
            HeldKind::Feature => Kind::Feature,
        }
    }
}

#[derive(Serialize, Deserialize)]
struct OnDisk {
    version: u32,
    held: Vec<Held>,
}

/// The drafts held under a config root.
pub struct HeldDrafts;

impl HeldDrafts {
    /// Every draft held under `config_root`, oldest first; none when there is no file or one
    /// purlis will not read.
    pub fn read(config_root: &Path) -> Vec<Held> {
        crate::machine::read_beside(
            config_root,
            FILE,
            MAX_BYTES,
            "the report drafts waiting for you",
        )
        .ok()
        .flatten()
        .and_then(|text| serde_json::from_str::<OnDisk>(&text).ok())
        .filter(|on_disk| on_disk.version == VERSION)
        .map(|on_disk| on_disk.held)
        .unwrap_or_default()
    }

    /// Keeps `held` in place of what was held, dropping what is past [`DAYS_KEPT`] or
    /// [`AT_MOST_KEPT`].
    pub fn write(config_root: &Path, held: Vec<Held>) -> io::Result<()> {
        crate::machine::supported()?;
        let oldest = chrono::Utc::now() - chrono::Duration::days(DAYS_KEPT);
        let mut kept: Vec<Held> = held
            .into_iter()
            .filter(|one| {
                chrono::DateTime::parse_from_rfc3339(&one.at).is_ok_and(|at| at >= oldest)
            })
            .collect();
        let over = kept.len().saturating_sub(AT_MOST_KEPT);
        kept.drain(..over);
        let dir = crate::machine::private_dir(config_root)?;
        let text = serde_json::to_string_pretty(&OnDisk {
            version: VERSION,
            held: kept,
        })
        .map_err(io::Error::other)?;
        crate::machine::write_beside(config_root, &dir.join(FILE), (text + "\n").as_bytes())
    }
}

/// A new draft's id: random, so two drafts of the same words are still two.
pub(super) fn new_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()[..16].to_owned()
}

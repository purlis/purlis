// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **When each source was last checked, and what that check found** (#3840): the line a source's
//! row and its page draw, "N skills, i installed · checked today 09:12", read with the source list
//! and never by asking a source.
//!
//! Each browse that reads a source records when it did and how many skills it offers; each
//! updates check that the source answers records when it did and keeps the count the last
//! browse found, since asking for a newer commit reads its refs alone. How many skills are
//! installed from a source is not recorded: it is read from the lockfiles, on this machine, as
//! the source list is read ([`installed_from`]).
//!
//! The record is a cache, `<state>/cache/skill-sources-checked.json`: deleting it costs only the
//! line until the next browse or check. A write that fails is logged and never refuses the
//! browse or the check that made it, and a file that is not the record purlis writes reads as
//! none.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::Remote;
use super::lock;

/// The record, relative to the project's state folder.
pub const FILE: &str = "cache/skill-sources-checked.json";

/// The record's version.
const V: u32 = 1;

/// The most of the record purlis reads: some two thousand sources' entries.
const MOST_BYTES: usize = 256 * 1024;

/// The most sources it keeps: the ones checked longest ago go first.
const MOST_SOURCES: usize = 512;

/// **What the last check of one source found.**
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Checked {
    #[serde(flatten)]
    pub remote: Remote,
    /// When it was last checked, in seconds since the epoch.
    pub at: u64,
    /// How many skills it offered when it was last browsed; `None` where it was only asked for
    /// its newest commit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skills: Option<u32>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Record {
    v: u32,
    #[serde(default)]
    sources: Vec<Checked>,
}

/// The record of the project at `plane`.
pub fn path(plane: &Path) -> PathBuf {
    crate::names::state(plane).join(FILE)
}

/// **What the last check of each source found**, as recorded: none for a record that is not
/// there, or not the one purlis writes.
pub fn read(plane: &Path) -> Vec<Checked> {
    let file = path(plane);
    let Ok(bytes) = crate::contain::read_no_link(plane, &file) else {
        return Vec::new();
    };
    if bytes.len() > MOST_BYTES {
        return Vec::new();
    }
    match serde_json::from_slice::<Record>(&bytes) {
        Ok(record) if record.v == V && record.sources.len() <= MOST_SOURCES => record.sources,
        _ => Vec::new(),
    }
}

/// **A browse read `remote`** now and found `skills` skills: answers when, as recorded.
pub fn browsed(plane: &Path, remote: &Remote, skills: usize) -> u64 {
    let skills = u32::try_from(skills).unwrap_or(u32::MAX);
    let at = now();
    noted(plane, remote, at, Some(skills));
    at
}

/// **An updates check asked `remote`** now, and it answered.
pub fn asked(plane: &Path, remote: &Remote) {
    noted(plane, remote, now(), None);
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

/// Records that `remote` was checked `at`, with the count a browse found (`None` keeps the one
/// recorded before). Logged, never refused, where it cannot be written.
pub(super) fn noted(plane: &Path, remote: &Remote, at: u64, skills: Option<u32>) {
    let mut sources = read(plane);
    let skills = skills.or_else(|| {
        sources
            .iter()
            .find(|one| one.remote == *remote)
            .and_then(|one| one.skills)
    });
    sources.retain(|one| one.remote != *remote);
    sources.push(Checked {
        remote: remote.clone(),
        at,
        skills,
    });
    sources.sort_by_key(|one| std::cmp::Reverse(one.at));
    sources.truncate(MOST_SOURCES);
    if let Err(why) = write(plane, Record { v: V, sources }) {
        tracing::warn!("purlis: when a skill source was checked was not recorded ({why})");
    }
}

fn write(plane: &Path, record: Record) -> std::io::Result<()> {
    let file = path(plane);
    let bytes = serde_json::to_vec(&record).map_err(std::io::Error::other)?;
    if let Some(dir) = file.parent() {
        crate::rewrite::create_dir_all(dir)?;
    }
    crate::rewrite::replace(plane, &file, &bytes, crate::rewrite::Mode::Private)
}

/// **How many skills are installed from `remote`**, at any level, as the lockfiles this machine
/// reads say: each skill counted once, whatever number of levels hold it.
pub fn installed_from(locked: &[(lock::Kept, lock::Locked)], remote: &Remote) -> u32 {
    let mut names: Vec<&str> = locked
        .iter()
        .filter(|(_, one)| one.source == *remote)
        .map(|(_, one)| one.name.as_str())
        .collect();
    names.sort_unstable();
    names.dedup();
    u32::try_from(names.len()).unwrap_or(u32::MAX)
}

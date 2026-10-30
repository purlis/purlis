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

/// **An updates check asked each of `remotes`** now, and each answered: one write for them all.
pub fn asked(plane: &Path, remotes: &[Remote]) {
    if remotes.is_empty() {
        return;
    }
    let at = now();
    let noting: Vec<_> = remotes.iter().map(|remote| (remote, None)).collect();
    noted_all(plane, &noting, at);
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

/// One change of the record at a time in this process: a browse and an updates check run on
/// threads of their own, and the second's read-change-write would undo the first's.
static WRITING: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Records that `remote` was checked `at`, with the count a browse found (`None` keeps the one
/// recorded before). Logged, never refused, where it cannot be written.
pub(super) fn noted(plane: &Path, remote: &Remote, at: u64, skills: Option<u32>) {
    noted_all(plane, &[(remote, skills)], at);
}

fn noted_all(plane: &Path, noting: &[(&Remote, Option<u32>)], at: u64) {
    let _one_at_a_time = WRITING
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut sources = read(plane);
    for (remote, skills) in noting {
        let skills = skills.or_else(|| {
            sources
                .iter()
                .find(|one| one.remote == **remote)
                .and_then(|one| one.skills)
        });
        sources.retain(|one| one.remote != **remote);
        sources.push(Checked {
            remote: (*remote).clone(),
            at,
            skills,
        });
    }
    sources.sort_by_key(|one| std::cmp::Reverse(one.at));
    sources.truncate(MOST_SOURCES);
    if let Err(why) = write(plane, sources) {
        tracing::warn!("purlis: when a skill source was checked was not recorded ({why})");
    }
}

/// Writes `sources`, newest first, leaving out the oldest until the record fits what [`read`]
/// reads, so a record of long URLs is never written past it and then read as none.
fn write(plane: &Path, mut sources: Vec<Checked>) -> std::io::Result<()> {
    let file = path(plane);
    let bytes = loop {
        let bytes = serde_json::to_vec(&Record {
            v: V,
            sources: sources.clone(),
        })
        .map_err(std::io::Error::other)?;
        if bytes.len() <= MOST_BYTES || sources.is_empty() {
            break bytes;
        }
        sources.truncate(sources.len() - sources.len().div_ceil(8));
    };
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

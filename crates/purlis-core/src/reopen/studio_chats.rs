// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The chats the Skill studio started, kept once they close** (#5248): a skill's Runs keeps
//! the chat after it ends, with when it finished. While a chat is open its link lives on its own
//! record ([`super::StudioJob`], in `reopen.json`); as it closes, the app keeps the link here
//! with the chat's number and name, so the skill's "Chats the studio started" still lists it.
//!
//! **Tier:** Machine, device-bound. It lives in this machine's app state of the project,
//! `<data>/app-state/<clone key>/studio-chats.json` ([`FILE`], `crate::appstate`): the chats are
//! this machine's. Read through no link, at most [`MOST_FILE_BYTES`], and a file that does not
//! hold together reads as empty. Losing it only takes those chats off Runs. Nothing starts, ends
//! or allows anything by it.

use std::io::Read as _;
use std::path::{Path, PathBuf};

use super::studio::{StudioJob, StudioOnDisk, time};

/// The file in this machine's app state of the project that holds the closed studio chats.
pub const FILE: &str = "studio-chats.json";

/// The lock file beside [`FILE`] whose `flock` each change holds while it reads, changes and
/// writes the file.
pub const LOCK: &str = "studio-chats.lock";

/// The version of the file this purlis writes and reads.
const VERSION: u32 = 1;

/// The most bytes of the file read.
const MOST_FILE_BYTES: u64 = 64 * 1024;

/// The most chats kept: past them the oldest go.
pub const MOST_CHATS: usize = 64;

/// How long a change waits for another to let go of [`LOCK`].
const LOCK_WAIT: std::time::Duration = std::time::Duration::from_secs(2);

/// **A studio chat closed here**: its number, its name as it closed, when, and its link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosedStudioChat {
    /// The chat's number, which the project never deals again: what a draft names its chat by.
    pub chat: u32,
    pub name: String,
    /// When it closed, as an RFC 3339 time.
    pub closed: String,
    /// What it was started for, with `finished` always set: the close, where nothing sooner.
    pub job: StudioJob,
}

/// One entry as the file keeps it: the chat, and its link as `reopen.json` holds it.
#[derive(serde::Serialize, serde::Deserialize)]
struct OnDisk {
    chat: u32,
    name: String,
    closed: String,
    studio: StudioOnDisk,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Kept {
    v: u32,
    #[serde(default)]
    chats: Vec<serde_json::Value>,
}

/// Where the file of the project at `root` is: `None` where this machine has no data home.
fn path_of(root: &Path) -> Option<PathBuf> {
    Some(crate::appstate::dir(root)?.join(FILE))
}

/// Whether `said` reads as an RFC 3339 time.
fn reads_as_time(said: &str) -> bool {
    chrono::DateTime::parse_from_rfc3339(said).is_ok()
}

/// One entry as it reads, or `None` where it does not hold together.
fn sound(value: serde_json::Value) -> Option<ClosedStudioChat> {
    let one: OnDisk = serde_json::from_value(value).ok()?;
    if !reads_as_time(&one.closed) {
        return None;
    }
    let mut job = one.studio.sound()?;
    job.finished.get_or_insert_with(|| one.closed.clone());
    Some(ClosedStudioChat {
        chat: one.chat,
        name: one.name,
        closed: one.closed,
        job,
    })
}

/// The file's entries as they read, newest first: none where it is absent, not a file of its
/// own, too large, or not what purlis writes.
fn read_values(path: &Path) -> Vec<serde_json::Value> {
    let Ok(meta) = std::fs::symlink_metadata(path) else {
        return Vec::new();
    };
    if !meta.is_file() || meta.len() > MOST_FILE_BYTES {
        return Vec::new();
    }
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    let Ok(file) = crate::contain::nofollow(&mut options).open(path) else {
        return Vec::new();
    };
    let mut text = String::new();
    if file
        .take(MOST_FILE_BYTES)
        .read_to_string(&mut text)
        .is_err()
    {
        return Vec::new();
    }
    match serde_json::from_str::<Kept>(&text) {
        Ok(kept) if kept.v == VERSION => kept.chats,
        _ => Vec::new(),
    }
}

/// **The studio chats closed in the project at `root`**, newest first: none where this machine
/// keeps none. An entry that does not hold together is left out.
pub fn closed(root: &Path) -> Vec<ClosedStudioChat> {
    let Some(path) = path_of(root) else {
        return Vec::new();
    };
    read_values(&path)
        .into_iter()
        .filter_map(sound)
        .take(MOST_CHATS)
        .collect()
}

/// What a change that failed says.
fn not_kept(why: &str) -> String {
    format!(
        "The studio's chat could not be kept on this machine ({}), so its skill's Runs no longer \
         lists it.",
        crate::shown::short(why)
    )
}

/// **Keeps that chat `chat`, called `name`, closed `now`, with the studio's link `job`**: the
/// newest entry, at most [`MOST_CHATS`] kept. Its job is finished as it closes, where nothing
/// stopped it sooner. An entry for the same number is replaced, as a chat closed twice is the
/// later close.
pub fn keep(
    root: &Path,
    chat: u32,
    name: &str,
    job: &StudioJob,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<(), String> {
    let dir = crate::appstate::made(root).map_err(|why| not_kept(&why))?;
    let mut options = std::fs::OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let lock = crate::contain::nofollow(&mut options)
        .open(dir.join(LOCK))
        .map_err(|why| not_kept(&why.to_string()))?;
    let _held = crate::filelock::lock_within(lock, LOCK_WAIT, "the studio's chats")
        .map_err(|why| not_kept(&why.to_string()))?;
    let closed = time(now);
    let mut job = job.clone();
    job.finished.get_or_insert_with(|| closed.clone());
    let entry = OnDisk {
        chat,
        name: name.to_owned(),
        closed,
        studio: StudioOnDisk::of(&job),
    };
    let entry = serde_json::to_value(entry).map_err(|why| not_kept(&why.to_string()))?;
    let path = dir.join(FILE);
    let others = read_values(&path)
        .into_iter()
        .filter(|one| one.get("chat").and_then(serde_json::Value::as_u64) != Some(u64::from(chat)));
    let chats: Vec<serde_json::Value> = std::iter::once(entry)
        .chain(others)
        .take(MOST_CHATS)
        .collect();
    let text = serde_json::to_string_pretty(&Kept { v: VERSION, chats })
        .map_err(|why| not_kept(&why.to_string()))?;
    crate::plane::write_private(&dir, &path, text.as_bytes())
        .map_err(|why| not_kept(&why.to_string()))
}

#[cfg(test)]
mod tests;

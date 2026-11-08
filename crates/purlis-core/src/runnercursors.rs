// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **How far this window logged each pool runner's skill runs** (#4219, SK-20c).
//!
//! The window reads a pool runner's event log over the relayed link (#3393) and logs each
//! `skill.started` and `skill.ended` of it in its project's skill log (SK-20b, #3084). It keeps
//! here, per runner device and project, the `seq` up to which it logged them for that project
//! (each project has a link of its own to a pool's runner), so a window started again reads
//! the runner's log on from there rather than from its end, and the runs a runner chat made while
//! no window read its log reach the studio. A row read twice across a restart is logged once
//! ([`crate::skilluse::relayed::logged`]).
//!
//! Machine, device-bound, rebuildable (`docs/plane-format.md`, `<config>/runner-log-cursors.json`):
//! lost or unreadable, a window reads on from the log's end, as before, and only the runs made
//! while none read it go. It holds a cursor per runner and nothing of the runner's events (ADR
//! 0078 §7).

use std::collections::BTreeMap;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// The file, inside [`crate::machine::DIR`].
pub const FILE: &str = "runner-log-cursors.json";

/// The format's version. Another one reads as no cursor.
const VERSION: u32 = 1;

/// The most cursors kept, a runner's in a project each: past it, those of the runner kept least
/// lately go first, its lowest first (#5657).
pub const MOST_CURSORS: usize = 256;

/// The longest device id kept: a ULID is 26 characters.
const MOST_ID: usize = 32;

/// The most the file may hold: a cursor per runner and project. A larger one is not read.
const MOST_BYTES: u64 = 64 * 1024;

/// Cursors by runner device, then by project id.
type Cursors = BTreeMap<String, BTreeMap<String, u64>>;

/// One read-modify-write at a time in this process: each link keeps its own runner's.
static WRITING: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// By runner device, when a cursor of it was last kept, as a turn: the higher, the later (#5657).
/// A runner made again mints a new device, whose log's `seq` starts again at the bottom, and
/// nothing removes the cursors of the device it replaced: so which runner's go first is said by
/// this, never by comparing one device's `seq` with another's.
type Turns = BTreeMap<String, u64>;

#[derive(Serialize, Deserialize)]
struct OnDisk {
    version: u32,
    cursors: Cursors,
    /// Absent from a file an earlier build wrote: each of its devices reads as kept before any
    /// other.
    #[serde(default)]
    kept_last: Turns,
}

/// Whether `id` is one a cursor is kept under, a runner's device or a project's: short, of
/// letters, digits, `-` and `_`.
fn an_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MOST_ID
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// Every cursor kept under `config_root`: none for no file, or one this build does not read.
fn read(config_root: &Path) -> Cursors {
    read_with_turns(config_root).0
}

/// Every cursor kept under `config_root`, with when each device's was last kept.
fn read_with_turns(config_root: &Path) -> (Cursors, Turns) {
    crate::machine::read_beside(config_root, FILE, MOST_BYTES, "the runner log cursors")
        .ok()
        .flatten()
        .and_then(|text| serde_json::from_str::<OnDisk>(&text).ok())
        .filter(|on_disk| on_disk.version == VERSION)
        .map(|on_disk| {
            let cursors: Cursors = on_disk
                .cursors
                .into_iter()
                .filter(|(device, _)| an_id(device))
                .map(|(device, projects)| {
                    let projects = projects
                        .into_iter()
                        .filter(|(project, _)| an_id(project))
                        .collect();
                    (device, projects)
                })
                .collect();
            let kept_last = on_disk
                .kept_last
                .into_iter()
                .filter(|(device, _)| cursors.contains_key(device))
                .collect();
            (cursors, kept_last)
        })
        .unwrap_or_default()
}

/// **The `seq` up to which runner `device`'s skill lines were logged here for `project`**, or
/// `None` where none was kept.
pub fn logged_up_to(config_root: &Path, device: &str, project: &str) -> Option<u64> {
    read(config_root).get(device)?.get(project).copied()
}

/// **The lowest cursor kept for runner `device`**, of any project: where a link to it reads on
/// from, so each project's link hears what it has not logged. `None` where none was kept.
pub fn lowest_for(config_root: &Path, device: &str) -> Option<u64> {
    read(config_root).get(device)?.values().min().copied()
}

/// **Keeps that runner `device`'s skill lines were logged here for `project` up to `seq`**, in
/// place of what was kept for it. An id that is not a plain one is not kept.
pub fn keep(config_root: &Path, device: &str, project: &str, seq: u64) -> io::Result<()> {
    if !an_id(device) || !an_id(project) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "a runner's device or a project's id purlis would not keep a cursor under",
        ));
    }
    let _one = WRITING
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let (mut cursors, mut kept_last) = read_with_turns(config_root);
    cursors
        .entry(device.to_owned())
        .or_default()
        .insert(project.to_owned(), seq);
    let turn = kept_last
        .values()
        .max()
        .map_or(1, |last| last.saturating_add(1));
    kept_last.insert(device.to_owned(), turn);
    while cursors.values().map(BTreeMap::len).sum::<usize>() > MOST_CURSORS {
        // A `seq` is compared only within one device's log.
        let first_to_go = cursors
            .iter()
            .flat_map(|(kept, projects)| {
                let when = kept_last.get(kept).copied().unwrap_or(0);
                projects
                    .iter()
                    .map(move |(of, seq)| (when, *seq, kept.clone(), of.clone()))
            })
            .filter(|(_, _, kept, of)| !(kept == device && of == project))
            .min();
        let Some((_, _, kept, of)) = first_to_go else {
            break;
        };
        if let Some(projects) = cursors.get_mut(&kept) {
            projects.remove(&of);
            if projects.is_empty() {
                cursors.remove(&kept);
                kept_last.remove(&kept);
            }
        }
    }
    crate::machine::supported()?;
    let dir = crate::machine::private_dir(config_root)?;
    let text = serde_json::to_string(&OnDisk {
        version: VERSION,
        cursors,
        kept_last,
    })
    .map_err(io::Error::other)?;
    crate::machine::write_beside(config_root, &dir.join(FILE), (text + "\n").as_bytes())
}

#[cfg(all(test, unix))]
mod tests;

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

/// The most cursors kept, a runner's in a project each: past it, the lowest go first.
pub const MOST_CURSORS: usize = 256;

/// The longest device id kept: a ULID is 26 characters.
const MOST_ID: usize = 32;

/// The most the file may hold: a cursor per runner and project. A larger one is not read.
const MOST_BYTES: u64 = 64 * 1024;

/// Cursors by runner device, then by project id.
type Cursors = BTreeMap<String, BTreeMap<String, u64>>;

/// One read-modify-write at a time in this process: each link keeps its own runner's.
static WRITING: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[derive(Serialize, Deserialize)]
struct OnDisk {
    version: u32,
    cursors: Cursors,
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
    crate::machine::read_beside(config_root, FILE, MOST_BYTES, "the runner log cursors")
        .ok()
        .flatten()
        .and_then(|text| serde_json::from_str::<OnDisk>(&text).ok())
        .filter(|on_disk| on_disk.version == VERSION)
        .map(|on_disk| {
            on_disk
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
                .collect()
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
    let mut cursors = read(config_root);
    cursors
        .entry(device.to_owned())
        .or_default()
        .insert(project.to_owned(), seq);
    while cursors.values().map(BTreeMap::len).sum::<usize>() > MOST_CURSORS {
        let lowest = cursors
            .iter()
            .flat_map(|(kept, projects)| {
                projects
                    .iter()
                    .map(move |(of, seq)| (kept.clone(), of.clone(), *seq))
            })
            .filter(|(kept, of, _)| !(kept == device && of == project))
            .min_by_key(|(_, _, seq)| *seq);
        let Some((kept, of, _)) = lowest else {
            break;
        };
        if let Some(projects) = cursors.get_mut(&kept) {
            projects.remove(&of);
            if projects.is_empty() {
                cursors.remove(&kept);
            }
        }
    }
    crate::machine::supported()?;
    let dir = crate::machine::private_dir(config_root)?;
    let text = serde_json::to_string(&OnDisk {
        version: VERSION,
        cursors,
    })
    .map_err(io::Error::other)?;
    crate::machine::write_beside(config_root, &dir.join(FILE), (text + "\n").as_bytes())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    const RUNNER: &str = "01JB0RUNNER0000000000000000";
    const PROJECT: &str = "01J9PRJCT00000000000000000";
    const OTHER: &str = "01J9PRJCT00000000000000OTH";

    #[test]
    fn a_runners_cursor_is_kept_per_project_and_its_lowest_is_where_a_link_reads_from() {
        let home = tempfile::tempdir().unwrap();
        assert_eq!(logged_up_to(home.path(), RUNNER, PROJECT), None);
        assert_eq!(lowest_for(home.path(), RUNNER), None);

        keep(home.path(), RUNNER, PROJECT, 41).unwrap();
        keep(home.path(), RUNNER, PROJECT, 42).unwrap();
        keep(home.path(), RUNNER, OTHER, 7).unwrap();

        assert_eq!(logged_up_to(home.path(), RUNNER, PROJECT), Some(42));
        assert_eq!(logged_up_to(home.path(), RUNNER, OTHER), Some(7));
        assert_eq!(lowest_for(home.path(), RUNNER), Some(7));
        assert_eq!(lowest_for(home.path(), "01JB0OTHER00000000000000000"), None);
    }

    #[test]
    fn an_id_that_is_no_plain_one_is_not_kept_and_a_file_this_build_cannot_read_is_none() {
        let home = tempfile::tempdir().unwrap();
        assert!(keep(home.path(), "../escape", PROJECT, 1).is_err());
        assert!(keep(home.path(), RUNNER, "../escape", 1).is_err());
        assert!(keep(home.path(), &"x".repeat(MOST_ID + 1), PROJECT, 1).is_err());

        keep(home.path(), RUNNER, PROJECT, 3).unwrap();
        let file = crate::machine::dir(home.path()).join(FILE);
        std::fs::write(&file, r#"{"version":2,"cursors":{}}"#).unwrap();
        assert_eq!(logged_up_to(home.path(), RUNNER, PROJECT), None);
        std::fs::write(&file, "not json").unwrap();
        assert_eq!(logged_up_to(home.path(), RUNNER, PROJECT), None);
    }

    #[test]
    fn past_the_most_cursors_the_lowest_go_first_never_the_one_kept_now() {
        let home = tempfile::tempdir().unwrap();
        for n in 0..MOST_CURSORS {
            keep(home.path(), RUNNER, &format!("P{n:04}"), 100 + n as u64).unwrap();
        }

        keep(home.path(), "NEWEST", PROJECT, 0).unwrap();

        let kept = read(home.path());
        assert_eq!(
            kept.values().map(BTreeMap::len).sum::<usize>(),
            MOST_CURSORS
        );
        assert_eq!(logged_up_to(home.path(), "NEWEST", PROJECT), Some(0));
        assert_eq!(
            logged_up_to(home.path(), RUNNER, "P0000"),
            None,
            "the lowest went"
        );
        assert_eq!(logged_up_to(home.path(), RUNNER, "P0001"), Some(101));
    }
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The branch tips purlis fetched from a pool's runners** (the person's ruling of 2026-11-07):
//! `<config>/tips-fetched/<project id>`, Machine and device-bound, one commit's full name a
//! line, oldest first, written as a fetch from a runner brings that runner's branches into a
//! project here ([`record`]).
//!
//! **What it is for**: an older runner of a pool goes only once all it holds is here
//! ([`crate::pools::left_beyond_a_fetch`]). A branch whose tip came here once came home, though
//! the person squashed or rebased it since, deleted the branch, or renamed the workspace it was
//! in: none of that leaves a ref here that reaches the tip, so without this record such a runner
//! would be kept for ever. A commit is the same commit whoever sent it, so the record is kept
//! per project, not per runner.
//!
//! - **Bounded**: each project keeps its newest [`MOST`] tips, within what a private file may be
//!   read at. A tip fetched again counts as new. One that falls out is only no longer counted,
//!   which keeps a runner, never lets one go.
//! - **Only what reads as a project's id and a commit's name** is written or read: a line that is
//!   not one is passed over.

use std::path::{Path, PathBuf};

/// The folder of the records, inside the machine's own folder ([`crate::machine::dir`]).
pub const DIR: &str = "tips-fetched";

/// The lock a record's read-modify-write takes, in the session host's folder.
const LOCK: &str = "tips-fetched.lock";

/// The most tips kept of one project: at most 65 bytes each, within what a private file may be
/// read at ([`purlis_same_user::A_PRIVATE_FILE_IS_AT_MOST`]).
pub const MOST: usize = 1000;

fn folder(config_root: &Path) -> PathBuf {
    crate::machine::dir(config_root).join(DIR)
}

/// Whether `project` is a project's id as purlis names one: a ULID in its canonical spelling.
fn a_project(project: &str) -> bool {
    crate::reopen::a_ulid(project).is_some_and(|id| id == project)
}

/// Whether `oid` is a commit's full name: 40 lower-case hex digits (SHA-1) or 64 (SHA-256).
fn an_oid(oid: &str) -> bool {
    (oid.len() == 40 || oid.len() == 64)
        && oid
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// **The tips purlis fetched into project `project` from a runner**, oldest first. None where it
/// fetched none, or where the record can't be read safely, which is said in the log: what is not
/// read keeps a runner, never lets one go.
pub fn read(config_root: &Path, project: &str) -> Vec<String> {
    if !a_project(project) {
        return Vec::new();
    }
    let path = folder(config_root).join(project);
    match purlis_same_user::read_private_file(&path) {
        Ok(text) => text
            .lines()
            .filter(|line| an_oid(line))
            .map(str::to_owned)
            .collect(),
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(why) => {
            tracing::warn!(
                "purlis: the branch tips fetched from runners could not be read safely ({}), so \
                 none counts as home: {why}",
                path.display()
            );
            Vec::new()
        }
    }
}

/// **Records that the commits `tips` came into project `project` from a runner**, each as its
/// newest, the oldest let go of past [`MOST`]. Nothing for a name that is no project's id; a
/// commit's name that is not one is passed over. Why not, where it could not be written.
pub fn record(config_root: &Path, project: &str, tips: &[String]) -> Result<(), String> {
    let tips: Vec<&String> = tips.iter().filter(|tip| an_oid(tip)).collect();
    if !a_project(project) || tips.is_empty() {
        return Ok(());
    }
    let _locked = crate::machine::Lock::named(config_root, LOCK);
    let mut kept = read(config_root, project);
    kept.retain(|one| !tips.contains(&one));
    for tip in tips {
        if !kept.contains(tip) {
            kept.push(tip.clone());
        }
    }
    let past = kept.len().saturating_sub(MOST);
    let text: String = kept[past..].iter().map(|one| format!("{one}\n")).collect();
    purlis_same_user::PrivateDirectory::open(&folder(config_root))
        .and_then(|dir| dir.replace_file(project, text.as_bytes()))
        .map_err(|why| {
            format!("the branch tips fetched from a runner could not be recorded: {why}")
        })
}

#[cfg(test)]
mod tests;

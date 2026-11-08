// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A native crash, from the crash handler's minidump to a Crash report** (#5691, spec #5683,
//! ruling O9).
//!
//! A panic hook cannot see a segfault, a fault on a stack's guard page or an uncaught
//! Objective-C exception: the process is gone first. The app's crash handler, a companion process
//! (`purlis-crash-handler`), writes a minidump of it from outside into [`crashes_dir`], named
//! `<seconds since 1970>-<id>.dmp`. On the next launch the app calls [`report_waiting`], which
//! hands each one to [`Reports`] as [`Evidence::NativeCrash`]: with a yes to Crash reports it goes
//! to the Report intake, and only there.
//!
//! **Tier: Machine, device-bound** (ADR 0069): `<config>/crashes/`, made `0700` by the handler.
//! A minidump holds the crashed process's stacks, so it never leaves this machine but to the
//! intake, and is kept at most [`KEPT_FOR_SECS`].

use std::path::{Path, PathBuf};

use crate::forge::Caller;

use super::{Evidence, Outcome, Reports};

/// A native crash: the minidump the crash handler wrote, and when.
#[derive(Clone, PartialEq, Eq)]
pub struct NativeCrash {
    /// When it crashed, in seconds since 1970, as the minidump's name says.
    pub at: Option<u64>,
    /// The minidump, as the crash handler wrote it.
    pub minidump: Vec<u8>,
}

impl std::fmt::Debug for NativeCrash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativeCrash")
            .field("at", &self.at)
            .field("minidump", &format_args!("{} bytes", self.minidump.len()))
            .finish()
    }
}

/// How long a minidump waits to be sent: one older is dropped unsent (spec #5683, Q24).
pub const KEPT_FOR_SECS: u64 = 14 * 24 * 60 * 60;

/// The folder the crash handler keeps minidumps in: `crashes/` in this machine's config home.
pub fn crashes_dir(config_root: &Path) -> PathBuf {
    crate::machine::dir(config_root).join("crashes")
}

/// Hands every minidump waiting in `crashes` to `reports`, oldest first, as a Crash report from a
/// trigger (nobody is at the keyboard), and answers with each outcome. A minidump the intake gave
/// no answer for ([`Outcome::Queued`]) waits here for a later launch (#5693, Q24); one that is
/// sent, may not go, was refused, or is older than [`KEPT_FOR_SECS`] is removed. A file still
/// being written (not `.dmp`, or in a folder) is left alone.
pub fn report_waiting(reports: &Reports, crashes: &Path) -> Vec<Outcome> {
    let now = chrono::Utc::now().timestamp().unsigned_abs();
    let mut outcomes = Vec::new();
    for (path, at) in waiting(crashes) {
        if at.is_none_or(|at| now.saturating_sub(at) > KEPT_FOR_SECS) {
            let _ = std::fs::remove_file(&path);
            continue;
        }
        let Ok(minidump) = std::fs::read(&path) else {
            continue;
        };
        let said = reports.take(
            Evidence::NativeCrash(NativeCrash { at, minidump }),
            &Caller::trigger(),
        );
        if said != Outcome::Queued {
            let _ = std::fs::remove_file(&path);
        }
        outcomes.push(said);
    }
    outcomes
}

/// The minidumps in `crashes`, oldest first, each with when it crashed as its name says.
fn waiting(crashes: &Path) -> Vec<(PathBuf, Option<u64>)> {
    let Ok(entries) = std::fs::read_dir(crashes) else {
        return Vec::new();
    };
    let mut found: Vec<(PathBuf, Option<u64>)> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "dmp"))
        .map(|path| {
            let at = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .and_then(|stem| stem.split('-').next())
                .and_then(|secs| secs.parse().ok());
            (path, at)
        })
        .collect();
    found.sort_by_key(|(_, at)| *at);
    found
}

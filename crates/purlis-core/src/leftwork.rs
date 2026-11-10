// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Work left in a chat's folder** (ADR 0121 decision 17, #6014): whether a chat that
//! finished left uncommitted work behind, which is what makes it **Idle** rather than Done.
//!
//! One `git status --porcelain` of the folder, with no optional locks, so it never writes the
//! index a chat may be using. The app asks it as a chat changes state, off the board's
//! thread, and keeps the answer until the next change: it is never asked per frame and never
//! on a timer.

use std::path::Path;

use crate::worktree::git;

/// Whether `folder` holds uncommitted work: a changed, staged or untracked file. `None` where
/// git cannot say (no folder, not a repository, git not there): nothing is claimed.
pub fn left_in(folder: &Path) -> Option<bool> {
    let ran = git::run(
        folder,
        &["--no-optional-locks", "status", "--porcelain"],
        git::READ,
    )
    .ok()?;
    ran.ok().then(|| !ran.out.trim().is_empty())
}

#[cfg(test)]
mod tests;

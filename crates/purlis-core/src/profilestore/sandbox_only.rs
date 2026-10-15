// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A sandboxed Codex chat waits for a sandbox-only profile** (#2037, the operator's ruling of
//! 2026-10-15). A sandboxed Codex chat, at the floor or the full sandbox, runs only on a profile
//! this machine keeps for sandboxed chats only, so no sandboxed chat writes a Codex home an
//! unsandboxed run uses. A start refused for want of one leaves one wait per project
//! ([`waits_on`]), which the window shows as one Ask, *Codex needs a sandbox-only profile here*:
//! its button makes one ([`make`]) and opens Codex's own sign-in in it. The wait goes once this
//! machine has a sandbox-only Codex profile, or the person dismisses it.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use super::{Homes, MachineProfile, Made};

/// The harness the wait is for.
pub const HARNESS: &str = "codex";

/// The name [`make`] gives the first sandbox-only Codex profile: the next is `codex-sandbox-2`.
pub const NAME: &str = "codex-sandbox";

/// What the window's Ask says.
pub const ASK: &str = "Codex needs a sandbox-only profile here";

/// **A project a sandboxed Codex chat was refused in**, for want of a sandbox-only profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wait {
    /// The project.
    pub root: PathBuf,
    /// When the first start was refused, in seconds since 1970.
    pub since: u64,
}

static WAITS: Mutex<Vec<Wait>> = Mutex::new(Vec::new());

/// Records that a sandboxed Codex chat in the project at `root` was refused for want of a
/// sandbox-only profile: one wait per project, however many starts were refused.
pub fn waits_on(root: &Path) {
    let mut all = WAITS.lock().unwrap_or_else(PoisonError::into_inner);
    if all.iter().any(|w| w.root == root) {
        return;
    }
    all.push(Wait {
        root: root.to_path_buf(),
        since: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs()),
    });
}

/// **Whether the project at `root` waits for one**, given this machine's `profiles`: a wait
/// goes once one of them is a sandbox-only Codex profile.
pub fn waiting(root: &Path, profiles: &[MachineProfile]) -> Option<Wait> {
    let mut all = WAITS.lock().unwrap_or_else(PoisonError::into_inner);
    if profiles
        .iter()
        .any(|p| p.sandbox_only && p.harness == HARNESS)
    {
        all.clear();
        return None;
    }
    all.iter().find(|w| w.root == root).cloned()
}

/// The person put the wait in the project at `root` away.
pub fn dismiss(root: &Path) {
    WAITS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .retain(|w| w.root != root);
}

/// **Makes a sandbox-only Codex profile** at `now`, named [`NAME`], or the first of
/// `codex-sandbox-2`, `-3`, … no profile has: the record, ready to be signed in through Codex's
/// own sign-in. Nothing is copied into it.
pub fn make(homes: &Homes, now: u64) -> Result<Made, String> {
    let taken = super::list(homes.config_root())?;
    let name = (1..)
        .map(|n| {
            if n == 1 {
                NAME.to_owned()
            } else {
                format!("{NAME}-{n}")
            }
        })
        .find(|name| {
            !taken.iter().any(|p| &p.name == name)
                && std::fs::symlink_metadata(homes.folder_of(name)).is_err()
        })
        .unwrap_or_else(|| NAME.to_owned());
    super::create_sandbox_only(homes, &name, HARNESS, now)
}

#[cfg(test)]
mod tests;

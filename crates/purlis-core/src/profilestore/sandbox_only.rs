// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A sandboxed chat that runs only on a sandbox-only profile waits for one** (#2037, the
//! operator's ruling of 2026-10-15; Claude Code on Linux, ADR 0067 as amended 2026-11-10). A
//! sandboxed Codex chat, at the floor or the full sandbox, and on Linux a sandboxed Claude Code
//! chat, runs only on a profile this machine keeps for sandboxed chats only
//! ([`crate::start::profile_refusal`]), so no sandboxed chat writes a harness home an
//! unsandboxed run uses. A start refused for want of one leaves one wait per project and
//! harness ([`waits_on`]), which the window shows as one Ask, *Codex needs a sandbox-only
//! profile here* or *Claude Code needs a sandbox-only profile here* ([`ask`]): its button makes
//! one ([`make`]) and opens the harness's own sign-in in it. The wait goes once this machine has
//! a sandbox-only profile of that harness, or the person dismisses it.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use super::{Homes, MachineProfile, Made};
use crate::harness::Harness;

/// The name [`make`] gives the first sandbox-only profile of `harness`: `codex-sandbox` or
/// `claude-sandbox`, and the next is `codex-sandbox-2`.
pub fn name(harness: Harness) -> String {
    format!("{}-sandbox", harness.name())
}

/// What a person calls `harness` in a sentence.
pub fn called(harness: Harness) -> &'static str {
    match harness {
        Harness::ClaudeCode => "Claude Code",
        Harness::Codex => "Codex",
        Harness::Opencode => "opencode",
    }
}

/// What the window's Ask for `harness` says first: *Codex needs a sandbox-only profile here*.
pub fn ask(harness: Harness) -> String {
    format!("{} needs a sandbox-only profile here", called(harness))
}

/// **A project a sandboxed chat of `harness` was refused in**, for want of a sandbox-only
/// profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wait {
    /// The project.
    pub root: PathBuf,
    /// The harness the chat was.
    pub harness: Harness,
    /// When the first start was refused, in seconds since 1970.
    pub since: u64,
}

static WAITS: Mutex<Vec<Wait>> = Mutex::new(Vec::new());

/// Records that a sandboxed chat of `harness` in the project at `root` was refused for want of
/// a sandbox-only profile: one wait per project and harness, however many starts were refused.
pub fn waits_on(root: &Path, harness: Harness) {
    let mut all = WAITS.lock().unwrap_or_else(PoisonError::into_inner);
    if all.iter().any(|w| w.root == root && w.harness == harness) {
        return;
    }
    all.push(Wait {
        root: root.to_path_buf(),
        harness,
        since: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs()),
    });
}

/// **What the project at `root` waits for**, given this machine's `profiles`: a harness's wait
/// goes once one of them is a sandbox-only profile of it.
pub fn waiting(root: &Path, profiles: &[MachineProfile]) -> Vec<Wait> {
    let mut all = WAITS.lock().unwrap_or_else(PoisonError::into_inner);
    all.retain(|w| {
        !profiles
            .iter()
            .any(|p| p.sandbox_only && p.harness == w.harness.name())
    });
    all.iter().filter(|w| w.root == root).cloned().collect()
}

/// The person put the wait for `harness` in the project at `root` away.
pub fn dismiss(root: &Path, harness: Harness) {
    WAITS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .retain(|w| w.root != root || w.harness != harness);
}

/// **Makes a sandbox-only profile of `harness`** at `now`, named [`name`], or the first of
/// `-2`, `-3`, … after it no profile has: the record, ready to be signed in through the
/// harness's own sign-in. Nothing is copied into it.
pub fn make(homes: &Homes, harness: Harness, now: u64) -> Result<Made, String> {
    let first = name(harness);
    let taken = super::list(homes.config_root())?;
    let named = (1..)
        .map(|n| {
            if n == 1 {
                first.clone()
            } else {
                format!("{first}-{n}")
            }
        })
        .find(|name| {
            !taken.iter().any(|p| &p.name == name)
                && std::fs::symlink_metadata(homes.folder_of(name)).is_err()
        })
        .unwrap_or_else(|| first.clone());
    super::create_sandbox_only(homes, &named, harness.name(), now)
}

#[cfg(test)]
mod tests;

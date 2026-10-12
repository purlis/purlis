// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What the window asks about a clone's remote, and what each answer does** (#1848, #1852): the
//! half of the push record ([`super::PUSH_REMOTES`]) the person sees.
//!
//! An automatic save pushes with the person's purlis sign-in only to the remote recorded for
//! the clone ([`super::helper_for_push`]). Where it cannot, the window says so as a Notice with
//! the remote named, and the person's press is what records it. Nothing here is asked from a
//! chat: every call takes the host's [`HostScope`], which a chat's process tree cannot claim.
//!
//! The question is read from what is true now (the clone's remote and the record), never kept
//! from the last save, so it goes the moment the record or the remote changes.

use std::path::Path;

use crate::forge::Forge;
use crate::forge::accounts::{self, State};
use crate::forge::route::HostScope;
use crate::worktree::git;

#[cfg(test)]
mod tests;

/// What the window asks about a clone's remote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Question {
    /// purlis has no remote recorded for the clone, and this machine has no forge CLI to push it
    /// with: its automatic saves wait until the person saves it once from the window, which
    /// confirms `now`, the remote's `<host>/<owner>/<repo>`.
    NotSavedYet { now: String },
    /// The clone's remote is `now`, and the one purlis last saved it to is `recorded` (D-254-6):
    /// automatic saves do not push until the person saves to `now` and remembers it
    /// ([`confirm`]), or keeps `recorded` ([`keep`]).
    Changed { recorded: String, now: String },
}

/// The question the window asks about `clone`, whose forge is `forge`: `None` when automatic
/// saves push it with purlis's sign-in already, or when purlis has no signed-in account on the
/// forge's host (the forge CLI's helper is all there is to push with then, as before).
pub fn question(
    _host: &HostScope,
    config_root: &Path,
    forge: &Forge,
    clone: &Path,
) -> Option<Question> {
    if !signed_in_on(config_root, &forge.host) {
        return None;
    }
    question_with(config_root, forge, clone, remote_now(clone)?, &|kind| {
        crate::forge::find_cli(kind.cli()).is_some()
    })
}

/// [`question`] for a clone whose remote is `now`, asking `has_cli` whether a kind's forge CLI
/// is on this machine.
pub(crate) fn question_with(
    config_root: &Path,
    forge: &Forge,
    clone: &Path,
    now: String,
    has_cli: &dyn Fn(crate::forge::Kind) -> bool,
) -> Option<Question> {
    if !signed_in_on(config_root, &forge.host) {
        return None;
    }
    let key = crate::sandbox::local::clone_key(clone);
    match super::read_push_remotes(config_root).get(&key) {
        Some(recorded) if *recorded == now => None,
        Some(recorded) => Some(Question::Changed {
            recorded: recorded.clone(),
            now,
        }),
        None if !has_cli(forge.kind) => Some(Question::NotSavedYet { now }),
        None => None,
    }
}

/// The person's press on the question: records the clone's remote as it is now as the one its
/// automatic saves push to, and answers it. Refused, with why, when the clone has no remote
/// purlis can read.
pub fn confirm(_host: &HostScope, config_root: &Path, clone: &Path) -> Result<String, String> {
    let now = remote_now(clone).ok_or_else(|| {
        "purlis could not read this repository's remote, so it recorded nothing".to_string()
    })?;
    record(config_root, clone, now)
}

/// The person's other press on [`Question::Changed`]: points the clone's `origin` back at the
/// remote recorded for it, over HTTPS, with no push URL of its own, so automatic saves push there
/// again. Answers the remote kept. Refused, with why, when none is recorded or git would not
/// change it.
pub fn keep(_host: &HostScope, config_root: &Path, clone: &Path) -> Result<String, String> {
    let key = crate::sandbox::local::clone_key(clone);
    let recorded = super::read_push_remotes(config_root)
        .remove(&key)
        .ok_or_else(|| "purlis has no remote recorded for this repository to keep".to_string())?;
    let url = format!("https://{recorded}");
    let set = git::run(clone, &["remote", "set-url", "origin", &url], git::READ)
        .map_err(|why| format!("purlis could not start git: {why}"))?;
    if !set.ok() {
        return Err(format!(
            "git would not point origin back at {recorded}: {}",
            set.err.trim()
        ));
    }
    // A push URL of its own would still send pushes by name elsewhere. Git refuses to delete
    // what is not there, which is the same answer.
    let _ = git::run(
        clone,
        &["remote", "set-url", "--push", "--delete", "origin", "."],
        git::READ,
    );
    match remote_now(clone) {
        Some(now) if now == recorded => Ok(recorded),
        _ => Err(format!(
            "origin still does not read as {recorded}: check it with git remote -v"
        )),
    }
}

/// Records `now` as the remote of `clone`, and answers it.
pub(crate) fn record(config_root: &Path, clone: &Path, now: String) -> Result<String, String> {
    let key = crate::sandbox::local::clone_key(clone);
    let mut recorded = super::read_push_remotes(config_root);
    recorded.insert(key, now.clone());
    super::write_push_remotes(config_root, &recorded)
        .map_err(|why| format!("purlis could not record the remote: {why}"))?;
    Ok(now)
}

/// Whether purlis has a signed-in account on exactly `host` (`host:port` included).
fn signed_in_on(config_root: &Path, host: &str) -> bool {
    let host = host.to_ascii_lowercase();
    let keyring = accounts::keyring_at(config_root);
    accounts::list(config_root, keyring.as_ref())
        .unwrap_or_default()
        .iter()
        .any(|account| account.state == State::SignedIn && account.host == host)
}

/// `<host>/<owner>/<repo>` of the clone's `origin` now, as git reads it.
pub(crate) fn remote_now(clone: &Path) -> Option<String> {
    let run = git::run(clone, &["remote", "get-url", "origin"], git::READ).ok()?;
    if !run.ok() {
        return None;
    }
    super::place_of(run.line().trim())
}

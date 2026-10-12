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
    question_with(config_root, forge, clone, &places_now(clone)?, &|kind| {
        crate::forge::find_cli(kind.cli()).is_some()
    })
}

/// [`question`] for a clone whose remote names `places` now ([`places_now`]), asking `has_cli`
/// whether a kind's forge CLI is on this machine.
pub(crate) fn question_with(
    config_root: &Path,
    forge: &Forge,
    clone: &Path,
    places: &[String],
    has_cli: &dyn Fn(crate::forge::Kind) -> bool,
) -> Option<Question> {
    if !signed_in_on(config_root, &forge.host) {
        return None;
    }
    let now = places.first()?.clone();
    let key = crate::sandbox::local::clone_key(clone);
    match super::read_push_remotes(config_root).get(&key) {
        Some(recorded) if places.iter().all(|place| place == recorded) => None,
        // The address that is not the recorded one: a push URL of its own, when only that
        // moved.
        Some(recorded) => Some(Question::Changed {
            recorded: recorded.clone(),
            now: places
                .iter()
                .find(|place| *place != recorded)
                .cloned()
                .unwrap_or(now),
        }),
        None if !has_cli(forge.kind) => Some(Question::NotSavedYet { now }),
        None => None,
    }
}

/// The person's press on the question: records `shown`, the remote the window named to them, as
/// the one the clone's automatic saves push to, and answers it. Recorded only while the clone's
/// remote is still `shown`: one that changed again after the window asked is never recorded on
/// a press that named another. Refused, with why, otherwise.
pub fn confirm(
    _host: &HostScope,
    config_root: &Path,
    clone: &Path,
    shown: &str,
) -> Result<String, String> {
    confirm_seen(config_root, clone, shown, remote_now(clone))
}

/// [`confirm`], for a clone whose remote reads as `now` (`None` when it names no one address).
pub(crate) fn confirm_seen(
    config_root: &Path,
    clone: &Path,
    shown: &str,
    now: Option<String>,
) -> Result<String, String> {
    let now = now.ok_or_else(|| {
        "purlis could not read one address for this repository's remote (it may push to \
         another than it fetches from), so it recorded nothing. Give origin one address, or \
         keep the one purlis saved to"
            .to_string()
    })?;
    if now != shown {
        return Err(
            "The remote changed again since the window asked, so nothing was recorded. Look at \
             it and choose again"
                .to_string(),
        );
    }
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

/// `<host>/<owner>/<repo>` of the clone's `origin` now, when every address it names (its push
/// URL too) is that one. `None` when git cannot say, or the remote fetches from one place and
/// pushes to another.
pub(crate) fn remote_now(clone: &Path) -> Option<String> {
    let places = places_now(clone)?;
    let first = places.first()?;
    places
        .iter()
        .all(|place| place == first)
        .then(|| first.clone())
}

/// Every address the clone's `origin` names now, as git reads them (its rewrites applied): its
/// URLs first, then its push URLs, each as `<host>/<owner>/<repo>`, or as it is written where it
/// is not a forge's. `None` when git cannot say (no repository, no `origin`).
pub(crate) fn places_now(clone: &Path) -> Option<Vec<String>> {
    let mut places = Vec::new();
    for asked in [
        &["remote", "get-url", "--all", "origin"][..],
        &["remote", "get-url", "--push", "--all", "origin"][..],
    ] {
        let run = git::run(clone, asked, git::READ).ok()?;
        if !run.ok() {
            return None;
        }
        places.extend(
            run.out
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(|url| super::place_of(url).unwrap_or_else(|| url.to_owned())),
        );
    }
    (!places.is_empty()).then_some(places)
}

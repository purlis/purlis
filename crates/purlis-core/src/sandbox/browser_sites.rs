// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The project's browser sites** (#2443, ruled 2026-10-17): `[sandbox] browser-sites` in the
//! committed file, the sites every chat's browser of everyone in the project may open, send to
//! and type on. Each entry is written as a host of `[sandbox] hosts` is ([`super::hosts::Host`]).
//!
//! - **For browsers alone.** A browser's judge and its reach proxy read them; a chat's own
//!   commands never do, so they still need their own Allow for that host.
//! - **Shown once on each machine before it applies there.** A site a teammate's change adds
//!   reaches no browser on a machine until the person there was shown it, site by site, in the
//!   Notice of the project's browser sites; this machine records each one shown
//!   (`browser_sites_seen` in its sandbox-grants file). A site removed here is forgotten at
//!   once; one a teammate's change takes out is forgotten once the Notice telling of its removal
//!   is read. Either way, putting it back after that is told again.
//! - **Held to an administrator's policy.** Where policy leaves out *everyone in the project*,
//!   or never allows a site's host, it is neither told nor in force.
//! - **Your own Allow is no news to you.** A site this machine's window writes is recorded as
//!   seen here.

use std::io;
use std::path::Path;

use super::Plane;
use super::grant::{Level, What};
use super::hosts::Host;
use super::policy::Locks;

/// Its key under `[sandbox]` in the committed file.
pub const KEY: &str = "browser-sites";

/// How the project's browser sites changed since this machine last showed them: each spelled as
/// the sandbox writes a host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    /// Listed now and never shown here: none of them reaches a browser here yet.
    pub added: Vec<String>,
    /// Shown here before and no longer listed.
    pub removed: Vec<String>,
}

/// **The browser sites the committed file lists that policy lets reach a browser**, in file
/// order: none where the project's chats are not sandboxed.
pub fn committed(root: &Path, locks: &Locks) -> Vec<Host> {
    let plane = Plane::read(root);
    if plane.in_force(locks).is_none() {
        return Vec::new();
    }
    plane
        .browser_sites()
        .map(|listed| listed.hosts)
        .unwrap_or_default()
        .into_iter()
        .filter(|host| {
            locks
                .refuses_grant(&What::Host(host.clone()), Level::Project)
                .is_none()
        })
        .collect()
}

/// Every site the committed file lists, policy or not, where the project's chats are sandboxed.
fn listed(root: &Path, locks: &Locks) -> Vec<String> {
    let plane = Plane::read(root);
    if plane.in_force(locks).is_none() {
        return Vec::new();
    }
    plane
        .browser_sites()
        .map(|listed| listed.hosts.iter().map(ToString::to_string).collect())
        .unwrap_or_default()
}

/// **The project's browser sites in force on this machine**: [`committed`], less each one the
/// person here was not shown yet.
pub fn in_force(root: &Path, locks: &Locks) -> Vec<Host> {
    let seen = super::local::browser_sites_seen(root);
    committed(root, locks)
        .into_iter()
        .filter(|host| seen.contains(&host.to_string()))
        .collect()
}

/// **What the Notice of the project's browser sites tells**: the sites added and never shown
/// here, and those shown and taken out since. `None` when there is neither.
pub fn changed(root: &Path, locks: &Locks) -> Option<Change> {
    // Where policy leaves out everyone in the project, nothing of it is told: none reaches.
    if locks.scope_removed(Level::Project).is_some() {
        return None;
    }
    // Nor where the project's sandbox is not in force, where none counts (#2691): a site seen
    // here is not told as taken out, and stays seen for when it counts again.
    Plane::read(root).in_force(locks)?;
    let now: Vec<String> = committed(root, locks)
        .iter()
        .map(ToString::to_string)
        .collect();
    let listed = listed(root, locks);
    let seen = super::local::browser_sites_seen(root);
    let added: Vec<String> = now
        .iter()
        .filter(|one| !seen.contains(one))
        .cloned()
        .collect();
    // Taken out of the file: one policy holds is still listed, and is not told as taken out.
    let removed: Vec<String> = seen
        .iter()
        .filter(|one| !listed.contains(one))
        .cloned()
        .collect();
    (!added.is_empty() || !removed.is_empty()).then_some(Change { added, removed })
}

/// **The person was shown `shown`**, sites the Notice named as added: each one the project
/// still lists is in force here from now on, and each one shown before and no longer listed is
/// forgotten.
pub fn acknowledge(root: &Path, shown: &[String]) -> io::Result<()> {
    let now: Vec<String> = committed(root, &Locks::of(root))
        .iter()
        .map(ToString::to_string)
        .collect();
    let mut seen: Vec<String> = super::local::browser_sites_seen(root)
        .into_iter()
        .filter(|one| now.contains(one))
        .collect();
    for one in shown {
        if now.contains(one) && !seen.contains(one) {
            seen.push(one.clone());
        }
    }
    super::local::keep_browser_sites_seen(root, seen)
}

/// **Lets every chat's browser of everyone in the project reach `site`**: written to the
/// committed file, and recorded as seen on this machine. A browser's Ask, never a chat.
pub fn grant(root: &Path, site: &Host) -> Result<(), String> {
    let written = site.to_string();
    crate::settings::browser_sites::edit(root, |list| {
        let there = list
            .iter()
            .any(|one| one.as_str().and_then(|one| Host::parse(one).ok()).as_ref() == Some(site));
        if !there {
            list.push(written.clone());
        }
        !there
    })?;
    let mut seen = super::local::browser_sites_seen(root);
    if !seen.contains(&written) {
        seen.push(written);
        super::local::keep_browser_sites_seen(root, seen).map_err(|why| {
            format!("{site} was written, and could not be recorded as seen here: {why}")
        })?;
    }
    Ok(())
}

/// **Takes `site` out of the committed file**: every entry naming it, and the key with its
/// last. It is forgotten as seen here, so a change that puts it back is shown again before it
/// applies. Answers whether one was there.
pub fn revoke(root: &Path, site: &Host) -> Result<bool, String> {
    let was = crate::settings::browser_sites::edit(root, |list| {
        let before = list.len();
        list.retain(|one| {
            one.as_str().and_then(|one| Host::parse(one).ok()).as_ref() != Some(site)
        });
        list.len() != before
    })?;
    let written = site.to_string();
    let mut seen = super::local::browser_sites_seen(root);
    if seen.contains(&written) {
        seen.retain(|one| *one != written);
        super::local::keep_browser_sites_seen(root, seen).map_err(|why| {
            format!("{site} was removed, and could not be forgotten as seen here: {why}")
        })?;
    }
    Ok(was)
}

#[cfg(test)]
#[path = "browser_sites_tests.rs"]
mod tests;

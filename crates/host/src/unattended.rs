// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A runner left without a link suspends itself** (#2051, spec #2041 story 39).
//!
//! A runner's chats keep working while its desktop is away: a link that ends leaves every
//! program running (a laptop closed overnight stops nothing). But work nobody comes back for
//! must not run for ever, so a runner's host that has had no link from its desktop for
//! [`UNLINKED_FOR`] suspends itself: it says so, and stops serving, which ends every program it
//! runs as a quit would and ends the runner's process, so its container stops and keeps its
//! home volume. The desktop starts it again as it next links to it.
//!
//! **A link is any link from the desktop** (`remote-link`): the window's relayed link, the kill
//! switch's, a pool's link check. Its liveness pings end a link whose other end went quiet
//! (ADR 0078, amended 2026-10-14), so a desktop that slept is no link.
//!
//! Only a runner's host suspends: a host with no desktop paired with it never does, and the
//! window's own host lives as `crate::lifetime` says.

use std::collections::HashSet;
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use purlis_session_protocol::auth::Scope;
use purlis_session_protocol::session::Peer;
use tokio::sync::Notify;

use crate::hosting::Hosting;

/// How long a runner's host goes without a link from its desktop before it suspends itself: a
/// day, the spec's default (#2041).
pub const UNLINKED_FOR: Duration = Duration::from_secs(24 * 60 * 60);

/// The desktop's links to this host, and since when it has had none.
pub(crate) struct Links {
    held: Mutex<Held>,
    changed: Notify,
}

struct Held {
    open: HashSet<u64>,
    /// Since when no link has been open: from the host's start, until the first.
    none_since: Option<Instant>,
}

impl Default for Links {
    fn default() -> Links {
        Links {
            held: Mutex::new(Held {
                open: HashSet::new(),
                none_since: Some(Instant::now()),
            }),
            changed: Notify::new(),
        }
    }
}

impl Links {
    fn held(&self) -> MutexGuard<'_, Held> {
        self.held
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// `peer` sent something: a link from the desktop is open.
    pub(crate) fn heard(&self, peer: &Peer) {
        if peer.scope() != Scope::RemoteLink || peer.has_ended() {
            return;
        }
        let mut held = self.held();
        if held.open.insert(peer.link()) {
            held.none_since = None;
            drop(held);
            self.changed.notify_one();
        }
    }

    /// `peer`'s link ended.
    pub(crate) fn ended(&self, peer: &Peer) {
        let mut held = self.held();
        if held.open.remove(&peer.link()) && held.open.is_empty() {
            held.none_since = Some(Instant::now());
            drop(held);
            self.changed.notify_one();
        }
    }

    /// When this host will have gone `after` without a link, while it has none.
    fn due(&self, after: Duration) -> Option<Instant> {
        self.held().none_since.map(|since| since + after)
    }
}

impl Hosting {
    /// **Resolves once this host has gone `after` with no link from its desktop**, while
    /// `paired` says a desktop is paired with it (#2051): what a runner's `purlis serve` serves
    /// until, so it suspends itself. Read again as each link comes or goes; a host not paired
    /// when the time comes waits on.
    pub async fn until_unlinked_for(
        &self,
        after: Duration,
        paired: impl Fn() -> bool + Send + Sync + 'static,
    ) {
        let paired = std::sync::Arc::new(paired);
        loop {
            let links = self.unattended_links();
            let waiting = links.changed.notified();
            match links.due(after) {
                Some(due) if Instant::now() >= due => {
                    let asking = std::sync::Arc::clone(&paired);
                    let paired_now = tokio::task::spawn_blocking(move || asking())
                        .await
                        .unwrap_or(false);
                    if paired_now {
                        crate::say(&format!(
                            "no desktop has linked to this runner for {}, so it suspends \
                             itself: every chat it runs ends, its home is kept, and its desktop \
                             starts it again as it next links to it",
                            spelled(after)
                        ));
                        return;
                    }
                    // Not paired: looked at again after as long once more.
                    tokio::select! {
                        () = waiting => {}
                        () = tokio::time::sleep(after) => {}
                    }
                }
                Some(due) => {
                    tokio::select! {
                        () = waiting => {}
                        () = tokio::time::sleep_until(due.into()) => {}
                    }
                }
                None => waiting.await,
            }
        }
    }
}

/// `after` as the log says it: whole hours in hours, and anything else in minutes, or seconds
/// below a minute or where it is no whole number of minutes (#3471: a pool's own time).
fn spelled(after: Duration) -> String {
    let secs = after.as_secs();
    let (n, one, many) = if secs >= 3600 && secs.is_multiple_of(3600) {
        (secs / 3600, "hour", "hours")
    } else if secs >= 60 && secs.is_multiple_of(60) {
        (secs / 60, "minute", "minutes")
    } else {
        (secs, "second", "seconds")
    };
    format!("{n} {}", if n == 1 { one } else { many })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_time_a_runner_was_left_is_said_in_hours_or_minutes() {
        let said = |secs| spelled(Duration::from_secs(secs));
        assert_eq!(said(24 * 3600), "24 hours");
        assert_eq!(said(3600), "1 hour");
        assert_eq!(said(90 * 60), "90 minutes");
        assert_eq!(said(30 * 60), "30 minutes");
        assert_eq!(said(60), "1 minute");
        assert_eq!(said(61), "61 seconds");
    }

    #[test]
    fn a_host_with_no_link_is_due_from_its_start_and_one_with_a_link_is_never_due() {
        let links = Links::default();
        assert!(links.due(Duration::from_secs(60)).is_some());
        links.held().open.insert(1);
        links.held().none_since = None;
        assert_eq!(links.due(Duration::from_secs(60)), None);
    }
}

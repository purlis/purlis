// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The window hands its host's supervision to the service manager** (FD-17, #655; ADR 0068
//! §3, D-2007-1 as #2007 left it for this ticket).
//!
//! While something keeps the session host running (a trigger or a paired device), the host is a
//! login item: a launchd agent or a systemd user unit starts it, and starts it again after a
//! failure. The window then starts no host of its own. It installs the login item, asks the
//! manager to start the host, waits for it to answer on its socket, and connects as it connects
//! to any host. A link that ends is the manager's to mend: the window connects again once the
//! host answers.
//!
//! **One breaker, whichever manager.** systemd holds the host stopped after three failures
//! within five minutes (`StartLimitBurst`, `StartLimitIntervalSec`), and the window reads that
//! from the unit ([`Manager::tripped`]). launchd throttles and never gives up, so the window's
//! own breaker counts the links it lost, by the same limits, and tells launchd to stop
//! ([`Manager::hold`]). Either way the window raises the same Notice (`session-host-failing`),
//! and Try again lets go of the hold and starts the host once more.

use std::path::{Path, PathBuf};
use std::sync::{Arc, PoisonError};
use std::time::{Duration, Instant};

use purlis_core::loginitem::Manager;

use super::{LINES_KEPT, LOOK_EVERY, Supervision, breaker, connect};
use crate::hostedsessions::Connection;

/// **The supervisor while the service manager runs the host**, on the host's own thread: the
/// login item installed and its host started, the window connected once it answers, and again
/// after each link that ends, until a breaker trips; then the Notice, and the person's Try
/// again. Nothing more once the window is ending.
pub(super) fn supervise(
    place: &purlis_host::Place,
    manager: &Arc<dyn Manager>,
    log: Option<&Path>,
    answers_within: Duration,
    connection: &Arc<Connection>,
    supervision: &Arc<Supervision>,
) {
    loop {
        if supervision.ending() {
            return;
        }
        match manager.install().and_then(|()| manager.start()) {
            Err(why) => {
                tracing::warn!("purlis: the service manager did not start the session host: {why}");
                connection.gone(format!("the service manager did not start it: {why}"));
            }
            Ok(()) if answers_in_time(place, supervision, answers_within) => {
                tracing::info!(
                    "purlis: the session host the service manager runs serves at {}",
                    place.socket.display()
                );
                connect(
                    place.socket.clone(),
                    place.credentials.clone(),
                    Arc::clone(connection),
                    Arc::clone(supervision),
                );
                if !super::wait_for_its_link_to_end(supervision) {
                    return;
                }
            }
            Ok(()) => {
                if supervision.ending() {
                    return;
                }
                tracing::warn!("purlis: the session host the service manager runs did not answer");
                connection.gone("the session host the service manager runs did not answer");
            }
        }
        if supervision.ending() {
            return;
        }
        connection.restarting();
        let after = if manager.tripped() {
            breaker::After::Tripped
        } else {
            supervision
                .breaker
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .exited(Instant::now())
        };
        if after == breaker::After::StartAgain {
            tracing::warn!(
                "purlis: the session host ended unexpectedly; its service manager starts it again"
            );
            continue;
        }
        if let Err(why) = manager.hold() {
            tracing::warn!("purlis: the service manager was not told to stop the host: {why}");
        }
        let said = log.map(last_lines).unwrap_or_default();
        super::say_it_is_failing(connection, supervision, said);
        if !super::wait_to_try_again(supervision) {
            return;
        }
        if let Err(why) = manager.try_again() {
            tracing::warn!("purlis: the service manager did not start the host again: {why}");
        }
        super::tried_again(connection, supervision);
    }
}

/// Whether the host answers on its socket within `within`, looked at every [`LOOK_EVERY`]; not
/// once the window is ending.
fn answers_in_time(
    place: &purlis_host::Place,
    supervision: &Supervision,
    within: Duration,
) -> bool {
    let deadline = Instant::now() + within;
    loop {
        if supervision.ending() {
            return false;
        }
        if purlis_host::answers(&place.socket) {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(LOOK_EVERY);
    }
}

/// The last lines of the host's log, oldest first, where launchd writes what it says: what the
/// Notice shows. A log too large is read from its end only.
fn last_lines(log: &Path) -> Vec<String> {
    use std::io::{Read as _, Seek as _, SeekFrom};
    const TAIL: u64 = 16 * 1024;
    let Ok(mut file) = std::fs::File::open(log) else {
        return Vec::new();
    };
    let length = file.metadata().map(|meta| meta.len()).unwrap_or(0);
    if file
        .seek(SeekFrom::Start(length.saturating_sub(TAIL)))
        .is_err()
    {
        return Vec::new();
    }
    let mut tail = Vec::new();
    if file.take(TAIL).read_to_end(&mut tail).is_err() {
        return Vec::new();
    }
    let text = String::from_utf8_lossy(&tail);
    let lines: Vec<String> = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(str::to_owned)
        .collect();
    lines[lines.len().saturating_sub(LINES_KEPT)..].to_vec()
}

/// Where launchd writes what a host that is a login item says (`StandardErrorPath`): beside the
/// app's own log. systemd keeps it in the journal.
pub fn log_here() -> Option<PathBuf> {
    purlis_core::applog::app_log_dir().map(|dir| dir.join("session-host.log"))
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **`purlis serve` hears `SIGTERM` and `SIGINT` as a quit** (#6700; ADR 0068 §3 as amended for
//! #6700).
//!
//! A runner's host is kept by systemd or launchd (RR-15, `purlis_core::runnerhost`), and either
//! one stops it with `SIGTERM`: at the machine's shutdown, at `purlis serve --remove`, at
//! `systemctl --user stop`, at a bootout. A person at a terminal stops a bare `purlis serve` with
//! Ctrl-C, `SIGINT`. A planned stop is a quit, so the host ends each of its programs as a quit
//! ends it (`Stopped::Quit`, its run ended `stopped`), empties its host record once each end is
//! kept, and exits 0. Only a host killed outright (`SIGKILL`, a power loss) leaves its record for
//! the next host to settle as a crash.
//!
//! The handlers are installed as [`told_to_stop`] is called, before the host says it is ready, so
//! a signal sent the moment it serves is heard and never ends the process by default.

use tokio::signal::unix::{SignalKind, signal};

/// What the host says on standard error, in its journal, as it hears a stop, after the signal's
/// name: `told to stop (SIGTERM); every program ends …`.
pub const TOLD_TO_STOP: &str = "every program ends as at a quit, and the host exits";

/// Resolves once this process is sent `SIGTERM` or `SIGINT`, saying [`TOLD_TO_STOP`] as it does.
/// Both are handled from this call on, for the rest of the process's life: neither ends it any
/// more. Called inside a Tokio runtime.
pub fn told_to_stop() -> std::io::Result<impl Future<Output = ()> + Send + use<>> {
    let mut terminate = signal(SignalKind::terminate())?;
    let mut interrupt = signal(SignalKind::interrupt())?;
    Ok(async move {
        let by = tokio::select! {
            _ = terminate.recv() => "SIGTERM",
            _ = interrupt.recv() => "SIGINT",
        };
        crate::say(&format!("told to stop ({by}); {TOLD_TO_STOP}"));
    })
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **`purlis serve` hears `SIGTERM` as a quit** (#6700; ADR 0068 §3 as amended for #6700).
//!
//! A runner's host is kept by systemd or launchd (RR-15, `purlis_core::runnerhost`), and either
//! one stops it with `SIGTERM`: at the machine's shutdown, at `purlis serve --remove`, at
//! `systemctl --user stop`, at a bootout. A planned stop is a quit, so the host ends each of its
//! programs as a quit ends it (`Stopped::Quit`, its run ended `stopped`), empties its host
//! record, and exits 0. Only a host killed outright (`SIGKILL`, a power loss) leaves its record
//! for the next host to settle as a crash.
//!
//! The handler is installed as [`told_to_stop`] is called, before the host says it is ready, so a
//! `SIGTERM` sent the moment it serves is heard and never ends the process by default.

/// What the host says on standard error, in its journal, as it hears `SIGTERM`.
pub const TOLD_TO_STOP: &str =
    "told to stop (SIGTERM); every program ends as at a quit, and the host exits";

/// Resolves once this process is sent `SIGTERM`, saying [`TOLD_TO_STOP`] as it does. `SIGTERM`
/// is handled from this call on, for the rest of the process's life: it no longer ends it. Called
/// inside a Tokio runtime.
pub fn told_to_stop() -> std::io::Result<impl Future<Output = ()> + Send + use<>> {
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    Ok(async move {
        terminate.recv().await;
        crate::say(TOLD_TO_STOP);
    })
}

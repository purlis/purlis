// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The one gate every program charter starts goes through, so that a descriptor which is not
//! close-on-exec yet cannot escape into a child.
//!
//! The fork lock itself, and why it exists (charter-app#53), is
//! [`purlis_terminal::forklock`]'s: the crate that opens terminals holds it (#1995). Opening a
//! terminal and making descriptors in two steps take it here as they do there, and
//! [`spawn`], [`output`] and [`status`] start a program under it.
//!
//! A lock cannot guard a call that does not take it, so `clippy.toml` disallows
//! [`std::process::Command`]'s own `spawn`, `output` and `status` in this crate. [`spawn`],
//! [`output`] and [`status`] here are the allowed way.
//!
//! # The gate's second step: the session bus
//!
//! Because every program charter starts comes through here, this is also the one place a rule
//! about what EVERY such program gets can hold without a call site forgetting it. There is one:
//! [`spawn`] first calls [`on_the_apps_session_bus`], in the open, before it takes the lock. In
//! an app that started without the session bus (charter#746) it gives the program the bus the
//! app was given, or no address — never the one the app points at nothing. Anywhere else it
//! changes nothing. It is a step of its own, named and documented, so the lock stays about the
//! lock.

use std::io;
use std::process::{Child, Command, ExitStatus, Output, Stdio};

pub use purlis_terminal::forklock::{while_a_terminal_is_opened, while_descriptors_are_made};

/// Starts `command`, as [`Command::spawn`] does, in two steps: **on the app's session bus**
/// ([`on_the_apps_session_bus`], which changes `command`'s environment only in an app running
/// without the bus), then **without a terminal half-open anywhere** (the fork lock,
/// [`purlis_terminal::forklock::spawn`]). [`output`] and [`status`] start their programs here,
/// so they take both steps too.
pub fn spawn(command: &mut Command) -> io::Result<Child> {
    on_the_apps_session_bus(command);
    purlis_terminal::forklock::spawn(command)
}

/// Puts `command` on the session bus this process was given, when it is an app running without
/// it: [`crate::chatenv::onto_the_kept_bus`], with this process's own
/// `DBUS_SESSION_BUS_ADDRESS` and `CHARTER_SESSION_BUS_KEPT`. On a process with its bus, which
/// is every `charter` and every app that found its portal, it leaves `command` as it was.
pub fn on_the_apps_session_bus(command: &mut Command) -> &mut Command {
    crate::chatenv::onto_the_kept_bus(
        command,
        std::env::var_os(crate::chatenv::SESSION_BUS).as_deref(),
        crate::envvar::var_os(crate::chatenv::SESSION_BUS_KEPT).as_deref(),
    );
    command
}

/// Runs `command` to the end and collects what it wrote, as [`Command::output`] does.
///
/// The lock is released as soon as the program has been started, and never held across the
/// wait: a `git` that takes a minute must not be a minute in which no chat can open a
/// terminal. Both pipes are read while waiting, so neither can fill and stop the program.
///
/// `stdin` is closed, which is what [`Command::output`] does when a caller sets none. A caller
/// that means to feed a program uses [`spawn`] and waits itself.
pub fn output(command: &mut Command) -> io::Result<Output> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    spawn(command)?.wait_with_output()
}

/// Runs `command` to the end on charter's own streams, as [`Command::status`] does. The lock
/// is released before the wait, for the reason [`output`] gives.
pub fn status(command: &mut Command) -> io::Result<ExitStatus> {
    spawn(command)?.wait()
}

#[cfg(all(test, unix))]
pub(crate) mod tests {
    use std::sync::{Mutex, MutexGuard, PoisonError};

    /// The fork lock's own tests are in `purlis_terminal::forklock`, in a test binary of their
    /// own; this serialises the tests in this one that hold the lock as a fork does.
    static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

    /// Hold the fork lock as a fork does, for a test elsewhere that asks what waits for one.
    /// Taken under [`alone`].
    pub(crate) fn as_a_fork_does() -> std::sync::RwLockReadGuard<'static, ()> {
        purlis_terminal::forklock::held_as_a_fork_holds_it()
    }

    /// Held for as long as a test is holding the fork lock as a fork does. The poison is taken
    /// rather than unwrapped: it guards no data.
    pub(crate) fn alone() -> MutexGuard<'static, ()> {
        ONE_AT_A_TIME.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

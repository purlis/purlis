// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Native crashes** (#5691, spec #5683, ADR 0083 ruling O9): the app watched by its crash
//! handler, and what that handler kept sent on the next launch.
//!
//! The panic hook (`panics.rs`) sees a Rust panic and nothing else: a segfault, a fault on a
//! stack's guard page or an uncaught Objective-C exception takes the process down before it can
//! write a word. So [`watch`] starts this binary again as the crash handler (`purlis-crash-
//! handler`), a companion process that writes a minidump of the app from outside when it
//! crashes, into the crashes folder of this machine's config home. On the next launch
//! [`report_waiting`] hands each to the core's Reports, which takes the crashing thread's stack
//! out of it on this device and sends that alone, with a yes to Crash reports, to the Report
//! intake alone (the operator's ruling (b) of 2026-11-10 on #5691: module names and offsets, no
//! memory, no other thread, no environment). The minidump itself stays here and ages out.
//!
//! **macOS only, for now.** Linux and Windows are follow-ups of #5691: the handler's crates
//! cover both, but neither has been proved here.
//!
//! **Started** (#5691, the operator's approval of 2026-11-10: *"go ahead end to end - to be all
//! enabled and working all without issues"*), on ruling (b) above and purlis's own glue over
//! `crash-handler` 0.8, which lets a crashing child process die. In the same change the release
//! profile began writing debug symbols (ADR 0092, amended 2026-11-10, #5692):
//! `debug = "line-tables-only"` and `split-debuginfo = "packed"`, keeping `strip = true`, so a
//! dSYM lands beside each binary for `release.yml`'s `symbols` job to send to the Report intake,
//! which symbolicates the stack's frames with it. `tools/crash-relaunch.mjs` is the end-to-end
//! check.
//!
//! **macOS's "quit unexpectedly" dialog still appears** on a native crash, and that is expected:
//! the handler passes the crash on to the system once the minidump is written, and does not
//! `_exit` to hide it as `minidumper-child` did (ADR 0083, amended 2026-11-10).

use std::sync::OnceLock;

/// Whether the app starts its crash handler: on since the operator's approval of 2026-11-10
/// (#5691; the module's doc says what came with it). Off again is one change, here.
pub const STARTED: bool = true;

/// What became of the watch, for the log once there is one: it is set before the log is.
static STATE: OnceLock<String> = OnceLock::new();

/// The app's watch, held for as long as it runs: dropping it would stop the handler.
#[cfg(target_os = "macos")]
static WATCHING: OnceLock<purlis_crash_handler::Watching> = OnceLock::new();

/// Started again as the crash handler: serve until the app ends, and exit. Called first in
/// `main`, before anything of the app starts.
pub fn serve_if_asked() {
    #[cfg(target_os = "macos")]
    if purlis_core::envvar::var_os(purlis_crash_handler::ENV).is_some() {
        // A companion with nowhere to keep a minidump has nothing to do.
        let Some(crashes) = crashes_dir() else {
            std::process::exit(0);
        };
        // Serves, and exits: it never returns in the companion.
        let _ = purlis_crash_handler::watch(&crashes);
        std::process::exit(0);
    }
}

/// Watches this process with the crash handler, from now on. Called once, early in `run`, in
/// the process that is the app (not the one that only waits for it). The app runs on without a
/// handler that could not start, and says why in its log.
pub fn watch() {
    let said = if !STARTED {
        "switched off in this build (#5691)".to_owned()
    } else {
        start()
    };
    let _ = STATE.set(said);
}

#[cfg(target_os = "macos")]
fn start() -> String {
    let Some(crashes) = crashes_dir() else {
        return "not started: this machine has no config home".to_owned();
    };
    match purlis_crash_handler::watch(&crashes) {
        Ok(watching) => {
            let _ = WATCHING.set(watching);
            "watching".to_owned()
        }
        Err(why) => format!("not started, so a native crash would go unreported: {why}"),
    }
}

#[cfg(not(target_os = "macos"))]
fn start() -> String {
    "not on this system yet (#5691)".to_owned()
}

/// What became of the watch, in a few words, for the app's log.
pub fn state() -> String {
    STATE
        .get()
        .cloned()
        .unwrap_or_else(|| "not asked to start".to_owned())
}

/// Tells the crash handler this process panicked and its panic is reported, so the abort that
/// follows is not reported again as a native crash. Called from the panic hook: nothing here
/// may panic.
pub fn panicked() {
    #[cfg(target_os = "macos")]
    if let Some(watching) = WATCHING.get() {
        watching.panicked();
    }
}

/// Hands every minidump the crash handler kept to Reports, on a thread of its own: a launch does
/// not wait for the intake. `then` is told once they are all reported.
pub fn report_waiting(then: impl FnOnce() + Send + 'static) {
    let Some(crashes) = crashes_dir() else {
        then();
        return;
    };
    let started = std::thread::Builder::new()
        .name("native-crashes".into())
        .spawn(move || {
            let reports = purlis_core::report::Reports::for_a_crash();
            for outcome in purlis_core::report::native::report_waiting(&reports, &crashes) {
                tracing::info!("a native crash that waited: {}", said(&outcome));
            }
            then();
        });
    if let Err(why) = started {
        tracing::warn!("the native crashes that waited were not reported: {why}");
    }
}

/// What became of a crash, in a line of the log. Never the minidump.
fn said(outcome: &purlis_core::report::Outcome) -> String {
    use purlis_core::report::{Outcome, Refusal};
    match outcome {
        Outcome::Sent { address } => format!("sent, to {address}"),
        Outcome::Refused(Refusal::NoConsent(_)) => "not sent: Crash reports are off".to_owned(),
        Outcome::Refused(Refusal::NoIntake) => {
            "not sent: this build has no report service".to_owned()
        }
        Outcome::Queued => "kept for a later launch: the report service gave no answer".to_owned(),
        Outcome::Counted { repeats } => format!("counted, {repeats} times"),
        Outcome::Refused(Refusal::NotSent(why)) => format!("not sent: {why}"),
        Outcome::Refused(other) => format!("not sent: {other:?}"),
        Outcome::Drafted { .. } => "drafted".to_owned(),
    }
}

/// The folder the crash handler keeps minidumps in, or none on a machine with no config home.
fn crashes_dir() -> Option<std::path::PathBuf> {
    purlis_core::machine::config_root().map(|root| purlis_core::report::native::crashes_dir(&root))
}

/// In a scenario build alone: crash on purpose, the way `PURLIS_E2E_CRASH` says, a second after
/// the window is up (`tools/crash-relaunch.mjs`). `abort` ends the process as an uncaught
/// Objective-C exception does; `panic` panics as a shipped build does, which aborts.
#[cfg(feature = "e2e")]
pub fn crash_if_asked() {
    let Some(how) = purlis_core::envvar::var_os("PURLIS_E2E_CRASH") else {
        return;
    };
    let how = how.to_string_lossy().into_owned();
    let _ = std::thread::Builder::new()
        .name("e2e-crash".into())
        .spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(1));
            match how.as_str() {
                "abort" => std::process::abort(),
                "panic" => {
                    purlis_core::panicend::fatal();
                    panic!("a panic a scenario test asked for");
                }
                _ => {}
            }
        });
}

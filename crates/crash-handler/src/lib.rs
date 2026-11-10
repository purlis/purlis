// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The app's out-of-process crash handler** (#5691, spec #5683, ADR 0083 ruling O9).
//!
//! A panic hook sees a Rust panic and nothing else. A segfault, a fault on a stack's guard page,
//! an uncaught Objective-C exception (which ends in `abort`), or a `panicend` abort after the
//! panic hook ran, all take the process down before it can write anything about itself. So a
//! companion process does it: [`watch`] starts this same binary again as the handler, with
//! [`ENV`] set to the socket the two talk over, and attaches a crash handler to this process
//! that, on a crash, asks the companion to write a minidump of it from outside. The companion
//! keeps it in the crashes folder it was given, as `<seconds since 1970>-<id>.dmp`, and exits.
//! The app reads the folder on its next launch and hands each minidump to Reports, which takes
//! the crashing thread's stack out of it on the device and alone decides whether that leaves
//! (ADR 0118: only to the Report intake, never to GitHub).
//!
//! **A panic is reported once.** The panic hook reports a panic itself, with its place and
//! message, and then the process aborts, which the handler would catch as a native crash too. So
//! the hook tells the companion first ([`Watching::panicked`]), and a crash that follows within
//! [`PANIC_COVERS`] leaves no minidump.
//!
//! The glue is purlis's own (#5691, the operator's ruling of 2026-11-10), over EmbarkStudios'
//! `crash-handler` (the exception port and `SIGABRT` on macOS, the signal handlers on Linux, the
//! exception filter on Windows) and `minidumper` (the socket, and rust-minidump's
//! `minidump-writer`): [`companion`] is the server, [`attach`] the client. It was
//! `minidumper-child`'s, which pinned a `crash-handler` that hung a crashing child process.

mod attach;
mod companion;

use std::path::{Path, PathBuf};
use std::time::Duration;

/// Set on the companion process alone: this binary, started again as the crash handler. Its
/// value is the path of the socket the companion serves and the app connects to.
pub const ENV: &str = "PURLIS_CRASH_HANDLER";

/// How long after a panic was told a crash is that panic's own end, and leaves no minidump.
pub const PANIC_COVERS: Duration = Duration::from_secs(30);

/// The message that says the process panicked and its panic is reported.
const PANICKED: u32 = 1;

/// How long the companion serves without hearing from the app before it takes the app for gone
/// and exits. The app pings it twice as often.
const STALE_AFTER: Duration = Duration::from_secs(5);

/// A process the crash handler watches. Dropping it stops the watch, so the app holds it for as
/// long as it runs.
pub struct Watching {
    attached: attach::Attached,
}

impl std::fmt::Debug for Watching {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Watching")
    }
}

impl Watching {
    /// Tells the handler this process panicked and its panic is reported: the abort that
    /// follows leaves no minidump. Nothing here may panic: it runs in the panic hook.
    pub fn panicked(&self) {
        let _ = self.attached.client.send_message(PANICKED, []);
    }
}

/// Watches this process, keeping a crash's minidump in `crashes`. An error, in words, when the
/// handler could not be started: the app runs on without it.
///
/// Started again as the handler (with [`ENV`] set), it serves until the process it watches ends,
/// and exits without returning: so it is called before anything of the app starts.
pub fn watch(crashes: &Path) -> Result<Watching, String> {
    if let Some(socket) = std::env::var_os(ENV) {
        // A companion that could not serve must not go on to run the app a second time.
        let served = companion::serve(Path::new(&socket), crashes);
        std::process::exit(if served.is_ok() { 0 } else { 70 });
    }
    let socket = socket_path();
    let child = std::env::current_exe()
        .and_then(|me| {
            std::process::Command::new(me)
                .env(ENV, &socket)
                .stdin(std::process::Stdio::null())
                .spawn()
        })
        .map_err(|why| format!("the companion did not start: {why}"))?;
    attach::attach(&socket, child)
        .map(|attached| Watching { attached })
        .map_err(|why| error_chain(&why))
}

/// A socket of this run's alone: one companion serves one app.
fn socket_path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "purlis-crash-{}.sock",
        uuid::Uuid::new_v4().simple()
    ))
}

/// `why` and every error under it, in one line.
fn error_chain(why: &dyn std::error::Error) -> String {
    let mut said = why.to_string();
    let mut under = why.source();
    while let Some(cause) = under {
        said.push_str(": ");
        said.push_str(&cause.to_string());
        under = cause.source();
    }
    said
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The app's out-of-process crash handler** (#5691, spec #5683, ADR 0083 ruling O9).
//!
//! A panic hook sees a Rust panic and nothing else. A segfault, a fault on a stack's guard page,
//! an uncaught Objective-C exception (which ends in `abort`), or a `panicend` abort after the
//! panic hook ran, all take the process down before it can write anything about itself. So a
//! companion process does it: [`watch`] starts this same binary again as the handler, and
//! attaches a crash handler to this process that, on a crash, asks the companion to write a
//! minidump of it from outside. The companion keeps it in the crashes folder it was given, as
//! `<seconds since 1970>-<id>.dmp`, and exits. The app reads the folder on its next launch and
//! hands each minidump to Reports, which alone decides whether it leaves and where (only to the
//! Report intake, never to GitHub).
//!
//! **A panic is reported once.** The panic hook reports a panic itself, with its place and
//! message, and then the process aborts, which the handler would catch as a native crash too. So
//! the hook tells the companion first ([`Watching::panicked`]), and a crash that follows within
//! [`PANIC_COVERS`] leaves no minidump.
//!
//! The companion is [`minidumper_child`]'s, over EmbarkStudios' `crash-handler` and `minidumper`
//! and rust-minidump's `minidump-writer`: the exception port and `SIGABRT` on macOS, the
//! signal handlers on Linux, the exception filter on Windows.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Set on the companion process alone: this binary, started again as the crash handler.
pub const ENV: &str = "PURLIS_CRASH_HANDLER";

/// How long after a panic was told a crash is that panic's own end, and leaves no minidump.
pub const PANIC_COVERS: Duration = Duration::from_secs(30);

/// The message that says the process panicked and its panic is reported.
const PANICKED: u32 = 1;

/// A process the crash handler watches. Dropping it stops the watch, so the app holds it for as
/// long as it runs.
pub struct Watching {
    handle: minidumper_child::ClientHandle,
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
        let _ = self.handle.send_message(PANICKED, []);
    }
}

/// Watches this process, keeping a crash's minidump in `crashes`. An error, in words, when the
/// handler could not be started: the app runs on without it.
///
/// Started again as the handler (with [`ENV`] set), it serves until the process it watches ends,
/// and exits without returning: so it is called before anything of the app starts.
pub fn watch(crashes: &Path) -> Result<Watching, String> {
    let companion = std::env::var_os(ENV).is_some();
    let started = start(crashes);
    if companion {
        // A companion that could not serve must not go on to run the app a second time.
        std::process::exit(if started.is_ok() { 0 } else { 70 });
    }
    started
}

fn start(crashes: &Path) -> Result<Watching, String> {
    let told: Arc<Mutex<Option<Instant>>> = Arc::default();
    let heard = told.clone();
    let keep_in = crashes.to_path_buf();
    minidumper_child::MinidumperChild::new()
        .with_server_env_var(ENV.to_owned())
        // Written here first, and moved into `crashes` whole: the app never reads half a file.
        .with_crashes_dir(crashes.join("writing"))
        .on_message(move |kind, _| {
            if kind == PANICKED
                && let Ok(mut told) = heard.lock()
            {
                *told = Some(Instant::now());
            }
        })
        .on_minidump(move |minidump, _| {
            let a_panic = told
                .lock()
                .ok()
                .and_then(|told| *told)
                .is_some_and(|at| at.elapsed() <= PANIC_COVERS);
            if !a_panic {
                let _ = keep(&keep_in, &minidump);
            }
        })
        .spawn()
        .map(|handle| Watching { handle })
        .map_err(|why| error_chain(&why))
}

/// Keeps `minidump` in `crashes` as `<seconds since 1970>-<id>.dmp`, whole or not at all: the
/// folder this user's alone (`0700`) and the file too (`0600`), since a minidump holds the
/// crashed process's stacks.
fn keep(crashes: &Path, minidump: &[u8]) -> std::io::Result<PathBuf> {
    private_dir(crashes)?;
    let at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    let name = format!("{at}-{}", uuid::Uuid::new_v4().simple());
    let writing = crashes.join(format!("{name}.part"));
    write_private(&writing, minidump)?;
    let kept = crashes.join(format!("{name}.dmp"));
    std::fs::rename(&writing, &kept)?;
    Ok(kept)
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

#[cfg(unix)]
fn private_dir(dir: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
}

#[cfg(not(unix))]
fn private_dir(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)
}

#[cfg(unix)]
fn write_private(file: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(file)?
        .write_all(bytes)
}

#[cfg(not(unix))]
fn write_private(file: &Path, bytes: &[u8]) -> std::io::Result<()> {
    std::fs::write(file, bytes)
}

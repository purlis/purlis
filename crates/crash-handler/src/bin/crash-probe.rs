// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Test-only: `crash-probe <abort|abort-elsewhere|late-abort|overflow|panic|exit>` watches itself with the crash handler, as
//! the app does, keeping minidumps in the folder [`CRASHES`] names, and then ends the way it is
//! told. `child` starts this program
//! again, unwatched, to overflow its stack, and says how that child ended: exit 0 when it died of
//! its crash within 20 seconds, 3 when it was still running and had to be killed.

use std::path::PathBuf;

/// The crashes folder, from the environment and not an argument: the handler's companion is this
/// program started again with no arguments, and only the environment reaches it. Read from an
/// argument, the companion's folder was empty, so it kept the minidump in its working directory.
const CRASHES: &str = "CRASH_PROBE_CRASHES";

fn main() {
    let mut args = std::env::args().skip(1);
    let how = args.next().unwrap_or_default();
    if how == "unwatched-overflow" {
        deeper(std::hint::black_box(0));
    }
    let crashes = PathBuf::from(std::env::var_os(CRASHES).unwrap_or_default());
    let watching = match purlis_crash_handler::watch(&crashes) {
        Ok(watching) => Some(watching),
        Err(why) => panic!("the handler did not start: {why}"),
    };
    match how.as_str() {
        "abort" => std::process::abort(),
        // The same crash from another place, for a fingerprint that tells the two apart.
        "abort-elsewhere" => abort_elsewhere(),
        "late-abort" => {
            // Past the deadline the companion gives an app that never connects (#6152).
            std::thread::sleep(
                purlis_crash_handler::SERVED_WITHIN + std::time::Duration::from_secs(2),
            );
            std::process::abort()
        }
        "overflow" => {
            // Every frame is kept, so the stack runs out: a fault on its guard page.
            deeper(std::hint::black_box(0));
        }
        "panic" => {
            // As the app's panic hook does, before `panicend` aborts.
            if let Some(watching) = &watching {
                watching.panicked();
            }
            std::process::abort()
        }
        "child" => std::process::exit(a_crashing_child()),
        _ => {}
    }
}

/// Starts this program again, unwatched, to overflow its stack, and answers 0 when it died of
/// it, 3 when it was still running after 20 seconds.
fn a_crashing_child() -> i32 {
    let Ok(mut child) = std::env::current_exe().and_then(|me| {
        std::process::Command::new(me)
            .arg("unwatched-overflow")
            .spawn()
    }) else {
        return 2;
    };
    let until = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while std::time::Instant::now() < until {
        if let Ok(Some(_)) = child.try_wait() {
            return 0;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let _ = child.kill();
    let _ = child.wait();
    3
}

fn deeper(n: u64) -> u64 {
    if n == u64::MAX {
        return n;
    }
    let frame = std::hint::black_box([n; 64]);
    deeper(frame[0] + 1) + frame[1]
}

/// Aborts, from a frame of its own.
#[inline(never)]
fn abort_elsewhere() -> ! {
    std::hint::black_box(());
    std::process::abort()
}

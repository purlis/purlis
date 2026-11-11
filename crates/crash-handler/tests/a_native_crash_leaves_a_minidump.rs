// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A native crash leaves one minidump for the next launch** (#5691, spec #5683, ruling O9).
//!
//! Each case runs `crash-probe`, a program that watches itself with the handler and then crashes
//! the way it is told, and reads the folder the handler keeps its minidumps in.
//!
//! macOS only, as the app watches itself only there for now: Linux and Windows are follow-ups.
//! The handler binds a Unix socket, so a sandbox that refuses one cannot run these.

#![cfg(target_os = "macos")]

use std::path::{Path, PathBuf};
use std::process::Command;

fn crash(how: &str, crashes: &Path) -> std::process::ExitStatus {
    Command::new(env!("CARGO_BIN_EXE_crash-probe"))
        .arg(how)
        // The companion is the probe started again with no arguments: only the environment
        // reaches it, and it is the one that keeps the minidump.
        .env("CRASH_PROBE_CRASHES", crashes)
        .status()
        .expect("the probe ran")
}

fn minidumps(crashes: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(crashes) else {
        return Vec::new();
    };
    entries
        .map(|entry| entry.expect("an entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "dmp"))
        .collect()
}

fn is_a_minidump(path: &Path) -> bool {
    std::fs::read(path)
        .expect("the minidump")
        .starts_with(b"MDMP")
}

#[test]
fn a_process_that_aborts_leaves_one_minidump() {
    let crashes = tempfile::tempdir().unwrap();

    let ended = crash("abort", crashes.path());

    assert!(!ended.success(), "the probe crashed: {ended:?}");
    let left = minidumps(crashes.path());
    assert_eq!(left.len(), 1, "{left:?}");
    assert!(is_a_minidump(&left[0]));
    use std::os::unix::fs::PermissionsExt;
    let mode = |path: &Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&left[0]), 0o600, "a minidump is this user's alone");
}

#[test]
fn a_process_that_crashes_after_the_companions_connect_deadline_still_leaves_one_minidump() {
    // The companion ends on its own only when no app connected in time (#6152): one that did is
    // watched for as long as it runs.
    let crashes = tempfile::tempdir().unwrap();

    let ended = crash("late-abort", crashes.path());

    assert!(!ended.success(), "the probe crashed: {ended:?}");
    let left = minidumps(crashes.path());
    assert_eq!(left.len(), 1, "{left:?}");
    assert!(is_a_minidump(&left[0]));
}

#[test]
fn a_process_that_overflows_its_stack_leaves_one_minidump() {
    let crashes = tempfile::tempdir().unwrap();

    let ended = crash("overflow", crashes.path());

    assert!(!ended.success(), "the probe crashed: {ended:?}");
    let left = minidumps(crashes.path());
    assert_eq!(left.len(), 1, "{left:?}");
    assert!(is_a_minidump(&left[0]));
}

#[test]
fn a_panic_already_told_leaves_no_minidump_since_its_own_report_has_it() {
    let crashes = tempfile::tempdir().unwrap();

    let ended = crash("panic", crashes.path());

    assert!(!ended.success(), "the probe crashed: {ended:?}");
    assert_eq!(minidumps(crashes.path()), Vec::<PathBuf>::new());
}

#[test]
fn a_process_that_ends_well_leaves_nothing() {
    let crashes = tempfile::tempdir().unwrap();

    let ended = crash("exit", crashes.path());

    assert!(ended.success(), "{ended:?}");
    assert_eq!(minidumps(crashes.path()), Vec::<PathBuf>::new());
}

#[test]
fn a_child_of_a_watched_process_that_crashes_dies_of_it_and_leaves_no_minidump() {
    // A child inherits its parent's exception port on macOS. A handler that answers the child's
    // exception as handled resumes its faulting thread, which faults again, for ever.
    let crashes = tempfile::tempdir().unwrap();

    let ended = crash("child", crashes.path());

    assert_eq!(
        ended.code(),
        Some(0),
        "the child died of its crash: {ended:?}"
    );
    assert_eq!(minidumps(crashes.path()), Vec::<PathBuf>::new());
}

/// What the probe is started with and must never leave the device: its environment.
const MARKER: &str = "an-environment-value-that-never-leaves-0f3c9a";

#[test]
fn a_real_crashs_report_carries_its_crashing_threads_stack_and_no_memory() {
    use purlis_core::report::intake::{Dsn, Recorded, Sentry};
    use purlis_core::report::{Consent, Known, Outcome, Reports, native};

    // A real crash, as the app's would be: the probe aborts, its companion writes the minidump.
    let home = tempfile::tempdir().unwrap();
    let crashes = native::crashes_dir(home.path());
    let ended = Command::new(env!("CARGO_BIN_EXE_crash-probe"))
        .arg("abort")
        .env("CRASH_PROBE_CRASHES", &crashes)
        .env("CRASH_PROBE_MARKER", MARKER)
        .status()
        .expect("the probe ran");
    assert!(!ended.success(), "the probe crashed: {ended:?}");
    let left = minidumps(&crashes);
    assert_eq!(left.len(), 1, "{left:?}");
    let minidump = std::fs::read(&left[0]).unwrap();
    assert!(
        minidump
            .windows(MARKER.len())
            .any(|bytes| bytes == MARKER.as_bytes()),
        "the minidump holds the probe's environment (the main thread's stack does): the check \
         below that none of it leaves would otherwise prove nothing"
    );
    let unfinished: Vec<_> = std::fs::read_dir(&crashes)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "part"))
        .collect();
    assert_eq!(
        unfinished,
        Vec::<PathBuf>::new(),
        "nothing half written is left"
    );

    // Its crashing thread's stack, taken out on the device.
    let stack = native::stack_of(&minidump).expect("the stack of a real crash");
    assert!(
        stack.frames.len() > 2,
        "the crashing thread was walked: {stack:?}"
    );
    assert!(
        stack
            .modules
            .iter()
            .any(|module| module.name == "crash-probe"),
        "its frames are in the probe: {stack:?}"
    );
    assert!(
        stack
            .modules
            .iter()
            .all(|module| !module.name.contains('/') && module.debug_id.is_some()),
        "each module by its file name and debug id: {stack:?}"
    );

    // And the Report Reports sends of it.
    let contract = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../purlis-core/tests/intake_contract/sentry/envelope.json");
    let recorded = std::sync::Arc::new(
        Recorded::parse(&std::fs::read_to_string(contract).unwrap()).expect("a recording"),
    );
    let dsn = concat!("https://f00dfeed1234", "@", "o1.ingest.de.sentry.io/4508");
    let reports = Reports::new(Known::default())
        .with_consent(Consent {
            crash_reports: Some(true),
            agent_drafts: None,
            asked: true,
            ..Consent::default()
        })
        .sending_to(
            std::sync::Arc::new(Sentry::new(Dsn::parse(dsn).unwrap(), recorded.clone())),
            home.path(),
        );

    let said = native::report_waiting(&reports, &crashes);

    assert!(matches!(said[..], [Outcome::Sent { .. }]), "{said:?}");
    let sent = String::from_utf8(recorded.received()[0].clone()).expect("text: no minidump");
    let lines: Vec<serde_json::Value> = sent
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let [_, item, event] = &lines[..] else {
        panic!("one event and no attachment: {sent}")
    };
    assert_eq!(item["type"], "event");
    let frames = event["exception"]["values"][0]["stacktrace"]["frames"]
        .as_array()
        .expect("the stack");
    assert_eq!(frames.len(), stack.frames.len());
    for frame in frames {
        let mut keys: Vec<&str> = frame
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            ["addr_mode", "instruction_addr", "package", "trust"],
            "a module and an offset, no register and no memory: {frame}"
        );
    }
    for absent in [
        "threads",
        "memory",
        "registers",
        "attachment",
        "event.minidump",
    ] {
        assert!(!sent.contains(absent), "no {absent} leaves: {sent}");
    }
    assert!(!sent.contains(MARKER), "no environment leaves");
    let leaked = memory_in(&minidump, sent.as_bytes());
    assert!(
        leaked.is_empty(),
        "no memory the minidump holds leaves: {} windows of it were sent, the first at {:#x}",
        leaked.len(),
        leaked.first().copied().unwrap_or_default()
    );
    assert!(
        left[0].with_extension("kept").exists(),
        "the minidump stays here"
    );
}

/// The minidump a probe told to crash `how` leaves, and the Crash event Reports sends of it.
fn crashed_and_reported(how: &str) -> (Vec<u8>, serde_json::Value) {
    use purlis_core::report::intake::{Dsn, Recorded, Sentry};
    use purlis_core::report::{Consent, Known, Outcome, Reports, native};

    let home = tempfile::tempdir().unwrap();
    let crashes = native::crashes_dir(home.path());
    let ended = crash(how, &crashes);
    assert!(!ended.success(), "the probe crashed: {ended:?}");
    let left = minidumps(&crashes);
    assert_eq!(left.len(), 1, "{left:?}");
    let minidump = std::fs::read(&left[0]).unwrap();

    let contract = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../purlis-core/tests/intake_contract/sentry/envelope.json");
    let recorded = std::sync::Arc::new(
        Recorded::parse(&std::fs::read_to_string(contract).unwrap()).expect("a recording"),
    );
    let dsn = concat!("https://f00dfeed1234", "@", "o1.ingest.de.sentry.io/4508");
    let reports = Reports::new(Known::default())
        .with_consent(Consent {
            crash_reports: Some(true),
            agent_drafts: None,
            asked: true,
            ..Consent::default()
        })
        .sending_to(
            std::sync::Arc::new(Sentry::new(Dsn::parse(dsn).unwrap(), recorded.clone())),
            home.path(),
        );
    let said = native::report_waiting(&reports, &crashes);
    assert!(matches!(said[..], [Outcome::Sent { .. }]), "{said:?}");
    let sent = String::from_utf8(recorded.received()[0].clone()).unwrap();
    let event = sent
        .lines()
        .nth(2)
        .map(|line| serde_json::from_str(line).unwrap())
        .expect("an event");
    (minidump, event)
}

/// The file name of the module each frame of `stack` is in, the innermost first.
fn modules_of(stack: &purlis_core::report::native::CrashStack) -> Vec<&str> {
    stack
        .frames
        .iter()
        .map(|frame| stack.modules[frame.module].name.as_str())
        .collect()
}

#[test]
fn an_aborts_stack_starts_where_it_aborted_not_in_the_crash_handler() {
    // An abort is a signal, heard on the crashing thread: the minidump holds that thread as it
    // waits in the handler, below the signal trampoline in libsystem_platform (#5691).
    let (minidump, _) = crashed_and_reported("abort");

    let stack = purlis_core::report::native::stack_of(&minidump).expect("a stack");

    let modules = modules_of(&stack);
    assert!(
        !modules.contains(&"libsystem_platform.dylib"),
        "neither the trampoline nor the handler above it: {modules:?}"
    );
    assert!(
        matches!(
            modules[..],
            ["libsystem_kernel.dylib" | "libsystem_pthread.dylib", ..]
        ),
        "it starts where the abort raised its signal (pthread_kill): {modules:?}"
    );
    assert!(
        modules[..4].contains(&"libsystem_c.dylib") && modules[..5].contains(&"crash-probe"),
        "then abort, called from the probe: {modules:?}"
    );
}

#[test]
fn a_stack_overflows_stack_is_kept_whole_from_where_it_faulted() {
    // A fault is a Mach exception, heard on the handler's own thread: the crashing thread is
    // where it faulted, with no trampoline to look past.
    let (minidump, _) = crashed_and_reported("overflow");

    let stack = purlis_core::report::native::stack_of(&minidump).expect("a stack");

    let modules = modules_of(&stack);
    assert_eq!(modules.first(), Some(&"crash-probe"), "{stack:?}");
    assert_eq!(stack.frames[0].trust, "context", "{stack:?}");
    assert!(
        modules.iter().all(|module| *module == "crash-probe"),
        "{modules:?}"
    );
}

#[test]
fn aborts_from_two_places_are_two_crashes_to_the_intake() {
    let (_, here) = crashed_and_reported("abort");
    let (_, elsewhere) = crashed_and_reported("abort-elsewhere");

    assert_ne!(
        here["fingerprint"], elsewhere["fingerprint"],
        "each its own issue, not one for every abort: {here} {elsewhere}"
    );
}

/// How many bytes a window of memory is: a run this long found in the report is memory, not chance.
const WINDOW: usize = 32;

/// The address of every [`WINDOW`]-byte window of the memory regions `minidump` holds (its
/// threads' stacks and the memory around their registers) that is found in `sent`, leaving out a
/// window of one repeated byte, which says nothing of where it came from. Fails unless the
/// minidump holds some memory, so finding none is a finding.
fn memory_in(minidump: &[u8], sent: &[u8]) -> Vec<u64> {
    let dump = minidump::Minidump::read(minidump).expect("a minidump");
    let memory = dump.get_memory().expect("the minidump's memory");
    let sent: std::collections::HashSet<&[u8]> = sent.windows(WINDOW).collect();
    let mut held = 0;
    let mut found = Vec::new();
    for region in memory.iter() {
        let bytes = region.bytes();
        let base = region.memory_range().map_or(0, |range| range.start);
        held += bytes.len();
        for (at, window) in bytes.windows(WINDOW).enumerate() {
            if window.iter().any(|b| *b != window[0]) && sent.contains(window) {
                found.push(base + at as u64);
            }
        }
    }
    assert!(held > 0, "the minidump holds memory to look for");
    found
}

#[test]
fn a_companion_whose_app_died_before_connecting_exits() {
    // The app starts its companion and dies before it connects (#6152): the companion is this
    // probe started with the socket to serve and nothing ever connecting to it.
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("crash.sock");
    let mut companion = Command::new(env!("CARGO_BIN_EXE_crash-probe"))
        .env(purlis_crash_handler::ENV, &socket)
        .env("CRASH_PROBE_CRASHES", dir.path().join("crashes"))
        .spawn()
        .expect("the companion started");

    let until = std::time::Instant::now() + std::time::Duration::from_secs(20);
    let ended = loop {
        if let Some(status) = companion.try_wait().unwrap() {
            break Some(status);
        }
        if std::time::Instant::now() >= until {
            let _ = companion.kill();
            let _ = companion.wait();
            break None;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    };

    let ended = ended.expect("the companion exited, with no app to serve, within 20 seconds");
    assert!(ended.success(), "it exited well: {ended:?}");
    assert!(!socket.exists(), "its socket is gone");
}

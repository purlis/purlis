// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The release profile unwinds, so that an Objective-C exception can reach `NSApplication`
//! (a paste aborted the app before). A Rust panic still ends the process, on any thread, the
//! way it did under `panic = "abort"`: `purlis_core::panicend::fatal`, which every shipped
//! binary calls first in its `main`.

/// Set in the child run alone.
const CHILD: &str = "PURLIS_TEST_PANIC_END_CHILD";

/// The child: in a run of this binary that the test started, a thread panics and the rest of the
/// process carries on, which only [`purlis_core::panicend::fatal`] stops. In any other run it
/// does nothing.
#[test]
fn panicking_child() {
    if std::env::var_os(CHILD).is_none() {
        return;
    }
    purlis_core::panicend::fatal();
    let _ = std::thread::spawn(|| panic!("a panic the test asked for")).join();
    // Unwinding alone would end the thread and land here.
    std::process::exit(0);
}

#[test]
fn a_panic_on_a_thread_aborts_the_whole_process_after_the_hooks_before_it_have_run() {
    let ran = purlis_core::forklock::output(
        std::process::Command::new(std::env::current_exe().expect("the test binary"))
            .args([
                "a_panic_on_any_thread_ends_a_shipped_binary::panicking_child",
                "--exact",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(CHILD, "1"),
    )
    .expect("the child runs");
    let said = String::from_utf8_lossy(&ran.stderr);
    assert!(
        !ran.status.success(),
        "the process went on after a panic: {said}"
    );
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        assert_eq!(
            ran.status.signal(),
            Some(6),
            "ended by SIGABRT, as under panic = \"abort\": {:?} {said}",
            ran.status
        );
    }
    // The hook that was there before (Rust's own, here) still said what panicked.
    assert!(said.contains("a panic the test asked for"), "{said}");
}

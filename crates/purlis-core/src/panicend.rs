// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A panic in Rust ends the process, on any thread, in every shipped binary.**
//!
//! The release profile unwinds (`panic = "unwind"`) so that an Objective-C exception AppKit raises
//! under one of the window's callbacks, such as the `sendEvent:` a paste goes through, can
//! unwind back to `NSApplication`, which reports it and goes on. Under `panic = "abort"` the app
//! aborted on it instead, and a paste could take the whole app down (tao#1354).
//!
//! A panic of purlis's own must still end the process, as it did under `abort`: no `Drop` runs
//! after it, nothing catches it, and a thread never dies alone while the rest go on. So each
//! shipped binary calls [`fatal`] first in its `main`, in a build without debug assertions
//! (`release`, `dev-release`). A debug build and the tests unwind, as they always have. Its hook runs every hook installed before
//! it and after it (the app's panic log, the guard's refusal), and then aborts, before any
//! unwinding starts.

/// From here on, a Rust panic on any thread aborts the process, once the hook that was installed
/// before this one has run. A hook installed afterwards that keeps the one before it, as the
/// app's panic log does, runs first; one that ends the process itself, as the guard's refusal
/// does, ends it its own way.
pub fn fatal() {
    let before = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        before(info);
        std::process::abort();
    }));
}

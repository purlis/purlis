// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The duration of a browser start's steps (#7182), as the same local `info` line the session
//! protocol's `steps` writes (ADR 0075: no telemetry, nothing leaves the machine). This crate
//! does not link the session protocol, so it keeps its own six lines of it.

use std::time::Instant;

/// A step that is running: its line is written when this is dropped.
#[must_use = "a step is timed until its guard is dropped"]
pub(crate) struct Step(&'static str, Instant);

pub(crate) fn begin(name: &'static str) -> Step {
    Step(name, Instant::now())
}

impl Drop for Step {
    fn drop(&mut self) {
        let name = self.0;
        let elapsed_ms = u64::try_from(self.1.elapsed().as_millis()).unwrap_or(u64::MAX);
        tracing::info!(
            target: "purlis::step", step = name, elapsed_ms, outcome = "done",
            "purlis: step {name} done after {elapsed_ms} ms"
        );
    }
}

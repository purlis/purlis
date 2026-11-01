// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Limits that can't change** (#4281, spec #4275): numbers that protect a boundary or keep one
//! runner from holding up the rest. Each says it is fixed, why in a few words, and what a person
//! can do instead, and offers no way to change it. They live here because the session host, which
//! doesn't link `purlis-core`, hits one of them; `purlis_core::poollimits::fixed` lists them.

/// One limit that can't change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedLimit {
    /// The word `purlis pool limit` would be asked for it by, which it refuses.
    pub key: &'static str,
    /// Its name in plain words.
    pub plain: &'static str,
    /// Why it is fixed, finishing "This limit is fixed: ".
    pub why: &'static str,
    /// What to do instead, finishing "Instead, ".
    pub instead: &'static str,
}

impl FixedLimit {
    /// **What a refusal adds when this limit is hit**: fixed, why, and what to do instead.
    #[must_use]
    pub fn said(&self) -> String {
        format!(
            "This limit is fixed: {}. Instead, {}.",
            self.why, self.instead
        )
    }
}

/// How long a stop or a re-arm waits for one runner.
pub const STOP_WAIT: FixedLimit = FixedLimit {
    key: "stopWait",
    plain: "Longest wait for a stop to reach a runner",
    why: "one runner that doesn't answer must not hold up the others",
    instead: "check that runner is up and its link works; it is tried again on its own",
};

/// The most purlis reads of what a runner, or the tool reaching it, prints.
pub const RUNNER_OUTPUT: FixedLimit = FixedLimit {
    key: "runnerOutput",
    plain: "Most purlis reads of a runner's output",
    why: "what a runner prints can't be trusted to be small, and this machine reads it into memory",
    instead: "check that this machine's tools and the runner's image are the ones you expect",
};

/// **Every limit that can't change.**
pub const ALL: [FixedLimit; 2] = [STOP_WAIT, RUNNER_OUTPUT];

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The CI grader's scoring** (#5726, spec #5714; ADR 0105 as amended 2026-11-08): recorded
//! machine-readable output of Claude Code, Codex and opencode in, a score per built-in skill and
//! its trigger bar out.
//!
//! It launches nothing. Whatever ran a harness hands it what the harness printed in its print
//! mode, one [`Run`] per chat, and it reads which skills each chat started ([`started`]) by §2's
//! routes: the harness's own Skill tool, a read of a briefed `SKILL.md` on Codex, or purlis's
//! `skill_load`, a plugin's namespace no part of the name. Only tool calls count, never text the
//! model printed.

mod output;

pub use output::{Harness, Listing, Started, Unread, started};

#[cfg(test)]
mod tests;

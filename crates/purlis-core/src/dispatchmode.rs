// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A task starts in its asking chat's permission mode, capped at auto** (#3787; the
//! operator's ruling of 2026-10-29, amending decision 20 of #1446, ADR 0090).
//!
//! | Asking chat          | Task    |
//! |----------------------|---------|
//! | default (asks)       | default |
//! | auto                 | auto    |
//! | bypass (prompts off) | auto    |
//!
//! Auto switches nothing off: the harness's classifier still judges every tool call, and
//! purlis's sandbox and guards stand. Bypass never travels, and nor does any mode that is not
//! classifier-backed (Claude Code's `dontAsk`, `acceptEdits`, `plan`): those start the task in
//! its harness's default.
//!
//! # Whose word it is
//!
//! **The app's own record of what the asking chat's harness reported** in its hook payloads'
//! `permission_mode` ([`Reported`], kept as [`Heard`]), never a line the chat sends. Only a
//! report the board took for the chat counts, as for [`crate::dispatchunattended::Mark`].
//! Unlike that mark, this goes both ways: the person may switch a chat out of auto, and its
//! next task then asks.
//!
//! # One neutral level, one adapter per harness
//!
//! [`Level`] is purlis's word; each harness says what its own reports mean
//! ([`crate::harness::HarnessAdapter::reported_mode`]) and how a chat is started at a level
//! ([`crate::harness::HarnessAdapter::task_mode_words`]). Claude Code reads both. Codex and
//! opencode read neither: Codex names `bypassPermissions` for every chat, and neither documents
//! a classifier-backed mode, so their tasks keep their default and their reports say nothing.

/// **How a chat's harness decides its own tool calls**, in purlis's words for every harness.
/// Ordered: a task is never started above [`Level::Classifier`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    /// It asks a person before each tool its rules do not allow: the harness's default.
    #[default]
    Asks,
    /// The harness's own classifier judges each tool call in a person's place (Claude Code's
    /// `auto`). Nothing else is switched off.
    Classifier,
}

/// **What a chat's harness said of its permission mode** on one hook report, as the hook
/// sends it to the app.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reported {
    /// Any mode that asks, or that is not classifier-backed, or that purlis does not know.
    Asks,
    /// Its classifier-backed mode.
    Classifier,
    /// Its prompts off.
    PromptsOff,
}

/// Claude Code's `permission_mode` for its classifier-backed mode, by its own table of modes
/// (2.1.312: `auto`'s external name is `auto`).
pub const CLAUDE_CODE_AUTO: &str = "auto";

impl Reported {
    /// **What Claude Code's `permission_mode` says**, or `None` where the payload names no
    /// mode. Read from 2.1.312's table of modes and their external names: `default` (shown
    /// as manual), `plan`, `acceptEdits`, `auto`, `dontAsk` and `bypassPermissions`. Only
    /// the exact words count; any other reads as asking.
    pub fn of_claude_code(permission_mode: Option<&str>) -> Option<Self> {
        Some(match permission_mode? {
            CLAUDE_CODE_AUTO => Self::Classifier,
            crate::floorguard::UNATTENDED_MODE => Self::PromptsOff,
            _ => Self::Asks,
        })
    }
}

/// **The level a task starts at** for an asking chat whose harness last reported `asker`:
/// the asker's own, capped at [`Level::Classifier`]. An asker that never said asks.
pub fn for_a_task(asker: Option<Reported>) -> Level {
    match asker {
        Some(Reported::Classifier | Reported::PromptsOff) => Level::Classifier,
        Some(Reported::Asks) | None => Level::Asks,
    }
}

/// **The app's record of one chat's mode**: the last mode its harness reported.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Heard(Option<Reported>);

impl Heard {
    /// Hears one report. One that names no mode leaves the record as it was.
    pub fn heard(&mut self, reported: Option<Reported>) {
        if reported.is_some() {
            self.0 = reported;
        }
    }

    /// [`for_a_task`] of what was heard last.
    pub fn for_a_task(self) -> Level {
        for_a_task(self.0)
    }
}

#[cfg(test)]
#[path = "dispatchmode_tests.rs"]
mod tests;

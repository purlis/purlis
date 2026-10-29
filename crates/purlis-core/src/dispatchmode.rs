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
//! `permission_mode` ([`Reported`], kept as [`Heard`]), never a line the chat sends: only the
//! chat's own harness's, not a sub-agent's, about the conversation the board holds for it
//! ([`of_its_own_harness`]). Unlike [`crate::dispatchunattended::Mark`] this goes both ways:
//! the person may switch a chat out of auto, and its next task then asks.
//!
//! **A profile's own mode wins**, as decision 20 treats profiles: where its command names
//! `--permission-mode` (`dontAsk` included, which is stricter than the default), nothing is
//! added. A profile that switches the prompts off is still refused for a task
//! ([`crate::dispatchunattended::bypass_refusal`]). A grade's chat gets nothing added
//! ([`task_words`]).
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

/// **The mode `report` names as its chat's own harness's**, or `None`: only from no sub-agent,
/// and about `now`, the conversation the board holds for the chat after it took the report. A
/// harness nested in the chat names another conversation, or one the board refused (ADR 0024),
/// and names no mode of the chat's.
pub fn of_its_own_harness(report: &crate::hookwire::Report, now: Option<&str>) -> Option<Reported> {
    let crate::hookwire::Conversation::Named(said) = &report.conversation else {
        return None;
    };
    if report.agent.is_some() || now != Some(said.as_str()) {
        return None;
    }
    report.detail.mode
}

/// **The words a chat on `harness` is started with for `level`**, after its profile's own
/// `command`: its adapter's ([`crate::harness::HarnessAdapter::task_mode_words`]), and none for
/// a grade's chat (`graded`), whose mode its grade pins and which a flag would outrank.
pub fn task_words(
    harness: crate::harness::Harness,
    level: Level,
    command: &[String],
    graded: bool,
) -> Vec<String> {
    if graded {
        return Vec::new();
    }
    harness.adapter().task_mode_words(level, command)
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

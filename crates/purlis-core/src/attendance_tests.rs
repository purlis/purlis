// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The attendance fact keys on the adapter's fact, never on which harness it is** (Q280): a
//! chat's mode is kept to check its reports against only where its harness's adapter reports a
//! mode purlis can check ([`Harness::reports_a_mode_purlis_can_check`]).

use super::*;

/// A start in `plan` mode, on any harness.
fn in_plan() -> Vec<String> {
    ["a-harness", "--permission-mode", "plan"]
        .map(String::from)
        .to_vec()
}

/// Q280: on an adapter that reports a mode purlis can check, whichever harness it is, a mode
/// purlis didn't set lowers the fact and is the Notice's; on one that reports none, nothing it
/// says moves the fact. Mutation caught: the mode kept on neither, or on both, and (the drift the
/// phase-1 gate found) a fact keyed on `Harness::ClaudeCode`, which a start can no longer name
/// here, since all it is handed is the adapter's answer.
#[test]
fn a_mode_is_checked_on_an_adapter_that_reports_one_and_on_no_other() {
    let mut reads = Fact::at_start_reading(Begun::Person, true, &in_plan());
    assert!(!reads.unattended(), "a person's chat");
    reads.hear(Some(ClaudeCodeMode::AcceptEdits));
    assert!(reads.unattended(), "a mode purlis didn't set lowers it");
    assert_eq!(
        reads.mode_differs(),
        Some(ModeDiffers {
            set: ClaudeCodeMode::Plan,
            reported: ClaudeCodeMode::AcceptEdits,
        })
    );

    let mut reads_none = Fact::at_start_reading(Begun::Person, false, &in_plan());
    reads_none.hear(Some(ClaudeCodeMode::AcceptEdits));
    assert!(!reads_none.unattended(), "no report of it is read");
    assert_eq!(reads_none.mode_differs(), None);
}

/// Q280: a start asks each harness's own adapter whether it reports a mode, and a chat on no
/// harness reads none. Mutation caught: `at_start` handing its helper `true` or `false`
/// whatever the harness, or reading a mode for a chat on no harness.
#[test]
fn a_start_asks_each_harness_s_adapter_whether_its_mode_is_read() {
    for harness in [Harness::ClaudeCode, Harness::Codex, Harness::Opencode] {
        let mut fact = Fact::at_start(Begun::Person, Some(harness), &in_plan());
        fact.hear(Some(ClaudeCodeMode::AcceptEdits));
        assert_eq!(
            fact.mode_differs().is_some(),
            harness.reports_a_mode_purlis_can_check(),
            "{harness:?}"
        );
    }
    let mut on_none = Fact::at_start(Begun::Person, None, &in_plan());
    on_none.hear(Some(ClaudeCodeMode::AcceptEdits));
    assert_eq!(on_none.mode_differs(), None);
    assert!(!on_none.unattended());
}

/// Round 2 of #7168, item 2: a person's own chat on a profile whose inline `--settings` carry
/// `hooks` or `disableAllHooks` starts `Unattended`. It fails closed: a hook of the profile's own
/// may answer allow ahead of purlis's guard, so nobody can be said to be asked.
#[test]
fn a_persons_chat_on_a_profile_with_inline_hooks_is_unattended() {
    let on = |value: &str| {
        let command = [
            "claude".to_owned(),
            "--settings".to_owned(),
            value.to_owned(),
        ];
        Fact::at_start(Begun::Person, Some(Harness::ClaudeCode), &command).attendance
    };
    for value in [
        r#"{"hooks":{"PreToolUse":[]}}"#,
        r#"{"disableAllHooks":true}"#,
        r#"{"allowManagedHooksOnly":true}"#,
        "{not json",
    ] {
        assert_eq!(on(value), Attendance::Unattended, "{value}");
    }
    assert_eq!(on(r#"{"model":"x"}"#), Attendance::Attended);
}

fn words(line: &[&str]) -> Vec<String> {
    line.iter().map(|word| (*word).to_owned()).collect()
}

/// M75-36 round 2, F1: the mode a Claude Code task starts in is read the way
/// [`crate::dispatchunattended::bypass_in`] reads it, so each spelling that starts the harness
/// with its prompts off (or its guards) is the bypass, not the default `mode-task-above-auto`
/// would let through.
#[test]
fn every_spelling_the_bypass_detector_knows_starts_claude_code_in_the_bypass_mode() {
    let inline = r#"{"permissions":{"defaultMode":"bypassPermissions"}}"#;
    let attached = format!("--settings={inline}");
    for line in [
        words(&["claude", "--dangerously-skip-permissions"]),
        words(&["claude", "--yolo"]),
        words(&["claude", "--safe-mode"]),
        words(&["claude", "--settings", inline]),
        words(&["claude", &attached]),
        words(&["claude", "--permission-mode", "bypassPermissions"]),
        // Said beside a lower mode, the bypass still starts it.
        words(&["claude", "--permission-mode", "plan", "--dangerously-skip-permissions"]),
    ] {
        assert_eq!(ClaudeCodeMode::set_in(&line), ClaudeCodeMode::BypassPermissions, "{line:?}");
        let task = Fact::at_start(Begun::Task, Some(Harness::ClaudeCode), &line);
        assert_eq!(task.start_mode(), Some(ClaudeCodeMode::BypassPermissions), "{line:?}");
    }
    // What is not a bypass keeps the mode it names, or the default.
    assert_eq!(ClaudeCodeMode::set_in(&words(&["claude"])), ClaudeCodeMode::Default);
    assert_eq!(in_plan_mode(), ClaudeCodeMode::Plan);
}

fn in_plan_mode() -> ClaudeCodeMode {
    ClaudeCodeMode::set_in(&in_plan())
}

/// M75-36 round 2, F2: a harness whose mode purlis does not read still has its start recorded as
/// the bypass where the host knows its words are one; any other start of it stays unrecorded
/// (`unknown`). What the chat later reports moves neither.
#[test]
fn a_codex_start_the_host_knows_is_a_bypass_is_recorded_as_one() {
    for line in [
        words(&["codex", "--dangerously-bypass-approvals-and-sandbox"]),
        words(&["codex", "--full-auto"]),
        words(&["codex", "--ask-for-approval", "never"]),
        words(&["codex", "-c", "approval_policy=never"]),
    ] {
        let mut fact = Fact::at_start(Begun::Task, Some(Harness::Codex), &line);
        assert_eq!(fact.start_mode(), Some(ClaudeCodeMode::BypassPermissions), "{line:?}");
        fact.hear(Some(ClaudeCodeMode::Default));
        assert_eq!(fact.start_mode(), Some(ClaudeCodeMode::BypassPermissions), "{line:?}");
        assert_eq!(fact.mode_differs(), None, "a harness whose reports are not read");
    }
    for harness in [Harness::Codex, Harness::Opencode] {
        let plain = Fact::at_start(Begun::Task, Some(harness), &words(&["a-harness"]));
        assert_eq!(plain.start_mode(), None, "{harness:?}: no record of a mode: unknown");
    }
    assert_eq!(Fact::kept(Attendance::Attended).start_mode(), None);
}

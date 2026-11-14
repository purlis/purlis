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

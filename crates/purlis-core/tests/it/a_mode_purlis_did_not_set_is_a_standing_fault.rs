// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A mode purlis didn't set is a standing fault** (T1-44, #6435; R-243, Q89, Q270, Q271): the
//! attendance fact ([`purlis_core::attendance::Fact`]) says, once a Claude Code chat's armed
//! hook reported a mode purlis didn't start it in, which mode was set and which was reported.
//! That is what the window's Notice says. The phase 3 and 4 standing faults ("purlis decides"
//! forbidden, a broken policy, a ruleset a policy requires) are not raised here (#6391).
//!
//! **Each pin names the mutation that shows it red** (Q208), in its doc comment.

use purlis_core::attendance::{Begun, ClaudeCodeMode, Fact, ModeDiffers};
use purlis_core::harness::Harness;
use purlis_core::hookwire::Report;
use purlis_core::state::Event;
use serde_json::Value;

/// A chat's hook line on `harness` (by its registry name) for `event`, its payload naming
/// `mode` as its `permission_mode`, or none.
fn line(harness: &str, event: Event, mode: Option<&str>) -> Report {
    let mut payload = serde_json::json!({ "session_id": "11111111-2222-4333-8444-555555555555" });
    if let Some(mode) = mode {
        payload["permission_mode"] = Value::from(mode);
    }
    let env = |name: &str| match name {
        "PURLIS_CHAT" => Some("7".to_owned()),
        "PURLIS_HARNESS" => Some(harness.to_owned()),
        _ => None,
    };
    Report::read(event, &payload.to_string(), &env).expect("a chat's hook line reads")
}

fn words(command: &[&str]) -> Vec<String> {
    command.iter().map(|word| (*word).to_owned()).collect()
}

/// A Claude Code chat started as `begun`, in `mode` where the command names one.
fn claude_code_chat(begun: Begun, mode: Option<&str>) -> Fact {
    let command = match mode {
        Some(mode) => words(&["claude", "--permission-mode", mode]),
        None => words(&["claude"]),
    };
    Fact::at_start(begun, Some(Harness::ClaudeCode), &command)
}

/// **R-243**: a Claude Code chat whose hook reports a mode purlis didn't set names both modes;
/// one that reports the mode purlis set names none. A string nobody measured is named as
/// unknown, never as the mode set (Q198, Q270).
///
/// Mutation that shows it red: have [`Fact::mode_differs`] answer `None` always.
#[test]
fn a_mode_purlis_did_not_set_names_the_mode_set_and_the_mode_reported() {
    purlis_core::unsteered!();
    let mut fact = claude_code_chat(Begun::Person, None);
    fact.heard(&line("claude-code", Event::Stop, Some("default")));
    assert_eq!(fact.mode_differs(), None, "the mode purlis set is no fault");

    fact.heard(&line("claude-code", Event::UserPromptSubmit, Some("plan")));
    assert_eq!(
        fact.mode_differs(),
        Some(ModeDiffers {
            set: ClaudeCodeMode::Default,
            reported: ClaudeCodeMode::Plan,
        })
    );

    let mut fact = claude_code_chat(Begun::Person, Some("acceptEdits"));
    fact.heard(&line(
        "claude-code",
        Event::Stop,
        Some("aModeNobodyMeasured"),
    ));
    assert_eq!(
        fact.mode_differs(),
        Some(ModeDiffers {
            set: ClaudeCodeMode::AcceptEdits,
            reported: ClaudeCodeMode::Unknown,
        })
    );
}

/// **R-243, R-318**: the fault stands once the fact flipped, as the fact does: a later report
/// of the mode purlis set clears nothing, and a later mode that differs again is the one named.
///
/// Mutation that shows it red: have [`Fact::mode_differs`] compare only the last report with
/// the mode set.
#[test]
fn a_mode_that_differed_stands_once_the_fact_flipped() {
    purlis_core::unsteered!();
    let mut fact = claude_code_chat(Begun::Person, Some("default"));
    fact.heard(&line("claude-code", Event::Stop, Some("auto")));
    fact.heard(&line("claude-code", Event::Stop, Some("default")));
    assert_eq!(
        fact.mode_differs(),
        Some(ModeDiffers {
            set: ClaudeCodeMode::Default,
            reported: ClaudeCodeMode::Auto,
        }),
        "a report of the mode set afterwards is no evidence anything changed"
    );
    fact.heard(&line("claude-code", Event::Stop, Some("bypassPermissions")));
    assert_eq!(
        fact.mode_differs(),
        Some(ModeDiffers {
            set: ClaudeCodeMode::Default,
            reported: ClaudeCodeMode::BypassPermissions,
        })
    );
}

/// **R-243, R-289, Q271**: only a mode Claude Code's armed hook reported is a fault. A chat
/// purlis started unattended, a line whose mode is unproven (its bypass bit still lowers the
/// fact), a build that stops sending `permission_mode`, and Codex's and opencode's reports name
/// none: the Notice is Claude Code's case alone.
///
/// Mutation that shows it red: have [`Fact::mode_differs`] answer for any unattended fact.
#[test]
fn only_a_mode_claude_codes_hook_reported_is_a_fault() {
    purlis_core::unsteered!();
    // Started unattended, as a task, and reporting the mode it was started in.
    let mut task = claude_code_chat(Begun::Task, Some("auto"));
    task.heard(&line("claude-code", Event::Stop, Some("auto")));
    assert_eq!(task.mode_differs(), None, "a task in the mode purlis set");

    // An unproven bypass line: unattended, but no mode was reported.
    let mut unproven = line("claude-code", Event::Stop, Some("bypassPermissions"));
    unproven.mode_unproven();
    let mut fact = claude_code_chat(Begun::Person, None);
    fact.heard(&unproven);
    assert_eq!(fact.mode_differs(), None, "an unproven line names no mode");

    // A build that stops sending `permission_mode` leaves the fact where it is (Q271).
    let mut fact = claude_code_chat(Begun::Person, None);
    fact.heard(&line("claude-code", Event::Stop, None));
    assert_eq!(fact.mode_differs(), None, "no mode sent, no fault");

    // Codex names bypass for every chat; opencode is read by nothing of Claude Code's.
    for (harness, kind, program) in [
        ("codex", Harness::Codex, "codex"),
        ("opencode", Harness::Opencode, "opencode"),
    ] {
        let mut fact = Fact::at_start(Begun::Person, Some(kind), &words(&[program]));
        fact.heard(&line(harness, Event::Stop, Some("bypassPermissions")));
        fact.heard(&line(harness, Event::Stop, Some("plan")));
        assert_eq!(fact.mode_differs(), None, "{harness} shows nothing");
    }
}

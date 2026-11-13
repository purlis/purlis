// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Whether a chat is attended is one fact purlis sets** (T1-17, #6408; T1-16, #6407, #3825):
//! [`purlis_core::attendance::Fact`], set by how purlis started the chat and moved only by a
//! mode its armed hook reports that purlis did not set. Read by nobody yet: the readers move in
//! later steps (#6409, #6410, #6411, #6412).
//!
//! **What Claude Code reports is measured, not assumed** (Q270): `tests/fixtures/harness-modes/
//! claude-code-permission-modes.json` is what the person's run of T1-16's script printed, one
//! `permission_mode` per mode and hook event, and every mode test here reads it.
//!
//! **Each pin names the mutation that shows it red** (Q208), in its doc comment.

use purlis_core::attendance::{Begun, ClaudeCodeMode, Fact};
use purlis_core::dispatchunattended::Attendance;
use purlis_core::harness::Harness;
use purlis_core::hookwire::Report;
use purlis_core::state::Event;
use serde_json::Value;

/// T1-16's measurement, as the person's run printed it (Q270).
const MEASURED: &str =
    include_str!("../../../../tests/fixtures/harness-modes/claude-code-permission-modes.json");

/// Every hook event a chat's report names, so a sequence of them covers each kind.
const EVENTS: [Event; 7] = [
    Event::SessionStart,
    Event::UserPromptSubmit,
    Event::Notification,
    Event::SubagentStop,
    Event::Stop,
    Event::StopFailure,
    Event::SessionEnd,
];

/// The fixture's events that reach the app as a chat's report. `PreToolUse` is a tool line,
/// which carries no mode to the app.
fn event_named(name: &str) -> Option<Event> {
    match name {
        "UserPromptSubmit" => Some(Event::UserPromptSubmit),
        "Stop" => Some(Event::Stop),
        _ => None,
    }
}

/// Each measured mode and the strings its reporting events said, from the fixture.
fn measured() -> Vec<(String, Vec<(String, Vec<String>)>)> {
    let read: Value = serde_json::from_str(MEASURED).expect("the fixture is JSON");
    let modes = read["modes"]
        .as_object()
        .expect("the fixture names its modes");
    modes
        .iter()
        .map(|(mode, events)| {
            let events = events
                .as_object()
                .expect("each mode names its events")
                .iter()
                .map(|(event, said)| {
                    let said = said
                        .as_array()
                        .expect("each event lists what it said")
                        .iter()
                        .map(|one| one.as_str().expect("a string").to_owned())
                        .collect();
                    (event.clone(), said)
                })
                .collect();
            (mode.clone(), events)
        })
        .collect()
}

/// Every string the fixture says a reporting event carried, with the event.
fn measured_reports() -> Vec<(Event, String)> {
    measured()
        .into_iter()
        .flat_map(|(_, events)| events)
        .filter_map(|(event, said)| event_named(&event).map(|event| (event, said)))
        .flat_map(|(event, said)| said.into_iter().map(move |one| (event, one)))
        .collect()
}

/// The measured mode names, one per mode.
fn measured_modes() -> Vec<String> {
    measured().into_iter().map(|(mode, _)| mode).collect()
}

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

/// A Claude Code chat a person started, in `mode` where the command names one.
fn claude_code_chat(mode: Option<&str>) -> Fact {
    let command = match mode {
        Some(mode) => words(&["claude", "--permission-mode", mode]),
        None => words(&["claude"]),
    };
    Fact::at_start(Begun::Person, Some(Harness::ClaudeCode), &command)
}

/// **R-095, T1-16's step**: the committed fixture names the harness version and a string per
/// mode, and it is what the mode reading reads: each measured string is its own mode, never
/// unknown, and the reading knows no mode the fixture does not name.
///
/// Mutation that shows it red: take `plan` out of [`ClaudeCodeMode::MEASURED`].
#[test]
fn the_measured_permission_mode_strings_are_the_fixture_the_mode_tests_read() {
    purlis_core::unsteered!();
    let read: Value = serde_json::from_str(MEASURED).expect("the fixture is JSON");
    let version = read["claude_code_version"].as_str().unwrap_or_default();
    assert!(
        version.contains("Claude Code") && version.chars().any(|c| c.is_ascii_digit()),
        "the fixture names the version it measured: {version:?}"
    );
    let modes = measured();
    assert!(!modes.is_empty(), "the fixture names a mode");
    for (mode, events) in &modes {
        for wanted in ["UserPromptSubmit", "PreToolUse", "Stop"] {
            let said = events
                .iter()
                .find(|(event, _)| event == wanted)
                .map(|(_, said)| said.clone())
                .unwrap_or_default();
            assert!(!said.is_empty(), "{mode} says a string on {wanted}");
            for one in said {
                let reading = ClaudeCodeMode::of(&one);
                assert_ne!(
                    reading,
                    ClaudeCodeMode::Unknown,
                    "{one:?} is a measured mode"
                );
                assert_eq!(reading.as_str(), mode, "{one:?} reads as {mode}");
            }
        }
    }
    let mut known: Vec<&str> = ClaudeCodeMode::MEASURED
        .iter()
        .map(|(said, _)| *said)
        .collect();
    known.sort_unstable();
    let mut fixture = measured_modes();
    fixture.sort_unstable();
    assert_eq!(
        known, fixture,
        "the reading knows exactly the measured modes"
    );
    assert_eq!(
        ClaudeCodeMode::of("aModeNobodyMeasured"),
        ClaudeCodeMode::Unknown,
        "a string nobody measured reads as unknown"
    );
}

/// **R-095**: with start arguments naming one mode and the armed hook another, the reported
/// mode is the hook's. The start arguments are never a report, and a line that cannot prove it
/// came from the armed hook reports nothing.
///
/// A pin where it already held (the hook line's mode is read from its payload alone, Q208).
/// Mutation that shows it red: have [`Fact::at_start`] take `reported` from the command's
/// `--permission-mode`, or have `Report::mode_unproven` keep the line's Claude Code mode.
#[test]
fn claude_code_mode_is_read_only_from_the_armed_hook() {
    purlis_core::unsteered!();
    for set in measured_modes() {
        let started = claude_code_chat(Some(&set));
        assert_eq!(started.reported(), None, "the start's {set} is no report");
        for (event, said) in measured_reports() {
            let report = line("claude-code", event, Some(&said));
            assert_eq!(
                report.detail.claude_code_mode,
                Some(ClaudeCodeMode::of(&said)),
                "the hook line carries {said:?}"
            );
            let mut fact = started;
            fact.heard(&report);
            assert_eq!(
                fact.reported(),
                Some(ClaudeCodeMode::of(&said)),
                "started in {set}, the hook said {said:?}: the hook's is the reported mode"
            );
        }
    }
    // A line without the chat's armed key: its mode is not read.
    let mut unproven = line("claude-code", Event::Stop, Some("bypassPermissions"));
    unproven.mode_unproven();
    assert_eq!(unproven.detail.claude_code_mode, None);
    let mut fact = claude_code_chat(None);
    fact.heard(&unproven);
    assert_eq!(fact.reported(), None, "an unproven line reports no mode");
    // A payload naming no mode says nothing.
    let silent = line("claude-code", Event::SessionStart, None);
    assert_eq!(silent.detail.claude_code_mode, None);
    // A string nobody measured reads as unknown, never as a mode.
    let new = line("claude-code", Event::Stop, Some("aModeNobodyMeasured"));
    assert_eq!(new.detail.claude_code_mode, Some(ClaudeCodeMode::Unknown));
}

/// **R-106**: a chat purlis started as a task, a loop or a runner is unattended, on every
/// harness; one a person started is attended, unless its profile's command switches the
/// prompts off.
#[test]
fn a_chat_purlis_started_as_a_task_loop_or_runner_is_unattended() {
    purlis_core::unsteered!();
    let harnesses = Harness::ALL.map(Some);
    for harness in harnesses.into_iter().chain([None]) {
        let command = words(&["harness"]);
        for begun in [Begun::Task, Begun::Loop, Begun::Runner] {
            assert_eq!(
                Fact::at_start(begun, harness, &command).attendance(),
                Attendance::Unattended,
                "{begun:?} on {harness:?}"
            );
        }
        assert_eq!(
            Fact::at_start(Begun::Person, harness, &command).attendance(),
            Attendance::Attended,
            "a person's start on {harness:?}"
        );
    }
    for bypass in [
        words(&["claude", "--dangerously-skip-permissions"]),
        words(&["claude", "--permission-mode", "bypassPermissions"]),
    ] {
        assert_eq!(
            Fact::at_start(Begun::Person, Some(Harness::ClaudeCode), &bypass).attendance(),
            Attendance::Unattended,
            "a profile that switches the prompts off: {bypass:?}"
        );
    }
}

/// **R-107**: a Claude Code chat whose armed hook reports a mode purlis did not set is
/// unattended, and one reporting the mode set stays attended; Codex's bypass report is no
/// such mode.
#[test]
fn a_mode_purlis_did_not_set_makes_a_claude_code_chat_unattended_and_codex_bypass_does_not() {
    purlis_core::unsteered!();
    for set in measured_modes() {
        if set == "bypassPermissions" {
            continue;
        }
        for (event, said) in measured_reports() {
            let mut fact = claude_code_chat(Some(&set));
            fact.heard(&line("claude-code", event, Some(&said)));
            let wanted = if said == set {
                Attendance::Attended
            } else {
                Attendance::Unattended
            };
            assert_eq!(fact.attendance(), wanted, "set {set}, reported {said:?}");
        }
    }
    // No mode in the command is the harness's default.
    let mut fact = claude_code_chat(None);
    fact.heard(&line("claude-code", Event::Stop, Some("default")));
    assert_eq!(fact.attendance(), Attendance::Attended);
    // A string nobody measured is never read as attended.
    let mut fact = claude_code_chat(Some("default"));
    fact.heard(&line(
        "claude-code",
        Event::Stop,
        Some("aModeNobodyMeasured"),
    ));
    assert_eq!(fact.attendance(), Attendance::Unattended);
    // Codex reports bypass for every chat.
    let codex = line("codex", Event::Stop, Some("bypassPermissions"));
    assert!(
        codex.detail.unattended,
        "the old reader's bit is as it was (no reader moves)"
    );
    let mut fact = Fact::at_start(Begun::Person, Some(Harness::Codex), &words(&["codex"]));
    fact.heard(&codex);
    assert_eq!(fact.attendance(), Attendance::Attended);
}

/// **R-289**: a Codex chat a person started stays attended whatever mode it reports, on every
/// event; one started as a task, loop or runner is unattended.
#[test]
fn codex_attendance_comes_only_from_how_purlis_started_it() {
    purlis_core::unsteered!();
    let command = words(&["codex"]);
    let mut said: Vec<Option<String>> = measured_modes().into_iter().map(Some).collect();
    said.extend([Some("aModeNobodyMeasured".to_owned()), None]);
    let mut fact = Fact::at_start(Begun::Person, Some(Harness::Codex), &command);
    for event in EVENTS {
        for mode in &said {
            let report = line("codex", event, mode.as_deref());
            assert_eq!(
                report.detail.claude_code_mode, None,
                "Codex's word is not read"
            );
            fact.heard(&report);
            assert_eq!(
                fact.attendance(),
                Attendance::Attended,
                "{event:?} said {mode:?}"
            );
        }
    }
    assert_eq!(fact.reported(), None);
    for begun in [Begun::Task, Begun::Loop, Begun::Runner] {
        assert_eq!(
            Fact::at_start(begun, Some(Harness::Codex), &command).attendance(),
            Attendance::Unattended,
            "{begun:?}"
        );
    }
}

/// **R-318**: once unattended, every event kind with every measured mode, the one set among
/// them, a line naming none and an unproven one leave the chat unattended.
///
/// A pin of the one-way rule the old mark kept (Q208). Mutation that shows it red: have
/// [`Fact::hear`] set the fact back to attended when the reported mode is the one set.
#[test]
fn attendance_moves_one_way() {
    purlis_core::unsteered!();
    let mut flipped = claude_code_chat(Some("default"));
    flipped.heard(&line("claude-code", Event::Stop, Some("acceptEdits")));
    assert_eq!(flipped.attendance(), Attendance::Unattended);
    let task = Fact::at_start(
        Begun::Task,
        Some(Harness::ClaudeCode),
        &words(&["claude", "--permission-mode", "default"]),
    );
    let mut said: Vec<Option<String>> = measured_modes().into_iter().map(Some).collect();
    said.extend([Some("aModeNobodyMeasured".to_owned()), None]);
    for mut fact in [flipped, task] {
        for event in EVENTS {
            for mode in &said {
                let mut report = line("claude-code", event, mode.as_deref());
                fact.heard(&report);
                assert_eq!(
                    fact.attendance(),
                    Attendance::Unattended,
                    "{event:?} {mode:?}"
                );
                report.mode_unproven();
                fact.heard(&report);
                assert_eq!(
                    fact.attendance(),
                    Attendance::Unattended,
                    "unproven {mode:?}"
                );
            }
        }
    }
}

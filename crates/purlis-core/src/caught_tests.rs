// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What a tool hook's payload makes a failure of purlis's own of, and what it never does
//! (#5697). Each payload is in the shape Claude Code hands its hook.

use super::*;
use serde_json::json;

/// Claude Code's `PostToolUseFailure` for a shell command: "Exit code N", then both streams.
fn command_failed(command: &str, error: &str) -> serde_json::Value {
    json!({
        "hook_event_name": "PostToolUseFailure",
        "tool_name": "Bash",
        "tool_input": {"command": command, "description": "run it"},
        "tool_use_id": "toolu_01",
        "error": error,
        "is_interrupt": false,
    })
}

/// Claude Code's `PostToolUseFailure` for an MCP tool that answered an error.
fn tool_failed(tool: &str, error: &str) -> serde_json::Value {
    json!({
        "hook_event_name": "PostToolUseFailure",
        "tool_name": tool,
        "tool_input": {"title": "a"},
        "tool_use_id": "toolu_02",
        "error": error,
        "is_interrupt": false,
    })
}

const BASH_WORD: &str = "posttoolusefailure-blocked";

#[test]
fn a_purlis_command_that_exits_with_the_internal_failure_code_is_caught() {
    let payload = command_failed(
        "cd /Users/dev/plane && purlis worktree commit -m \"a message\" --all",
        "Exit code 70\npurlis: internal failure: the index moved under the commit\n",
    );

    let caught = of_hook(BASH_WORD, &payload).expect("caught");

    assert_eq!(caught.call, "purlis worktree commit");
    assert_eq!(caught.exit, Some(70));
    assert_eq!(
        caught.error,
        "purlis: internal failure: the index moved under the commit"
    );
}

#[test]
fn a_refusal_or_a_usage_error_of_a_purlis_command_is_never_caught() {
    // A guard's refusal and clap's usage error both exit 1, and a harness's block exits 2.
    for (command, error) in [
        (
            "purlis workspace remove beta",
            "Exit code 1\npurlis: removing a workspace is the person's to decide",
        ),
        (
            "purlis worktree comit",
            "Exit code 2\nerror: unrecognized subcommand 'comit'\n\nUsage: purlis worktree <COMMAND>",
        ),
        (
            "purlis report bug",
            "Exit code 1\nerror: a report needs words",
        ),
    ] {
        assert_eq!(
            of_hook(BASH_WORD, &command_failed(command, error)),
            None,
            "{command}"
        );
    }
}

#[test]
fn another_program_s_exit_seventy_is_never_purlis_s() {
    for command in [
        "cargo test",
        // The status a pipe hands back is its last program's.
        "purlis status | grep alpha",
        // A command purlis runs for the chat ends with that command's own status.
        "purlis secret exec -- ./deploy.sh",
    ] {
        let payload = command_failed(command, "Exit code 70\nsoftware error");
        assert_eq!(of_hook(BASH_WORD, &payload), None, "{command}");
    }
}

#[test]
fn a_purlis_command_under_its_old_name_or_after_an_assignment_is_purlis_s() {
    let payload = command_failed(
        "RUST_LOG=debug charter dispatch read 12",
        "Exit code 70\nboom",
    );
    let caught = of_hook(BASH_WORD, &payload).expect("caught");
    assert_eq!(caught.call, "charter dispatch read");
}

#[test]
fn a_purlis_tool_that_failed_inside_purlis_is_caught() {
    let payload = tool_failed(
        "mcp__purlis__session_record",
        &tool_failed_inside("task 12 panicked"),
    );

    let caught = of_hook(TOOL_WORD, &payload).expect("caught");

    assert_eq!(caught.call, "mcp__purlis__session_record");
    assert_eq!(caught.exit, None);
    assert_eq!(caught.error, tool_failed_inside("task 12 panicked"));
}

#[test]
fn a_purlis_tool_that_refused_is_never_caught() {
    let payload = tool_failed(
        "mcp__purlis__workspace_vision",
        "nothing was written: this project is read-only here",
    );
    assert_eq!(of_hook(TOOL_WORD, &payload), None);
}

#[test]
fn another_server_s_tool_is_never_purlis_s() {
    let payload = tool_failed(
        "mcp__other__thing",
        &tool_failed_inside("it says the words"),
    );
    assert_eq!(of_hook(TOOL_WORD, &payload), None);
}

#[test]
fn no_other_hook_word_is_read() {
    let payload = command_failed("purlis status", "Exit code 70\nboom");
    assert_eq!(of_hook("posttooluse", &payload), None);
    assert_eq!(of_hook("stop", &payload), None);
}

#[test]
fn the_error_is_cut_to_what_a_draft_carries() {
    let long = "x".repeat(MOST_ERROR_BYTES * 2);
    let payload = command_failed("purlis status", &format!("Exit code 70\n{long}"));
    let caught = of_hook(BASH_WORD, &payload).expect("caught");
    assert!(
        caught.error.len() <= MOST_ERROR_BYTES,
        "{}",
        caught.error.len()
    );
}

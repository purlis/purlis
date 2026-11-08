// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;

use super::*;

/// The output a harness printed, as recorded under `recorded/`.
macro_rules! recorded {
    ($path:literal) => {
        include_str!(concat!("recorded/", $path))
    };
}

fn nothing_briefed() -> Listing<'static> {
    Listing {
        cwd: Path::new("/home/runner/work/grade/piece"),
        briefed: &[],
    }
}

fn names(started: Result<Started, Unread>) -> Vec<String> {
    started.expect("the output reads").skills
}

#[test]
fn claude_code_starting_a_skill_by_its_skill_tool_is_a_start_of_it_without_the_plugin_namespace() {
    let output = recorded!("claude-code/a-start.jsonl");
    assert_eq!(
        names(started(Harness::ClaudeCode, output, nothing_briefed())),
        ["purlis-secrets"]
    );
}

#[test]
fn claude_code_loading_a_skill_through_purlis_s_skill_load_is_a_start_and_finding_one_is_not() {
    let output = recorded!("claude-code/a-load.jsonl");
    assert_eq!(
        names(started(Harness::ClaudeCode, output, nothing_briefed())),
        ["purlis-handoff"]
    );
}

#[test]
fn claude_code_naming_a_skill_in_its_text_or_calling_its_skill_tool_in_vain_starts_nothing() {
    let output = recorded!("claude-code/no-start.jsonl");
    assert!(names(started(Harness::ClaudeCode, output, nothing_briefed())).is_empty());
}

#[test]
fn output_that_is_not_the_harness_s_json_is_no_run_and_says_so() {
    let unread = started(
        Harness::ClaudeCode,
        "Error: unknown option '--output-format'\n",
        nothing_briefed(),
    )
    .expect_err("text is no run");
    assert!(unread.why.contains("line 1"), "{}", unread.why);
}

#[test]
fn a_claude_code_chat_that_errored_before_starting_anything_is_no_run_and_says_why() {
    let output = recorded!("claude-code/errored.jsonl");
    let unread =
        started(Harness::ClaudeCode, output, nothing_briefed()).expect_err("an error is no run");
    assert!(unread.why.contains("Invalid API key"), "{}", unread.why);
}

#[test]
fn output_with_nothing_in_it_is_no_run() {
    started(Harness::ClaudeCode, "", nothing_briefed()).expect_err("nothing is no run");
}

/// What a Codex chat is briefed on: purlis's skills and the crowd's, each by its `SKILL.md`.
fn briefed_on_codex() -> Vec<crate::Skill> {
    let skill = |name: &str, folder: &str| crate::Skill {
        name: name.to_owned(),
        description: format!("What {name} is for."),
        path: Path::new(folder).join(name).join("SKILL.md"),
    };
    vec![
        skill("purlis-secrets", "/home/runner/work/purlis/plugin/skills"),
        skill("purlis-browser", "/home/runner/work/purlis/plugin/skills"),
        skill("env-files", "/home/runner/work/purlis/crowd/skills"),
    ]
}

fn on_codex(output: &str) -> Result<Started, Unread> {
    let briefed = briefed_on_codex();
    started(
        Harness::Codex,
        output,
        Listing {
            cwd: Path::new("/home/runner/work/grade/piece"),
            briefed: &briefed,
        },
    )
}

#[test]
fn codex_reading_a_briefed_skill_md_in_its_shell_is_a_start_of_that_skill() {
    assert_eq!(
        names(on_codex(recorded!("codex/a-read.jsonl"))),
        ["purlis-secrets"]
    );
}

#[test]
fn codex_editing_a_skill_md_or_reading_one_that_is_not_there_starts_nothing() {
    assert!(names(on_codex(recorded!("codex/no-start.jsonl"))).is_empty());
}

#[test]
fn codex_loading_a_skill_through_purlis_s_skill_load_is_a_start_and_a_failed_load_is_not() {
    assert_eq!(
        names(on_codex(recorded!("codex/a-load.jsonl"))),
        ["purlis-handoff"]
    );
}

#[test]
fn a_codex_turn_that_failed_before_starting_anything_is_no_run_and_says_why() {
    let unread = on_codex(recorded!("codex/errored.jsonl")).expect_err("a failed turn is no run");
    assert!(unread.why.contains("401 Unauthorized"), "{}", unread.why);
}

#[test]
fn opencode_starting_a_skill_by_its_skill_tool_or_purlis_s_skill_load_is_a_start_of_each() {
    let output = recorded!("opencode/a-start.jsonl");
    assert_eq!(
        names(started(Harness::Opencode, output, nothing_briefed())),
        ["purlis-secrets", "purlis-browser"]
    );
}

#[test]
fn opencode_calling_its_skill_tool_in_vain_or_reading_a_skill_md_starts_nothing() {
    let output = recorded!("opencode/no-start.jsonl");
    assert!(names(started(Harness::Opencode, output, nothing_briefed())).is_empty());
}

#[test]
fn an_opencode_chat_that_errored_before_starting_anything_is_no_run_and_says_why() {
    let output = recorded!("opencode/errored.jsonl");
    let unread =
        started(Harness::Opencode, output, nothing_briefed()).expect_err("an error is no run");
    assert!(unread.why.contains("invalid x-api-key"), "{}", unread.why);
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Use skill…'s text on each harness (SK-12): what is typed for a skill of a chat's set, by its
//! level, with what the person gave its arguments.

use super::*;
use crate::skills::{Argument, Given, Origin};

fn invoked<'a>(name: &'a str, origin: Origin, given: &'a Given) -> SkillToStart<'a> {
    SkillToStart {
        name,
        origin,
        given,
    }
}

fn named(pairs: &[(&str, &str)]) -> Given {
    Given::Named(
        pairs
            .iter()
            .map(|(name, value)| Argument {
                name: (*name).to_owned(),
                value: (*value).to_owned(),
            })
            .collect(),
    )
}

#[test]
fn claude_code_runs_a_skill_by_its_plugins_name_with_each_argument_a_shell_word() {
    let claude = Harness::ClaudeCode;
    assert_eq!(
        claude.skill_start_text(&invoked("handoff", Origin::BuiltIn, &Given::Nothing)),
        "/purlis:handoff"
    );
    assert_eq!(
        claude.skill_start_text(&invoked("deploy", Origin::Project, &Given::Nothing)),
        "/skills:deploy"
    );
    assert_eq!(
        claude.skill_start_text(&invoked(
            "review",
            Origin::Workspace,
            &named(&[("pr", "42"), ("focus", "the tests"), ("tone", "")])
        )),
        "/skills:review 42 \"the tests\" \"\""
    );
    assert_eq!(
        claude.skill_start_text(&invoked(
            "review",
            Origin::Project,
            &Given::Raw("42 and \"more\"".to_owned())
        )),
        "/skills:review 42 and \"more\"",
        "a hinted skill's one line goes as it was typed, for $ARGUMENTS"
    );
    assert_eq!(
        claude.skill_name("handoff", Origin::BuiltIn),
        "purlis:handoff"
    );
}

#[test]
fn opencode_runs_a_skill_as_its_own_slash_command_with_each_argument_one_quoted_word() {
    let opencode = Harness::Opencode;
    // Its set holds a built-in skill as `purlis-<name>` (`Route::built_in_name`), the name
    // opencode reads in the `SKILL.md` purlis hands it.
    assert_eq!(
        opencode.skill_start_text(&invoked(
            &crate::skills::Route::Config.built_in_name("handoff"),
            Origin::BuiltIn,
            &Given::Nothing
        )),
        "/purlis-handoff"
    );
    assert_eq!(
        opencode.skill_start_text(&invoked(
            "review",
            Origin::Project,
            &named(&[
                ("pr", "42"),
                ("focus", "it's \"done\""),
                ("by", "say \"hi\"")
            ])
        )),
        "/review 42 \"it's 'done'\" 'say \"hi\"'"
    );
    assert_eq!(opencode.skill_name("deploy", Origin::Project), "deploy");
}

#[test]
fn codex_mentions_a_skill_and_says_its_arguments_in_words_it_does_not_substitute() {
    let codex = Harness::Codex;
    assert_eq!(
        codex.skill_start_text(&invoked("handoff", Origin::BuiltIn, &Given::Nothing)),
        "$handoff"
    );
    assert_eq!(
        codex.skill_start_text(&invoked(
            "review",
            Origin::Workspace,
            &named(&[("pr", ""), ("focus", "the tests")])
        )),
        "$review pr: none; focus: the tests"
    );
    assert_eq!(
        codex.skill_start_text(&invoked(
            "review",
            Origin::Project,
            &Given::Raw("#12".to_owned())
        )),
        "$review #12"
    );
    assert_eq!(codex.skill_name("deploy", Origin::Project), "deploy");
}

#[test]
fn every_harnesss_text_is_one_line_with_nothing_that_could_send_it() {
    let given = named(&[("a", "x y"), ("b", "")]);
    for harness in [Harness::ClaudeCode, Harness::Codex, Harness::Opencode] {
        let text = harness.skill_start_text(&invoked("s", Origin::Project, &given));
        assert!(!text.contains(['\n', '\r']), "{harness:?}: {text:?}");
    }
}

#[test]
fn every_harness_has_purliss_budget_and_says_what_it_lists_of_a_skill() {
    let claude = Harness::ClaudeCode.skill_budget();
    let codex = Harness::Codex.skill_budget();
    let opencode = Harness::Opencode.skill_budget();
    for budget in [claude, codex, opencode] {
        assert_eq!((budget.skills, budget.chars), (30, 6_000));
    }
    assert_eq!(claude.per_description, Some(1_536));
    assert_eq!(codex.per_description, Some(1_024));
    assert_eq!(opencode.per_description, None, "opencode cuts nothing");
    // A Codex chat is briefed with each skill's path on its line.
    assert!(codex.lists_paths);
    assert!(!claude.lists_paths && !opencode.lists_paths);
}

#[test]
fn only_claude_code_names_the_skill_a_prompt_started_and_reads_it_from_the_expansion() {
    assert_eq!(
        Harness::ClaudeCode.typed_start(),
        crate::skillspan::Confirmed::ByName
    );
    for harness in [Harness::Codex, Harness::Opencode] {
        assert_eq!(harness.typed_start(), crate::skillspan::Confirmed::ByPrompt);
    }
    let claude = Harness::ClaudeCode.adapter();
    let expansion = serde_json::json!({
        "hook_event_name": "UserPromptExpansion",
        "expansion_type": "slash_command",
        "command_name": "skills:deploy",
        "command_args": "42",
        "prompt": "/skills:deploy 42",
    });
    assert_eq!(
        claude.prompt_started(&expansion).as_deref(),
        Some("skills:deploy")
    );
    let mut other = expansion.clone();
    other["expansion_type"] = "file".into();
    assert_eq!(claude.prompt_started(&other), None);
    assert_eq!(
        claude.prompt_started(&serde_json::json!({"hook_event_name": "PostToolUse"})),
        None
    );
    assert_eq!(Harness::Codex.adapter().prompt_started(&expansion), None);
}

#[test]
fn claude_codes_name_for_a_skill_is_its_plugins_for_every_level_purlis_hands_over() {
    let claude = Harness::ClaudeCode;
    assert_eq!(claude.skill_name("a", Origin::BuiltIn), "purlis:a");
    for origin in [Origin::Project, Origin::Workspace, Origin::Persona] {
        assert_eq!(claude.skill_name("a", origin), "skills:a", "{origin:?}");
    }
    assert_eq!(claude.skill_name("a", Origin::Harness), "a");
    // The skill_load start names it the same way.
    assert_eq!(
        claude
            .adapter()
            .narrowed_start("a", Origin::Project)
            .as_deref(),
        Some("skills:a")
    );
}

#[test]
fn a_word_is_quoted_only_where_it_must_be() {
    assert_eq!(shell_word("plain"), "plain");
    assert_eq!(shell_word(""), "\"\"");
    assert_eq!(shell_word("two words"), "\"two words\"");
    assert_eq!(shell_word("it's"), "\"it's\"");
    assert_eq!(shell_word("say \"hi\""), "'say \"hi\"'");
    assert_eq!(shell_word("it's \"x\""), "\"it's 'x'\"");
}

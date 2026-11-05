// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use purlis_skills::runs::StartedBy;

use super::SkillPrompt;
use crate::harness::Harness;
use crate::skills::Origin;
use crate::skillspan::{Confirmed, PromptMarks, PromptSalt};

fn salt() -> PromptSalt {
    PromptSalt::mint().unwrap()
}

fn evals(origin: Origin) -> SkillPrompt {
    SkillPrompt {
        skill: "purlis-skill-studio".to_owned(),
        origin,
        words: "write outcome evals for login in skills/login".to_owned(),
        sentence: "Use the purlis-skill-studio skill to write them.".to_owned(),
        direct: true,
    }
}

#[test]
fn claude_code_starts_a_built_in_skill_by_its_plugin_and_name() {
    assert_eq!(
        evals(Origin::BuiltIn).typed_on(Some(Harness::ClaudeCode)),
        "/purlis:purlis-skill-studio write outcome evals for login in skills/login"
    );
}

#[test]
fn claude_code_starts_a_skill_of_the_persons_set_under_the_linked_plugin() {
    assert_eq!(
        evals(Origin::Project).typed_on(Some(Harness::ClaudeCode)),
        "/skills:purlis-skill-studio write outcome evals for login in skills/login"
    );
}

#[test]
fn opencode_starts_a_skill_by_its_bare_name() {
    for origin in [Origin::BuiltIn, Origin::Project] {
        assert_eq!(
            evals(origin).typed_on(Some(Harness::Opencode)),
            "/purlis-skill-studio write outcome evals for login in skills/login"
        );
    }
}

#[test]
fn codex_is_only_told_of_purliss_skills_so_it_gets_the_sentence() {
    for origin in [
        Origin::BuiltIn,
        Origin::Project,
        Origin::Workspace,
        Origin::Persona,
    ] {
        assert_eq!(
            evals(origin).typed_on(Some(Harness::Codex)),
            "Use the purlis-skill-studio skill to write them."
        );
    }
}

#[test]
fn a_harness_purlis_has_no_adapter_for_gets_the_sentence() {
    assert_eq!(
        evals(Origin::BuiltIn).typed_on(None),
        "Use the purlis-skill-studio skill to write them."
    );
}

#[test]
fn a_direct_start_is_one_line_when_its_words_are() {
    for harness in Harness::ALL {
        assert!(
            !evals(Origin::BuiltIn)
                .typed_on(Some(harness))
                .contains('\n')
        );
    }
}

#[test]
fn a_direct_start_holds_its_run_under_the_name_the_harness_reports() {
    let salt = salt();
    let held = evals(Origin::BuiltIn)
        .run_held(
            Some(Harness::ClaudeCode),
            &salt,
            Some("ops".to_owned()),
            StartedBy::Curation,
        )
        .unwrap();
    assert_eq!(held.skill, "purlis:purlis-skill-studio");
    assert_eq!(held.started_by, StartedBy::Curation);
    assert_eq!(held.confirmed, Confirmed::ByName);
    assert_eq!(held.workspace.as_deref(), Some("ops"));
    assert!(held.waits);
}

#[test]
fn a_direct_start_confirmed_by_its_prompt_is_found_however_long_its_words_are() {
    let salt = salt();
    let prompt = evals(Origin::BuiltIn);
    let held = prompt
        .run_held(Some(Harness::Opencode), &salt, None, StartedBy::UseSkill)
        .unwrap();
    assert_eq!(held.confirmed, Confirmed::ByPrompt);
    let sent = prompt.typed_on(Some(Harness::Opencode));
    assert!(PromptMarks::of(&sent, &salt).holds(held.text));
}

#[test]
fn the_sentence_holds_no_run_because_the_chat_starts_the_skill_its_own_way() {
    let salt = salt();
    let prompt = evals(Origin::BuiltIn);
    assert!(
        prompt
            .run_held(Some(Harness::Codex), &salt, None, StartedBy::UseSkill)
            .is_none()
    );
    assert!(
        prompt
            .run_held(None, &salt, None, StartedBy::UseSkill)
            .is_none()
    );
}

#[test]
fn a_skill_that_cannot_be_started_by_name_is_only_named_on_every_harness() {
    let prompt = SkillPrompt {
        direct: false,
        ..evals(Origin::Project)
    };
    for harness in Harness::ALL {
        assert_eq!(
            prompt.typed_on(Some(harness)),
            "Use the purlis-skill-studio skill to write them."
        );
    }
}

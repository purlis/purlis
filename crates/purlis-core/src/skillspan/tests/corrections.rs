// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A correction records a miss** (#5738, spec #5714): the person starting a built-in skill
//! themselves, by Use skill… or a slash command, in the prompt right after a turn that did not
//! start it. Not on a chat's first prompt, and not when the turn before started it.

use super::*;
use purlis_skills::runs::Level;

const BUILT_IN: Level = Level {
    origin: crate::skills::Origin::BuiltIn,
    local: false,
};

const PROJECT: Level = Level {
    origin: crate::skills::Origin::Project,
    local: false,
};

/// Claude Code saying a prompt the person sent started `skill`, a copy at `level`.
fn expanded(skill: &str, level: Level) -> ToolCall {
    let mut call = named(skill);
    call.skill.as_mut().unwrap().level = Some(level);
    call
}

/// The agent starting `skill`, a copy at `level`, with its own Skill tool.
fn agent_started(skill: &str, level: Level) -> ToolCall {
    let mut call = named(skill);
    call.tool_hook = "posttooluse-skill".to_owned();
    call.tool = Some("Skill".to_owned());
    call.call = Some(format!("toolu_{skill}"));
    let started = call.skill.as_mut().unwrap();
    started.started_by = StartedBy::SkillTool;
    started.level = Some(level);
    call
}

/// What Use skill… holds as it types built-in `skill` into a chat whose prompts mark it.
fn typed_built_in(skill: &str, text: &str) -> Typed {
    Typed {
        level: Some(BUILT_IN),
        ..typed(skill, RUN, text, Confirmed::ByPrompt)
    }
}

/// A chat that has had one turn already: its first prompt and that turn's end.
fn after_a_turn() -> Tracker {
    let mut tracker = Tracker::default();
    tracker.report("4", &prompt("set up the release"), 0, || None);
    tracker.report("4", &report(Event::Stop), 1, || None);
    tracker
}

fn missed(tracker: &mut Tracker) -> Vec<String> {
    tracker
        .missed("4")
        .into_iter()
        .map(|missed| missed.skill)
        .collect()
}

#[test]
fn a_built_in_the_person_types_by_hand_after_a_turn_without_it_is_a_miss_by_its_own_name() {
    let mut tracker = after_a_turn();
    tracker.report("4", &prompt("/purlis:purlis-secrets"), 2, || None);
    tracker.call("4", &expanded("purlis:purlis-secrets", BUILT_IN), 3, || {
        None
    });
    assert_eq!(missed(&mut tracker), ["purlis-secrets"]);
    assert!(missed(&mut tracker).is_empty(), "taken once");
}

#[test]
fn a_built_in_named_before_its_prompt_is_sent_is_decided_by_that_prompt() {
    let mut tracker = after_a_turn();
    tracker.call("4", &expanded("purlis:purlis-secrets", BUILT_IN), 2, || {
        None
    });
    assert!(missed(&mut tracker).is_empty(), "not sent yet");
    tracker.report("4", &prompt("/purlis:purlis-secrets"), 3, || None);
    assert_eq!(missed(&mut tracker), ["purlis-secrets"]);
}

#[test]
fn a_built_in_use_skill_typed_after_a_turn_without_it_is_a_miss_on_every_harness() {
    let mut tracker = after_a_turn();
    tracker.typed("4", typed_built_in("purlis-handoff", "/purlis-handoff"), 2);
    let sent = tracker.report("4", &prompt("/purlis-handoff"), 3, || None);
    assert_eq!(sent.opened.len(), 1, "its run opens as before");
    assert_eq!(missed(&mut tracker), ["purlis-handoff"]);
}

#[test]
fn nothing_is_missed_on_a_chats_first_prompt() {
    let mut tracker = Tracker::default();
    tracker.typed("4", typed_built_in("purlis-handoff", "/purlis-handoff"), 0);
    tracker.report("4", &prompt("/purlis-handoff"), 1, || None);
    tracker.call("4", &expanded("purlis:purlis-secrets", BUILT_IN), 2, || {
        None
    });
    assert!(missed(&mut tracker).is_empty());

    let mut named_first = Tracker::default();
    named_first.call("4", &expanded("purlis:purlis-secrets", BUILT_IN), 0, || {
        None
    });
    named_first.report("4", &prompt("/purlis:purlis-secrets"), 1, || None);
    assert!(missed(&mut named_first).is_empty());
}

#[test]
fn nothing_is_missed_when_the_turn_before_started_that_skill() {
    let mut tracker = Tracker::default();
    tracker.report("4", &prompt("keep this token safe"), 0, || None);
    tracker.call(
        "4",
        &agent_started("purlis:purlis-secrets", BUILT_IN),
        1,
        || None,
    );
    tracker.report("4", &report(Event::Stop), 2, || None);
    tracker.report("4", &prompt("/purlis:purlis-secrets"), 3, || None);
    tracker.call("4", &expanded("purlis:purlis-secrets", BUILT_IN), 4, || {
        None
    });
    assert!(missed(&mut tracker).is_empty());

    // Only the turn right before counts: one more turn without it, and it is a miss again.
    tracker.report("4", &report(Event::Stop), 5, || None);
    tracker.report("4", &prompt("something else"), 6, || None);
    tracker.report("4", &report(Event::Stop), 7, || None);
    tracker.report("4", &prompt("/purlis:purlis-secrets"), 8, || None);
    tracker.call("4", &expanded("purlis:purlis-secrets", BUILT_IN), 9, || {
        None
    });
    assert_eq!(missed(&mut tracker), ["purlis-secrets"]);
}

#[test]
fn a_skill_that_is_not_a_built_in_and_a_start_the_agent_made_are_no_miss() {
    let mut tracker = after_a_turn();
    tracker.report("4", &prompt("/skills:deploy"), 2, || None);
    tracker.call("4", &expanded("skills:deploy", PROJECT), 3, || None);
    tracker.call(
        "4",
        &agent_started("purlis:purlis-handoff", BUILT_IN),
        4,
        || None,
    );
    assert!(missed(&mut tracker).is_empty());
}

#[test]
fn a_chat_whose_session_ends_forgets_its_turns() {
    let mut tracker = after_a_turn();
    tracker.report("4", &report(Event::SessionEnd), 2, || None);
    tracker.report("4", &prompt("/purlis:purlis-secrets"), 3, || None);
    tracker.call("4", &expanded("purlis:purlis-secrets", BUILT_IN), 4, || {
        None
    });
    assert!(
        missed(&mut tracker).is_empty(),
        "the first prompt of its next session"
    );
}

#[test]
fn a_miss_says_its_skill_and_nothing_of_the_prompt() {
    let mut tracker = after_a_turn();
    tracker.report(
        "4",
        &prompt("/purlis:purlis-secrets my deploy token"),
        2,
        || None,
    );
    tracker.call("4", &expanded("purlis:purlis-secrets", BUILT_IN), 3, || {
        None
    });
    let [miss] = &tracker.missed("4")[..] else {
        panic!("one miss");
    };
    let dumped = format!("{miss:?}");
    assert!(!dumped.contains("deploy"), "{dumped}");
}

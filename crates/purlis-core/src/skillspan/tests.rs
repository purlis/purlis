// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::hookwire::{Conversation, Report, SkillStarted, ToolCall};
use crate::state::{Detail, Event};

const RUN: &str = "01JB0000000000000000000001";
const OTHER: &str = "01JB0000000000000000000002";

fn report(event: Event) -> Report {
    Report {
        chat: 4,
        event,
        conversation: Conversation::Unknown,
        pid: None,
        agent: None,
        detail: Detail::default(),
    }
}

/// A prompt the person sent, whose text was `prompt`, as its hook marks it.
fn prompt(prompt: &str) -> Report {
    let mut sent = report(Event::UserPromptSubmit);
    sent.detail.prompt_marks = PromptMarks::of(prompt);
    sent
}

fn typed(skill: &str, run: &str, text: &str, confirmed: Confirmed) -> Typed {
    Typed {
        skill: skill.to_owned(),
        run: run.parse().unwrap(),
        workspace: Some("ops".to_owned()),
        level: None,
        text: PromptMarks::digest(text),
        confirmed,
    }
}

/// The line a hook sends as Claude Code says a prompt the person sent started `skill`.
fn named(skill: &str) -> ToolCall {
    ToolCall {
        chat: 4,
        tool_hook: "userpromptexpansion".to_owned(),
        tool: None,
        call: None,
        args: None,
        decision: crate::hookwire::Decision::None,
        rule: None,
        hook_ms: 1,
        agent: None,
        at_ms: 0,
        skill: Some(Box::new(SkillStarted {
            skill: skill.to_owned(),
            run: crate::reopen::mint(),
            started_by: StartedBy::UseSkill,
            workspace: None,
            level: None,
        })),
    }
}

#[test]
fn a_typed_skill_opens_with_the_next_prompt_that_holds_its_text_started_by_use_skill() {
    let mut tracker = Tracker::default();
    tracker.typed(
        "4",
        typed("deploy", RUN, "$deploy env: prod", Confirmed::ByPrompt),
        0,
    );
    assert!(
        tracker.inside("4").is_none(),
        "nothing runs until it is sent"
    );
    // A turn that ends before the prompt is sent opens nothing, and keeps it held.
    let stopped = tracker.report("4", &report(Event::Stop), 10, || None);
    assert!(stopped.opened.is_empty());

    let sent = tracker.report("4", &prompt("$deploy env: prod\nand be quick"), 20, || {
        Some(7)
    });
    let [opened] = &sent.opened[..] else {
        panic!("one run opens: {:?}", sent.opened);
    };
    assert_eq!(opened.skill, "deploy");
    assert_eq!(opened.id, RUN);
    assert_eq!(opened.started_by, StartedBy::UseSkill);
    assert_eq!(opened.tokens_at_start, Some(7));
    assert_eq!(opened.whose.workspace.as_deref(), Some("ops"));

    // Once: the prompt after it opens nothing more, and closes it as interrupted.
    let next = tracker.report("4", &prompt("$deploy env: prod"), 30, || None);
    assert!(next.opened.is_empty());
    assert_eq!(next.closed.len(), 1);
    assert_eq!(next.closed[0].ended, Ended::Interrupted);
}

#[test]
fn a_prompt_that_does_not_hold_the_typed_text_drops_it_unopened() {
    let mut tracker = Tracker::default();
    tracker.typed(
        "4",
        typed("deploy", RUN, "$deploy env: prod", Confirmed::ByPrompt),
        0,
    );
    let other = tracker.report("4", &prompt("something else entirely"), 1, || None);
    assert!(other.opened.is_empty());
    let later = tracker.report("4", &prompt("$deploy env: prod"), 2, || None);
    assert!(later.opened.is_empty(), "the next prompt decided");
}

#[test]
fn every_pick_is_held_beside_the_others_and_each_opens_when_its_text_is_sent() {
    let mut tracker = Tracker::default();
    tracker.typed("4", typed("deploy", RUN, "/deploy", Confirmed::ByPrompt), 0);
    tracker.typed(
        "4",
        typed("review", OTHER, "/review 42", Confirmed::ByPrompt),
        1,
    );
    let sent = tracker.report("4", &prompt("/deploy\n/review 42"), 2, || None);
    let opened: Vec<&str> = sent.opened.iter().map(|one| one.skill.as_str()).collect();
    assert_eq!(opened, ["deploy", "review"]);
}

#[test]
fn a_held_skill_goes_after_ten_minutes_at_the_session_end_or_with_the_chat() {
    let mut tracker = Tracker::default();
    tracker.typed("4", typed("deploy", RUN, "/deploy", Confirmed::ByPrompt), 0);
    let late = tracker.report("4", &prompt("/deploy"), HELD_FOR_MS + 1, || None);
    assert!(late.opened.is_empty(), "held too long");

    tracker.typed("4", typed("deploy", RUN, "/deploy", Confirmed::ByPrompt), 0);
    tracker.report("4", &report(Event::SessionEnd), 1, || None);
    assert!(
        tracker
            .report("4", &prompt("/deploy"), 2, || None)
            .opened
            .is_empty()
    );

    tracker.typed("4", typed("deploy", RUN, "/deploy", Confirmed::ByPrompt), 0);
    tracker.close("4", Ended::Session, 1);
    assert!(
        tracker
            .report("4", &prompt("/deploy"), 2, || None)
            .opened
            .is_empty()
    );
}

#[test]
fn a_skill_confirmed_by_name_opens_when_its_harness_names_it_before_or_after_the_prompt() {
    // Named after the prompt was sent: it opens on the name.
    let mut tracker = Tracker::default();
    tracker.typed(
        "4",
        typed("skills:deploy", RUN, "/skills:deploy", Confirmed::ByName),
        0,
    );
    let sent = tracker.report("4", &prompt("/skills:deploy"), 1, || None);
    assert!(sent.opened.is_empty(), "the prompt alone does not open it");
    let opened = tracker
        .call("4", &named("skills:deploy"), 2, || None)
        .unwrap();
    assert_eq!(opened.id, RUN);
    assert_eq!(opened.started_by, StartedBy::UseSkill);
    assert_eq!(
        tracker.inside("4").map(|open| open.signals.tool_calls),
        Some(0),
        "the name is no tool call"
    );

    // Named first: it opens with the prompt.
    let mut tracker = Tracker::default();
    tracker.typed(
        "4",
        typed("skills:deploy", RUN, "/skills:deploy", Confirmed::ByName),
        0,
    );
    assert!(
        tracker
            .call("4", &named("skills:deploy"), 1, || None)
            .is_none()
    );
    let sent = tracker.report("4", &prompt("anything"), 2, || None);
    assert_eq!(sent.opened.len(), 1);
}

#[test]
fn a_name_that_is_not_held_opens_nothing_and_a_turn_that_never_names_it_drops_it() {
    let mut tracker = Tracker::default();
    // Someone typed `/skills:other` by hand: no run is opened for it here.
    assert!(
        tracker
            .call("4", &named("skills:other"), 0, || None)
            .is_none()
    );

    tracker.typed(
        "4",
        typed("skills:deploy", RUN, "/skills:deploy", Confirmed::ByName),
        0,
    );
    tracker.report("4", &prompt("/skills:deploy"), 1, || None);
    tracker.report("4", &report(Event::Stop), 2, || None);
    assert!(
        tracker
            .call("4", &named("skills:deploy"), 3, || None)
            .is_none(),
        "its turn ended without it"
    );
}

#[test]
fn a_sub_agents_prompt_opens_nothing_typed_into_its_chat() {
    let mut tracker = Tracker::default();
    tracker.typed("4", typed("deploy", RUN, "/deploy", Confirmed::ByPrompt), 0);
    let mut of_an_agent = prompt("/deploy");
    of_an_agent.agent = Some("agent-1".to_owned());
    assert!(
        tracker
            .report("4", &of_an_agent, 1, || None)
            .opened
            .is_empty()
    );
    let sent = tracker.report("4", &prompt("/deploy"), 2, || None);
    assert_eq!(sent.opened.len(), 1, "still held for the chat's own prompt");
}

#[test]
fn a_prompts_marks_hold_each_lines_stretches_and_nothing_else() {
    let marks = PromptMarks::of("  /deploy  /review   42 \"the tests\"\nplease\n");
    for held in [
        "/deploy",
        "/review 42 \"the tests\"",
        "/deploy /review",
        "please",
        "42",
    ] {
        assert!(marks.holds(PromptMarks::digest(held)), "{held}");
    }
    assert!(
        !marks.holds(PromptMarks::digest("/review please")),
        "across lines"
    );
    assert!(!marks.holds(PromptMarks::digest("/lint")));
    assert!(PromptMarks::of("").is_empty());
    assert!(!marks.is_empty());
}

#[test]
fn use_skill_is_a_start_word_of_its_own() {
    assert_eq!(StartedBy::UseSkill.word(), "use-skill");
    assert_eq!(StartedBy::parse("use-skill"), Some(StartedBy::UseSkill));
    assert_eq!(
        serde_json::to_value(StartedBy::UseSkill).unwrap(),
        serde_json::json!("use-skill")
    );
}

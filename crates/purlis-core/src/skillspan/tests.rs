// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::hookwire::{Conversation, Report, SkillStarted, ToolCall};
use crate::state::{Detail, Event};

const RUN: &str = "01JB0000000000000000000001";
const OTHER: &str = "01JB0000000000000000000002";

/// The chat's own salt: what its hook and the window digest its prompts under.
fn salt() -> PromptSalt {
    PromptSalt::of_chat("01JB00000000000000000000C4").expect("a chat's id")
}

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
    sent.detail.prompt_marks = PromptMarks::of(prompt, &salt());
    sent
}

fn typed(skill: &str, run: &str, text: &str, confirmed: Confirmed) -> Typed {
    Typed {
        skill: skill.to_owned(),
        run: run.parse().unwrap(),
        workspace: Some("ops".to_owned()),
        level: None,
        text: PromptMarks::digest(text, &salt()),
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
    let marks = PromptMarks::of("  /deploy  /review   42 \"the tests\"\nplease\n", &salt());
    for held in [
        "/deploy",
        "/review 42 \"the tests\"",
        "/deploy /review",
        "please",
        "42",
    ] {
        assert!(marks.holds(PromptMarks::digest(held, &salt())), "{held}");
    }
    assert!(
        !marks.holds(PromptMarks::digest("/review please", &salt())),
        "across lines"
    );
    assert!(!marks.holds(PromptMarks::digest("/lint", &salt())));
    assert!(PromptMarks::of("", &salt()).is_empty());
    assert!(!marks.is_empty());
}

#[test]
fn a_prompts_marks_are_salted_by_its_chat_so_another_chats_digest_of_the_same_words_misses() {
    // SK-38 (#3029): the same words digest apart in two chats, so a filter of a short prompt is
    // not matched against digests made anywhere but its own chat.
    let other = PromptSalt::of_chat("01JB00000000000000000000C5").expect("a chat's id");
    assert_ne!(
        PromptMarks::digest("/deploy", &salt()),
        PromptMarks::digest("/deploy", &other)
    );
    let marks = PromptMarks::of("/deploy", &salt());
    assert!(marks.holds(PromptMarks::digest("/deploy", &salt())));
    assert!(!marks.holds(PromptMarks::digest("/deploy", &other)));
    assert_eq!(PromptSalt::of_chat("  "), None, "no id, no salt");
}

#[test]
fn a_debug_dump_of_a_salt_or_of_a_prompts_marks_shows_neither() {
    assert_eq!(format!("{:?}", salt()), "PromptSalt(..)");
    let marks = PromptMarks::of("/deploy now", &salt());
    assert_eq!(format!("{marks:?}"), "PromptMarks(..)");
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

/// A tool call that starts `skill` by the harness's own route `started_by`.
fn started(skill: &str, started_by: StartedBy) -> ToolCall {
    let mut call = named(skill);
    call.tool_hook = "posttooluse-skill".to_owned();
    call.tool = Some("Skill".to_owned());
    if let Some(started) = call.skill.as_mut() {
        started.started_by = started_by;
    }
    call
}

const CURATION: &str = "Use purlis's compact skill to compact and improve the persona ops.";

#[test]
fn a_skill_the_turn_of_a_curation_prompt_starts_is_started_by_the_curation_action() {
    let mut tracker = Tracker::default();
    tracker.prompted("4", Prompted::of(StartedBy::Curation, CURATION, &salt()));
    tracker.report("4", &prompt(CURATION), 0, || None);
    let opened = tracker
        .call(
            "4",
            &started("purlis:compact", StartedBy::SkillTool),
            1,
            || None,
        )
        .expect("the harness's pick opens the run");
    assert_eq!(opened.started_by, StartedBy::Curation);
    // A skill started inside it is the harness's own pick.
    let child = tracker
        .call("4", &started("memory", StartedBy::Read), 2, || None)
        .expect("a child run opens");
    assert_eq!(child.started_by, StartedBy::Read);

    // The next turn is the person's own.
    tracker.report("4", &report(Event::Stop), 3, || None);
    tracker.report("4", &prompt("now compact the workspace too"), 4, || None);
    let next = tracker
        .call(
            "4",
            &started("purlis:compact", StartedBy::SkillTool),
            5,
            || None,
        )
        .expect("a run opens");
    assert_eq!(next.started_by, StartedBy::SkillTool);
}

#[test]
fn a_curation_prompt_the_person_replaced_before_sending_starts_nothing_of_its_own() {
    let mut tracker = Tracker::default();
    tracker.prompted("4", Prompted::of(StartedBy::Curation, CURATION, &salt()));
    tracker.report("4", &prompt("something else entirely"), 0, || None);
    let opened = tracker
        .call(
            "4",
            &started("purlis:compact", StartedBy::McpLoad),
            1,
            || None,
        )
        .expect("a run opens");
    assert_eq!(opened.started_by, StartedBy::McpLoad);
}

#[test]
fn a_curation_prompt_is_held_until_it_is_sent_and_goes_with_the_session() {
    let mut tracker = Tracker::default();
    tracker.prompted("4", Prompted::of(StartedBy::Curation, CURATION, &salt()));
    // Read for long before it is sent: still the curation action's.
    tracker.report("4", &prompt(CURATION), HELD_FOR_MS * 3, || None);
    let opened = tracker
        .call(
            "4",
            &started("purlis:compact", StartedBy::SkillTool),
            1,
            || None,
        )
        .expect("a run opens");
    assert_eq!(opened.started_by, StartedBy::Curation);

    tracker.prompted("4", Prompted::of(StartedBy::Curation, CURATION, &salt()));
    tracker.report("4", &report(Event::SessionEnd), 2, || None);
    tracker.report("4", &prompt(CURATION), 3, || None);
    let after = tracker
        .call("4", &started("theme", StartedBy::SkillTool), 4, || None)
        .expect("a run opens");
    assert_eq!(after.started_by, StartedBy::SkillTool);
}

#[test]
fn only_a_curation_action_or_a_trigger_is_held_for_a_prompt() {
    let mut tracker = Tracker::default();
    tracker.prompted("4", Prompted::of(StartedBy::Read, CURATION, &salt()));
    tracker.report("4", &prompt(CURATION), 0, || None);
    let opened = tracker
        .call(
            "4",
            &started("purlis:compact", StartedBy::SkillTool),
            1,
            || None,
        )
        .expect("a run opens");
    assert_eq!(opened.started_by, StartedBy::SkillTool);
}

#[test]
fn every_start_has_a_word_of_its_own_read_back_as_itself() {
    let all = [
        StartedBy::SkillTool,
        StartedBy::Read,
        StartedBy::McpLoad,
        StartedBy::UseSkill,
        StartedBy::Curation,
        StartedBy::Trigger,
    ];
    let words: std::collections::HashSet<&str> = all.iter().map(|by| by.word()).collect();
    assert_eq!(words.len(), all.len());
    for by in all {
        assert_eq!(StartedBy::parse(by.word()), Some(by));
        assert_eq!(
            serde_json::to_value(by).unwrap(),
            serde_json::json!(by.word())
        );
    }
}

#[test]
fn a_debug_dump_of_a_prompt_held_says_what_put_it_there_and_nothing_of_the_prompt() {
    let prompted = Prompted::of(StartedBy::Curation, CURATION, &salt());
    let dumped = format!("{prompted:?}");
    assert!(dumped.contains("Curation"), "{dumped}");
    assert!(!dumped.contains(&prompted.lead.to_string()), "{dumped}");
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::hookwire::{Conversation, Report};
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

fn typed(skill: &str, run: &str) -> Typed {
    Typed {
        skill: skill.to_owned(),
        run: run.to_owned(),
    }
}

#[test]
fn a_typed_skill_opens_its_run_as_the_next_prompt_started_by_use_skill() {
    let mut tracker = Tracker::default();
    tracker.typed("4", typed("purlis:handoff", RUN));
    assert!(
        tracker.inside("4").is_none(),
        "nothing runs until it is sent"
    );
    // A turn that ends before the prompt is sent opens nothing.
    let stopped = tracker.report("4", &report(Event::Stop), 10, || None);
    assert!(stopped.opened.is_none());

    let sent = tracker.report("4", &report(Event::UserPromptSubmit), 20, || Some(7));
    let opened = sent.opened.expect("the prompt opens it");
    assert_eq!(opened.skill, "purlis:handoff");
    assert_eq!(opened.id, RUN);
    assert_eq!(opened.started_by, StartedBy::UseSkill);
    assert_eq!(opened.tokens_at_start, Some(7));
    assert_eq!(tracker.inside("4").map(|open| open.id.as_str()), Some(RUN));

    // Once: the prompt after it opens nothing more, and closes it as interrupted.
    let next = tracker.report("4", &report(Event::UserPromptSubmit), 30, || None);
    assert!(next.opened.is_none());
    assert_eq!(next.closed.len(), 1);
    assert_eq!(next.closed[0].ended, Ended::Interrupted);
}

#[test]
fn a_second_typing_replaces_the_first_and_the_chats_end_drops_it() {
    let mut tracker = Tracker::default();
    tracker.typed("4", typed("deploy", RUN));
    tracker.typed("4", typed("review", OTHER));
    let sent = tracker.report("4", &report(Event::UserPromptSubmit), 1, || None);
    assert_eq!(
        sent.opened.map(|open| open.skill),
        Some("review".to_owned())
    );

    tracker.typed("4", typed("deploy", RUN));
    tracker.report("4", &report(Event::SessionEnd), 2, || None);
    let after = tracker.report("4", &report(Event::UserPromptSubmit), 3, || None);
    assert!(after.opened.is_none(), "the session's end took it");

    tracker.typed("4", typed("deploy", RUN));
    tracker.close("4", Ended::Session, 4);
    let after = tracker.report("4", &report(Event::UserPromptSubmit), 5, || None);
    assert!(after.opened.is_none(), "the chat's close took it");
}

#[test]
fn a_typed_skill_is_a_claim_and_one_with_no_name_or_no_ulid_holds_nothing() {
    let mut tracker = Tracker::default();
    tracker.typed("4", typed("  ", RUN));
    tracker.typed("5", typed("deploy", "not-a-ulid"));
    for key in ["4", "5"] {
        let sent = tracker.report(key, &report(Event::UserPromptSubmit), 1, || None);
        assert!(sent.opened.is_none(), "{key}");
    }
}

#[test]
fn a_sub_agents_prompt_opens_nothing_typed_into_its_chat() {
    let mut tracker = Tracker::default();
    tracker.typed("4", typed("deploy", RUN));
    let mut of_an_agent = report(Event::UserPromptSubmit);
    of_an_agent.agent = Some("agent-1".to_owned());
    assert!(
        tracker
            .report("4", &of_an_agent, 1, || None)
            .opened
            .is_none()
    );
    let sent = tracker.report("4", &report(Event::UserPromptSubmit), 2, || None);
    assert!(
        sent.opened.is_some(),
        "still held for the chat's own prompt"
    );
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

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use purlis_core::hookwire::{Conversation, Decision, Report, SkillStarted, ToolCall};
use purlis_core::state::{Detail, Event};
use std::sync::mpsc;

const RUN: &str = "01JB0000000000000000000001";

fn call(chat: u32, id: &str, skill: Option<&str>) -> ToolCall {
    ToolCall {
        chat,
        tool_hook: "posttooluse-skill".to_owned(),
        tool: Some("Skill".to_owned()),
        call: Some(id.to_owned()),
        args: None,
        decision: Decision::None,
        rule: None,
        hook_ms: 1,
        agent: None,
        at_ms: 0,
        skill: skill.map(|skill| {
            Box::new(SkillStarted {
                skill: skill.to_owned(),
                run: RUN.to_owned(),
                started_by: purlis_core::skills::runs::StartedBy::SkillTool,
                workspace: None,
                level: None,
            })
        }),
    }
}

fn report(chat: u32, event: Event) -> Report {
    Report {
        chat,
        event,
        conversation: Conversation::Unknown,
        pid: None,
        agent: None,
        detail: Detail::default(),
    }
}

fn listening() -> (Arc<SkillRuns>, mpsc::Receiver<ChatSkillRun>) {
    let runs = SkillRuns::new(PlaneId::for_tests(std::path::Path::new("/planes/p")));
    let (tx, rx) = mpsc::channel();
    let tx = Mutex::new(tx);
    runs.tell_to(Arc::new(move |told| {
        let _ = tx.lock().unwrap().send(told);
    }));
    (runs, rx)
}

#[test]
fn a_skill_started_puts_its_run_on_the_chats_tab_counting_its_tool_calls_until_the_turn_ends() {
    let (runs, told) = listening();

    runs.heard(&call(4, "toolu_1", None));
    runs.heard(&call(4, "toolu_2", Some("deploy")));
    runs.heard(&call(4, "toolu_3", None));
    runs.heard(&call(4, "toolu_3", None));
    runs.reported(&report(4, Event::Stop));

    let said: Vec<_> = told.try_iter().map(|t| t.run).collect();
    let open = |tool_calls| {
        Some(OpenRun {
            skill: "deploy".to_owned(),
            id: RUN.to_owned(),
            tool_calls,
        })
    };
    assert_eq!(
        said,
        vec![open(1), open(2), None],
        "nothing before the skill, each call once and told once, and gone with the turn"
    );
    assert!(runs.now([4]).is_empty());
}

#[test]
fn a_window_that_opens_is_told_each_chats_open_run_and_a_closed_chat_loses_its_own() {
    let (runs, told) = listening();
    runs.heard(&call(4, "toolu_1", Some("deploy")));
    runs.heard(&call(5, "toolu_1", None));

    let now = runs.now([4, 5]);
    assert_eq!(now.len(), 1);
    assert_eq!(now[0].session, 4);

    runs.closed(4);
    assert_eq!(
        told.try_iter().last().map(|t| (t.session, t.run)),
        Some((4, None))
    );
}

#[test]
fn a_refused_skill_call_or_a_sub_agents_stop_changes_nothing() {
    let (runs, told) = listening();
    let mut refused = call(4, "toolu_1", Some("deploy"));
    refused.decision = Decision::Deny;
    runs.heard(&refused);
    assert!(runs.now([4]).is_empty());

    runs.heard(&call(4, "toolu_2", Some("deploy")));
    let mut child = report(4, Event::Stop);
    child.agent = Some("agent-1".to_owned());
    runs.reported(&child);
    assert_eq!(runs.now([4]).len(), 1);
    assert_eq!(told.try_iter().count(), 1);
}

#[test]
fn a_skill_use_skill_typed_is_on_the_chats_tab_from_the_prompt_it_was_typed_into() {
    let (runs, told) = listening();
    use purlis_core::skillspan::{Confirmed, PromptMarks, PromptSalt, Typed};
    let salt = PromptSalt::of_chat("01JB00000000000000000000C4").unwrap();
    runs.typed(
        4,
        Typed {
            skill: "deploy".to_owned(),
            run: RUN.parse().unwrap(),
            workspace: None,
            level: None,
            text: PromptMarks::digest("$deploy", &salt),
            confirmed: Confirmed::ByPrompt,
        },
    );
    assert!(runs.now([4]).is_empty(), "nothing runs until it is sent");
    assert_eq!(told.try_iter().count(), 0);

    let mut sent = report(4, Event::UserPromptSubmit);
    sent.detail.prompt_marks = PromptMarks::of("$deploy", &salt);
    runs.reported(&sent);
    runs.heard(&call(4, "toolu_1", None));
    runs.reported(&report(4, Event::Stop));

    let said: Vec<_> = told.try_iter().map(|t| t.run).collect();
    let open = |tool_calls| {
        Some(OpenRun {
            skill: "deploy".to_owned(),
            id: RUN.to_owned(),
            tool_calls,
        })
    };
    assert_eq!(said, vec![open(0), open(1), None]);
}

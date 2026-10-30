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

/// A skill call that starts `skill`, which the hook found is the copy at `origin`.
fn call_of(chat: u32, skill: &str, origin: purlis_core::skills::Origin) -> ToolCall {
    let mut call = call(chat, "toolu_1", Some(skill));
    if let Some(started) = call.skill.as_mut() {
        started.level = Some(purlis_core::skills::runs::Level {
            origin,
            local: false,
        });
    }
    call
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
            name: "deploy".to_owned(),
            id: RUN.to_owned(),
            tool_calls,
            tokens: None,
            level: None,
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
            name: "deploy".to_owned(),
            id: RUN.to_owned(),
            tool_calls,
            tokens: None,
            level: None,
        })
    };
    assert_eq!(said, vec![open(0), open(1), None]);
}

#[test]
fn a_runs_chip_opens_the_skill_the_core_counts_it_a_run_of_never_a_second_parsing_of_its_name() {
    use purlis_core::skills::Origin;
    let (runs, _told) = listening();
    // opencode is handed the built-in `secrets` as `purlis-secrets` (#3279).
    runs.heard(&call_of(4, "purlis-secrets", Origin::BuiltIn));
    // A plane skill whose own name starts so is its own.
    runs.heard(&call_of(5, "purlis-notes", Origin::Project));
    // Claude Code starts a built-in under purlis's plugin.
    runs.heard(&call_of(6, "purlis:secrets", Origin::BuiltIn));

    let named: Vec<_> = runs
        .now([4, 5, 6])
        .into_iter()
        .filter_map(|told| told.run.map(|run| (run.skill, run.name)))
        .collect();
    assert_eq!(
        named,
        [
            ("purlis-secrets".to_owned(), "secrets".to_owned()),
            ("purlis-notes".to_owned(), "purlis-notes".to_owned()),
            ("purlis:secrets".to_owned(), "secrets".to_owned()),
        ]
    );
}

/// SK-16 (#2779): a grade watching a chat is told each skill that starts in it once, and the end
/// of its turn; a chat it does not watch tells it nothing.
#[test]
fn a_grade_watching_a_chat_is_told_each_skill_it_started_and_the_end_of_its_turn() {
    let (runs, _told) = listening();
    let watched = runs.watch(4);

    runs.heard(&call(5, "toolu_0", Some("other")));
    runs.heard(&call(4, "toolu_1", None));
    runs.heard(&call(4, "toolu_2", Some("deploy")));
    runs.heard(&call(4, "toolu_3", None));
    runs.reported(&report(4, Event::Stop));
    runs.unwatch(4);
    runs.heard(&call(4, "toolu_4", Some("deploy")));

    assert_eq!(
        watched.try_iter().collect::<Vec<_>>(),
        vec![Watched::Started("deploy".to_owned()), Watched::TurnEnded],
    );
}

/// SK-16 (the review of #3229): a grade watching a chat is told when it asks the person, by a
/// call put to them or by a notice that it waits.
#[test]
fn a_grade_watching_a_chat_is_told_when_it_asks_the_person() {
    let (runs, _told) = listening();
    let watched = runs.watch(4);
    let mut asked = call(4, "toolu_1", None);
    asked.decision = Decision::Ask;
    runs.heard(&asked);
    runs.reported(&report(4, Event::Notification));
    assert_eq!(
        watched.try_iter().collect::<Vec<_>>(),
        vec![Watched::Asked, Watched::Asked],
    );
}

/// SK-15e (#3233): a skill the chat loaded from the index through purlis's `skill_load` is a
/// skill started, as one its harness's own Skill tool started is, so a grade's trigger test
/// counts it as triggered.
#[test]
fn a_grade_watching_a_chat_is_told_of_a_skill_loaded_through_skill_load() {
    let (runs, _told) = listening();
    let watched = runs.watch(4);
    let mut load = call(4, "toolu_1", Some("deploy"));
    load.tool_hook = purlis_core::hookwire::SKILL_LOAD.to_owned();
    load.tool = Some(purlis_core::skilltools::SKILL_LOAD.to_owned());
    if let Some(started) = load.skill.as_mut() {
        started.started_by = purlis_core::skills::runs::StartedBy::McpLoad;
    }

    runs.heard(&load);

    assert_eq!(
        watched.try_iter().collect::<Vec<_>>(),
        vec![Watched::Started("deploy".to_owned())],
    );
}

/// SK-26 (#3694): the chip counts the tokens the chat spent since its run opened, from what its
/// harness says, as each tool call inside the run is heard; where the harness says none it shows
/// none; and the run's level goes with it, for the studio's scope.
#[test]
fn an_open_runs_tokens_rise_from_its_start_and_its_level_goes_with_it() {
    let (runs, told) = listening();
    let spent = Arc::new(Mutex::new(Some(1_000_u64)));
    runs.count_tokens_by({
        let spent = Arc::clone(&spent);
        Arc::new(move |chat| {
            if chat == 4 {
                *spent.lock().unwrap()
            } else {
                None
            }
        })
    });

    runs.heard(&call_of(4, "deploy", purlis_core::skills::Origin::Persona));
    *spent.lock().unwrap() = Some(4_500);
    runs.heard(&call(4, "toolu_2", None));
    *spent.lock().unwrap() = Some(12_000);
    runs.heard(&call(4, "toolu_3", None));
    runs.reported(&report(4, Event::Stop));

    let said: Vec<_> = told.try_iter().map(|t| t.run).collect();
    let tokens: Vec<_> = said
        .iter()
        .map(|run| run.as_ref().map(|run| run.tokens))
        .collect();
    assert_eq!(
        tokens,
        vec![Some(Some(0)), Some(Some(3_500)), Some(Some(11_000)), None]
    );
    assert_eq!(
        said[0]
            .as_ref()
            .and_then(|run| run.level.clone())
            .as_deref(),
        Some("persona")
    );
    assert_eq!(runs.now([4]), Vec::new(), "the turn's end closed it");

    // A chat whose harness says no tokens shows none, never a zero.
    runs.heard(&call(5, "toolu_9", Some("deploy")));
    let five: Vec<_> = told.try_iter().map(|t| t.run).collect();
    assert_eq!(five.len(), 1);
    assert_eq!(five[0].as_ref().map(|run| run.tokens), Some(None));
    assert_eq!(
        runs.now([5])[0].run.as_ref().map(|run| run.tokens),
        Some(None)
    );
}

/// #3694 review: a run first seen before its harness said any usage counts from the first figure
/// it says, rather than never.
#[test]
fn a_run_seen_before_any_usage_counts_from_the_first_figure() {
    let (runs, told) = listening();
    let spent = Arc::new(Mutex::new(None::<u64>));
    runs.count_tokens_by({
        let spent = Arc::clone(&spent);
        Arc::new(move |_| *spent.lock().unwrap())
    });

    runs.heard(&call(4, "toolu_1", Some("deploy")));
    *spent.lock().unwrap() = Some(2_000);
    runs.heard(&call(4, "toolu_2", None));
    *spent.lock().unwrap() = Some(2_600);
    runs.heard(&call(4, "toolu_3", None));

    let tokens: Vec<_> = told
        .try_iter()
        .map(|t| t.run.and_then(|run| run.tokens))
        .collect();
    assert_eq!(tokens, vec![None, Some(0), Some(600)]);
}

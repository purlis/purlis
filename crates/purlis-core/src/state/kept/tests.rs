// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::sync::{Arc, Mutex};

use super::*;
use crate::harness::Harness;
use crate::hookwire::{Conversation, Report, ToolCall};
use crate::state::{Board, Event, FailedTask, HowFailed, Need, State, Waits};

const CONVERSATION: &str = "0199a2f4-1c2b-7f00-8000-000000000001";
const PID: u32 = 4242;

/// A report of chat `chat`'s own harness, a Claude Code started under [`CONVERSATION`].
fn its_own(chat: u32, event: Event) -> Report {
    Report {
        chat,
        event,
        conversation: Conversation::Named(CONVERSATION.into()),
        pid: Some(PID),
        agent: None,
        detail: crate::state::Detail::default(),
    }
}

/// What a reader of chat `number` sees on `board`: its state, whether it is in the queue, its
/// reasons and rows, and the queue itself. Not when it last moved, which is this process's
/// count.
fn seen(board: &Board, number: u32) -> String {
    format!(
        "{:?} queue={:?} asking={} held={} reports={:?} stopped={:?} needs={:?} refusals={:?} \
         failed={:?} children={:?} turns={} conversation={:?}",
        board.state(number),
        board.needs_you(),
        board.asking(number),
        board.held(number),
        board.reports(number),
        board.stopped_of(number),
        board.needs_of(number),
        board.refusals(number),
        board.failed_tasks(number),
        board.children(number),
        board.turns(number),
        board.conversation(number),
    )
}

/// A board told every move `window` makes, as the session host keeps one (#2194).
fn kept_beside(window: &mut Board) -> Arc<Mutex<Board>> {
    let host = Arc::new(Mutex::new(Board::new()));
    let keeping = Arc::clone(&host);
    window.told_to(Arc::new(move |number, moved: &Move| {
        keeping.lock().unwrap().make(number, moved.clone());
    }));
    host
}

#[test]
fn a_board_told_every_move_of_another_reads_as_that_one_does() {
    let mut window = Board::new();
    let host = kept_beside(&mut window);

    window.opened(1, Some(Harness::ClaudeCode), Some(CONVERSATION.into()));
    window.opened(2, Some(Harness::ClaudeCode), Some(CONVERSATION.into()));
    window.reported(&its_own(1, Event::UserPromptSubmit));
    window.reported_while(&its_own(1, Event::Stop), Waits::default());
    window.reported(&its_own(1, Event::UserPromptSubmit));
    window.reported(&its_own(1, Event::Notification));
    assert!(window.answered_by(1, &crate::harness::hooked::Asker::Chat));
    window.reported(&its_own(1, Event::Stop));
    window.reported(&its_own(2, Event::UserPromptSubmit));
    window.reported_while(
        &its_own(2, Event::Stop),
        Waits {
            tasks: 1,
            ..Waits::default()
        },
    );
    window.reported_back(2, "the task");
    window.stopped_below(2, "another task");
    window.task_failed(
        2,
        FailedTask::new("task-1", "the task", HowFailed::Failed, "it broke"),
    );
    window.commit_refused(1, "a key in config.toml");
    window.needs(1, Need::RootUpdateRefused);
    window.child_heard(1, "helper-1");
    window.ignored(1);
    window.exited(2, Some(0));

    for number in [1, 2] {
        assert_eq!(seen(&host.lock().unwrap(), number), seen(&window, number));
    }
    assert_eq!(window.state(2), State::Done);

    window.closed(1);
    assert_eq!(host.lock().unwrap().needs_you(), window.needs_you());
    assert_eq!(host.lock().unwrap().state(1), State::Unknown);
}

#[test]
fn a_board_that_takes_another_whole_reads_as_that_one_does_and_moves_on_from_it() {
    let mut host = Board::new();
    host.opened(1, Some(Harness::ClaudeCode), Some(CONVERSATION.into()));
    host.opened(3, Some(Harness::Codex), None);
    host.reported(&its_own(1, Event::UserPromptSubmit));
    host.reported(&its_own(1, Event::Stop));
    host.commit_refused(1, "a key in config.toml");
    host.reported_back(3, "the task");

    // Across the link, as JSON.
    let kept: Kept =
        serde_json::from_value(serde_json::to_value(host.kept()).expect("said")).expect("read");
    let mut window = Board::new();
    window.opened(9, None, None);
    window.keep(kept);

    assert_eq!(window.chats().collect::<Vec<_>>(), vec![1, 3]);
    for number in [1, 3, 9] {
        assert_eq!(seen(&window, number), seen(&host, number));
    }
    assert_eq!(window.needs_you(), vec![1]);
    // Where the host's count stood, so a move made here is later than every one it kept.
    assert_eq!(window.moved_at(1), host.moved_at(1));
    assert!(window.moved_at(3) > 0);
    // The chat's own harness is still the one it adopted: another process's moves nothing.
    let nested = Report {
        pid: Some(PID + 1),
        ..its_own(1, Event::UserPromptSubmit)
    };
    assert!(!window.reported(&nested));
    assert!(window.reported(&its_own(1, Event::UserPromptSubmit)));
    assert_eq!(window.state(1), State::Running);
    assert!(window.moved_at(1) > host.moved_at(3));
}

/// A tool call of chat `chat`'s, as its hook sends one.
fn a_tool_call(chat: u32, decision: crate::hookwire::Decision, rule: Option<&str>) -> ToolCall {
    ToolCall {
        chat,
        tool_hook: "pretooluse".into(),
        tool: Some("Bash".into()),
        call: None,
        args: None,
        decision,
        rule: rule.map(str::to_owned),
        hook_ms: 1,
        agent: None,
        at_ms: 0,
        skill: None,
    }
}

#[test]
fn with_no_window_the_host_moves_its_board_by_what_the_lines_say() {
    use crate::hookwire::relay::Heard;
    use crate::hookwire::{CommitRefused, Decision, Doing};

    let mut board = Board::new();
    board.opened(1, Some(Harness::ClaudeCode), Some(CONVERSATION.into()));

    assert!(board.hear(&Heard::Report(its_own(1, Event::UserPromptSubmit))));
    assert!(board.hear(&Heard::Report(its_own(1, Event::Stop))));
    // With nobody to say what it waits on, the end of its turn is the person's.
    assert_eq!(board.state(1), State::Waiting);
    assert_eq!(board.needs_you(), vec![1]);

    assert!(board.hear(&Heard::Refused(CommitRefused {
        chat: 1,
        commit_refused: "a key in config.toml".into(),
    })));
    assert_eq!(board.refusals(1), vec!["a key in config.toml".to_owned()]);

    // A refused root update is the chat's to retry with `purlis worktree update` (#3877): it
    // raises nothing for the person, and neither does any other refusal.
    for rule in [crate::rootupdate::REASON, "another"] {
        assert!(!board.hear(&Heard::Tool(a_tool_call(1, Decision::Deny, Some(rule)))));
    }
    assert_eq!(board.needs_of(1), Vec::<Need>::new());

    assert!(board.hear(&Heard::Tool(ToolCall {
        agent: Some("helper-1".into()),
        ..a_tool_call(1, Decision::Allow, None)
    })));
    assert_eq!(board.children(1).len(), 1);

    // The chat asks mid-turn, and a tool of its own run says it got past the prompt.
    board.hear(&Heard::Report(its_own(1, Event::UserPromptSubmit)));
    board.hear(&Heard::Report(its_own(1, Event::Notification)));
    assert!(board.waits_on_its_prompt(1));
    let speaker = crate::hookwire::Speaker {
        pid: Some(PID),
        conversation: Conversation::Named(CONVERSATION.into()),
    };
    let tool = |doing| {
        Heard::Doing(Doing {
            chat: 1,
            doing,
            agent: None,
            speaker: speaker.clone(),
        })
    };
    board.hear(&tool(crate::doing::Said::Began {
        kind: crate::doing::Kind::Command,
        name: None,
    }));
    assert!(board.hear(&tool(crate::doing::Said::Ended { kind: None })));
    assert!(!board.waits_on_its_prompt(1));

    // A line that moves no board.
    assert!(!board.hear(&Heard::AskerGone { connection: 3 }));
}

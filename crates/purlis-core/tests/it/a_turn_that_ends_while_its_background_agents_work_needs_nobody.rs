// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A Claude Code chat whose turn ends while background work of its own is in flight is not
//! waiting on the person (#1626): it is **waiting** on that work (#3878), and Claude Code wakes
//! it when the work is done or due.
//!
//! Replayed here as the hooks of such a chat reach the app: each payload read the way a hook
//! reads it ([`Report::read`]) and applied to the board the window draws from. The payloads
//! are shaped as Claude Code 2.1.296's hook input schema says: its `Stop` input carries
//! `background_tasks`, the in-flight background work of the session, each entry with a `type`
//! (`subagent`, `shell`, `monitor`, `workflow`, ...), and an empty array when nothing is in
//! flight; 2.1.312's carries `session_crons` beside it, each with a cron `schedule` and whether
//! it is `recurring`.

use purlis_core::harness::Harness;
use purlis_core::hookwire::{
    CHAT_ENV, CLAUDE_CONVERSATION_ENV, CLAUDE_PID_ENV, HARNESS_ENV, Report, SOCKET_ENV,
};
use purlis_core::state::{Board, Event, State};

const SESSION: &str = "11111111-2222-4333-8444-555555555555";
const CHAT: u32 = 7;

/// The environment a hook of chat [`CHAT`]'s own Claude Code runs in.
fn env(name: &str) -> Option<String> {
    match name {
        SOCKET_ENV => Some("/tmp/s.sock".to_owned()),
        CHAT_ENV => Some(CHAT.to_string()),
        CLAUDE_CONVERSATION_ENV => Some(SESSION.to_owned()),
        CLAUDE_PID_ENV => Some("4242".to_owned()),
        HARNESS_ENV => Some("claude-code".to_owned()),
        _ => None,
    }
}

/// A board with chat [`CHAT`] open on Claude Code, in a turn.
fn in_a_turn() -> Board {
    let mut board = Board::new();
    board.opened(CHAT, Some(Harness::ClaudeCode), Some(SESSION.to_owned()));
    hear(
        &mut board,
        Event::SessionStart,
        serde_json::json!({"source": "startup"}),
    );
    hear(
        &mut board,
        Event::UserPromptSubmit,
        serde_json::json!({"prompt": "review it"}),
    );
    assert_eq!(board.state(CHAT), State::Running);
    board
}

/// The hook of `event` heard with `payload`, as Claude Code hands it over.
fn hear(board: &mut Board, event: Event, mut payload: serde_json::Value) {
    payload["session_id"] = SESSION.into();
    payload["hook_event_name"] = event.word().into();
    let report = Report::read(event, &payload.to_string(), &env).expect("a report");
    // Applied as the app applies it: with its clock (#3878).
    board.reported_while(
        &report,
        purlis_core::state::Waits {
            now: now(),
            ..Default::default()
        },
    );
}

/// A `Stop` whose `background_tasks` lists `tasks`, as `(type, status)`; `None` leaves the
/// field out, as a Claude Code older than it does.
fn stop(board: &mut Board, tasks: Option<&[(&str, &str)]>) {
    stop_with_crons(board, tasks, &[]);
}

/// [`stop`], with `session_crons` listing `crons` as `(schedule, recurring)`.
fn stop_with_crons(board: &mut Board, tasks: Option<&[(&str, &str)]>, crons: &[(&str, bool)]) {
    let mut payload = serde_json::json!({
        "stop_hook_active": false,
        "last_assistant_message": "Waiting for the background agents to finish.",
    });
    if let Some(tasks) = tasks {
        payload["background_tasks"] = tasks
            .iter()
            .enumerate()
            .map(|(n, (kind, status))| {
                serde_json::json!({
                    "id": format!("t{n}"),
                    "type": kind,
                    "status": status,
                    "description": "Review one module",
                    "agent_type": "general-purpose",
                })
            })
            .collect();
    }
    if !crons.is_empty() {
        payload["session_crons"] = crons
            .iter()
            .enumerate()
            .map(|(n, (schedule, recurring))| {
                serde_json::json!({
                    "id": format!("c{n}"),
                    "schedule": schedule,
                    "recurring": recurring,
                    "prompt": "check the deploy",
                })
            })
            .collect();
    }
    hear(board, Event::Stop, payload);
}

/// Now on the machine's clock, in seconds since 1970.
fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

/// A five-field cron schedule that fires once a day, `minutes` from now on the local clock.
fn daily_in(minutes: i64) -> String {
    use chrono::Timelike;
    let at = chrono::Local::now() + chrono::Duration::minutes(minutes);
    format!("{} {} * * *", at.minute(), at.hour())
}

/// A one-shot wake-up's schedule, as Claude Code writes one: the minute `minutes` from now on
/// the local clock, on its day of its month.
fn once_in(minutes: i64) -> String {
    use chrono::{Datelike, Timelike};
    let at = chrono::Local::now() + chrono::Duration::minutes(minutes);
    format!(
        "{} {} {} {} *",
        at.minute(),
        at.hour(),
        at.day(),
        at.month()
    )
}

/// `n` background agents still running, as `stop` takes them.
fn agents(n: usize) -> Vec<(&'static str, &'static str)> {
    vec![("subagent", "running"); n]
}

#[test]
fn a_turn_that_ends_while_four_background_agents_work_is_waiting_on_them() {
    purlis_core::unsteered!();
    let mut board = in_a_turn();

    stop(&mut board, Some(&agents(4)));

    assert!(
        board.needs_you().is_empty(),
        "the chat was put in the queue"
    );
    assert_eq!(board.state(CHAT), State::Waiting);
    let work = board.own_work(CHAT).expect("waiting on its own work");
    assert_eq!((work.helpers, work.until), (4, None));
}

#[test]
fn it_needs_the_person_once_its_agents_are_done_and_it_stops_with_nothing_in_flight() {
    purlis_core::unsteered!();
    let mut board = in_a_turn();
    stop(&mut board, Some(&agents(4)));
    for agent in ["a1", "a2", "a3", "a4"] {
        hear(
            &mut board,
            Event::SubagentStop,
            serde_json::json!({"agent_id": agent, "agent_type": "general-purpose"}),
        );
        assert!(
            board.needs_you().is_empty(),
            "a helper's end was the chat's"
        );
    }

    // Claude Code wakes the chat with what its agents said, and that turn ends with nothing
    // in flight: the next move is the person's.
    stop(&mut board, Some(&[]));

    assert_eq!(board.needs_you(), vec![CHAT]);
    assert_eq!(board.state(CHAT), State::Waiting);
}

#[test]
fn a_question_while_its_agents_work_still_needs_the_person() {
    purlis_core::unsteered!();
    let mut board = in_a_turn();
    stop(&mut board, Some(&agents(2)));

    hear(
        &mut board,
        Event::Notification,
        serde_json::json!({"notification_type": "permission_prompt", "message": "Claude needs your permission to use Bash"}),
    );

    assert_eq!(board.needs_you(), vec![CHAT]);
    assert!(board.waits_on_its_prompt(CHAT));
}

#[test]
fn a_prompt_a_helper_shows_when_the_turn_ends_stays_in_front_of_the_person() {
    purlis_core::unsteered!();
    let mut board = in_a_turn();
    hear(
        &mut board,
        Event::Notification,
        serde_json::json!({"notification_type": "permission_prompt", "message": "Claude needs your permission to use Bash"}),
    );

    stop(&mut board, Some(&agents(1)));

    assert_eq!(board.needs_you(), vec![CHAT]);
    assert!(board.waits_on_its_prompt(CHAT), "the prompt's Notice went");
}

#[test]
fn a_chat_its_agents_never_wake_needs_the_person_once_it_sits_idle_and_asks_nothing() {
    purlis_core::unsteered!();
    // Its agents were stopped, and no turn of its own followed: Claude Code's idle nudge,
    // which waits for its background agents, is what is left to say it.
    let mut board = in_a_turn();
    stop(&mut board, Some(&agents(4)));

    hear(
        &mut board,
        Event::Notification,
        serde_json::json!({"notification_type": "idle_prompt", "message": "Claude is waiting for your input"}),
    );

    assert_eq!(board.needs_you(), vec![CHAT]);
    assert_eq!(board.state(CHAT), State::Waiting);
    assert!(
        !board.waits_on_its_prompt(CHAT),
        "an idle nudge read as a question"
    );
}

#[test]
fn monitors_and_workflows_hold_the_end_of_a_turn_with_no_limit() {
    purlis_core::unsteered!();
    for kind in ["monitor", "workflow"] {
        let mut board = in_a_turn();
        stop(&mut board, Some(&[(kind, "running"), (kind, "pending")]));
        assert!(
            board.needs_you().is_empty(),
            "{kind} was handed to the person"
        );
        assert_eq!(board.own_work_limits(), vec![], "{kind} has a limit");
        assert!(!board.own_work_lapsed(CHAT, now() + 86_400 * 365, Default::default()));
    }
}

#[test]
fn a_background_shell_holds_the_end_of_a_turn_for_an_hour_on_the_app_s_clock() {
    purlis_core::unsteered!();
    let mut board = in_a_turn();
    let ended = now();
    stop(&mut board, Some(&[("shell", "running")]));

    assert!(board.needs_you().is_empty());
    let [(chat, until)] = board.own_work_limits()[..] else {
        panic!("one limit: {:?}", board.own_work_limits());
    };
    assert_eq!(chat, CHAT);
    assert!((ended + 3600..=now() + 3600).contains(&until), "{until}");

    // The chat never wakes: the app's clock ends the wait.
    assert!(!board.own_work_lapsed(CHAT, until - 1, Default::default()));
    assert!(board.needs_you().is_empty());
    assert!(board.own_work_lapsed(CHAT, until, Default::default()));
    assert_eq!(board.needs_you(), vec![CHAT]);
    assert_eq!(board.own_work(CHAT), None);
}

#[test]
fn a_recurring_cron_holds_the_end_of_a_turn_only_when_its_next_run_is_within_the_hour() {
    purlis_core::unsteered!();
    let mut board = in_a_turn();
    stop_with_crons(&mut board, Some(&[]), &[(&daily_in(20), true)]);
    assert!(board.needs_you().is_empty(), "a run in 20 minutes");
    let work = board.own_work(CHAT).expect("waiting");
    assert!(work.wakes_at.is_some_and(|at| at > now()), "{work:?}");

    let mut board = in_a_turn();
    stop_with_crons(&mut board, Some(&[]), &[(&daily_in(150), true)]);
    assert_eq!(board.needs_you(), vec![CHAT], "a run in 150 minutes");
}

#[test]
fn a_one_shot_wake_up_holds_the_end_of_a_turn_however_far_off() {
    purlis_core::unsteered!();
    let mut board = in_a_turn();
    stop_with_crons(&mut board, Some(&[]), &[(&once_in(300), false)]);
    assert!(board.needs_you().is_empty());
    // The wake-up starts the next turn, which ends with nothing in flight: the person's move.
    hear(
        &mut board,
        Event::UserPromptSubmit,
        serde_json::json!({"prompt": "check the deploy"}),
    );
    stop(&mut board, Some(&[]));
    assert_eq!(board.needs_you(), vec![CHAT]);
}

#[test]
fn a_permission_or_a_question_while_it_waits_on_a_shell_needs_the_person_at_once() {
    purlis_core::unsteered!();
    for kind in ["permission_prompt", "elicitation_dialog"] {
        let mut board = in_a_turn();
        stop(&mut board, Some(&[("shell", "running")]));
        hear(
            &mut board,
            Event::Notification,
            serde_json::json!({"notification_type": kind}),
        );
        assert_eq!(board.needs_you(), vec![CHAT], "{kind}");
        assert_eq!(board.own_work(CHAT), None, "{kind}");
    }
}

#[test]
fn an_idle_nudge_does_not_end_a_wait_on_a_monitor() {
    purlis_core::unsteered!();
    let mut board = in_a_turn();
    stop(
        &mut board,
        Some(&[("monitor", "running"), ("subagent", "running")]),
    );

    hear(
        &mut board,
        Event::Notification,
        serde_json::json!({"notification_type": "idle_prompt"}),
    );

    // No agent is in flight, as the nudge says: the monitor still is.
    assert!(board.needs_you().is_empty());
    let work = board.own_work(CHAT).expect("waiting on the monitor");
    assert_eq!((work.helpers, work.monitors), (0, 1));
}

#[test]
fn another_harness_s_background_work_is_not_read() {
    purlis_core::unsteered!();
    let mut board = in_a_turn();
    let mut payload = serde_json::json!({
        "session_id": SESSION,
        "background_tasks": [{"id": "t", "type": "monitor", "status": "running"}],
    });
    payload["hook_event_name"] = "stop".into();
    let codex = |name: &str| match name {
        HARNESS_ENV => Some("codex".to_owned()),
        _ => env(name),
    };
    let report = Report::read(Event::Stop, &payload.to_string(), &codex).expect("a report");
    assert!(report.detail.own_work.is_nothing());
    board.reported_while(
        &report,
        purlis_core::state::Waits {
            now: now(),
            ..Default::default()
        },
    );
    assert_eq!(board.needs_you(), vec![CHAT]);
}

#[test]
fn a_harness_that_does_not_say_what_is_in_flight_hands_the_person_its_turn_s_end() {
    purlis_core::unsteered!();
    let mut board = in_a_turn();

    stop(&mut board, None);

    assert_eq!(board.needs_you(), vec![CHAT]);
    assert_eq!(board.state(CHAT), State::Waiting);
}

#[test]
fn a_one_shot_wake_up_already_due_holds_the_end_of_a_turn_only_a_few_minutes() {
    purlis_core::unsteered!();
    let mut board = in_a_turn();
    // Due a minute ago: due now, not next year.
    stop_with_crons(&mut board, Some(&[]), &[(&once_in(-1), false)]);
    assert!(board.needs_you().is_empty());
    let [(_, until)] = board.own_work_limits()[..] else {
        panic!("one limit: {:?}", board.own_work_limits());
    };
    assert!(until <= now() + 300, "{until}");
    assert!(board.own_work_lapsed(CHAT, until, Default::default()));
    assert_eq!(board.needs_you(), vec![CHAT]);
}

#[test]
fn a_sandbox_block_heard_while_it_waits_on_a_monitor_needs_the_person_at_once() {
    purlis_core::unsteered!();
    let mut board = in_a_turn();
    stop(&mut board, Some(&[("monitor", "running")]));
    assert!(board.needs_you().is_empty());

    assert!(board.blocked_while_waiting(CHAT));

    assert_eq!(board.needs_you(), vec![CHAT]);
    assert_eq!(board.own_work(CHAT), None);
}

#[test]
fn a_turn_that_ends_on_a_monitor_after_a_sandbox_block_s_notice_is_the_person_s() {
    purlis_core::unsteered!();
    let mut board = in_a_turn();
    let mut payload = serde_json::json!({
        "session_id": SESSION,
        "background_tasks": [{"id": "t", "type": "monitor", "status": "running"}],
    });
    payload["hook_event_name"] = "stop".into();
    let report = Report::read(Event::Stop, &payload.to_string(), &env).expect("a report");
    board.reported_while(
        &report,
        purlis_core::state::Waits {
            on_a_notice: true,
            now: now(),
            ..Default::default()
        },
    );
    assert_eq!(board.needs_you(), vec![CHAT]);
    assert_eq!(board.own_work(CHAT), None);
}

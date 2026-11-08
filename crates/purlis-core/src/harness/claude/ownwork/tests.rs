// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

/// 14:00:30 on 2026-10-29, a Thursday, on the chat's own wall clock.
fn now() -> NaiveDateTime {
    NaiveDateTime::parse_from_str("2026-10-29 14:00:30", "%Y-%m-%d %H:%M:%S").expect("a time")
}

/// Seconds from [`now`] to `when` (`HH:MM` today, or `YYYY-MM-DD HH:MM`).
fn in_secs(when: &str) -> u32 {
    let full = if when.len() == 5 {
        format!("2026-10-29 {when}")
    } else {
        when.to_owned()
    };
    let at =
        NaiveDateTime::parse_from_str(&format!("{full}:00"), "%Y-%m-%d %H:%M:%S").expect("a time");
    u32::try_from((at - now()).num_seconds()).expect("ahead")
}

const LATE: u32 = 300;

/// A `Stop` payload with `tasks` as `(type, status)` and `crons` as `(schedule, recurring)`,
/// shaped as Claude Code 2.1.312's hook input schema says.
fn stop(tasks: &[(&str, &str)], crons: &[(&str, bool)]) -> serde_json::Value {
    serde_json::json!({
        "hook_event_name": "Stop",
        "background_tasks": tasks.iter().enumerate().map(|(n, (kind, status))| serde_json::json!({
            "id": format!("t{n}"), "type": kind, "status": status, "description": "work",
        })).collect::<Vec<_>>(),
        "session_crons": crons.iter().enumerate().map(|(n, (schedule, recurring))| serde_json::json!({
            "id": format!("c{n}"), "schedule": schedule, "recurring": recurring, "prompt": "check",
        })).collect::<Vec<_>>(),
    })
}

fn read_stop(tasks: &[(&str, &str)], crons: &[(&str, bool)]) -> Said {
    read(Some(&stop(tasks, crons)), now())
}

#[test]
fn each_kind_in_flight_is_counted_by_claude_code_s_label() {
    let said = read_stop(
        &[
            ("subagent", "running"),
            ("subagent", "pending"),
            ("workflow", "running"),
            ("monitor", "running"),
            ("shell", "running"),
        ],
        &[],
    );
    assert_eq!(
        (said.helpers, said.workflows, said.monitors, said.shells),
        (2, 1, 1, 1)
    );
}

#[test]
fn work_not_in_flight_and_kinds_not_known_count_as_nothing() {
    let said = read_stop(
        &[
            ("subagent", "completed"),
            ("monitor", "failed"),
            ("dream", "running"),
            ("MCP task", "running"),
        ],
        &[],
    );
    assert!(said.is_nothing(), "{said:?}");
}

#[test]
fn a_one_shot_wake_up_is_read_as_seconds_from_now_however_far_off() {
    let said = read_stop(&[], &[("30 18 29 10 *", false)]);
    assert_eq!(said.wakes_in, Some(in_secs("18:30")));
    assert_eq!(said.lapses_in, Some(in_secs("18:30") + LATE));
    let said = read_stop(&[], &[("0 9 30 10 *", false)]);
    assert_eq!(said.wakes_in, Some(in_secs("2026-10-30 09:00")));
}

#[test]
fn a_one_shot_wake_up_due_this_minute_or_overdue_is_due_now_not_next_year() {
    for schedule in ["0 14 29 10 *", "45 13 29 10 *", "30 14 28 10 *"] {
        let said = read_stop(&[], &[(schedule, false)]);
        assert_eq!(said.wakes_in, Some(0), "{schedule}");
        assert_eq!(said.lapses_in, Some(LATE), "{schedule}");
    }
}

#[test]
fn a_recurring_cron_counts_only_when_its_next_run_is_within_the_hour() {
    let soon = read_stop(&[], &[("*/20 * * * *", true)]);
    assert_eq!(soon.wakes_in, Some(in_secs("14:20")));
    assert!(read_stop(&[], &[("0 9 * * *", true)]).is_nothing());
    assert_eq!(
        read_stop(&[], &[("0 15 * * *", true)]).wakes_in,
        Some(in_secs("15:00"))
    );
    // A recurring run in the minute the turn ended in has fired already.
    assert_eq!(
        read_stop(&[], &[("0,30 14 * * *", true)]).wakes_in,
        Some(in_secs("14:30"))
    );
}

#[test]
fn days_of_the_month_and_of_the_week_match_as_cron_has_them() {
    assert_eq!(
        read_stop(&[], &[("30 14 * * 1-5", true)]).wakes_in,
        Some(in_secs("14:30"))
    );
    assert!(read_stop(&[], &[("30 14 * * 0,7", true)]).is_nothing());
    assert_eq!(
        read_stop(&[], &[("30 14 1 * 4", true)]).wakes_in,
        Some(in_secs("14:30"))
    );
    assert_eq!(
        read_stop(&[], &[("0 8 * * 7", false)]).wakes_in,
        Some(in_secs("2026-11-01 08:00"))
    );
}

#[test]
fn a_schedule_this_cannot_read_counts_as_nothing() {
    for schedule in [
        "@hourly",
        "* * * *",
        "61 * * * *",
        "MON * * * *",
        "*/0 * * * *",
        "",
    ] {
        let said = read_stop(&[], &[(schedule, true)]);
        assert!(said.is_nothing(), "{schedule}: {said:?}");
    }
    let payload = serde_json::json!({"session_crons": [{"id": "c", "schedule": "*/5 * * * *"}]});
    assert!(read(Some(&payload), now()).is_nothing());
}

#[test]
fn an_unreadable_payload_waits_on_nothing() {
    assert!(read(None, now()).is_nothing());
    for payload in [
        serde_json::json!({}),
        serde_json::json!({"background_tasks": "lots", "session_crons": 3}),
        serde_json::json!({"background_tasks": [1, "x", {"type": "monitor"}]}),
    ] {
        assert!(read(Some(&payload), now()).is_nothing(), "{payload}");
    }
}

#[test]
fn what_it_reads_is_seconds_from_now_on_the_chat_s_clock_whatever_the_day() {
    // The same schedule read at the same wall time on any day says the same: no time zone and
    // no time of day leave the chat.
    let later = now() + Duration::days(40);
    let one = read(Some(&stop(&[], &[("*/20 * * * *", true)])), now());
    let other = read(Some(&stop(&[], &[("*/20 * * * *", true)])), later);
    assert_eq!(one, other);
}

// ---- each piece, as its tool starts it and as the `Stop` says it (#5750) --------------------

/// A `PostToolUse` payload of `tool`, called as `input`, that came back `response`.
fn post(tool: &str, input: serde_json::Value, response: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "hook_event_name": "PostToolUse",
        "tool_name": tool,
        "tool_use_id": "toolu_01ABC",
        "tool_input": input,
        "tool_response": response,
    })
}

fn output_in_tasks(id: &str) -> Option<String> {
    Some(format!("/tmp/claude-502/p/s1/tasks/{id}.output"))
}

fn no_output(_: &str) -> Option<String> {
    None
}

#[test]
fn a_command_moved_to_the_background_is_a_shell_with_its_output_file() {
    let piece = started(
        &post(
            "Bash",
            serde_json::json!({"command": "npm test", "run_in_background": true}),
            serde_json::json!({"stdout": "", "stderr": "", "interrupted": false,
                "backgroundTaskId": "b5ul9gyit"}),
        ),
        &output_in_tasks,
    )
    .expect("a piece");
    assert_eq!(
        (piece.kind, piece.id.as_str(), piece.what.as_str()),
        (Kind::Shell, "b5ul9gyit", "npm test")
    );
    assert_eq!(
        piece.output.as_deref(),
        Some("/tmp/claude-502/p/s1/tasks/b5ul9gyit.output")
    );
}

#[test]
fn a_command_that_finished_starts_nothing() {
    let payload = post(
        "Bash",
        serde_json::json!({"command": "ls"}),
        serde_json::json!({"stdout": "a", "stderr": "", "interrupted": false}),
    );
    assert_eq!(started(&payload, &output_in_tasks), None);
}

#[test]
fn a_monitor_a_background_agent_and_a_workflow_are_each_their_kind() {
    let monitor = started(
        &post(
            "Monitor",
            serde_json::json!({"description": "CI on #5790", "command": "gh run watch"}),
            serde_json::json!({"taskId": "m1", "timeoutMs": 0}),
        ),
        &no_output,
    )
    .expect("a monitor");
    assert_eq!(
        (monitor.kind, monitor.what.as_str()),
        (Kind::Monitor, "CI on #5790")
    );
    let agent = started(
        &post(
            "Agent",
            serde_json::json!({"description": "find the callers", "run_in_background": true}),
            serde_json::json!({"status": "async_launched", "agentId": "a7", "description":
                "find the callers", "outputFile": "/tmp/claude-502/p/s1/tasks/a7.output"}),
        ),
        &no_output,
    )
    .expect("a subagent");
    assert_eq!(
        (agent.kind, agent.output.as_deref()),
        (Kind::Subagent, Some("/tmp/claude-502/p/s1/tasks/a7.output"))
    );
    let workflow = started(
        &post(
            "Workflow",
            serde_json::json!({"script": "…"}),
            serde_json::json!({"status": "async_launched", "taskId": "w2",
                "workflowName": "review-changes"}),
        ),
        &output_in_tasks,
    )
    .expect("a workflow");
    assert_eq!(
        (workflow.kind, workflow.what.as_str()),
        (Kind::Workflow, "review-changes")
    );
}

#[test]
fn an_agent_that_ran_in_the_foreground_starts_nothing() {
    let payload = post(
        "Agent",
        serde_json::json!({"description": "x"}),
        serde_json::json!({"status": "completed", "content": []}),
    );
    assert_eq!(started(&payload, &no_output), None);
}

#[test]
fn a_cron_keeps_its_schedule_and_prompt_and_a_one_shot_is_a_wake_up() {
    let cron = started(
        &post(
            "CronCreate",
            serde_json::json!({"cron": "*/5 * * * *", "prompt": "check the queue"}),
            serde_json::json!({"id": "c9", "humanSchedule": "every 5 minutes", "recurring": true}),
        ),
        &no_output,
    )
    .expect("a cron");
    assert_eq!(
        (cron.kind, cron.schedule.as_deref(), cron.prompt.as_deref()),
        (Kind::Cron, Some("*/5 * * * *"), Some("check the queue"))
    );
    let once = started(
        &post(
            "CronCreate",
            serde_json::json!({"cron": "30 14 28 2 *", "prompt": "remind", "recurring": false}),
            serde_json::json!({"id": "c10", "recurring": false, "nextFireAt": 1_800_000_000_000_i64}),
        ),
        &no_output,
    )
    .expect("a wake-up");
    assert_eq!(
        (once.kind, once.due_at),
        (Kind::WakeUp, Some(1_800_000_000))
    );
}

#[test]
fn a_scheduled_wake_up_is_due_when_its_result_says() {
    let wake = started(
        &post(
            "ScheduleWakeup",
            serde_json::json!({"delaySeconds": 600, "prompt": "/loop check", "reason": "CI"}),
            serde_json::json!({"scheduledFor": 1_800_000_600_000_i64, "clampedDelaySeconds": 600,
                "wasClamped": false}),
        ),
        &no_output,
    )
    .expect("a wake-up");
    assert_eq!(
        (
            wake.kind,
            wake.id.as_str(),
            wake.due_at,
            wake.prompt.as_deref()
        ),
        (
            Kind::WakeUp,
            "toolu_01ABC",
            Some(1_800_000_600),
            Some("/loop check")
        )
    );
    let stopped = post(
        "ScheduleWakeup",
        serde_json::json!({"stop": true}),
        serde_json::json!({"scheduledFor": 0, "clampedDelaySeconds": 0, "wasClamped": false,
            "stopped": true}),
    );
    assert_eq!(started(&stopped, &no_output), None);
}

#[test]
fn a_stop_names_each_piece_in_flight_and_each_cron() {
    let payload = stop(
        &[("shell", "running"), ("subagent", "completed")],
        &[("*/5 * * * *", true), ("30 18 29 10 *", false)],
    );
    let epoch = 1_000_000;
    let said = pieces(Some(&payload), now(), epoch, &output_in_tasks).expect("pieces");
    let kinds: Vec<(Kind, &str)> = said.iter().map(|one| (one.kind, one.id.as_str())).collect();
    assert_eq!(
        kinds,
        [
            (Kind::Shell, "t0"),
            (Kind::Cron, "c0"),
            (Kind::WakeUp, "c1")
        ]
    );
    assert_eq!(
        said[0].output.as_deref(),
        Some("/tmp/claude-502/p/s1/tasks/t0.output")
    );
    assert_eq!(said[1].prompt.as_deref(), Some("check"));
    assert_eq!(said[2].due_at, Some(epoch + i64::from(in_secs("18:30"))));
}

#[test]
fn a_stop_that_names_neither_list_says_nothing_and_one_with_both_empty_says_none_runs() {
    let older = serde_json::json!({"hook_event_name": "Stop"});
    assert_eq!(pieces(Some(&older), now(), 1, &no_output), None);
    assert_eq!(
        pieces(Some(&stop(&[], &[])), now(), 1, &no_output),
        Some(Vec::new())
    );
}

#[test]
fn an_output_file_is_found_under_the_chat_s_own_temporary_folder() {
    let root = tempfile::tempdir().expect("a folder");
    let uid = rustix::process::getuid().as_raw();
    let tasks = root
        .path()
        .join(format!("claude-{uid}"))
        .join("-Users-me-proj")
        .join("0b7c-session")
        .join("tasks");
    std::fs::create_dir_all(&tasks).expect("made");
    std::fs::write(tasks.join("b1.output"), "hi\n").expect("written");
    let env = |name: &str| {
        (name == "CLAUDE_CODE_TMPDIR").then(|| root.path().to_str().expect("utf-8").to_owned())
    };
    assert_eq!(
        output_file("0b7c-session", "b1", &env).map(std::path::PathBuf::from),
        Some(tasks.join("b1.output"))
    );
    assert_eq!(output_file("0b7c-session", "b2", &env), None);
    assert_eq!(output_file("../x", "b1", &env), None);
}

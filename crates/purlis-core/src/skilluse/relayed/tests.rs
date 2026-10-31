// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

const RUNNER: &str = "01JB0RUNNER0000000000000000";
const CHAT: &str = "01K6W0RKER000000000000000B";
const RUN: &str = "01K6SKILLRUN00000000000000";
const PARENT: &str = "01K6PARENTRUN0000000000000";

fn line(kind: &str, body: Value) -> Event {
    Event {
        v: 1,
        device_id: RUNNER.into(),
        seq: 7,
        // 2026-10-31T10:00:00Z, as its time part says.
        ulid: ulid::Ulid::from_parts(1_793_440_800_000, 1).to_string(),
        chat: Some(CHAT.into()),
        run: Some("01K6CHATRUN000000000000000".into()),
        parent_run: None,
        kind: kind.into(),
        body,
    }
}

fn started() -> Event {
    line(
        "skill.started",
        serde_json::json!({
            "started_by": "skill-tool",
            "skill_level": "project",
            "routing": "01HZY3M6S4K9Q2W7E8R5T1Y6V0",
            "skill": "review",
            "skill_run": RUN,
        }),
    )
}

fn ended() -> Event {
    line(
        "skill.ended",
        serde_json::json!({
            "skill": "review",
            "skill_run": RUN,
            "parent_skill_run": PARENT,
            "ended": "turn",
            "ms": 1200,
            "tool_calls": 3,
            "tool_errors": 1,
            "refusals": 0,
            "asks": 2,
            "tokens": 900,
        }),
    )
}

fn a_plane() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a plane");
    std::fs::write(dir.path().join(crate::plane::MANIFEST), "").expect("its manifest");
    dir
}

// Seen red: with `logged` writing nothing, the plane's log held no run of the runner's chat.
#[test]
fn a_runners_skill_run_is_logged_in_the_desktops_plane_as_a_local_run_is_with_the_window_s_word_for_where_it_ran()
 {
    let plane = a_plane();
    let ran_as = RanAs {
        persona: Some("steward"),
        harness: Some("claude"),
        workspace: None,
    };
    for event in [started(), ended()] {
        let relayed = Relayed::of(&event).expect("a skill run's line");
        assert!(logged(plane.path(), &relayed, &ran_as).is_some());
    }

    let runs = crate::skilluse::runs(plane.path());
    assert_eq!(runs.len(), 1, "{runs:?}");
    let run = &runs[0];
    assert_eq!(run.id.as_deref(), Some(RUN));
    assert_eq!(run.skill, "review");
    assert_eq!(run.persona.as_deref(), Some("steward"));
    assert_eq!(run.harness.as_deref(), Some("claude"));
    assert_eq!(run.workspace, Some(None), "at the plane root");
    assert_eq!(run.device, RUNNER, "where it ran");
    assert_eq!(run.started_by, Some(StartedBy::SkillTool));
    assert_eq!(run.started, "2026-10-31T10:00:00+00:00");
    let end = run.end.as_ref().expect("its end");
    assert_eq!(end.ended, Ended::Turn);
    assert_eq!(
        (end.ms, end.tool_calls, end.tool_errors, end.asks),
        (1200, 3, 1, 2)
    );
    assert_eq!(end.tokens, Some(900));
    assert_eq!(end.parent.as_deref(), Some(PARENT));
}

#[test]
fn only_a_skill_runs_line_with_ids_a_run_has_is_read() {
    assert_eq!(Relayed::of(&line("run.ended", serde_json::json!({}))), None);
    // An id that names a path, a device among them, is no run's.
    let mut elsewhere = started();
    elsewhere.device_id = "../../escaped".into();
    assert_eq!(Relayed::of(&elsewhere), None);
    let mut long = started();
    long.body["skill_run"] = "x".repeat(MOST_ID + 1).into();
    assert_eq!(Relayed::of(&long), None);
    let mut no_chat = started();
    no_chat.chat = None;
    assert_eq!(Relayed::of(&no_chat), None);
    for skill in [String::new(), "x".repeat(MOST_SKILL + 1), "a\nb".to_owned()] {
        let mut odd = started();
        odd.body["skill"] = skill.into();
        assert_eq!(Relayed::of(&odd), None);
    }
    // A word this purlis does not know is read as unknown, never as another's.
    let mut newer = started();
    newer.body["started_by"] = "a-newer-route".into();
    newer.body["skill_level"] = "a-newer-level".into();
    let Some(Relayed {
        line: Line::Started {
            started_by, level, ..
        },
        ..
    }) = Relayed::of(&newer)
    else {
        panic!("a start");
    };
    assert_eq!((started_by, level), (StartedBy::Unknown, None));
}

#[test]
fn a_runners_skill_run_is_logged_in_no_project_this_purlis_may_not_write() {
    let not_a_plane = tempfile::tempdir().expect("a folder");
    let relayed = Relayed::of(&started()).expect("a start");
    assert_eq!(
        logged(not_a_plane.path(), &relayed, &RanAs::default()),
        None
    );
    assert!(crate::skilluse::runs(not_a_plane.path()).is_empty());
}

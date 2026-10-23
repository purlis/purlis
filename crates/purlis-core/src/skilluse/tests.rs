// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use purlis_skills::runs::{Call, Heard, Spans, Start};

const RUN: &str = "01JB0000000000000000000RUN";

fn may() -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::parse_from_rfc3339("2026-05-04T11:32:17+00:00")
        .unwrap()
        .with_timezone(&chrono::Utc)
}

fn a_plane() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(crate::plane::MANIFEST), "schema = 2\n").unwrap();
    dir
}

/// A run of `skill`, opened in a span of its own.
fn opened(spans: &mut Spans, skill: &str) -> Open {
    spans
        .open(
            "chat-run",
            Start {
                skill,
                id: RUN,
                started_by: StartedBy::SkillTool,
                whose: &purlis_skills::runs::Whose::default(),
                at_ms: 1_000,
                tokens: None,
            },
        )
        .cloned()
        .unwrap()
}

#[test]
fn a_skill_runs_start_is_logged_with_the_persona_or_null() {
    let dir = a_plane();
    let when = may();
    let mut spans = Spans::default();
    let p = started(
        dir.path(),
        &Starting::from(&opened(&mut spans, "charter:persona")),
        Some("ops"),
        when,
        "box",
    )
    .unwrap();
    started(
        dir.path(),
        &Starting::from(&opened(&mut Spans::default(), "x")),
        None,
        when,
        "box",
    )
    .unwrap();
    assert_eq!(
        started(
            dir.path(),
            &Starting::from(&opened(&mut Spans::default(), " ")),
            None,
            when,
            "box"
        ),
        None
    );
    assert_eq!(
        std::fs::read_to_string(p).unwrap(),
        "{\"persona\": \"ops\", \"run\": \"01JB0000000000000000000RUN\", \"skill\": \
         \"charter:persona\", \"started_by\": \"skill-tool\", \"ts\": \
         \"2026-05-04T11:32:17+00:00\"}\n{\"persona\": null, \"run\": \
         \"01JB0000000000000000000RUN\", \"skill\": \"x\", \"started_by\": \"skill-tool\", \
         \"ts\": \"2026-05-04T11:32:17+00:00\"}\n"
    );
}

#[test]
fn no_skill_run_is_logged_in_a_project_this_purlis_may_not_write() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join(crate::plane::MANIFEST),
        "schema = 2\nrequires = [{ feature = \"from-the-future\", since = \"9.0.0\" }]\n",
    )
    .unwrap();
    let open = opened(&mut Spans::default(), "charter:persona");
    assert_eq!(
        started(
            dir.path(),
            &Starting::from(&open),
            Some("ops"),
            chrono::Utc::now(),
            "box"
        ),
        None
    );
    assert!(!dir.path().join("personas").exists());
}

#[test]
fn a_runs_start_and_end_read_back_as_one_run_with_its_signals_and_never_a_verdict() {
    let dir = a_plane();
    let when = may();
    let mut spans = Spans::default();
    let open = opened(&mut spans, "deploy");
    started(dir.path(), &Starting::from(&open), Some("ops"), when, "box").unwrap();
    spans.heard(
        "chat-run",
        Heard {
            call: Some(Call::Id("toolu_1")),
            refused: true,
            ..Heard::default()
        },
    );
    let closed = spans.close("chat-run", Ended::Turn, 4_500);
    ended(dir.path(), &closed[0], Some(1200), when, "box").unwrap();

    let runs = runs(dir.path());

    assert_eq!(
        runs,
        vec![Run {
            id: Some(RUN.to_owned()),
            skill: "deploy".to_owned(),
            persona: Some("ops".to_owned()),
            started_by: Some(StartedBy::SkillTool),
            started: "2026-05-04T11:32:17+00:00".to_owned(),
            device: "box".to_owned(),
            copy: None,
            harness: None,
            workspace: None,
            end: Some(End {
                ended: Ended::Turn,
                at: "2026-05-04T11:32:17+00:00".to_owned(),
                ms: 3_500,
                tool_calls: 1,
                tool_errors: 0,
                refusals: 1,
                asks: 0,
                tokens: Some(1200),
                parent: None,
            }),
        }]
    );
}

#[test]
fn a_row_written_before_runs_were_recorded_reads_as_a_run_of_its_own_with_no_start_or_signals() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("personas").join(DIR_NAME);
    std::fs::create_dir_all(&log).unwrap();
    std::fs::write(
        log.join("2026-03.laptop.jsonl"),
        "{\"persona\": \"ops\", \"skill\": \"charter:secrets\", \"ts\": \"2026-03-02T09:00:00+00:00\"}\n\
         {\"persona\": null, \"skill\": \"x\", \"ts\": \"2026-03-03T09:00:00+00:00\"}\n",
    )
    .unwrap();

    let runs = runs(dir.path());

    assert_eq!(runs.len(), 2, "{runs:#?}");
    assert_eq!(runs[0].skill, "x", "newest first");
    assert_eq!(runs[0].persona, None);
    assert_eq!(runs[1].skill, "charter:secrets");
    assert_eq!(runs[1].persona.as_deref(), Some("ops"));
    assert_eq!(runs[1].device, "laptop");
    for run in &runs {
        assert_eq!(
            (run.id.as_deref(), run.started_by, &run.end),
            (None, None, &None),
            "recorded before runs were: {run:#?}"
        );
    }
}

/// Logs a run of `skill` on `harness` in `workspace` (`None` at the plane root) by `persona`,
/// ended with `tool_calls` and `tokens` where `ended`.
fn logged(
    plane: &Path,
    id: &str,
    (harness, workspace, persona): (Option<&str>, Option<&str>, Option<&str>),
    end: Option<(u32, Option<u64>)>,
) {
    let mut spans = Spans::default();
    let open = spans
        .open(
            "chat-run",
            Start {
                skill: "deploy",
                id,
                started_by: StartedBy::SkillTool,
                whose: &purlis_skills::runs::Whose {
                    workspace: workspace.map(str::to_owned),
                    level: None,
                },
                at_ms: 0,
                tokens: None,
            },
        )
        .cloned()
        .unwrap();
    let starting = Starting {
        harness,
        ..Starting::from(&open)
    };
    started(plane, &starting, persona, may(), "box").unwrap();
    if let Some((calls, tokens)) = end {
        for n in 0..calls {
            let id = format!("toolu_{n}");
            spans.heard(
                "chat-run",
                Heard {
                    call: Some(Call::Id(&id)),
                    ..Heard::default()
                },
            );
        }
        let closed = spans.close("chat-run", Ended::Turn, 2_000);
        ended(plane, &closed[0], tokens, may(), "box").unwrap();
    }
}

#[test]
fn a_runs_start_says_its_harness_and_its_workspace_null_at_the_plane_root() {
    let dir = a_plane();
    logged(dir.path(), "01JB0000000000000000000001", (Some("codex"), None, None), None);
    logged(
        dir.path(),
        "01JB0000000000000000000002",
        (Some("claude"), Some("ops"), Some("steward")),
        None,
    );
    let mut runs = runs(dir.path());
    runs.sort_by(|a, b| a.id.cmp(&b.id));
    assert_eq!(runs[0].harness.as_deref(), Some("codex"));
    assert_eq!(runs[0].workspace, Some(None), "said: the plane root");
    assert_eq!(runs[1].harness.as_deref(), Some("claude"));
    assert_eq!(runs[1].workspace, Some(Some("ops".to_owned())));
}

#[test]
fn runs_are_totalled_by_harness_persona_and_workspace_with_their_signals_and_cost() {
    let dir = a_plane();
    logged(
        dir.path(),
        "01JB0000000000000000000001",
        (Some("claude"), Some("ops"), Some("steward")),
        Some((3, Some(1_000))),
    );
    logged(
        dir.path(),
        "01JB0000000000000000000002",
        (Some("claude"), None, None),
        Some((1, None)),
    );
    logged(
        dir.path(),
        "01JB0000000000000000000003",
        (Some("codex"), Some("ops"), Some("steward")),
        None,
    );
    // A row from before SK-20 says neither its harness nor its workspace.
    let log = path_for(dir.path(), may(), "box");
    let mut text = std::fs::read_to_string(&log).unwrap();
    text.push_str(
        "{\"persona\": \"steward\", \"skill\": \"deploy\", \"ts\": \"2026-03-02T09:00:00+00:00\"}\n",
    );
    std::fs::write(&log, text).unwrap();
    let runs = runs(dir.path());

    let by_harness = stats(&runs, By::Harness);
    let groups: Vec<(&Group, u64, u64)> = by_harness
        .iter()
        .map(|row| (&row.group, row.totals.runs, row.totals.ended))
        .collect();
    assert_eq!(
        groups,
        [
            (&Group::Named("claude".to_owned()), 2, 2),
            (&Group::Named("codex".to_owned()), 1, 0),
            (&Group::NotRecorded, 1, 0),
        ]
    );
    let claude = by_harness[0].totals;
    assert_eq!(
        (claude.tool_calls, claude.tokens, claude.tokens_said, claude.ms),
        (4, 1_000, 1, 4_000)
    );

    let by_persona: Vec<(Group, u64)> = stats(&runs, By::Persona)
        .into_iter()
        .map(|row| (row.group, row.totals.runs))
        .collect();
    assert_eq!(
        by_persona,
        [(Group::Named("steward".to_owned()), 3), (Group::None, 1)]
    );

    let by_workspace: Vec<(Group, u64)> = stats(&runs, By::Workspace)
        .into_iter()
        .map(|row| (row.group, row.totals.runs))
        .collect();
    assert_eq!(
        by_workspace,
        [
            (Group::Named("ops".to_owned()), 2),
            (Group::None, 1),
            (Group::NotRecorded, 1),
        ]
    );
}

#[test]
fn a_group_is_said_by_its_kind_and_name() {
    assert_eq!(
        serde_json::to_value(Group::Named("ops".to_owned())).unwrap(),
        serde_json::json!({"kind": "named", "name": "ops"})
    );
    assert_eq!(
        serde_json::to_value(Group::NotRecorded).unwrap(),
        serde_json::json!({"kind": "not-recorded"})
    );
}
